//! Agent access: lets coding agents (Claude Code, Cursor, Codex, …) drive Wallgates through MCP.
//!
//! Two halves:
//! - **Local API** (inside the running app): a tiny HTTP server on `127.0.0.1` with a random port and
//!   token, active only while *Settings → Agent access* is on. Its address and token are written to
//!   `<data dir>/agent.json`, readable only by the current user, and removed when access is turned off.
//! - **MCP server** (`Wallgates --mcp`): speaks the Model Context Protocol over stdio and forwards every
//!   tool call to the local API of the running app. No Node or Python needed.

use serde_json::{json, Value};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// Where the running app publishes its API address. Must match `app_data_dir()` of the app.
pub fn data_dir() -> PathBuf {
    let id = "app.wallgates.desktop";
    #[cfg(windows)]
    {
        if let Some(a) = std::env::var_os("APPDATA") {
            return PathBuf::from(a).join(id);
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(h) = std::env::var_os("HOME") {
            return PathBuf::from(h).join("Library/Application Support").join(id);
        }
    }
    if let Some(x) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(x).join(id);
    }
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share").join(id)
}

pub fn token() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for i in 0..4u64 {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(i);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        h.write_u32(std::process::id());
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

// ------------------------------------------------------------------ tool catalogue

/// Tools offered to agents. `handler` names are dispatched by the app (see `lib.rs::agent_call`).
pub fn tools() -> Value {
    let s = |props: Value, required: &[&str]| json!({ "type": "object", "properties": props, "required": required });
    let id = json!({ "type": "string", "description": "Wallpaper id, e.g. builtin/system-terminal or user/my-wallpaper (see list_wallpapers)" });
    json!([
        { "name": "status", "description": "Displays, what each one shows, pause state, app version and recent wallpaper errors.",
          "inputSchema": s(json!({}), &[]) },
        { "name": "list_wallpapers", "description": "Every wallpaper in the library with id, name, type, source (builtin/user), description and option names.",
          "inputSchema": s(json!({}), &[]) },
        { "name": "get_wallpaper", "description": "One wallpaper in full: manifest (all options with type, range, default), current option values, its files and folder.",
          "inputSchema": s(json!({ "id": id }), &["id"]) },
        { "name": "set_wallpaper", "description": "Show a wallpaper. display: 1, 2, … (as numbered in Wallgates, left to right) or \"all\" (default).",
          "inputSchema": s(json!({ "id": id, "display": { "type": ["integer", "string"], "description": "1-based display number or \"all\"" } }), &["id"]) },
        { "name": "set_options", "description": "Change one or more options of a wallpaper (keys from get_wallpaper). Applied live.",
          "inputSchema": s(json!({ "id": id, "values": { "type": "object", "description": "{ optionKey: value }" } }), &["id", "values"]) },
        { "name": "reset_options", "description": "Reset all options of a wallpaper to their defaults.",
          "inputSchema": s(json!({ "id": id }), &["id"]) },
        { "name": "duplicate_wallpaper", "description": "Copy a wallpaper (e.g. a built-in) into the user library so its code can be customised. Returns the new id.",
          "inputSchema": s(json!({ "id": id }), &["id"]) },
        { "name": "create_wallpaper", "description": "Create a new user wallpaper from a template and show it. template: \"html\" or \"shader\".",
          "inputSchema": s(json!({ "name": { "type": "string" }, "template": { "type": "string", "enum": ["html", "shader"] } }), &["name", "template"]) },
        { "name": "list_files", "description": "Text files of a wallpaper (relative paths).",
          "inputSchema": s(json!({ "id": id }), &["id"]) },
        { "name": "read_file", "description": "Read a text file of a wallpaper.",
          "inputSchema": s(json!({ "id": id, "path": { "type": "string" } }), &["id", "path"]) },
        { "name": "write_file", "description": "Write a text file of a USER wallpaper (duplicate built-ins first). The desktop reloads immediately; then call status to see compile/script errors.",
          "inputSchema": s(json!({ "id": id, "path": { "type": "string" }, "content": { "type": "string" } }), &["id", "path", "content"]) },
        { "name": "pause", "description": "Pause or resume all wallpapers.",
          "inputSchema": s(json!({ "paused": { "type": "boolean" } }), &["paused"]) },
        { "name": "next_wallpaper", "description": "Advance playlists / switch to the next wallpaper.",
          "inputSchema": s(json!({}), &[]) },
        { "name": "authoring_guide", "description": "The wallpaper format reference (manifest, option types, capabilities, SDK, shader uniforms). Read this before writing wallpaper code.",
          "inputSchema": s(json!({}), &[]) }
    ])
}

pub const GUIDE: &str = include_str!("../../docs/wallpaper-format.md");

// ------------------------------------------------------------------ local API (inside the app)

pub struct Server {
    pub port: u16,
    pub token: String,
}

/// Start the local API. `call` runs a tool; `enabled` says whether agent access is on right now.
pub fn serve<C, E>(call: C, enabled: E) -> Option<Server>
where
    C: Fn(&str, &Value) -> Result<Value, String> + Send + 'static,
    E: Fn() -> bool + Send + 'static,
{
    let server = tiny_http::Server::http("127.0.0.1:0").ok()?;
    let port = server.server_addr().to_ip().map(|a| a.port())?;
    let token = token();
    let expected = format!("Bearer {token}");
    std::thread::spawn(move || {
        for mut req in server.incoming_requests() {
            let authorized = req.headers().iter().any(|h| h.field.equiv("Authorization") && h.value.as_str() == expected);
            let reply = |status: u16, body: Value| tiny_http::Response::from_string(body.to_string()).with_status_code(status)
                .with_header("Content-Type: application/json".parse::<tiny_http::Header>().unwrap());
            // Browsers must never reach this API: reject anything with an Origin header.
            let from_browser = req.headers().iter().any(|h| h.field.equiv("Origin"));
            if from_browser || !authorized {
                let _ = req.respond(reply(401, json!({ "ok": false, "error": "unauthorized" })));
                continue;
            }
            if !enabled() {
                let _ = req.respond(reply(403, json!({ "ok": false, "error": "Agent access is off: Wallgates → Settings → Agent access" })));
                continue;
            }
            let mut body = String::new();
            let _ = req.as_reader().take(4 * 1024 * 1024).read_to_string(&mut body);
            let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            let name = v.get("name").and_then(Value::as_str).unwrap_or("");
            let args = v.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let out = match call(name, &args) {
                Ok(result) => json!({ "ok": true, "result": result }),
                Err(e) => json!({ "ok": false, "error": e }),
            };
            let _ = req.respond(reply(200, out));
        }
    });
    Some(Server { port, token })
}

/// Publish (or withdraw) the API address for the MCP server.
pub fn publish(dir: &Path, server: &Server, on: bool) {
    let file = dir.join("agent.json");
    if on {
        let _ = std::fs::write(&file, json!({ "port": server.port, "token": server.token, "pid": std::process::id() }).to_string());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600));
        }
    } else {
        let _ = std::fs::remove_file(&file);
    }
}

// ------------------------------------------------------------------ MCP server (`Wallgates --mcp`)

fn forward(name: &str, args: &Value) -> Result<Value, String> {
    let info: Value = std::fs::read(data_dir().join("agent.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or("Wallgates is not reachable. Start Wallgates and turn on Settings → Agent access.")?;
    let port = info["port"].as_u64().unwrap_or(0);
    let token = info["token"].as_str().unwrap_or("");
    let res = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(30))
        .build()
        .post(&format!("http://127.0.0.1:{port}/tool"))
        .set("Authorization", &format!("Bearer {token}"))
        .send_json(json!({ "name": name, "arguments": args }));
    let v: Value = match res {
        Ok(r) => r.into_json().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(_, r)) => r.into_json().unwrap_or_else(|_| json!({ "ok": false, "error": "request refused" })),
        Err(_) => return Err("Wallgates is not running (or Agent access was turned off). Start it and enable Settings → Agent access.".into()),
    };
    if v["ok"].as_bool() == Some(true) {
        Ok(v["result"].clone())
    } else {
        Err(v["error"].as_str().unwrap_or("error").to_string())
    }
}

/// Handle one JSON-RPC message; returns the response (None for notifications).
pub fn handle_mcp(msg: &Value, call: &dyn Fn(&str, &Value) -> Result<Value, String>) -> Option<Value> {
    let id = msg.get("id").cloned()?; // notifications have no id
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let ok = |result: Value| Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    match method {
        "initialize" => ok(json!({
            "protocolVersion": msg.pointer("/params/protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "wallgates", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Wallgates is a live wallpaper engine. Use list_wallpapers / get_wallpaper to discover wallpapers and their options, set_wallpaper to show one on a display, set_options to customise it. To change a built-in wallpaper's code, duplicate_wallpaper first, then read_file / write_file on the copy and check status for errors. Read authoring_guide before writing wallpaper code."
        })),
        "ping" => ok(json!({})),
        "tools/list" => ok(json!({ "tools": tools() })),
        "tools/call" => {
            let name = msg.pointer("/params/name").and_then(Value::as_str).unwrap_or("");
            let args = msg.pointer("/params/arguments").cloned().unwrap_or_else(|| json!({}));
            let result = if name == "authoring_guide" { Ok(json!(GUIDE)) } else { call(name, &args) };
            let (text, is_error) = match result {
                Ok(Value::String(s)) => (s, false),
                Ok(v) => (serde_json::to_string_pretty(&v).unwrap_or_default(), false),
                Err(e) => (e, true),
            };
            ok(json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }))
        }
        _ => Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("method not found: {method}") } })),
    }
}

pub fn run_mcp() {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle_mcp(&msg, &forward),
            Err(_) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "parse error" } })),
        };
        if let Some(r) = reply {
            let _ = writeln!(out, "{r}");
            let _ = out.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_handshake_and_tools() {
        let init = handle_mcp(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26" } }), &|_, _| Ok(json!(null))).unwrap();
        assert_eq!(init["result"]["protocolVersion"], json!("2025-03-26"));
        assert!(handle_mcp(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }), &|_, _| Ok(json!(null))).is_none());
        let list = handle_mcp(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }), &|_, _| Ok(json!(null))).unwrap();
        let names: Vec<&str> = list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"set_wallpaper") && names.contains(&"write_file"));
        let call = handle_mcp(&json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "status", "arguments": {} } }),
            &|n, _| if n == "status" { Ok(json!({ "paused": false })) } else { Err("no".into()) }).unwrap();
        assert_eq!(call["result"]["isError"], json!(false));
        assert!(call["result"]["content"][0]["text"].as_str().unwrap().contains("paused"));
        let err = handle_mcp(&json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "x" } }), &|_, _| Err("boom".into())).unwrap();
        assert_eq!(err["result"]["isError"], json!(true));
    }

    #[test]
    fn local_api_requires_token() {
        let s = serve(|n, _| Ok(json!(n)), || true).unwrap();
        let url = format!("http://127.0.0.1:{}/tool", s.port);
        let agent = ureq::AgentBuilder::new().build();
        assert!(matches!(agent.post(&url).send_json(json!({ "name": "status" })), Err(ureq::Error::Status(401, _))));
        let ok: Value = agent.post(&url).set("Authorization", &format!("Bearer {}", s.token)).send_json(json!({ "name": "status" })).unwrap().into_json().unwrap();
        assert_eq!(ok, json!({ "ok": true, "result": "status" }));
        let browser = agent.post(&url).set("Authorization", &format!("Bearer {}", s.token)).set("Origin", "http://evil.example").send_json(json!({ "name": "status" }));
        assert!(matches!(browser, Err(ureq::Error::Status(401, _))));
    }
}
