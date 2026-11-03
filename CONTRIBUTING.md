# Contributing

Thanks for helping make Wallgates better!

- **Wallpapers**: see [docs/wallpaper-format.md](docs/wallpaper-format.md). Gallery submissions go in `gallery/`
  (run `python3 scripts/build_gallery.py`).
- **Coding agents**: read [AGENTS.md](AGENTS.md) — layout, checks, rules. Preview wallpapers with `node tools/preview/render.js <name>`.
- **Code**: `npm install && npm run dev`. Run `cargo test` in `src-tauri/` before opening a PR.
- **Translations**: strings live in `ui/i18n.js` (UI) and `tr()` in `src-tauri/src/lib.rs` (tray).
- **Bugs**: include your Windows version (`winver`) and the log at `%APPDATA%\app.wallgates.desktop\wallgates.log`.

By contributing you agree your code is licensed under GPL-3.0-or-later.

## Releasing (maintainers)
Bump `version` in `src-tauri/tauri.conf.json` and `package.json`, add a `## x.y.z` section to
`CHANGELOG.md`, and push to `main`. CI creates the `vX.Y.Z` tag and a public release with the
installers and checksums automatically.
