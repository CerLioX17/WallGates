//! Wallpaper library: discovery, manifests, import, editing and deletion.
//!
//! A wallpaper is a folder containing `wallpaper.json` (see docs/wallpaper-format.md).
//! Wallpapers live in two roots: `builtin` (shipped with the app, read-only)
//! and `user` (app data directory, writable). Ids are `<root>/<folder>`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const MANIFEST: &str = "wallpaper.json";

/// Data a wallpaper may request. The user sees these as permission badges.
pub const CAPABILITIES: &[&str] = &["audio", "mouse", "system", "media", "weather", "markets", "network"];

#[derive(Clone, Debug)]
pub struct Paths {
    pub builtin: PathBuf,
    pub user: PathBuf,
    pub data: PathBuf,
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
    /// Requested data/permissions, see [`CAPABILITIES`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Provenance: the wallpaper was made with generative AI.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ai_generated: bool,
    /// "everyone" (default) or "mature".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<String>,
    /// Compatibility layer, e.g. "wallpaperengine" for imported web wallpapers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compat: Option<String>,
}

impl Manifest {
    pub fn has(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct WallpaperInfo {
    pub id: String,
    pub source: String,
    pub editable: bool,
    pub manifest: Manifest,
    pub size_bytes: u64,
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

    pub fn dir(&self, paths: &Paths) -> PathBuf {
        let (source, folder) = self.id.split_once('/').unwrap_or(("user", &self.id));
        paths.root(source).unwrap_or(&paths.user).join(folder)
    }
}

/// A single path segment that cannot escape its parent directory.
pub fn is_safe_segment(s: &str) -> bool {
    !s.is_empty() && !s.contains(['/', '\\', ':']) && s != "." && s != ".."
}

/// Relative path that stays inside the wallpaper folder.
pub fn is_safe_relative(p: &str) -> bool {
    let path = Path::new(p);
    !p.is_empty()
        && !p.contains(':')
        && path.components().all(|c| matches!(c, Component::Normal(_)))
}

fn dir_size(dir: &Path, depth: u32) -> u64 {
    if depth > 8 {
        return 0;
    }
    let Ok(entries) = fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path(), depth + 1),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

pub fn parse_manifest(dir: &Path) -> Result<Manifest, String> {
    let raw = fs::read(dir.join(MANIFEST)).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut manifest: Manifest =
        serde_json::from_slice(&raw).map_err(|e| format!("{}: {e}", dir.display()))?;
    manifest.capabilities.retain(|c| CAPABILITIES.contains(&c.as_str()));
    if !is_safe_relative(&manifest.entry) || !dir.join(&manifest.entry).is_file() {
        return Err(format!("{}: entry '{}' not found", dir.display(), manifest.entry));
    }
    if let Some(p) = &manifest.preview {
        if !is_safe_relative(p) || !dir.join(p).is_file() {
            manifest.preview = None;
        }
    }
    Ok(manifest)
}

fn load(dir: &Path, source: &str) -> Result<WallpaperInfo, String> {
    let folder = dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("invalid folder name")?
        .to_string();
    let manifest = parse_manifest(dir)?;
    Ok(WallpaperInfo {
        id: format!("{source}/{folder}"),
        source: source.to_string(),
        editable: source == "user",
        manifest,
        size_bytes: dir_size(dir, 0),
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

pub fn slugify(name: &str) -> String {
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

pub fn unique_dir(root: &Path, name: &str) -> PathBuf {
    let base = slugify(name);
    let mut candidate = root.join(&base);
    let mut n = 2;
    while candidate.exists() {
        candidate = root.join(format!("{base}-{n}"));
        n += 1;
    }
    candidate
}

pub fn id_of(dir: &Path) -> String {
    format!("user/{}", dir.file_name().unwrap_or_default().to_string_lossy())
}

pub const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp", "avif"];
pub const VIDEO_EXT: &[&str] = &["mp4", "webm", "m4v", "mov"];

pub const MEDIA_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "avif", "mp4", "webm", "m4v", "mov",
];

pub fn fit_property() -> Value {
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

pub fn video_properties(props: &mut Map<String, Value>) {
    props.insert(
        "speed".into(),
        json!({ "label": "Playback speed", "type": "slider", "min": 0.25, "max": 2, "step": 0.05, "default": 1 }),
    );
    props.insert(
        "volume".into(),
        json!({ "label": "Volume", "type": "slider", "min": 0, "max": 1, "step": 0.05, "default": 0 }),
    );
}

pub fn write_manifest(dir: &Path, manifest: &Manifest) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|e| e.to_string())?;
    fs::write(dir.join(MANIFEST), bytes).map_err(|e| e.to_string())
}

pub fn kind_of_media(path: &Path) -> Option<(Kind, String)> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if IMAGE_EXT.contains(&ext.as_str()) {
        Some((Kind::Image, ext))
    } else if VIDEO_EXT.contains(&ext.as_str()) {
        Some((Kind::Video, ext))
    } else {
        None
    }
}

/// Turn a single image or video file into a user wallpaper. Returns the new id.
pub fn import_file(paths: &Paths, file: &Path) -> Result<String, String> {
    let (kind, ext) = kind_of_media(file).ok_or("unsupported file type")?;
    let name = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Wallpaper")
        .to_string();

    let dir = unique_dir(&paths.user, &name);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let entry = format!("media.{ext}");
    if let Err(e) = fs::copy(file, dir.join(&entry)) {
        let _ = fs::remove_dir_all(&dir);
        return Err(e.to_string());
    }

    let mut properties = Map::new();
    properties.insert("fit".into(), fit_property());
    if kind == Kind::Video {
        video_properties(&mut properties);
    }
    let manifest = Manifest {
        name,
        author: String::new(),
        description: String::new(),
        kind,
        entry: entry.clone(),
        preview: (kind == Kind::Image).then_some(entry),
        properties,
        capabilities: Vec::new(),
        tags: Vec::new(),
        license: None,
        version: None,
        ai_generated: false,
        rating: None,
        compat: None,
    };
    write_manifest(&dir, &manifest)?;
    Ok(id_of(&dir))
}

pub fn copy_dir(from: &Path, to: &Path, depth: u32) -> Result<(), String> {
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

/// Validate a staged folder and move it into the user library.
pub fn adopt_folder(paths: &Paths, staged: &Path) -> Result<String, String> {
    let manifest = parse_manifest(staged)?;
    let dir = unique_dir(&paths.user, &manifest.name);
    if fs::rename(staged, &dir).is_err() {
        copy_dir(staged, &dir, 0)?;
        let _ = fs::remove_dir_all(staged);
    }
    Ok(id_of(&dir))
}

/// Copy a wallpaper folder (one that contains wallpaper.json) into the user library.
pub fn import_folder(paths: &Paths, folder: &Path) -> Result<String, String> {
    parse_manifest(folder).map_err(|e| format!("not a Wallgates wallpaper folder ({e})"))?;
    let staged = staging_dir(paths)?;
    copy_dir(folder, &staged, 0)?;
    adopt_folder(paths, &staged).inspect_err(|_| {
        let _ = fs::remove_dir_all(&staged);
    })
}

pub fn staging_dir(paths: &Paths) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = paths.data.join("staging").join(format!("{nanos}"));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn delete(paths: &Paths, id: &str) -> Result<(), String> {
    let (source, folder) = id.split_once('/').ok_or("invalid id")?;
    if source != "user" || !is_safe_segment(folder) {
        return Err("only your own wallpapers can be deleted".into());
    }
    fs::remove_dir_all(paths.user.join(folder)).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ editor

/// Resolve a file inside a user wallpaper for editing.
fn editable_path(paths: &Paths, id: &str, rel: &str) -> Result<PathBuf, String> {
    let (source, folder) = id.split_once('/').ok_or("invalid id")?;
    if source != "user" || !is_safe_segment(folder) {
        return Err("built-in wallpapers are read-only: duplicate it first".into());
    }
    if !is_safe_relative(rel) {
        return Err("invalid file path".into());
    }
    Ok(paths.user.join(folder).join(rel))
}

const TEXT_EXT: &[&str] = &["html", "htm", "js", "mjs", "css", "json", "frag", "glsl", "txt", "svg", "md"];

pub fn list_text_files(paths: &Paths, id: &str) -> Result<Vec<String>, String> {
    let info = find(paths, id).ok_or("wallpaper not found")?;
    let root = info.dir(paths);
    let mut out = Vec::new();
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>, depth: u32) {
        if depth > 6 || out.len() > 200 {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() => walk(root, &p, out, depth + 1),
                Ok(t) if t.is_file() => {
                    let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
                    if TEXT_EXT.contains(&ext.as_str()) {
                        if let Ok(rel) = p.strip_prefix(root) {
                            out.push(rel.to_string_lossy().replace('\\', "/"));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    walk(&root, &root, &mut out, 0);
    out.sort_by_key(|f| (f != MANIFEST, f != &info.manifest.entry, f.clone()));
    Ok(out)
}

pub fn read_text(paths: &Paths, id: &str, rel: &str) -> Result<String, String> {
    let info = find(paths, id).ok_or("wallpaper not found")?;
    if !is_safe_relative(rel) {
        return Err("invalid file path".into());
    }
    let path = info.dir(paths).join(rel);
    if fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > 2 * 1024 * 1024 {
        return Err("file too large to edit".into());
    }
    fs::read_to_string(path).map_err(|e| e.to_string())
}

pub fn write_text(paths: &Paths, id: &str, rel: &str, content: &str) -> Result<(), String> {
    let path = editable_path(paths, id, rel)?;
    if rel == MANIFEST {
        // Refuse a manifest that would break the wallpaper.
        let m: Manifest = serde_json::from_str(content).map_err(|e| format!("wallpaper.json: {e}"))?;
        if !is_safe_relative(&m.entry) {
            return Err("wallpaper.json: invalid entry".into());
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, content).map_err(|e| e.to_string())
}

/// Copy any wallpaper (e.g. a built-in) into the user library so it can be edited.
pub fn duplicate(paths: &Paths, id: &str) -> Result<String, String> {
    let info = find(paths, id).ok_or("wallpaper not found")?;
    let staged = staging_dir(paths)?;
    copy_dir(&info.dir(paths), &staged, 0)?;
    let mut manifest = parse_manifest(&staged)?;
    manifest.name = format!("{} (copy)", manifest.name);
    write_manifest(&staged, &manifest)?;
    adopt_folder(paths, &staged)
}

pub const SHADER_TEMPLATE: &str = r#"// Write a Shadertoy-style shader. Uniforms u_<property> come from wallpaper.json.
// Available: iResolution, iTime, iTimeDelta, iFrame, iMouse, iAudio[64], iAudioLevel.

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / iResolution.xy;
  float t = iTime * u_speed;
  vec3 col = 0.5 + 0.5 * cos(t + uv.xyx * 3.0 + vec3(0.0, 2.0, 4.0));
  col = mix(col, u_tint, 0.35);
  // React to music: brighten with the overall audio level.
  col *= 0.8 + 0.6 * iAudioLevel;
  fragColor = vec4(col, 1.0);
}
"#;

pub const HTML_TEMPLATE: &str = r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<style>
  html, body { margin: 0; height: 100%; overflow: hidden; }
  body { display: grid; place-items: center; background: var(--bg, #0b0b16); color: #fff;
         font: 300 8vw/1 system-ui, sans-serif; transition: background .4s; }
</style>
</head>
<body>
<div id="title">Hello, Wallgates</div>
<script>
  // Wallgates sends { type: 'wallgates', event, values, paused, ... } via postMessage.
  addEventListener('message', (e) => {
    const m = e.data;
    if (!m || m.type !== 'wallgates') return;
    if (m.values) {
      document.body.style.setProperty('--bg', m.values.background);
      document.getElementById('title').textContent = m.values.text;
    }
  });
</script>
</body>
</html>
"#;

/// Create a new user wallpaper from a template. Returns its id.
pub fn create_from_template(paths: &Paths, template: &str, name: &str) -> Result<String, String> {
    let staged = staging_dir(paths)?;
    let name = if name.trim().is_empty() { "My wallpaper" } else { name.trim() };
    let (kind, entry, content, properties) = match template {
        "shader" => (
            Kind::Shader,
            "main.frag",
            SHADER_TEMPLATE,
            json!({
                "speed": { "label": "Speed", "type": "slider", "min": 0, "max": 3, "step": 0.05, "default": 1 },
                "tint": { "label": "Tint", "type": "color", "default": "#8b5cf6" }
            }),
        ),
        "html" => (
            Kind::Html,
            "index.html",
            HTML_TEMPLATE,
            json!({
                "text": { "label": "Text", "type": "text", "default": "Hello, Wallgates" },
                "background": { "label": "Background", "type": "color", "default": "#0b0b16" }
            }),
        ),
        _ => return Err("unknown template".into()),
    };
    fs::write(staged.join(entry), content).map_err(|e| e.to_string())?;
    let manifest = Manifest {
        name: name.to_string(),
        author: String::new(),
        description: String::new(),
        kind,
        entry: entry.into(),
        preview: None,
        properties: properties.as_object().cloned().unwrap_or_default(),
        capabilities: if kind == Kind::Shader { vec!["audio".into()] } else { Vec::new() },
        tags: Vec::new(),
        license: Some("CC-BY-4.0".into()),
        version: Some("1.0.0".into()),
        ai_generated: false,
        rating: None,
        compat: None,
    };
    write_manifest(&staged, &manifest)?;
    adopt_folder(paths, &staged)
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
        assert!(!is_safe_segment("C:"));
        assert!(!is_safe_relative("../x"));
        assert!(!is_safe_relative("/etc/passwd"));
        assert!(!is_safe_relative("C:/Windows"));
        assert!(is_safe_relative("assets/a.png"));
    }

    fn test_paths(tag: &str) -> Paths {
        let base = std::env::temp_dir().join(format!("wg-lib-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let p = Paths {
            builtin: base.join("builtin"),
            user: base.join("user"),
            data: base.join("data"),
            config_file: base.join("config.json"),
        };
        fs::create_dir_all(&p.builtin).unwrap();
        fs::create_dir_all(&p.user).unwrap();
        p
    }

    #[test]
    fn values_merge_defaults_and_overrides() {
        let manifest: Manifest = serde_json::from_value(json!({
            "name": "t", "type": "shader", "entry": "a.frag",
            "properties": { "speed": { "type": "slider", "default": 1 }, "c": { "type": "color", "default": "#fff" } }
        }))
        .unwrap();
        let info = WallpaperInfo { id: "builtin/t".into(), source: "builtin".into(), editable: false, manifest, size_bytes: 0 };
        let mut o = Map::new();
        o.insert("speed".into(), json!(2));
        o.insert("stale".into(), json!(true));
        let v = info.values(Some(&o));
        assert_eq!(v["speed"], json!(2));
        assert_eq!(v["c"], json!("#fff"));
        assert!(!v.contains_key("stale"));
    }

    #[test]
    fn template_edit_duplicate_delete() {
        let p = test_paths("tpl");
        let id = create_from_template(&p, "shader", "Neon").unwrap();
        assert_eq!(id, "user/neon");
        let files = list_text_files(&p, &id).unwrap();
        assert_eq!(files[0], MANIFEST);
        assert!(files.contains(&"main.frag".to_string()));
        write_text(&p, &id, "main.frag", "void mainImage(out vec4 c, in vec2 f){c=vec4(1);}").unwrap();
        assert!(read_text(&p, &id, "main.frag").unwrap().contains("vec4(1)"));
        assert!(write_text(&p, &id, MANIFEST, "{ broken").is_err());
        assert!(write_text(&p, &id, "../escape.txt", "x").is_err());
        let copy = duplicate(&p, &id).unwrap();
        assert_eq!(find(&p, &copy).unwrap().manifest.name, "Neon (copy)");
        delete(&p, &copy).unwrap();
        assert!(find(&p, &copy).is_none());
        assert!(delete(&p, "builtin/x").is_err());
        let _ = fs::remove_dir_all(p.user.parent().unwrap());
    }
}
