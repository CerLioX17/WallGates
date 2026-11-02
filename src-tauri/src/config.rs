use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Persistent user settings, stored as JSON in the app config directory.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    /// Active wallpaper id, e.g. `builtin/aurora` or `user/my-video`.
    pub active: Option<String>,
    /// Per-wallpaper property overrides: wallpaper id -> (property key -> value).
    pub properties: HashMap<String, Map<String, Value>>,
    /// Paused manually by the user.
    pub paused: bool,
    pub pause_on_fullscreen: bool,
    pub pause_on_maximized: bool,
    pub pause_on_battery: bool,
    /// Frame cap for shader wallpapers (0 = uncapped / display refresh rate).
    pub fps_limit: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            active: Some("builtin/aurora".into()),
            properties: HashMap::new(),
            paused: false,
            pause_on_fullscreen: true,
            pause_on_maximized: false,
            pause_on_battery: true,
            fps_limit: 60,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
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
}

/// Partial update sent by the settings UI. Missing fields are left untouched.
#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub struct SettingsPatch {
    pub paused: Option<bool>,
    pub pause_on_fullscreen: Option<bool>,
    pub pause_on_maximized: Option<bool>,
    pub pause_on_battery: Option<bool>,
    pub fps_limit: Option<u32>,
}

impl SettingsPatch {
    pub fn apply(self, cfg: &mut Config) {
        if let Some(v) = self.paused {
            cfg.paused = v;
        }
        if let Some(v) = self.pause_on_fullscreen {
            cfg.pause_on_fullscreen = v;
        }
        if let Some(v) = self.pause_on_maximized {
            cfg.pause_on_maximized = v;
        }
        if let Some(v) = self.pause_on_battery {
            cfg.pause_on_battery = v;
        }
        if let Some(v) = self.fps_limit {
            cfg.fps_limit = v.min(240);
        }
    }
}
