# CLAUDE.md

@AGENTS.md

Claude Code specifics:
- `.mcp.json` registers the Wallgates MCP server from this checkout (`cargo run -- --mcp`). It talks to a running
  Wallgates with **Settings → Agent access** on. Use it to apply and inspect wallpapers you are editing.
- Before finishing a wallpaper change, render it with `node tools/preview/render.js <name>` and look at the PNG.
