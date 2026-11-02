# Wallpaper format

A Wallgates wallpaper is a folder with a `wallpaper.json` manifest plus its files.
Drop the folder into your library (**Open folder** in the app) or use **Import folder**.

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
  "description": "Shown as a tooltip and in the details panel.",
  "type": "html",
  "entry": "index.html",
  "preview": "preview.jpg",
  "properties": {
    "speed": { "label": "Speed", "type": "slider", "min": 0, "max": 3, "step": 0.1, "default": 1 }
  }
}
```

| Field         | Required | Notes                                                        |
| ------------- | -------- | ------------------------------------------------------------ |
| `name`        | yes      |                                                              |
| `type`        | yes      | `image`, `video`, `html` or `shader`                         |
| `entry`       | yes      | Relative path inside the folder                              |
| `author`      | no       |                                                              |
| `description` | no       |                                                              |
| `preview`     | no       | Thumbnail for the library                                    |
| `properties`  | no       | User-editable options, shown in the order they are declared |

## Properties

Every property has a `label`, a `type` and a `default`.

| Type     | Extra fields                                   | Value sent        |
| -------- | ---------------------------------------------- | ----------------- |
| `slider` | `min`, `max`, `step`                           | number            |
| `color`  |                                                | `"#rrggbb"`       |
| `bool`   |                                                | `true` / `false`  |
| `select` | `options`: `["a","b"]` or `[{value,label}]`    | the option value  |
| `text`   |                                                | string            |

The user's choices are saved per wallpaper and survive restarts.

## Type `image` / `video`

Supported built-in properties (all optional): `fit` (`cover` / `contain` / `fill`),
and for video `speed` (playback rate) and `volume` (0–1, 0 = muted).
Importing a file from the app creates this manifest automatically.

Video formats: MP4 (H.264) and WebM (VP9/AV1) work everywhere.

## Type `html`

The page runs in a **sandboxed iframe**: scripts are allowed, but it has no
access to Wallgates internals, the file system or other wallpapers. It may load
files from its own folder with relative URLs.

Wallgates talks to the page with `postMessage`:

```js
window.addEventListener('message', (e) => {
  const m = e.data;
  if (!m || m.type !== 'wallgates') return;
  // m.event: 'init' | 'properties' | 'pause'
  // m.values: { [propertyKey]: value }   — always the full set
  // m.paused: boolean                     — stop animating when true
  applyOptions(m.values);
  m.paused ? stopAnimation() : startAnimation();
});
```

Please honour `paused`: it is how Wallgates saves CPU/GPU while games or
fullscreen apps run. See `wallpapers/particles` and `wallpapers/clock`.

## Type `shader`

A GLSL ES 3.0 fragment shader in Shadertoy style. Implement `mainImage`:

```glsl
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / iResolution.xy;
  fragColor = vec4(uv * u_brightness, 0.5 + 0.5 * sin(iTime * u_speed), 1.0);
}
```

Uniforms provided by the host (do **not** declare them yourself):

| Uniform        | Type    |                                               |
| -------------- | ------- | --------------------------------------------- |
| `iResolution`  | `vec3`  | pixels                                        |
| `iTime`        | `float` | seconds, stops while paused                   |
| `iTimeDelta`   | `float` | seconds since last frame                      |
| `iFrame`       | `int`   |                                               |
| `iMouse`       | `vec4`  | reserved, currently zero                      |
| `u_<property>` | varies  | one per property: `slider`/`bool`/`select` → `float` (select = option index), `color` → `vec3` (0–1) |

Compile errors are shown on the desktop so you can iterate quickly.
Respect the licence of any shader you adapt (many Shadertoy shaders are CC BY-NC-SA).
