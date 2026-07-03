mod model;
mod provider;

use model::{
    AppSettings, CodexStatus, ProviderHealth, StatusSnapshot, WindowAnchor, WindowPlacement,
};
use provider::{CodexStateProvider, LocalCodexProvider};
use serde::Serialize;
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
    time::Duration,
};
use tauri::{
    menu::{CheckMenuItem, ContextMenu, Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, State, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

const COLLAPSED_WIDTH: f64 = 280.0;
const COLLAPSED_HEIGHT: f64 = 72.0;
const EXPANDED_WIDTH: f64 = 560.0;
const EXPANDED_HEIGHT: f64 = 360.0;
const EDGE_GAP: i32 = 24;
const SNAP_DISTANCE: f64 = 16.0;

struct AppState {
    snapshot: Arc<RwLock<StatusSnapshot>>,
    settings: Arc<RwLock<AppSettings>>,
    preview_original: Arc<RwLock<Option<AppSettings>>>,
    settings_path: PathBuf,
    placement_revision: Arc<AtomicU64>,
}

struct TrayControls {
    top: CheckMenuItem<tauri::Wry>,
    startup: CheckMenuItem<tauri::Wry>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DisplayInfo {
    name: String,
    scale_factor: f64,
    width: u32,
    height: u32,
}

#[tauri::command]
fn get_status_snapshot(state: State<'_, AppState>) -> StatusSnapshot {
    state.snapshot.read().unwrap().clone()
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.settings.read().unwrap().clone()
}

#[tauri::command]
fn get_display_info(app: tauri::AppHandle) -> Result<DisplayInfo, String> {
    let window = app.get_webview_window("main").ok_or("主窗口不存在")?;
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or_else(|| app.primary_monitor().ok().flatten())
        .ok_or("未检测到显示器")?;
    Ok(DisplayInfo {
        name: monitor
            .name()
            .map(String::as_str)
            .unwrap_or("当前显示器")
            .to_string(),
        scale_factor: monitor.scale_factor(),
        width: monitor.size().width,
        height: monitor.size().height,
    })
}

#[tauri::command]
fn preview_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<(), String> {
    let next = settings.normalized();
    {
        let mut original = state.preview_original.write().unwrap();
        if original.is_none() {
            *original = Some(state.settings.read().unwrap().clone());
        }
    }
    apply_visual_settings(&app, &next)?;
    app.emit("settings-previewed", &next)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn cancel_settings_preview(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let original = state
        .preview_original
        .write()
        .unwrap()
        .take()
        .unwrap_or_else(|| state.settings.read().unwrap().clone());
    apply_visual_settings(&app, &original)?;
    app.emit("settings-previewed", &original)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_settings_command(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<(), String> {
    let next = settings.normalized();
    persist_settings(&app, &state, next)
}

fn persist_settings(
    app: &tauri::AppHandle,
    state: &AppState,
    next: AppSettings,
) -> Result<(), String> {
    apply_visual_settings(app, &next)?;
    if next.launch_at_startup {
        app.autolaunch()
            .enable()
            .map_err(|error| error.to_string())?;
    } else {
        app.autolaunch()
            .disable()
            .map_err(|error| error.to_string())?;
    }
    save_settings(&state.settings_path, &next)?;
    *state.settings.write().unwrap() = next.clone();
    *state.preview_original.write().unwrap() = None;
    sync_tray_checks(app, &next);
    app.emit("settings-saved", &next)
        .map_err(|error| error.to_string())
}

fn apply_visual_settings(app: &tauri::AppHandle, settings: &AppSettings) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_always_on_top(settings.always_on_top)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn set_main_expanded(app: tauri::AppHandle, expanded: bool) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("主窗口不存在")?;
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or_else(|| app.primary_monitor().ok().flatten())
        .ok_or("未检测到显示器")?;
    let work = monitor.work_area();
    let scale = monitor.scale_factor();
    let current_position = window.outer_position().map_err(|error| error.to_string())?;
    let current_size = window.outer_size().map_err(|error| error.to_string())?;
    let (logical_width, logical_height) = if expanded {
        (EXPANDED_WIDTH, EXPANDED_HEIGHT)
    } else {
        (COLLAPSED_WIDTH, COLLAPSED_HEIGHT)
    };
    let target_width = (logical_width * scale).round() as i32;
    let target_height = (logical_height * scale).round() as i32;
    let left = work.position.x;
    let top = work.position.y;
    let right = left + work.size.width as i32;
    let bottom = top + work.size.height as i32;
    let current_right = current_position.x + current_size.width as i32;
    let current_bottom = current_position.y + current_size.height as i32;
    let anchor_right = (right - current_right).abs() < (current_position.x - left).abs();
    let anchor_bottom = (bottom - current_bottom).abs() < (current_position.y - top).abs();
    let mut x = if anchor_right {
        current_right - target_width
    } else {
        current_position.x
    };
    let mut y = if anchor_bottom {
        current_bottom - target_height
    } else {
        current_position.y
    };
    x = x.clamp(left, right - target_width);
    y = y.clamp(top, bottom - target_height);
    window
        .set_size(LogicalSize::new(logical_width, logical_height))
        .map_err(|error| error.to_string())?;
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn show_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("settings").ok_or("设置窗口不存在")?;
    window.center().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    app.emit("settings-opened", ())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn hide_settings_window(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    cancel: bool,
) -> Result<(), String> {
    if cancel {
        cancel_settings_preview(app.clone(), state)?;
    }
    if let Some(window) = app.get_webview_window("settings") {
        window.hide().map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn set_main_window_visible(app: tauri::AppHandle, visible: bool) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("主窗口不存在")?;
    if visible {
        window.show().map_err(|error| error.to_string())?;
    } else {
        window.hide().map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn toggle_main_window(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("主窗口不存在")?;
    if window.is_visible().map_err(|error| error.to_string())? {
        window.hide().map_err(|error| error.to_string())?;
    } else {
        window.show().map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn open_codex() -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::core::w;
        use windows::Win32::UI::WindowsAndMessaging::{
            FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE,
        };
        unsafe {
            if let Ok(hwnd) = FindWindowW(None, w!("Codex")) {
                if !hwnd.is_invalid() {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                    let _ = SetForegroundWindow(hwnd);
                    return Ok(());
                }
            }
        }
        std::process::Command::new("cmd")
            .args(["/C", "start", "", "codex://"])
            .spawn()
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn show_context_menu(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let settings = state.settings.read().unwrap().clone();
    let menu = build_menu(&app, &settings).map_err(|error| error.to_string())?;
    let window = app.get_window("main").ok_or("主窗口不存在")?;
    menu.popup(window).map_err(|error| error.to_string())
}

fn build_menu(app: &tauri::AppHandle, settings: &AppSettings) -> tauri::Result<Menu<tauri::Wry>> {
    let toggle = MenuItem::with_id(app, "toggle", "显示 / 隐藏", true, None::<&str>)?;
    let top = CheckMenuItem::with_id(
        app,
        "top",
        "始终置顶",
        true,
        settings.always_on_top,
        None::<&str>,
    )?;
    let startup = CheckMenuItem::with_id(
        app,
        "startup",
        "开机启动",
        true,
        settings.launch_at_startup,
        None::<&str>,
    )?;
    let settings_item = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出 CodePulse", true, None::<&str>)?;
    Menu::with_items(
        app,
        &[&toggle, &top, &startup, &settings_item, &separator, &quit],
    )
}

fn handle_menu_event(app: &tauri::AppHandle, id: &str) {
    match id {
        "toggle" => {
            let _ = toggle_main_window(app);
        }
        "settings" => {
            let _ = show_settings_window(app.clone());
        }
        "top" | "startup" => {
            if let Some(state) = app.try_state::<AppState>() {
                let mut next = state.settings.read().unwrap().clone();
                if id == "top" {
                    next.always_on_top = !next.always_on_top;
                }
                if id == "startup" {
                    next.launch_at_startup = !next.launch_at_startup;
                }
                let _ = persist_settings(app, &state, next);
            }
        }
        "quit" => app.exit(0),
        _ => {}
    }
}

fn setup_tray(app: &tauri::App, settings: &AppSettings) -> tauri::Result<TrayControls> {
    let toggle = MenuItem::with_id(app, "toggle", "显示 / 隐藏", true, None::<&str>)?;
    let top = CheckMenuItem::with_id(
        app,
        "top",
        "始终置顶",
        true,
        settings.always_on_top,
        None::<&str>,
    )?;
    let startup = CheckMenuItem::with_id(
        app,
        "startup",
        "开机启动",
        true,
        settings.launch_at_startup,
        None::<&str>,
    )?;
    let settings_item = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出 CodePulse", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&toggle, &top, &startup, &settings_item, &separator, &quit],
    )?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("CodePulse · 正在检测 Codex")
        .menu(&menu);
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.on_tray_icon_event(|tray, event| {
        if matches!(event, TrayIconEvent::DoubleClick { .. }) {
            let _ = toggle_main_window(tray.app_handle());
        }
    })
    .build(app)?;
    Ok(TrayControls { top, startup })
}

fn sync_tray_checks(app: &tauri::AppHandle, settings: &AppSettings) {
    if let Some(controls) = app.try_state::<TrayControls>() {
        let _ = controls.top.set_checked(settings.always_on_top);
        let _ = controls.startup.set_checked(settings.launch_at_startup);
    }
}

fn save_settings(path: &PathBuf, settings: &AppSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(path, bytes).map_err(|error| error.to_string())
}

fn load_settings(path: &PathBuf) -> AppSettings {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<AppSettings>(&bytes).ok())
        .unwrap_or_default()
        .normalized()
}

fn monitor_for_placement(
    app: &tauri::AppHandle,
    placement: Option<&WindowPlacement>,
) -> Option<tauri::Monitor> {
    let monitors = app.available_monitors().ok()?;
    if let Some(name) = placement.and_then(|value| value.monitor_name.as_deref()) {
        if let Some(monitor) = monitors
            .iter()
            .find(|monitor| monitor.name().map(String::as_str) == Some(name))
        {
            return Some(monitor.clone());
        }
    }
    app.primary_monitor()
        .ok()
        .flatten()
        .or_else(|| monitors.into_iter().next())
}

fn restore_main_position(app: &tauri::AppHandle, settings: &AppSettings) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("主窗口不存在")?;
    let remembered = if settings.remember_position {
        settings.placement.as_ref()
    } else {
        None
    };
    let monitor = monitor_for_placement(app, remembered).ok_or("未检测到显示器")?;
    let work = monitor.work_area();
    let scale = monitor.scale_factor();
    let size = PhysicalSize::new(
        (COLLAPSED_WIDTH * scale).round() as u32,
        (COLLAPSED_HEIGHT * scale).round() as u32,
    );
    let placement = remembered.cloned().unwrap_or_default();
    let gap_x = if remembered.is_some() {
        placement.offset_x
    } else {
        EDGE_GAP
    };
    let gap_y = if remembered.is_some() {
        placement.offset_y
    } else {
        EDGE_GAP
    };
    let left = work.position.x;
    let top = work.position.y;
    let right = left + work.size.width as i32;
    let bottom = top + work.size.height as i32;
    let (x, y) = match placement.anchor {
        WindowAnchor::TopLeft => (left + gap_x, top + gap_y),
        WindowAnchor::TopRight => (right - size.width as i32 - gap_x, top + gap_y),
        WindowAnchor::BottomLeft => (left + gap_x, bottom - size.height as i32 - gap_y),
        WindowAnchor::BottomRight => (
            right - size.width as i32 - gap_x,
            bottom - size.height as i32 - gap_y,
        ),
    };
    let x = x.clamp(left, right - size.width as i32);
    let y = y.clamp(top, bottom - size.height as i32);
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())
}

fn snap_and_store(app: &tauri::AppHandle, position: PhysicalPosition<i32>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let settings_snapshot = state.settings.read().unwrap().clone();
    if !settings_snapshot.remember_position && !settings_snapshot.edge_snap {
        return;
    }
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let work = monitor.work_area();
    let scale = monitor.scale_factor();
    let threshold = (SNAP_DISTANCE * scale).round() as i32;
    let left = work.position.x;
    let top = work.position.y;
    let right = left + work.size.width as i32;
    let bottom = top + work.size.height as i32;
    let mut target = position;
    let target_right = right - size.width as i32;
    let target_bottom = bottom - size.height as i32;
    if settings_snapshot.edge_snap {
        if (target.x - left).abs() < threshold {
            target.x = left;
        }
        if (target.x - target_right).abs() < threshold {
            target.x = target_right;
        }
        if (target.y - top).abs() < threshold {
            target.y = top;
        }
        if (target.y - target_bottom).abs() < threshold {
            target.y = target_bottom;
        }
        if target != position {
            let _ = window.set_position(target);
        }
    }
    if !settings_snapshot.remember_position {
        return;
    }

    let distance_left = (target.x - left).abs();
    let distance_right = (right - (target.x + size.width as i32)).abs();
    let distance_top = (target.y - top).abs();
    let distance_bottom = (bottom - (target.y + size.height as i32)).abs();
    let anchor = match (
        distance_right < distance_left,
        distance_bottom < distance_top,
    ) {
        (false, false) => WindowAnchor::TopLeft,
        (true, false) => WindowAnchor::TopRight,
        (false, true) => WindowAnchor::BottomLeft,
        (true, true) => WindowAnchor::BottomRight,
    };
    let placement = WindowPlacement {
        monitor_name: monitor.name().map(ToOwned::to_owned),
        anchor,
        offset_x: if matches!(anchor, WindowAnchor::TopRight | WindowAnchor::BottomRight) {
            distance_right
        } else {
            distance_left
        },
        offset_y: if matches!(anchor, WindowAnchor::BottomLeft | WindowAnchor::BottomRight) {
            distance_bottom
        } else {
            distance_top
        },
        scale_factor: scale,
    };
    {
        let mut settings = state.settings.write().unwrap();
        settings.placement = Some(placement);
    }
    let revision = state.placement_revision.fetch_add(1, Ordering::SeqCst) + 1;
    let handle = app.clone();
    let revision_counter = state.placement_revision.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        if revision_counter.load(Ordering::SeqCst) == revision {
            if let Some(state) = handle.try_state::<AppState>() {
                let settings = state.settings.read().unwrap().clone();
                let _ = save_settings(&state.settings_path, &settings);
            }
        }
    });
}

fn snapshots_differ(current: &StatusSnapshot, next: &StatusSnapshot) -> bool {
    current.aggregate != next.aggregate
        || current.threads != next.threads
        || current.health.level != next.health.level
        || current.health.message != next.health.message
}

fn status_label(status: CodexStatus) -> &'static str {
    match status {
        CodexStatus::Offline => "离线",
        CodexStatus::Idle => "空闲",
        CodexStatus::Thinking => "思考中",
        CodexStatus::Executing => "执行中",
        CodexStatus::WaitingApproval => "等待确认",
        CodexStatus::Completed => "已完成",
        CodexStatus::Failed => "执行失败",
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let settings = load_settings(&settings_path);
            let codex_home = dirs::home_dir().unwrap_or_default().join(".codex");
            let mut provider = LocalCodexProvider::new(codex_home);
            let initial = provider.snapshot();
            let snapshot = Arc::new(RwLock::new(initial));
            app.manage(AppState {
                snapshot: snapshot.clone(),
                settings: Arc::new(RwLock::new(settings.clone())),
                preview_original: Arc::new(RwLock::new(None)),
                settings_path,
                placement_revision: Arc::new(AtomicU64::new(0)),
            });
            apply_visual_settings(app.handle(), &settings).map_err(std::io::Error::other)?;
            restore_main_position(app.handle(), &settings).map_err(std::io::Error::other)?;
            let controls = setup_tray(app, &settings)?;
            app.manage(controls);

            let handle = app.handle().clone();
            let mut change_rx = provider.take_change_receiver();
            tauri::async_runtime::spawn(async move {
                let mut previous_health: Option<ProviderHealth> = None;
                loop {
                    if let Some(receiver) = change_rx.as_mut() {
                        let signal =
                            tokio::time::timeout(Duration::from_secs(2), receiver.recv()).await;
                        if matches!(signal, Ok(Some(_))) {
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    } else {
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                    let next = provider.snapshot();
                    let state = handle.state::<AppState>();
                    let changed = snapshots_differ(&state.snapshot.read().unwrap(), &next);
                    *state.snapshot.write().unwrap() = next.clone();
                    if changed {
                        let _ = handle.emit("status-changed", &next);
                        if let Some(tray) = handle.tray_by_id("main") {
                            let _ = tray.set_tooltip(Some(format!(
                                "CodePulse · {}",
                                status_label(next.aggregate)
                            )));
                        }
                    }
                    let health_changed = previous_health
                        .as_ref()
                        .map(|health| (&health.level, &health.message))
                        != Some((&next.health.level, &next.health.message));
                    if health_changed {
                        previous_health = Some(next.health.clone());
                        let _ = handle.emit("provider-health-changed", &next.health);
                    }
                }
            });
            Ok(())
        })
        .on_menu_event(|app, event| handle_menu_event(app, event.id().as_ref()))
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                if window.label() == "settings" {
                    if let Some(state) = window.app_handle().try_state::<AppState>() {
                        let _ = cancel_settings_preview(window.app_handle().clone(), state);
                    }
                }
                let _ = window.hide();
            }
            WindowEvent::Moved(position) if window.label() == "main" => {
                snap_and_store(window.app_handle(), *position)
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_status_snapshot,
            get_settings,
            get_display_info,
            preview_settings,
            cancel_settings_preview,
            save_settings_command,
            set_main_expanded,
            show_settings_window,
            hide_settings_window,
            set_main_window_visible,
            open_codex,
            show_context_menu,
        ])
        .run(tauri::generate_context!())
        .expect("CodePulse failed to start");
}
