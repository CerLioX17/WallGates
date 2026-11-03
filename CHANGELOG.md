# Changelog

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
