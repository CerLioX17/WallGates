# Wallpaper format

A Wallgates wallpaper is a folder with a `wallpaper.json` manifest plus its files.
Create one from **Create** in the app (with a live editor), drop a folder into the app window,
or share it as a `.wallgates` file (a zip of the folder: double-click to install).

```
my-wallpaper/
├── wallpaper.json
├── index.html        # or a .frag shader, an image, a video
└── preview.jpg       # optional, 16:9, ~640x360
```

## Manifest

```json
{
  "name": "Neon Rain",
  "author": "you",
  "description": "Shown in the library and details panel.",
  "type": "html",
  "entry": "index.html",
  "preview": "preview.jpg",
  "license": "CC-BY-4.0",
  "version": "1.0.0",
  "tags": ["rain", "neon"],
  "capabilities": ["audio"],
  "properties": {
    "speed": { "label": "Speed", "type": "slider", "min": 0, "max": 3, "step": 0.1, "default": 1 }
  }
}
```

| Field          | Required | Notes |
| -------------- | -------- | ----- |
| `name`         | yes      | |
| `type`         | yes      | `image`, `video`, `html` or `shader` |
| `entry`        | yes      | Relative path inside the folder |
| `author`, `description`, `version`, `tags` | no | Shown in the library and used by search |
| `preview`      | no       | Thumbnail. For `html` wallpapers it is also used for lock screen sync |
| `license`      | no       | SPDX id, e.g. `CC-BY-4.0`, `MIT` |
| `capabilities` | no       | Data the wallpaper needs. Shown to users as permissions (see below) |
| `ai_generated` | no       | `true` if made with generative AI (shown as a label) |
| `rating`       | no       | `everyone` (default) or `mature` (hidden unless the user opts in) |
| `properties`   | no       | User-editable options, in the order declared |

## Properties

Every property has a `label`, a `type` and a `default`.

| Type     | Extra fields                                 | Value             |
| -------- | -------------------------------------------- | ----------------- |
| `slider` | `min`, `max`, `step`                         | number            |
| `color`  |                                              | `"#rrggbb"`       |
| `bool`   |                                              | `true` / `false`  |
| `select` | `options`: `["a","b"]` or `[{value,label}]`  | the option value  |
| `text`   | `placeholder`                                | string            |
| `textarea` | `rows`, `placeholder`                      | multi-line string |
| `group`  | (no `default`)                               | section heading in the panel |
| `note`   | (no `default`)                               | short explanation shown in the panel |

Choices are saved per wallpaper and survive restarts and updates. Use `group` and `note`
to keep wallpapers with many options easy to configure.

## Capabilities

| Capability | Gives the wallpaper | Notes |
| ---------- | ------------------- | ----- |
| `audio`    | System audio spectrum: 64 bands + level, ~30/s | What the PC plays, never the microphone (Windows) |
| `mouse`    | Cursor position over the display | Position only: no clicks or keys are read |
| `system`   | CPU (total, per core, frequency, temperature), GPU (usage, VRAM, temperature), memory, swap, disks, network rates, top processes, uptime, host/OS | Every 2 s. See *System data* below |
| `media`    | Now playing: title, artist, album, playing, cover art | Any app using Windows media controls |
| `weather`  | Current weather + today's high/low, sunrise/sunset | Only if the user enabled weather and set a city |
| `markets`  | Quotes, intraday series, FX rates for the instruments in your properties | See *Market data* below |
| `network`  | Internet access for `html` wallpapers | **Without it, web wallpapers are fully offline** |

Feeds run only while a wallpaper that asked for them is visible and not paused.

## System data

`Wallgates.on('system', s => ...)` receives:
`cpu`, `cpuName`, `cores[]`, `cpuFreq` (MHz), `cpuTemp` (°C or null), `cpuPower`, `memory` (%),
`memoryUsed`, `memoryTotal`, `swapUsed`, `swapTotal`,
`gpu: { name, usage, vramUsed, vramTotal, temp, fan, power }` (or null),
`disks[]: { name, mount, used, total }`, `net: { rx, tx }` (bytes/s),
`processes[]: { name, cpu, memory, count }` (top 10 by CPU, grouped by name), `memoryHogs[]`,
`processCount`, `uptime`, `host`, `os`, `sensors[]: { label, unit, value, type }`, and
`sources.temperatures` (`hwinfo`, `gpu-driver`, `system` or null).

On Windows, GPU data comes from the driver (as in Task Manager). CPU temperature and every other
sensor become available when [HWiNFO](https://www.hwinfo.com) runs with *Shared Memory Support* enabled.
Wallgates only reads that shared memory, no driver is installed.

## Market data

Wallpapers with the `markets` capability declare their instruments in these property keys:

| Key | Meaning |
| --- | --- |
| `watchlist`, `indices` | ISINs or tickers separated by spaces, commas or new lines. `#` starts a comment |
| `portfolio` | one holding per line: `ISIN_or_TICKER QUANTITY AVERAGE_PRICE` |
| `market` | preferred exchange for ISINs: `auto`, `milan`, `xetra`, `us`, `paris`, `amsterdam`, `london`, `madrid`, `swiss`, … |
| `currency` | base currency for FX rates (e.g. `EUR`) |
| `refresh` | seconds between updates (min 15) |

`Wallgates.on('markets', d => ...)` receives `quotes[]` (`id`, `symbol`, `name`, `exchange`, `currency`,
`price`, `change`, `changePct`, `previousClose`, `dayHigh`, `dayLow`, `state`, `spark[]`, …),
`fx` (e.g. `{ "USDEUR": 0.92 }`), `errors[]` and `updated`. Tickers use Yahoo Finance notation
(`AAPL`, `ENI.MI`, `^GSPC`, `EURUSD=X`, `BTC-EUR`). Data is free but delayed on many exchanges and is
for information only. The user can switch it off in Settings → Privacy.

## Type `image` / `video`

Built-in properties (all optional): `fit` (`cover` / `contain` / `fill`), and for video
`speed` (playback rate) and `volume` (0–1, 0 = muted). Importing a file creates these automatically.
MP4 (H.264) and WebM (VP9/AV1) work everywhere; HEVC needs the Windows HEVC extension.

## Type `html`

The page runs in a **sandboxed iframe**: scripts are allowed, but it has no access to Wallgates
internals, your files or other wallpapers, and no network unless it declares `network`.
It can load files from its own folder with relative URLs.

### With the SDK (recommended)

```html
<script src="/__wallgates/wallgates-sdk.js"></script>
<script>
  Wallgates.on('properties', (v) => applyOptions(v));     // initial + every change
  Wallgates.on('pause', (paused) => paused ? stop() : start());
  Wallgates.on('audio', ({ bands, level }) => draw(bands)); // needs "audio"
  Wallgates.on('mouse', ({ x, y, inside }) => parallax(x, y)); // 0..1, needs "mouse"
  Wallgates.on('media', (m) => showTrack(m.title, m.artist, m.thumbnail));
  Wallgates.on('system', (s) => showLoad(s.cpu, s.memory));
  Wallgates.on('weather', (w) => showWeather(w.temperature, w.condition, w.isDay));
</script>
```

`Wallgates.values`, `Wallgates.paused` and `Wallgates.renderScale` are also available.
See `wallpapers/dashboard` for a complete example.

### Raw messages

Without the SDK, listen to `message` events: `{ type: 'wallgates', event, ... }` where `event` is
`init`, `properties`, `pause`, `audio`, `mouse`, `system`, `media` or `weather`.

Please honour `paused`: it is how Wallgates saves CPU/GPU while games run.

### Wallpaper Engine web wallpapers

Imported Wallpaper Engine **web** projects keep working: Wallgates injects a compatibility layer
that implements `wallpaperPropertyListener.applyUserProperties`, `setPaused`,
`wallpaperRegisterAudioListener` and the media listeners. Scene and application wallpapers can't
be imported (proprietary format / executables).

## Type `shader`

A GLSL ES 3.0 fragment shader in Shadertoy style. Implement `mainImage`:

```glsl
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / iResolution.xy;
  float pulse = 0.6 + 0.4 * iAudioLevel;
  fragColor = vec4(uv * pulse, 0.5 + 0.5 * sin(iTime * u_speed), 1.0);
}
```

Uniforms provided by the host (**don't** declare them yourself):

| Uniform        | Type          | |
| -------------- | ------------- | - |
| `iResolution`  | `vec3`        | pixels (affected by the user's render quality) |
| `iTime`        | `float`       | seconds, stops while paused |
| `iTimeDelta`   | `float`       | seconds since last frame |
| `iFrame`       | `int`         | |
| `iMouse`       | `vec4`        | cursor in pixels (`xy`), needs `mouse` |
| `iAudio`       | `float[64]`   | spectrum 0..1, low → high frequencies, needs `audio` |
| `iAudioLevel`  | `float`       | overall loudness 0..1, needs `audio` |
| `u_<property>` | varies        | one per property: `slider`/`bool`/`select` → `float` (select = option index), `color` → `vec3` |

Compile errors appear in the editor and on the desktop, and the last working version keeps running.
Respect the licence of any shader you adapt (many Shadertoy shaders are CC BY-NC-SA).

## Publishing to the gallery

Open a pull request adding your folder to `gallery/` and run `python3 scripts/build_gallery.py`.
Gallery items must be original or properly licensed, include a preview, and must not request `network`
unless essential.
