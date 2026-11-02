// Wallgates wallpaper host: runs in each desktop window and renders the
// assigned wallpaper (image / video / html / shader).
//
// Third-party HTML runs in a sandboxed iframe (no IPC, opaque origin, offline
// unless it declares "network"); it receives properties and live data via
// postMessage. Live data is only forwarded for capabilities the wallpaper declares.
'use strict';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const WP_BASE = /Windows/.test(navigator.userAgent) ? 'http://wp.localhost/' : 'wp://localhost/';
const stage = document.getElementById('stage');
const errorBox = document.getElementById('error');

function fileUrl(wallpaper, rel) {
  const parts = [...wallpaper.id.split('/'), ...rel.split('/')];
  return WP_BASE + parts.map(encodeURIComponent).join('/');
}

function showError(message) {
  errorBox.textContent = message;
  errorBox.style.display = message ? 'block' : 'none';
}

const FITS = new Set(['cover', 'contain', 'fill']);
const fitOf = (values) => (FITS.has(values.fit) ? values.fit : 'cover');
const clamp01 = (v) => Math.min(1, Math.max(0, Number(v) || 0));

// Live data shared by all renderers.
const live = {
  audio: new Float32Array(64),
  level: 0,
  mouse: { x: 0.5, y: 0.5, inside: false, desktop: false },
};

// ------------------------------------------------------------------ renderers
// Each renderer: { update(state), setPaused(bool), capture()?, deliver(event, data)?, destroy() }

function imageRenderer(layer, w, state) {
  const img = document.createElement('img');
  img.crossOrigin = 'anonymous';
  img.src = fileUrl(w, w.manifest.entry);
  img.alt = '';
  layer.append(img);
  const r = {
    ready: new Promise((res) => { img.onload = res; img.onerror = res; }),
    update(s) { img.style.objectFit = fitOf(s.values); },
    setPaused() {},
    destroy() {},
  };
  r.update(state);
  return r;
}

function videoRenderer(layer, w, state) {
  const video = document.createElement('video');
  video.crossOrigin = 'anonymous';
  Object.assign(video, { loop: true, muted: true, autoplay: true, playsInline: true, preload: 'auto' });
  video.src = fileUrl(w, w.manifest.entry);
  layer.append(video);
  let paused = false;

  const play = () => {
    if (paused) return;
    video.play().catch(() => {
      // Autoplay with sound can be blocked: fall back to muted playback.
      video.muted = true;
      video.play().catch(() => {});
    });
  };

  const r = {
    ready: new Promise((res) => {
      video.addEventListener('loadeddata', res, { once: true });
      video.addEventListener('error', res, { once: true });
      setTimeout(res, 4000);
    }),
    update(s) {
      video.style.objectFit = fitOf(s.values);
      const speed = Number(s.values.speed);
      video.playbackRate = speed > 0 ? speed : 1;
      const volume = clamp01(s.values.volume) * clamp01(s.volume);
      video.volume = volume;
      video.muted = s.muted || volume === 0;
      play();
    },
    setPaused(p) { paused = p; p ? video.pause() : play(); },
    capture() {
      const c = document.createElement('canvas');
      c.width = video.videoWidth || innerWidth;
      c.height = video.videoHeight || innerHeight;
      c.getContext('2d').drawImage(video, 0, 0, c.width, c.height);
      return c.toDataURL('image/png');
    },
    destroy() { video.pause(); video.removeAttribute('src'); video.load(); },
  };
  r.update(state);
  return r;
}

function htmlRenderer(layer, w, state) {
  const frame = document.createElement('iframe');
  // allow-scripts without allow-same-origin: opaque origin, no IPC, no storage of ours.
  frame.setAttribute('sandbox', 'allow-scripts');
  frame.setAttribute('allow', 'autoplay');
  frame.src = fileUrl(w, w.manifest.entry);
  layer.append(frame);

  let current = state;
  const caps = new Set(w.manifest.capabilities || []);
  const base = () => ({
    type: 'wallgates',
    values: current.values,
    paused: current.paused,
    renderScale: current.render_scale,
    fps: current.fps_limit || 60,
    muted: current.muted,
    volume: current.volume,
    monitor: { index: current.index, width: current.width, height: current.height, span: current.span },
  });
  const post = (msg) => frame.contentWindow?.postMessage(msg, '*');

  const ready = new Promise((res) => {
    frame.addEventListener('load', async () => {
      post({ ...base(), event: 'init' });
      // Replay cached feeds so dashboards are filled immediately.
      try {
        const snap = await invoke('feed_snapshot');
        for (const [kind, data] of Object.entries(snap)) if (caps.has(kind)) post({ type: 'wallgates', event: kind, ...data });
      } catch (_) { /* ignore */ }
      res();
    }, { once: true });
    setTimeout(res, 5000);
  });

  return {
    ready,
    update(s) { current = s; post({ ...base(), event: 'properties' }); },
    setPaused(p) { current = { ...current, paused: p }; post({ ...base(), event: 'pause' }); },
    deliver(event, data) { if (caps.has(event)) post({ type: 'wallgates', event, ...data }); },
    capture: async () => {
      // Cross-origin iframe pixels can't be read: use the preview image instead.
      if (!w.manifest.preview) return null;
      const img = new Image();
      img.crossOrigin = 'anonymous';
      img.src = fileUrl(w, w.manifest.preview);
      await img.decode();
      const c = document.createElement('canvas');
      c.width = img.naturalWidth; c.height = img.naturalHeight;
      c.getContext('2d').drawImage(img, 0, 0);
      return c.toDataURL('image/png');
    },
    destroy() { frame.src = 'about:blank'; },
  };
}

const VERTEX = `#version 300 es
in vec2 position;
void main() { gl_Position = vec4(position, 0.0, 1.0); }`;

function hexToRgb(hex) {
  const m = /^#?([0-9a-f]{6})$/i.exec(String(hex || ''));
  const n = m ? parseInt(m[1], 16) : 0xffffff;
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

/** GLSL uniform declarations + value converters for manifest properties. */
function shaderUniforms(properties) {
  const out = [];
  for (const [key, def] of Object.entries(properties || {})) {
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) continue;
    const name = `u_${key}`;
    switch (def.type) {
      case 'color':
        out.push({ key, name, decl: `uniform vec3 ${name};`, set: (gl, loc, v) => gl.uniform3fv(loc, hexToRgb(v)) });
        break;
      case 'bool':
        out.push({ key, name, decl: `uniform float ${name};`, set: (gl, loc, v) => gl.uniform1f(loc, v ? 1 : 0) });
        break;
      case 'select': {
        const opts = (def.options || []).map((o) => (typeof o === 'object' ? o.value : o));
        out.push({ key, name, decl: `uniform float ${name};`, set: (gl, loc, v) => gl.uniform1f(loc, Math.max(0, opts.indexOf(v))) });
        break;
      }
      case 'slider':
        out.push({ key, name, decl: `uniform float ${name};`, set: (gl, loc, v) => gl.uniform1f(loc, Number(v) || 0) });
        break;
      default:
        break; // text properties have no shader representation
    }
  }
  return out;
}

async function shaderRenderer(layer, w, state) {
  const res = await fetch(fileUrl(w, w.manifest.entry));
  if (!res.ok) throw new Error(`cannot load shader (${res.status})`);
  const source = await res.text();

  const canvas = document.createElement('canvas');
  layer.append(canvas);
  const gl = canvas.getContext('webgl2', { antialias: false, alpha: false, powerPreference: 'low-power', preserveDrawingBuffer: false });
  if (!gl) throw new Error('WebGL2 is not available');

  const uniforms = shaderUniforms(w.manifest.properties);
  const fragment = [
    '#version 300 es',
    'precision highp float;',
    'uniform vec3 iResolution;',
    'uniform float iTime;',
    'uniform float iTimeDelta;',
    'uniform int iFrame;',
    'uniform vec4 iMouse;',
    'uniform float iAudio[64];',
    'uniform float iAudioLevel;',
    ...uniforms.map((u) => u.decl),
    'out vec4 wf_FragColor;',
    '#line 1',
    source,
    'void main() { mainImage(wf_FragColor, gl_FragCoord.xy); wf_FragColor.a = 1.0; }',
  ].join('\n');

  const compile = (type, src) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) || 'shader compile error');
    return s;
  };
  const program = gl.createProgram();
  gl.attachShader(program, compile(gl.VERTEX_SHADER, VERTEX));
  gl.attachShader(program, compile(gl.FRAGMENT_SHADER, fragment));
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(program) || 'link error');
  gl.useProgram(program);

  // One oversized triangle covers the screen.
  const buffer = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  const pos = gl.getAttribLocation(program, 'position');
  gl.enableVertexAttribArray(pos);
  gl.vertexAttribPointer(pos, 2, gl.FLOAT, false, 0, 0);

  const loc = (n) => gl.getUniformLocation(program, n);
  const u = {
    res: loc('iResolution'), time: loc('iTime'), delta: loc('iTimeDelta'), frame: loc('iFrame'),
    mouse: loc('iMouse'), audio: loc('iAudio'), level: loc('iAudioLevel'),
  };
  const propLocs = uniforms.map((x) => ({ ...x, loc: loc(x.name) })).filter((x) => x.loc);

  let current = state;
  let time = 0, frame = 0, last = 0, lastDraw = 0, raf = 0, running = false;

  const applyValues = () => propLocs.forEach((x) => x.set(gl, x.loc, current.values[x.key]));

  const resize = () => {
    const scale = Math.min(1, Math.max(0.25, Number(current.render_scale) || 1));
    const dpr = (window.devicePixelRatio || 1) * scale;
    const cw = Math.max(1, Math.round(innerWidth * dpr)), ch = Math.max(1, Math.round(innerHeight * dpr));
    if (canvas.width !== cw || canvas.height !== ch) {
      canvas.width = cw; canvas.height = ch;
      gl.viewport(0, 0, cw, ch);
    }
    gl.uniform3f(u.res, canvas.width, canvas.height, 1);
  };

  const draw = (dt) => {
    resize();
    gl.uniform1f(u.time, time);
    gl.uniform1f(u.delta, dt);
    gl.uniform1i(u.frame, frame++);
    if (u.audio) gl.uniform1fv(u.audio, live.audio);
    if (u.level) gl.uniform1f(u.level, live.level);
    if (u.mouse) gl.uniform4f(u.mouse, live.mouse.x * canvas.width, (1 - live.mouse.y) * canvas.height, 0, 0);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  };

  const loop = (now) => {
    raf = requestAnimationFrame(loop);
    const dt = last ? Math.min((now - last) / 1000, 0.25) : 0;
    last = now;
    time += dt;
    const fps = current.fps_limit;
    if (fps > 0 && now - lastDraw < 1000 / fps - 2) return;
    draw(lastDraw ? Math.min((now - lastDraw) / 1000, 0.25) : dt);
    lastDraw = now;
  };

  const start = () => { if (!running) { running = true; last = 0; raf = requestAnimationFrame(loop); } };
  const stop = () => { running = false; cancelAnimationFrame(raf); };

  addEventListener('resize', resize);
  applyValues();
  draw(0);
  if (!state.paused) start();

  return {
    ready: Promise.resolve(),
    update(s) { current = s; applyValues(); draw(0); }, // instant feedback, even while paused
    setPaused(p) { p ? stop() : start(); },
    capture() { draw(0); return canvas.toDataURL('image/png'); },
    destroy() {
      stop();
      removeEventListener('resize', resize);
      gl.getExtension('WEBGL_lose_context')?.loseContext();
    },
  };
}

// ------------------------------------------------------------------ layers & transitions

let active = null; // { key, layer, renderer, wallpaper }
let pending = Promise.resolve();

function keyOf(s) {
  const w = s.wallpaper;
  return w && !s.stopped ? `${w.id}|${w.manifest.type}|${w.manifest.entry}|${s.revision}` : null;
}

async function createLayer(s) {
  const w = s.wallpaper;
  const layer = document.createElement('div');
  layer.className = 'layer';
  stage.append(layer);
  let renderer;
  switch (w.manifest.type) {
    case 'image': renderer = imageRenderer(layer, w, s); break;
    case 'video': renderer = videoRenderer(layer, w, s); break;
    case 'html': renderer = htmlRenderer(layer, w, s); break;
    case 'shader': renderer = await shaderRenderer(layer, w, s); break;
    default: throw new Error(`unknown wallpaper type: ${w.manifest.type}`);
  }
  return { layer, renderer };
}

function retire(entry, delay) {
  if (!entry) return;
  setTimeout(() => {
    try { entry.renderer?.destroy(); } catch (_) { /* ignore */ }
    entry.layer.remove();
  }, delay);
}

async function apply(s) {
  const key = keyOf(s);
  if (key !== (active?.key ?? null)) {
    const previous = active;
    active = null;
    showError('');
    if (!key) {
      // Stopped (frees GPU memory) or nothing assigned.
      retire(previous, 0);
      return;
    }
    let created;
    try {
      created = await createLayer(s);
      invoke('report_error', { id: s.wallpaper.id, message: '' }).catch(() => {});
    } catch (e) {
      const message = String(e.message || e);
      showError(`${s.wallpaper.manifest.name}: ${message}`);
      invoke('report_error', { id: s.wallpaper.id, message }).catch(() => {});
      stage.querySelectorAll('.layer:empty').forEach((l) => l.remove());
      active = previous; // keep showing the old wallpaper
      return;
    }
    active = { key, wallpaper: s.wallpaper, ...created };
    await Promise.race([created.renderer.ready, new Promise((r) => setTimeout(r, 3000))]);
    const ms = previous ? Math.max(0, s.transition_ms | 0) : 0;
    created.layer.style.transition = `opacity ${ms}ms ease`;
    requestAnimationFrame(() => { created.layer.style.opacity = '1'; });
    retire(previous, ms + 50);
  } else if (active) {
    active.renderer.update(s);
  }
  active?.renderer.setPaused(s.paused);
}

// Serialize updates so async loading never races a newer state.
const enqueue = (s) => { if (s) pending = pending.then(() => apply(s)).catch(console.error); };

listen('wf-state', (e) => enqueue(e.payload));
listen('wf-audio', (e) => {
  const { bands, level } = e.payload;
  for (let i = 0; i < 64; i++) live.audio[i] = bands[i] || 0;
  live.level = level || 0;
  active?.renderer.deliver?.('audio', { bands, level });
});
listen('wf-mouse', (e) => {
  live.mouse = e.payload;
  active?.renderer.deliver?.('mouse', e.payload);
});
listen('wf-feed', (e) => active?.renderer.deliver?.(e.payload.kind, e.payload.data));
listen('wf-capture', async () => {
  try {
    const data = await active?.renderer.capture?.();
    if (data) await invoke('submit_frame', { data });
  } catch (err) {
    console.warn('capture failed', err);
  }
});
invoke('host_state').then(enqueue);
