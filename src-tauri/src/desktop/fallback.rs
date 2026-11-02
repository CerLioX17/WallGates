//! Platforms without desktop integration yet (macOS): the wallpaper is shown as a
//! borderless window covering the monitor. See ROADMAP for native support.

use super::{PauseRules, Rect};
use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

pub fn attach(window: &WebviewWindow, rect: Rect) -> Result<String, String> {
    window
        .set_position(PhysicalPosition::new(rect.x, rect.y))
        .map_err(|e| e.to_string())?;
    window
        .set_size(PhysicalSize::new(rect.width, rect.height))
        .map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    Ok("strategy: borderless window (no desktop integration on this platform)".into())
}

pub fn should_auto_pause(_rules: PauseRules) -> bool {
    false
}

pub fn restore() {}
