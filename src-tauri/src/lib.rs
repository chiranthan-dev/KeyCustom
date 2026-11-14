//! Tauri shell around the KeyForge controller: commands for the UI, the tray
//! menu, single-instance handling and window behaviour. All keyboard logic
//! lives in `keyforge-engine`; nothing here touches the keyboard.

use keyforge_core::config::Config;
use keyforge_core::keys::{KeyDef, KEYS};
use keyforge_core::store::BackupInfo;
use keyforge_engine::controller::{ApplyOutcome, Controller, Options, Preview, ScriptInfo, Sink, Snapshot, Status};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::image::Image;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent, Wry};

/// Name of the per-user startup entry, and the flag it starts KeyForge with.
const STARTUP_NAME: &str = "KeyForge";
const AUTOSTART_FLAG: &str = "--autostart";

fn tray_icon(on: bool) -> Image<'static> {
    let bytes: &'static [u8] = if on { include_bytes!("../icons/tray-on.png") } else { include_bytes!("../icons/tray-off.png") };
    Image::from_bytes(bytes).expect("tray icon is a valid PNG")
}

type Ctl<'a> = State<'a, Arc<Controller>>;

#[derive(Clone)]
struct TrayItems {
    pause: MenuItem<Wry>,
    resume: MenuItem<Wry>,
}

/// Forwards controller events to the UI and keeps the tray in sync.
struct TauriSink {
    app: AppHandle,
    tray: Mutex<Option<TrayItems>>,
}

impl Sink for TauriSink {
    fn emit(&self, event: &str, payload: Value) {
        if event == "status" {
            // The controller calls this while holding its lock. Menu updates
            // must run on the main thread, so post them without waiting;
            // waiting here could deadlock with a main-thread caller.
            let enabled = payload["enabled"].as_bool().unwrap_or(false);
            let items = self.tray.lock().unwrap().clone();
            let app = self.app.clone();
            let _ = self.app.run_on_main_thread(move || {
                if let Some(items) = items {
                    let _ = items.pause.set_enabled(enabled);
                    let _ = items.resume.set_enabled(!enabled);
                }
                if let Some(tray) = app.tray_by_id("main") {
                    let state = if enabled { "mappings on" } else { "mappings off" };
                    let _ = tray.set_tooltip(Some(format!("KeyForge: {state}")));
                    let _ = tray.set_icon(Some(tray_icon(enabled)));
                }
            });
        }
        if event == "overlay" {
            let show = payload["show"].as_bool().unwrap_or(false);
            let app = self.app.clone();
            let _ = self.app.run_on_main_thread(move || show_overlay(&app, show));
        }
        let _ = self.app.emit(&format!("kf://{event}"), payload);
    }
}

/// Browser arguments for every webview. With `KEYFORGE_REMOTE_DEBUG_PORT`
/// set (CI's UI smoke test does this), WebView2 also opens a local DevTools
/// port. Wry's default arguments are kept, and all windows must use the same
/// arguments because they share one WebView2 environment.
fn browser_args() -> Option<String> {
    let port: u16 = std::env::var("KEYFORGE_REMOTE_DEBUG_PORT").ok()?.parse().ok()?;
    Some(format!("--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port={port}"))
}

const OVERLAY_SIZE: (f64, f64) = (360.0, 560.0);
const OVERLAY_MARGIN: f64 = 16.0;

/// The cheat-sheet window: transparent, always on top, never focused and
/// click-through, so it cannot take keys or clicks from the app in front.
fn create_overlay(app: &AppHandle) -> tauri::Result<()> {
    let mut builder = tauri::WebviewWindowBuilder::new(app, "overlay", tauri::WebviewUrl::App("index.html".into()));
    if let Some(args) = browser_args() {
        builder = builder.additional_browser_args(&args);
    }
    let window = builder
        .title("KeyForge cheat sheet")
        .inner_size(OVERLAY_SIZE.0, OVERLAY_SIZE.1)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focusable(false)
        .focused(false)
        .visible(false)
        .build()?;
    let _ = window.set_ignore_cursor_events(true);
    Ok(())
}

/// Show the cheat sheet in the bottom-right corner of the screen the mouse
/// is on (above the taskbar), or hide it.
fn show_overlay(app: &AppHandle, show: bool) {
    let Some(window) = app.get_webview_window("overlay") else { return };
    if !show {
        let _ = window.hide();
        return;
    }
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let (w, h) = (OVERLAY_SIZE.0 * scale, OVERLAY_SIZE.1 * scale);
        let margin = OVERLAY_MARGIN * scale;
        let x = area.position.x as f64 + area.size.width as f64 - w - margin;
        let y = area.position.y as f64 + area.size.height as f64 - h - margin;
        let _ = window.set_position(tauri::PhysicalPosition::new(x.round() as i32, y.round() as i32));
    }
    let _ = window.show();
}

/// Run controller work off the main thread: it may wait for the engine, and
/// the controller lock must never be awaited on the main thread.
async fn blocking<T: Send + 'static>(ctl: &Ctl<'_>, f: impl FnOnce(&Arc<Controller>) -> Result<T, String> + Send + 'static) -> Result<T, String> {
    let ctl = Arc::clone(ctl.inner());
    tauri::async_runtime::spawn_blocking(move || f(&ctl)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_snapshot(ctl: Ctl<'_>) -> Result<Snapshot, String> {
    blocking(&ctl, |c| Ok(c.snapshot())).await
}

#[tauri::command]
async fn finish_first_run(ctl: Ctl<'_>) -> Result<(), String> {
    blocking(&ctl, |c| Ok(c.mark_first_run_done())).await
}

/// Whether KeyForge starts when the user signs in (and was not disabled in
/// Task Manager's Startup tab).
#[tauri::command]
fn get_autostart() -> bool {
    #[cfg(windows)]
    return keyforge_engine::win::run_at_login(STARTUP_NAME).is_some();
    #[cfg(not(windows))]
    false
}

fn startup_command() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(format!("\"{}\" {AUTOSTART_FLAG}", exe.display()))
}

#[tauri::command]
fn set_autostart(enabled: bool) -> Result<bool, String> {
    #[cfg(windows)]
    {
        let command = if enabled { Some(startup_command()?) } else { None };
        keyforge_engine::win::set_run_at_login(STARTUP_NAME, command.as_deref()).map_err(|e| format!("Could not change the startup setting: {e}"))?;
        Ok(get_autostart())
    }
    #[cfg(not(windows))]
    {
        let _ = (enabled, startup_command);
        Err("Starting with Windows is only available on Windows.".into())
    }
}

#[tauri::command]
fn list_keys() -> &'static [KeyDef] {
    KEYS
}

#[tauri::command]
async fn preview_config(ctl: Ctl<'_>, config: Config) -> Result<Preview, String> {
    blocking(&ctl, move |c| Ok(c.preview(&config))).await
}

#[tauri::command]
async fn apply_config(ctl: Ctl<'_>, config: Config) -> Result<ApplyOutcome, String> {
    blocking(&ctl, move |c| c.apply(config)).await
}

#[tauri::command]
async fn set_enabled(ctl: Ctl<'_>, enabled: bool) -> Result<Status, String> {
    blocking(&ctl, move |c| c.set_enabled(enabled)).await
}

#[tauri::command]
async fn set_profile(ctl: Ctl<'_>, id: String) -> Result<Status, String> {
    blocking(&ctl, move |c| c.set_profile(&id)).await
}

#[tauri::command]
async fn start_detect(ctl: Ctl<'_>) -> Result<Status, String> {
    blocking(&ctl, |c| c.start_detect()).await
}

#[tauri::command]
async fn cancel_detect(ctl: Ctl<'_>) -> Result<(), String> {
    blocking(&ctl, |c| Ok(c.cancel_detect())).await
}

#[tauri::command]
async fn test_mapping(ctl: Ctl<'_>, profile: String, mapping: String) -> Result<String, String> {
    blocking(&ctl, move |c| c.test_mapping(&profile, &mapping)).await
}

#[tauri::command]
async fn confirm_script(ctl: Ctl<'_>, path: String) -> Result<ScriptInfo, String> {
    blocking(&ctl, move |c| c.confirm_script(&path)).await
}

#[tauri::command]
async fn import_config(ctl: Ctl<'_>, path: PathBuf) -> Result<ApplyOutcome, String> {
    blocking(&ctl, move |c| c.import(&path)).await
}

#[tauri::command]
async fn export_config(ctl: Ctl<'_>, path: PathBuf) -> Result<(), String> {
    blocking(&ctl, move |c| c.export(&path)).await
}

#[tauri::command]
async fn backup_now(ctl: Ctl<'_>) -> Result<String, String> {
    blocking(&ctl, |c| c.backup_now()).await
}

#[tauri::command]
async fn list_backups(ctl: Ctl<'_>) -> Result<Vec<BackupInfo>, String> {
    blocking(&ctl, |c| Ok(c.list_backups())).await
}

#[tauri::command]
async fn restore_backup(ctl: Ctl<'_>, name: String) -> Result<ApplyOutcome, String> {
    blocking(&ctl, move |c| c.restore_backup(&name)).await
}

#[tauri::command]
async fn reset_config(ctl: Ctl<'_>) -> Result<ApplyOutcome, String> {
    blocking(&ctl, |c| c.reset()).await
}

#[tauri::command]
async fn get_logs(ctl: Ctl<'_>) -> Result<Vec<String>, String> {
    blocking(&ctl, |c| Ok(c.logs())).await
}

#[tauri::command]
async fn dismiss_notice(ctl: Ctl<'_>) -> Result<(), String> {
    blocking(&ctl, |c| Ok(c.dismiss_notice())).await
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn pause_or_resume(app: &AppHandle, enabled: bool) {
    let ctl = Arc::clone(app.state::<Arc<Controller>>().inner());
    std::thread::spawn(move || {
        let _ = ctl.set_enabled(enabled);
    });
}

fn build_tray(app: &AppHandle, sink: &TauriSink) -> tauri::Result<()> {
    let pause = MenuItem::with_id(app, "pause", "Pause mappings", false, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume", "Resume mappings", true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open settings", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit", "Exit KeyForge", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&pause, &resume, &PredefinedMenuItem::separator(app)?, &open, &exit])?;
    TrayIconBuilder::with_id("main")
        .icon(tray_icon(false))
        .tooltip("KeyForge: mappings off")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "pause" => pause_or_resume(app, false),
            "resume" => pause_or_resume(app, true),
            "open" => show_main(app),
            "exit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    *sink.tray.lock().unwrap() = Some(TrayItems { pause, resume });
    Ok(())
}

pub fn run() {
    let app = tauri::Builder::default()
        // A second KeyForge would start a second engine; focus the first instead.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let mut options = Options::new(app.path().app_local_data_dir()?);
            options.bundled_ahk_dir = app.path().resource_dir().ok().map(|d| d.join("engine"));
            let sink = Arc::new(TauriSink { app: handle.clone(), tray: Mutex::new(None) });
            let controller = Controller::new(options, sink.clone());
            // Register the controller before any window exists: a page that
            // loads first would call commands that need it and fail.
            app.manage(Arc::clone(&controller));
            #[cfg(windows)]
            controller.register_emergency_hotkey();
            let main = app.config().app.windows.iter().find(|w| w.label == "main").expect("main window in tauri.conf.json").clone();
            let mut builder = tauri::WebviewWindowBuilder::from_config(&handle, &main)?;
            if let Some(args) = browser_args() {
                builder = builder.additional_browser_args(&args);
            }
            builder.build()?;
            create_overlay(&handle)?;
            build_tray(&handle, &sink)?;

            // Started at sign-in: stay in the tray. Otherwise show the window.
            let autostarted = std::env::args().any(|a| a == AUTOSTART_FLAG);
            if !autostarted {
                show_main(&handle);
            }
            // Keep the startup entry pointing at this executable, e.g. after
            // KeyForge was reinstalled to another folder.
            #[cfg(windows)]
            if keyforge_engine::win::run_at_login(STARTUP_NAME).is_some() {
                if let Ok(command) = startup_command() {
                    let _ = keyforge_engine::win::set_run_at_login(STARTUP_NAME, Some(&command));
                }
            }
            std::thread::spawn(move || controller.start_on_launch());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let to_tray = window.app_handle().try_state::<Arc<Controller>>().is_none_or(|c| c.close_to_tray());
                if to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            list_keys,
            preview_config,
            apply_config,
            set_enabled,
            set_profile,
            start_detect,
            cancel_detect,
            test_mapping,
            confirm_script,
            import_config,
            export_config,
            backup_now,
            list_backups,
            restore_backup,
            reset_config,
            get_logs,
            dismiss_notice,
            finish_first_run,
            get_autostart,
            set_autostart,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build KeyForge");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            if let Some(ctl) = handle.try_state::<Arc<Controller>>() {
                // Stopping waits for the engine; do it off the main thread so
                // pending status updates can still be delivered.
                let ctl = Arc::clone(ctl.inner());
                let _ = std::thread::spawn(move || ctl.shutdown()).join();
            }
        }
    });
}
