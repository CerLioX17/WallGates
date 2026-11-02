/*! Wallgates compatibility layer for web wallpapers made for Wallpaper Engine.
 *  Implements the documented JavaScript hooks (property listener, audio and
 *  media listeners) on top of Wallgates messages. Injected automatically into
 *  imported Wallpaper Engine web wallpapers. MIT licence. */
(function () {
  'use strict';
  const audio = [], mediaProps = [], mediaPlayback = [], mediaThumb = [], mediaStatus = [];
  const call = (list, arg) => list.forEach((cb) => { try { cb(arg); } catch (e) { console.error(e); } });

  window.wallpaperRegisterAudioListener = (cb) => audio.push(cb);
  window.wallpaperRegisterMediaPropertiesListener = (cb) => mediaProps.push(cb);
  window.wallpaperRegisterMediaPlaybackListener = (cb) => mediaPlayback.push(cb);
  window.wallpaperRegisterMediaThumbnailListener = (cb) => mediaThumb.push(cb);
  window.wallpaperRegisterMediaStatusListener = (cb) => { mediaStatus.push(cb); cb({ enabled: true }); };
  window.wallpaperRegisterMediaTimelineListener = () => {};

  const toWe = (v) => {
    if (typeof v === 'string') {
      const hex = /^#([0-9a-f]{6})$/i.exec(v);
      if (hex) {
        const n = parseInt(hex[1], 16);
        return [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((c) => (c / 255).toFixed(4)).join(' ');
      }
      if (/^-?\d+(\.\d+)?$/.test(v)) return Number(v);
    }
    return v;
  };
  const listener = () => window.wallpaperPropertyListener || {};

  let lastPaused = null;
  addEventListener('message', (e) => {
    const m = e.data;
    if (!m || m.type !== 'wallgates') return;
    const l = listener();
    switch (m.event) {
      case 'init':
        if (l.applyGeneralProperties) l.applyGeneralProperties({ fps: m.fps || 60 });
        // fallthrough
      case 'properties':
        if (m.values && l.applyUserProperties) {
          const props = {};
          for (const [k, v] of Object.entries(m.values)) props[k] = { value: toWe(v) };
          l.applyUserProperties(props);
        }
        // fallthrough
      case 'pause':
        if (typeof m.paused === 'boolean' && m.paused !== lastPaused) {
          lastPaused = m.paused;
          if (l.setPaused) l.setPaused(m.paused);
        }
        break;
      case 'audio': {
        // WE delivers 64 left + 64 right bins.
        const bands = m.bands || [];
        const arr = bands.concat(bands);
        call(audio, arr);
        break;
      }
      case 'media':
        call(mediaProps, { title: m.title || '', artist: m.artist || '', albumTitle: m.album || '', albumArtist: m.artist || '', genres: '', subTitle: '', contentType: 'music' });
        call(mediaPlayback, { state: m.playing ? 1 : m.title ? 2 : 0 });
        if (m.thumbnail) call(mediaThumb, { thumbnail: m.thumbnail, primaryColor: '#000000', secondaryColor: '#ffffff', tertiaryColor: '#888888', textColor: '#ffffff', highContrastColor: '#ffffff' });
        break;
    }
  });
})();
