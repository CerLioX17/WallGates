#!/usr/bin/env node
// Wallgates headless preview: renders wallpapers exactly like the app (real host.html + SDK) with mock live data,
// saves a PNG and prints any JavaScript / shader errors. Made for humans and coding agents.
//
//   node tools/preview/render.js <wallpaper> [more…] [--out dir] [--width 1920] [--height 1080] [--wait ms]
//        [--values '{"speed":2}'] [--values-file values.json] [--markets markets.json] [--user]
//
// <wallpaper> is a folder name in wallpapers/ (or in the user library with --user, or any folder via --dir path).
// Needs Playwright with Chromium once:  npx -y playwright@1 install chromium   (then: npm i --no-save playwright)
let chromium;
try { ({ chromium } = require('playwright')); } catch { console.error('Playwright is missing. Run: npm i --no-save playwright && npx playwright install chromium'); process.exit(2); }
const fs = require('fs'), path = require('path');
const ROOT = path.resolve(__dirname, '../..');
const argv = process.argv.slice(2), opt = {}, names = [];
for (let i = 0; i < argv.length; i++) { if (argv[i].startsWith('--')) { const k = argv[i].slice(2); opt[k] = argv[i + 1] && !argv[i + 1].startsWith('--') ? argv[++i] : true; } else names.push(argv[i]); }
if (!names.length) { console.error('usage: node tools/preview/render.js <wallpaper> [--out dir] [--width W] [--height H] [--wait ms] [--values JSON] [--markets file]'); process.exit(1); }
const userLib = () => { const id = 'app.wallgates.desktop'; return process.platform === 'win32' ? path.join(process.env.APPDATA || '', id, 'wallpapers') : process.platform === 'darwin' ? path.join(process.env.HOME, 'Library/Application Support', id, 'wallpapers') : path.join(process.env.XDG_DATA_HOME || path.join(process.env.HOME, '.local/share'), id, 'wallpapers'); };
const WPDIR = opt.dir ? path.resolve(opt.dir) : opt.user ? userLib() : path.join(ROOT, 'wallpapers');
const OUT = path.resolve(opt.out || 'preview-out');
const VW = Number(opt.width) || 1920, VH = Number(opt.height) || 1080;
const OVERRIDES = opt['values-file'] ? JSON.parse(fs.readFileSync(opt['values-file'], 'utf8')) : JSON.parse(opt.values || '{}');
fs.mkdirSync(OUT, { recursive: true });
const MIME = { html: 'text/html', js: 'text/javascript', css: 'text/css', frag: 'text/plain', json: 'application/json', png: 'image/png', jpg: 'image/jpeg', woff2: 'font/woff2' };
const info = (f) => ({ id: (opt.user ? 'user/' : 'builtin/') + f, source: opt.user ? 'user' : 'builtin', editable: !!opt.user, size_bytes: 0, manifest: JSON.parse(fs.readFileSync(path.join(WPDIR, f, 'wallpaper.json'))) });
const defaults = (m, over = {}) => Object.fromEntries(Object.entries(m.properties || {}).map(([k, d]) => [k, k in over ? over[k] : d.default]));
const serveFile = (route, file) => fs.existsSync(file)
  ? route.fulfill({ status: 200, body: fs.readFileSync(file), headers: { 'content-type': MIME[path.extname(file).slice(1)] || 'application/octet-stream', 'access-control-allow-origin': '*' } })
  : route.fulfill({ status: 404, body: '' });

const MOCK = require('./mockfeeds.js');
const FEEDS = {
  markets: opt.markets ? JSON.parse(fs.readFileSync(opt.markets)) : MOCK.markets,
  weather: MOCK.weather(),
  media: { title: 'Midnight City', artist: 'M83', album: "Hurry Up, We're Dreaming", playing: true, thumbnail: null },
  system: MOCK.system(0),
};

(async () => {
  const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--use-angle=swiftshader', '--ignore-gpu-blocklist'] });
  const results = {};
  const list = names;
  for (const f of list) {
    const w = info(f);
    const state = { label: 'wp-0-0', monitor_id: 'm', index: 0, span: false, wallpaper: w, values: defaults(w.manifest, OVERRIDES), paused: false, stopped: false, eco: false, muted: true, volume: 1, fps_limit: 60, render_scale: 1, transition_ms: 0, revision: 0, reason: null, width: 1600, height: 900 };
    const ctx = await browser.newContext({ viewport: { width: VW, height: VH }, userAgent: 'Mozilla/5.0 (Windows NT 10.0) Headless' });
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
    await page.route('http://tauri.localhost/**', (r) => serveFile(r, path.join(ROOT, 'ui', decodeURIComponent(new URL(r.request().url()).pathname))));
    await page.route('http://wp.localhost/**', (r) => {
      const p = decodeURIComponent(new URL(r.request().url()).pathname);
      if (p.startsWith('/__wallgates/')) return serveFile(r, path.join(ROOT, 'ui/sdk', p.slice(13)));
      return serveFile(r, path.join(WPDIR, p.replace(/^\/(builtin|user)\//, '')));
    });
    await page.addInitScript(`
      window.__l = {}; window.__reported = [];
      window.__TAURI__ = {
        core: { invoke: async (cmd, a) => {
          if (cmd === 'host_state') return ${JSON.stringify(state)};
          if (cmd === 'feed_snapshot') return ${JSON.stringify(FEEDS)};
          if (cmd === 'report_error') { window.__reported.push(a.message); return; }
          if (cmd === 'submit_frame') { window.__frame = a.data.length; return; }
          throw new Error('unexpected ' + cmd); } },
        event: { listen: async (name, cb) => { (window.__l[name] ||= []).push(cb); } },
      };
      window.__emit = (name, payload) => (window.__l[name] || []).forEach((cb) => cb({ payload }));
      // Fake music: bass-heavy spectrum with a beat.
      setInterval(() => {
        const t = performance.now() / 1000, beat = Math.max(0, Math.sin(t * 7.5)) ** 4;
        const bands = Array.from({ length: 64 }, (_, i) => Math.max(0, 0.85 - i / 80 + 0.25 * Math.sin(i * 0.7 + t * 4) * 0.5 + (i < 8 ? beat * 0.4 : 0)));
        window.__emit('wf-audio', { bands, level: 0.35 + beat * 0.3 });
        window.__emit('wf-mouse', { x: 0.62, y: 0.4, inside: true, desktop: true });
      }, 33);
      window.__sysT = 0;
      setInterval(() => { window.__sysT += 0.6; window.__emit('wf-feed', { kind: 'system', data: (${MOCK.system.toString()})(window.__sysT) }); }, 400);
    `);
    await page.goto('http://tauri.localhost/host.html');
    await page.waitForTimeout(Number(opt.wait) || (f.includes('terminal') ? 14000 : 5000));
    await page.screenshot({ path: path.join(OUT, `${f}.png`) });
    // Capture for lock-screen sync.
    await page.evaluate(() => window.__emit('wf-capture', null));
    await page.waitForTimeout(800);
    results[f] = { image: path.join(OUT, `${f}.png`), errors, reported: await page.evaluate(() => window.__reported.filter(Boolean)), capturedBytes: await page.evaluate(() => window.__frame || 0), errorBox: await page.textContent('#error') };
    await ctx.close();
  }
  console.log(JSON.stringify(results, null, 1));
  await browser.close();
})();
