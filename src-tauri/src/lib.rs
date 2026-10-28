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
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent, Wry};

type Ctl<'a> = State<'a, Arc<Controller>>;

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
            let enabled = payload["enabled"].as_bool().unwrap_or(false);
            if let Some(items) = self.tray.lock().unwrap().as_ref() {
                let _ = items.pause.set_enabled(enabled);
                let _ = items.resume.set_enabled(!enabled);
            }
            if let Some(tray) = self.app.tray_by_id("main") {
                let state = if enabled { "mappings on" } else { "mappings off" };
                let _ = tray.set_tooltip(Some(format!("KeyForge: {state}")));
            }
        }
        let _ = self.app.emit(&format!("kf://{event}"), payload);
    }
}

/// Run blocking controller work (engine start, waiting for the engine) off
/// the UI thread.
async fn blocking<T: Send + 'static>(ctl: &Ctl<'_>, f: impl FnOnce(&Arc<Controller>) -> Result<T, String> + Send + 'static) -> Result<T, String> {
    let ctl = Arc::clone(ctl.inner());
    tauri::async_runtime::spawn_blocking(move || f(&ctl)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn get_snapshot(ctl: Ctl<'_>) -> Snapshot {
    ctl.snapshot()
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
fn cancel_detect(ctl: Ctl<'_>) {
    ctl.cancel_detect();
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
fn list_backups(ctl: Ctl<'_>) -> Vec<BackupInfo> {
    ctl.list_backups()
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
fn get_logs(ctl: Ctl<'_>) -> Vec<String> {
    ctl.logs()
}

#[tauri::command]
fn dismiss_notice(ctl: Ctl<'_>) {
    ctl.dismiss_notice();
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
        .icon(app.default_window_icon().cloned().expect("app icon"))
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
            #[cfg(windows)]
            controller.register_emergency_hotkey();
            build_tray(&handle, &sink)?;
            app.manage(Arc::clone(&controller));
            if controller.snapshot().config.settings.enable_on_launch {
                std::thread::spawn(move || {
                    let _ = controller.set_enabled(true);
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let to_tray = window.app_handle().try_state::<Arc<Controller>>().is_some_and(|c| c.snapshot().config.settings.close_to_tray);
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
        ])
        .build(tauri::generate_context!())
        .expect("failed to build KeyForge");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            if let Some(ctl) = handle.try_state::<Arc<Controller>>() {
                ctl.shutdown();
            }
        }
    });
}
