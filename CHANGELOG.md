# Changelog

## 0.10.3
**Trade Republic Portfolio: fixes impossible week / month / YTD / 1-year amounts.**
- Price history from a listing in another currency (e.g. a London line quoted in **pence**) was read in the live
  venue's currency, so the portfolio's past value could be counted ~100× too high and periods showed thousands of
  euros "lost". The history now carries its own currency, and any series whose latest close doesn't match the live
  price is rescaled onto it.
- If Lang & Schwarz has no quote for an instrument, the most recently traded venue is used instead of a stale one.

## 0.10.2
**Trade Republic Portfolio: daily changes from Trade Republic's own venue.**
- Live prices now come from the **Lang & Schwarz Exchange** (through onvista), the venue Trade Republic trades on,
  and "today" is measured against LS's own previous close — the same reference the Trade Republic app uses.
  Previously Xetra prices (closed after 17:30, 15 min delay) were compared with a Yahoo "previous close" field that
  is unreliable on some listings, which produced wrong signs on US-heavy ETFs in the evening.
- When Yahoo is used, the daily reference is the last completed daily close from the price history.
- New **Live prices** option (Lang & Schwarz · Tradegate · gettex · Yahoo) and a **Per-instrument source** list
  (`ISIN venue` or `ISIN YAHOO.SYMBOL`) to choose the venue of each holding yourself — no reinstall needed.
- The status bar shows how many holdings use each source; each holding shows its own source.

## 0.10.1
**Trade Republic Portfolio: correct daily changes, bonds and private markets.**
- Daily % now matches what a euro investor sees: instruments resolve to **EUR listings** (Xetra, Frankfurt,
  Stuttgart, Munich, … then Euronext) instead of a USD or GBP line. Previously e.g. Take-Two used the US listing,
  so "today" was the US session move in dollars.
- Daily changes are **FX-aware** (yesterday's price at yesterday's rate) when only a foreign listing exists.
- "Today" only counts the current session: before a market opens or at weekends a position shows 0, not
  yesterday's move.
- Symbol cache rebuilt automatically (old cached USD listings are dropped).
- **Bonds (reddito fisso)**: priced in % of nominal (shown as e.g. 99.10%), coupons counted as income,
  repayments at maturity handled as sales, "Bonds" in the allocation.
- **Private markets (ELTIF)**: valued at the latest NAV, excluded from the daily move, shown as
  "Private markets" in the allocation.

## 0.10.0
**New default wallpaper: Violet Flow.** A liquid violet marble photograph brought to life: the waves flow along
their own lines (WebGL2 flow map), glowing contours drift across the surface, it follows the cursor and pulses with
music. Speed, flow, waves, line glow and colour, hue, saturation, brightness, vignette, zoom, 30/60 fps and render
resolution are all adjustable. New installs start with it.

**Different wallpaper per display, made obvious.** The library now has a display picker (Display 1 · Display 2 ·
All) that starts on your primary display instead of "All displays", every card shows which display it is on,
and each click confirms where it was applied. If span mode is on, the library says so and offers a one-click switch
back to per-display wallpapers.

**Agent friendly.**
- Built-in **MCP server**: `Wallgates --mcp`. Turn on Settings → *Agent access (MCP)*, add the shown command to
  Claude Code / Cursor / Codex, and your agent can list wallpapers, set one per display, change options, duplicate
  a built-in and edit its code live, pause, and read the authoring guide. Local only, token-protected, off by default.
- `AGENTS.md` / `CLAUDE.md` guide for coding agents, `.mcp.json`, and `tools/preview/render.js`: a headless
  renderer that shows any wallpaper with live mock data and reports its errors.

## 0.9.1
**Trade Republic Portfolio 1.1**
- New **Compact layout** (Customize → Layout): the same figures in larger type for 24" 1080p screens. Holdings
  scroll automatically when the list is longer than the panel, a ticker tape shows each position's move
  today, and Recent activity is removed.
- More accurate values: instruments Yahoo Finance doesn't list (turbos, warrants, certificates) are now
  priced through onvista instead of at the last trade price.
- **Manual prices** (one per line: `ISIN price`) for any instrument without a public quote.
- Every holding shows its price source (listing and exchange); a missing FX rate is reported in the
  status bar instead of being silently treated as 1:1.
- Chart labels scale with the text size.

## 0.9.0
**New: Trade Republic Portfolio.**
- Load the CSV export from the Trade Republic app (Customize → Trade Republic export → Choose file…).
  The file is read on your PC; only ISINs are used to look up prices.
- Portfolio value, gain/loss **today, 1 week, 1 month, year to date, 1 year and all time** (net of
  deposits), value-vs-deposits chart, monthly bars and yearly returns, holdings with live price, today's
  move, gain/loss and weight, closed positions with realised P/L, allocation by asset class, dividends,
  interest, fees, taxes and recent activity. Privacy mode hides all amounts.
- Instruments without a public quote (turbos, warrants) are valued at the last trade price and marked ◆.
- Markets feed: daily price history (~2 years) and Trade Republic CSV support.

**Fonts.** Eight offline fonts bundled for every wallpaper (Share Tech Mono, JetBrains Mono, IBM Plex
Mono, Fira Code, Space Mono, VT323, Orbitron, Rajdhani). System Terminal gets a font and a title-font picker.

**System Terminal 4.2.** Missing sensors are simply not shown (no more "no sensor" or "—"). Workstation
gets a live CPU/GPU/RAM/network performance chart, more depth (accent light, technical grid, corner
brackets) and Orbitron titles.

**Settings.** New `file` option type: wallpapers can ask for a local file (e.g. a CSV export).

## 0.8.0
**System Terminal 4.1 — new Workstation style.**
- New default style: a full-screen monitoring workstation with header and status bars, three
  panels and the Share Tech Mono font (bundled, works offline).
- Left: systems status rows with load bars and NOMINAL / CAUTION / CRITICAL tags, a CPU core grid
  that lights up busy cores, storage volumes and a hardware summary.
- Center: top 5 processes, weather (UV, air quality, wind, sunrise/sunset), a 12-hour and 4-day
  forecast, and a live event log built from real events (temperature and load changes, heavy disk
  and network activity, top-process changes, weather updates, now playing, periodic health reports).
- Right: statistics, fans, real alerts with your thresholds, and a small audio analyser with
  bass/treble levels.
- Wallgates boot sequence that detects your real hardware.
- Fully customizable: colour presets (Wallgates violet/cyan, amber, green, ice cyan, mono white)
  or your own colours and background gradient, size, text size, glow, optional scanlines, boot
  sequence, every panel on/off, health-report interval and log length.
- The previous look is still available: Style → Classic terminal.

## 0.7.0
**System Terminal 4.0 — a bigger, more detailed terminal monitor.**
- Now a two-column, btop-style panel layout using up to 70% of the screen width (default 66%,
  centered), with larger text that scales to fit 16:9, 16:10 and 21:9 screens.
- CPU: load, temperature, frequency, power, process count, load history and per-core load bars.
- Memory: RAM/swap meters, RAM temperature (or "no sensor"), used/free/total, top memory user.
- Storage: every SSD with model, temperature, activity and read/write speed, plus volume usage.
- GPU: load, temperature, VRAM, hotspot, fan, power and history. Top-5 processes with CPU bars.
- Network rates with history, all fans, and weather: 12-hour temperature/rain strip, UV, air
  quality, 4-day forecast, sunrise/sunset and moon phase.
- New options: per-core CPU load and all drives & volumes. Themes: Terminal grey, PowerShell,
  Wallgates, green, amber.

## 0.6.0
**System Terminal 3.0 — a real command prompt.**
- Redesigned as a plain console window (Command Prompt, PowerShell, Wallgates, green or amber
  palettes) that sits where you want it — right, left, center or a corner — with adjustable width,
  opacity and an optional title bar. No gauges, graphs, noise, glow or background effects.
- Everything is text: htop-style meters `[||||    48.0%]`, temperatures, a top-5 process table,
  network, fans, weather with UV / air quality, a 4-day forecast table, sun and moon, now playing and
  optional `▁▃▅▇` load history. Types `wallgates status --live` on start.
- Text automatically scales so the window always fits the screen (16:9, 16:10, 21:9).

**Sensors**
- RAM temperature detection now also recognises DIMM / SPD-hub sensors reported under other hardware
  names. When a memory kit has no temperature sensor, the terminal says so instead of a blank value.

## 0.5.0
**Advanced sensors, built in.** No second app to install: Wallgates → Settings → *Advanced sensors* →
*Enable* adds CPU and RAM temperatures, CPU power, GPU hotspot and every fan to wallpapers like System
Terminal.
- Powered by LibreHardwareMonitor (MPL-2.0), shipped inside the Wallgates installer as a hidden
  component, and the Microsoft-signed PawnIO driver (downloaded from its official release and
  signature-checked only when you enable it).
- One administrator prompt to enable; afterwards it starts with Wallgates without prompts and stops
  30 seconds after Wallgates closes.
- *Disable and remove*, or uninstalling Wallgates, removes the component and PawnIO (only if Wallgates
  installed it). Updates keep it enabled.
- HWiNFO is still supported as an alternative source.
- The Windows download is now the setup `.exe` only (the MSI could not clean up advanced sensors).

## 0.4.0
**System Terminal 2.0**: a complete redesign with much more data.
- Four hardware cards (**CPU, GPU, RAM, SSD**), each with load and temperature gauges, a combined
  load/temperature history graph and details (frequency, power, threads, VRAM, fan, available memory,
  swap, top memory user, read/write speed, disk space).
- **Task manager**: the 5 heaviest processes with load bars.
- **Clock** with seconds, date, week number, day and year progress.
- **Weather**: current conditions, feels-like, humidity, wind, pressure, rain chance, **UV index** with
  colour scale, **air quality** (European AQI, PM2.5), **today + next 3 days** forecast, a 24-hour
  temperature/rain chart, sunrise, sunset, daylight left and **moon phase**.
- **Network** graph with peak, **fans** (via HWiNFO), **now playing**, audio spectrum and a live log
  with temperature and load alerts.
- Scales to 16:9, 16:10 and ultrawide 21:9.

**Engine**
- SSD model, **temperature** and activity on Windows without admin rights (storage query + disk counters).
- **RAM temperature** (DDR5 / DIMM sensors) and fan speeds from HWiNFO shared memory.
- Weather now includes UV index, 4-day forecast, 24-hour forecast and air quality (Open-Meteo).

## 0.3.0
**Two new terminal wallpapers, fully customizable**
- **Market Terminal**: your watchlist and portfolio on the desktop. Paste ISINs or tickers, choose the
  exchange (Milan, Xetra, NYSE/Nasdaq, Euronext, London, …) and your currency. It shows prices, daily
  change, day range, intraday charts, a scrolling index tape, world market clocks, portfolio value,
  today's change, total P/L with currency conversion, allocation and top movers. Includes a privacy mode,
  5 themes and CRT effects.
- **System Terminal**: CPU load per core, frequency, temperature and power; GPU load, VRAM, temperature
  and fan; memory, swap, disks, network graphs; a task-manager style top-processes list; a live system log
  with temperature and load alerts. Boots with the Wallgates gate in ASCII. 4 themes and CRT effects
  (scanlines, noise, glow, chromatic aberration, vignette, flicker).

**Engine**
- New `markets` capability: ISIN → listing resolution with your preferred exchange (cached), quotes,
  intraday series and FX rates (Yahoo Finance, delayed, can be switched off).
- `system` data now includes GPU usage, VRAM, temperature and fan on Windows (driver counters, no admin),
  HWiNFO shared-memory sensors when available, per-core load, disks, network rates and top processes.
- New property types `textarea`, `group` and `note` keep wallpapers with many options easy to manage.
  Wallpaper Engine imports map their groups and notes too.


## 0.2.0
A complete rewrite of the engine and the app, with a new brand.

**Engine**
- A different wallpaper per display, or one spanning all displays.
- Displays identified by hardware ID: assignments survive sleep and reconnects.
- Watchdog re-attaches after Explorer restarts, monitor changes and resume. No extra WorkerW windows left behind.
- Smarter auto-pause: any window covering a display (borderless games included), maximized windows,
  battery / battery saver, per-app rules. Actions: Pause, Eco, Unload (frees GPU memory), Mute.
- Frame cap, render quality (25–100%), master volume, mute unless the desktop is in front.
- Playlists: interval, shuffle, time of day, sunrise/sunset. Crossfade transitions.

**Interactive wallpapers**
- System audio spectrum (WASAPI loopback + FFT), cursor position, now playing with cover art,
  CPU/RAM, weather (Open-Meteo, no API key).
- Capability permissions shown to users; HTML wallpapers are sandboxed and **offline by default**.
- Creator SDK (`/__wallgates/wallgates-sdk.js`).

**Create & share**
- Live code editor with templates; compile errors shown inline while the desktop keeps the last good version.
- `.wallgates` packages (import/export, file association, drag & drop).
- Curated online gallery.
- Wallpaper Engine **video** and **web** import (single project or the whole workshop folder),
  with a compatibility layer for web wallpapers.

**App**
- New Wallgates brand: logo, custom title bar, onboarding, branded installers (NSIS + MSI).
- English and Italian; global shortcuts; tray actions (pause, next, mute, re-attach); autostart without services.
- Lock screen and Windows background sync (optional).
- New wallpapers: Gatescape, Pulse, Dashboard, Liquid; gallery: Synthwave, Hyperspace, Ocean Night.

**Distribution**
- Public GitHub Releases with plain installer files and SHA-256 checksums.
- Full publisher/version metadata, per-user install without admin rights.

## 0.1.0
First release: shader, HTML, video and image wallpapers behind the desktop icons on Windows.
