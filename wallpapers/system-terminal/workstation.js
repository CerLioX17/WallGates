// System Terminal — Workstation style (CC BY 4.0). Full-screen monitoring console with live event log.
(function () {
  'use strict';
  const $ = (id) => document.getElementById(id);
  const lang = navigator.language || 'en-US';
  let o = {}, s = null, prev = null, w = null, media = null, booted = false, booting = false, built = false;
  const log = [], last = {};
  let audio = null, smooth = new Array(32).fill(0), bass = 0, treble = 0;

  const PALETTES = {
    wallgates: { p: '#a78bfa', a: '#22d3ee', t: '#dcd8ff', bg1: '#06050d', bg2: '#0f0b20' },
    amber: { p: '#ffaa66', a: '#ffcc88', t: '#ffd9b0', bg1: '#070301', bg2: '#140a05' },
    green: { p: '#33ff77', a: '#b6ffcb', t: '#c8ffd8', bg1: '#010602', bg2: '#04140a' },
    cyan: { p: '#22d3ee', a: '#a5f3fc', t: '#d5f8ff', bg1: '#01060a', bg2: '#051520' },
    white: { p: '#cfd3dc', a: '#ffffff', t: '#e6e8ee', bg1: '#060607', bg2: '#141518' },
  };
  const LOGO = [
    '            ▄▄▄██████▄▄▄',
    '         ▄██▀▀ .:░░:. ▀▀██▄',
    '       ▄█▀   .:░▒▓▓▒░:.   ▀█▄',
    '      ██   .:░▒▓▓▓▓▓▓▒░:.   ██',
    '     ██  .:░▒▓▓▓▓▓▓▓▓▓▓▒░:.  ██',
    '     ██  :░▒▓▓▓▓▓▓▓▓▓▓▓▓▒░:  ██',
    '     ██  ~~~~~~~~~~~~~~~~~~  ██',
    '     ██  ░▒▒▓▓▓▓▓▓▓▓▓▓▓▓▒▒░  ██',
    '     ██  ~ ~~ ~~~ ~~~ ~~ ~   ██',
    '     ██  ░░▒▒▒▒▒▒▒▒▒▒▒▒▒▒░░  ██',
    '  ▀▀▀██▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀██▀▀▀',
    '',
    '     W A L L G A T E S',
    '     system monitor · open source',
  ];

  // ------------------------------------------------------------ helpers
  const esc = (t) => String(t ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  const bytes = (b) => { if (b == null || !isFinite(b)) return '—'; const u = ['B', 'KB', 'MB', 'GB', 'TB']; let i = 0; while (b >= 1024 && i < 4) { b /= 1024; i++; } return `${b.toFixed(b < 10 && i ? 1 : 0)} ${u[i]}`; };
  const rate = (b) => (b == null ? '—' : bytes(b) + '/s');
  const deg = (c) => (c == null ? '—' : o.fahrenheit ? `${Math.round(c * 9 / 5 + 32)}°F` : `${Math.round(c)}°C`);
  const pct = (v, d = 0) => (v == null ? '—' : `${v.toFixed(d)}%`);
  const lv = (v, warn, bad) => (v == null ? '' : v >= bad ? 'bad' : v >= warn ? 'warn' : 'ok');
  const tagFor = (l) => (l === 'bad' ? '<span class="tag bad-t">CRITICAL</span>' : l === 'warn' ? '<span class="tag warn-t">CAUTION</span>' : l === 'ok' ? '<span class="tag ok">NOMINAL</span>' : '<span class="tag dim">N/A</span>');
  const clock = (d, sec = true) => d.toLocaleTimeString(lang, { hour: '2-digit', minute: '2-digit', ...(sec ? { second: '2-digit' } : {}), hour12: !o.format24 });
  const clean = (n) => String(n || '').replace(/\.exe$/i, '');
  const cpuName = () => String(s?.cpuName || 'CPU').replace(/\(R\)|\(TM\)|CPU|Processor|\d+-Core/gi, '').replace(/\s+/g, ' ').trim();
  const T = () => ({ cpu: o.cpuTempMax || 85, gpu: o.gpuTempMax || 83, ssd: o.ssdTempMax || 65, load: o.loadMax || 92 });
  const row = (label, value, opts = {}) => `<div class="ws-row ${opts.cls ? 'lv-' + opts.cls : ''}"${opts.p != null ? ` style="--p:${Math.max(0, Math.min(100, opts.p)).toFixed(1)}"` : ''}><span>${label}</span><span class="v">${value}</span></div>`;
  const uptime = (u) => `${Math.floor(u / 86400)}d ${String(Math.floor(u % 86400 / 3600)).padStart(2, '0')}:${String(Math.floor(u % 3600 / 60)).padStart(2, '0')}`;
  const sensorsName = () => ({ 'wallgates-sensors': 'ADVANCED', hwinfo: 'HWiNFO', 'gpu-driver': 'DRIVER', system: 'SYSTEM' }[s?.sources?.temperatures] || 'BASIC');

  // ------------------------------------------------------------ markup
  function build() {
    if (built) return; built = true;
    const root = document.createElement('div');
    root.id = 'ws';
    root.innerHTML = `
      <header class="ws-head"><div class="l"><span class="mark">◢◤</span><span>WALLGATES SYSTEM MONITOR <span class="dim">v4.1</span></span></div><div class="c" id="wsClock"></div><div class="r" id="wsUser"></div></header>
      <section class="ws-col">
        <div class="ws-sec" id="wsSysSec"><div class="ws-title">SYSTEMS <small id="wsSysSub"></small></div><div id="wsSys"></div></div>
        <div class="ws-sec" id="wsCoreSec"><div class="ws-title">CPU CORES <small id="wsCoreSub"></small></div><div class="ws-cells" id="wsCores"></div></div>
        <div class="ws-sec" id="wsVolSec"><div class="ws-title">STORAGE</div><div id="wsVols"></div></div>
        <div class="ws-sec" id="wsHwSec"><div class="ws-title">HARDWARE</div><div id="wsHw"></div></div>
      </section>
      <section class="ws-col ws-mid">
        <div class="ws-top">
          <div class="ws-sec" id="wsProcSec"><div class="ws-title">TOP PROCESSES <small id="wsProcSub"></small></div><div id="wsProcs"></div></div>
          <div class="ws-sec" id="wsWxSec"><div class="ws-title">WEATHER <small id="wsWxSub"></small></div><div id="wsWx"></div></div>
        </div>
        <div class="ws-fc" id="wsFcSec"><div class="ws-title">FORECAST <small>next 12 hours · 4 days</small></div><div class="ws-cells ws-hours" id="wsHours"></div><div class="ws-cells" id="wsDays" style="margin-top:0.45em"></div></div>
        <div class="ws-logwrap" id="wsLogSec"><div class="ws-title">EVENT LOG <small id="wsLogSub">live</small></div><div class="ws-log" id="wsLog"></div></div>
      </section>
      <section class="ws-col">
        <div class="ws-sec" id="wsStatSec"><div class="ws-title">STATISTICS</div><div id="wsStats"></div></div>
        <div class="ws-sec" id="wsFanSec"><div class="ws-title">FANS</div><div id="wsFans"></div></div>
        <div class="ws-sec" id="wsAlertSec"><div class="ws-title">ALERTS <small id="wsAlertSub"></small></div><div id="wsAlerts"></div></div>
        <div class="ws-sec ws-audio" id="wsAudioSec"><div class="ws-title">AUDIO ANALYSIS</div><canvas id="wsViz" width="320" height="90"></canvas>
          <div class="ws-astats"><div><span>Bass</span><span id="wsBass">0%</span></div><div><span>Treble</span><span id="wsTreble">0%</span></div></div></div>
        <div class="ws-media" id="wsMedia"></div>
      </section>
      <footer class="ws-foot"><div class="l" id="wsStatus"></div><div class="c" id="wsDate"></div><div class="r" id="wsFoot"></div></footer>`;
    const boot = document.createElement('div');
    boot.id = 'wsBoot';
    boot.innerHTML = '<pre id="wsBootTxt"></pre>';
    const scan = document.createElement('div');
    scan.id = 'wsScan';
    document.body.append(root, boot, scan);
  }

  // ------------------------------------------------------------ look
  function applyLook() {
    const on = o.style !== 'classic';
    document.body.classList.toggle('ws', on);
    if (!on) return;
    build();
    const pal = o.wsPalette === 'custom'
      ? { p: o.primaryColor, a: o.accentColor, t: o.textColor, bg1: o.bgColor1, bg2: o.bgColor2 }
      : PALETTES[o.wsPalette] || PALETTES.wallgates;
    const r = document.documentElement.style;
    r.setProperty('--ws-p', pal.p || '#a78bfa'); r.setProperty('--ws-a', pal.a || '#22d3ee'); r.setProperty('--ws-t', pal.t || '#dcd8ff');
    r.setProperty('--ws-bg1', pal.bg1 || '#06050d'); r.setProperty('--ws-bg2', pal.bg2 || '#0f0b20');
    r.setProperty('--ws-rot', `${o.gradientRotation ?? 135}deg`);
    r.setProperty('--ws-glow', o.glow ?? 0.5);
    r.setProperty('--scan-op', o.scanlineOpacity ?? 0.15);
    r.setProperty('--scan-speed', o.scanlineSpeed ?? 1);
    const size = Math.max(50, Math.min(100, o.wsSize ?? 100));
    r.setProperty('--ws-w', `${size}vw`); r.setProperty('--ws-h', `${size}vh`);
    document.body.classList.toggle('scan', !!o.scanlines);
    document.body.classList.toggle('paused', !!Wallgates.paused);
    $('ws').classList.toggle('no-head', o.showHeader === false);
    if (!booted && !booting) startBoot();
    render();
  }

  // ------------------------------------------------------------ boot sequence
  async function startBoot() {
    booting = true;
    if (o.boot === false) { finishBoot(); return; }
    document.body.classList.add('booting');
    const out = $('wsBootTxt');
    const wait = (ms) => new Promise((r) => setTimeout(r, ms));
    const sp = () => Math.max(40, o.bootSpeed ?? 160);
    out.innerHTML = '';
    for (const line of LOGO) { out.innerHTML += `<span class="logo">${esc(line)}</span>\n`; await wait(45); }
    await wait(900);
    for (let i = 0; i < 30 && !s; i++) await wait(100); // give the system feed a moment
    const g = s?.gpu || {};
    const ok = (t) => `<span class="ok">${t}</span>`;
    const lines = [
      `<span class="pri">WALLGATES SYSTEM MONITOR v4.1</span>`,
      `<span class="dim">open source · CC BY 4.0 · github.com/CerLioX17/WallGates</span>`, '',
      'Initializing core services...',
      `Loading telemetry modules... [<span class="acc">████████████████████</span>] 100%`,
      `Detecting processor....  ${esc(cpuName())} · ${(s?.cores || []).length || '?'} threads   ${ok('OK')}`,
      `Detecting graphics.....  ${esc(g.name || 'unknown')}${g.vramTotal ? ' · ' + bytes(g.vramTotal) : ''}   ${g.name ? ok('OK') : '<span class="warn-t">LIMITED</span>'}`,
      `Detecting memory.......  ${bytes(s?.memoryTotal)}   ${ok('OK')}`,
      `Detecting storage......  ${(s?.drives || []).length || (s?.disks || []).length} device(s)   ${ok('OK')}`,
      `Reading sensors........  ${sensorsName()}   ${s?.cpuTemp != null ? ok('OK') : '<span class="warn-t">PARTIAL</span>'}`,
      `Weather uplink.........  ${w ? esc(String(w.location || '').split(',')[0]) + '   ' + ok('ESTABLISHED') : '<span class="warn-t">OFFLINE</span>'}`, '',
      `Welcome, <span class="acc">${esc((o.operator || 'Operator').toUpperCase())}</span>.`,
      `Host: ${esc(s?.host || 'wallgates')} · ${esc(String(s?.os || '').replace(/\s*\(.*\)/, ''))}`, '',
      'System ready. Switching to workstation interface...',
    ];
    out.innerHTML = '';
    for (const l of lines) { out.innerHTML += l + '\n'; await wait(sp() + Math.random() * sp()); }
    await wait(1200);
    finishBoot();
  }
  function finishBoot() {
    booted = true; booting = false;
    document.body.classList.remove('booting');
    addLog('Workstation interface loaded — monitoring active', 'acc');
    if (s) {
      addLog(`${cpuName()} · ${(s.cores || []).length} threads · ${bytes(s.memoryTotal)} RAM${s.gpu?.name ? ' · ' + s.gpu.name : ''}`);
      addLog(`Sensors online · source ${sensorsName()}${s.cpuTemp == null ? ' · CPU temperature unavailable' : ''}`, s.cpuTemp == null ? 'warn-t' : 'ok');
      addLog(`Storage · ${(s.drives || []).length} drive(s) · ${(s.disks || []).length} volume(s) mounted`);
      addLog(`${s.processCount ?? '?'} processes running · uptime ${uptime(s.uptime || 0)}`);
    }
    if (w) addLog(`Weather uplink · ${String(w.location || '').split(',')[0]} ${Math.round(w.temperature)}° ${String(w.description || '').toLowerCase()}`, 'info');
    render();
  }

  // ------------------------------------------------------------ event log
  function addLog(msg, cls = '') {
    log.push({ t: new Date(), msg, cls });
    const max = Math.max(10, o.logLines ?? 60);
    while (log.length > max) log.shift();
    paintLog();
  }
  function paintLog() {
    const el = $('wsLog'); if (!el) return;
    el.innerHTML = log.map((e) => `<div><span class="ts">[${clock(e.t)}]</span> <span class="${e.cls}">${esc(e.msg)}</span></div>`).join('');
  }
  const throttle = (key, ms) => { const now = Date.now(); if (last[key] && now - last[key] < ms) return false; last[key] = now; return true; };
  const state = {};
  function edge(key, level, onMsg, offMsg) { // log only when a level changes
    if (state[key] === level) return; const before = state[key]; state[key] = level;
    if (before === undefined && level === 'ok') return;
    if (level === 'ok') { if (offMsg) addLog(offMsg, 'ok'); } else addLog(onMsg(level), level === 'bad' ? 'bad-t' : 'warn-t');
  }

  function detectEvents() {
    if (!booted || !s) return;
    const t = T(), g = s.gpu || {};
    edge('cpuT', lv(s.cpuTemp, t.cpu - 10, t.cpu) || 'ok', (l) => `CPU temperature ${deg(s.cpuTemp)} — ${l === 'bad' ? 'CRITICAL' : 'elevated'}`, 'CPU temperature back to normal');
    edge('gpuT', lv(g.temp, t.gpu - 10, t.gpu) || 'ok', (l) => `GPU temperature ${deg(g.temp)} — ${l === 'bad' ? 'CRITICAL' : 'elevated'}`, 'GPU temperature back to normal');
    edge('cpuL', lv(s.cpu, t.load - 15, t.load) || 'ok', () => `CPU load high: ${pct(s.cpu, 1)}`, 'CPU load back to normal');
    edge('gpuL', lv(g.usage, t.load - 15, t.load) || 'ok', () => `GPU load high: ${pct(g.usage, 1)}`, 'GPU load back to normal');
    edge('ram', lv(s.memory, 85, 93) || 'ok', () => `Memory pressure: ${pct(s.memory, 1)} used, ${bytes(s.memoryAvailable)} free`, 'Memory pressure relieved');
    (s.drives || []).forEach((d, i) => {
      const tag = (d.letters || '').trim() || `disk ${i}`;
      edge(`ssdT${i}`, lv(d.temp, t.ssd - 8, t.ssd) || 'ok', () => `${tag} temperature ${deg(d.temp)}`, `${tag} temperature back to normal`);
      const io = (d.read || 0) + (d.write || 0);
      if (io > 150 * 1048576 && throttle(`io${i}`, 30000)) addLog(`${tag} heavy I/O · read ${rate(d.read)} · write ${rate(d.write)}`, 'info');
    });
    if ((s.net?.rx || 0) > 5 * 1048576 && throttle('rx', 30000)) addLog(`Network burst ▼ ${rate(s.net.rx)}`, 'info');
    if ((s.net?.tx || 0) > 3 * 1048576 && throttle('tx', 30000)) addLog(`Upload burst ▲ ${rate(s.net.tx)}`, 'info');
    const top = s.processes?.[0];
    if (top && prev?.processes?.[0] && clean(top.name) !== clean(prev.processes[0].name) && top.cpu > 3 && throttle('top', 20000)) addLog(`Top process now ${clean(top.name)} (${top.cpu.toFixed(1)}% CPU)`);
    if (prev && s.processCount && prev.processCount && Math.abs(s.processCount - prev.processCount) >= 8 && throttle('pc', 15000)) {
      const d = s.processCount - prev.processCount; addLog(`${s.processCount} processes running (${d > 0 ? '+' : ''}${d})`);
    }
    // Periodic health reports keep the log alive.
    if (throttle('beat', Math.max(3, o.logInterval ?? 8) * 1000)) {
      const k = (state.beatN = ((state.beatN || 0) + 1) % 5);
      const vols = (s.disks || []).slice(0, 3).map((v) => `${String(v.mount).replace(/\\$/, '')} ${v.total ? ((v.used / v.total) * 100).toFixed(0) : '?'}%`).join(' · ');
      const msg = [
        `Health check · CPU ${pct(s.cpu)} ${deg(s.cpuTemp)} · GPU ${pct(g.usage)} ${deg(g.temp)} · RAM ${pct(s.memory)}`,
        `Storage check · ${vols || 'no volumes'}`,
        `Network · ▼ ${rate(s.net?.rx)} · ▲ ${rate(s.net?.tx)}`,
        `Uptime ${uptime(s.uptime || 0)} · ${s.processCount ?? '?'} processes · ${s.cpuFreq ? (s.cpuFreq / 1000).toFixed(2) + ' GHz' : ''}`,
        w ? `Weather · ${String(w.location || '').split(',')[0]} ${Math.round(w.temperature)}° ${String(w.description || '').toLowerCase()} · UV ${w.uv != null ? Math.round(w.uv) : '—'}` : `Sensors · source ${sensorsName()}`,
      ][k];
      if (msg) addLog(msg);
    }
  }

  // ------------------------------------------------------------ panels
  function sysPanel() {
    const t = T(), g = s.gpu || {};
    const vram = g.vramTotal ? (g.vramUsed / g.vramTotal) * 100 : null;
    const L = [];
    const cl = lv(s.cpuTemp, t.cpu - 10, t.cpu) || lv(s.cpu, t.load - 15, t.load) || 'ok';
    L.push(row('CPU', `${pct(s.cpu, 1)}<b>${deg(s.cpuTemp)}</b>${tagFor(cl)}`, { p: s.cpu, cls: cl }));
    const gl = g.name ? lv(g.temp, t.gpu - 10, t.gpu) || lv(g.usage, t.load - 15, t.load) || 'ok' : '';
    L.push(row('GPU', `${pct(g.usage, 1)}<b>${deg(g.temp)}</b>${tagFor(gl)}`, { p: g.usage, cls: gl }));
    L.push(row('VRAM', `${bytes(g.vramUsed)}<b>/ ${bytes(g.vramTotal)}</b>`, { p: vram }));
    const ml = lv(s.memory, 85, 93) || 'ok';
    L.push(row('Memory', `${pct(s.memory, 1)}<b>${s.ramTemp == null ? 'no sensor' : deg(s.ramTemp)}</b>${tagFor(ml)}`, { p: s.memory, cls: ml }));
    (s.drives || []).slice(0, 3).forEach((d, i) => {
      const tag = (d.letters || '').trim() || `Disk ${d.index ?? i}`;
      const l = lv(d.temp, t.ssd - 8, t.ssd) || 'ok';
      L.push(row(`SSD ${tag}`, `${pct(d.active, 0)}<b>${deg(d.temp)}</b>${tagFor(l)}`, { p: d.active, cls: l }));
    });
    L.push(row('Network', `▼ ${rate(s.net?.rx)}<b>▲ ${rate(s.net?.tx)}</b>`));
    L.push(row('Sensors', `<span class="${s.cpuTemp != null ? 'ok' : 'warn-t'} ${s.cpuTemp != null ? '' : 'blink'}">${sensorsName()}</span>`));
    $('wsSys').innerHTML = L.join('');
    $('wsSysSub').textContent = s.host || '';
  }

  function corePanel() {
    const cores = s.cores || [];
    const cols = cores.length > 24 ? 8 : cores.length > 16 ? 6 : 4;
    const el = $('wsCores');
    el.style.gridTemplateColumns = `repeat(${cols}, minmax(0, 1fr))`;
    const t = T();
    el.innerHTML = cores.map((v, i) => `<div class="ws-cell ${v >= t.load ? 'hot' : v >= 50 ? 'on' : ''}"><div class="id">C${String(i).padStart(2, '0')}</div><div class="val">${Math.round(v)}%</div><div class="bar" style="width:${Math.min(100, v).toFixed(0)}%"></div></div>`).join('');
    $('wsCoreSub').textContent = `${cores.length} threads · ${s.cpuFreq ? (s.cpuFreq / 1000).toFixed(2) + ' GHz' : ''}`;
  }

  function volPanel() {
    const vols = (s.disks || []).slice(0, 5);
    $('wsVols').innerHTML = vols.map((v) => {
      const p = v.total ? (v.used / v.total) * 100 : null;
      return row(esc(String(v.mount).replace(/\\$/, '')), `${pct(p, 1)}<b>${bytes(v.used)} / ${bytes(v.total)}</b>`, { p, cls: lv(p, 85, 95) });
    }).join('') || '<div class="dim">no volumes</div>';
  }

  function hwPanel() {
    const g = s.gpu || {};
    const L = [row('Processor', esc(cpuName())), row('Graphics', esc(String(g.name || '—').replace(/NVIDIA |AMD |\(R\)|\(TM\)/g, '')))];
    L.push(row('Memory', `${bytes(s.memoryTotal)}<b>${s.ramTemp == null ? '' : deg(s.ramTemp)}</b>`));
    (s.drives || []).slice(0, 3).forEach((d, i) => L.push(row(esc((d.letters || '').trim() || 'Disk ' + i), esc(d.model || '—'))));
    L.push(row('System', esc(String(s.os || '—').replace(/\s*\(.*\)/, ''))));
    $('wsHw').innerHTML = L.join('');
  }

  function procPanel() {
    const procs = (s.processes || []).slice(0, 5);
    const top = Math.max(1, ...procs.map((p) => p.cpu));
    $('wsProcs').innerHTML = `<table class="ws-tab"><colgroup><col style="width:1.8em"><col><col style="width:28%"><col style="width:4.6em"><col style="width:5.4em"></colgroup>
      <tr><th>#</th><th>PROCESS</th><th>LOAD</th><th class="n">CPU</th><th class="n">MEMORY</th></tr>
      ${procs.map((p, i) => `<tr><td class="num">${i + 1}</td><td>${esc(clean(p.name))}${p.count > 1 ? ` <span class="dim">×${p.count}</span>` : ''}</td>
        <td><span class="bar" style="width:${Math.max(2, (p.cpu / top) * 100).toFixed(0)}%"></span></td><td class="n acc">${p.cpu.toFixed(1)}%</td><td class="n">${bytes(p.memory)}</td></tr>`).join('')}</table>`;
    $('wsProcSub').textContent = `${s.processCount ?? '—'} running`;
  }

  function wxPanel() {
    if (!w) {
      $('wsWx').innerHTML = '<div class="dim">Weather offline.<br>Wallgates → Settings → Weather &amp; location.</div>';
      $('wsHours').innerHTML = ''; $('wsDays').innerHTML = ''; $('wsWxSub').textContent = '';
      return;
    }
    const uvTxt = (u) => (u == null ? '—' : u < 3 ? 'LOW' : u < 6 ? 'MODERATE' : u < 8 ? 'HIGH' : u < 11 ? 'VERY HIGH' : 'EXTREME');
    const uvCls = (u) => (u == null ? '' : u >= 8 ? 'bad-t' : u >= 6 ? 'warn-t' : 'ok');
    const aqTxt = (a) => (a == null ? '—' : a <= 20 ? 'GOOD' : a <= 40 ? 'FAIR' : a <= 60 ? 'MODERATE' : a <= 80 ? 'POOR' : 'VERY POOR');
    const dirs = ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'];
    const today = (w.days || [])[0] || {};
    const hm = (iso) => (iso ? clock(new Date(iso), false) : '—');
    $('wsWxSub').textContent = String(w.location || '').split(',')[0];
    $('wsWx').innerHTML = `
      <div class="ws-wxnow"><span class="big">${Math.round(w.temperature)}°</span><span><span class="desc">${esc(w.description || w.condition || '')}</span><br><span class="dim">H ${Math.round(w.high)}° · L ${Math.round(w.low)}° · feels ${Math.round(w.feelsLike)}°</span></span></div>
      <div class="ws-kv">
        ${row('Humidity', `${Math.round(w.humidity)}%`)}${row('Wind', `${Math.round(w.wind)} ${esc(w.windUnits || '')} ${w.windDir != null ? dirs[Math.round(w.windDir / 45) % 8] : ''}`)}
        ${row('UV', `<span class="${uvCls(w.uv)}">${w.uv != null ? Math.round(w.uv) : '—'} ${uvTxt(w.uv)}</span>`)}${row('UV max', `<span class="${uvCls(w.uvMax)}">${w.uvMax != null ? Math.round(w.uvMax) : '—'}</span>`)}
        ${row('Air', `<span class="${w.air?.aqi > 60 ? 'warn-t' : 'ok'}">${w.air?.aqi != null ? Math.round(w.air.aqi) : '—'} ${aqTxt(w.air?.aqi)}</span>`)}${row('Pressure', `${w.pressure != null ? Math.round(w.pressure) : '—'} hPa`)}
        ${o.showSun !== false ? row('Sunrise', hm(today.sunrise)) + row('Sunset', hm(today.sunset)) : ''}
      </div>`;
    const hours = (w.hours || []).slice(0, 12);
    const hEl = $('wsHours');
    hEl.style.gridTemplateColumns = `repeat(${hours.length || 1}, minmax(0, 1fr))`;
    hEl.innerHTML = hours.map((h) => `<div class="ws-cell ${(h.rainChance ?? 0) >= 50 ? 'on' : ''}"><div class="id">${new Date(h.time).toLocaleTimeString(lang, { hour: '2-digit', hour12: !o.format24 }).replace(/\s/g, '')}</div><div class="t">${Math.round(h.temp)}°</div><div class="r ${(h.rainChance ?? 0) >= 50 ? 'wet' : ''}">${h.rainChance ?? 0}%</div></div>`).join('');
    const days = (w.days || []).slice(0, 4);
    const dEl = $('wsDays');
    dEl.style.gridTemplateColumns = `repeat(${days.length || 1}, minmax(0, 1fr))`;
    dEl.innerHTML = days.map((d, i) => `<div class="ws-cell ${i === 0 ? 'on' : ''}"><div class="day">${i === 0 ? 'TODAY' : new Date(d.date + 'T12:00').toLocaleDateString(lang, { weekday: 'short' }).toUpperCase()}</div>
      <div class="sky">${esc(d.description || d.condition || '')}</div><div class="mm">${Math.round(d.max)}° <span class="dim">/ ${Math.round(d.min)}°</span></div>
      <div class="sub">rain <span class="${d.rainChance >= 60 ? 'acc' : ''}">${d.rainChance ?? '—'}%</span> · UV <span class="${uvCls(d.uvMax)}">${d.uvMax != null ? Math.round(d.uvMax) : '—'}</span></div></div>`).join('');
  }

  function statsPanel() {
    const g = s.gpu || {};
    const swapP = s.swapTotal ? (s.swapUsed / s.swapTotal) * 100 : null;
    $('wsStats').innerHTML = [
      row('Uptime', uptime(s.uptime || 0)),
      row('Processes', `${s.processCount ?? '—'}`),
      row('CPU clock', s.cpuFreq ? `${(s.cpuFreq / 1000).toFixed(2)} GHz` : '—'),
      row('CPU power', s.cpuPower != null ? `${Math.round(s.cpuPower)} W` : '—'),
      row('GPU power', g.power != null ? `${Math.round(g.power)} W<b>hot ${deg(g.hotspot)}</b>` : '—'),
      row('RAM used', `${bytes(s.memoryUsed)}<b>/ ${bytes(s.memoryTotal)}</b>`, { p: s.memory }),
      row('Swap', s.swapTotal ? `${bytes(s.swapUsed)}<b>/ ${bytes(s.swapTotal)}</b>` : 'off', { p: swapP }),
      row('Download', rate(s.net?.rx)),
      row('Upload', rate(s.net?.tx)),
    ].join('');
    const fans = (s.fans || []).slice(0, 6);
    $('wsFanSec').classList.toggle('hidden', !fans.length || o.showNet === false);
    $('wsFans').innerHTML = fans.map((f) => row(esc(f.label), `${f.rpm} rpm`)).join('');
  }

  function currentAlerts() {
    const t = T(), g = s.gpu || {}, A = [];
    const add = (cls, msg) => A.push({ cls, msg });
    if (s.cpuTemp != null && s.cpuTemp >= t.cpu - 10) add(s.cpuTemp >= t.cpu ? 'bad-t' : 'warn-t', `CPU at ${deg(s.cpuTemp)}`);
    if (g.temp != null && g.temp >= t.gpu - 10) add(g.temp >= t.gpu ? 'bad-t' : 'warn-t', `GPU at ${deg(g.temp)}`);
    if (s.cpu >= t.load - 15) add(s.cpu >= t.load ? 'bad-t' : 'warn-t', `CPU load ${pct(s.cpu)}`);
    if (s.memory >= 85) add(s.memory >= 93 ? 'bad-t' : 'warn-t', `Memory ${pct(s.memory)} used`);
    (s.drives || []).forEach((d, i) => { if (d.temp != null && d.temp >= t.ssd - 8) add(d.temp >= t.ssd ? 'bad-t' : 'warn-t', `${(d.letters || '').trim() || 'disk ' + i} at ${deg(d.temp)}`); });
    (s.disks || []).forEach((v) => { const p = v.total ? (v.used / v.total) * 100 : 0; if (p >= 90) add(p >= 95 ? 'bad-t' : 'warn-t', `${String(v.mount).replace(/\\$/, '')} ${p.toFixed(0)}% full`); });
    return A;
  }
  function alertPanel(A) {
    const info = [];
    if (s.cpuTemp == null) info.push('> CPU/RAM temperatures: Settings → Advanced sensors');
    if (!w) info.push('> Weather: Settings → Weather & location');
    if (w && w.uvMax >= 6) info.push(`> UV ${Math.round(w.uvMax)} today: sunscreen advised`);
    if (w && (w.days?.[1]?.rainChance ?? 0) >= 60) info.push(`> Rain likely tomorrow (${w.days[1].rainChance}%)`);
    $('wsAlerts').innerHTML = (A.length ? A.map((a) => `<div class="ws-alert ${a.cls}">> ${esc(a.msg)}</div>`).join('') : '<div class="ws-alert ok">> All systems nominal</div>')
      + info.map((m) => `<div class="ws-alert info">${esc(m)}</div>`).join('');
    $('wsAlertSub').textContent = A.length ? `${A.length} active` : '';
  }

  // ------------------------------------------------------------ audio
  function drawAudio() {
    const c = $('wsViz'); if (!c || !booted || o.showAudio === false) return;
    const ctx = c.getContext('2d');
    const W = c.width, H = c.height;
    const p = getComputedStyle(document.documentElement).getPropertyValue('--ws-p').trim() || '#a78bfa';
    const a = getComputedStyle(document.documentElement).getPropertyValue('--ws-a').trim() || '#22d3ee';
    ctx.clearRect(0, 0, W, H);
    ctx.globalAlpha = 0.12; ctx.strokeStyle = p; ctx.lineWidth = 1;
    for (let y = 0; y <= H; y += H / 5) { ctx.beginPath(); ctx.moveTo(0, y + 0.5); ctx.lineTo(W, y + 0.5); ctx.stroke(); }
    ctx.globalAlpha = 1;
    const bands = audio?.bands || [];
    const sens = o.audioSens ?? 1;
    const n = 32, bw = W / n;
    for (let i = 0; i < n; i++) {
      const v = Math.min(1, (((bands[i * 2] || 0) + (bands[i * 2 + 1] || 0)) / 2) * sens);
      smooth[i] = smooth[i] * 0.7 + v * 0.3;
      const h = smooth[i] * (H - 4);
      ctx.fillStyle = i < 22 ? p : a;
      ctx.fillRect(i * bw + 1, H - h - 2, bw - 2, h);
    }
    ctx.fillStyle = p; ctx.globalAlpha = 0.6; ctx.fillRect(0, H - 2, W, 2); ctx.globalAlpha = 1;
    const avg = (from, to) => { let t = 0; for (let i = from; i < to; i++) t += bands[i] || 0; return t / (to - from); };
    bass = bass * 0.8 + Math.min(1, avg(0, 8) * sens) * 0.2;
    treble = treble * 0.8 + Math.min(1, avg(40, 64) * sens * 1.6) * 0.2;
    $('wsBass').textContent = `${Math.round(bass * 100)}%`;
    $('wsTreble').textContent = `${Math.round(treble * 100)}%`;
  }
  let raf = 0;
  function audioLoop() { raf = 0; drawAudio(); }

  // ------------------------------------------------------------ render
  function render() {
    if (o.style === 'classic' || !built) return;
    const now = new Date();
    $('wsClock').textContent = clock(now);
    $('wsUser').innerHTML = `USER: <span class="acc">${esc((o.operator || 'Operator').toUpperCase())}</span> · HOST: ${esc((s?.host || 'wallgates').toUpperCase())}`;
    $('wsDate').textContent = `${now.toLocaleDateString(lang, { weekday: 'long', day: '2-digit', month: 'long', year: 'numeric' }).toUpperCase()}`;
    const vis = (id, on) => $(id).classList.toggle('hidden', !on);
    vis('wsCoreSec', o.showCores !== false); vis('wsVolSec', o.showDisks !== false);
    vis('wsProcSec', o.showProcs !== false); vis('wsWxSec', o.showWeather !== false); vis('wsFcSec', o.showWeather !== false);
    vis('wsLogSec', o.showLog !== false); vis('wsAlertSec', o.showAlerts !== false); vis('wsAudioSec', o.showAudio !== false);
    vis('wsStatSec', o.showStats !== false); vis('wsSysSec', o.showSystems !== false); vis('wsHwSec', o.showHardware !== false);
    $('wsMedia').innerHTML = o.showMedia !== false && media?.title
      ? `<span class="acc">${media.playing ? '▶' : '❚❚'}</span> ${esc(media.title)}${media.artist ? ` <span class="dim">— ${esc(media.artist)}</span>` : ''}`
      : '<span class="dim">♪ nothing playing</span>';
    if (!s) { $('wsStatus').innerHTML = 'STATUS: <span class="warn-t blink">CONNECTING</span>'; return fit(); }
    sysPanel(); corePanel(); volPanel(); hwPanel(); procPanel(); wxPanel(); statsPanel();
    const A = currentAlerts();
    alertPanel(A);
    const bad = A.some((a) => a.cls === 'bad-t');
    $('wsStatus').innerHTML = `STATUS: ${A.length ? `<span class="${bad ? 'bad-t' : 'warn-t'} blink">${bad ? 'CRITICAL' : 'WARNING'} · ${A.length} ALERT${A.length > 1 ? 'S' : ''}</span>` : '<span class="ok">OPERATIONAL</span>'}`;
    $('wsFoot').innerHTML = `${esc(String(s.os || '').replace(/\s*\(.*\)/, '').toUpperCase())} · SENSORS: <span class="acc">${sensorsName()}</span>`;
    fit();
  }

  // Scale the text with the screen, then shrink until no panel overflows.
  let lastFit = '';
  function fit() {
    const ws = $('ws'); if (!ws || !booted) return;
    const size = Math.max(50, Math.min(100, o.wsSize ?? 100)) / 100;
    const key = `${innerWidth}x${innerHeight}|${size}|${o.wsText}|${o.showHeader}`;
    let fs = Math.min(innerWidth * size / 1920, innerHeight * size / 1080) * 14.5 * (o.wsText || 1);
    if (key === lastFit && !overflow()) return;
    lastFit = key;
    for (let i = 0; i < 25; i++) {
      document.documentElement.style.setProperty('--ws-fs', `${Math.max(7, fs).toFixed(2)}px`);
      if (!overflow()) break;
      fs *= 0.96;
    }
  }
  function overflow() {
    return [...document.querySelectorAll('#ws .ws-col')].some((c) => c.scrollHeight > c.clientHeight + 1);
  }

  // ------------------------------------------------------------ wiring
  Wallgates.on('properties', (v) => { o = v; lastFit = ''; applyLook(); });
  Wallgates.on('pause', () => { if (o.style !== 'classic') document.body.classList.toggle('paused', !!Wallgates.paused); });
  Wallgates.on('system', (d) => { prev = s; s = d; if (o.style === 'classic' || Wallgates.paused) return; detectEvents(); if (booted) render(); });
  Wallgates.on('weather', (d) => {
    const first = !w; w = d;
    if (booted && (first || throttle('wx', 600000))) addLog(`Weather uplink · ${String(d.location || '').split(',')[0]} ${Math.round(d.temperature)}° ${String(d.description || '').toLowerCase()}`, 'info');
    render();
  });
  Wallgates.on('media', (m) => {
    const changed = m?.title && m.title !== media?.title;
    media = m;
    if (booted && changed) addLog(`Now playing · ${m.title}${m.artist ? ' — ' + m.artist : ''}`, 'acc');
  });
  Wallgates.on('audio', (d) => { audio = d; if (!raf && !Wallgates.paused && o.style !== 'classic') raf = requestAnimationFrame(audioLoop); });
  setInterval(() => { if (!Wallgates.paused && booted && o.style !== 'classic') { render(); } }, 1000);
  addEventListener('resize', () => { lastFit = ''; fit(); });
})();
