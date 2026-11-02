//! `wp://` custom protocol: serves wallpaper files to the webviews.
//!
//! URL layout: `wp://localhost/<source>/<folder>/<path>` (on Windows the webview
//! sees it as `http://wp.localhost/...`). Paths are canonicalized and must stay
//! inside the library root. HTTP Range requests are supported so video can seek and loop.
//!
//! Security: HTML wallpapers are served with a Content-Security-Policy that
//! blocks all network access unless the wallpaper declares the `network`
//! capability ("offline by default"). `/__wallgates/*` serves the creator SDK
//! and the Wallpaper Engine compatibility shim, which is injected into
//! imported WE web wallpapers.

use crate::library::{self, Paths};
use percent_encoding::percent_decode_str;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use tauri::http::{header, Request, Response, StatusCode};

/// Max bytes returned for an open-ended range request (`bytes=N-`).
const RANGE_CHUNK: u64 = 4 * 1024 * 1024;

fn mime(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "frag" | "glsl" | "txt" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

const SDK_JS: &str = include_str!("../../ui/sdk/wallgates-sdk.js");
const WE_SHIM_JS: &str = include_str!("../../ui/sdk/we-shim.js");

/// Allowed sources for wallpapers without the `network` capability.
const OFFLINE_CSP: &str = "default-src 'self' http://wp.localhost wp: data: blob: 'unsafe-inline' 'unsafe-eval'; \
     form-action 'none'";

fn internal(name: &str) -> Response<Vec<u8>> {
    let body = match name {
        "wallgates-sdk.js" => SDK_JS,
        "we-shim.js" => WE_SHIM_JS,
        _ => return status(StatusCode::NOT_FOUND),
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/javascript; charset=utf-8")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(body.as_bytes().to_vec())
        .unwrap()
}

/// Insert a script tag as early as possible in an HTML document.
fn inject_script(html: &[u8], src: &str) -> Vec<u8> {
    let text = String::from_utf8_lossy(html);
    let tag = format!("<script src=\"{src}\"></script>");
    let lower = text.to_ascii_lowercase();
    let pos = lower
        .find("<head")
        .and_then(|i| lower[i..].find('>').map(|j| i + j + 1))
        .or_else(|| lower.find("<html").and_then(|i| lower[i..].find('>').map(|j| i + j + 1)))
        .unwrap_or(0);
    let mut out = String::with_capacity(text.len() + tag.len());
    out.push_str(&text[..pos]);
    out.push_str(&tag);
    out.push_str(&text[pos..]);
    out.into_bytes()
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(code)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Vec::new())
        .unwrap()
}

/// Parse `bytes=start-end` (single range only).
fn parse_range(value: &str, len: u64) -> Option<(u64, u64)> {
    let spec = value.strip_prefix("bytes=")?;
    let (start, end) = spec.split_once('-')?;
    if start.is_empty() {
        // Suffix range: last N bytes.
        let n: u64 = end.parse().ok()?;
        let n = n.min(len);
        return (n > 0).then(|| (len - n, len - 1));
    }
    let start: u64 = start.parse().ok()?;
    if start >= len {
        return None;
    }
    let end = if end.is_empty() {
        (start + RANGE_CHUNK - 1).min(len - 1)
    } else {
        end.parse::<u64>().ok()?.min(len - 1)
    };
    (start <= end).then_some((start, end))
}

pub fn handle(paths: &Paths, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let decoded = percent_decode_str(request.uri().path()).decode_utf8_lossy().into_owned();
    let rel = decoded.trim_start_matches('/');
    let Some((source, rest)) = rel.split_once('/') else {
        return status(StatusCode::NOT_FOUND);
    };
    if source == "__wallgates" {
        return internal(rest);
    }
    let Some(root) = paths.root(source) else {
        return status(StatusCode::NOT_FOUND);
    };
    let (Ok(root), Ok(file)) = (root.canonicalize(), root.join(rest).canonicalize()) else {
        return status(StatusCode::NOT_FOUND);
    };
    if !file.starts_with(&root) || !file.is_file() {
        return status(StatusCode::NOT_FOUND);
    }

    let Ok(mut f) = File::open(&file) else {
        return status(StatusCode::NOT_FOUND);
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let content_type = mime(&file);
    let builder = Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CACHE_CONTROL, "no-cache");

    let range = request
        .headers()
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok());

    if let Some(range) = range {
        let Some((start, end)) = parse_range(range, len) else {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{len}"))
                .body(Vec::new())
                .unwrap();
        };
        let mut buf = vec![0u8; (end - start + 1) as usize];
        if f.seek(SeekFrom::Start(start)).is_err() || f.read_exact(&mut buf).is_err() {
            return status(StatusCode::INTERNAL_SERVER_ERROR);
        }
        return builder
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"))
            .header(header::CONTENT_LENGTH, buf.len())
            .body(buf)
            .unwrap();
    }

    let mut buf = Vec::with_capacity(len as usize);
    if f.read_to_end(&mut buf).is_err() {
        return status(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let mut builder = builder;
    if content_type.starts_with("text/html") {
        // Look up the wallpaper this page belongs to: <source>/<folder>/...
        let folder = rest.split('/').next().unwrap_or("");
        let manifest = library::find(paths, &format!("{source}/{folder}")).map(|w| w.manifest);
        let network = manifest.as_ref().is_some_and(|m| m.has("network"));
        if !network {
            builder = builder.header("Content-Security-Policy", OFFLINE_CSP);
        }
        if manifest.as_ref().and_then(|m| m.compat.as_deref()) == Some("wallpaperengine") {
            buf = inject_script(&buf, "/__wallgates/we-shim.js");
        }
    }
    builder.status(StatusCode::OK).body(buf).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_early() {
        let out = String::from_utf8(inject_script(b"<html><HEAD lang=x><title>t</title></head></html>", "/s.js")).unwrap();
        assert_eq!(out, "<html><HEAD lang=x><script src=\"/s.js\"></script><title>t</title></head></html>");
        let bare = String::from_utf8(inject_script(b"<p>x</p>", "/s.js")).unwrap();
        assert!(bare.starts_with("<script"));
    }

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=900-", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=0-5000", 1000), Some((0, 999)));
        assert_eq!(parse_range("bytes=1000-", 1000), None);
        assert_eq!(parse_range("items=0-1", 1000), None);
    }
}
