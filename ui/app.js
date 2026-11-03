// Wallgates settings app.
// Manifest data is untrusted (third-party wallpapers): never use innerHTML with it.
'use strict';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const WP_BASE = /Windows/.test(navigator.userAgent) ? 'http://wp.localhost/' : 'wp://localhost/';
const REPO_URL = 'https://github.com/CerLioX17/WallGates';
const $ = (sel, root = document) => root.querySelector(sel);

// ------------------------------------------------------------------ icons (trusted, static)
const P = (d) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">${d}</svg>`;
const ICONS = {
  grid: P('<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>'),
  monitor: P('<rect x="2" y="4" width="20" height="13" rx="2"/><path d="M8 21h8M12 17v4"/>'),
  list: P('<path d="M8 6h13M8 12h13M8 18h13"/><circle cx="3.5" cy="6" r="1"/><circle cx="3.5" cy="12" r="1"/><circle cx="3.5" cy="18" r="1"/>'),
  sparkles: P('<path d="M12 3l1.8 4.7L18.5 9.5l-4.7 1.8L12 16l-1.8-4.7L5.5 9.5l4.7-1.8z"/><path d="M19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z"/>'),
  code: P('<path d="M16 18l6-6-6-6M8 6l-6 6 6 6"/>'),
  settings: P('<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>'),
  pause: P('<rect x="6" y="4" width="4" height="16" rx="1"/><rect x="14" y="4" width="4" height="16" rx="1"/>'),
  play: P('<path d="M6 4l14 8-14 8z"/>'),
  next: P('<path d="M5 4l10 8-10 8zM19 5v14"/>'),
  volume: P('<path d="M11 5L6 9H2v6h4l5 4z"/><path d="M15.5 8.5a5 5 0 0 1 0 7M19 5a10 10 0 0 1 0 14"/>'),
  mute: P('<path d="M11 5L6 9H2v6h4l5 4z"/><path d="M23 9l-6 6M17 9l6 6"/>'),
  min: P('<path d="M5 12h14"/>'),
  max: P('<rect x="5" y="5" width="14" height="14" rx="1"/>'),
  close: P('<path d="M6 6l12 12M18 6L6 18"/>'),
  heart: P('<path d="M20.8 4.6a5.5 5.5 0 0 0-7.8 0L12 5.7l-1-1.1a5.5 5.5 0 0 0-7.8 7.8l1 1L12 21l7.8-7.6 1-1a5.5 5.5 0 0 0 0-7.8z"/>'),
  download: P('<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"/>'),
  upload: P('<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M17 8l-5-5-5 5M12 3v12"/>'),
  plus: P('<path d="M12 5v14M5 12h14"/>'),
  trash: P('<path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6"/>'),
  copy: P('<rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1"/>'),
  edit: P('<path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z"/>'),
  music: P('<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>'),
  mouse: P('<rect x="6" y="3" width="12" height="18" rx="6"/><path d="M12 7v4"/>'),
  cpu: P('<rect x="5" y="5" width="14" height="14" rx="2"/><rect x="9" y="9" width="6" height="6"/><path d="M9 1v3M15 1v3M9 20v3M15 20v3M20 9h3M20 14h3M1 9h3M1 14h3"/>'),
  media: P('<circle cx="12" cy="12" r="10"/><path d="M10 8l6 4-6 4z"/>'),
  cloud: P('<path d="M18 10h-1.3A8 8 0 1 0 9 20h9a5 5 0 0 0 0-10z"/>'),
  globe: P('<circle cx="12" cy="12" r="10"/><path d="M2 12h20M12 2a15 15 0 0 1 0 20M12 2a15 15 0 0 0 0 20"/>'),
  shield: P('<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><path d="M9 12l2 2 4-4"/>'),
  arrowUp: P('<path d="M18 15l-6-6-6 6"/>'),
  arrowDown: P('<path d="M6 9l6 6 6-6"/>'),
  back: P('<path d="M15 18l-6-6 6-6"/>'),
  save: P('<path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><path d="M17 21v-8H7v8M7 3v5h8"/>'),
  folder: P('<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>'),
  image: P('<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/><path d="M21 15l-5-5L5 21"/>'),
  package: P('<path d="M21 16V8a2 2 0 0 0-1-1.7l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.7l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/><path d="M3.3 7L12 12l8.7-5M12 22V12"/>'),
  gate: P('<path d="M6 21V10a6 6 0 0 1 12 0v11"/><path d="M3 21h18"/>'),
  chart: P('<path d="M3 3v18h18"/><path d="M7 15l4-4 3 3 5-6"/>'),
};
const CAP_ICON = { audio: 'music', mouse: 'mouse', system: 'cpu', media: 'media', weather: 'cloud', markets: 'chart', network: 'globe' };

function icon(name, cls = 'ico') {
  const s = document.createElement('span');
  s.className = cls;
  s.innerHTML = ICONS[name] || '';
  return s;
}
document.querySelectorAll('[data-icon]').forEach((n) => { n.innerHTML = ICONS[n.dataset.icon] || ''; });

// ------------------------------------------------------------------ helpers
function el(tag, props = {}, ...children) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (v === undefined || v === null || v === false) continue;
    if (k === 'class') node.className = v;
    else if (k === 'dataset') Object.assign(node.dataset, v);
    else if (k === 'style') node.style.cssText = v;
    else if (k.startsWith('on')) node.addEventListener(k.slice(2), v);
    else node[k] = v;
  }
  node.append(...children.flat().filter((c) => c !== null && c !== undefined && c !== false));
  return node;
}

function toast(message, isError = false) {
  const tEl = $('#toast');
  tEl.textContent = message;
  tEl.classList.toggle('error', isError);
  tEl.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => (tEl.hidden = true), isError ? 6000 : 3200);
}

async function call(cmd, args) {
  try {
    return await invoke(cmd, args);
  } catch (e) {
    toast(String(e), true);
    throw e;
  }
}

function throttle(fn, ms) {
  let timer = null, lastArgs = null;
  return (...args) => {
    lastArgs = args;
    if (timer) return;
    timer = setTimeout(() => { timer = null; fn(...lastArgs); }, ms);
  };
}

function modal(build) {
  const root = $('#modal-root');
  const back = el('div', { class: 'modal-back' });
  const close = () => back.remove();
  back.addEventListener('mousedown', (e) => { if (e.target === back) close(); });
  const box = el('div', { class: 'modal' });
  box.append(...[].concat(build(close)));
  back.append(box);
  root.append(back);
  return close;
}

function fileUrl(w, rel) {
  return WP_BASE + [...w.id.split('/'), ...rel.split('/')].map(encodeURIComponent).join('/');
}

function fmtSize(bytes) {
  if (!bytes) return '—';
  const u = ['B', 'KB', 'MB', 'GB'];
  let i = 0, n = bytes;
  while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
  return `${n.toFixed(n < 10 && i > 0 ? 1 : 0)} ${u[i]}`;
}

const TYPE_LABEL = { image: 'Image', video: 'Video', html: 'Web', shader: 'Shader' };

function previewNode(w, { live = false } = {}) {
  const m = w.manifest;
  if (m.preview) return el('img', { src: fileUrl(w, m.preview), alt: '', loading: 'lazy' });
  if (m.type === 'image') return el('img', { src: fileUrl(w, m.entry), alt: '', loading: 'lazy' });
  if (m.type === 'video') {
    return el('video', { src: fileUrl(w, m.entry) + (live ? '' : '#t=1'), muted: true, preload: 'metadata', autoplay: live, loop: live, playsInline: true });
  }
  return el('div', { class: `placeholder t-${m.type}` }, icon(m.type === 'shader' ? 'sparkles' : 'code'), TYPE_LABEL[m.type] || m.type);
}

// ------------------------------------------------------------------ state
let ui = null;
let wallpapers = [];
let route = 'library';
const lib = { filter: 'all', query: '', target: 'default', selected: null, detailsFor: null };
let editor = null;
let plSelected = null;
let plDraft = null;

const wpById = (id) => wallpapers.find((w) => w.id === id);
const cfg = () => ui?.config || {};
const monitorName = (m) => `${t('disp.display')} ${m.index + 1}`;
const patch = (p) => call('update_config', { patch: p });

function sourceOf(target) {
  const c = cfg();
  return target === 'default' ? c.default_source : (c.monitors || {})[target] || null;
}
function encodeSource(s) { return s ? (s.kind === 'playlist' ? `p:${s.id}` : `w:${s.id}`) : ''; }
function decodeSource(v) { return v ? { kind: v[0] === 'p' ? 'playlist' : 'wallpaper', id: v.slice(2) } : null; }
function displaysOf(id) {
  const slots = (ui?.slots || []).filter((s) => s.wallpaper_id === id);
  const mons = ui?.monitors || [];
  if (mons.length < 2) return t('lib.onDisplay');
  const names = slots.map((s) => mons.find((m) => m.id === s.monitor_id)).filter(Boolean).map((m) => String(m.index + 1));
  return names.length ? `${t('disp.display')} ${names.join(' · ')}` : t('lib.onDisplay');
}
function liveIds() { return new Set((ui?.slots || []).map((s) => s.wallpaper_id).filter(Boolean)); }

async function refreshLibrary() {
  wallpapers = await call('list_wallpapers');
  if (route === 'library') { renderGrid(); renderDetails(true); }
}

// ------------------------------------------------------------------ title bar & status
const appWindow = window.__TAURI__.window.getCurrentWindow();
$('#w-min').append(icon('min'));
$('#w-max').append(icon('max'));
$('#w-close').append(icon('close'));
$('#w-min').onclick = () => appWindow.minimize();
$('#w-max').onclick = () => appWindow.toggleMaximize();
$('#w-close').onclick = () => appWindow.close();
$('#q-pause').onclick = () => patch({ paused: !cfg().paused });
$('#q-next').onclick = () => call('next_wallpaper');
$('#q-mute').onclick = () => patch({ muted: !cfg().muted });

function reasonText(r) {
  if (!r) return '';
  if (r.startsWith('app:')) return `${t('reason.app')} ${r.slice(4)}`;
  return t(`reason.${r}`);
}

function renderStatus() {
  if (!ui) return;
  const c = cfg();
  const slots = ui.slots || [];
  const pill = $('#status');
  let cls = 'running', text = t('status.running'), title = '';
  const autoPaused = slots.find((s) => (s.paused || s.stopped) && s.reason && s.reason !== 'manual');
  const eco = slots.find((s) => s.eco);
  if (c.paused) { cls = 'paused'; text = t('status.paused'); title = reasonText('manual'); }
  else if (autoPaused) { cls = 'paused'; text = autoPaused.stopped ? t('status.stopped') : t('status.auto'); title = reasonText(autoPaused.reason); }
  else if (eco) { cls = 'eco'; text = t('status.eco'); title = reasonText(eco.reason); }
  pill.className = `pill ${cls}`;
  pill.textContent = text;
  pill.title = title;
  const qp = $('#q-pause');
  qp.replaceChildren(icon(c.paused ? 'play' : 'pause'));
  qp.title = c.paused ? t('action.resume') : t('action.pause');
  const qm = $('#q-mute');
  qm.replaceChildren(icon(c.muted ? 'mute' : 'volume'));
  qm.title = c.muted ? t('action.unmute') : t('action.mute');
  $('#q-next').replaceChildren(icon('next'));
  $('#version').textContent = `Wallgates ${ui.version}`;

  const banner = $('#update-banner');
  if (ui.update) {
    banner.hidden = false;
    banner.replaceChildren(
      el('strong', {}, t('upd.available', { version: ui.update.version })),
      el('button', { class: 'btn primary small', onclick: () => call('open_url', { url: ui.update.url }) }, icon('download'), t('upd.get')),
    );
  } else banner.hidden = true;
}

// ------------------------------------------------------------------ router
document.querySelectorAll('.nav').forEach((b) => b.addEventListener('click', () => go(b.dataset.route)));

function go(r) {
  if (editor && r !== 'create') editor = null;
  route = r;
  document.querySelectorAll('.nav').forEach((b) => b.classList.toggle('active', b.dataset.route === r));
  renderView();
}

function renderView() {
  const view = $('#view');
  view.replaceChildren();
  switch (route) {
    case 'library': return renderLibrary(view);
    case 'displays': return renderDisplays(view);
    case 'playlists': return renderPlaylists(view);
    case 'gallery': return renderGallery(view);
    case 'create': return editor ? renderEditor(view) : renderCreate(view);
    case 'settings': return renderSettings(view);
  }
}

// ------------------------------------------------------------------ import
async function doImport(kind) {
  const res = await call('import_dialog', { kind });
  importResult(res, kind === 'wallpaper_engine');
}

function importResult(res, fromWE = false) {
  if (!res || res.cancelled) return;
  const parts = [];
  if (res.imported.length) parts.push(t('imp.done', { n: res.imported.length }));
  if (res.skipped.length) parts.push(t('imp.skipped', { n: res.skipped.length }));
  const msg = parts.join(' · ') + (res.skipped.length ? `\n${res.skipped.slice(0, 3).join('\n')}` : '');
  toast(msg + (fromWE && res.imported.length ? `\n${t('imp.weNote')}` : ''), !res.imported.length);
  if (res.imported.length) { lib.selected = res.imported[0]; refreshLibrary(); }
}

function importMenu() {
  return modal((close) => [
    el('h2', {}, t('lib.import')),
    el('div', { class: 'stack', style: 'margin-top:14px' },
      ...[['media', 'image', 'lib.importMedia'], ['folder', 'folder', 'lib.importFolder'], ['package', 'package', 'lib.importPackage'], ['wallpaper_engine', 'gate', 'lib.importWE']]
        .map(([kind, ic, key]) => el('button', { class: 'btn', onclick: () => { close(); doImport(kind); } }, icon(ic), t(key))),
      el('p', { class: 'faint', style: 'font-size:12px;margin-top:6px' }, t('lib.dropHint')),
    ),
  ]);
}

// ------------------------------------------------------------------ library
function renderLibrary(view) {
  const monitors = ui?.monitors || [];
  const span = cfg().layout === 'span';
  const multi = monitors.length > 1;
  if (!multi || span) lib.target = 'default';
  // Start on the primary display rather than "All", so one click never changes every screen by surprise.
  else if (!lib.targetPicked || (lib.target !== 'default' && !monitors.some((m) => m.id === lib.target))) { lib.target = (monitors.find((m) => m.primary) || monitors[0]).id; lib.targetPicked = true; }

  // Explicit display picker: clicking a wallpaper applies it only to the display chosen here.
  const targetSel = !multi ? null : span
    ? el('div', { class: 'row target-note' }, el('span', { class: 'muted' }, t('lib.spanOn')),
        el('button', { class: 'btn small', onclick: async () => { await patch({ layout: 'per_monitor' }); lib.target = monitors[0].id; } }, t('lib.perDisplay')))
    : el('div', { class: 'row', style: 'gap:8px' }, el('span', { class: 'muted' }, t('lib.applyTo')),
        el('div', { class: 'seg target-seg' },
          ...monitors.map((m) => el('button', {
            class: lib.target === m.id ? 'active' : '', title: `${m.rect.width}×${m.rect.height}${m.primary ? ' · ' + t('disp.primary') : ''}`,
            onclick: (e) => { lib.target = m.id; e.target.parentNode.querySelectorAll('button').forEach((b) => b.classList.toggle('active', b === e.target)); lib.selected = null; renderGrid(); renderDetails(true); },
          }, monitorName(m))),
          el('button', {
            class: lib.target === 'default' ? 'active' : '',
            onclick: (e) => { lib.target = 'default'; e.target.parentNode.querySelectorAll('button').forEach((b) => b.classList.toggle('active', b === e.target)); renderGrid(); },
          }, t('lib.allDisplays'))));

  const chips = el('div', { class: 'seg' },
    ...['all', 'builtin', 'user', 'favorites'].map((f) => el('button', {
      class: lib.filter === f ? 'active' : '',
      onclick: (e) => { lib.filter = f; e.target.parentNode.querySelectorAll('button').forEach((b) => b.classList.toggle('active', b === e.target)); renderGrid(); },
    }, t({ all: 'lib.all', builtin: 'lib.builtin', user: 'lib.mine', favorites: 'lib.favorites' }[f]))));

  const page = el('section', { class: 'page' },
    el('div', { class: 'toolbar' },
      el('input', { type: 'search', placeholder: t('lib.search'), value: lib.query, oninput: (e) => { lib.query = e.target.value; renderGrid(); } }),
      chips,
      el('div', { class: 'spacer' }),
      targetSel,
      el('button', { class: 'btn primary', onclick: importMenu }, icon('upload'), t('lib.import')),
    ),
    el('div', { id: 'grid', class: 'grid' }),
    el('p', { id: 'empty', class: 'empty', hidden: true }, t('lib.empty')),
  );
  view.append(el('div', { class: 'library' }, page, el('aside', { id: 'details', class: 'details' })));
  renderGrid();
  renderDetails(true);
}

function renderGrid() {
  const grid = $('#grid');
  if (!grid) return;
  const c = cfg();
  const live = liveIds();
  const favs = new Set(c.favorites || []);
  const targetSrc = sourceOf(lib.target);
  const q = lib.query.trim().toLowerCase();
  const items = wallpapers.filter((w) => {
    const m = w.manifest;
    if (m.rating === 'mature' && !c.show_mature) return false;
    if (lib.filter === 'favorites' && !favs.has(w.id)) return false;
    if ((lib.filter === 'builtin' || lib.filter === 'user') && w.source !== lib.filter) return false;
    if (!q) return true;
    return [m.name, m.author, m.description, m.type, ...(m.tags || [])].some((s) => (s || '').toLowerCase().includes(q));
  });
  grid.replaceChildren(...items.map((w) => {
    const isTarget = targetSrc?.kind === 'wallpaper' && targetSrc.id === w.id;
    return el('div', {
      class: 'card' + (lib.selected === w.id ? ' selected' : ''),
      title: w.manifest.description || w.manifest.name,
      onclick: () => applyWallpaper(w.id),
    },
      el('div', { class: 'thumb' }, previewNode(w),
        el('div', { class: 'badges' },
          live.has(w.id) ? el('span', { class: 'badge live' }, displaysOf(w.id)) : el('span'),
          el('span', { class: 'badge' }, TYPE_LABEL[w.manifest.type] || w.manifest.type))),
      el('button', {
        class: 'fav' + (favs.has(w.id) ? ' on' : ''), title: t('det.favorite'),
        onclick: (e) => { e.stopPropagation(); call('toggle_favorite', { id: w.id }); },
      }, icon('heart')),
      el('div', { class: 'card-body' },
        el('strong', {}, w.manifest.name),
        el('small', {}, w.manifest.author || (w.source === 'user' ? t('lib.mine') : 'Wallgates'),
          w.manifest.ai_generated ? el('span', { class: 'chip' }, 'AI') : null,
          isTarget ? null : null)),
    );
  }));
  $('#empty').hidden = items.length > 0;
}

async function applyWallpaper(id) {
  lib.selected = id;
  await call('set_source', { target: lib.target, source: { kind: 'wallpaper', id } });
  const mon = (ui?.monitors || []).find((m) => m.id === lib.target);
  if ((ui?.monitors || []).length > 1) toast(`✓ ${wpById(id)?.manifest.name || ''} → ${mon ? monitorName(mon) : t('lib.allDisplays')}`);
  renderGrid();
  renderDetails(true);
}

function control(w, key, def, value) {
  const set = (v) => call('set_property', { id: w.id, key, value: v });
  const label = def.label || key;
  switch (def.type) {
    case 'slider': {
      const min = Number(def.min ?? 0), max = Number(def.max ?? 1), step = Number(def.step ?? 0.01);
      const out = el('output', {}, String(value));
      const send = throttle(set, 40);
      return el('div', { class: 'prop' }, el('div', { class: 'prop-head' }, el('span', {}, label), out),
        el('input', { type: 'range', min, max, step, value, oninput: (e) => { const v = Number(e.target.value); out.textContent = String(v); send(v); } }));
    }
    case 'color': {
      const send = throttle(set, 40);
      return el('label', { class: 'prop row' }, el('span', {}, label),
        el('input', { type: 'color', value: /^#[0-9a-f]{6}$/i.test(value) ? value : '#ffffff', oninput: (e) => send(e.target.value) }));
    }
    case 'bool':
      return el('label', { class: 'prop row' }, el('span', {}, label),
        el('input', { type: 'checkbox', class: 'switch', checked: !!value, onchange: (e) => set(e.target.checked) }));
    case 'select':
      return el('label', { class: 'prop row' }, el('span', {}, label),
        el('select', { onchange: (e) => set(e.target.value) },
          ...(def.options || []).map((o) => {
            const opt = typeof o === 'object' ? o : { value: o, label: String(o) };
            return el('option', { value: opt.value, selected: String(opt.value) === String(value) }, opt.label ?? String(opt.value));
          })));
    case 'text': {
      const send = throttle(set, 250);
      return el('label', { class: 'prop' }, el('span', {}, label),
        el('input', { type: 'text', value: value ?? '', maxLength: 500, placeholder: def.placeholder || '', oninput: (e) => send(e.target.value) }));
    }
    case 'textarea': {
      const send = throttle(set, 600);
      return el('label', { class: 'prop' }, el('span', {}, label),
        el('textarea', { class: 'prop-area', rows: def.rows || 5, value: value ?? '', maxLength: 8000, spellcheck: false, placeholder: def.placeholder || '', oninput: (e) => send(e.target.value) }));
    }
    case 'file': {
      // Reads a local text file (e.g. a CSV export) and stores its contents as the value. Stays on this PC.
      const size = (n) => (n > 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);
      const status = el('small', { class: 'prop-file-status' }, value ? t('prop.file.loaded', { size: size(new Blob([value]).size) }) : t('prop.file.none'));
      const input = el('input', {
        type: 'file', accept: def.accept || '', hidden: true,
        onchange: async (e) => {
          const f = e.target.files?.[0];
          if (!f) return;
          if (f.size > 2 * 1048576) { status.textContent = t('prop.file.tooBig'); return; }
          const text = await f.text();
          await set(text);
          status.textContent = `${f.name} · ${t('prop.file.loaded', { size: size(f.size) })}`;
        },
      });
      return el('div', { class: 'prop' }, el('span', {}, label),
        el('div', { class: 'prop-file' },
          el('button', { class: 'btn small', onclick: () => input.click() }, t('prop.file.choose')),
          value ? el('button', { class: 'btn small ghost', onclick: async () => { await set(''); status.textContent = t('prop.file.none'); } }, t('prop.file.clear')) : null,
          status, input));
    }
    case 'group':
      return el('h4', { class: 'prop-group' }, label);
    case 'note':
      return el('p', { class: 'prop-note' }, label);
    default:
      return null;
  }
}

async function renderDetails(force = false) {
  const box = $('#details');
  if (!box) return;
  // Default to what the target display currently shows.
  if (!lib.selected) {
    const src = sourceOf(lib.target);
    if (src?.kind === 'wallpaper') lib.selected = src.id;
  }
  const w = wpById(lib.selected);
  if (!force && lib.detailsFor === (w?.id ?? null)) return;
  lib.detailsFor = w?.id ?? null;
  if (!w) { box.replaceChildren(el('p', { class: 'muted' }, t('det.pick'))); return; }

  const m = w.manifest;
  const values = {};
  for (const [k, def] of Object.entries(m.properties || {})) values[k] = def.default;
  Object.assign(values, (cfg().properties || {})[w.id] || {});
  const props = Object.entries(m.properties || {}).map(([k, def]) => control(w, k, def, values[k])).filter(Boolean);
  const caps = m.capabilities || [];

  box.replaceChildren(...[
    el('div', { class: 'hero' }, previewNode(w, { live: true })),
    el('div', { class: 'row', style: 'justify-content:flex-start;gap:6px;flex-wrap:wrap' },
      el('span', { class: 'chip grad' }, TYPE_LABEL[m.type] || m.type),
      m.ai_generated ? el('span', { class: 'chip' }, t('lib.ai')) : null,
      m.rating === 'mature' ? el('span', { class: 'chip warn' }, t('lib.mature')) : null,
      m.compat === 'wallpaperengine' ? el('span', { class: 'chip' }, 'Wallpaper Engine') : null),
    el('h2', { style: 'margin-top:8px' }, m.name),
    m.author ? el('p', { class: 'muted' }, `${t('det.by')} ${m.author}`) : null,
    m.description ? el('p', {}, m.description) : null,
    el('div', { class: 'actions' },
      el('button', { class: 'btn primary small', onclick: () => applyWallpaper(w.id) }, icon('play'), t('det.set')),
      w.editable
        ? el('button', { class: 'btn small', onclick: () => openEditor(w.id) }, icon('edit'), t('det.edit'))
        : el('button', { class: 'btn small', onclick: async () => { const id = await call('duplicate_wallpaper', { id: w.id }); await refreshLibrary(); openEditor(id); } }, icon('copy'), t('det.duplicate')),
      el('button', { class: 'btn small ghost', onclick: () => call('export_wallpaper', { id: w.id }).then((ok) => ok && toast('✓')) }, icon('package'), t('det.export'))),
    el('h3', {}, t('det.customize')),
    props.length ? el('div', { class: 'props' }, ...props) : el('p', { class: 'muted' }, t('det.noOptions')),
    props.length ? el('div', { class: 'actions' },
      el('button', { class: 'btn ghost small', onclick: async () => { await call('reset_properties', { id: w.id }); setTimeout(() => renderDetails(true), 120); } }, t('det.reset'))) : null,
    el('h3', {}, t('det.permissions')),
    caps.length
      ? el('div', {}, ...caps.map((c) => el('div', { class: 'perm' }, icon(CAP_ICON[c] || 'shield'), t(`cap.${c}`))))
      : el('div', { class: 'perm' }, icon('shield'), t('det.noPermissions')),
    el('h3', {}, t('det.info')),
    el('dl', { class: 'kv' },
      el('dt', {}, t('det.size')), el('dd', {}, fmtSize(w.size_bytes)),
      el('dt', {}, t('det.license')), el('dd', {}, m.license || '—'),
      m.version ? el('dt', {}, 'Version') : null, m.version ? el('dd', {}, m.version) : null),
    w.editable ? el('div', { class: 'actions' },
      el('button', {
        class: 'btn danger small',
        onclick: async () => {
          if (!confirm(t('det.confirmDelete', { name: m.name }))) return;
          await call('delete_wallpaper', { id: w.id });
          lib.selected = null;
          await refreshLibrary();
        },
      }, icon('trash'), t('det.delete'))) : null,
  ].filter(Boolean));
}

// ------------------------------------------------------------------ displays
function sourceSelect(value, includeDefault, onchange) {
  const c = cfg();
  return el('select', { onchange: (e) => onchange(decodeSource(e.target.value)) },
    includeDefault ? el('option', { value: '', selected: !value }, `— ${t('disp.useDefault')} —`) : null,
    el('optgroup', { label: t('disp.wallpapers') },
      ...wallpapers.map((w) => el('option', { value: `w:${w.id}`, selected: value === `w:${w.id}` }, w.manifest.name))),
    (c.playlists || []).length ? el('optgroup', { label: t('disp.playlists') },
      ...c.playlists.map((p) => el('option', { value: `p:${p.id}`, selected: value === `p:${p.id}` }, `▶ ${p.name}`))) : null);
}

function renderDisplays(view) {
  const c = cfg();
  const monitors = ui?.monitors || [];
  const page = el('section', { class: 'page' },
    el('div', { class: 'page-head' }, el('div', {}, el('h1', {}, t('disp.title')), el('p', {}, t('disp.hint')))));

  // Scaled map of the monitor layout.
  const map = el('div', { class: 'monitor-map' });
  if (monitors.length) {
    const minX = Math.min(...monitors.map((m) => m.rect.x)), minY = Math.min(...monitors.map((m) => m.rect.y));
    const maxX = Math.max(...monitors.map((m) => m.rect.x + m.rect.width)), maxY = Math.max(...monitors.map((m) => m.rect.y + m.rect.height));
    const W = 720, H = 240;
    const s = Math.min(W / (maxX - minX), H / (maxY - minY));
    const offX = (W - (maxX - minX) * s) / 2;
    for (const m of monitors) {
      const slot = (ui.slots || []).find((x) => x.monitor_id === m.id || x.monitor_id === 'span');
      const w = slot?.wallpaper_id ? wpById(slot.wallpaper_id) : null;
      map.append(el('div', {
        class: 'mon',
        style: `left:${offX + (m.rect.x - minX) * s + 4}px;top:${(m.rect.y - minY) * s + 4}px;width:${m.rect.width * s - 8}px;height:${m.rect.height * s - 8}px`,
        onclick: () => { lib.target = c.layout === 'span' ? 'default' : m.id; lib.targetPicked = true; go('library'); },
      }, w ? previewNode(w) : el('div', { class: 'placeholder t-image' }),
        el('div', { class: 'label' }, el('strong', {}, `${monitorName(m)}${m.primary ? ' ★' : ''}`), el('span', {}, slot?.wallpaper_name || ''))));
    }
  }
  page.append(map);

  page.append(el('div', { class: 'panel' },
    el('div', { class: 'row' }, el('strong', {}, t('disp.layout')),
      el('div', { class: 'seg' },
        el('button', { class: c.layout !== 'span' ? 'active' : '', onclick: () => patch({ layout: 'per_monitor' }) }, t('disp.perMonitor')),
        el('button', { class: c.layout === 'span' ? 'active' : '', onclick: () => patch({ layout: 'span' }) }, t('disp.span'))))));

  const rows = el('div', { class: 'panel' },
    el('div', { class: 'assign-row' }, el('strong', {}, t('disp.default')),
      sourceSelect(encodeSource(c.default_source), false, (s) => s && call('set_source', { target: 'default', source: s }))));
  if (c.layout !== 'span') {
    for (const m of monitors) {
      rows.append(el('div', { class: 'assign-row' },
        el('span', {}, el('strong', {}, monitorName(m)), el('span', { class: 'muted' }, `  ${m.rect.width}×${m.rect.height}${m.primary ? ' · ' + t('disp.primary') : ''}`)),
        sourceSelect(encodeSource((c.monitors || {})[m.id]), true, (s) => call('set_source', { target: m.id, source: s }))));
    }
  }
  page.append(rows);
  view.append(page);
}

// ------------------------------------------------------------------ playlists
function newPlaylist() {
  return { id: '', name: t('pl.new'), mode: 'interval', interval_minutes: 15, shuffle: false, items: [] };
}

function renderPlaylists(view) {
  const c = cfg();
  const list = c.playlists || [];
  if (plSelected && !plDraft) plDraft = structuredClone(list.find((p) => p.id === plSelected) || newPlaylist());

  const left = el('div', { class: 'pl-list' },
    el('button', { class: 'btn primary', onclick: () => { plSelected = null; plDraft = newPlaylist(); rerender(); } }, icon('plus'), t('pl.new')),
    ...list.map((p) => el('button', {
      class: 'pl-item' + (plDraft && plDraft.id === p.id ? ' active' : ''),
      onclick: () => { plSelected = p.id; plDraft = structuredClone(p); rerender(); },
    }, el('strong', {}, p.name), el('small', {}, `${p.items.length} · ${p.mode === 'interval' ? `${p.interval_minutes} min` : t('pl.timeOfDay')}`))));

  const rerender = () => { const v = $('#view'); v.replaceChildren(); renderPlaylists(v); };
  let right;
  if (!plDraft) {
    right = el('div', { class: 'panel' }, el('p', { class: 'muted' }, t('pl.empty')));
  } else {
    const d = plDraft;
    const entries = el('div', { class: 'stack' }, ...d.items.map((item, i) => {
      const w = wpById(item.wallpaper);
      const at = item.at || '07:00';
      const special = at === 'sunrise' || at === 'sunset';
      const timeCtl = d.mode === 'time_of_day'
        ? el('div', { class: 'row', style: 'gap:6px' },
            el('select', { onchange: (e) => { item.at = e.target.value === 'time' ? '07:00' : e.target.value; rerender(); } },
              el('option', { value: 'time', selected: !special }, '⏱'),
              el('option', { value: 'sunrise', selected: at === 'sunrise' }, t('pl.sunrise')),
              el('option', { value: 'sunset', selected: at === 'sunset' }, t('pl.sunset'))),
            special ? null : el('input', { type: 'time', value: at, style: 'width:100px', onchange: (e) => { item.at = e.target.value; } }))
        : el('span');
      return el('div', { class: 'pl-entry' },
        el('div', { class: 'mini' }, w ? previewNode(w) : el('div', { class: 'placeholder t-image' }, '?')),
        el('span', {}, w ? w.manifest.name : item.wallpaper),
        timeCtl,
        el('div', { class: 'row', style: 'gap:2px' },
          el('button', { class: 'icon-btn', disabled: i === 0, onclick: () => { [d.items[i - 1], d.items[i]] = [d.items[i], d.items[i - 1]]; rerender(); } }, icon('arrowUp')),
          el('button', { class: 'icon-btn', disabled: i === d.items.length - 1, onclick: () => { [d.items[i + 1], d.items[i]] = [d.items[i], d.items[i + 1]]; rerender(); } }, icon('arrowDown')),
          el('button', { class: 'icon-btn', onclick: () => { d.items.splice(i, 1); rerender(); } }, icon('trash'))));
    }));

    const targets = [['default', t('disp.default')], ...(ui.monitors || []).filter(() => c.layout !== 'span' && ui.monitors.length > 1).map((m) => [m.id, monitorName(m)])];
    right = el('div', { class: 'panel stack' },
      el('label', { class: 'stack', style: 'gap:4px' }, el('span', { class: 'muted' }, t('pl.name')),
        el('input', { type: 'text', value: d.name, oninput: (e) => { d.name = e.target.value; } })),
      el('div', { class: 'row' }, el('span', { class: 'muted' }, t('pl.mode')),
        el('div', { class: 'seg' },
          el('button', { class: d.mode === 'interval' ? 'active' : '', onclick: () => { d.mode = 'interval'; rerender(); } }, t('pl.interval')),
          el('button', { class: d.mode === 'time_of_day' ? 'active' : '', onclick: () => { d.mode = 'time_of_day'; d.items.forEach((it, i) => { it.at = it.at || ['07:00', '12:00', '18:00', '21:00'][i % 4]; }); rerender(); } }, t('pl.timeOfDay')))),
      d.mode === 'interval' ? el('div', { class: 'row' },
        el('label', { class: 'row', style: 'gap:8px' }, el('span', {}, t('pl.minutes')),
          el('input', { type: 'number', min: 1, max: 1440, value: d.interval_minutes, style: 'width:90px', oninput: (e) => { d.interval_minutes = Math.max(1, Number(e.target.value) || 15); } })),
        el('label', { class: 'row', style: 'gap:8px' }, el('span', {}, t('pl.shuffle')),
          el('input', { type: 'checkbox', class: 'switch', checked: d.shuffle, onchange: (e) => { d.shuffle = e.target.checked; } })))
        : el('p', { class: 'faint', style: 'font-size:12.5px' }, t('pl.sunNote')),
      el('h3', { style: 'margin:8px 0 0' }, t('pl.items')),
      entries,
      el('button', {
        class: 'btn', onclick: () => pickWallpapers((ids) => {
          ids.forEach((id) => d.items.push(d.mode === 'time_of_day' ? { wallpaper: id, at: '12:00' } : { wallpaper: id }));
          rerender();
        }),
      }, icon('plus'), t('pl.add')),
      el('div', { class: 'row', style: 'margin-top:10px;flex-wrap:wrap' },
        el('div', { class: 'row', style: 'gap:8px' },
          el('button', {
            class: 'btn primary', onclick: async () => {
              const id = await call('save_playlist', { playlist: d });
              plSelected = id; d.id = id; toast(t('pl.saved'));
            },
          }, icon('save'), t('pl.save')),
          d.id ? el('button', { class: 'btn danger', onclick: async () => { await call('delete_playlist', { id: d.id }); plDraft = null; plSelected = null; } }, icon('trash'), t('pl.delete')) : null),
        d.id ? el('label', { class: 'row', style: 'gap:8px' }, el('span', { class: 'muted' }, t('pl.use')),
          el('select', { onchange: (e) => { if (e.target.value) call('set_source', { target: e.target.value, source: { kind: 'playlist', id: d.id } }).then(() => toast('✓')); e.target.value = ''; } },
            el('option', { value: '' }, '…'), ...targets.map(([v, label]) => el('option', { value: v }, label)))) : null),
    );
  }
  view.append(el('section', { class: 'page' },
    el('div', { class: 'page-head' }, el('h1', {}, t('pl.title'))),
    el('div', { class: 'pl-layout' }, left, right)));
}

function pickWallpapers(onDone) {
  const chosen = [];
  modal((close) => [
    el('div', { class: 'row' }, el('h2', {}, t('pl.add')),
      el('button', { class: 'btn primary small', onclick: () => { close(); onDone(chosen); } }, t('common.ok'))),
    el('div', { class: 'picker' }, ...wallpapers.map((w) => el('div', {
      class: 'card', onclick: (e) => {
        const card = e.currentTarget;
        const i = chosen.indexOf(w.id);
        if (i >= 0) chosen.splice(i, 1); else chosen.push(w.id);
        card.classList.toggle('selected', i < 0);
      },
    }, el('div', { class: 'thumb' }, previewNode(w)), el('div', { class: 'card-body' }, el('strong', {}, w.manifest.name))))),
  ]);
}

// ------------------------------------------------------------------ gallery
async function renderGallery(view) {
  const grid = el('div', { class: 'grid' });
  const status = el('p', { class: 'muted' }, t('gal.loading'));
  view.append(el('section', { class: 'page' },
    el('div', { class: 'hero-banner' }, el('h1', {}, el('span', { class: 'grad-text' }, t('gal.title'))), el('p', { class: 'muted' }, t('gal.subtitle'))),
    status, grid));
  if (!cfg().integrations?.gallery) { status.textContent = t('gal.disabled'); return; }
  let index;
  try {
    index = await invoke('gallery_index');
  } catch (e) {
    status.textContent = `${t('gal.offline')} (${e})`;
    return;
  }
  status.remove();
  grid.replaceChildren(...(index.items || []).map((item) => {
    const preview = item.preview ? el('img', { src: `${index.base}${encodeURIComponent(item.id)}/${encodeURIComponent(item.preview)}`, alt: '', loading: 'lazy' })
      : el('div', { class: `placeholder t-${item.type}` }, TYPE_LABEL[item.type] || item.type);
    const btn = el('button', {
      class: 'btn primary small', onclick: async () => {
        btn.disabled = true;
        try {
          const id = await call('gallery_install', { id: item.id });
          toast(t('gal.installed'));
          lib.selected = id;
          await refreshLibrary();
        } finally { btn.disabled = false; }
      },
    }, icon('download'), t('gal.install'));
    return el('div', { class: 'card', style: 'cursor:default' },
      el('div', { class: 'thumb' }, preview, el('div', { class: 'badges' }, el('span'), el('span', { class: 'badge' }, TYPE_LABEL[item.type] || item.type))),
      el('div', { class: 'card-body' }, el('strong', {}, String(item.name || item.id)),
        el('small', {}, String(item.author || ''), item.size ? ` · ${fmtSize(item.size)}` : ''),
        el('p', { class: 'muted', style: 'font-size:12.5px;min-height:38px' }, String(item.description || '')),
        el('div', { class: 'row' }, el('span', { class: 'chip' }, String(item.license || '')), btn)));
  }));
}

// ------------------------------------------------------------------ create & editor
function renderCreate(view) {
  const mine = wallpapers.filter((w) => w.editable);
  const tpl = (kind, ic, title, desc) => {
    const name = el('input', { type: 'text', placeholder: t('cr.name') });
    return el('div', { class: 'tpl' }, icon(ic), el('h2', {}, title), el('p', { class: 'muted' }, desc), name,
      el('button', {
        class: 'btn primary', onclick: async () => {
          const id = await call('create_wallpaper', { template: kind, name: name.value });
          await refreshLibrary();
          openEditor(id);
        },
      }, icon('plus'), t('cr.create')));
  };
  view.append(el('section', { class: 'page' },
    el('div', { class: 'hero-banner' }, el('h1', {}, el('span', { class: 'grad-text' }, t('cr.title'))), el('p', { class: 'muted' }, t('cr.subtitle'))),
    el('div', { class: 'tpl-grid' },
      tpl('shader', 'sparkles', t('cr.shader'), t('cr.shaderDesc')),
      tpl('html', 'code', t('cr.html'), t('cr.htmlDesc')),
      el('div', { class: 'tpl' }, icon('upload'), el('h2', {}, t('lib.import')),
        ...[['media', 'image', 'lib.importMedia'], ['folder', 'folder', 'lib.importFolder'], ['package', 'package', 'lib.importPackage'], ['wallpaper_engine', 'gate', 'lib.importWE']]
          .map(([kind, ic, key]) => el('button', { class: 'btn', onclick: () => doImport(kind) }, icon(ic), t(key))),
        el('p', { class: 'faint', style: 'font-size:12px' }, t('imp.weNote')))),
    el('h3', {}, t('cr.mine')),
    mine.length ? el('div', { class: 'grid' }, ...mine.map((w) => el('div', { class: 'card', onclick: () => openEditor(w.id) },
      el('div', { class: 'thumb' }, previewNode(w)),
      el('div', { class: 'card-body' }, el('strong', {}, w.manifest.name), el('small', {}, icon('edit'), t('cr.edit'))))))
      : el('p', { class: 'muted' }, t('cr.noEditable')),
    el('p', { style: 'margin-top:20px' }, el('a', { href: '#', onclick: (e) => { e.preventDefault(); call('open_url', { url: `${REPO_URL}/blob/main/docs/wallpaper-format.md` }); } }, t('cr.docs'))),
  ));
}

async function openEditor(id) {
  const w = wpById(id);
  if (!w) return;
  const files = await call('editor_files', { id });
  editor = { id, files, current: files.find((f) => f === w.manifest.entry) || files[0], buffers: {}, dirty: new Set(), live: true, timer: null };
  for (const f of files) editor.buffers[f] = await call('editor_read', { id, path: f });
  // Show it on the desktop while editing.
  await call('set_source', { target: 'default', source: { kind: 'wallpaper', id } });
  route = 'create';
  document.querySelectorAll('.nav').forEach((b) => b.classList.toggle('active', b.dataset.route === 'create'));
  renderView();
}

async function saveEditor(path = editor?.current) {
  if (!editor || !editor.dirty.has(path)) return;
  try {
    await invoke('editor_write', { id: editor.id, path, content: editor.buffers[path] });
    editor.dirty.delete(path);
    renderEditorChrome();
    setStatus(t('cr.saved'));
    if (path === 'wallpaper.json') refreshLibrary();
  } catch (e) {
    setStatus(String(e), true);
  }
}

function setStatus(text, isError = false) {
  const s = $('#editor-status');
  if (!s) return;
  s.textContent = text;
  s.classList.toggle('error', isError);
}

function renderEditorChrome() {
  const tabs = $('#editor-tabs');
  if (!tabs || !editor) return;
  tabs.replaceChildren(...editor.files.map((f) => el('button', {
    class: 'tab' + (f === editor.current ? ' active' : '') + (editor.dirty.has(f) ? ' dirty' : ''),
    onclick: () => { editor.current = f; $('#code').value = editor.buffers[f]; renderEditorChrome(); },
  }, f)));
}

function renderEditor(view) {
  const w = wpById(editor.id);
  const code = el('textarea', {
    id: 'code', class: 'code', spellcheck: false, value: editor.buffers[editor.current] || '',
    oninput: (e) => {
      editor.buffers[editor.current] = e.target.value;
      editor.dirty.add(editor.current);
      renderEditorChrome();
      if (editor.live && editor.current !== 'wallpaper.json') {
        clearTimeout(editor.timer);
        editor.timer = setTimeout(() => saveEditor(editor.current), 700);
      }
    },
    onkeydown: (e) => {
      if (e.key === 'Tab') {
        e.preventDefault();
        const ta = e.target, s = ta.selectionStart;
        ta.setRangeText('  ', s, ta.selectionEnd, 'end');
        ta.dispatchEvent(new Event('input'));
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
        e.preventDefault();
        saveEditor();
      }
    },
  });
  view.append(el('div', { class: 'editor' },
    el('div', { class: 'editor-bar' },
      el('button', { class: 'btn ghost small', onclick: () => { editor = null; renderView(); } }, icon('back'), t('cr.back')),
      el('strong', { style: 'font-family:var(--display)' }, w ? w.manifest.name : editor.id),
      el('div', { id: 'editor-tabs', class: 'tabs' }),
      el('div', { class: 'spacer' }),
      el('label', { class: 'row', style: 'gap:8px' }, el('span', { class: 'muted' }, t('cr.live')),
        el('input', { type: 'checkbox', class: 'switch', checked: editor.live, onchange: (e) => { editor.live = e.target.checked; } })),
      el('button', { class: 'btn primary small', onclick: () => saveEditor() }, icon('save'), `${t('cr.save')} (Ctrl+S)`)),
    code,
    el('div', { id: 'editor-status', class: 'editor-status' }, '')));
  renderEditorChrome();
  updateEditorErrors();
}

function updateEditorErrors() {
  if (!editor) return;
  const err = ui?.errors?.[editor.id];
  if (err) setStatus(`${t('cr.error')}: ${err}`, true);
}

// ------------------------------------------------------------------ settings
// Agent access: coding agents (Claude Code, Cursor, Codex…) control Wallgates through MCP.
function agentPanel(c) {
  const cmd = el('code', { class: 'agent-cmd' }, '…');
  call('agent_info').then((i) => { cmd.textContent = `claude mcp add wallgates -- "${i.exe}" --mcp`; }).catch(() => {});
  const copy = el('button', { class: 'btn small', onclick: () => navigator.clipboard.writeText(cmd.textContent).then(() => toast('✓')) }, t('set.agentCopy'));
  return el('div', { class: 'panel' },
    el('h2', {}, t('set.agent')),
    el('p', { class: 'muted', style: 'font-size:13px' }, t('set.agentText')),
    setting(t('set.agentOn'), sw(c.agent_access, (v) => patch({ agent_access: v }))),
    el('div', { class: 'stack', style: 'gap:6px;margin-top:8px' }, el('span', { class: 'muted', style: 'font-size:12.5px' }, t('set.agentHow')), cmd, el('div', { class: 'row' }, copy)));
}

function setting(label, control) {
  return el('label', { class: 'setting' }, el('span', {}, label), control);
}
function sw(checked, onchange) {
  return el('input', { type: 'checkbox', class: 'switch', checked: !!checked, onchange: (e) => onchange(e.target.checked) });
}
function sel(value, options, onchange) {
  return el('select', { onchange: (e) => onchange(e.target.value) },
    ...options.map(([v, label]) => el('option', { value: v, selected: String(v) === String(value) }, label)));
}

async function renderSettings(view) {
  const c = cfg();
  const p = c.performance || {};
  const acts = [['none', t('set.act.none')], ['eco', t('set.act.eco')], ['pause', t('set.act.pause')], ['stop', t('set.act.stop')]];
  const autostart = await invoke('get_autostart').catch(() => false);
  const sensors = await invoke('sensors_status').catch(() => ({ supported: false }));
  const sensorsBox = el('div', { class: 'stack' });
  const drawSensors = (st, busy = false) => {
    const state = !st.supported ? t('sen.unsupported') : st.enabled ? (st.running ? t('sen.active') : t('sen.enabledIdle')) : t('sen.off');
    sensorsBox.replaceChildren(...[
      el('p', { class: 'muted', style: 'font-size:13px' }, t('sen.text')),
      el('div', { class: 'setting' }, el('span', {}, t('sen.status')), el('strong', { style: `color:${st.enabled ? 'var(--ok)' : 'var(--muted)'}` }, state)),
      st.supported ? el('div', { class: 'row', style: 'justify-content:flex-start;gap:8px' },
        st.enabled
          ? el('button', { class: 'btn', disabled: busy, onclick: async () => { drawSensors(st, true); try { const n = await invoke('sensors_disable'); toast(t('sen.disabled')); drawSensors(n); } catch (e) { toast(String(e) === 'cancelled' ? t('sen.cancelled') : String(e), String(e) !== 'cancelled'); drawSensors(st); } } }, t('sen.disable'))
          : el('button', { class: 'btn primary', disabled: busy || !st.helper, onclick: async () => { drawSensors(st, true); try { const n = await invoke('sensors_enable'); toast(t('sen.enabled')); drawSensors(n); } catch (e) { toast(String(e) === 'cancelled' ? t('sen.cancelled') : String(e), String(e) !== 'cancelled'); drawSensors(st); } } }, icon('cpu'), busy ? t('sen.working') : t('sen.enable')),
      ) : null,
      el('p', { class: 'faint', style: 'font-size:12px' }, t('sen.fine')),
    ].filter(Boolean));
  };
  drawSensors(sensors);

  const rulesBox = el('div', { class: 'rules' });
  const drawRules = () => {
    const rules = structuredClone(c.app_rules || []);
    const save = () => patch({ app_rules: rules });
    rulesBox.replaceChildren(...rules.map((r, i) => el('div', { class: 'rule' },
      el('input', { type: 'text', value: r.exe, onchange: (e) => { r.exe = e.target.value.trim(); save(); } }),
      sel(r.action, ['pause', 'stop', 'mute', 'eco'].map((a) => [a, t(`set.rule.${a}`)]), (v) => { r.action = v; save(); }),
      el('button', { class: 'icon-btn', onclick: () => { rules.splice(i, 1); save(); } }, icon('trash')))),
      (() => {
        const exe = el('input', { type: 'text', placeholder: t('set.ruleExe') });
        const act = sel('pause', ['pause', 'stop', 'mute', 'eco'].map((a) => [a, t(`set.rule.${a}`)]), () => {});
        return el('div', { class: 'rule' }, exe, act,
          el('button', { class: 'btn small', onclick: () => { if (!exe.value.trim()) return; rules.push({ exe: exe.value.trim(), action: act.value }); save(); } }, icon('plus')));
      })());
  };
  drawRules();

  const cityResults = el('div', { class: 'results' });
  const cityInput = el('input', { type: 'text', placeholder: t('set.citySearch'), value: c.integrations?.location?.name || '' });
  const searchCity = async () => {
    const q = cityInput.value.trim();
    if (!q) return;
    const results = await call('geocode', { query: q, language: LANG });
    cityResults.replaceChildren(...results.map((r) => el('button', {
      class: 'btn small ghost', onclick: () => { patch({ integrations: { location: r } }); cityInput.value = r.name; cityResults.replaceChildren(); },
    }, r.name)));
  };
  cityInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') searchCity(); });

  const hk = c.hotkeys || {};
  const hkInput = (key) => el('input', { type: 'text', value: hk[key] || '', style: 'width:220px', onchange: (e) => patch({ hotkeys: { [key]: e.target.value.trim() } }) });

  view.append(el('section', { class: 'page' },
    el('div', { class: 'page-head' }, el('h1', {}, t('set.title'))),
    el('div', { class: 'settings-grid' },
      el('div', { class: 'panel' },
        el('h2', {}, t('set.general')),
        setting(t('set.language'), sel(c.language, [['auto', t('set.auto')], ['en', 'English'], ['it', 'Italiano']], (v) => patch({ language: v }))),
        setting(t('set.autostart'), sw(autostart, (v) => call('set_autostart', { enabled: v }))),
        setting(t('set.transition'), sel(c.transition_ms, [[0, t('set.instant')], [400, '0.4 s'], [900, '0.9 s'], [1500, '1.5 s'], [3000, '3 s']], (v) => patch({ transition_ms: Number(v) }))),
        el('h3', {}, t('set.hotkeys')),
        setting(t('set.hotkeysOn'), sw(hk.enabled, (v) => patch({ hotkeys: { enabled: v } }))),
        setting(t('set.hkPause'), hkInput('pause')),
        setting(t('set.hkNext'), hkInput('next')),
        setting(t('set.hkShow'), hkInput('show'))),
      el('div', { class: 'panel' },
        el('h2', {}, t('set.performance')),
        setting(t('set.whenFullscreen'), sel(p.fullscreen, acts, (v) => patch({ performance: { fullscreen: v } }))),
        setting(t('set.whenMaximized'), sel(p.maximized, acts, (v) => patch({ performance: { maximized: v } }))),
        setting(t('set.whenBattery'), sel(p.battery, acts, (v) => patch({ performance: { battery: v } }))),
        setting(t('set.scope'), sel(p.pause_scope, [['monitor', t('set.scope.monitor')], ['all', t('set.scope.all')]], (v) => patch({ performance: { pause_scope: v } }))),
        setting(t('set.fps'), sel(p.fps_limit, [[24, '24'], [30, '30'], [60, '60'], [120, '120'], [144, '144'], [0, t('set.unlimited')]], (v) => patch({ performance: { fps_limit: Number(v) } }))),
        setting(t('set.scale'), sel(p.render_scale, [[1, '100%'], [0.75, '75%'], [0.5, '50%'], [0.35, '35%']], (v) => patch({ performance: { render_scale: Number(v) } }))),
        el('h3', {}, t('set.audio')),
        setting(t('set.muteUnless'), sw(p.mute_unless_desktop, (v) => patch({ performance: { mute_unless_desktop: v } }))),
        el('div', { class: 'setting' }, el('span', {}, t('set.volume')),
          el('input', { type: 'range', min: 0, max: 1, step: 0.05, value: p.volume, style: 'max-width:200px', oninput: throttle((e) => patch({ performance: { volume: Number(e.target.value) } }), 120) }))),
      el('div', { class: 'panel' },
        el('h2', {}, t('sen.title')),
        sensorsBox),
      el('div', { class: 'panel' },
        el('h2', {}, t('set.rules')),
        el('p', { class: 'muted', style: 'font-size:13px' }, t('set.rulesHint')),
        rulesBox),
      el('div', { class: 'panel' },
        el('h2', {}, t('set.weather')),
        setting(t('set.weatherOn'), sw(c.integrations?.weather, (v) => patch({ integrations: { weather: v } }))),
        el('div', { class: 'setting' }, el('span', {}, t('set.city')),
          el('div', { class: 'row', style: 'gap:6px' }, cityInput, el('button', { class: 'btn small', onclick: searchCity }, '🔍'))),
        cityResults,
        setting(t('set.fahrenheit'), sw(c.integrations?.fahrenheit, (v) => patch({ integrations: { fahrenheit: v } }))),
        el('h3', {}, t('set.lock')),
        setting(t('set.syncLock'), sw(c.sync_lock_screen, (v) => patch({ sync_lock_screen: v }))),
        setting(t('set.syncDesktop'), sw(c.sync_desktop, (v) => patch({ sync_desktop: v })))),
      el('div', { class: 'panel' },
        el('h2', {}, t('set.privacy')),
        el('p', { class: 'muted', style: 'font-size:13px' }, t('set.privacyText')),
        setting(t('set.galleryOn'), sw(c.integrations?.gallery, (v) => patch({ integrations: { gallery: v } }))),
        setting(t('set.updates'), sw(c.integrations?.update_check, (v) => patch({ integrations: { update_check: v } }))),
        setting(t('set.marketsOn'), sw(c.integrations?.markets, (v) => patch({ integrations: { markets: v } }))),
        setting(t('set.mature'), sw(c.show_mature, (v) => patch({ show_mature: v })))),
      agentPanel(c),
      el('div', { class: 'panel' },
        el('h2', {}, t('set.about')),
        el('div', { class: 'row', style: 'justify-content:flex-start;gap:14px;margin:8px 0 14px' },
          el('img', { src: 'icon.png', width: 56, height: 56, style: 'border-radius:12px' }),
          el('div', {}, el('strong', { style: 'font-family:var(--display);font-size:18px' }, 'Wallgates'), el('p', { class: 'muted' }, `${t('set.version')} ${ui.version} · GPL-3.0`))),
        el('div', { class: 'actions', style: 'display:flex;flex-wrap:wrap;gap:8px' },
          el('button', { class: 'btn small', onclick: async () => { const u = await call('check_update'); if (!u) toast(t('set.upToDate')); } }, t('set.checkUpdate')),
          el('button', { class: 'btn small', onclick: () => call('open_folder', { which: 'library' }) }, icon('folder'), t('set.openLibrary')),
          el('button', { class: 'btn small', onclick: () => call('open_folder', { which: 'logs' }) }, t('set.openLogs')),
          el('button', { class: 'btn small', onclick: () => call('reapply') }, t('set.reattach')),
          el('button', { class: 'btn small ghost', onclick: () => call('open_url', { url: REPO_URL }) }, t('set.website')),
          el('button', { class: 'btn small primary', onclick: () => call('open_url', { url: REPO_URL }) }, icon('heart'), t('set.sponsor')))),
    )));
}

// ------------------------------------------------------------------ onboarding
function showOnboarding() {
  const root = $('#onboarding');
  root.hidden = false;
  const choice = { autostart: true, games: true };
  let step = 0;
  const dots = () => el('div', { class: 'dots' }, ...[0, 1, 2].map((i) => el('span', { class: i === step ? 'on' : '' })));
  const draw = () => {
    let body;
    if (step === 0) {
      body = [el('img', { class: 'logo', src: 'icon.png', alt: '' }),
        el('h1', {}, el('span', { class: 'grad-text' }, t('onb.welcome'))),
        el('p', { class: 'tag' }, t('onb.tagline')),
        el('div', { class: 'seg', style: 'margin-bottom:22px' },
          ...[['auto', t('set.auto')], ['en', 'English'], ['it', 'Italiano']].map(([v, l]) => el('button', {
            class: cfg().language === v ? 'active' : '',
            onclick: async () => { await patch({ language: v }); setLanguage(v); draw(); },
          }, l))),
        el('div', {}, el('button', { class: 'btn primary', onclick: () => { step = 1; draw(); } }, t('onb.start')))];
    } else if (step === 1) {
      body = [el('h1', {}, t('onb.prefs')),
        el('div', { class: 'choices' },
          el('label', {}, sw(choice.autostart, (v) => { choice.autostart = v; }), t('onb.autostart')),
          el('label', {}, sw(choice.games, (v) => { choice.games = v; }), t('onb.pauseGames'))),
        el('div', {}, el('button', { class: 'btn primary', onclick: () => { step = 2; draw(); } }, t('onb.start')))];
    } else {
      const picks = wallpapers.filter((w) => w.source === 'builtin').slice(0, 6);
      body = [el('h1', {}, t('onb.pick')),
        el('div', { class: 'picker' }, ...picks.map((w) => el('div', {
          class: 'card' + (sourceOf('default')?.id === w.id ? ' selected' : ''),
          onclick: async (e) => {
            await call('set_source', { target: 'default', source: { kind: 'wallpaper', id: w.id } });
            root.querySelectorAll('.card').forEach((c) => c.classList.toggle('selected', c === e.currentTarget));
          },
        }, el('div', { class: 'thumb' }, previewNode(w)), el('div', { class: 'card-body' }, el('strong', {}, w.manifest.name))))),
        el('p', { class: 'faint', style: 'margin-bottom:18px' }, t('onb.importWE')),
        el('button', {
          class: 'btn primary', onclick: async () => {
            await call('set_autostart', { enabled: choice.autostart }).catch(() => {});
            await patch({ onboarding_done: true, performance: { fullscreen: choice.games ? 'pause' : 'none' } });
            root.hidden = true;
            lib.selected = null;
            renderView();
          },
        }, t('onb.done'))];
    }
    root.replaceChildren(el('div', { class: 'onb' }, ...body, dots()));
  };
  draw();
}

// ------------------------------------------------------------------ drag & drop
try {
  window.__TAURI__.webview.getCurrentWebview().onDragDropEvent(async (event) => {
    const p = event.payload;
    if (p.type === 'enter' || p.type === 'over') $('#drop-overlay').hidden = false;
    else if (p.type === 'leave') $('#drop-overlay').hidden = true;
    else if (p.type === 'drop') {
      $('#drop-overlay').hidden = true;
      if (p.paths?.length) importResult(await call('import_paths', { paths: p.paths }));
    }
  });
} catch (e) { console.warn('drag & drop unavailable', e); }

// ------------------------------------------------------------------ boot
let lastLang = null;
function onUi(next) {
  const prev = ui;
  ui = next;
  if (cfg().language !== lastLang) { lastLang = cfg().language; setLanguage(lastLang); if (prev) renderView(); }
  renderStatus();
  if (!prev) return;
  // Light-touch updates: never rebuild controls the user may be dragging.
  if (route === 'library') {
    renderGrid();
    const libChanged = JSON.stringify(prev.config.favorites) !== JSON.stringify(ui.config.favorites);
    if (libChanged) renderGrid();
  } else if (route === 'displays' || (route === 'playlists' && JSON.stringify(prev.config.playlists) !== JSON.stringify(ui.config.playlists))) {
    if (route === 'playlists') plDraft = plSelected ? structuredClone((ui.config.playlists || []).find((p) => p.id === plSelected) || newPlaylist()) : plDraft;
    renderView();
  } else if (route === 'create' && editor) {
    updateEditorErrors();
  } else if (route === 'settings' && JSON.stringify(prev.config.app_rules) !== JSON.stringify(ui.config.app_rules)) {
    renderView();
  }
}

(async () => {
  listen('wf-ui', (e) => onUi(e.payload));
  window.addEventListener('focus', () => refreshLibrary().catch(() => {}));
  onUi(await call('ui_state'));
  wallpapers = await call('list_wallpapers');
  go('library');
  if (!cfg().onboarding_done) showOnboarding();
})();
