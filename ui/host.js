// Wallpaper host: runs in each per-monitor desktop window and renders the
// active wallpaper (image / video / html / shader). Third-party HTML runs in a
// sandboxed iframe without access to Tauri IPC; it receives properties via postMessage.
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

// ------------------------------------------------------------------ renderers
// Each renderer: { update(values), setPaused(bool), setFps(n), destroy() }

function imageRenderer(wallpaper, values) {
  const img = document.createElement('img');
  img.src = fileUrl(wallpaper, wallpaper.manifest.entry);
  img.alt = '';
  stage.append(img);
  const r = {
    update(v) { img.style.objectFit = fitOf(v); },
    setPaused() {},
    setFps() {},
    destroy() { img.remove(); },
  };
  r.update(values);
  return r;
}

function videoRenderer(wallpaper, values) {
  const video = document.createElement('video');
  Object.assign(video, { loop: true, muted: true, autoplay: true, playsInline: true });
  video.src = fileUrl(wallpaper, wallpaper.manifest.entry);
  stage.append(video);
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
    update(v) {
      video.style.objectFit = fitOf(v);
      const speed = Number(v.speed);
      video.playbackRate = speed > 0 ? speed : 1;
      const volume = Math.min(1, Math.max(0, Number(v.volume) || 0));
      video.volume = volume;
      video.muted = volume === 0;
      play();
    },
    setPaused(p) { paused = p; p ? video.pause() : play(); },
    setFps() {},
    destroy() { video.pause(); video.removeAttribute('src'); video.load(); video.remove(); },
  };
  r.update(values);
  return r;
}

function htmlRenderer(wallpaper, values, paused) {
  const frame = document.createElement('iframe');
  // allow-scripts without allow-same-origin: opaque origin, no IPC, no storage of ours.
  frame.setAttribute('sandbox', 'allow-scripts');
  frame.setAttribute('allow', 'autoplay');
  frame.src = fileUrl(wallpaper, wallpaper.manifest.entry);
  stage.append(frame);

  let state = { values, paused };
  const post = (event) =>
    frame.contentWindow?.postMessage({ type: 'wallgates', event, ...state }, '*');
  frame.addEventListener('load', () => post('init'));

  return {
    update(v) { state.values = v; post('properties'); },
    setPaused(p) { state.paused = p; post('pause'); },
    setFps() {},
    destroy() { frame.remove(); },
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

async function shaderRenderer(wallpaper, values, paused, fps) {
  const res = await fetch(fileUrl(wallpaper, wallpaper.manifest.entry));
  if (!res.ok) throw new Error(`cannot load shader (${res.status})`);
  const source = await res.text();

  const canvas = document.createElement('canvas');
  stage.append(canvas);
  const gl = canvas.getContext('webgl2', { antialias: false, alpha: false, powerPreference: 'low-power' });
  if (!gl) throw new Error('WebGL2 is not available');

  const uniforms = shaderUniforms(wallpaper.manifest.properties);
  const fragment = [
    '#version 300 es',
    'precision highp float;',
    'uniform vec3 iResolution;',
    'uniform float iTime;',
    'uniform float iTimeDelta;',
    'uniform int iFrame;',
    'uniform vec4 iMouse;',
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
  const uRes = loc('iResolution'), uTime = loc('iTime'), uDelta = loc('iTimeDelta'), uFrame = loc('iFrame'), uMouse = loc('iMouse');
  gl.uniform4f(uMouse, 0, 0, 0, 0);
  const propLocs = uniforms.map((u) => ({ ...u, loc: loc(u.name) })).filter((u) => u.loc);

  let currentValues = values;
  let fpsLimit = fps;
  let time = 0, frame = 0, last = 0, lastDraw = 0, raf = 0, running = false;

  const applyValues = () => propLocs.forEach((u) => u.set(gl, u.loc, currentValues[u.key]));

  const resize = () => {
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.round(innerWidth * dpr)), h = Math.max(1, Math.round(innerHeight * dpr));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w; canvas.height = h;
      gl.viewport(0, 0, w, h);
    }
    gl.uniform3f(uRes, canvas.width, canvas.height, 1);
  };

  const draw = (dt) => {
    resize();
    gl.uniform1f(uTime, time);
    gl.uniform1f(uDelta, dt);
    gl.uniform1i(uFrame, frame++);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  };

  const loop = (now) => {
    raf = requestAnimationFrame(loop);
    const dt = last ? Math.min((now - last) / 1000, 0.25) : 0;
    last = now;
    time += dt;
    if (fpsLimit > 0 && now - lastDraw < 1000 / fpsLimit - 2) return;
    draw(now - lastDraw > 0 && lastDraw ? (now - lastDraw) / 1000 : dt);
    lastDraw = now;
  };

  const start = () => { if (!running) { running = true; last = 0; raf = requestAnimationFrame(loop); } };
  const stop = () => { running = false; cancelAnimationFrame(raf); };

  addEventListener('resize', resize);
  applyValues();
  draw(0);
  if (!paused) start();

  return {
    update(v) { currentValues = v; applyValues(); draw(0); }, // instant feedback, even while paused
    setPaused(p) { p ? stop() : start(); },
    setFps(n) { fpsLimit = n; },
    destroy() {
      stop();
      removeEventListener('resize', resize);
      gl.getExtension('WEBGL_lose_context')?.loseContext();
      canvas.remove();
    },
  };
}

// ------------------------------------------------------------------ state

let renderer = null;
let currentKey = null;
let pending = Promise.resolve();

function keyOf(state) {
  const w = state.wallpaper;
  return w ? `${w.id}|${w.manifest.type}|${w.manifest.entry}` : null;
}

async function apply(state) {
  const key = keyOf(state);
  if (key !== currentKey) {
    renderer?.destroy();
    renderer = null;
    stage.replaceChildren();
    showError('');
    currentKey = key;
    const w = state.wallpaper;
    if (!w) return;
    try {
      switch (w.manifest.type) {
        case 'image': renderer = imageRenderer(w, state.values); break;
        case 'video': renderer = videoRenderer(w, state.values); break;
        case 'html': renderer = htmlRenderer(w, state.values, state.paused); break;
        case 'shader': renderer = await shaderRenderer(w, state.values, state.paused, state.fps_limit); break;
        default: throw new Error(`unknown wallpaper type: ${w.manifest.type}`);
      }
    } catch (e) {
      showError(`${w.manifest.name}: ${e.message || e}`);
      return;
    }
  } else if (renderer) {
    renderer.update(state.values);
  }
  renderer?.setFps(state.fps_limit);
  renderer?.setPaused(state.paused);
}

// Serialize updates so async shader loading never races a newer state.
const enqueue = (state) => { pending = pending.then(() => apply(state)).catch(console.error); };

listen('wf-state', (event) => enqueue(event.payload));
invoke('host_state').then(enqueue);
