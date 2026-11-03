// Trade Republic Portfolio — Wallgates built-in wallpaper (CC BY 4.0).
// Replays the Trade Republic CSV export locally, values it with live prices and daily history.
(function () {
  'use strict';
  const $ = (id) => document.getElementById(id);
  const lang = navigator.language || 'it-IT';
  let o = {}, mk = null, book = null, csvSeen = null, compact = false;
  const DAY = 86400000;

  const PALETTES = {
    midnight: { acc: '#8b5cf6', acc2: '#22d3ee', gain: '#34d399', loss: '#fb7185', bg: '#070912', bg2: '#0e1426', card: '#0c1120', fg: '#e6e9f5' },
    wallgates: { acc: '#a78bfa', acc2: '#22d3ee', gain: '#4ade80', loss: '#fb7185', bg: '#06050d', bg2: '#120d26', card: '#0d0a1c', fg: '#e3defc' },
    terminal: { acc: '#ffaa66', acc2: '#ffcc88', gain: '#9be564', loss: '#ff6b5a', bg: '#070301', bg2: '#150a04', card: '#0f0703', fg: '#ffdcb8' },
    matrix: { acc: '#33ff77', acc2: '#b6ffcb', gain: '#33ff77', loss: '#ff5f56', bg: '#010602', bg2: '#03140a', card: '#020b05', fg: '#c8ffd8' },
  };
  const FONTS = { 'share-tech': "'Share Tech Mono'", jetbrains: "'JetBrains Mono'", plex: "'IBM Plex Mono'", fira: "'Fira Code'", space: "'Space Mono'", vt323: "'VT323'", cascadia: "'Cascadia Mono', Consolas" };
  const TITLE_FONTS = { same: 'inherit', orbitron: "'Orbitron'", rajdhani: "'Rajdhani'" };
  const SERIES = ['#8b5cf6', '#22d3ee', '#34d399', '#fbbf24', '#f472b6', '#60a5fa', '#a3e635', '#fb923c', '#e879f9', '#2dd4bf', '#f87171', '#94a3b8'];

  // ------------------------------------------------------------ formatting
  const cur = () => o.currency || 'EUR';
  const nf = (d) => new Intl.NumberFormat(lang, { minimumFractionDigits: d, maximumFractionDigits: d });
  const money = (v, d = 2) => (v == null || !isFinite(v) ? '—' : o.privacy ? '•••••' : new Intl.NumberFormat(lang, { style: 'currency', currency: cur(), minimumFractionDigits: d, maximumFractionDigits: d }).format(v));
  const smoney = (v, d = 2) => (v == null || !isFinite(v) ? '—' : o.privacy ? '•••' : (v > 0 ? '+' : v < 0 ? '−' : '') + money(Math.abs(v), d));
  const spct = (v, d = 2) => (v == null || !isFinite(v) ? '—' : (v > 0 ? '+' : v < 0 ? '−' : '') + nf(d).format(Math.abs(v)) + '%');
  const cls = (v) => (v > 0.004 ? 'up' : v < -0.004 ? 'dn' : '');
  const esc = (t) => String(t ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  const num = (s) => { const v = parseFloat(String(s ?? '').trim()); return isFinite(v) ? v : 0; };
  const dayKey = (t) => new Date(t).toISOString().slice(0, 10);

  // ------------------------------------------------------------ CSV
  function csvFields(line) {
    const out = []; let cur = '', q = false;
    for (let i = 0; i < line.length; i++) {
      const c = line[i];
      if (c === '"' && q && line[i + 1] === '"') { cur += '"'; i++; }
      else if (c === '"') q = !q;
      else if (c === ',' && !q) { out.push(cur); cur = ''; }
      else cur += c;
    }
    out.push(cur);
    return out;
  }
  function parseCsv(text) {
    const lines = String(text || '').split(/\r?\n/).filter((l) => l.trim());
    if (lines.length < 2) return [];
    const head = csvFields(lines[0]).map((h) => h.trim().toLowerCase());
    return lines.slice(1).map((l) => { const f = csvFields(l); const r = {}; head.forEach((h, i) => (r[h] = f[i] ?? '')); return r; })
      .filter((r) => r.datetime || r.date)
      .map((r) => ({ ...r, t: Date.parse(r.datetime || r.date) }))
      .filter((r) => isFinite(r.t))
      .sort((a, b) => a.t - b.t);
  }
  const idOf = (r) => (String(r.asset_class).toUpperCase() === 'CRYPTO' ? `${String(r.symbol).toUpperCase()}-EUR` : String(r.symbol || '').toUpperCase());

  // Replay every transaction: positions, cost basis, cash, income and costs. Only amounts in EUR exist in the export.
  function buildBook(rows) {
    const pos = {}, flows = [], events = [];
    let cash = 0, deposits = 0, withdrawals = 0, dividends = 0, interest = 0, fees = 0, taxes = 0, realized = 0;
    for (const r of rows) {
      const amount = num(r.amount), fee = num(r.fee), tax = num(r.tax), shares = num(r.shares);
      const type = String(r.type).toUpperCase(), cat = String(r.category).toUpperCase();
      cash += amount + fee + tax;
      fees += fee; taxes += tax;
      if (cat === 'TRADING' && r.symbol) {
        const id = idOf(r);
        const p = (pos[id] ||= { id, isin: r.symbol, name: r.name || r.symbol, cls: r.asset_class || '', shares: 0, cost: 0, realized: 0, lastPrice: 0, trades: [], first: r.t });
        p.name = (r.name || p.name).trim();
        if (num(r.price)) p.lastPrice = num(r.price);
        if (shares > 0) { p.cost += -amount - fee; p.shares += shares; }
        else if (shares < 0) {
          const avg = p.shares > 0 ? p.cost / p.shares : 0, sold = Math.min(-shares, p.shares);
          const pl = amount + fee + tax - avg * sold;
          p.realized += pl; realized += pl; p.cost -= avg * sold; p.shares -= sold;
          if (p.shares < 1e-6) { p.shares = 0; p.cost = 0; }
        }
        p.trades.push({ t: r.t, shares });
        events.push({ t: r.t, kind: shares >= 0 ? 'buy' : 'sell', name: p.name, amount: amount + fee + tax });
      } else if (type.includes('TRANSFER') || type === 'DEPOSIT' || type === 'WITHDRAWAL') {
        flows.push({ t: r.t, v: amount });
        if (amount >= 0) deposits += amount; else withdrawals += -amount;
        events.push({ t: r.t, kind: amount >= 0 ? 'in' : 'out', name: amount >= 0 ? 'Deposit' : 'Withdrawal', amount });
      } else if (type === 'DIVIDEND') {
        dividends += amount;
        events.push({ t: r.t, kind: 'div', name: `Dividend · ${r.name || r.symbol}`, amount: amount + tax });
      } else if (type.startsWith('INTEREST')) {
        interest += amount;
        if (amount >= 0.05) events.push({ t: r.t, kind: 'int', name: 'Interest', amount: amount + tax });
      } else if (type === 'MANUAL_CASH_TRANSFER' || type === 'TAX_CREDIT') {
        taxes += amount; // tax credits come back as manual cash transfers
      } else if (cat === 'CARD' || type.startsWith('CARD')) {
        // Card spending leaves the investment account like a withdrawal.
        flows.push({ t: r.t, v: amount });
        if (amount < 0) withdrawals += -amount;
      }
    }
    return { rows, pos, flows, events, cash, deposits, withdrawals, dividends, interest, fees, taxes, realized, first: rows[0]?.t ?? Date.now() };
  }

  // ------------------------------------------------------------ prices
  function fxTo(ccy) {
    if (!ccy) return 1;
    let c = ccy === 'GBp' ? 'GBP' : ccy, k = ccy === 'GBp' ? 0.01 : 1;
    if (c === cur()) return k;
    const r = mk?.fx?.[`${c}${cur()}`];
    return r ? r * k : k; // unknown pair: assume same currency
  }
  const quoteOf = (id) => (mk?.quotes || []).find((q) => q.id === id);
  // Price (in the base currency) of an instrument at time t, from daily history; null if unknown.
  function priceAt(p, t) {
    const q = quoteOf(p.id);
    const h = q?.history;
    if (h?.length) {
      let lo = 0, hi = h.length - 1, best = -1; const s = t / 1000;
      while (lo <= hi) { const m = (lo + hi) >> 1; if (h[m][0] <= s) { best = m; lo = m + 1; } else hi = m - 1; }
      if (best >= 0) return h[best][1] * fxTo(q.currency);
      return h[0][1] * fxTo(q.currency);
    }
    return null;
  }
  // Manual prices ("ISIN price" per line) win over feeds: for instruments without any public quote.
  let overrides = {};
  const parseOverrides = (txt) => Object.fromEntries(String(txt || '').split(/\r?\n/).map((l) => l.split('#')[0].trim().split(/[\s;=]+/))
    .filter((a) => a.length >= 2 && isFinite(parseFloat(a[1].replace(',', '.')))).map((a) => [a[0].toUpperCase(), parseFloat(a[1].replace(',', '.'))]));
  const missingFx = new Set();
  const priceNow = (p) => {
    if (overrides[p.isin] != null) return overrides[p.isin];
    const q = quoteOf(p.id);
    if (q?.price == null) return null;
    const c = q.currency === 'GBp' ? 'GBP' : q.currency;
    if (c && c !== cur() && !mk?.fx?.[`${c}${cur()}`]) missingFx.add(c);
    return q.price * fxTo(q.currency);
  };
  const sharesAt = (p, t) => p.trades.reduce((s, tr) => (tr.t <= t ? s + tr.shares : s), 0);
  const flowsBetween = (a, b) => book.flows.reduce((s, f) => (f.t > a && f.t <= b ? s + f.v : s), 0);
  const cashAt = (t) => book.rows.reduce((s, r) => (r.t <= t ? s + num(r.amount) + num(r.fee) + num(r.tax) : s), 0);

  function valueAt(t) {
    let v = 0;
    for (const p of Object.values(book.pos)) {
      const sh = sharesAt(p, t);
      if (sh <= 1e-9) continue;
      const px = priceAt(p, t) ?? (p.cost && p.shares ? p.cost / p.shares : p.lastPrice);
      v += sh * px;
    }
    return v + (o.includeCash !== false ? cashAt(t) : 0);
  }
  function valueNow() {
    let v = 0;
    for (const p of Object.values(book.pos)) if (p.shares > 1e-9) v += p.shares * (priceNow(p) ?? p.lastPrice);
    return v + (o.includeCash !== false ? book.cash : 0);
  }
  // Gain over [t0, now] net of deposits/withdrawals, and a Modified-Dietz style percentage.
  function periodGain(t0) {
    const now = Date.now();
    if (t0 <= book.first) {
      const invested = book.deposits - book.withdrawals + (o.includeCash !== false ? 0 : -book.cash);
      const g = valueNow() - invested;
      return { g, p: invested ? (g / invested) * 100 : null };
    }
    const v0 = valueAt(t0), v1 = valueNow();
    const fl = book.flows.filter((f) => f.t > t0 && f.t <= now);
    const net = fl.reduce((s, f) => s + f.v, 0);
    const w = fl.reduce((s, f) => s + f.v * ((now - f.t) / (now - t0)), 0);
    const g = v1 - v0 - net;
    const base = v0 + w;
    return { g, p: base > 1 ? (g / base) * 100 : null };
  }
  function dayGain() {
    let g = 0, base = 0;
    for (const p of Object.values(book.pos)) {
      if (p.shares <= 1e-9) continue;
      const q = quoteOf(p.id); if (!q || q.previousClose == null) continue;
      const k = fxTo(q.currency);
      g += p.shares * (q.price - q.previousClose) * k; base += p.shares * q.previousClose * k;
    }
    return { g, p: base ? (g / base) * 100 : null };
  }

  // ------------------------------------------------------------ look
  function applyLook() {
    compact = o.layout === 'compact';
    document.body.classList.toggle('compact', compact);
    overrides = parseOverrides(o.priceOverrides);
    const r = document.documentElement.style;
    const pal = o.palette === 'custom'
      ? { ...PALETTES.midnight, acc: o.accentColor, gain: o.gainColor, loss: o.lossColor, bg: o.bgColor, bg2: o.bgColor }
      : PALETTES[o.palette] || PALETTES.midnight;
    for (const [k, v] of Object.entries(pal)) if (v) r.setProperty(`--${k}`, v);
    r.setProperty('--font', FONTS[o.font] || FONTS.jetbrains);
    r.setProperty('--title-font', TITLE_FONTS[o.titleFont] || TITLE_FONTS.rajdhani);
    r.setProperty('--glow', o.glow ?? 0.35);
    const size = Math.max(50, Math.min(100, o.size ?? 100));
    r.setProperty('--w', `${size}vw`); r.setProperty('--h', `${size}vh`);
    document.body.classList.toggle('paused', !!Wallgates.paused);
    const show = (id, on) => $(id).classList.toggle('hidden', on === false);
    show('chartCard', o.showChart); show('allocCard', o.showAllocation); show('monthCard', o.showMonthly);
    show('incomeCard', o.showIncome); show('activityCard', o.showActivity && !compact); show('tape', compact && o.showTape !== false);
    lastFit = '';
  }

  // ------------------------------------------------------------ panels
  const sparkSvg = (vals) => {
    if (vals.length < 2) return '';
    const mn = Math.min(...vals), mx = Math.max(...vals), rg = mx - mn || 1;
    const pts = vals.map((v, i) => `${(i / (vals.length - 1)) * 100},${100 - ((v - mn) / rg) * 90}`).join(' ');
    return `<svg viewBox="0 0 100 100" preserveAspectRatio="none"><polyline points="${pts}" fill="none" stroke="currentColor" stroke-width="2" vector-effect="non-scaling-stroke"/></svg>`;
  };
  const series = (t0, n = 30) => { const now = Date.now(), out = []; for (let i = 0; i <= n; i++) { const t = t0 + ((now - t0) * i) / n; out.push(valueAt(t) - (book.deposits ? flowsBetween(t0, t) : 0)); } return out; };

  function heroPanel() {
    const now = new Date(), total = valueNow();
    const all = periodGain(0);
    $('total').innerHTML = `<span class="money">${money(total)}</span>`;
    $('totalSub').innerHTML = `<span class="${cls(all.g)}">${smoney(all.g)} · ${spct(all.p)}</span> <span class="dim">all time</span>`;
    const invested = Object.values(book.pos).reduce((s, p) => s + p.cost, 0);
    $('heroKv').innerHTML = `<span>Invested <b class="money">${money(invested)}</b></span><span>Cash <b class="money">${money(book.cash)}</b></span><span>Deposited <b class="money">${money(book.deposits - book.withdrawals)}</b></span>`;
    const y0 = new Date(now.getFullYear(), 0, 1).getTime();
    const P = [
      ['Today', dayGain(), null],
      ['1 week', periodGain(Date.now() - 7 * DAY), Date.now() - 7 * DAY],
      ['1 month', periodGain(Date.now() - 30 * DAY), Date.now() - 30 * DAY],
      ['YTD', periodGain(y0), y0],
      ['1 year', periodGain(Date.now() - 365 * DAY), Date.now() - 365 * DAY],
      ['All time', all, book.first],
    ];
    $('periods').innerHTML = P.map(([k, r, t0]) => `<div class="per ${cls(r.g)}"><span class="k">${k}</span><span class="v money">${smoney(r.g)}</span><span class="p">${spct(r.p)}</span>${t0 ? sparkSvg(series(Math.max(t0, book.first), 24)) : ''}</div>`).join('');
  }

  function holdingRows() {
    return Object.values(book.pos).filter((p) => p.shares > 1e-9).map((p) => {
      const q = quoteOf(p.id), px = priceNow(p), manual = overrides[p.isin] != null;
      const value = p.shares * (px ?? p.lastPrice);
      const day = !manual && q && q.previousClose ? ((q.price - q.previousClose) / q.previousClose) * 100 : null;
      const src = manual ? 'manual price' : q ? `${q.symbol || p.isin}${q.exchange ? ' · ' + q.exchange : ''}` : 'last trade price';
      return { p, q, px: px ?? p.lastPrice, live: px != null, manual, value, pl: value - p.cost, plp: p.cost ? ((value - p.cost) / p.cost) * 100 : null, day, src };
    }).sort((x, y) => y.value - x.value);
  }

  function holdingsPanel() {
    const rows = holdingRows();
    const tot = rows.reduce((s, r) => s + r.value, 0) || 1;
    const closed = Object.values(book.pos).filter((p) => p.shares <= 1e-9 && Math.abs(p.realized) > 0.005);
    $('holdSub').textContent = `${rows.length} open · ${closed.length} closed`;
    const flag = (r) => (r.manual ? '<span class="flag" title="Manual price">✎</span>' : r.live ? '' : '<span class="flag" title="No public quote: valued at the last trade price">◆</span>');
    let html;
    if (compact) {
      html = `<table class="ctab"><colgroup><col style="width:44%"><col style="width:20%"><col style="width:14%"><col style="width:22%"></colgroup>
        ${rows.map((r) => `<tr><td><span class="nm">${esc(r.p.name)}</span>${flag(r)}<div class="tk">${esc(r.src)} · ${nf(r.p.shares < 10 ? 3 : 2).format(r.p.shares)} × ${o.privacy ? '•••' : money(r.px)}</div></td>
          <td class="money">${money(r.value)}<div class="tk">${nf(1).format((r.value / tot) * 100)}%</div></td>
          <td class="${cls(r.day)}">${spct(r.day)}</td>
          <td class="${cls(r.pl)}"><span class="money">${smoney(r.pl)}</span><div class="tk ${cls(r.pl)}">${spct(r.plp, 1)}</div></td></tr>`).join('')}
        ${o.showClosed !== false ? closed.map((p) => `<tr class="closed"><td>${esc(p.name)}<div class="tk">closed position</div></td><td></td><td></td><td class="${cls(p.realized)}"><span class="money">${smoney(p.realized)}</span><div class="tk">realised</div></td></tr>`).join('') : ''}</table>`;
      scrollInto('holdings', html, `<table class="ctab head"><colgroup><col style="width:44%"><col style="width:20%"><col style="width:14%"><col style="width:22%"></colgroup><tr><th>Instrument</th><th>Value</th><th>Today</th><th>Gain / loss</th></tr></table>`);
    } else {
      html = `<table><colgroup><col style="width:30%"><col style="width:9%"><col style="width:11%"><col style="width:13%"><col style="width:9%"><col style="width:15%"><col style="width:13%"></colgroup>
      <tr><th>Instrument</th><th>Shares</th><th>Price</th><th>Value</th><th>Today</th><th>Gain / loss</th><th>Weight</th></tr>
      ${rows.map((r) => `<tr><td><span class="nm">${esc(r.p.name)}</span><span class="tk">${esc(r.src)}</span>${flag(r)}</td>
        <td>${nf(r.p.shares < 10 ? 3 : 2).format(r.p.shares)}</td><td class="money">${money(r.px)}</td><td class="money">${money(r.value)}</td>
        <td class="${cls(r.day)}">${spct(r.day)}</td><td class="${cls(r.pl)}"><span class="money">${smoney(r.pl)}</span> <span class="mute">${spct(r.plp, 1)}</span></td>
        <td><span class="w" style="width:${Math.max(2, (r.value / tot) * 60).toFixed(1)}%"></span>${nf(1).format((r.value / tot) * 100)}%</td></tr>`).join('')}
      ${o.showClosed !== false ? closed.slice(-4).map((p) => `<tr class="closed"><td>${esc(p.name)} <span class="tk">closed</span></td><td></td><td></td><td></td><td></td><td class="${cls(p.realized)}"><span class="money">${smoney(p.realized)}</span> <span class="mute">realised</span></td><td></td></tr>`).join('') : ''}
      </table>`;
      $('holdings').innerHTML = html;
    }
    return rows;
  }

  // Auto-scrolling list: when the content is taller than its box it loops slowly upwards (no restart on updates).
  function scrollInto(id, html, header = '') {
    const host = $(id);
    if (!host.querySelector('.scroller')) host.innerHTML = '<div class="shead"></div><div class="scroller"><div class="scroll-inner"></div></div>';
    host.querySelector('.shead').innerHTML = header;
    const box = host.querySelector('.scroller'), inner = box.querySelector('.scroll-inner');
    inner.innerHTML = html;
    const over = inner.scrollHeight > box.clientHeight + 4;
    if (over) {
      inner.innerHTML = html + '<div class="gap"></div>' + html;
      inner.style.animationDuration = `${Math.max(20, inner.scrollHeight / 2 / 22)}s`;
    }
    inner.classList.toggle('loop', over);
  }

  // Bottom tape: every position with today's move, scrolling right to left.
  function tapePanel(rows) {
    const items = rows.map((r) => `<span class="ti"><b>${esc(r.p.name)}</b> <span class="money">${money(r.px)}</span> <span class="${cls(r.day)}">${r.day == null ? '' : (r.day > 0 ? '▲ ' : r.day < 0 ? '▼ ' : '') + spct(r.day)}</span></span>`).join('<span class="sep">│</span>');
    const t = $('tapeInner');
    const html = items + '<span class="sep">│</span>' + items + '<span class="sep">│</span>';
    if (t.dataset.k !== html) { t.innerHTML = html; t.dataset.k = html; t.style.animationDuration = `${Math.max(40, rows.length * 7)}s`; }
  }

  function allocPanel(rows) {
    const groups = {};
    const label = (c) => ({ FUND: 'ETFs & funds', STOCK: 'Stocks', CRYPTO: 'Crypto', DERIVATIVE: 'Derivatives', BOND: 'Bonds' }[String(c).toUpperCase()] || 'Other');
    for (const r of rows) groups[label(r.p.cls)] = (groups[label(r.p.cls)] || 0) + r.value;
    if (o.includeCash !== false && book.cash > 0.5) groups.Cash = book.cash;
    const items = Object.entries(groups).sort((a, b) => b[1] - a[1]);
    const tot = items.reduce((s, [, v]) => s + v, 0) || 1;
    const c = $('donut'), ctx = c.getContext('2d'), R = c.width / 2;
    ctx.clearRect(0, 0, c.width, c.height);
    let a = -Math.PI / 2;
    items.forEach(([, v], i) => {
      const da = (v / tot) * Math.PI * 2;
      const gap = Math.min(0.02, da / 4);
      ctx.beginPath(); ctx.arc(R, R, R - 14, a + gap, a + da - gap); ctx.lineWidth = 26; ctx.strokeStyle = SERIES[i % SERIES.length];
      ctx.shadowColor = SERIES[i % SERIES.length]; ctx.shadowBlur = 12 * (o.glow ?? 0.35); ctx.stroke(); a += da;
    });
    ctx.shadowBlur = 0;
    const css = getComputedStyle(document.documentElement);
    ctx.fillStyle = css.getPropertyValue('--fg').trim(); ctx.textAlign = 'center';
    ctx.font = `700 34px ${css.getPropertyValue('--title-font')}, sans-serif`; ctx.fillText(String(rows.length), R, R + 4);
    ctx.font = `400 16px ${css.getPropertyValue('--font')}, monospace`; ctx.fillStyle = css.getPropertyValue('--dim').trim(); ctx.fillText('positions', R, R + 28);
    $('allocList').innerHTML = items.map(([k, v], i) => `<div class="al-row"><i style="background:${SERIES[i % SERIES.length]}"></i><span>${esc(k)}</span><b>${nf(1).format((v / tot) * 100)}%</b></div>`).join('')
      + rows.slice(0, 4).map((r) => `<div class="al-row"><i style="background:transparent;border:1px solid var(--line2)"></i><span class="dim">${esc(r.p.name)}</span><span class="dim">${nf(1).format((r.value / tot) * 100)}%</span></div>`).join('');
    $('allocSub').textContent = 'by asset class';
  }

  function monthsPanel() {
    const now = new Date(), M = [];
    for (let i = 11; i >= 0; i--) {
      const a = new Date(now.getFullYear(), now.getMonth() - i, 1).getTime();
      const b = Math.min(Date.now(), new Date(now.getFullYear(), now.getMonth() - i + 1, 1).getTime() - 1);
      if (b < book.first) { M.push({ a, g: null }); continue; }
      const start = Math.max(a, book.first - 1);
      const v0 = a <= book.first ? 0 : valueAt(a), v1 = b >= Date.now() - 1000 ? valueNow() : valueAt(b);
      M.push({ a, g: v1 - v0 - flowsBetween(start, b) });
    }
    const mx = Math.max(1, ...M.map((m) => Math.abs(m.g || 0)));
    const hasNeg = M.some((m) => (m.g || 0) < 0);
    $('months').className = 'months';
    $('months').innerHTML = M.map((m) => {
      const h = m.g == null ? 0 : (Math.abs(m.g) / mx) * (hasNeg ? 46 : 82);
      const label = new Date(m.a).toLocaleDateString(lang, { month: 'short' }).slice(0, 3);
      return `<div class="mb ${cls(m.g)}" style="${hasNeg ? 'justify-content:center' : ''}">${m.g == null ? '' : `<span class="val ${cls(m.g)}">${o.privacy ? '' : Math.round(m.g)}</span>`}<div class="bar2" style="height:${h.toFixed(1)}%;${hasNeg && m.g < 0 ? 'border-radius:0 0 .2em .2em' : ''}"></div><small>${label}</small></div>`;
    }).join('');
    const years = [];
    for (let y = new Date(book.first).getFullYear(); y <= now.getFullYear(); y++) {
      const a = new Date(y, 0, 1).getTime(), b = Math.min(Date.now(), new Date(y + 1, 0, 1).getTime() - 1);
      const v0 = a <= book.first ? 0 : valueAt(a), v1 = y === now.getFullYear() ? valueNow() : valueAt(b);
      const fl = flowsBetween(a <= book.first ? book.first - 1 : a, b), g = v1 - v0 - fl;
      years.push(`<div class="kv"><span>${y}${y === now.getFullYear() ? ' (YTD)' : ''}</span><span class="${cls(g)}"><span class="money">${smoney(g)}</span> <span class="mute">${spct(v0 + fl / 2 > 1 ? (g / (v0 + fl / 2)) * 100 : null, 1)}</span></span></div>`);
    }
    $('years').innerHTML = years.reverse().join('');
  }

  function incomePanel() {
    const totalPL = Object.values(book.pos).reduce((s, p) => s + p.realized, 0);
    $('income').innerHTML = [
      ['Dividends', book.dividends, 'up'], ['Interest', book.interest, 'up'], ['Realised gains', totalPL, cls(totalPL)],
      ['Fees', book.fees, 'dn'], ['Taxes', book.taxes, cls(book.taxes)],
    ].map(([k, v, c]) => `<div class="kv"><span>${k}</span><span class="${c} money">${smoney(v)}</span></div>`).join('');
  }

  function activityPanel() {
    const icon = { buy: '<span class="acc">▲</span>', sell: '<span class="up">▼</span>', in: '<span class="up">↓</span>', out: '<span class="dn">↑</span>', div: '<span class="up">◆</span>', int: '<span class="dim">%</span>' };
    $('activity').innerHTML = book.events.slice(-9).reverse().map((e) => `<div class="act"><span class="d">${new Date(e.t).toLocaleDateString(lang, { day: '2-digit', month: 'short' })}</span><span class="ic">${icon[e.kind] || '·'}</span><span>${esc(e.kind === 'buy' ? 'Buy · ' + e.name : e.kind === 'sell' ? 'Sell · ' + e.name : e.name)}</span><span class="${e.amount >= 0 ? 'up' : ''} money">${smoney(e.amount)}</span></div>`).join('');
  }

  function chartPanel() {
    const c = $('chart'), box = c.getBoundingClientRect();
    const dpr = Math.min(2, devicePixelRatio || 1);
    c.width = Math.max(10, box.width * dpr); c.height = Math.max(10, box.height * dpr);
    const ctx = c.getContext('2d'); ctx.scale(dpr, dpr);
    const W = box.width, H = box.height;
    const now = Date.now();
    const t0 = { '1m': now - 30 * DAY, '3m': now - 91 * DAY, ytd: new Date(new Date().getFullYear(), 0, 1).getTime(), '1y': now - 365 * DAY }[o.chartRange] ?? book.first;
    const start = Math.max(t0, book.first), N = 120;
    const val = [], inv = [];
    let invested = book.flows.reduce((s, f) => (f.t <= start ? s + f.v : s), 0);
    for (let i = 0; i <= N; i++) {
      const t = start + ((now - start) * i) / N;
      val.push(i === N ? valueNow() : valueAt(t));
      inv.push(book.flows.reduce((s, f) => (f.t <= t ? s + f.v : s), 0) - (o.includeCash !== false ? 0 : cashAt(t)));
    }
    const mn = Math.min(...val, ...inv) * 0.97, mx = Math.max(...val, ...inv) * 1.02, rg = mx - mn || 1;
    const css = getComputedStyle(document.documentElement);
    const acc = css.getPropertyValue('--acc').trim(), acc2 = css.getPropertyValue('--acc2').trim(), line = css.getPropertyValue('--line').trim(), mute = css.getPropertyValue('--mute').trim();
    const fsz = parseFloat(css.getPropertyValue("--fs")) || 14, padL = 8, padR = fsz * 5.6, padT = fsz, padB = fsz * 1.7;
    const X = (i) => padL + (i / N) * (W - padL - padR), Y = (v) => padT + (1 - (v - mn) / rg) * (H - padT - padB);
    ctx.font = `${Math.round(parseFloat(css.getPropertyValue("--fs")) * 0.74)}px ${css.getPropertyValue("--font")}, monospace`; ctx.fillStyle = mute; ctx.strokeStyle = line; ctx.lineWidth = 1;
    for (let k = 0; k <= 4; k++) {
      const v = mn + (rg * k) / 4, y = Y(v);
      ctx.beginPath(); ctx.moveTo(padL, y + 0.5); ctx.lineTo(W - padR, y + 0.5); ctx.stroke();
      ctx.textAlign = 'left'; ctx.fillText(o.privacy ? '' : money(v, 0), W - padR + 8, y + 4);
    }
    for (let k = 0; k <= 4; k++) { ctx.textAlign = k === 0 ? 'left' : k === 4 ? 'right' : 'center'; const t = start + ((now - start) * k) / 4; ctx.fillText(new Date(t).toLocaleDateString(lang, { day: '2-digit', month: 'short', ...(now - start > 300 * DAY ? { year: '2-digit' } : {}) }), X((N * k) / 4), H - 5); }
    // invested (dashed)
    ctx.setLineDash([5, 5]); ctx.strokeStyle = mute; ctx.lineWidth = 1.4; ctx.beginPath();
    inv.forEach((v, i) => (i ? ctx.lineTo(X(i), Y(v)) : ctx.moveTo(X(i), Y(v)))); ctx.stroke(); ctx.setLineDash([]);
    // value area
    const g = ctx.createLinearGradient(0, padT, 0, H - padB);
    g.addColorStop(0, acc + '66'); g.addColorStop(1, acc + '00');
    ctx.beginPath(); val.forEach((v, i) => (i ? ctx.lineTo(X(i), Y(v)) : ctx.moveTo(X(i), Y(v))));
    ctx.lineTo(X(N), H - padB); ctx.lineTo(X(0), H - padB); ctx.closePath(); ctx.fillStyle = g; ctx.fill();
    const lg = ctx.createLinearGradient(padL, 0, W - padR, 0); lg.addColorStop(0, acc); lg.addColorStop(1, acc2);
    ctx.beginPath(); val.forEach((v, i) => (i ? ctx.lineTo(X(i), Y(v)) : ctx.moveTo(X(i), Y(v))));
    ctx.strokeStyle = lg; ctx.lineWidth = 2.2; ctx.shadowColor = acc2; ctx.shadowBlur = 10 * (o.glow ?? 0.35); ctx.stroke(); ctx.shadowBlur = 0;
    // end marker
    const ex = X(N), ey = Y(val[N]);
    ctx.fillStyle = acc2; ctx.beginPath(); ctx.arc(ex, ey, 3.5, 0, Math.PI * 2); ctx.fill();
    ctx.strokeStyle = acc2 + '55'; ctx.lineWidth = 6; ctx.beginPath(); ctx.arc(ex, ey, 7, 0, Math.PI * 2); ctx.stroke();
    const rangeG = val[N] - val[0] - (inv[N] - inv[0]);
    $('chartSub').innerHTML = `${{ '1m': '1 month', '3m': '3 months', ytd: 'year to date', '1y': '1 year' }[o.chartRange] || 'since first deposit'} · <span class="${cls(rangeG)}">${smoney(rangeG)}</span>`;
    $('legend').innerHTML = `<span><i style="background:linear-gradient(90deg,${acc},${acc2})"></i>Value</span><span><i style="background:repeating-linear-gradient(90deg,${mute} 0 4px,transparent 4px 8px)"></i>Net deposits</span>`;
  }

  // ------------------------------------------------------------ render
  function render() {
    missingFx.clear();
    const now = new Date();
    $('clock').textContent = now.toLocaleTimeString(lang, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
    if (!book || !book.rows.length) { document.body.classList.add('empty'); return; }
    document.body.classList.remove('empty');
    const qs = mk?.quotes || [];
    const open = qs.filter((q) => q.state === 'open').length;
    $('mkt').innerHTML = `<span><span class="dot ${open ? 'open' : ''}"></span>${open ? `<b>${open}</b> markets open` : 'markets closed'}</span><span>updated <b>${mk?.updated ? new Date(mk.updated * 1000).toLocaleTimeString(lang, { hour: '2-digit', minute: '2-digit' }) : '—'}</b></span>`;
    $('acct').textContent = `since ${new Date(book.first).toLocaleDateString(lang, { month: 'short', year: 'numeric' })}`;
    heroPanel();
    const rows = holdingsPanel();
    if (o.showAllocation !== false) allocPanel(rows);
    if (o.showMonthly !== false) monthsPanel();
    if (o.showIncome !== false) incomePanel();
    if (o.showActivity !== false && !compact) activityPanel();
    if (compact) tapePanel(rows);
    const noQuote = Object.values(book.pos).filter((p) => p.shares > 1e-9 && !quoteOf(p.id) && overrides[p.isin] == null).length;
    $('status').innerHTML = mk ? `<span class="dot open"></span>LIVE · ${qs.length} quotes` : '<span class="dot"></span>WAITING FOR PRICES…';
    $('errs').textContent = [noQuote ? `◆ ${noQuote} without public quote — valued at last trade price (add it under Manual prices)` : '', missingFx.size ? `FX rate missing for ${[...missingFx].join(', ')}` : ''].filter(Boolean).join(' · ');
    fit();
    if (o.showChart !== false) chartPanel();
  }

  let lastFit = '';
  function fit() {
    const size = Math.max(50, Math.min(100, o.size ?? 100)) / 100;
    const key = `${innerWidth}x${innerHeight}|${size}|${o.textSize}|${o.font}|${compact}`;
    if (key === lastFit) return;
    lastFit = key;
    let fs = Math.min(innerWidth * size / 1920, innerHeight * size / 1080) * (compact ? 19 : 14) * (o.textSize || 1) * (o.font === 'vt323' ? 1.3 : 1);
    const over = () => [...document.querySelectorAll('#app .card')].some((c) => c.scrollHeight > c.clientHeight + 2);
    for (let i = 0; i < 25; i++) {
      document.documentElement.style.setProperty('--fs', `${Math.max(7, fs).toFixed(2)}px`);
      if (!over()) break;
      fs *= 0.96;
    }
  }

  Wallgates.on('properties', (v) => {
    o = v;
    if (v.tradeRepublicCsv !== csvSeen) { csvSeen = v.tradeRepublicCsv; book = buildBook(parseCsv(csvSeen)); }
    applyLook(); render();
  });
  Wallgates.on('pause', () => document.body.classList.toggle('paused', !!Wallgates.paused));
  Wallgates.on('markets', (d) => { mk = d; render(); });
  setInterval(() => { if (!Wallgates.paused) $('clock').textContent = new Date().toLocaleTimeString(lang, { hour: '2-digit', minute: '2-digit', second: '2-digit' }); }, 1000);
  setInterval(() => { if (!Wallgates.paused) render(); }, 60000);
  addEventListener('resize', () => { lastFit = ''; render(); });
})();
