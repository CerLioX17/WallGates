// Realistic mock feeds for the terminal wallpapers.
function sparkFrom(prev, last, n = 60) {
  const out = []; let v = prev;
  for (let i = 0; i < n; i++) { v += (last - prev) / n + (Math.sin(i * 0.9) + Math.cos(i * 0.37)) * prev * 0.0015; out.push(+v.toFixed(4)); }
  out[n - 1] = last; return out;
}
function q(id, symbol, name, ccy, prev, last, state = 'open', ex = 'Milan') {
  const change = last - prev;
  return { id, symbol, name, currency: ccy, exchange: ex, price: last, previousClose: prev, change, changePct: change / prev * 100,
    dayHigh: Math.max(prev, last) * 1.004, dayLow: Math.min(prev, last) * 0.996, state, spark: sparkFrom(prev, last), time: Date.now() / 1000 };
}
const markets = {
  updated: Math.floor(Date.now() / 1000), source: 'Yahoo Finance (delayed)', errors: [],
  fx: { USDEUR: 0.9223 },
  quotes: [
    q('IE00B4L5Y983', 'SWDA.MI', 'iShares Core MSCI World UCITS ETF', 'EUR', 104.10, 105.32),
    q('US0378331005', 'AAPL', 'Apple Inc.', 'USD', 225.10, 228.42, 'pre', 'NasdaqGS'),
    q('US5949181045', 'MSFT', 'Microsoft Corporation', 'USD', 436.60, 432.11, 'pre', 'NasdaqGS'),
    q('US67066G1040', 'NVDA', 'NVIDIA Corporation', 'USD', 136.90, 141.25, 'pre', 'NasdaqGS'),
    q('NL0010273215', 'ASML.AS', 'ASML Holding N.V.', 'EUR', 718.20, 712.40, 'open', 'Amsterdam'),
    q('IT0003132476', 'ENI.MI', 'Eni S.p.A.', 'EUR', 14.48, 14.62),
    q('IT0000072618', 'ISP.MI', 'Intesa Sanpaolo S.p.A.', 'EUR', 3.952, 3.981),
    q('^GSPC', '^GSPC', 'S&P 500', 'USD', 5864.6, 5891.3), q('^NDX', '^NDX', 'NASDAQ 100', 'USD', 20790, 20872),
    q('^DJI', '^DJI', 'Dow Jones', 'USD', 43820, 43711), q('^STOXX50E', '^STOXX50E', 'EURO STOXX 50', 'EUR', 4931, 4962),
    q('^GDAXI', '^GDAXI', 'DAX', 'EUR', 19720, 19810), q('FTSEMIB.MI', 'FTSEMIB.MI', 'FTSE MIB', 'EUR', 34320, 34512),
    q('EURUSD=X', 'EURUSD=X', 'EUR/USD', 'USD', 1.0861, 1.0842), q('BTC-EUR', 'BTC-EUR', 'Bitcoin EUR', 'EUR', 60110, 61230),
    q('GC=F', 'GC=F', 'Gold', 'USD', 2621, 2644),
  ],
};
function system(t) {
  const w = (a, b, f = 1) => a + (b - a) * (0.5 + 0.5 * Math.sin(t * f));
  const cores = Array.from({ length: 16 }, (_, i) => +w(4, 70, 0.7 + i * 0.13).toFixed(1));
  return {
    cpu: +w(18, 46, 0.5).toFixed(1), cpuName: 'AMD Ryzen 9 7950X 16-Core Processor', cores, cpuFreq: w(4300, 5450, 0.4), cpuTemp: w(52, 71, 0.3), cpuPower: w(48, 120, 0.4),
    memoryUsed: 19.4 * 2 ** 30, memoryTotal: 64 * 2 ** 30, memory: 30.3, swapUsed: 1.1 * 2 ** 30, swapTotal: 8 * 2 ** 30,
    gpu: { name: 'NVIDIA GeForce RTX 4080', usage: +w(8, 64, 0.6).toFixed(1), vramUsed: 6.2 * 2 ** 30, vramTotal: 16 * 2 ** 30, temp: w(44, 66, 0.25), fan: 1180, power: w(60, 210, 0.6) },
    disks: [{ name: 'Samsung 990 PRO', mount: 'C:\\', total: 1.82 * 2 ** 40, used: 1.07 * 2 ** 40 }, { name: 'Data', mount: 'D:\\', total: 3.64 * 2 ** 40, used: 2.1 * 2 ** 40 }],
    net: { rx: w(40e3, 9.6e6, 0.9), tx: w(10e3, 1.2e6, 0.7) },
    processes: [['chrome.exe', 6.2, 3.1e9, 24], ['Code.exe', 3.8, 1.4e9, 11], ['Discord.exe', 1.9, 6.1e8, 6], ['wallgates.exe', 0.7, 1.2e8, 1], ['explorer.exe', 0.6, 2.2e8, 1],
      ['Spotify.exe', 0.5, 3.4e8, 5], ['steam.exe', 0.3, 2.9e8, 3], ['MsMpEng.exe', 0.3, 2.4e8, 1], ['dwm.exe', 0.2, 1.6e8, 1], ['svchost.exe', 0.2, 9.1e8, 84]]
      .map(([name, cpu, memory, count]) => ({ name, cpu, memory, count })),
    memoryAvailable: 44.6 * 2 ** 30, ramTemp: w(41, 47, 0.2),
    memoryHogs: [{ name: 'chrome.exe', memory: 3.1e9, cpu: 6.2, count: 24 }],
    drives: [{ index: 0, model: 'Samsung SSD 990 PRO 2TB', letters: 'C:', temp: w(39, 52, 0.35), active: +w(1, 38, 1.1).toFixed(1), read: w(2e5, 3.1e8, 1.3), write: w(1e5, 9e7, 0.8) },
             { index: 1, model: 'WD_BLACK SN850X 4TB', letters: 'D:', temp: 41, active: 3.2, read: 1.2e6, write: 4e5 }],
    fans: [{ label: 'CPU', rpm: Math.round(w(980, 1450, 0.3)) }, { label: 'System 1', rpm: 820 }, { label: 'System 2', rpm: 790 }, { label: 'Pump', rpm: 2400 }],
    processCount: 312, uptime: 3 * 86400 + 5 * 3600 + 1240, host: 'WORKSTATION', os: 'Windows 11 Pro 26H2',
    sensors: [], sources: { temperatures: 'hwinfo' },
  };
}
function weather() {
  const d = (i) => new Date(Date.now() + i * 86400000).toISOString().slice(0, 10);
  const today = d(0);
  const hours = Array.from({ length: 24 }, (_, i) => { const h = (new Date().getHours() + i) % 24; return {
    time: new Date(Date.now() + i * 3600000).toISOString().slice(0, 13) + ':00', temp: 11 + 7 * Math.sin((h - 9) / 24 * 2 * Math.PI), rainChance: h > 17 && h < 23 ? 40 + h : 5, uv: Math.max(0, 5 * Math.sin((h - 6) / 12 * Math.PI)), code: 2, condition: 'cloudy' }; });
  return { location: 'Milano, Lombardia, Italia', temperature: 17.6, feelsLike: 16.9, humidity: 62, wind: 11, windDir: 220, uv: 3.4, pressure: 1017, clouds: 40,
    code: 2, condition: 'cloudy', description: 'Partly cloudy', isDay: true, high: 19, low: 9, uvMax: 4.1, rainChance: 20, units: '°C', windUnits: 'km/h',
    air: { aqi: 34, pm25: 9.4, pm10: 17.2 },
    days: [
      { date: today, code: 2, condition: 'cloudy', description: 'Partly cloudy', max: 19, min: 9, rainChance: 20, uvMax: 4.1, sunrise: today + 'T06:58', sunset: today + 'T16:59' },
      { date: d(1), code: 61, condition: 'rain', description: 'Rain', max: 15, min: 10, rainChance: 80, uvMax: 1.8 },
      { date: d(2), code: 0, condition: 'clear', description: 'Clear sky', max: 21, min: 8, rainChance: 0, uvMax: 5.6 },
      { date: d(3), code: 95, condition: 'storm', description: 'Thunderstorm', max: 16, min: 11, rainChance: 70, uvMax: 2.2 },
    ], hours };
}
const media = { title: 'Midnight City', artist: 'M83', album: "Hurry Up, We're Dreaming", playing: true, thumbnail: null };
module.exports = { markets, system, weather, media };
