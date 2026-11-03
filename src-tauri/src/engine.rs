//! The engine decides, for every wallpaper window ("slot"), what it shows and
//! how: which wallpaper (direct or from a playlist), paused / stopped / eco,
//! muted, frame cap, render scale — and pushes live data feeds to the
//! wallpapers that asked for them. It also watches the desktop and rebuilds the
//! windows after Explorer restarts or monitor changes.

use crate::config::{AppAction, Config, LayoutMode, PauseScope, PlaylistMode, PowerAction, Source};
use crate::desktop::{self, MonitorInfo, Observation, Rect, Slot, SPAN_ID};
use crate::feeds;
use crate::library::{self, Kind, Paths, WallpaperInfo};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

pub const STATE_EVENT: &str = "wf-state";
pub const UI_EVENT: &str = "wf-ui";

pub struct PlaylistRt {
    pub pos: usize,
    pub order: Vec<usize>,
    pub next_switch: Instant,
    pub signature: String,
}

#[derive(Default, Clone)]
pub struct Needs {
    pub audio: Vec<String>,
    pub mouse: Vec<(String, Rect)>,
    pub system: Vec<String>,
    pub media: Vec<String>,
    pub weather: Vec<String>,
    /// (label, instruments requested by that wallpaper)
    pub markets: Vec<(String, feeds::markets::Request)>,
}

#[derive(Default)]
pub struct Runtime {
    pub slots: Vec<Slot>,
    pub span: bool,
    pub obs: Observation,
    pub playlists: HashMap<String, PlaylistRt>,
    /// Bumped when a wallpaper's files are edited so hosts reload it.
    pub revisions: HashMap<String, u32>,
    /// Last runtime error reported by a host, per wallpaper id.
    pub errors: HashMap<String, String>,
    sent: HashMap<String, String>,
    sent_ui: String,
    pub needs: Needs,
    pub feed_cache: Map<String, Value>,
    pub update: Option<Value>,
    pub rebuilding: bool,
    last_sync: String,
    rng: u64,
}

pub struct AppState {
    config: Mutex<Config>,
    rt: Mutex<Runtime>,
    pub paths: Paths,
}

impl AppState {
    pub fn new(config: Config, paths: Paths) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(42)
            | 1;
        Self { config: Mutex::new(config), rt: Mutex::new(Runtime { rng: seed, ..Default::default() }), paths }
    }

    pub fn config(&self) -> MutexGuard<'_, Config> {
        self.config.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn rt(&self) -> MutexGuard<'_, Runtime> {
        self.rt.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Mutate, sanitize, persist and broadcast the config.
    pub fn update<R>(&self, app: &AppHandle, f: impl FnOnce(&mut Config) -> R) -> Result<R, String> {
        let (result, layout_changed) = {
            let mut cfg = self.config();
            let before = cfg.layout;
            let r = f(&mut cfg);
            cfg.sanitize();
            cfg.save(&self.paths.config_file)?;
            (r, before != cfg.layout)
        };
        if layout_changed {
            rebuild(app);
        } else {
            broadcast(app, false);
        }
        Ok(result)
    }
}

// ------------------------------------------------------------------ resolution

#[derive(Serialize, Clone)]
pub struct SlotState {
    pub label: String,
    pub monitor_id: String,
    pub index: usize,
    pub span: bool,
    pub wallpaper: Option<WallpaperInfo>,
    pub values: Map<String, Value>,
    pub paused: bool,
    /// Unload the wallpaper entirely (frees GPU memory).
    pub stopped: bool,
    pub eco: bool,
    pub muted: bool,
    pub volume: f32,
    pub fps_limit: u32,
    pub render_scale: f32,
    pub transition_ms: u32,
    pub revision: u32,
    pub reason: Option<String>,
    pub width: u32,
    pub height: u32,
}

fn rank(a: PowerAction) -> u8 {
    match a {
        PowerAction::None => 0,
        PowerAction::Eco => 1,
        PowerAction::Pause => 2,
        PowerAction::Stop => 3,
    }
}

fn now_minutes() -> u32 {
    use chrono::Timelike;
    let now = chrono::Local::now();
    now.hour() * 60 + now.minute()
}

fn sun(cfg: &Config) -> (u32, u32) {
    use chrono::{Datelike, Offset};
    let fallback = (7 * 60, 19 * 60);
    let Some(loc) = &cfg.integrations.location else { return fallback };
    let now = chrono::Local::now();
    let offset = now.offset().fix().local_minus_utc() / 60;
    feeds::sun::sun_times(loc.latitude, loc.longitude, now.ordinal(), offset).unwrap_or(fallback)
}

/// "07:30" / "sunrise" / "sunset" -> minutes after midnight.
pub fn parse_at(at: &str, sun: (u32, u32)) -> Option<u32> {
    match at.trim().to_ascii_lowercase().as_str() {
        "sunrise" => Some(sun.0),
        "sunset" => Some(sun.1),
        t => {
            let (h, m) = t.split_once(':')?;
            let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
            (h < 24 && m < 60).then_some(h * 60 + m)
        }
    }
}

/// Index of the time-of-day item active at `now` (latest start <= now, wrapping).
pub fn time_of_day_index(starts: &[Option<u32>], now: u32) -> Option<usize> {
    let valid: Vec<(usize, u32)> = starts.iter().enumerate().filter_map(|(i, s)| s.map(|s| (i, s))).collect();
    if valid.is_empty() {
        return None;
    }
    valid
        .iter()
        .filter(|(_, s)| *s <= now)
        .max_by_key(|(_, s)| *s)
        .or_else(|| valid.iter().max_by_key(|(_, s)| *s))
        .map(|(i, _)| *i)
}

fn resolve(cfg: &Config, rt: &Runtime, source: &Source) -> Option<String> {
    match source {
        Source::Wallpaper { id } => Some(id.clone()),
        Source::Playlist { id } => {
            let pl = cfg.playlist(id)?;
            if pl.items.is_empty() {
                return None;
            }
            let idx = match pl.mode {
                PlaylistMode::Interval => rt
                    .playlists
                    .get(id)
                    .and_then(|p| p.order.get(p.pos).copied())
                    .unwrap_or(0),
                PlaylistMode::TimeOfDay => {
                    let s = sun(cfg);
                    let starts: Vec<Option<u32>> =
                        pl.items.iter().map(|i| i.at.as_deref().and_then(|a| parse_at(a, s))).collect();
                    time_of_day_index(&starts, now_minutes()).unwrap_or(0)
                }
            };
            pl.items.get(idx.min(pl.items.len() - 1)).map(|i| i.wallpaper.clone())
        }
    }
}

fn exe_matches(rule: &str, exe: &str) -> bool {
    let rule = rule.trim().to_ascii_lowercase();
    let rule = rule.rsplit(['\\', '/']).next().unwrap_or(&rule);
    rule == exe || format!("{rule}.exe") == exe
}

fn compute_states(cfg: &Config, rt: &Runtime, paths: &Paths) -> Vec<SlotState> {
    let obs = &rt.obs;
    let perf = &cfg.performance;
    let mut cache: HashMap<String, Option<WallpaperInfo>> = HashMap::new();

    rt.slots
        .iter()
        .map(|slot| {
            let span = slot.monitor_id == SPAN_ID;
            let source = if span { cfg.default_source.as_ref() } else { cfg.source_for(&slot.monitor_id) };
            let wallpaper = source
                .and_then(|s| resolve(cfg, rt, s))
                .and_then(|id| cache.entry(id.clone()).or_insert_with(|| library::find(paths, &id)).clone());

            let hits = |rects: &[Rect]| match perf.pause_scope {
                PauseScope::All => !rects.is_empty(),
                PauseScope::Monitor => rects.iter().any(|r| r.intersects(&slot.rect)),
            };
            let mut action = PowerAction::None;
            let mut reason: Option<String> = None;
            let mut apply = |a: PowerAction, why: &str| {
                if rank(a) > rank(action) {
                    action = a;
                    reason = Some(why.to_string());
                }
            };
            if hits(&obs.covered) {
                apply(perf.fullscreen, "fullscreen");
            }
            if hits(&obs.maximized) {
                apply(perf.maximized, "maximized");
            }
            if obs.on_battery {
                apply(perf.battery, "battery");
            }
            let mut rule_mute = false;
            if let Some(exe) = &obs.foreground_exe {
                for rule in cfg.app_rules.iter().filter(|r| exe_matches(&r.exe, exe)) {
                    match rule.action {
                        AppAction::Pause => apply(PowerAction::Pause, &format!("app:{exe}")),
                        AppAction::Stop => apply(PowerAction::Stop, &format!("app:{exe}")),
                        AppAction::Eco => apply(PowerAction::Eco, &format!("app:{exe}")),
                        AppAction::Mute => rule_mute = true,
                    }
                }
            }
            if cfg.paused {
                apply(PowerAction::Pause, "manual");
            }

            let eco = action == PowerAction::Eco;
            let base_fps = if perf.fps_limit == 0 { 0 } else { perf.fps_limit };
            let fps_limit = if eco { if base_fps == 0 { 20 } else { base_fps.min(20) } } else { base_fps };
            let render_scale = if eco { perf.render_scale.min(0.5) } else { perf.render_scale };
            let muted = cfg.muted || rule_mute || (perf.mute_unless_desktop && !obs.desktop_focused);
            let values = wallpaper.as_ref().map(|w| w.values(cfg.properties.get(&w.id))).unwrap_or_default();
            let revision = wallpaper.as_ref().and_then(|w| rt.revisions.get(&w.id).copied()).unwrap_or(0);

            SlotState {
                label: slot.label.clone(),
                monitor_id: slot.monitor_id.clone(),
                index: slot.index,
                span,
                wallpaper,
                values,
                paused: rank(action) >= rank(PowerAction::Pause),
                stopped: action == PowerAction::Stop,
                eco,
                muted,
                volume: perf.volume,
                fps_limit,
                render_scale,
                transition_ms: cfg.transition_ms,
                revision,
                reason,
                width: slot.rect.width,
                height: slot.rect.height,
            }
        })
        .collect()
}

fn needs_of(states: &[SlotState], slots: &[Slot], cfg: &Config) -> Needs {
    let mut n = Needs::default();
    for (st, slot) in states.iter().zip(slots) {
        let Some(w) = &st.wallpaper else { continue };
        if st.paused {
            continue;
        }
        let m = &w.manifest;
        let has = |c: &str| m.has(c);
        if has("audio") {
            n.audio.push(st.label.clone());
        }
        if has("mouse") {
            n.mouse.push((st.label.clone(), slot.rect));
        }
        if has("system") {
            n.system.push(st.label.clone());
        }
        if has("media") {
            n.media.push(st.label.clone());
        }
        if has("weather") && cfg.integrations.weather && cfg.integrations.location.is_some() {
            n.weather.push(st.label.clone());
        }
        if has("markets") && cfg.integrations.markets {
            n.markets.push((st.label.clone(), feeds::markets::request_from(&st.values)));
        }
    }
    n
}

pub fn host_state_for(app: &AppHandle, label: &str) -> Option<SlotState> {
    let state = app.state::<AppState>();
    let cfg = state.config().clone();
    let rt = state.rt();
    compute_states(&cfg, &rt, &state.paths).into_iter().find(|s| s.label == label)
}

// ------------------------------------------------------------------ UI state

#[derive(Serialize)]
pub struct SlotSummary {
    pub label: String,
    pub monitor_id: String,
    pub wallpaper_id: Option<String>,
    pub wallpaper_name: Option<String>,
    pub paused: bool,
    pub stopped: bool,
    pub eco: bool,
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct UiState {
    pub config: Config,
    pub monitors: Vec<MonitorInfo>,
    pub slots: Vec<SlotSummary>,
    pub update: Option<Value>,
    pub on_battery: bool,
    pub errors: HashMap<String, String>,
    pub version: &'static str,
}

pub fn ui_state(app: &AppHandle) -> UiState {
    let state = app.state::<AppState>();
    let cfg = state.config().clone();
    let rt = state.rt();
    let states = compute_states(&cfg, &rt, &state.paths);
    UiState {
        monitors: desktop::monitors(app),
        slots: states
            .iter()
            .map(|s| SlotSummary {
                label: s.label.clone(),
                monitor_id: s.monitor_id.clone(),
                wallpaper_id: s.wallpaper.as_ref().map(|w| w.id.clone()),
                wallpaper_name: s.wallpaper.as_ref().map(|w| w.manifest.name.clone()),
                paused: s.paused,
                stopped: s.stopped,
                eco: s.eco,
                reason: s.reason.clone(),
            })
            .collect(),
        update: rt.update.clone(),
        on_battery: rt.obs.on_battery,
        errors: rt.errors.clone(),
        version: env!("CARGO_PKG_VERSION"),
        config: cfg,
    }
}

// ------------------------------------------------------------------ broadcast

/// Push changed per-slot states to the wallpaper windows and the UI state to
/// the settings window. With `force`, everything is re-sent.
pub fn broadcast(app: &AppHandle, force: bool) {
    let state = app.state::<AppState>();
    let cfg = state.config().clone();
    let (to_send, sync) = {
        let mut rt = state.rt();
        let states = compute_states(&cfg, &rt, &state.paths);
        rt.needs = needs_of(&states, &rt.slots, &cfg);
        let mut out = Vec::new();
        for st in &states {
            let json = serde_json::to_string(st).unwrap_or_default();
            if force || rt.sent.get(&st.label) != Some(&json) {
                rt.sent.insert(st.label.clone(), json);
                out.push(st.clone());
            }
        }
        // Lock screen / desktop sync follows the primary (or spanning) slot.
        let sync = if cfg.sync_lock_screen || cfg.sync_desktop {
            let monitors = desktop::monitors(app);
            let primary = monitors.iter().find(|m| m.primary).map(|m| m.id.clone());
            states
                .iter()
                .find(|s| s.span || Some(&s.monitor_id) == primary.as_ref())
                .or(states.first())
                .and_then(|s| {
                    let w = s.wallpaper.as_ref()?;
                    let sig = format!("{}|{}|{}", w.id, serde_json::to_string(&s.values).ok()?, s.revision);
                    (sig != rt.last_sync).then(|| {
                        rt.last_sync = sig;
                        (s.label.clone(), w.clone())
                    })
                })
        } else {
            None
        };
        (out, sync)
    };
    for st in to_send {
        let _ = app.emit_to(st.label.as_str(), STATE_EVENT, &st);
    }
    emit_ui(app, force);
    if let Some((label, wallpaper)) = sync {
        request_sync(app, label, wallpaper);
    }
}

pub fn emit_ui(app: &AppHandle, force: bool) {
    let ui = ui_state(app);
    let json = serde_json::to_string(&ui).unwrap_or_default();
    let state = app.state::<AppState>();
    {
        let mut rt = state.rt();
        if !force && rt.sent_ui == json {
            return;
        }
        rt.sent_ui = json;
    }
    let _ = app.emit_to("main", UI_EVENT, &ui);
}

// ------------------------------------------------------------------ lock screen / desktop sync

fn request_sync(app: &AppHandle, label: String, wallpaper: WallpaperInfo) {
    let app = app.clone();
    std::thread::spawn(move || {
        if wallpaper.manifest.kind == Kind::Image {
            let path = wallpaper.dir(&app.state::<AppState>().paths).join(&wallpaper.manifest.entry);
            apply_sync(&app, &path);
        } else {
            // Let the transition finish, then ask the host for a frame.
            std::thread::sleep(Duration::from_millis(2500));
            let _ = app.emit_to(label.as_str(), "wf-capture", ());
        }
    });
}

/// Called with a captured PNG (or an image wallpaper's file).
pub fn apply_sync(app: &AppHandle, image: &std::path::Path) {
    let state = app.state::<AppState>();
    let (lock, desk) = {
        let c = state.config();
        (c.sync_lock_screen, c.sync_desktop)
    };
    if lock {
        if let Err(e) = desktop::set_lock_screen(image) {
            desktop::log(app, &format!("lock screen sync failed: {e}"));
        }
    }
    if desk {
        if let Err(e) = desktop::set_static_wallpaper(image) {
            desktop::log(app, &format!("desktop sync failed: {e}"));
        }
    }
}

// ------------------------------------------------------------------ playlists

fn shuffle(order: &mut [usize], rng: &mut u64) {
    for i in (1..order.len()).rev() {
        *rng ^= *rng << 13;
        *rng ^= *rng >> 7;
        *rng ^= *rng << 17;
        let j = (*rng % (i as u64 + 1)) as usize;
        order.swap(i, j);
    }
}

/// Advance interval playlists that are due. Returns true if anything changed.
fn tick_playlists(cfg: &Config, rt: &mut Runtime, now: Instant) -> bool {
    let mut changed = false;
    for pl in cfg.playlists.iter().filter(|p| p.mode == PlaylistMode::Interval) {
        if pl.items.is_empty() {
            rt.playlists.remove(&pl.id);
            continue;
        }
        let signature = format!(
            "{}|{}|{}",
            pl.shuffle,
            pl.interval_minutes,
            pl.items.iter().map(|i| i.wallpaper.as_str()).collect::<Vec<_>>().join(",")
        );
        let interval = Duration::from_secs(pl.interval_minutes as u64 * 60);
        let mut rng = rt.rng;
        let entry = rt.playlists.entry(pl.id.clone()).or_insert_with(|| PlaylistRt {
            pos: 0,
            order: Vec::new(),
            next_switch: now + interval,
            signature: String::new(),
        });
        if entry.signature != signature {
            entry.order = (0..pl.items.len()).collect();
            if pl.shuffle {
                shuffle(&mut entry.order, &mut rng);
            }
            entry.pos = 0;
            entry.next_switch = now + interval;
            entry.signature = signature;
            changed = true;
        } else if now >= entry.next_switch {
            entry.pos += 1;
            if entry.pos >= entry.order.len() {
                entry.pos = 0;
                if pl.shuffle {
                    shuffle(&mut entry.order, &mut rng);
                }
            }
            entry.next_switch = now + interval;
            changed = true;
        }
        rt.rng = rng;
    }
    changed
}

/// "Next" action (tray / hotkey): advance playlists in use, otherwise move the
/// default wallpaper to the next one in the library.
pub fn next(app: &AppHandle) {
    let state = app.state::<AppState>();
    let cfg = state.config().clone();
    let mut used: Vec<String> = cfg
        .monitors
        .values()
        .chain(cfg.default_source.iter())
        .filter_map(|s| match s {
            Source::Playlist { id } => Some(id.clone()),
            _ => None,
        })
        .collect();
    used.sort();
    used.dedup();
    if !used.is_empty() {
        {
            let mut rt = state.rt();
            for id in &used {
                if let Some(p) = rt.playlists.get_mut(id) {
                    p.next_switch = Instant::now();
                }
            }
            tick_playlists(&cfg, &mut rt, Instant::now());
        }
        broadcast(app, false);
        return;
    }
    let all = library::scan(&state.paths);
    if all.is_empty() {
        return;
    }
    let current = match &cfg.default_source {
        Some(Source::Wallpaper { id }) => all.iter().position(|w| &w.id == id),
        _ => None,
    };
    let next = &all[current.map(|i| (i + 1) % all.len()).unwrap_or(0)];
    let id = next.id.clone();
    let _ = state.update(app, |c| c.default_source = Some(Source::Wallpaper { id }));
}

// ------------------------------------------------------------------ windows lifecycle

/// Recreate the wallpaper windows on a background thread (window creation
/// must not block the main thread on Windows).
pub fn rebuild(app: &AppHandle) {
    let state = app.state::<AppState>();
    {
        let mut rt = state.rt();
        if rt.rebuilding {
            return;
        }
        rt.rebuilding = true;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let span = state.config().layout == LayoutMode::Span;
        let result = desktop::create_windows(&app, span);
        {
            let mut rt = state.rt();
            match result {
                Ok(slots) => {
                    rt.slots = slots;
                    rt.span = span;
                }
                Err(e) => {
                    rt.slots.clear();
                    desktop::log(&app, &format!("could not create wallpaper windows: {e}"));
                }
            }
            rt.sent.clear();
            rt.last_sync.clear();
            rt.rebuilding = false;
        }
        broadcast(&app, true);
    });
}

// ------------------------------------------------------------------ loops

fn emit_to_labels<S: Serialize + Clone>(app: &AppHandle, labels: &[String], event: &str, payload: S) {
    for l in labels {
        let _ = app.emit_to(l.as_str(), event, payload.clone());
    }
}

/// Slow loop: desktop observation, playlists, watchdog, system/media/weather feeds.
pub fn spawn_slow_loop(app: AppHandle) {
    std::thread::Builder::new()
        .name("wallgates-engine".into())
        .spawn(move || {
            let mut tick: u64 = 0;
            let mut system = feeds::system::SystemFeed::default();
            let markets = std::sync::Arc::new(std::sync::Mutex::new(feeds::markets::Markets::new(&app.state::<AppState>().paths.data)));
            let markets_busy = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let mut last_markets: Option<Instant> = None;
            let mut markets_key = String::new();
            let mut media = feeds::media::MediaFeed::default();
            let mut last_weather: Option<Instant> = None;
            let mut weather_key = String::new();
            let mut last_rebuild_check = Instant::now();
            // Back-off: never re-attach more than 3 times per minute (avoids flicker
            // loops on unusual desktop configurations).
            let mut recent_rebuilds: Vec<Instant> = Vec::new();
            loop {
                std::thread::sleep(Duration::from_millis(250));
                tick += 1;
                let state = app.state::<AppState>();

                // Every 1.5 s: observe the desktop, advance playlists, push changes.
                if tick % 6 == 0 {
                    let obs = desktop::observe();
                    let cfg = state.config().clone();
                    {
                        let mut rt = state.rt();
                        rt.obs = obs;
                        tick_playlists(&cfg, &mut rt, Instant::now());
                    }
                    broadcast(&app, false);
                }

                // Every 2 s: watchdog (Explorer restart, monitors changed, resume from sleep).
                if last_rebuild_check.elapsed() >= Duration::from_secs(2) {
                    last_rebuild_check = Instant::now();
                    let (valid, rebuilding) = {
                        let rt = state.rt();
                        (rt.rebuilding || desktop::layout_still_valid(&app, &rt.slots, rt.span), rt.rebuilding)
                    };
                    recent_rebuilds.retain(|t| t.elapsed() < Duration::from_secs(60));
                    if !valid && !rebuilding && recent_rebuilds.len() < 3 {
                        recent_rebuilds.push(Instant::now());
                        desktop::log(&app, "desktop changed (Explorer restart / monitors / resume): re-attaching");
                        rebuild(&app);
                    }
                }

                let needs = state.rt().needs.clone();

                // Every 2 s: system + media.
                if tick % 8 == 0 {
                    if !needs.system.is_empty() {
                        let data = system.sample();
                        state.rt().feed_cache.insert("system".into(), data.clone());
                        emit_to_labels(&app, &needs.system, "wf-feed", json!({ "kind": "system", "data": data }));
                    }
                    if !needs.media.is_empty() {
                        if let Some(data) = media.sample() {
                            let changed = state.rt().feed_cache.get("media") != Some(&data);
                            if changed {
                                state.rt().feed_cache.insert("media".into(), data.clone());
                                emit_to_labels(&app, &needs.media, "wf-feed", json!({ "kind": "media", "data": data }));
                            }
                        }
                    }
                }

                // Markets: at the wallpaper's refresh interval (min 15 s), off-thread.
                if !needs.markets.is_empty() {
                    let reqs: Vec<feeds::markets::Request> = needs.markets.iter().map(|(_, r)| r.clone()).collect();
                    let refresh = reqs.iter().map(|r| r.refresh_secs).min().unwrap_or(60);
                    let key = format!("{:?}", reqs.iter().map(|r| (&r.ids, &r.market, &r.currency)).collect::<Vec<_>>());
                    let due = last_markets.is_none_or(|t| t.elapsed() >= Duration::from_secs(refresh)) || key != markets_key;
                    if due && !markets_busy.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        last_markets = Some(Instant::now());
                        markets_key = key;
                        let (app2, m, busy) = (app.clone(), markets.clone(), markets_busy.clone());
                        std::thread::spawn(move || {
                            let data = m.lock().map(|mut m| m.fetch(&reqs)).unwrap_or(Value::Null);
                            busy.store(false, std::sync::atomic::Ordering::SeqCst);
                            if data.is_null() {
                                return;
                            }
                            let st = app2.state::<AppState>();
                            st.rt().feed_cache.insert("markets".into(), data.clone());
                            let labels: Vec<String> = st.rt().needs.markets.iter().map(|(l, _)| l.clone()).collect();
                            emit_to_labels(&app2, &labels, "wf-feed", json!({ "kind": "markets", "data": data }));
                        });
                    }
                }

                // Weather: every 20 min (or when the location changes), off-thread.
                if !needs.weather.is_empty() {
                    let (loc, f) = {
                        let c = state.config();
                        (c.integrations.location.clone(), c.integrations.fahrenheit)
                    };
                    if let Some(loc) = loc {
                        let key = format!("{}|{}|{}", loc.latitude, loc.longitude, f);
                        let due = last_weather.is_none_or(|t| t.elapsed() > Duration::from_secs(20 * 60)) || key != weather_key;
                        if due {
                            last_weather = Some(Instant::now());
                            weather_key = key;
                            let app2 = app.clone();
                            std::thread::spawn(move || match feeds::weather::fetch(&loc, f) {
                                Ok(data) => {
                                    let st = app2.state::<AppState>();
                                    st.rt().feed_cache.insert("weather".into(), data.clone());
                                    let labels = st.rt().needs.weather.clone();
                                    emit_to_labels(&app2, &labels, "wf-feed", json!({ "kind": "weather", "data": data }));
                                }
                                Err(e) => desktop::log(&app2, &format!("weather: {e}")),
                            });
                        }
                    }
                }
            }
        })
        .expect("engine thread");
}

/// Fast loop (~30 Hz): audio spectrum and cursor position.
pub fn spawn_fast_loop(app: AppHandle) {
    std::thread::Builder::new()
        .name("wallgates-realtime".into())
        .spawn(move || {
            let mut capture: Option<feeds::audio::AudioCapture> = None;
            let mut idle_since: Option<Instant> = None;
            let mut last_cursor: Option<(i32, i32)> = None;
            let mut silent_frames = 0u32;
            loop {
                std::thread::sleep(Duration::from_millis(33));
                let state = app.state::<AppState>();
                let (needs, desktop_focused) = {
                    let rt = state.rt();
                    (rt.needs.clone(), rt.obs.desktop_focused)
                };

                if needs.audio.is_empty() {
                    if capture.is_some() && idle_since.get_or_insert_with(Instant::now).elapsed() > Duration::from_secs(5) {
                        capture = None;
                    }
                } else {
                    idle_since = None;
                    if capture.is_none() {
                        capture = feeds::audio::AudioCapture::start();
                    }
                    if let Some(c) = &capture {
                        let (bands, level) = c.snapshot();
                        let quiet = level < 0.002 && bands.iter().all(|b| *b < 0.01);
                        silent_frames = if quiet { silent_frames + 1 } else { 0 };
                        // Keep sending a few frames of silence, then go quiet to save IPC.
                        if silent_frames < 15 {
                            let bands: Vec<f32> = bands.iter().map(|b| (b * 1000.0).round() / 1000.0).collect();
                            emit_to_labels(&app, &needs.audio, "wf-audio", json!({ "bands": bands, "level": (level * 1000.0).round() / 1000.0 }));
                        }
                    }
                }

                if !needs.mouse.is_empty() {
                    if let Some(pos) = desktop::cursor() {
                        if Some(pos) != last_cursor {
                            last_cursor = Some(pos);
                            for (label, r) in &needs.mouse {
                                let x = (pos.0 - r.x) as f64 / r.width.max(1) as f64;
                                let y = (pos.1 - r.y) as f64 / r.height.max(1) as f64;
                                let _ = app.emit_to(
                                    label.as_str(),
                                    "wf-mouse",
                                    json!({ "x": x, "y": y, "inside": r.contains_point(pos.0, pos.1), "desktop": desktop_focused }),
                                );
                            }
                        }
                    }
                }
            }
        })
        .expect("realtime thread");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_of_day() {
        let starts = [Some(7 * 60), Some(12 * 60), Some(19 * 60)];
        assert_eq!(time_of_day_index(&starts, 8 * 60), Some(0));
        assert_eq!(time_of_day_index(&starts, 13 * 60), Some(1));
        assert_eq!(time_of_day_index(&starts, 23 * 60), Some(2));
        // Before the first start of the day -> last item of the previous day.
        assert_eq!(time_of_day_index(&starts, 3 * 60), Some(2));
        assert_eq!(time_of_day_index(&[None, None], 100), None);
    }

    #[test]
    fn parse_times() {
        assert_eq!(parse_at("07:30", (0, 0)), Some(450));
        assert_eq!(parse_at("sunset", (400, 1200)), Some(1200));
        assert_eq!(parse_at("25:00", (0, 0)), None);
    }

    #[test]
    fn exe_rules() {
        assert!(exe_matches("Game.exe", "game.exe"));
        assert!(exe_matches("game", "game.exe"));
        assert!(exe_matches("C:\\Games\\game.exe", "game.exe"));
        assert!(!exe_matches("gam", "game.exe"));
    }

    #[test]
    fn shuffle_is_permutation() {
        let mut v: Vec<usize> = (0..20).collect();
        let mut rng = 12345;
        shuffle(&mut v, &mut rng);
        let mut s = v.clone();
        s.sort();
        assert_eq!(s, (0..20).collect::<Vec<_>>());
    }
}
