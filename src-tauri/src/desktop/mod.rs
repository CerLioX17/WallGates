//! Places wallpaper windows behind the desktop icons and observes the desktop
//! (fullscreen apps, battery, cursor) for the engine.
//!
//! Platform specifics live in `windows.rs` / `linux.rs` / `fallback.rs`.

use serde::Serialize;
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
pub const SPAN_ID: &str = "span";

/// Physical-pixel rectangle in virtual-screen coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn right(&self) -> i32 {
        self.x + self.width as i32
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.height as i32
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
    pub fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect { x, y, width: (self.right().max(o.right()) - x) as u32, height: (self.bottom().max(o.bottom()) - y) as u32 }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MonitorInfo {
    /// Stable identity (survives sleep, reconnects and reordering where the OS allows).
    pub id: String,
    /// OS device name, e.g. `\\.\DISPLAY1`.
    pub device: String,
    pub index: usize,
    pub rect: Rect,
    pub primary: bool,
    pub scale: f64,
}

/// A live wallpaper window.
#[derive(Clone, Debug, Serialize)]
pub struct Slot {
    pub label: String,
    /// Monitor id, or [`SPAN_ID`] when one window spans every monitor.
    pub monitor_id: String,
    pub index: usize,
    pub rect: Rect,
    #[serde(skip)]
    pub handle: platform::Handle,
}

/// What is happening on the desktop right now.
#[derive(Clone, Debug, Default)]
pub struct Observation {
    /// Monitors fully covered by a window (games, borderless, fullscreen video).
    pub covered: Vec<Rect>,
    /// Monitors showing a maximized window.
    pub maximized: Vec<Rect>,
    pub on_battery: bool,
    /// Lower-case file name of the foreground process, e.g. `game.exe`.
    pub foreground_exe: Option<String>,
    pub desktop_focused: bool,
}

static GENERATION: AtomicU32 = AtomicU32::new(0);

pub fn is_wallpaper_window(label: &str) -> bool {
    label.starts_with(LABEL_PREFIX)
}

pub fn monitors(app: &AppHandle) -> Vec<MonitorInfo> {
    let primary = app.primary_monitor().ok().flatten().map(|m| m.name().cloned().unwrap_or_default());
    let mut list: Vec<MonitorInfo> = app
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let device = m.name().cloned().unwrap_or_else(|| "monitor".into());
            let pos = m.position();
            let size = m.size();
            MonitorInfo {
                id: platform::stable_monitor_id(&device),
                primary: primary.as_deref() == Some(device.as_str()),
                device,
                index: 0,
                rect: Rect { x: pos.x, y: pos.y, width: size.width, height: size.height },
                scale: m.scale_factor(),
            }
        })
        .collect();
    // Left-to-right, top-to-bottom: "Display 1" is the leftmost.
    list.sort_by_key(|m| (m.rect.x, m.rect.y));
    // Disambiguate identical ids (e.g. two identical monitors on a platform without serials).
    let mut seen = std::collections::HashMap::<String, u32>::new();
    for (i, m) in list.iter_mut().enumerate() {
        m.index = i;
        let n = seen.entry(m.id.clone()).or_insert(0);
        if *n > 0 {
            m.id = format!("{}#{}", m.id, n);
        }
        *n += 1;
    }
    list
}

fn span_rect(monitors: &[MonitorInfo]) -> Option<Rect> {
    let first = monitors.first()?.rect;
    Some(monitors.iter().skip(1).fold(first, |acc, m| acc.union(&m.rect)))
}

/// (Re)create the wallpaper windows: one per monitor, or one spanning all.
pub fn create_windows(app: &AppHandle, span: bool) -> Result<Vec<Slot>, String> {
    destroy_windows(app);
    let monitors = monitors(app);
    if monitors.is_empty() {
        return Err("no monitors detected".into());
    }
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed);

    let targets: Vec<(String, Rect)> = if span {
        vec![(SPAN_ID.to_string(), span_rect(&monitors).unwrap())]
    } else {
        monitors.iter().map(|m| (m.id.clone(), m.rect)).collect()
    };

    let mut slots = Vec::new();
    for (index, (monitor_id, rect)) in targets.into_iter().enumerate() {
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
            Ok((handle, info)) => {
                log(app, &format!("slot {index} ({monitor_id}): {info}"));
                slots.push(Slot { label, monitor_id, index, rect, handle });
            }
            Err(e) => {
                log(app, &format!("slot {index} ({monitor_id}): attach FAILED: {e}"));
                let _ = window.destroy();
                return Err(e);
            }
        }
    }
    Ok(slots)
}

pub fn destroy_windows(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if is_wallpaper_window(&label) {
            let _ = window.destroy();
        }
    }
}

/// True while every slot is still attached to a live desktop and the monitor
/// layout is unchanged. False after an Explorer restart, a monitor being
/// plugged/unplugged, a resolution change, etc.
pub fn layout_still_valid(app: &AppHandle, slots: &[Slot], span: bool) -> bool {
    if slots.is_empty() || !slots.iter().all(|s| platform::is_attached(&s.handle)) {
        return false;
    }
    let monitors = monitors(app);
    if monitors.is_empty() {
        // Transient state (e.g. during display mode switch): don't thrash.
        return true;
    }
    if span {
        slots.len() == 1 && Some(slots[0].rect) == span_rect(&monitors)
    } else {
        slots.len() == monitors.len()
            && slots.iter().zip(&monitors).all(|(s, m)| s.monitor_id == m.id && s.rect == m.rect)
    }
}

pub fn observe() -> Observation {
    platform::observe()
}

pub fn cursor() -> Option<(i32, i32)> {
    platform::cursor()
}

/// Set the static OS desktop background (used for sync and when quitting).
pub fn set_static_wallpaper(path: &std::path::Path) -> Result<(), String> {
    platform::set_static_wallpaper(path)
}

pub fn set_lock_screen(path: &std::path::Path) -> Result<(), String> {
    platform::set_lock_screen(path)
}

/// Undo desktop changes before quitting (e.g. repaint the original wallpaper).
pub fn restore() {
    platform::restore()
}

/// Append a line to `<app data>/wallgates.log` (kept small) for troubleshooting.
pub fn log(app: &AppHandle, line: &str) {
    use std::io::Write;
    eprintln!("[wallgates] {line}");
    let Ok(dir) = app.path().app_data_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_math() {
        let a = Rect { x: 0, y: 0, width: 1920, height: 1080 };
        let b = Rect { x: 1920, y: 0, width: 2560, height: 1440 };
        assert!(!a.intersects(&b));
        assert_eq!(a.union(&b), Rect { x: 0, y: 0, width: 4480, height: 1440 });
        assert!(b.contains_point(2000, 10));
        assert!(!b.contains_point(1919, 10));
        let left = Rect { x: -1920, y: 0, width: 1920, height: 1080 };
        assert_eq!(left.union(&a).x, -1920);
    }
}
