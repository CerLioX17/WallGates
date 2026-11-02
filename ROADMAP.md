# Roadmap

## v0.2: daily-driver quality
- [ ] Start with Windows (autostart toggle)
- [ ] Recover automatically after an Explorer restart or monitor change (listen for `TaskbarCreated` / display changes)
- [ ] A different wallpaper per monitor
- [ ] Playlists: rotate wallpapers on a timer or by time of day
- [ ] Per-wallpaper volume with a global mute
- [ ] Render-scale option (e.g. 50%) for heavy shaders on 4K screens

## v0.3: interactive wallpapers
- [ ] Mouse position for shaders (`iMouse`) and HTML (low-level mouse hook on Windows)
- [ ] Audio-reactive wallpapers (system audio spectrum via WASAPI loopback)
- [ ] System data for HTML wallpapers: CPU/RAM, weather, media now-playing (opt-in permissions)
- [ ] Shader textures / multi-pass buffers

## v0.4: platforms
- [ ] Linux auto-pause (X11 `_NET_WM_STATE_FULLSCREEN`)
- [ ] Wayland via wlr-layer-shell (KDE, Hyprland, Sway)
- [ ] macOS: window at desktop level, behind icons

## Ecosystem
- [ ] Online gallery to browse and install community wallpapers
- [ ] Wallpaper editor: live-edit shaders and HTML with instant preview
- [ ] Signed releases and auto-update
