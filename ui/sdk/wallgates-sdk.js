/*! Wallgates SDK for HTML wallpapers — MIT licence (use it in wallpapers under any licence).
 *
 *
 *  <script src="/__wallgates/wallgates-sdk.js"></script>
 *  Wallgates.on('properties', (values) => ...);   // initial + every change
 *  Wallgates.on('pause', (paused) => ...);         // stop animating while paused
 *  Wallgates.on('audio', ({ bands, level }) => ...)   // needs "audio" capability
 *  Wallgates.on('mouse', ({ x, y, desktop }) => ...)  // needs "mouse"
 *  Wallgates.on('system', ({ cpu, memory }) => ...)   // needs "system"
 *  Wallgates.on('media', ({ title, artist, playing, thumbnail }) => ...) // needs "media"
 *  Wallgates.on('weather', ({ temperature, code, isDay }) => ...)        // needs "weather"
 */
(function () {
  'use strict';
  const handlers = {};
  const last = {};
  const api = {
    values: {},
    paused: false,
    renderScale: 1,
    on(event, cb) {
      (handlers[event] = handlers[event] || []).push(cb);
      if (event in last) cb(last[event]); // replay the latest value
      return api;
    },
  };
  const emit = (event, data) => {
    last[event] = data;
    (handlers[event] || []).forEach((cb) => { try { cb(data); } catch (e) { console.error(e); } });
  };
  addEventListener('message', (e) => {
    const m = e.data;
    if (!m || m.type !== 'wallgates') return;
    switch (m.event) {
      case 'init':
      case 'properties':
      case 'pause':
        if (m.values) { api.values = m.values; emit('properties', m.values); }
        if (typeof m.renderScale === 'number') api.renderScale = m.renderScale;
        if (typeof m.paused === 'boolean' && (m.event !== 'properties' || m.paused !== api.paused)) {
          api.paused = m.paused; emit('pause', m.paused);
        }
        break;
      default:
        emit(m.event, m);
    }
  });
  window.Wallgates = api;
})();
