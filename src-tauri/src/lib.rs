mod host;

use host::{ApplyResult, Host, Status};
use keyforge_core::config::Mapping;
use keyforge_core::keys::{KeyDef, KEYS};
use std::sync::Arc;
use tauri::{Manager, RunEvent, State};

type HostState<'a> = State<'a, Arc<Host>>;

/// Engine calls block (process start, waiting for the engine), so they run
/// on the blocking pool instead of the UI thread.
async fn blocking<T: Send + 'static>(host: &HostState<'_>, f: impl FnOnce(&Arc<Host>) -> Result<T, String> + Send + 'static) -> Result<T, String> {
    let host = Arc::clone(host.inner());
    tauri::async_runtime::spawn_blocking(move || f(&host)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn get_status(host: HostState<'_>) -> Status {
    host.status()
}

#[tauri::command]
fn list_keys() -> &'static [KeyDef] {
    KEYS
}

#[tauri::command]
async fn apply_mappings(host: HostState<'_>, mappings: Vec<Mapping>) -> Result<ApplyResult, String> {
    blocking(&host, move |h| h.apply(mappings)).await
}

#[tauri::command]
async fn set_enabled(host: HostState<'_>, enabled: bool) -> Result<Status, String> {
    blocking(&host, move |h| h.set_enabled(enabled)).await
}

#[tauri::command]
async fn start_detect(host: HostState<'_>) -> Result<Status, String> {
    blocking(&host, |h| h.start_detect()).await
}

#[tauri::command]
fn cancel_detect(host: HostState<'_>) {
    host.cancel_detect();
}

pub fn run() {
    let app = tauri::Builder::default()
        .setup(|app| {
            let script = app.path().app_local_data_dir()?.join("engine").join("keyforge.ahk");
            let bundled = app.path().resource_dir().ok().map(|d| d.join("engine"));
            let host = Host::new(app.handle().clone(), script, bundled);

            #[cfg(windows)]
            {
                let h = Arc::clone(&host);
                let registered = keyforge_engine::win::listen_emergency_hotkey(move || h.emergency_disable()).is_ok();
                host.set_emergency_registered(registered);
            }

            app.manage(host);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_status, list_keys, apply_mappings, set_enabled, start_detect, cancel_detect])
        .build(tauri::generate_context!())
        .expect("failed to build KeyForge");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            handle.state::<Arc<Host>>().shutdown();
        }
    });
}
