//! Persistent settings (JSON in the app config dir), with migration from v0.1.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub const CONFIG_VERSION: u32 = 2;

/// What a monitor (or the whole desktop) shows.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Source {
    Wallpaper { id: String },
    Playlist { id: String },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LayoutMode {
    /// Every monitor can show something different (falls back to the default source).
    #[default]
    PerMonitor,
    /// One wallpaper stretched across all monitors.
    Span,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistMode {
    /// Rotate every `interval_minutes`.
    #[default]
    Interval,
    /// Each item has an `at` time ("07:30", "sunrise", "sunset").
    TimeOfDay,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlaylistItem {
    pub wallpaper: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub mode: PlaylistMode,
    pub interval_minutes: u32,
    pub shuffle: bool,
    pub items: Vec<PlaylistItem>,
}

impl Default for Playlist {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: "Playlist".into(),
            mode: PlaylistMode::Interval,
            interval_minutes: 15,
            shuffle: false,
            items: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PowerAction {
    /// Keep running normally.
    None,
    /// Lower frame rate and resolution.
    #[default]
    Eco,
    /// Freeze the wallpaper.
    Pause,
    /// Unload the wallpaper entirely (frees GPU memory).
    Stop,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PauseScope {
    /// Only the monitor covered by the fullscreen app.
    #[default]
    Monitor,
    /// Every monitor.
    All,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Performance {
    /// What to do while a game / fullscreen app (incl. borderless) runs.
    pub fullscreen: PowerAction,
    /// What to do while another app is maximized.
    pub maximized: PowerAction,
    /// What to do on battery power.
    pub battery: PowerAction,
    pub pause_scope: PauseScope,
    /// Frame cap (0 = display refresh rate).
    pub fps_limit: u32,
    /// Internal resolution for shader wallpapers, 0.25..=1.
    pub render_scale: f32,
    /// Mute wallpaper audio unless the desktop is focused.
    pub mute_unless_desktop: bool,
    /// Master volume 0..=1 applied on top of each wallpaper's own volume.
    pub volume: f32,
}

impl Default for Performance {
    fn default() -> Self {
        Self {
            fullscreen: PowerAction::Pause,
            maximized: PowerAction::None,
            battery: PowerAction::Eco,
            pause_scope: PauseScope::Monitor,
            fps_limit: 60,
            render_scale: 1.0,
            mute_unless_desktop: true,
            volume: 1.0,
        }
    }
}

/// Per-application rule: while `exe` is in the foreground, apply `action`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AppRule {
    pub exe: String,
    pub action: AppAction,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppAction {
    Pause,
    Stop,
    Mute,
    Eco,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Location {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Integrations {
    /// Allow Wallgates to fetch weather (Open-Meteo, no account, no key).
    pub weather: bool,
    pub location: Option<Location>,
    pub fahrenheit: bool,
    /// Allow browsing the online gallery.
    pub gallery: bool,
    /// Check GitHub for new versions.
    pub update_check: bool,
    /// Allow market data (Yahoo Finance) for wallpapers with the "markets" capability.
    pub markets: bool,
}

impl Default for Integrations {
    fn default() -> Self {
        Self { weather: false, location: None, fahrenheit: false, gallery: true, update_check: true, markets: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Hotkeys {
    pub enabled: bool,
    pub pause: String,
    pub next: String,
    pub show: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            enabled: true,
            pause: "Ctrl+Alt+P".into(),
            next: "Ctrl+Alt+N".into(),
            show: "Ctrl+Alt+W".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub layout: LayoutMode,
    /// Shown on every monitor without its own assignment (and in span mode).
    pub default_source: Option<Source>,
    /// Monitor id -> source (per-monitor mode).
    pub monitors: HashMap<String, Source>,
    /// Wallpaper id -> property overrides.
    pub properties: HashMap<String, Map<String, Value>>,
    pub playlists: Vec<Playlist>,
    pub paused: bool,
    pub muted: bool,
    pub performance: Performance,
    pub app_rules: Vec<AppRule>,
    pub integrations: Integrations,
    pub hotkeys: Hotkeys,
    /// Copy the current wallpaper to the Windows lock screen.
    pub sync_lock_screen: bool,
    /// Copy the current wallpaper to the static Windows desktop background.
    pub sync_desktop: bool,
    /// "auto", "en" or "it".
    pub language: String,
    pub onboarding_done: bool,
    /// Wallpaper ids the user marked as favourite.
    pub favorites: Vec<String>,
    /// Crossfade duration when the wallpaper changes (0 = instant).
    pub transition_ms: u32,
    /// Show wallpapers rated "mature" in the library.
    pub show_mature: bool,

    /// v0.1 field, migrated into `default_source`.
    #[serde(skip_serializing)]
    active: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            layout: LayoutMode::PerMonitor,
            default_source: Some(Source::Wallpaper { id: "builtin/aurora".into() }),
            monitors: HashMap::new(),
            properties: HashMap::new(),
            playlists: Vec::new(),
            paused: false,
            muted: false,
            performance: Performance::default(),
            app_rules: Vec::new(),
            integrations: Integrations::default(),
            hotkeys: Hotkeys::default(),
            sync_lock_screen: false,
            sync_desktop: false,
            language: "auto".into(),
            onboarding_done: false,
            favorites: Vec::new(),
            transition_ms: 900,
            show_mature: false,
            active: None,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Self {
        let raw: Option<Value> = fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let Some(raw) = raw else { return Self::default() };
        let mut cfg: Config = serde_json::from_value(raw.clone()).unwrap_or_default();
        // v0.1 files have no "version" field (serde fills in the current default).
        let file_version = raw.get("version").and_then(Value::as_u64).unwrap_or(1);
        if file_version < 2 {
            cfg.migrate_v1(&raw);
        }
        cfg.sanitize();
        cfg
    }

    fn migrate_v1(&mut self, raw: &Value) {
        if let Some(id) = self.active.take() {
            self.default_source = Some(Source::Wallpaper { id });
        }
        let b = |k: &str| raw.get(k).and_then(Value::as_bool);
        if let Some(v) = b("pause_on_fullscreen") {
            self.performance.fullscreen = if v { PowerAction::Pause } else { PowerAction::None };
        }
        if let Some(v) = b("pause_on_maximized") {
            self.performance.maximized = if v { PowerAction::Pause } else { PowerAction::None };
        }
        if let Some(v) = b("pause_on_battery") {
            self.performance.battery = if v { PowerAction::Pause } else { PowerAction::None };
        }
        if let Some(v) = raw.get("fps_limit").and_then(Value::as_u64) {
            self.performance.fps_limit = v as u32;
        }
        // v0.1 users have already seen the app.
        self.onboarding_done = true;
        self.version = CONFIG_VERSION;
    }

    pub fn sanitize(&mut self) {
        let p = &mut self.performance;
        p.fps_limit = p.fps_limit.min(240);
        p.render_scale = if p.render_scale.is_finite() { p.render_scale.clamp(0.25, 1.0) } else { 1.0 };
        p.volume = if p.volume.is_finite() { p.volume.clamp(0.0, 1.0) } else { 1.0 };
        for pl in &mut self.playlists {
            pl.interval_minutes = pl.interval_minutes.clamp(1, 24 * 60);
        }
        self.transition_ms = self.transition_ms.min(5000);
        if !["auto", "en", "it"].contains(&self.language.as_str()) {
            self.language = "auto".into();
        }
    }

    /// Atomic write: temp file + rename, so a crash never leaves a truncated config.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    pub fn playlist(&self, id: &str) -> Option<&Playlist> {
        self.playlists.iter().find(|p| p.id == id)
    }

    /// Source for a monitor, honouring the layout mode.
    pub fn source_for(&self, monitor_id: &str) -> Option<&Source> {
        match self.layout {
            LayoutMode::Span => self.default_source.as_ref(),
            LayoutMode::PerMonitor => self.monitors.get(monitor_id).or(self.default_source.as_ref()),
        }
    }

    /// Drop references to a wallpaper that no longer exists.
    pub fn forget_wallpaper(&mut self, id: &str) {
        self.properties.remove(id);
        self.favorites.retain(|f| f != id);
        let gone = |s: &Source| matches!(s, Source::Wallpaper { id: w } if w == id);
        if self.default_source.as_ref().is_some_and(gone) {
            self.default_source = Some(Source::Wallpaper { id: "builtin/aurora".into() });
        }
        self.monitors.retain(|_, s| !gone(s));
        for pl in &mut self.playlists {
            pl.items.retain(|i| i.wallpaper != id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_v1() {
        let dir = std::env::temp_dir().join(format!("wg-cfg-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        fs::write(
            &path,
            r#"{"active":"user/x","properties":{},"paused":false,"pause_on_fullscreen":true,
               "pause_on_maximized":false,"pause_on_battery":false,"fps_limit":30}"#,
        )
        .unwrap();
        let cfg = Config::load(&path);
        assert_eq!(cfg.version, 2);
        assert_eq!(cfg.default_source, Some(Source::Wallpaper { id: "user/x".into() }));
        assert_eq!(cfg.performance.fps_limit, 30);
        assert_eq!(cfg.performance.battery, PowerAction::None);
        assert!(cfg.onboarding_done);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn source_resolution() {
        let mut cfg = Config::default();
        cfg.monitors.insert("m2".into(), Source::Playlist { id: "p".into() });
        assert_eq!(cfg.source_for("m2"), Some(&Source::Playlist { id: "p".into() }));
        assert_eq!(cfg.source_for("m1"), cfg.default_source.as_ref());
        cfg.layout = LayoutMode::Span;
        assert_eq!(cfg.source_for("m2"), cfg.default_source.as_ref());
    }

    #[test]
    fn forgets_deleted_wallpaper() {
        let mut cfg = Config::default();
        cfg.default_source = Some(Source::Wallpaper { id: "user/a".into() });
        cfg.monitors.insert("m".into(), Source::Wallpaper { id: "user/a".into() });
        cfg.playlists.push(Playlist {
            id: "p".into(),
            items: vec![PlaylistItem { wallpaper: "user/a".into(), at: None }],
            ..Default::default()
        });
        cfg.forget_wallpaper("user/a");
        assert!(cfg.monitors.is_empty());
        assert!(cfg.playlists[0].items.is_empty());
        assert_eq!(cfg.default_source, Some(Source::Wallpaper { id: "builtin/aurora".into() }));
    }
}
