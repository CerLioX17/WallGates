//! Platforms without desktop integration yet (macOS): the wallpaper is shown as a
//! borderless window covering the monitor. See ROADMAP for native support.

use super::{Observation, Rect};
use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

#[derive(Clone, Debug, Default)]
pub struct Handle;

pub fn stable_monitor_id(device: &str) -> String {
    device.to_string()
}

pub fn attach(window: &WebviewWindow, rect: Rect) -> Result<(Handle, String), String> {
    window
        .set_position(PhysicalPosition::new(rect.x, rect.y))
        .map_err(|e| e.to_string())?;
    window
        .set_size(PhysicalSize::new(rect.width, rect.height))
        .map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    Ok((Handle, "strategy: borderless window (no desktop integration on this platform)".into()))
}

pub fn is_attached(_h: &Handle) -> bool {
    true
}

pub fn observe() -> Observation {
    Observation { desktop_focused: true, ..Default::default() }
}

pub fn cursor() -> Option<(i32, i32)> {
    None
}

pub fn set_static_wallpaper(_path: &std::path::Path) -> Result<(), String> {
    Err("not supported on this platform yet".into())
}

pub fn set_lock_screen(_path: &std::path::Path) -> Result<(), String> {
    Err("not supported on this platform yet".into())
}

pub fn restore() {}
