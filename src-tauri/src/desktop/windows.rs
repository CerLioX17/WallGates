//! Windows: render behind the desktop icons using the WorkerW technique.
//!
//! Sending the undocumented message 0x052C to `Progman` makes Explorer create a
//! `WorkerW` window that sits between the wallpaper and the icon layer
//! (`SHELLDLL_DefView`). Parenting our window to it puts it behind the icons.
//!
//! Explorer's window tree differs by version:
//! * Windows 10 / 11 before 24H2: the target `WorkerW` is a top-level window,
//!   the sibling right after the window that hosts `SHELLDLL_DefView`.
//! * Windows 11 24H2+: `SHELLDLL_DefView` and the `WorkerW` are both children of
//!   `Progman`; we parent to that child `WorkerW`. Confirmed working on 26H2
//!   (Progman children: ["SHELLDLL_DefView", "WorkerW"], 4K single monitor).
//! If neither is found we parent to `Progman` and insert ourselves below
//! `SHELLDLL_DefView` in the z-order.
//!
//! No hooks, no injected code, no services: only window parenting and
//! read-only queries, which keeps antivirus heuristics quiet.

use super::{Observation, Rect};
use std::ffi::c_void;
use std::path::Path;
use tauri::WebviewWindow;
use windows::core::{w, BOOL, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesW, GetMonitorInfoW, DISPLAY_DEVICEW, MonitorFromWindow, ScreenToClient, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN};
use windows::Win32::UI::WindowsAndMessaging::*;

const SPAWN_WORKERW: u32 = 0x052C;
const EDD_GET_DEVICE_INTERFACE_NAME: u32 = 0x1;

/// Raw window handles (stored as integers so the slot list is Send).
#[derive(Clone, Debug, Default)]
pub struct Handle {
    hwnd: isize,
    parent: isize,
}

fn hwnd(v: isize) -> HWND {
    HWND(v as *mut c_void)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Device interface path of the monitor, e.g.
/// `\\?\DISPLAY#GSM5B7F#5&2a0a2ce3&0&UID4352#{e6f07b5f-...}` -> `GSM5B7F#5&2a0a2ce3&0&UID4352`.
/// Unlike `\\.\DISPLAY1` this survives sleep and reordering.
pub fn stable_monitor_id(device: &str) -> String {
    unsafe {
        let name = wide(device);
        let mut dd = DISPLAY_DEVICEW { cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32, ..Default::default() };
        if EnumDisplayDevicesW(PCWSTR(name.as_ptr()), 0, &mut dd, EDD_GET_DEVICE_INTERFACE_NAME).as_bool() {
            let id = String::from_utf16_lossy(&dd.DeviceID);
            let id = id.trim_end_matches('\0');
            let parts: Vec<&str> = id.split('#').collect();
            if parts.len() >= 3 && !parts[1].is_empty() {
                return format!("{}#{}", parts[1], parts[2]);
            }
        }
    }
    device.to_string()
}

unsafe fn find_child(parent: Option<HWND>, after: Option<HWND>, class: PCWSTR) -> Option<HWND> {
    FindWindowExW(parent, after, class, PCWSTR::null())
        .ok()
        .filter(|h| !h.0.is_null())
}

unsafe extern "system" fn enum_workerw(top: HWND, lparam: LPARAM) -> BOOL {
    // Pre-24H2 layout: the window that contains SHELLDLL_DefView is followed by our WorkerW.
    if find_child(Some(top), None, w!("SHELLDLL_DefView")).is_some() {
        if let Some(worker) = find_child(None, Some(top), w!("WorkerW")) {
            *(lparam.0 as *mut HWND) = worker;
            return BOOL(0); // stop enumerating
        }
    }
    BOOL(1)
}

/// Look for an existing wallpaper layer. Returns (parent, insert_after, strategy).
unsafe fn locate(progman: HWND) -> Option<(HWND, Option<HWND>, &'static str)> {
    let mut worker = HWND(std::ptr::null_mut());
    let _ = EnumWindows(Some(enum_workerw), LPARAM(&mut worker as *mut HWND as isize));
    if !worker.0.is_null() {
        return Some((worker, None, "top-level WorkerW (Windows 10 / 11 pre-24H2)"));
    }
    if let Some(worker) = find_child(Some(progman), None, w!("WorkerW")) {
        return Some((worker, None, "WorkerW child of Progman (Windows 11 24H2+)"));
    }
    None
}

unsafe fn find_target() -> Result<(HWND, Option<HWND>, &'static str), String> {
    let progman = FindWindowW(w!("Progman"), PCWSTR::null())
        .map_err(|e| format!("Progman not found: {e}"))?;

    // Reuse an existing layer first: asking Explorer again leaves extra WorkerW windows behind.
    if let Some(found) = locate(progman) {
        return Ok(found);
    }
    let mut result = 0usize;
    for (wp, lp) in [(0xD, 0x1), (0, 0)] {
        SendMessageTimeoutW(progman, SPAWN_WORKERW, WPARAM(wp), LPARAM(lp), SMTO_NORMAL, 1000, Some(&mut result));
    }
    if let Some(found) = locate(progman) {
        return Ok(found);
    }
    // Last resort: inside Progman, below the icon layer.
    let defview = find_child(Some(progman), None, w!("SHELLDLL_DefView"));
    Ok((progman, defview, "Progman below SHELLDLL_DefView (fallback)"))
}

unsafe fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 128];
    let len = GetClassNameW(h, &mut buf);
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

pub fn attach(window: &WebviewWindow, rect: Rect) -> Result<(Handle, String), String> {
    let raw = window.hwnd().map_err(|e| e.to_string())?;
    let me = HWND(raw.0 as *mut c_void);

    unsafe {
        let (parent, insert_after, strategy) = find_target()?;

        // Turn the popup into a non-activating child window.
        let style = GetWindowLongPtrW(me, GWL_STYLE) as u32;
        let style = (style & !(WS_POPUP.0 | WS_CAPTION.0 | WS_THICKFRAME.0 | WS_SYSMENU.0)) | WS_CHILD.0 | WS_VISIBLE.0;
        SetWindowLongPtrW(me, GWL_STYLE, style as isize);
        let ex = GetWindowLongPtrW(me, GWL_EXSTYLE) as u32;
        let ex = (ex & !WS_EX_APPWINDOW.0) | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0;
        SetWindowLongPtrW(me, GWL_EXSTYLE, ex as isize);

        SetParent(me, Some(parent)).map_err(|e| format!("SetParent failed: {e}"))?;

        // Child coordinates are relative to the parent's client area, which
        // starts at the virtual-screen origin (can be negative with multiple monitors).
        let mut origin = POINT { x: rect.x, y: rect.y };
        let _ = ScreenToClient(parent, &mut origin);

        let mut flags = SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED;
        if insert_after.is_none() {
            flags |= SWP_NOZORDER;
        }
        SetWindowPos(me, insert_after, origin.x, origin.y, rect.width as i32, rect.height as i32, flags)
            .map_err(|e| format!("SetWindowPos failed: {e}"))?;

        let info = format!(
            "strategy: {strategy}; parent class: {}; position {},{} size {}x{}",
            class_name(parent),
            origin.x,
            origin.y,
            rect.width,
            rect.height
        );
        Ok((Handle { hwnd: me.0 as isize, parent: parent.0 as isize }, info))
    }
}

pub fn is_attached(h: &Handle) -> bool {
    unsafe {
        IsWindow(Some(hwnd(h.hwnd))).as_bool()
            && IsWindow(Some(hwnd(h.parent))).as_bool()
            && GetParent(hwnd(h.hwnd)).map(|p| p.0 as isize) == Ok(h.parent)
    }
}

unsafe fn monitor_rect(h: HWND) -> Option<RECT> {
    let monitor = MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST);
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    GetMonitorInfoW(monitor, &mut info).as_bool().then_some(info.rcMonitor)
}

fn to_rect(r: RECT) -> Rect {
    Rect { x: r.left, y: r.top, width: (r.right - r.left).max(0) as u32, height: (r.bottom - r.top).max(0) as u32 }
}

struct Scan {
    own_pid: u32,
    covered: Vec<Rect>,
    maximized: Vec<Rect>,
}

const IGNORED_CLASSES: &[&str] = &[
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "NotifyIconOverflowWindow",
    "Windows.UI.Core.CoreWindow",
    "XamlExplorerHostIslandWindow",
];

unsafe extern "system" fn enum_visible(h: HWND, lparam: LPARAM) -> BOOL {
    let scan = &mut *(lparam.0 as *mut Scan);
    if !IsWindowVisible(h).as_bool() || IsIconic(h).as_bool() {
        return BOOL(1);
    }
    // Suspended UWP apps and hidden shell surfaces are "cloaked" but still visible.
    let mut cloaked = 0u32;
    if DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut c_void, 4).is_ok() && cloaked != 0 {
        return BOOL(1);
    }
    let ex = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
    // Overlays (GPU vendor overlays, recording widgets) are transparent / tool windows.
    if ex & WS_EX_TOOLWINDOW.0 != 0 || ex & WS_EX_NOACTIVATE.0 != 0 || (ex & WS_EX_LAYERED.0 != 0 && ex & WS_EX_TRANSPARENT.0 != 0) {
        return BOOL(1);
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(h, Some(&mut pid));
    if pid == scan.own_pid {
        return BOOL(1);
    }
    if IGNORED_CLASSES.contains(&class_name(h).as_str()) {
        return BOOL(1);
    }
    let mut win = RECT::default();
    if GetWindowRect(h, &mut win).is_err() {
        return BOOL(1);
    }
    let Some(m) = monitor_rect(h) else { return BOOL(1) };
    if win.left <= m.left && win.top <= m.top && win.right >= m.right && win.bottom >= m.bottom {
        let r = to_rect(m);
        if !scan.covered.contains(&r) {
            scan.covered.push(r);
        }
    } else if IsZoomed(h).as_bool() {
        let r = to_rect(m);
        if !scan.maximized.contains(&r) {
            scan.maximized.push(r);
        }
    }
    BOOL(1)
}

unsafe fn process_name(h: HWND) -> Option<String> {
    let mut pid = 0u32;
    GetWindowThreadProcessId(h, Some(&mut pid));
    if pid == 0 {
        return None;
    }
    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    let mut buf = [0u16; 520];
    let mut len = buf.len() as u32;
    let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
    let _ = CloseHandle(process);
    if !ok {
        return None;
    }
    let full = String::from_utf16_lossy(&buf[..len as usize]);
    full.rsplit(['\\', '/']).next().map(|s| s.to_ascii_lowercase())
}

pub fn observe() -> Observation {
    unsafe {
        let mut obs = Observation::default();

        let mut power = SYSTEM_POWER_STATUS::default();
        if GetSystemPowerStatus(&mut power).is_ok() {
            // ACLineStatus 0 = on battery; SystemStatusFlag 1 = battery saver on.
            obs.on_battery = power.ACLineStatus == 0 || power.SystemStatusFlag == 1;
        }

        let mut scan = Scan { own_pid: std::process::id(), covered: Vec::new(), maximized: Vec::new() };
        let _ = EnumWindows(Some(enum_visible), LPARAM(&mut scan as *mut Scan as isize));
        obs.covered = scan.covered;
        obs.maximized = scan.maximized;

        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            obs.desktop_focused = true;
        } else {
            let class = class_name(fg);
            let mut fg_pid = 0u32;
            GetWindowThreadProcessId(fg, Some(&mut fg_pid));
            // Our own settings window counts as "desktop" so previews stay audible.
            obs.desktop_focused = matches!(class.as_str(), "Progman" | "WorkerW") || fg_pid == std::process::id();
            obs.foreground_exe = process_name(fg);
            // Exclusive-fullscreen D3D games and presentation mode.
            if let Ok(state) = SHQueryUserNotificationState() {
                if state == QUNS_RUNNING_D3D_FULL_SCREEN || state == QUNS_PRESENTATION_MODE {
                    if let Some(m) = monitor_rect(fg) {
                        let r = to_rect(m);
                        if !obs.covered.contains(&r) {
                            obs.covered.push(r);
                        }
                    }
                }
            }
        }
        obs
    }
}

pub fn cursor() -> Option<(i32, i32)> {
    unsafe {
        let mut p = POINT::default();
        GetCursorPos(&mut p).ok().map(|_| (p.x, p.y))
    }
}

pub fn set_static_wallpaper(path: &Path) -> Result<(), String> {
    let mut wpath = wide(&path.to_string_lossy());
    unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(wpath.as_mut_ptr() as *mut c_void),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
        .map_err(|e| e.to_string())
    }
}

pub fn set_lock_screen(path: &Path) -> Result<(), String> {
    use windows::Storage::StorageFile;
    use windows::System::UserProfile::LockScreen;
    let path = HSTRING::from(path.as_os_str());
    // WinRT async calls block here; run off the UI thread.
    std::thread::spawn(move || -> Result<(), String> {
        let file = StorageFile::GetFileFromPathAsync(&path).map_err(|e| e.to_string())?.get().map_err(|e| e.to_string())?;
        LockScreen::SetImageFileAsync(&file).map_err(|e| e.to_string())?.get().map_err(|e| e.to_string())
    })
    .join()
    .map_err(|_| "lock screen thread panicked".to_string())?
}

/// Re-apply the user's static Windows wallpaper so the last rendered frame of
/// our window does not stay stuck on the desktop after we quit.
pub fn restore() {
    unsafe {
        let mut buf = [0u16; 1024];
        if SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buf.len() as u32,
            Some(buf.as_mut_ptr() as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_ok()
        {
            let _ = SystemParametersInfoW(SPI_SETDESKWALLPAPER, 0, Some(buf.as_mut_ptr() as *mut c_void), SPIF_UPDATEINIFILE);
        }
    }
}
