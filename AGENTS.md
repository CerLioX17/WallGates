# AGENTS.md — working on Wallgates with a coding agent

This file is for coding agents (Claude Code, Cursor, Codex, Copilot, Aider, …) and for humans who use them.
It tells you how the project is laid out, how to check your work, and the rules that keep Wallgates safe.

## Two ways to work

| You want to… | Use |
| --- | --- |
| Change, customise or create **wallpapers on your own PC** while Wallgates runs | The **Wallgates MCP server** (below): no repo, no build |
| Change **Wallgates itself** (engine, UI, built-in wallpapers) | This repository: build, test, open a PR |

### Wallgates MCP server (control the running app)

1. Wallgates → **Settings → Agent access (MCP)** → turn on **Allow coding agents**.
2. Add it to your agent, e.g. Claude Code (the settings page shows the exact command with your path):
   ```bash
   claude mcp add wallgates -- "C:\path\to\Wallgates.exe" --mcp
   ```
   Other agents: command = the Wallgates executable, args = `["--mcp"]`, transport = stdio.
   From a source checkout: `cargo run --manifest-path src-tauri/Cargo.toml -- --mcp` (see `.mcp.json`).
   Run `cargo build --manifest-path src-tauri/Cargo.toml` once first: agents give an MCP server ~30 s to start.

Tools: `status`, `list_wallpapers`, `get_wallpaper`, `set_wallpaper` (per display: 1, 2, … or `all`),
`set_options`, `reset_options`, `duplicate_wallpaper`, `create_wallpaper`, `list_files`, `read_file`,
`write_file`, `check_prices`, `pause`, `next_wallpaper`, `authoring_guide`.

Typical customisation of a built-in wallpaper:
`get_wallpaper builtin/system-terminal` → `duplicate_wallpaper` → `read_file`/`write_file` on the copy →
`set_wallpaper` → `status` (shows script/shader errors). Built-ins are read-only; always edit a copy.
How it works: `src-tauri/src/agent.rs` (token-protected API on 127.0.0.1, off by default) — see *Security*.

## Repository map

```
src-tauri/src/
  lib.rs           Tauri commands (UI ↔ engine), tray, hotkeys, agent tool dispatch (agent_call)
  agent.rs         MCP stdio server (`--mcp`) + local agent API
  engine.rs        per-display state, pause/eco rules, playlists, feeds, watchdog
  config.rs        config.json schema + migrations (serde defaults: new fields need a Default)
  library.rs       wallpaper manifests, user library, editor file access (path-safe)
  protocol.rs      wp:// file server for wallpapers (offline CSP, CORS, ranges) + /__wallgates/ SDK & fonts
  package.rs       .wallgates packages, Wallpaper Engine import
  desktop/         Windows WorkerW attach, monitor ids, observation; Linux/X11; fallback
  feeds/           audio (WASAPI), media (GSMTC), system, gpu/drives (Windows), weather, markets, sensors
  sensors.rs       optional LibreHardwareMonitor helper (sensors/ is its C# source)
ui/                settings app (index.html, app.js, i18n.js EN/IT, style.css) and wallpaper host (host.html/host.js)
ui/sdk/            wallgates-sdk.js (MIT), we-shim.js, fonts.css + fonts/ (OFL) — served at /__wallgates/
wallpapers/        built-in wallpapers, one folder each (wallpaper.json + files)
gallery/           online gallery items (+ scripts/build_gallery.py → gallery/index.json)
docs/              wallpaper-format.md (THE authoring reference), distribution.md, website (index.html)
tools/preview/     headless renderer with mock live data → PNG + error report
```

## Check your work (always)

```bash
cd src-tauri && cargo test                      # unit tests (CI runs exactly this)
cargo check --target x86_64-pc-windows-msvc     # Windows-only code compiles (rustup target add …)
python3 scripts/build_gallery.py                # if you touched gallery/ (CI fails if index.json is stale)
node tools/preview/render.js <wallpaper> --out preview-out   # see a wallpaper, get JS/shader errors
```

`tools/preview/render.js` runs the real `ui/host.html` and SDK in Chromium with realistic mock data
(system, weather, media, audio, markets), so a wallpaper looks exactly as on the desktop. Options:
`--width/--height`, `--wait ms`, `--values '{"key":value}'`, `--markets file.json`, `--user` (user library),
`--dir path`. It prints `errors` (console/page errors) and `reported` (errors the host caught). Fix them all.
First run: `npm i --no-save playwright && npx playwright install chromium`.

Run the app: `npm install && npm run dev` (Windows/Linux with Tauri prerequisites).

## Making or changing a wallpaper

Read **docs/wallpaper-format.md** first (manifest, option types, capabilities, SDK events, shader uniforms).
Rules that matter:
- Every user-facing option goes in `wallpaper.json → properties` with a `label` and `default`. Use `group`/`note`
  to keep long lists readable. Option values arrive through `Wallgates.on('properties', …)`.
- HTML wallpapers run in a **sandboxed, offline iframe** (opaque origin). Load only files from the wallpaper's own
  folder or `/__wallgates/…`. Images used as WebGL textures need `img.crossOrigin = 'anonymous'`.
  Never add `network` unless essential, and never call third-party URLs from a built-in.
- Honour `Wallgates.on('pause')` (stop rAF/timers) and `Wallgates.renderScale`. Wallpapers run all day:
  keep CPU/GPU low, avoid per-frame layout, prefer transforms for animation.
- Fonts: `<link rel="stylesheet" href="/__wallgates/fonts.css">` (Share Tech Mono, JetBrains Mono, IBM Plex Mono,
  Fira Code, Space Mono, VT323, Orbitron, Rajdhani).
- Missing data (e.g. a sensor) is simply not shown — no "n/a", "no sensor" or "—" placeholders.
- Built-ins ship a 640×360 `preview.jpg` (and a 960×540 copy in `docs/assets/previews/` for the website);
  render it with `tools/preview/render.js` and resize. Never use personal data in previews.

## Conventions

- Rust: no `unwrap()` on user/OS data; errors are `Result<_, String>` shown to the user. Keep Windows-only code in
  `#[cfg(windows)]` modules. New config fields must have defaults (`#[serde(default)]` on `Config`).
- UI strings go in `ui/i18n.js` in **both** English and Italian.
- Keep changes small and focused; update `CHANGELOG.md` (top section) and docs when behaviour changes.
- Releasing (maintainers): bump the version in `src-tauri/tauri.conf.json`, `package.json`
  (+ `npm install --package-lock-only`) and `src-tauri/Cargo.toml`, add a `## x.y.z` CHANGELOG section, push to `main`.
  CI tags and publishes the release.

## Security & privacy — non-negotiable

- **No telemetry.** Network access only for features listed under *Privacy* in the README, each switchable off.
- **Never commit personal data**: CSV exports, portfolios, IBANs, names, hostnames, API keys, `config.json`.
  Wallpapers that read personal files (e.g. Trade Republic Portfolio) use a `file` option: the content stays in the
  local config and only identifiers (ISINs) leave the PC.
- Wallgates never runs executables from wallpapers. Keep the wp:// sandbox, CSP and path checks intact
  (`library::is_safe_relative`, `protocol.rs`).
- The agent API listens on 127.0.0.1 only, needs the per-session token from `agent.json` (user-only file),
  rejects requests carrying an `Origin` header (browsers), and is off unless the user enables it.
  Built-in wallpapers are read-only through it. Don't weaken any of this.
