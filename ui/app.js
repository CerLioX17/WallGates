// Settings window: wallpaper library, per-wallpaper properties, global settings.
// Manifest data is untrusted (third-party wallpapers): never use innerHTML with it.
'use strict';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const WP_BASE = /Windows/.test(navigator.userAgent) ? 'http://wp.localhost/' : 'wp://localhost/';
const $ = (sel) => document.querySelector(sel);

let wallpapers = [];
let hostState = null;
let filter = 'all';
let query = '';
let detailsFor = null; // wallpaper id currently rendered in the details panel

function el(tag, props = {}, ...children) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k === 'class') node.className = v;
    else if (k === 'dataset') Object.assign(node.dataset, v);
    else if (k.startsWith('on')) node.addEventListener(k.slice(2), v);
    else if (v !== undefined && v !== null) node[k] = v;
  }
  node.append(...children.filter((c) => c !== null && c !== undefined));
  return node;
}

function fileUrl(wallpaper, rel) {
  return WP_BASE + [...wallpaper.id.split('/'), ...rel.split('/')].map(encodeURIComponent).join('/');
}

function toast(message, isError = false) {
  const t = $('#toast');
  t.textContent = message;
  t.classList.toggle('error', isError);
  t.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => (t.hidden = true), 3500);
}

async function call(cmd, args) {
  try {
    return await invoke(cmd, args);
  } catch (e) {
    toast(String(e), true);
    throw e;
  }
}

// ------------------------------------------------------------------ library

const TYPE_LABEL = { image: 'Image', video: 'Video', html: 'Web', shader: 'Shader' };

function previewOf(w) {
  const m = w.manifest;
  if (m.preview) return el('img', { src: fileUrl(w, m.preview), alt: '', loading: 'lazy' });
  if (m.type === 'image') return el('img', { src: fileUrl(w, m.entry), alt: '', loading: 'lazy' });
  if (m.type === 'video') {
    return el('video', { src: fileUrl(w, m.entry) + '#t=1', muted: true, preload: 'metadata' });
  }
  return el('div', { class: `placeholder t-${m.type}` }, TYPE_LABEL[m.type] || m.type);
}

function renderGrid() {
  const grid = $('#grid');
  const active = hostState?.wallpaper?.id;
  const q = query.trim().toLowerCase();
  const items = wallpapers.filter((w) => {
    if (filter !== 'all' && w.source !== filter) return false;
    if (!q) return true;
    const m = w.manifest;
    return [m.name, m.author, m.description, m.type].some((s) => (s || '').toLowerCase().includes(q));
  });

  grid.replaceChildren(
    ...items.map((w) =>
      el(
        'button',
        {
          class: 'card' + (w.id === active ? ' active' : ''),
          title: w.manifest.description || w.manifest.name,
          onclick: () => w.id !== active && call('set_active', { id: w.id }),
        },
        el('div', { class: 'thumb' }, previewOf(w), el('span', { class: 'badge' }, TYPE_LABEL[w.manifest.type] || w.manifest.type)),
        el('div', { class: 'card-body' },
          el('strong', {}, w.manifest.name),
          el('small', {}, w.manifest.author || (w.source === 'user' ? 'Imported' : 'Wallgates')),
        ),
      ),
    ),
  );
  $('#empty').hidden = items.length > 0;
}

async function reloadLibrary() {
  wallpapers = await call('list_wallpapers');
  renderGrid();
}

// ------------------------------------------------------------------ properties

function throttle(fn, ms) {
  let timer = null, lastArgs = null;
  return (...args) => {
    lastArgs = args;
    if (timer) return;
    timer = setTimeout(() => { timer = null; fn(...lastArgs); }, ms);
  };
}

function control(wallpaper, key, def, value) {
  const set = (v) => call('set_property', { id: wallpaper.id, key, value: v });
  const label = def.label || key;

  switch (def.type) {
    case 'slider': {
      const min = Number(def.min ?? 0), max = Number(def.max ?? 1), step = Number(def.step ?? 0.01);
      const out = el('output', {}, String(value));
      const send = throttle(set, 40);
      const input = el('input', {
        type: 'range', min, max, step, value,
        oninput: (e) => { const v = Number(e.target.value); out.textContent = String(v); send(v); },
      });
      return el('div', { class: 'prop' }, el('div', { class: 'prop-head' }, el('span', {}, label), out), input);
    }
    case 'color': {
      const send = throttle(set, 40);
      return el('label', { class: 'prop row' }, el('span', {}, label),
        el('input', { type: 'color', value: /^#[0-9a-f]{6}$/i.test(value) ? value : '#ffffff', oninput: (e) => send(e.target.value) }));
    }
    case 'bool':
      return el('label', { class: 'prop row' }, el('span', {}, label),
        el('input', { type: 'checkbox', class: 'switch', checked: !!value, onchange: (e) => set(e.target.checked) }));
    case 'select': {
      const select = el('select', { onchange: (e) => set(e.target.value) },
        ...(def.options || []).map((o) => {
          const opt = typeof o === 'object' ? o : { value: o, label: String(o) };
          return el('option', { value: opt.value, selected: opt.value === value }, opt.label ?? String(opt.value));
        }));
      return el('label', { class: 'prop row' }, el('span', {}, label), select);
    }
    case 'text': {
      const send = throttle(set, 250);
      return el('label', { class: 'prop' }, el('span', {}, label),
        el('input', { type: 'text', value: value ?? '', maxLength: 500, oninput: (e) => send(e.target.value) }));
    }
    default:
      return null;
  }
}

function renderDetails(force = false) {
  const w = hostState?.wallpaper;
  if (!force && w?.id === detailsFor) return; // keep controls stable while the user drags
  detailsFor = w?.id ?? null;
  const box = $('#details');
  if (!w) {
    box.replaceChildren(el('p', { class: 'muted' }, 'Pick a wallpaper from the library.'));
    return;
  }
  const m = w.manifest;
  const props = Object.entries(m.properties || {})
    .map(([key, def]) => control(w, key, def, hostState.values[key]))
    .filter(Boolean);

  box.replaceChildren(
    el('div', { class: 'details-head' },
      el('span', { class: 'badge inline' }, TYPE_LABEL[m.type] || m.type),
      el('h2', {}, m.name),
      m.author ? el('p', { class: 'muted' }, `by ${m.author}`) : null,
      m.description ? el('p', {}, m.description) : null,
    ),
    el('h3', {}, 'Customize'),
    props.length ? el('div', { class: 'props' }, ...props) : el('p', { class: 'muted' }, 'This wallpaper has no options.'),
    el('div', { class: 'details-actions' },
      props.length ? el('button', { class: 'btn ghost', onclick: async () => { await call('reset_properties', { id: w.id }); hostState = await call('host_state'); renderDetails(true); } }, 'Reset to defaults') : null,
      w.editable ? el('button', {
        class: 'btn danger',
        onclick: async () => {
          if (!confirm(`Delete "${m.name}" from your library?`)) return;
          await call('delete_wallpaper', { id: w.id });
          await reloadLibrary();
        },
      }, 'Delete') : null,
    ),
  );
}

// ------------------------------------------------------------------ status & settings

function renderStatus() {
  if (!hostState) return;
  const pill = $('#status');
  const text = hostState.manual_paused ? 'Paused' : hostState.auto_paused ? 'Auto-paused' : 'Running';
  pill.textContent = text;
  pill.className = 'pill ' + (hostState.paused ? 'paused' : 'running');
  pill.title = hostState.auto_paused ? 'Paused because of a fullscreen/maximized app or battery power' : '';
  $('#pause-btn').textContent = hostState.manual_paused ? 'Resume' : 'Pause';
}

async function loadSettings() {
  const cfg = await call('get_settings');
  document.querySelectorAll('[data-setting]').forEach((input) => {
    const key = input.dataset.setting;
    if (input.type === 'checkbox') input.checked = !!cfg[key];
    else input.value = String(cfg[key]);
    input.addEventListener('change', () => {
      const value = input.type === 'checkbox' ? input.checked : Number(input.value);
      call('update_settings', { patch: { [key]: value } });
    });
  });
}

function onState(state) {
  const changedWallpaper = state.wallpaper?.id !== hostState?.wallpaper?.id;
  hostState = state;
  renderStatus();
  if (changedWallpaper) {
    renderGrid();
    renderDetails();
  }
}

// ------------------------------------------------------------------ wiring

$('#pause-btn').addEventListener('click', () =>
  call('update_settings', { patch: { paused: !hostState?.manual_paused } }));

$('#search').addEventListener('input', (e) => { query = e.target.value; renderGrid(); });

document.querySelectorAll('.chip').forEach((chip) =>
  chip.addEventListener('click', () => {
    document.querySelectorAll('.chip').forEach((c) => c.classList.toggle('active', c === chip));
    filter = chip.dataset.filter;
    renderGrid();
  }));

for (const [id, cmd] of [['#import-file', 'import_file'], ['#import-folder', 'import_folder']]) {
  $(id).addEventListener('click', async () => {
    const newId = await call(cmd);
    if (newId) {
      await reloadLibrary();
      toast('Wallpaper imported');
    }
  });
}

$('#open-folder').addEventListener('click', () => call('open_library_folder'));
$('#reapply').addEventListener('click', () => call('reapply').then(() => toast('Re-attached to desktop')));
window.addEventListener('focus', reloadLibrary); // pick up folders dropped into the library

(async () => {
  listen('wf-state', (event) => onState(event.payload));
  await loadSettings();
  await reloadLibrary();
  onState(await call('host_state'));
})();
