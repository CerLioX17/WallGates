//! Places one wallpaper window per monitor behind the desktop icons.
//!
//! Platform specifics live in `windows.rs` / `linux.rs`; every platform exposes
//! `attach(window, rect)`, `should_auto_pause(rules)` and `restore()`.

use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use self::windows as platform;

#[cfg(not(any(windows, target_os = "linux")))]
mod fallback;
#[cfg(not(any(windows, target_os = "linux")))]
use fallback as platform;

pub const LABEL_PREFIX: &str = "wp-";

/// Physical-pixel rectangle of a monitor in virtual-screen coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Auto-pause rules, copied out of the config so no lock is held while polling.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct PauseRules {
    pub fullscreen: bool,
    pub maximized: bool,
    pub battery: bool,
}

static GENERATION: AtomicU32 = AtomicU32::new(0);

pub fn is_wallpaper_window(label: &str) -> bool {
    label.starts_with(LABEL_PREFIX)
}

/// (Re)create the wallpaper windows, one per connected monitor.
pub fn create_windows(app: &AppHandle) -> Result<(), String> {
    for (label, window) in app.webview_windows() {
        if is_wallpaper_window(&label) {
            let _ = window.destroy();
        }
    }

    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    if monitors.is_empty() {
        return Err("no monitors detected".into());
    }
    // Unique labels per generation: destroyed windows may not be gone yet.
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed);

    for (index, monitor) in monitors.iter().enumerate() {
        let pos = monitor.position();
        let size = monitor.size();
        let rect = Rect { x: pos.x, y: pos.y, width: size.width, height: size.height };
        let label = format!("{LABEL_PREFIX}{generation}-{index}");

        let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("host.html".into()))
            .title("Wallgates wallpaper")
            .decorations(false)
            .resizable(false)
            .skip_taskbar(true)
            .focused(false)
            .shadow(false)
            .visible(false)
            .build()
            .map_err(|e| e.to_string())?;

        match platform::attach(&window, rect) {
            Ok(info) => log(app, &format!("monitor {index} ({}): {info}", monitor_name(monitor))),
            Err(e) => {
                log(app, &format!("monitor {index}: attach FAILED: {e}"));
                let _ = window.destroy();
                return Err(e);
            }
        }
    }
    Ok(())
}

fn monitor_name(m: &tauri::Monitor) -> String {
    m.name().cloned().unwrap_or_else(|| "unnamed".into())
}

/// Append a line to `<app data>/wallgates.log` (kept small) for troubleshooting.
pub fn log(app: &AppHandle, line: &str) {
    use std::io::Write;
    eprintln!("[wallgates] {line}");
    let Ok(dir) = app.path().app_data_dir() else { return };
    let path = dir.join("wallgates.log");
    if std::fs::metadata(&path).map(|m| m.len() > 256 * 1024).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[{secs}] v{} {line}", env!("CARGO_PKG_VERSION"));
    }
}

pub fn should_auto_pause(rules: PauseRules) -> bool {
    platform::should_auto_pause(rules)
}

/// Undo desktop changes before quitting (e.g. repaint the original wallpaper).
pub fn restore() {
    platform::restore()
}
