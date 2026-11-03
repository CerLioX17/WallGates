mod config;
mod desktop;
mod engine;
mod feeds;
mod library;
mod online;
mod package;
mod protocol;
mod sensors;

use base64::Engine as _;
use config::{Config, Location, Playlist, Source};
use engine::{AppState, SlotState, UiState};
use library::WallpaperInfo;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent, State, WebviewWindow, WindowEvent};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_opener::OpenerExt;

const AUTOSTART_ARG: &str = "--autostart";

// ---------------------------------------------------------------- i18n (tray & dialogs)

fn language(cfg: &Config) -> &'static str {
    match cfg.language.as_str() {
        "it" => "it",
        "en" => "en",
        _ => {
            let sys = sys_locale::get_locale().unwrap_or_default().to_ascii_lowercase();
            if sys.starts_with("it") {
                "it"
            } else {
                "en"
            }
        }
    }
}

fn tr(lang: &str, key: &str) -> &'static str {
    let it = lang == "it";
    match key {
        "open" => if it { "Apri Wallgates" } else { "Open Wallgates" },
        "pause" => if it { "Pausa / Riprendi" } else { "Pause / Resume" },
        "next" => if it { "Sfondo successivo" } else { "Next wallpaper" },
        "mute" => if it { "Audio sì / no" } else { "Mute / Unmute" },
        "reattach" => if it { "Ricollega al desktop" } else { "Re-attach to desktop" },
        "quit" => if it { "Esci" } else { "Quit" },
        "pick_media" => if it { "Importa immagine o video" } else { "Import image or video" },
        "pick_folder" => if it { "Importa cartella wallpaper" } else { "Import wallpaper folder" },
        "pick_package" => if it { "Importa pacchetto .wallgates" } else { "Import .wallgates package" },
        "pick_we" => if it {
            "Cartella di un progetto Wallpaper Engine (o la cartella workshop 431960)"
        } else {
            "Wallpaper Engine project folder (or the 431960 workshop folder)"
        },
        "export" => if it { "Esporta wallpaper" } else { "Export wallpaper" },
        _ => "",
    }
}

// ---------------------------------------------------------------- commands: state

#[tauri::command]
fn ui_state(app: AppHandle) -> UiState {
    engine::ui_state(&app)
}

#[tauri::command]
fn list_wallpapers(state: State<AppState>) -> Vec<WallpaperInfo> {
    library::scan(&state.paths)
}

#[tauri::command]
fn host_state(app: AppHandle, window: WebviewWindow) -> Option<SlotState> {
    engine::host_state_for(&app, window.label())
}

#[tauri::command]
fn feed_snapshot(state: State<AppState>) -> Map<String, Value> {
    state.rt().feed_cache.clone()
}

/// target = "default" or a monitor id. `source: null` removes a monitor override.
#[tauri::command]
fn set_source(app: AppHandle, state: State<AppState>, target: String, source: Option<Source>) -> Result<(), String> {
    match &source {
        Some(Source::Wallpaper { id }) if library::find(&state.paths, id).is_none() => {
            return Err(format!("wallpaper not found: {id}"))
        }
        Some(Source::Playlist { id }) if state.config().playlist(id).is_none() => {
            return Err(format!("playlist not found: {id}"))
        }
        _ => {}
    }
    state.update(&app, |cfg| {
        if target == "default" {
            if source.is_some() {
                cfg.default_source = source;
            }
        } else {
            match source {
                Some(s) => {
                    cfg.monitors.insert(target, s);
                }
                None => {
                    cfg.monitors.remove(&target);
                }
            }
        }
    })
}

#[tauri::command]
fn set_property(app: AppHandle, state: State<AppState>, id: String, key: String, value: Value) -> Result<(), String> {
    let wallpaper = library::find(&state.paths, &id).ok_or("wallpaper not found")?;
    if !wallpaper.manifest.properties.contains_key(&key) {
        return Err(format!("unknown property: {key}"));
    }
    state.update(&app, |cfg| {
        cfg.properties.entry(id).or_default().insert(key, value);
    })
}

#[tauri::command]
fn reset_properties(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    state.update(&app, |cfg| {
        cfg.properties.remove(&id);
    })
}

fn merge(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(b), Value::Object(p)) => {
            for (k, v) in p {
                if v.is_object() && b.get(k).is_some_and(Value::is_object) {
                    merge(b.get_mut(k).unwrap(), v);
                } else {
                    b.insert(k.clone(), v.clone());
                }
            }
        }
        (b, p) => *b = p.clone(),
    }
}

/// Deep-merge a partial config from the UI (settings pages).
#[tauri::command]
fn update_config(app: AppHandle, state: State<AppState>, patch: Value) -> Result<(), String> {
    let lang_before = language(&state.config()).to_string();
    let hotkeys_before = serde_json::to_string(&state.config().hotkeys).unwrap_or_default();
    state.update(&app, |cfg| -> Result<(), String> {
        let mut v = serde_json::to_value(&*cfg).map_err(|e| e.to_string())?;
        merge(&mut v, &patch);
        let mut next: Config = serde_json::from_value(v).map_err(|e| e.to_string())?;
        next.version = config::CONFIG_VERSION;
        *cfg = next;
        Ok(())
    })??;
    if language(&state.config()) != lang_before {
        let _ = build_tray(&app);
    }
    if serde_json::to_string(&state.config().hotkeys).unwrap_or_default() != hotkeys_before {
        register_hotkeys(&app);
    }
    Ok(())
}

#[tauri::command]
fn save_playlist(app: AppHandle, state: State<AppState>, mut playlist: Playlist) -> Result<String, String> {
    if playlist.id.is_empty() {
        playlist.id = format!(
            "pl-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
        );
    }
    playlist.items.retain(|i| library::find(&state.paths, &i.wallpaper).is_some());
    let id = playlist.id.clone();
    state.update(&app, |cfg| match cfg.playlists.iter_mut().find(|p| p.id == playlist.id) {
        Some(p) => *p = playlist,
        None => cfg.playlists.push(playlist),
    })?;
    Ok(id)
}

#[tauri::command]
fn delete_playlist(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    state.update(&app, |cfg| {
        cfg.playlists.retain(|p| p.id != id);
        let gone = |s: &Source| matches!(s, Source::Playlist { id: p } if *p == id);
        cfg.monitors.retain(|_, s| !gone(s));
        if cfg.default_source.as_ref().is_some_and(gone) {
            cfg.default_source = Some(Source::Wallpaper { id: "builtin/aurora".into() });
        }
    })
}

#[tauri::command]
fn next_wallpaper(app: AppHandle) {
    engine::next(&app);
}

#[tauri::command]
fn toggle_favorite(app: AppHandle, state: State<AppState>, id: String) -> Result<bool, String> {
    state.update(&app, |cfg| {
        if let Some(pos) = cfg.favorites.iter().position(|f| f == &id) {
            cfg.favorites.remove(pos);
            false
        } else {
            cfg.favorites.push(id);
            true
        }
    })
}

// ---------------------------------------------------------------- commands: import / export

fn import_one(paths: &library::Paths, path: &Path, report: &mut (Vec<String>, Vec<String>)) {
    let result: Result<Vec<String>, String> = if path.is_dir() {
        if path.join(library::MANIFEST).is_file() {
            library::import_folder(paths, path).map(|id| vec![id])
        } else {
            package::import_we(paths, path).map(|r| {
                report.1.extend(r.skipped);
                r.imported
            })
        }
    } else {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        match ext.as_str() {
            "wallgates" | "zip" => package::import_package(paths, path).map(|id| vec![id]),
            "json" if path.file_name().is_some_and(|n| n == "project.json") => {
                package::import_we(paths, path.parent().unwrap_or(path)).map(|r| {
                    report.1.extend(r.skipped);
                    r.imported
                })
            }
            _ => library::import_file(paths, path).map(|id| vec![id]),
        }
    };
    match result {
        Ok(ids) => report.0.extend(ids),
        Err(e) => report.1.push(format!("{}: {e}", path.file_name().unwrap_or_default().to_string_lossy())),
    }
}

fn finish_import(app: &AppHandle, report: (Vec<String>, Vec<String>)) -> Value {
    let state = app.state::<AppState>();
    if report.0.len() == 1 {
        let id = report.0[0].clone();
        let _ = state.update(app, |cfg| cfg.default_source = Some(Source::Wallpaper { id }));
    } else {
        engine::emit_ui(app, true);
    }
    json!({ "imported": report.0, "skipped": report.1 })
}

/// Import from a native file/folder picker. kind: media | folder | package | wallpaper_engine
#[tauri::command]
async fn import_dialog(app: AppHandle, kind: String) -> Result<Value, String> {
    let lang = language(&app.state::<AppState>().config()).to_string();
    let dialog = app.dialog().file();
    let picked: Vec<PathBuf> = match kind.as_str() {
        "media" => dialog
            .set_title(tr(&lang, "pick_media"))
            .add_filter("Images / videos", library::MEDIA_EXTENSIONS)
            .blocking_pick_files()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|f| f.into_path().ok())
            .collect(),
        "package" => dialog
            .set_title(tr(&lang, "pick_package"))
            .add_filter("Wallgates", &["wallgates", "zip"])
            .blocking_pick_files()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|f| f.into_path().ok())
            .collect(),
        "folder" => dialog.set_title(tr(&lang, "pick_folder")).blocking_pick_folder().and_then(|f| f.into_path().ok()).into_iter().collect(),
        "wallpaper_engine" => dialog.set_title(tr(&lang, "pick_we")).blocking_pick_folder().and_then(|f| f.into_path().ok()).into_iter().collect(),
        _ => return Err("unknown import kind".into()),
    };
    if picked.is_empty() {
        return Ok(json!({ "imported": [], "skipped": [], "cancelled": true }));
    }
    let state = app.state::<AppState>();
    let mut report = (Vec::new(), Vec::new());
    for p in &picked {
        if kind == "wallpaper_engine" {
            match package::import_we(&state.paths, p) {
                Ok(r) => {
                    report.0.extend(r.imported);
                    report.1.extend(r.skipped);
                }
                Err(e) => report.1.push(e),
            }
        } else {
            import_one(&state.paths, p, &mut report);
        }
    }
    Ok(finish_import(&app, report))
}

/// Drag & drop, file association and "open with": detects the type by itself.
#[tauri::command]
async fn import_paths(app: AppHandle, paths: Vec<String>) -> Result<Value, String> {
    let state = app.state::<AppState>();
    let mut report = (Vec::new(), Vec::new());
    for p in paths.iter().take(500) {
        import_one(&state.paths, Path::new(p), &mut report);
    }
    Ok(finish_import(&app, report))
}

#[tauri::command]
async fn export_wallpaper(app: AppHandle, id: String) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let info = library::find(&state.paths, &id).ok_or("wallpaper not found")?;
    let lang = language(&state.config()).to_string();
    let Some(dest) = app
        .dialog()
        .file()
        .set_title(tr(&lang, "export"))
        .set_file_name(format!("{}.wallgates", library::slugify(&info.manifest.name)))
        .add_filter("Wallgates", &["wallgates"])
        .blocking_save_file()
    else {
        return Ok(false);
    };
    let dest = dest.into_path().map_err(|e| e.to_string())?;
    package::export(&state.paths, &id, &dest)?;
    Ok(true)
}

#[tauri::command]
fn delete_wallpaper(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    library::delete(&state.paths, &id)?;
    state.update(&app, |cfg| cfg.forget_wallpaper(&id))
}

#[tauri::command]
fn duplicate_wallpaper(app: AppHandle, state: State<AppState>, id: String) -> Result<String, String> {
    let new_id = library::duplicate(&state.paths, &id)?;
    engine::emit_ui(&app, true);
    Ok(new_id)
}

#[tauri::command]
fn create_wallpaper(app: AppHandle, state: State<AppState>, template: String, name: String) -> Result<String, String> {
    let id = library::create_from_template(&state.paths, &template, &name)?;
    let id2 = id.clone();
    state.update(&app, |cfg| cfg.default_source = Some(Source::Wallpaper { id: id2 }))?;
    Ok(id)
}

// ---------------------------------------------------------------- commands: editor

#[tauri::command]
fn editor_files(state: State<AppState>, id: String) -> Result<Vec<String>, String> {
    library::list_text_files(&state.paths, &id)
}

#[tauri::command]
fn editor_read(state: State<AppState>, id: String, path: String) -> Result<String, String> {
    library::read_text(&state.paths, &id, &path)
}

#[tauri::command]
fn editor_write(app: AppHandle, state: State<AppState>, id: String, path: String, content: String) -> Result<(), String> {
    library::write_text(&state.paths, &id, &path, &content)?;
    {
        let mut rt = state.rt();
        *rt.revisions.entry(id.clone()).or_insert(0) += 1;
        rt.errors.remove(&id);
    }
    engine::broadcast(&app, false);
    Ok(())
}

/// Hosts report shader compile errors / script errors here (shown in the editor).
#[tauri::command]
fn report_error(app: AppHandle, state: State<AppState>, id: String, message: String) {
    let message: String = message.chars().take(4000).collect();
    let changed = {
        let mut rt = state.rt();
        if message.is_empty() {
            rt.errors.remove(&id).is_some()
        } else {
            rt.errors.insert(id, message.clone()) != Some(message)
        }
    };
    if changed {
        engine::emit_ui(&app, false);
    }
}

// ---------------------------------------------------------------- commands: system

#[tauri::command]
fn open_folder(app: AppHandle, state: State<AppState>, which: String) -> Result<(), String> {
    let dir = match which.as_str() {
        "library" => state.paths.user.clone(),
        "logs" => state.paths.data.clone(),
        _ => return Err("unknown folder".into()),
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("only https links can be opened".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn reapply(app: AppHandle) {
    engine::rebuild(&app);
}

#[tauri::command]
fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let al = app.autolaunch();
    if enabled { al.enable() } else { al.disable() }.map_err(|e| e.to_string())?;
    Ok(al.is_enabled().unwrap_or(false))
}

#[tauri::command]
async fn geocode(query: String, language: String) -> Result<Vec<Location>, String> {
    let lang = if language == "it" { "it" } else { "en" };
    tauri::async_runtime::spawn_blocking(move || feeds::weather::geocode(&query, lang))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn gallery_index(state: State<'_, AppState>) -> Result<Value, String> {
    if !state.config().integrations.gallery {
        return Err("gallery disabled in settings".into());
    }
    tauri::async_runtime::spawn_blocking(online::gallery_index).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn gallery_install(app: AppHandle, id: String) -> Result<String, String> {
    let paths = app.state::<AppState>().paths.clone();
    let new_id = tauri::async_runtime::spawn_blocking(move || online::gallery_install(&paths, &id))
        .await
        .map_err(|e| e.to_string())??;
    let id2 = new_id.clone();
    app.state::<AppState>().update(&app, |cfg| cfg.default_source = Some(Source::Wallpaper { id: id2 }))?;
    Ok(new_id)
}

#[tauri::command]
async fn check_update(app: AppHandle) -> Result<Option<Value>, String> {
    let update = tauri::async_runtime::spawn_blocking(online::check_update).await.map_err(|e| e.to_string())??;
    app.state::<AppState>().rt().update = update.clone();
    engine::emit_ui(&app, false);
    Ok(update)
}

fn sensors_helper(app: &AppHandle) -> Option<PathBuf> {
    app.path().resource_dir().ok().map(|d| d.join("sensors").join("WallgatesSensors.exe"))
}

#[tauri::command]
fn sensors_status(app: AppHandle, state: State<AppState>) -> Value {
    sensors::status(sensors_helper(&app).as_deref(), &state.paths.data)
}

/// One UAC prompt: installs PawnIO (if missing) and the hidden sensor task.
#[tauri::command]
async fn sensors_enable(app: AppHandle) -> Result<Value, String> {
    let helper = sensors_helper(&app).ok_or("installation folder not found")?;
    let data = app.state::<AppState>().paths.data.clone();
    let d2 = data.clone();
    tauri::async_runtime::spawn_blocking(move || sensors::enable(&helper, &d2)).await.map_err(|e| e.to_string())??;
    desktop::log(&app, "advanced sensors enabled");
    Ok(sensors::status(sensors_helper(&app).as_deref(), &data))
}

#[tauri::command]
async fn sensors_disable(app: AppHandle) -> Result<Value, String> {
    let helper = sensors_helper(&app).ok_or("installation folder not found")?;
    let data = app.state::<AppState>().paths.data.clone();
    let d2 = data.clone();
    tauri::async_runtime::spawn_blocking(move || sensors::disable(&helper, &d2)).await.map_err(|e| e.to_string())??;
    desktop::log(&app, "advanced sensors disabled");
    Ok(sensors::status(sensors_helper(&app).as_deref(), &data))
}

/// A host sends a PNG frame (data URL or bare base64) for lock screen / desktop sync.
#[tauri::command]
fn submit_frame(app: AppHandle, state: State<AppState>, data: String) -> Result<(), String> {
    let b64 = data.split_once(',').map(|(_, b)| b).unwrap_or(&data);
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 * 1024 || !bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Err("invalid frame".into());
    }
    // Alternate file names: Windows caches lock screen images by path.
    static FLIP: Mutex<bool> = Mutex::new(false);
    let name = {
        let mut f = FLIP.lock().unwrap_or_else(|e| e.into_inner());
        *f = !*f;
        if *f { "sync-a.png" } else { "sync-b.png" }
    };
    let path = state.paths.data.join(name);
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    let app2 = app.clone();
    std::thread::spawn(move || engine::apply_sync(&app2, &path));
    Ok(())
}

// ---------------------------------------------------------------- tray, hotkeys, windows

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn toggle_pause(app: &AppHandle) {
    let _ = app.state::<AppState>().update(app, |cfg| cfg.paused = !cfg.paused);
}

fn toggle_mute(app: &AppHandle) {
    let _ = app.state::<AppState>().update(app, |cfg| cfg.muted = !cfg.muted);
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let lang = language(&app.state::<AppState>().config()).to_string();
    let item = |id: &str| MenuItem::with_id(app, id, tr(&lang, id), true, None::<&str>);
    let (open, pause, next, mute, reattach, quit) =
        (item("open")?, item("pause")?, item("next")?, item("mute")?, item("reattach")?, item("quit")?);
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &sep1, &pause, &next, &mute, &reattach, &sep2, &quit])?;

    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(menu))?;
        return Ok(());
    }
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Wallgates")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "pause" => toggle_pause(app),
            "next" => engine::next(app),
            "mute" => toggle_mute(app),
            "reattach" => engine::rebuild(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

static HOTKEYS: Mutex<Option<HashMap<u32, &'static str>>> = Mutex::new(None);

fn register_hotkeys(app: &AppHandle) {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut map = HashMap::new();
    let hk = app.state::<AppState>().config().hotkeys.clone();
    if hk.enabled {
        for (combo, action) in [(&hk.pause, "pause"), (&hk.next, "next"), (&hk.show, "show")] {
            if combo.trim().is_empty() {
                continue;
            }
            match combo.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                Ok(sc) => match gs.register(sc) {
                    Ok(()) => {
                        map.insert(sc.id(), action);
                    }
                    Err(e) => desktop::log(app, &format!("hotkey {combo} unavailable: {e}")),
                },
                Err(e) => desktop::log(app, &format!("invalid hotkey {combo}: {e}")),
            }
        }
    }
    *HOTKEYS.lock().unwrap_or_else(|e| e.into_inner()) = Some(map);
}

fn resolve_paths(app: &AppHandle) -> Result<library::Paths, Box<dyn std::error::Error>> {
    let mut builtin = app.path().resource_dir()?.join("wallpapers");
    if !builtin.is_dir() {
        // `cargo run` without bundling: use the repository folder.
        builtin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../wallpapers");
    }
    let data = app.path().app_data_dir()?;
    let user = data.join("wallpapers");
    std::fs::create_dir_all(&user)?;
    let _ = std::fs::remove_dir_all(data.join("staging"));
    let config_file = app.path().app_config_dir()?.join("config.json");
    Ok(library::Paths { builtin, user, data, config_file })
}

/// Paths passed on the command line ("open with", file association).
fn file_args(args: &[String]) -> Vec<String> {
    args.iter()
        .skip(1)
        .filter(|a| !a.starts_with("--") && Path::new(a.as_str()).exists())
        .cloned()
        .collect()
}

fn import_args(app: &AppHandle, args: Vec<String>) {
    if args.is_empty() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = import_paths(app.clone(), args).await;
        show_main(&app);
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let files = file_args(&args);
            if files.is_empty() {
                show_main(app);
            } else {
                import_args(app, files);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![AUTOSTART_ARG]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    let action = HOTKEYS
                        .lock()
                        .ok()
                        .and_then(|m| m.as_ref().and_then(|m| m.get(&shortcut.id()).copied()));
                    match action {
                        Some("pause") => toggle_pause(app),
                        Some("next") => engine::next(app),
                        Some("show") => show_main(app),
                        _ => {}
                    }
                })
                .build(),
        )
        .register_asynchronous_uri_scheme_protocol("wp", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            std::thread::spawn(move || {
                let response = match app.try_state::<AppState>() {
                    Some(state) => protocol::handle(&state.paths, &request),
                    None => tauri::http::Response::builder().status(503).body(Vec::new()).unwrap(),
                };
                responder.respond(response);
            });
        })
        .setup(|app| {
            let handle = app.handle().clone();
            let paths = resolve_paths(&handle)?;
            let config = Config::load(&paths.config_file);
            let check_updates = config.integrations.update_check;
            app.manage(AppState::new(config, paths));

            build_tray(&handle)?;
            register_hotkeys(&handle);
            engine::rebuild(&handle);
            engine::spawn_slow_loop(handle.clone());
            engine::spawn_fast_loop(handle.clone());

            let args: Vec<String> = std::env::args().collect();
            if !args.iter().any(|a| a == AUTOSTART_ARG) {
                show_main(&handle);
            }
            import_args(&handle, file_args(&args));

            if check_updates {
                let h = handle.clone();
                tauri::async_runtime::spawn(async move {
                    tokio_sleep(10).await;
                    let _ = check_update(h).await;
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the settings window keeps the wallpaper running in the tray.
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            ui_state,
            list_wallpapers,
            host_state,
            feed_snapshot,
            set_source,
            set_property,
            reset_properties,
            update_config,
            save_playlist,
            delete_playlist,
            next_wallpaper,
            toggle_favorite,
            import_dialog,
            import_paths,
            export_wallpaper,
            delete_wallpaper,
            duplicate_wallpaper,
            create_wallpaper,
            editor_files,
            editor_read,
            editor_write,
            report_error,
            open_folder,
            open_url,
            reapply,
            get_autostart,
            set_autostart,
            geocode,
            gallery_index,
            gallery_install,
            check_update,
            submit_frame,
            sensors_status,
            sensors_enable,
            sensors_disable,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Wallgates");

    app.run(|_app, event| {
        if let RunEvent::Exit = event {
            desktop::restore();
        }
    });
}

async fn tokio_sleep(secs: u64) {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(secs));
        let _ = tx.send(());
    });
    let _ = tauri::async_runtime::spawn_blocking(move || rx.recv()).await;
}
