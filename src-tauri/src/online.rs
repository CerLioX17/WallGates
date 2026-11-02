//! Network features, all optional and user-controllable:
//! * the curated gallery (static files in the GitHub repository, no server),
//! * the update check (GitHub releases API).
//! Nothing else ever leaves the machine: no telemetry, no accounts.

use crate::library::{self, Paths};
use serde_json::{json, Value};
use std::fs;
use std::io::Read;
use std::time::Duration;

pub const REPO: &str = "CerLioX17/WallGates";
const GALLERY_BASE: &str = "https://raw.githubusercontent.com/CerLioX17/WallGates/main/gallery/";
const MAX_GALLERY_ITEM_BYTES: u64 = 512 * 1024 * 1024;

fn agent() -> ureq::Agent {
    let mut b = ureq::AgentBuilder::new();
    if let Ok(tls) = native_tls::TlsConnector::new() {
        b = b.tls_connector(std::sync::Arc::new(tls));
    }
    b.timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(60))
        .user_agent(concat!("Wallgates/", env!("CARGO_PKG_VERSION")))
        .build()
}

pub fn gallery_index() -> Result<Value, String> {
    let v: Value = agent()
        .get(&format!("{GALLERY_BASE}index.json"))
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;
    Ok(json!({ "base": GALLERY_BASE, "items": v.get("items").cloned().unwrap_or(json!([])) }))
}

/// Download a gallery item into the user library. Returns the new id.
pub fn gallery_install(paths: &Paths, item_id: &str) -> Result<String, String> {
    if !library::is_safe_segment(item_id) {
        return Err("invalid gallery item".into());
    }
    let index = gallery_index()?;
    let item = index["items"]
        .as_array()
        .and_then(|a| a.iter().find(|i| i.get("id").and_then(Value::as_str) == Some(item_id)))
        .cloned()
        .ok_or("gallery item not found")?;
    let files: Vec<String> = item
        .get("files")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|f| f.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    if files.is_empty() || !files.iter().any(|f| f == library::MANIFEST) {
        return Err("gallery item has no wallpaper.json".into());
    }
    let staged = library::staging_dir(paths)?;
    let result = (|| {
        let mut total = 0u64;
        for rel in &files {
            if !library::is_safe_relative(rel) {
                return Err(format!("invalid file path in gallery item: {rel}"));
            }
            let url = format!("{GALLERY_BASE}{item_id}/{}", rel.split('/').map(urlencode).collect::<Vec<_>>().join("/"));
            let resp = agent().get(&url).call().map_err(|e| format!("{rel}: {e}"))?;
            let mut buf = Vec::new();
            resp.into_reader()
                .take(MAX_GALLERY_ITEM_BYTES - total + 1)
                .read_to_end(&mut buf)
                .map_err(|e| e.to_string())?;
            total += buf.len() as u64;
            if total > MAX_GALLERY_ITEM_BYTES {
                return Err("gallery item is too large".into());
            }
            let out = staged.join(rel);
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::write(out, buf).map_err(|e| e.to_string())?;
        }
        library::adopt_folder(paths, &staged)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staged);
    }
    result
}

fn urlencode(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}

fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let core = v.split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u32>().ok());
    Some((it.next()??, it.next().flatten().unwrap_or(0), it.next().flatten().unwrap_or(0)))
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

/// Returns Some({version, url, notes}) when a newer release exists.
pub fn check_update() -> Result<Option<Value>, String> {
    let v: Value = agent()
        .get(&format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;
    let tag = v.get("tag_name").and_then(Value::as_str).unwrap_or("");
    if !is_newer(tag, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    Ok(Some(json!({
        "version": tag.trim_start_matches('v'),
        "url": v.get("html_url").and_then(Value::as_str).unwrap_or("https://github.com/CerLioX17/WallGates/releases"),
        "notes": v.get("body").and_then(Value::as_str).unwrap_or("").chars().take(2000).collect::<String>(),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("v0.3.0", "0.2.0"));
        assert!(is_newer("0.2.1", "0.2.0"));
        assert!(!is_newer("v0.2.0", "0.2.0"));
        assert!(!is_newer("garbage", "0.2.0"));
        assert!(is_newer("1.0", "0.9.9"));
    }
}
