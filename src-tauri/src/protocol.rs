//! `wp://` custom protocol: serves wallpaper files to the webviews.
//!
//! URL layout: `wp://localhost/<source>/<folder>/<path>` (on Windows the webview
//! sees it as `http://wp.localhost/...`). Paths are canonicalized and must stay
//! inside the library root. HTTP Range requests are supported so video can seek and loop.

use crate::library::Paths;
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
    builder.status(StatusCode::OK).body(buf).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

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
