mod model;
mod provider;

use model::{AppSettings, ProviderHealth, StatusSnapshot};
use provider::{CodexStateProvider, LocalCodexProvider};
use std::{fs, path::PathBuf, sync::{Arc, RwLock}, time::Duration};
use tauri::{Emitter, Manager, PhysicalPosition, State, WindowEvent};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri_plugin_autostart::{ManagerExt, MacosLauncher};

struct AppState {
    snapshot: Arc<RwLock<StatusSnapshot>>,
    settings: Arc<RwLock<AppSettings>>,
    settings_path: PathBuf,
}

#[tauri::command]
fn get_status_snapshot(state: State<'_, AppState>) -> StatusSnapshot { state.snapshot.read().unwrap().clone() }

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> AppSettings { state.settings.read().unwrap().clone() }

#[tauri::command]
fn update_settings(app: tauri::AppHandle, state: State<'_, AppState>, settings: AppSettings) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") { window.set_always_on_top(settings.always_on_top).map_err(|e| e.to_string())?; }
    let autostart = app.autolaunch();
    if settings.launch_at_startup { autostart.enable().map_err(|e| e.to_string())?; } else { autostart.disable().map_err(|e| e.to_string())?; }
    *state.settings.write().unwrap() = settings.clone();
    save_settings(&state.settings_path, &settings)
}

#[tauri::command]
fn toggle_window(app: tauri::AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("主窗口不存在")?;
    if window.is_visible().map_err(|e| e.to_string())? { window.hide() } else { window.show() }.map_err(|e| e.to_string())
}

#[tauri::command]
fn open_codex() -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::core::w;
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE};
        unsafe {
            if let Ok(hwnd) = FindWindowW(None, w!("Codex")) {
                if !hwnd.is_invalid() { let _ = ShowWindow(hwnd, SW_RESTORE); let _ = SetForegroundWindow(hwnd); return Ok(()); }
            }
        }
        std::process::Command::new("cmd").args(["/C", "start", "", "codex://"]).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn save_settings(path: &PathBuf, settings: &AppSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    fs::write(path, serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn load_settings(path: &PathBuf) -> AppSettings {
    fs::read(path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "显示 / 隐藏", true, None::<&str>)?;
    let top = MenuItem::with_id(app, "top", "切换始终置顶", true, None::<&str>)?;
    let startup = MenuItem::with_id(app, "startup", "切换开机启动", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 CodePulse", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &top, &startup, &quit])?;
    let mut tray = TrayIconBuilder::new().tooltip("CodePulse · Codex 状态指示灯").menu(&menu);
    if let Some(icon) = app.default_window_icon() { tray = tray.icon(icon.clone()); }
    tray.on_menu_event(|app, event| match event.id.as_ref() {
        "toggle" => { let _ = toggle_window(app.clone()); }
        "top" => if let Some(state) = app.try_state::<AppState>() {
            let mut next = state.settings.read().unwrap().clone(); next.always_on_top = !next.always_on_top; let _ = update_settings(app.clone(), state, next);
        }
        "startup" => if let Some(state) = app.try_state::<AppState>() {
            let mut next = state.settings.read().unwrap().clone(); next.launch_at_startup = !next.launch_at_startup; let _ = update_settings(app.clone(), state, next);
        }
        "quit" => app.exit(0),
        _ => {}
    }).on_tray_icon_event(|tray, event| {
        if matches!(event, tauri::tray::TrayIconEvent::DoubleClick { .. }) { let _ = toggle_window(tray.app_handle().clone()); }
    }).build(app)?;
    Ok(())
}

fn snap_and_store(app: &tauri::AppHandle, position: PhysicalPosition<i32>) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let mut settings = state.settings.write().unwrap();
    let mut target = position;
    if settings.edge_snap {
        if let Some(window) = app.get_webview_window("main") {
            if let (Ok(Some(monitor)), Ok(size)) = (window.current_monitor(), window.outer_size()) {
                let origin = monitor.position(); let screen = monitor.size(); let threshold = 18;
                let right = origin.x + screen.width as i32 - size.width as i32;
                let bottom = origin.y + screen.height as i32 - size.height as i32;
                if (target.x - origin.x).abs() < threshold { target.x = origin.x; }
                if (target.x - right).abs() < threshold { target.x = right; }
                if (target.y - origin.y).abs() < threshold { target.y = origin.y; }
                if (target.y - bottom).abs() < threshold { target.y = bottom; }
                if target != position { let _ = window.set_position(target); }
            }
        }
    }
    settings.window_x = Some(target.x); settings.window_y = Some(target.y);
    let _ = save_settings(&state.settings_path, &settings);
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let settings_path = config_dir.join("settings.json");
            let settings = load_settings(&settings_path);
            let codex_home = dirs::home_dir().unwrap_or_default().join(".codex");
            let mut provider = LocalCodexProvider::new(codex_home);
            let initial = provider.snapshot();
            app.manage(AppState { snapshot: Arc::new(RwLock::new(initial)), settings: Arc::new(RwLock::new(settings.clone())), settings_path });
            if let Some(window) = app.get_webview_window("main") {
                window.set_always_on_top(settings.always_on_top)?;
                if let (Some(x), Some(y)) = (settings.window_x, settings.window_y) { window.set_position(PhysicalPosition::new(x, y))?; }
            }
            setup_tray(app)?;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut previous_health: Option<ProviderHealth> = None;
                loop {
                    let paused = handle.state::<AppState>().settings.read().unwrap().paused;
                    if !paused {
                        let next = provider.snapshot();
                        let state = handle.state::<AppState>();
                        let changed = *state.snapshot.read().unwrap() != next;
                        if changed { *state.snapshot.write().unwrap() = next.clone(); let _ = handle.emit("status-changed", &next); }
                        if previous_health.as_ref() != Some(&next.health) { previous_health = Some(next.health.clone()); let _ = handle.emit("provider-health-changed", &next.health); }
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => { api.prevent_close(); let _ = window.hide(); }
            WindowEvent::Moved(position) => snap_and_store(window.app_handle(), *position),
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![get_status_snapshot, get_settings, update_settings, toggle_window, open_codex])
        .run(tauri::generate_context!())
        .expect("CodePulse failed to start");
}
