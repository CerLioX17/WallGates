//! Sharing and migration:
//! * `.wallgates` packages (a zip of a wallpaper folder) — import & export.
//! * Wallpaper Engine projects (`project.json`) of type `video` and `web`,
//!   converted to Wallgates wallpapers. `scene` and `application` projects are
//!   not supported (proprietary format / executables are never run).

use crate::library::{self, copy_dir, staging_dir, Kind, Manifest, Paths};
use serde_json::{json, Map, Value};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

const MAX_PACKAGE_BYTES: u64 = 4 * 1024 * 1024 * 1024; // 4 GiB uncompressed
const MAX_PACKAGE_FILES: usize = 10_000;

// ------------------------------------------------------------------ .wallgates

pub fn export(paths: &Paths, id: &str, dest: &Path) -> Result<(), String> {
    let info = library::find(paths, id).ok_or("wallpaper not found")?;
    let root = info.dir(paths);
    let file = File::create(dest).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true);

    fn add(zip: &mut zip::ZipWriter<File>, root: &Path, dir: &Path, opts: zip::write::SimpleFileOptions, depth: u32) -> Result<(), String> {
        if depth > 16 {
            return Err("folder nesting too deep".into());
        }
        for e in fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
            let path = e.path();
            let rel = path.strip_prefix(root).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
            match e.file_type() {
                Ok(t) if t.is_dir() => {
                    zip.add_directory(format!("{rel}/"), opts).map_err(|e| e.to_string())?;
                    add(zip, root, &path, opts, depth + 1)?;
                }
                Ok(t) if t.is_file() => {
                    // Videos and images are already compressed.
                    let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
                    let o = if library::MEDIA_EXTENSIONS.contains(&ext.as_str()) {
                        opts.compression_method(zip::CompressionMethod::Stored)
                    } else {
                        opts
                    };
                    zip.start_file(rel, o).map_err(|e| e.to_string())?;
                    let mut f = File::open(&path).map_err(|e| e.to_string())?;
                    std::io::copy(&mut f, zip).map_err(|e| e.to_string())?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    add(&mut zip, &root, &root, options, 0)?;
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// Import a `.wallgates` (or `.zip`) package. The manifest may be at the root
/// or inside a single top-level folder.
pub fn import_package(paths: &Paths, file: &Path) -> Result<String, String> {
    let f = File::open(file).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(f).map_err(|e| format!("not a valid package: {e}"))?;
    if zip.len() > MAX_PACKAGE_FILES {
        return Err("package has too many files".into());
    }
    let staged = staging_dir(paths)?;
    let result = (|| {
        let mut total = 0u64;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            // enclosed_name() rejects absolute paths and `..` (zip-slip).
            let Some(rel) = entry.enclosed_name() else { continue };
            let out = staged.join(rel);
            if entry.is_dir() {
                fs::create_dir_all(&out).map_err(|e| e.to_string())?;
                continue;
            }
            total += entry.size();
            if total > MAX_PACKAGE_BYTES {
                return Err("package is too large".to_string());
            }
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut w = File::create(&out).map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                let n = entry.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                w.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            }
        }
        // Manifest in a single sub-folder?
        let root = if staged.join(library::MANIFEST).is_file() {
            staged.clone()
        } else {
            let dirs: Vec<_> = fs::read_dir(&staged)
                .map_err(|e| e.to_string())?
                .flatten()
                .filter(|e| e.path().is_dir())
                .collect();
            match dirs.as_slice() {
                [only] if only.path().join(library::MANIFEST).is_file() => only.path(),
                _ => return Err("package does not contain wallpaper.json".into()),
            }
        };
        library::adopt_folder(paths, &root)
    })();
    let _ = fs::remove_dir_all(&staged);
    result
}

// ------------------------------------------------------------------ Wallpaper Engine

/// Strip HTML tags and Wallpaper Engine localisation keys from a label.
fn clean_label(raw: &str, key: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in raw.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let out = out.trim().to_string();
    // Well-known Wallpaper Engine keys.
    match key {
        "schemecolor" => return "Scheme color".into(),
        "audioprocessing" => return "Audio processing".into(),
        _ => {}
    }
    if out.is_empty() || out.starts_with("ui_") {
        // Turn `schemecolor` / `ui_browse_properties_scheme_color` into something readable.
        let k = key.trim_start_matches("ui_").replace('_', " ");
        let mut c = k.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    } else {
        out
    }
}

/// "0.1 0.5 1" (Wallpaper Engine colour) -> "#1a80ff".
pub fn we_color_to_hex(v: &str) -> String {
    let parts: Vec<f64> = v.split_whitespace().filter_map(|p| p.parse().ok()).collect();
    if parts.len() < 3 {
        return "#ffffff".into();
    }
    let to = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", to(parts[0]), to(parts[1]), to(parts[2]))
}

fn convert_properties(general: Option<&Value>) -> Map<String, Value> {
    let mut out = Map::new();
    let Some(props) = general.and_then(|g| g.get("properties")).and_then(Value::as_object) else {
        return out;
    };
    let mut entries: Vec<(&String, &Value)> = props.iter().collect();
    entries.sort_by_key(|(_, p)| p.get("order").and_then(Value::as_i64).unwrap_or(0));
    for (key, p) in entries {
        let label = clean_label(p.get("text").and_then(Value::as_str).unwrap_or(""), key);
        let value = p.get("value").cloned().unwrap_or(Value::Null);
        let converted = match p.get("type").and_then(Value::as_str).unwrap_or("") {
            "slider" => json!({
                "label": label, "type": "slider",
                "min": p.get("min").cloned().unwrap_or(json!(0)),
                "max": p.get("max").cloned().unwrap_or(json!(100)),
                "step": if p.get("fraction").and_then(Value::as_bool).unwrap_or(false) {
                    p.get("precision").and_then(Value::as_u64).map(|n| 10f64.powi(-(n as i32))).map(Value::from).unwrap_or(json!(0.01))
                } else { json!(1) },
                "default": value,
            }),
            "color" => json!({
                "label": label, "type": "color", "we": "color",
                "default": we_color_to_hex(value.as_str().unwrap_or("1 1 1")),
            }),
            "bool" => json!({ "label": label, "type": "bool", "default": value.as_bool().unwrap_or(false) }),
            "combo" => {
                let options: Vec<Value> = p
                    .get("options")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .map(|o| {
                                let v = o.get("value").cloned().unwrap_or(Value::Null);
                                let v = match v {
                                    Value::String(s) => Value::String(s),
                                    other => Value::String(other.to_string()),
                                };
                                json!({ "value": v, "label": clean_label(o.get("label").and_then(Value::as_str).unwrap_or(""), v.as_str().unwrap_or("")) })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let default = match value {
                    Value::String(s) => s,
                    other => other.to_string(),
                };
                json!({ "label": label, "type": "select", "options": options, "default": default })
            }
            "textinput" => json!({ "label": label, "type": "text", "default": value.as_str().unwrap_or("") }),
            "group" => json!({ "label": label, "type": "group" }),
            // Wallpaper Engine "text" entries are labels/notes in the settings panel.
            "text" if label.chars().any(|c| c.is_alphanumeric()) && label != clean_label("", key) => {
                json!({ "label": label, "type": "note" })
            }
            _ => continue, // file pickers, directories, groups: not supported
        };
        out.insert(key.clone(), converted);
    }
    out
}

pub struct WeImportReport {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
}

/// Import one Wallpaper Engine project folder (containing project.json).
pub fn import_we_project(paths: &Paths, folder: &Path) -> Result<String, String> {
    let raw = fs::read(folder.join("project.json")).map_err(|_| "no project.json in this folder".to_string())?;
    let project: Value = serde_json::from_slice(&raw).map_err(|e| format!("invalid project.json: {e}"))?;
    let kind = project.get("type").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
    let file = project.get("file").and_then(Value::as_str).unwrap_or("").to_string();
    let title = project.get("title").and_then(Value::as_str).unwrap_or("Imported wallpaper").to_string();
    if !library::is_safe_relative(&file) || !folder.join(&file).is_file() {
        return Err(format!("{title}: entry file not found"));
    }
    let mut properties = convert_properties(project.get("general"));
    let (kind, compat) = match kind.as_str() {
        "video" => {
            properties.insert("fit".into(), library::fit_property());
            library::video_properties(&mut properties);
            (Kind::Video, None)
        }
        "web" => (Kind::Html, Some("wallpaperengine".to_string())),
        "scene" => return Err(format!("{title}: scene wallpapers use a proprietary format and can't be imported")),
        "application" => return Err(format!("{title}: application wallpapers run executables and are never imported")),
        other => return Err(format!("{title}: unsupported type '{other}'")),
    };

    let staged = staging_dir(paths)?;
    let result = (|| {
        copy_dir(folder, &staged, 0)?;
        let preview = project
            .get("preview")
            .and_then(Value::as_str)
            .filter(|p| library::is_safe_relative(p) && staged.join(p).is_file())
            .map(str::to_string);
        let mut tags: Vec<String> = project
            .get("tags")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|t| t.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        tags.push("wallpaper-engine".into());
        let rating = project
            .get("contentrating")
            .and_then(Value::as_str)
            .map(|r| if r.eq_ignore_ascii_case("everyone") { "everyone" } else { "mature" }.to_string());
        let manifest = Manifest {
            name: title.clone(),
            author: String::new(),
            description: project.get("description").and_then(Value::as_str).unwrap_or("").chars().take(500).collect(),
            kind,
            entry: file.clone(),
            preview,
            properties: properties.clone(),
            // Web wallpapers from WE may use audio; network stays off unless the user enables it.
            capabilities: if kind == Kind::Html { vec!["audio".into(), "media".into()] } else { Vec::new() },
            tags,
            license: None,
            version: None,
            ai_generated: false,
            rating,
            compat: compat.clone(),
        };
        library::write_manifest(&staged, &manifest)?;
        library::adopt_folder(paths, &staged)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staged);
    }
    result
}

/// Import a single project folder, or every project found one level below
/// (e.g. `steamapps/workshop/content/431960`).
pub fn import_we(paths: &Paths, folder: &Path) -> Result<WeImportReport, String> {
    let mut report = WeImportReport { imported: Vec::new(), skipped: Vec::new() };
    if folder.join("project.json").is_file() {
        match import_we_project(paths, folder) {
            Ok(id) => report.imported.push(id),
            Err(e) => report.skipped.push(e),
        }
        return Ok(report);
    }
    let entries = fs::read_dir(folder).map_err(|e| e.to_string())?;
    for e in entries.flatten() {
        let p = e.path();
        if p.join("project.json").is_file() {
            match import_we_project(paths, &p) {
                Ok(id) => report.imported.push(id),
                Err(err) => report.skipped.push(err),
            }
        }
    }
    if report.imported.is_empty() && report.skipped.is_empty() {
        return Err("no Wallpaper Engine projects (project.json) found in this folder".into());
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths(tag: &str) -> Paths {
        let base = std::env::temp_dir().join(format!("wg-pkg-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let p = Paths { builtin: base.join("b"), user: base.join("u"), data: base.join("d"), config_file: base.join("c.json") };
        fs::create_dir_all(&p.user).unwrap();
        p
    }

    #[test]
    fn colors() {
        assert_eq!(we_color_to_hex("1 0.5 0"), "#ff8000");
        assert_eq!(we_color_to_hex("bad"), "#ffffff");
    }

    #[test]
    fn package_roundtrip() {
        let p = test_paths("rt");
        let id = library::create_from_template(&p, "html", "Round Trip").unwrap();
        let out = p.data.join("x.wallgates");
        fs::create_dir_all(&p.data).unwrap();
        export(&p, &id, &out).unwrap();
        let id2 = import_package(&p, &out).unwrap();
        assert_ne!(id, id2);
        assert_eq!(library::find(&p, &id2).unwrap().manifest.name, "Round Trip");
        let _ = fs::remove_dir_all(p.user.parent().unwrap());
    }

    #[test]
    fn rejects_zip_slip() {
        let p = test_paths("slip");
        fs::create_dir_all(&p.data).unwrap();
        let bad = p.data.join("bad.wallgates");
        let mut zip = zip::ZipWriter::new(File::create(&bad).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        zip.start_file("../../evil.txt", o).unwrap();
        zip.write_all(b"x").unwrap();
        zip.finish().unwrap();
        assert!(import_package(&p, &bad).is_err());
        assert!(!p.user.parent().unwrap().join("evil.txt").exists());
        let _ = fs::remove_dir_all(p.user.parent().unwrap());
    }

    #[test]
    fn imports_we_web_project() {
        let p = test_paths("we");
        let src = p.data.join("123456");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("index.html"), "<html><head></head><body></body></html>").unwrap();
        fs::write(
            src.join("project.json"),
            r#"{"title":"Rainy Window","type":"web","file":"index.html","contentrating":"Everyone",
                "general":{"properties":{
                  "schemecolor":{"order":0,"text":"ui_browse_properties_scheme_color","type":"color","value":"0 0 1"},
                  "speed":{"order":1,"text":"<b>Speed</b>","type":"slider","min":0,"max":10,"value":3},
                  "mode":{"order":2,"text":"Mode","type":"combo","value":1,"options":[{"label":"A","value":1},{"label":"B","value":2}]}
                }}}"#,
        )
        .unwrap();
        fs::write(p.data.join("readme.txt"), "x").unwrap();
        let report = import_we(&p, &p.data).unwrap();
        assert_eq!(report.imported.len(), 1, "{:?}", report.skipped);
        let info = library::find(&p, &report.imported[0]).unwrap();
        assert_eq!(info.manifest.compat.as_deref(), Some("wallpaperengine"));
        assert_eq!(info.manifest.properties["schemecolor"]["default"], json!("#0000ff"));
        assert_eq!(info.manifest.properties["speed"]["label"], json!("Speed"));
        assert_eq!(info.manifest.properties["mode"]["default"], json!("1"));
        assert_eq!(info.manifest.rating.as_deref(), Some("everyone"));
        let _ = fs::remove_dir_all(p.user.parent().unwrap());
    }
}
