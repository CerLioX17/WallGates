#![cfg_attr(not(windows), allow(dead_code))]
//! "Advanced sensors": the optional Wallgates Sensors helper (LibreHardwareMonitor
//! + PawnIO) shipped inside the installer. Enabling it takes one administrator
//! prompt; disabling it — or uninstalling Wallgates — removes it again.

use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const TASK_NAME: &str = "Wallgates Sensors";
#[cfg_attr(not(windows), allow(dead_code))]
const PAWNIO_URL: &str = "https://github.com/namazso/PawnIO.Setup/releases/latest/download/PawnIO_setup.exe";

#[derive(Deserialize, Clone, Debug)]
pub struct LhmReading {
    pub hw: String,
    #[serde(rename = "hwType")]
    pub hw_type: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub value: f64,
}

#[derive(Deserialize)]
struct LhmFile {
    time: u64,
    readings: Vec<LhmReading>,
}

pub fn json_path(data_dir: &Path) -> PathBuf {
    data_dir.join("sensors.json")
}

/// Latest readings written by the helper (ignored when older than 15 s).
pub fn read(data_dir: &Path) -> Option<Vec<LhmReading>> {
    let bytes = std::fs::read(json_path(data_dir)).ok()?;
    let file: LhmFile = serde_json::from_slice(&bytes).ok()?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    (now.saturating_sub(file.time) <= 15).then_some(file.readings)
}

/// Values the system feed needs, distilled from LibreHardwareMonitor sensors.
#[derive(Default, Debug)]
pub struct Summary {
    pub cpu_temp: Option<f64>,
    pub cpu_power: Option<f64>,
    pub ram_temp: Option<f64>,
    pub gpu_temp: Option<f64>,
    pub gpu_hotspot: Option<f64>,
    pub gpu_power: Option<f64>,
    pub storage: Vec<(String, f64)>,
    pub fans: Vec<(String, f64)>,
}

fn max(it: impl Iterator<Item = f64>) -> Option<f64> {
    it.filter(|v| v.is_finite()).fold(None, |m, v| Some(m.map_or(v, |m: f64| m.max(v))))
}

fn temps<'a>(r: &'a [LhmReading], hw: fn(&str) -> bool) -> impl Iterator<Item = &'a LhmReading> + 'a {
    r.iter().filter(move |x| x.kind == "Temperature" && hw(&x.hw_type) && x.value > 0.0 && x.value < 150.0)
}

fn named(r: &[LhmReading], hw: fn(&str) -> bool, kind: &str, names: &[&str]) -> Option<f64> {
    names.iter().find_map(|n| r.iter().find(|x| x.kind == kind && hw(&x.hw_type) && x.name.eq_ignore_ascii_case(n)).map(|x| x.value))
}

fn is_cpu(t: &str) -> bool {
    t == "Cpu"
}
fn is_gpu(t: &str) -> bool {
    t.starts_with("Gpu")
}
fn is_memory(t: &str) -> bool {
    t == "Memory"
}
/// RAM module sensors show up under different names depending on the
/// LibreHardwareMonitor version and board: "Memory" hardware, "DIMM #n",
/// SPD hub ("SPD5118"), "DDR5"... GPU memory sensors are excluded.
fn is_ram_sensor(x: &LhmReading) -> bool {
    if is_gpu(&x.hw_type) || is_storage(&x.hw_type) || is_cpu(&x.hw_type) {
        return false;
    }
    if is_memory(&x.hw_type) {
        return true;
    }
    let text = format!("{} {}", x.hw, x.name).to_ascii_lowercase();
    ["dimm", "spd", "ddr4", "ddr5", "memory module", "ram "].iter().any(|k| text.contains(k))
}

fn is_storage(t: &str) -> bool {
    t == "Storage"
}

pub fn summarize(r: &[LhmReading]) -> Summary {
    let cpu_temp = named(r, is_cpu, "Temperature", &["Core (Tctl/Tdie)", "CPU Package", "Core Max", "Core Average", "CPU Core"])
        .or_else(|| max(temps(r, is_cpu).map(|x| x.value)));
    let mut fans: Vec<(String, f64)> = r
        .iter()
        .filter(|x| x.kind == "Fan" && x.value > 0.0 && !is_gpu(&x.hw_type))
        .map(|x| (x.name.clone(), x.value.round()))
        .collect();
    fans.truncate(6);
    Summary {
        cpu_temp,
        cpu_power: named(r, is_cpu, "Power", &["Package", "CPU Package"]),
        ram_temp: max(r.iter().filter(|x| x.kind == "Temperature" && x.value > 0.0 && x.value < 120.0 && is_ram_sensor(x)).map(|x| x.value)),
        gpu_temp: named(r, is_gpu, "Temperature", &["GPU Core"]),
        gpu_hotspot: named(r, is_gpu, "Temperature", &["GPU Hot Spot", "GPU Memory Junction"]),
        gpu_power: named(r, is_gpu, "Power", &["GPU Package", "GPU Power", "GPU Core"]),
        storage: temps(r, is_storage).map(|x| (x.hw.clone(), x.value)).collect(),
        fans,
    }
}

// ------------------------------------------------------------------ install / status (Windows)

#[cfg(windows)]
mod win {
    use super::*;
    use std::os::windows::process::CommandExt;
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, HWND};
    use windows::Win32::System::Registry::{RegCloseKey, RegOpenKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ};
    use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
    use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub fn pawnio_installed() -> bool {
        unsafe {
            let mut key = HKEY::default();
            let ok = RegOpenKeyExW(HKEY_LOCAL_MACHINE, w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\PawnIO"), Some(0), KEY_READ, &mut key).is_ok();
            if ok {
                let _ = RegCloseKey(key);
            }
            ok
        }
    }

    pub fn task_exists() -> bool {
        std::process::Command::new("schtasks.exe")
            .args(["/Query", "/TN", TASK_NAME])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Verify an Authenticode signature (WinVerifyTrust, whole chain, no UI).
    pub fn signature_valid(path: &Path) -> bool {
        use windows::Win32::Security::WinTrust::{
            WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO,
            WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY, WTD_UI_NONE,
        };
        let wide = HSTRING::from(path.as_os_str());
        unsafe {
            let mut file = WINTRUST_FILE_INFO {
                cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
                pcwszFilePath: PCWSTR(wide.as_ptr()),
                ..Default::default()
            };
            let mut data = WINTRUST_DATA {
                cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
                dwUIChoice: WTD_UI_NONE,
                fdwRevocationChecks: WTD_REVOKE_NONE,
                dwUnionChoice: WTD_CHOICE_FILE,
                Anonymous: WINTRUST_DATA_0 { pFile: &mut file },
                dwStateAction: WTD_STATEACTION_VERIFY,
                ..Default::default()
            };
            let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
            let result = WinVerifyTrust(HWND(-1isize as *mut _), &mut action, &mut data as *mut _ as *mut _);
            data.dwStateAction = WTD_STATEACTION_CLOSE;
            let _ = WinVerifyTrust(HWND(-1isize as *mut _), &mut action, &mut data as *mut _ as *mut _);
            result == 0
        }
    }

    /// Run a program elevated (one UAC prompt) and wait for its exit code.
    pub fn run_elevated(exe: &Path, params: &str) -> Result<u32, String> {
        let file = HSTRING::from(exe.as_os_str());
        let args = HSTRING::from(params);
        unsafe {
            let mut info = SHELLEXECUTEINFOW {
                cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
                lpVerb: w!("runas"),
                lpFile: PCWSTR(file.as_ptr()),
                lpParameters: PCWSTR(args.as_ptr()),
                nShow: SW_HIDE.0,
                ..Default::default()
            };
            if let Err(e) = ShellExecuteExW(&mut info) {
                if e.code() == ERROR_CANCELLED.to_hresult() {
                    return Err("cancelled".into());
                }
                return Err(e.to_string());
            }
            if info.hProcess.is_invalid() {
                return Err("could not start the helper".into());
            }
            WaitForSingleObject(info.hProcess, INFINITE);
            let mut code = 0u32;
            let _ = GetExitCodeProcess(info.hProcess, &mut code);
            let _ = CloseHandle(info.hProcess);
            Ok(code)
        }
    }
}

/// Start the helper task if sensors are enabled but no fresh data arrives
/// (no prompt: the task already has the rights it needs).
pub fn ensure_running(data_dir: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if read(data_dir).is_none() && win::task_exists() {
            let _ = std::process::Command::new("schtasks.exe")
                .args(["/Run", "/TN", TASK_NAME])
                .creation_flags(0x0800_0000)
                .spawn();
        }
    }
    #[cfg(not(windows))]
    let _ = data_dir;
}

pub fn status(helper: Option<&Path>, data_dir: &Path) -> Value {
    #[cfg(windows)]
    {
        json!({
            "supported": true,
            "helper": helper.is_some_and(|h| h.is_file()),
            "enabled": win::task_exists(),
            "pawnio": win::pawnio_installed(),
            "running": read(data_dir).is_some(),
        })
    }
    #[cfg(not(windows))]
    {
        let _ = (helper, data_dir);
        json!({ "supported": false, "helper": false, "enabled": false, "pawnio": false, "running": false })
    }
}

fn exit_message(code: u32) -> String {
    match code {
        0 => "ok".into(),
        10 => "the PawnIO driver could not be installed".into(),
        11 => "the sensor task could not be created".into(),
        12 => "administrator rights were not granted".into(),
        c => format!("the sensor helper failed (code {c})"),
    }
}

/// Install PawnIO (if missing) and the logon task. Blocking; shows one UAC prompt.
pub fn enable(helper: &Path, data_dir: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        if !helper.is_file() {
            return Err("sensor helper not found in the installation".into());
        }
        let mut params = format!(
            "--install --user \"{}\\{}\" --out \"{}\"",
            std::env::var("USERDOMAIN").unwrap_or_default(),
            std::env::var("USERNAME").unwrap_or_default(),
            json_path(data_dir).display()
        );
        if !win::pawnio_installed() {
            let setup = data_dir.join("PawnIO_setup.exe");
            let mut b = ureq::AgentBuilder::new();
            if let Ok(tls) = native_tls::TlsConnector::new() {
                b = b.tls_connector(std::sync::Arc::new(tls));
            }
            let resp = b
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .get(PAWNIO_URL)
                .call()
                .map_err(|e| format!("could not download PawnIO: {e}"))?;
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut std::io::Read::take(resp.into_reader(), 64 * 1024 * 1024), &mut bytes)
                .map_err(|e| e.to_string())?;
            std::fs::write(&setup, bytes).map_err(|e| e.to_string())?;
            if !win::signature_valid(&setup) {
                let _ = std::fs::remove_file(&setup);
                return Err("the PawnIO installer is not correctly signed; aborted".into());
            }
            params.push_str(&format!(" --pawnio \"{}\"", setup.display()));
        }
        let code = win::run_elevated(helper, &params)?;
        let _ = std::fs::remove_file(data_dir.join("PawnIO_setup.exe"));
        if code == 0 { Ok(()) } else { Err(exit_message(code)) }
    }
    #[cfg(not(windows))]
    {
        let _ = (helper, data_dir);
        Err("advanced sensors are available on Windows only".into())
    }
}

/// Remove the task (and PawnIO if Wallgates installed it). Shows one UAC prompt.
pub fn disable(helper: &Path, data_dir: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let code = win::run_elevated(helper, "--uninstall")?;
        let _ = std::fs::remove_file(json_path(data_dir));
        if code == 0 { Ok(()) } else { Err(exit_message(code)) }
    }
    #[cfg(not(windows))]
    {
        let _ = (helper, data_dir);
        Err("advanced sensors are available on Windows only".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(hw: &str, t: &str, name: &str, kind: &str, v: f64) -> LhmReading {
        LhmReading { hw: hw.into(), hw_type: t.into(), name: name.into(), kind: kind.into(), value: v }
    }

    #[test]
    fn summarizes_lhm_readings() {
        let list = vec![
            r("AMD Ryzen 9 7950X", "Cpu", "Core (Tctl/Tdie)", "Temperature", 68.4),
            r("AMD Ryzen 9 7950X", "Cpu", "CCD1 (Tdie)", "Temperature", 64.0),
            r("AMD Ryzen 9 7950X", "Cpu", "Package", "Power", 88.2),
            r("NVIDIA GeForce RTX 4080", "GpuNvidia", "GPU Core", "Temperature", 61.0),
            r("NVIDIA GeForce RTX 4080", "GpuNvidia", "GPU Hot Spot", "Temperature", 72.0),
            r("NVIDIA GeForce RTX 4080", "GpuNvidia", "GPU Fan", "Fan", 1100.0),
            r("DIMM #1", "Memory", "Temperature", "Temperature", 44.5),
            r("DIMM #2", "Memory", "Temperature", "Temperature", 46.0),
            r("Samsung SSD 990 PRO 2TB", "Storage", "Temperature", "Temperature", 48.0),
            r("Nuvoton NCT6799D", "SuperIO", "CPU Fan", "Fan", 1240.0),
            r("Nuvoton NCT6799D", "SuperIO", "Fan #3", "Fan", 0.0),
        ];
        let s = summarize(&list);
        assert_eq!(s.cpu_temp, Some(68.4));
        assert_eq!(s.cpu_power, Some(88.2));
        assert_eq!(s.ram_temp, Some(46.0));
        // Other naming schemes (SuperIO / SPD hub hardware).
        let alt = summarize(&[r("Kingston FURY DDR5 (SPD5118)", "Motherboard", "DIMM #2 Temperature", "Temperature", 41.5),
                              r("NVIDIA GeForce RTX 4080", "GpuNvidia", "GPU Memory Junction", "Temperature", 80.0)]);
        assert_eq!(alt.ram_temp, Some(41.5));
        assert_eq!(s.gpu_hotspot, Some(72.0));
        assert_eq!(s.storage, vec![("Samsung SSD 990 PRO 2TB".to_string(), 48.0)]);
        assert_eq!(s.fans, vec![("CPU Fan".to_string(), 1240.0)]);
    }

    #[test]
    fn ignores_stale_files() {
        let dir = std::env::temp_dir().join(format!("wg-sens-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(json_path(&dir), r#"{"version":1,"time":1,"readings":[]}"#).unwrap();
        assert!(read(&dir).is_none());
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        std::fs::write(json_path(&dir), format!(r#"{{"version":1,"time":{now},"readings":[{{"hw":"x","hwType":"Cpu","name":"CPU Package","type":"Temperature","value":50}}]}}"#)).unwrap();
        assert_eq!(read(&dir).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
