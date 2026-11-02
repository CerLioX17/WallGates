//! Wallpaper library: discovery, manifests, import and deletion.
//!
//! A wallpaper is a folder containing `wallpaper.json` (see docs/wallpaper-format.md).
//! Wallpapers live in two roots: `builtin` (shipped with the app, read-only)
//! and `user` (app data directory, writable). Ids are `<root>/<folder>`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const MANIFEST: &str = "wallpaper.json";

#[derive(Clone, Debug)]
pub struct Paths {
    pub builtin: PathBuf,
    pub user: PathBuf,
    pub config_file: PathBuf,
}

impl Paths {
    pub fn root(&self, source: &str) -> Option<&Path> {
        match source {
            "builtin" => Some(&self.builtin),
            "user" => Some(&self.user),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Image,
    Video,
    Html,
    Shader,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Manifest {
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "type")]
    pub kind: Kind,
    pub entry: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    /// Ordered map of property key -> property definition (label, type, default, ...).
    #[serde(default)]
    pub properties: Map<String, Value>,
}

#[derive(Serialize, Clone, Debug)]
pub struct WallpaperInfo {
    pub id: String,
    pub source: String,
    pub editable: bool,
    pub manifest: Manifest,
}

impl WallpaperInfo {
    /// Property defaults merged with the user's saved overrides.
    /// Overrides for keys the manifest no longer declares are dropped.
    pub fn values(&self, overrides: Option<&Map<String, Value>>) -> Map<String, Value> {
        let mut out = Map::new();
        for (key, def) in &self.manifest.properties {
            let default = def.get("default").cloned().unwrap_or(Value::Null);
            let value = overrides.and_then(|o| o.get(key)).cloned().unwrap_or(default);
            out.insert(key.clone(), value);
        }
        out
    }
}

/// A single path segment that cannot escape its parent directory.
fn is_safe_segment(s: &str) -> bool {
    !s.is_empty() && !s.contains(['/', '\\']) && s != "." && s != ".."
}

/// Relative path that stays inside the wallpaper folder.
fn is_safe_relative(p: &str) -> bool {
    let path = Path::new(p);
    !p.is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)))
}

fn load(dir: &Path, source: &str) -> Result<WallpaperInfo, String> {
    let folder = dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("invalid folder name")?
        .to_string();
    let raw = fs::read(dir.join(MANIFEST)).map_err(|e| format!("{}: {e}", dir.display()))?;
    let manifest: Manifest =
        serde_json::from_slice(&raw).map_err(|e| format!("{}: {e}", dir.display()))?;
    if !is_safe_relative(&manifest.entry) || !dir.join(&manifest.entry).is_file() {
        return Err(format!("{}: entry '{}' not found", dir.display(), manifest.entry));
    }
    Ok(WallpaperInfo {
        id: format!("{source}/{folder}"),
        source: source.to_string(),
        editable: source == "user",
        manifest,
    })
}

pub fn scan(paths: &Paths) -> Vec<WallpaperInfo> {
    let mut out = Vec::new();
    for source in ["builtin", "user"] {
        let Some(root) = paths.root(source) else { continue };
        let Ok(entries) = fs::read_dir(root) else { continue };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.join(MANIFEST).is_file() {
                continue;
            }
            match load(&dir, source) {
                Ok(info) => out.push(info),
                Err(e) => eprintln!("[wallgates] skipping wallpaper: {e}"),
            }
        }
    }
    out.sort_by(|a, b| {
        (a.source != "builtin", a.manifest.name.to_lowercase())
            .cmp(&(b.source != "builtin", b.manifest.name.to_lowercase()))
    });
    out
}

pub fn find(paths: &Paths, id: &str) -> Option<WallpaperInfo> {
    let (source, folder) = id.split_once('/')?;
    if !is_safe_segment(folder) {
        return None;
    }
    load(&paths.root(source)?.join(folder), source).ok()
}

fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-').chars().take(48).collect::<String>();
    if slug.is_empty() {
        "wallpaper".into()
    } else {
        slug
    }
}

fn unique_dir(root: &Path, name: &str) -> PathBuf {
    let base = slugify(name);
    let mut candidate = root.join(&base);
    let mut n = 2;
    while candidate.exists() {
        candidate = root.join(format!("{base}-{n}"));
        n += 1;
    }
    candidate
}

const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp", "avif"];
const VIDEO_EXT: &[&str] = &["mp4", "webm", "m4v", "mov"];

pub const MEDIA_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "avif", "mp4", "webm", "m4v", "mov",
];

fn fit_property() -> Value {
    json!({
        "label": "Fit",
        "type": "select",
        "options": [
            { "value": "cover", "label": "Fill (crop)" },
            { "value": "contain", "label": "Fit (letterbox)" },
            { "value": "fill", "label": "Stretch" }
        ],
        "default": "cover"
    })
}

/// Turn a single image or video file into a user wallpaper. Returns the new id.
pub fn import_file(paths: &Paths, file: &Path) -> Result<String, String> {
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .ok_or("file has no extension")?;
    let kind = if IMAGE_EXT.contains(&ext.as_str()) {
        Kind::Image
    } else if VIDEO_EXT.contains(&ext.as_str()) {
        Kind::Video
    } else {
        return Err(format!("unsupported file type: .{ext}"));
    };
    let name = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Wallpaper")
        .to_string();

    let dir = unique_dir(&paths.user, &name);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let entry = format!("media.{ext}");
    fs::copy(file, dir.join(&entry)).map_err(|e| e.to_string())?;

    let mut properties = Map::new();
    properties.insert("fit".into(), fit_property());
    if kind == Kind::Video {
        properties.insert(
            "speed".into(),
            json!({ "label": "Playback speed", "type": "slider", "min": 0.25, "max": 2, "step": 0.05, "default": 1 }),
        );
        properties.insert(
            "volume".into(),
            json!({ "label": "Volume", "type": "slider", "min": 0, "max": 1, "step": 0.05, "default": 0 }),
        );
    }
    let manifest = Manifest {
        name,
        author: String::new(),
        description: String::new(),
        kind,
        entry: entry.clone(),
        preview: (kind == Kind::Image).then_some(entry),
        properties,
    };
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    fs::write(dir.join(MANIFEST), bytes).map_err(|e| e.to_string())?;
    Ok(format!("user/{}", dir.file_name().unwrap().to_string_lossy()))
}

fn copy_dir(from: &Path, to: &Path, depth: u32) -> Result<(), String> {
    if depth > 16 {
        return Err("folder nesting too deep".into());
    }
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let target = to.join(entry.file_name());
        if ty.is_dir() {
            copy_dir(&entry.path(), &target, depth + 1)?;
        } else if ty.is_file() {
            fs::copy(entry.path(), target).map_err(|e| e.to_string())?;
        }
        // Symlinks are skipped on purpose: they could point outside the wallpaper.
    }
    Ok(())
}

/// Copy a wallpaper folder (one that contains wallpaper.json) into the user library.
pub fn import_folder(paths: &Paths, folder: &Path) -> Result<String, String> {
    let raw = fs::read(folder.join(MANIFEST))
        .map_err(|_| format!("no {MANIFEST} found in the selected folder"))?;
    let manifest: Manifest = serde_json::from_slice(&raw).map_err(|e| format!("invalid {MANIFEST}: {e}"))?;
    let dir = unique_dir(&paths.user, &manifest.name);
    copy_dir(folder, &dir, 0)?;
    let id = format!("user/{}", dir.file_name().unwrap().to_string_lossy());
    match load(&dir, "user") {
        Ok(_) => Ok(id),
        Err(e) => {
            let _ = fs::remove_dir_all(&dir);
            Err(e)
        }
    }
}

pub fn delete(paths: &Paths, id: &str) -> Result<(), String> {
    let (source, folder) = id.split_once('/').ok_or("invalid id")?;
    if source != "user" || !is_safe_segment(folder) {
        return Err("only user wallpapers can be deleted".into());
    }
    fs::remove_dir_all(paths.user.join(folder)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slugify("My Cool Video!"), "my-cool-video");
        assert_eq!(slugify("???"), "wallpaper");
    }

    #[test]
    fn rejects_traversal() {
        assert!(!is_safe_segment(".."));
        assert!(!is_safe_segment("a/b"));
        assert!(!is_safe_relative("../x"));
        assert!(!is_safe_relative("/etc/passwd"));
        assert!(is_safe_relative("assets/a.png"));
    }

    #[test]
    fn values_merge_defaults_and_overrides() {
        let manifest: Manifest = serde_json::from_value(json!({
            "name": "t", "type": "shader", "entry": "a.frag",
            "properties": { "speed": { "type": "slider", "default": 1 }, "c": { "type": "color", "default": "#fff" } }
        }))
        .unwrap();
        let info = WallpaperInfo { id: "builtin/t".into(), source: "builtin".into(), editable: false, manifest };
        let mut o = Map::new();
        o.insert("speed".into(), json!(2));
        o.insert("stale".into(), json!(true));
        let v = info.values(Some(&o));
        assert_eq!(v["speed"], json!(2));
        assert_eq!(v["c"], json!("#fff"));
        assert!(!v.contains_key("stale"));
    }
}
