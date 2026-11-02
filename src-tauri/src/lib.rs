mod config;
mod desktop;
mod library;
mod protocol;

use config::{Config, SettingsPatch};
use library::{Paths, WallpaperInfo};
use serde::Serialize;
use serde_json::{Map, Value};
use std::sync::Mutex;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

/// Event sent to every window whenever anything the wallpaper depends on changes.
const STATE_EVENT: &str = "wf-state";

pub struct AppState {
    config: Mutex<Config>,
    auto_paused: Mutex<bool>,
    paths: Paths,
}

impl AppState {
    fn config(&self) -> std::sync::MutexGuard<'_, Config> {
        self.config.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn auto_paused(&self) -> bool {
        *self.auto_paused.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Mutate the config, persist it and notify all windows.
    fn update<R>(&self, app: &AppHandle, f: impl FnOnce(&mut Config) -> R) -> Result<R, String> {
        let result = {
            let mut cfg = self.config();
            let r = f(&mut cfg);
            cfg.save(&self.paths.config_file)?;
            r
        };
        broadcast(app);
        Ok(result)
    }
}

/// Everything the wallpaper host page and the settings UI need to render.
#[derive(Serialize, Clone)]
struct HostState {
    wallpaper: Option<WallpaperInfo>,
    values: Map<String, Value>,
    /// Effective pause state (manual or automatic).
    paused: bool,
    manual_paused: bool,
    auto_paused: bool,
    fps_limit: u32,
}

fn host_state_of(state: &AppState) -> HostState {
    let cfg = state.config().clone();
    let auto = state.auto_paused();
    let wallpaper = cfg.active.as_deref().and_then(|id| library::find(&state.paths, id));
    let values = wallpaper
        .as_ref()
        .map(|w| w.values(cfg.properties.get(&w.id)))
        .unwrap_or_default();
    HostState {
        wallpaper,
        values,
        paused: cfg.paused || auto,
        manual_paused: cfg.paused,
        auto_paused: auto,
        fps_limit: cfg.fps_limit,
    }
}

fn broadcast(app: &AppHandle) {
    let state = app.state::<AppState>();
    let _ = app.emit(STATE_EVENT, host_state_of(&state));
}

// ---------------------------------------------------------------- commands

#[tauri::command]
fn list_wallpapers(state: State<AppState>) -> Vec<WallpaperInfo> {
    library::scan(&state.paths)
}

#[tauri::command]
fn host_state(state: State<AppState>) -> HostState {
    host_state_of(&state)
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Config {
    state.config().clone()
}

#[tauri::command]
fn set_active(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    if library::find(&state.paths, &id).is_none() {
        return Err(format!("wallpaper not found: {id}"));
    }
    state.update(&app, |cfg| cfg.active = Some(id))
}

#[tauri::command]
fn set_property(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    key: String,
    value: Value,
) -> Result<(), String> {
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

#[tauri::command]
fn update_settings(app: AppHandle, state: State<AppState>, patch: SettingsPatch) -> Result<Config, String> {
    state.update(&app, |cfg| {
        patch.apply(cfg);
        cfg.clone()
    })
}

#[tauri::command]
async fn import_file(app: AppHandle) -> Result<Option<String>, String> {
    let Some(picked) = app
        .dialog()
        .file()
        .set_title("Import image or video")
        .add_filter("Images and videos", library::MEDIA_EXTENSIONS)
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    let id = library::import_file(&state.paths, &path)?;
    state.update(&app, |cfg| cfg.active = Some(id.clone()))?;
    Ok(Some(id))
}

#[tauri::command]
async fn import_folder(app: AppHandle) -> Result<Option<String>, String> {
    let Some(picked) = app
        .dialog()
        .file()
        .set_title("Import wallpaper folder (must contain wallpaper.json)")
        .blocking_pick_folder()
    else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    let id = library::import_folder(&state.paths, &path)?;
    state.update(&app, |cfg| cfg.active = Some(id.clone()))?;
    Ok(Some(id))
}

#[tauri::command]
fn delete_wallpaper(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    library::delete(&state.paths, &id)?;
    state.update(&app, |cfg| {
        cfg.properties.remove(&id);
        if cfg.active.as_deref() == Some(id.as_str()) {
            cfg.active = Some("builtin/aurora".into());
        }
    })
}

#[tauri::command]
fn open_library_folder(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    std::fs::create_dir_all(&state.paths.user).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(state.paths.user.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Recreate wallpaper windows (after monitor changes or an Explorer restart).
#[tauri::command]
async fn reapply(app: AppHandle) -> Result<(), String> {
    desktop::create_windows(&app)
}

// ---------------------------------------------------------------- setup

fn resolve_paths(app: &AppHandle) -> Result<Paths, Box<dyn std::error::Error>> {
    let mut builtin = app.path().resource_dir()?.join("wallpapers");
    if !builtin.is_dir() {
        // `cargo run` without bundling: use the repository folder.
        builtin = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../wallpapers");
    }
    let user = app.path().app_data_dir()?.join("wallpapers");
    std::fs::create_dir_all(&user)?;
    let config_file = app.path().app_config_dir()?.join("config.json");
    Ok(Paths { builtin, user, config_file })
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn toggle_pause(app: &AppHandle) {
    let state = app.state::<AppState>();
    let _ = state.update(app, |cfg| cfg.paused = !cfg.paused);
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Wallgates", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Pause / resume", true, None::<&str>)?;
    let reapply = MenuItem::with_id(app, "reapply", "Re-attach to desktop", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &pause, &reapply, &sep, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Wallgates")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "pause" => toggle_pause(app),
            "reapply" => {
                if let Err(e) = desktop::create_windows(app) {
                    eprintln!("[wallgates] reapply failed: {e}");
                }
            }
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

/// Poll the desktop and pause the wallpaper while a game / fullscreen app runs.
fn spawn_auto_pause(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(1500));
        let state = app.state::<AppState>();
        let rules = {
            let cfg = state.config();
            desktop::PauseRules {
                fullscreen: cfg.pause_on_fullscreen,
                maximized: cfg.pause_on_maximized,
                battery: cfg.pause_on_battery,
            }
        };
        let auto = desktop::should_auto_pause(rules);
        let changed = {
            let mut current = state.auto_paused.lock().unwrap_or_else(|e| e.into_inner());
            let changed = *current != auto;
            *current = auto;
            changed
        };
        if changed {
            broadcast(&app);
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
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
            app.manage(AppState {
                config: Mutex::new(config),
                auto_paused: Mutex::new(false),
                paths,
            });
            build_tray(&handle)?;
            if let Err(e) = desktop::create_windows(&handle) {
                eprintln!("[wallgates] could not create wallpaper windows: {e}");
            }
            spawn_auto_pause(handle);
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
            list_wallpapers,
            host_state,
            get_settings,
            set_active,
            set_property,
            reset_properties,
            update_settings,
            import_file,
            import_folder,
            delete_wallpaper,
            open_library_folder,
            reapply,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Wallgates");

    app.run(|_app, event| {
        if let RunEvent::Exit = event {
            desktop::restore();
        }
    });
}
