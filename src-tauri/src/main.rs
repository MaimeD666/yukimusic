#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod audio;
#[cfg(windows)]
mod audio_devices;
mod media;
mod models;
mod persistence;
mod soundcloud;
mod stream_cache;
#[cfg(windows)]
mod taskbar;
#[cfg(windows)]
mod window_motion;
use models::{Library, Playback};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, PhysicalPosition, PhysicalSize,
};

struct WindowGeometry {
    size: PhysicalSize<u32>,
    position: PhysicalPosition<i32>,
    maximized: bool,
}

struct AppState {
    hidden_epoch: AtomicU64,
    api: Arc<soundcloud::SoundCloud>,
    player: audio::Player,
    directory: PathBuf,
    library: Mutex<Library>,
    geometry: Mutex<Option<WindowGeometry>>,
}
#[tauri::command]
async fn search(state: tauri::State<'_, AppState>, query: String) -> Result<Value, String> {
    state.api.search(&query).await.map_err(|e| e.to_string())
}
#[tauri::command]
async fn search_more(
    state: tauri::State<'_, AppState>,
    kind: String,
    url: String,
) -> Result<Value, String> {
    state
        .api
        .search_more(&kind, &url)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn artist_tracks(
    state: tauri::State<'_, AppState>,
    id: u64,
    next: Option<String>,
) -> Result<Value, String> {
    state
        .api
        .artist_tracks(id, next)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn playlist_tracks(state: tauri::State<'_, AppState>, id: u64) -> Result<Value, String> {
    state
        .api
        .playlist_tracks(id)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn artist_profile(
    state: tauri::State<'_, AppState>,
    id: u64,
) -> Result<models::Artist, String> {
    state
        .api
        .artist_profile(id)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn artwork(state: tauri::State<'_, AppState>, url: String) -> Result<String, String> {
    state.api.artwork(&url).await.map_err(|e| e.to_string())
}
#[tauri::command]
async fn resolve(state: tauri::State<'_, AppState>, url: String) -> Result<Value, String> {
    state.api.resolve(&url).await.map_err(|e| e.to_string())
}
#[tauri::command]
fn snapshot(state: tauri::State<'_, AppState>) -> Playback {
    state.player.state.lock().unwrap().clone()
}
#[tauri::command]
fn get_library(state: tauri::State<'_, AppState>) -> Library {
    state.library.lock().unwrap().clone()
}
#[tauri::command]
fn save_library(state: tauri::State<'_, AppState>, library: Library) -> Result<(), String> {
    let mut stored = state.library.lock().unwrap();
    persistence::atomic_write(&state.directory.join("library.json"), &library)?;
    *stored = library;
    Ok(())
}
#[tauri::command]
fn player_action(
    state: tauri::State<'_, AppState>,
    action: String,
    value: Value,
) -> Result<(), String> {
    state.player.action(action, value)
}
#[tauri::command]
fn visualization(state: tauri::State<'_, AppState>, visible: bool, enabled: bool) {
    state
        .player
        .visualization
        .store(visible && enabled, Ordering::Relaxed);
    state.player.visible.store(visible, Ordering::Relaxed);
}
#[tauri::command]
async fn window_action(window: tauri::WebviewWindow, action: String) -> Result<(), String> {
    if action == "exit" {
        window.app_handle().state::<AppState>().player.shutdown();
        window.app_handle().exit(0);
        return Ok(());
    }
    // Window mutations dispatch asynchronously from command threads. Run the whole
    // transition on the UI thread so each geometry read sees the preceding change.
    let (send, receive) = tokio::sync::oneshot::channel();
    let target = window.clone();
    window
        .run_on_main_thread(move || {
            let result = apply_window_action(&target, &action).map_err(|e| e.to_string());
            let _ = send.send(result);
        })
        .map_err(|e| e.to_string())?;
    receive.await.map_err(|e| e.to_string())?
}

fn apply_window_action(window: &tauri::WebviewWindow, action: &str) -> tauri::Result<()> {
    #[cfg(windows)]
    let _motion = matches!(action, "mini" | "immersive" | "restore")
        .then(|| window_motion::QuietTransition::new(window))
        .transpose()?;
    let state = window.app_handle().state::<AppState>();
    match action {
        "mini" | "immersive" => {
            let mut geometry = state.geometry.lock().unwrap();
            if geometry.is_none() {
                let maximized = window.is_maximized()?;
                // Windows retains maximized client insets when a borderless window
                // goes straight to fullscreen. Restore first, and save normal bounds
                // rather than the maximized outer rect (which includes resize borders).
                if maximized {
                    window.unmaximize()?;
                }
                *geometry = Some(WindowGeometry {
                    size: window.inner_size()?,
                    position: window.outer_position()?,
                    maximized,
                });
            }
        }
        _ => {}
    }
    match action {
        "minimize" => window.minimize()?,
        "maximize" => {
            if window.is_maximized()? {
                window.unmaximize()?;
            } else {
                window.maximize()?;
            }
        }
        "close" => {
            hide_main(window.app_handle());
        }
        "mini" => {
            window.set_fullscreen(false)?;
            window.unmaximize()?;
            window.set_min_size(Some(tauri::LogicalSize::new(380., 250.)))?;
            window.set_size(tauri::LogicalSize::new(430., 260.))?;
            window.set_always_on_top(true)?;
        }
        "immersive" => {
            window.set_always_on_top(false)?;
            window.set_fullscreen(true)?;
        }
        "restore" => {
            window.set_fullscreen(false)?;
            window.set_always_on_top(false)?;
            window.set_min_size(Some(tauri::LogicalSize::new(800., 620.)))?;
            let mut geometry = state.geometry.lock().unwrap();
            if let Some(saved) = geometry.as_ref() {
                window.unmaximize()?;
                window.set_size(saved.size)?;
                window.set_position(saved.position)?;
                if saved.maximized {
                    window.maximize()?;
                }
                *geometry = None;
            }
        }
        _ => {}
    }
    Ok(())
}
fn hide_main(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    state.player.visible.store(false, Ordering::Relaxed);
    state.player.visualization.store(false, Ordering::Relaxed);
    let epoch = state.hidden_epoch.fetch_add(1, Ordering::Relaxed) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let state = handle.state::<AppState>();
            if state.hidden_epoch.load(Ordering::Relaxed) == epoch
                && !state.player.visible.load(Ordering::Relaxed)
            {
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.destroy();
                }
                *state.geometry.lock().unwrap() = None;
            }
        });
    });
}
fn show_main(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    state.hidden_epoch.fetch_add(1, Ordering::Relaxed);
    if app.get_webview_window("main").is_none() {
        if let Ok(builder) =
            tauri::WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])
        {
            if let Ok(window) = builder
                .visible(false)
                .data_directory(state.directory.join("webview"))
                .build()
            {
                #[cfg(windows)]
                {
                    let _ = window.with_webview(|webview| unsafe {
                        if let Ok(core) = webview.controller().CoreWebView2() {
                            if let Ok(settings) = core.Settings() {
                                let _ = settings.SetAreDefaultContextMenusEnabled(false);
                            }
                        }
                    });
                    taskbar::update(&state.player.state.lock().unwrap());
                    taskbar::attach(&window, state.player.sender.clone());
                }
            }
        }
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        state.player.visible.store(true, Ordering::Relaxed);
        let _ = window.emit("playback", state.player.state.lock().unwrap().clone());
    }
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_main(app)
        }))
        .setup(|app| {
            let directory = std::env::var_os("YUKICLOUD_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or(app.path().app_data_dir()?);
            std::fs::create_dir_all(&directory)?;
            let library = std::fs::read(directory.join("library.json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
            let api = Arc::new(soundcloud::SoundCloud::new(directory.clone()));
            let ffmpeg = app.path().resource_dir()?.join("resources/ffmpeg.exe");
            let ffmpeg = if ffmpeg.exists() {
                ffmpeg
            } else {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/ffmpeg.exe")
            };
            let player =
                audio::Player::new(app.handle().clone(), api.clone(), directory.clone(), ffmpeg);
            #[cfg(windows)]
            if let Err(error) = audio_devices::attach(player.sender.clone()) {
                eprintln!("Audio device notifications: {error}");
            }
            let media_window = tauri::window::WindowBuilder::new(app, "media")
                .title("yukicloud")
                .visible(false)
                .skip_taskbar(true)
                .build()?;
            *player.media.lock().unwrap() = media::Controls::new(
                media_window.hwnd()?.0,
                player.sender.clone(),
                player.state.clone(),
            );
            app.manage(AppState {
                hidden_epoch: AtomicU64::new(0),
                api,
                player,
                directory,
                library: Mutex::new(library),
                geometry: Mutex::new(None),
            });
            show_main(app.handle());
            let show = MenuItem::with_id(app, "show", "Открыть yukicloud", true, None::<&str>)?;
            let toggle =
                MenuItem::with_id(app, "toggle", "Воспроизведение / пауза", true, None::<&str>)?;
            let next = MenuItem::with_id(app, "next", "Следующий трек", true, None::<&str>)?;
            let exit = MenuItem::with_id(app, "exit", "Выйти", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &toggle, &next, &exit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("yukicloud")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main(app),
                    "toggle" | "next" => {
                        let state = app.state::<AppState>();
                        let _ = state
                            .player
                            .action(event.id.as_ref().to_string(), Value::Null);
                    }
                    "exit" => {
                        app.state::<AppState>().player.shutdown();
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    hide_main(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            search,
            search_more,
            artist_profile,
            artist_tracks,
            playlist_tracks,
            artwork,
            resolve,
            snapshot,
            get_library,
            save_library,
            player_action,
            visualization,
            window_action
        ])
        .build(tauri::generate_context!())
        .expect("Unable to start yukicloud")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                #[cfg(windows)]
                audio_devices::detach();
                app.state::<AppState>().player.shutdown();
            }
        });
}
