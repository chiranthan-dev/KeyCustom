//! Owns the engine on behalf of the UI. All keyboard interception happens in
//! the engine process; the UI only sends requests and receives events.

use keyforge_core::config::Mapping;
use keyforge_core::protocol::{exit_code, Command, DetectedKey, Event};
use keyforge_core::validate::{self, Report, EMERGENCY_LABEL};
use keyforge_core::ahk;
use keyforge_engine::{locate, wait_ready, Engine, EngineEvent, Launch};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const READY_TIMEOUT: Duration = Duration::from_secs(5);
const STOP_GRACE: Duration = Duration::from_millis(1500);

pub struct Host {
    app: AppHandle,
    script_path: PathBuf,
    bundled_dir: Option<PathBuf>,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    engine: Option<Engine>,
    /// Incremented for every engine start, so events from an engine that has
    /// already been replaced are ignored.
    generation: u64,
    /// Mappings currently loaded in the engine, when it runs with mappings.
    mappings: Vec<Mapping>,
    /// The user wants mappings active. Always false at startup.
    enabled: bool,
    detecting: bool,
    ahk_version: Option<String>,
    elevated_engine: bool,
    last_error: Option<String>,
    emergency_registered: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    ahk_path: Option<String>,
    ahk_version: Option<String>,
    engine_running: bool,
    enabled: bool,
    detecting: bool,
    elevated_engine: bool,
    last_error: Option<String>,
    emergency_shortcut: &'static str,
    emergency_registered: bool,
    mappings: Vec<Mapping>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    report: Report,
    status: Status,
}

impl Host {
    pub fn new(app: AppHandle, script_path: PathBuf, bundled_dir: Option<PathBuf>) -> Arc<Host> {
        Arc::new(Host { app, script_path, bundled_dir, inner: Mutex::new(Inner::default()) })
    }

    pub fn set_emergency_registered(&self, registered: bool) {
        self.inner.lock().unwrap().emergency_registered = registered;
    }

    fn ahk_path(&self) -> Option<PathBuf> {
        locate::find(self.bundled_dir.as_deref())
    }

    pub fn status(&self) -> Status {
        let inner = self.inner.lock().unwrap();
        self.status_of(&inner)
    }

    fn status_of(&self, inner: &Inner) -> Status {
        Status {
            ahk_path: self.ahk_path().map(|p| p.display().to_string()),
            ahk_version: inner.ahk_version.clone(),
            engine_running: inner.engine.is_some(),
            enabled: inner.enabled,
            detecting: inner.detecting,
            elevated_engine: inner.elevated_engine,
            last_error: inner.last_error.clone(),
            emergency_shortcut: EMERGENCY_LABEL,
            emergency_registered: inner.emergency_registered,
            mappings: inner.mappings.clone(),
        }
    }

    fn publish(&self, inner: &Inner) {
        let _ = self.app.emit("kf://status", self.status_of(inner));
    }

    /// Validate `mappings` and, if mappings are enabled, switch the engine to
    /// them. If the new engine fails to start the previous mappings are
    /// restored. Invalid mappings are never stored or applied.
    pub fn apply(self: &Arc<Self>, mappings: Vec<Mapping>) -> Result<ApplyResult, String> {
        let (report, _) = validate::validate(&mappings);
        let mut inner = self.inner.lock().unwrap();
        if !report.ok() {
            return Ok(ApplyResult { report, status: self.status_of(&inner) });
        }
        let previous = std::mem::replace(&mut inner.mappings, mappings);
        if inner.enabled {
            if let Err(e) = self.start(&mut inner, true) {
                inner.mappings = previous;
                let restored = self.start(&mut inner, true);
                inner.last_error = Some(match restored {
                    Ok(()) => format!("{e}\nThe previous mappings are still active."),
                    Err(_) => {
                        inner.enabled = false;
                        format!("{e}\nMappings are disabled.")
                    }
                });
                self.publish(&inner);
                return Err(e);
            }
        }
        inner.last_error = None;
        self.publish(&inner);
        Ok(ApplyResult { report, status: self.status_of(&inner) })
    }

    pub fn set_enabled(self: &Arc<Self>, enabled: bool) -> Result<Status, String> {
        let mut inner = self.inner.lock().unwrap();
        if enabled {
            if let Err(e) = self.start(&mut inner, true) {
                inner.enabled = false;
                inner.last_error = Some(e.clone());
                self.publish(&inner);
                return Err(e);
            }
            inner.enabled = true;
            inner.last_error = None;
        } else {
            inner.enabled = false;
            self.stop(&mut inner);
        }
        self.publish(&inner);
        Ok(self.status_of(&inner))
    }

    /// Ask the engine to report the next key press. Starts a mapping-free
    /// engine when mappings are disabled; it is stopped again afterwards.
    pub fn start_detect(self: &Arc<Self>) -> Result<Status, String> {
        let mut inner = self.inner.lock().unwrap();
        if inner.engine.is_none() {
            self.start(&mut inner, false)?;
        }
        let engine = inner.engine.as_mut().expect("engine started");
        engine.send(Command::Detect).map_err(|e| format!("Could not reach the engine: {e}"))?;
        inner.detecting = true;
        self.publish(&inner);
        Ok(self.status_of(&inner))
    }

    pub fn cancel_detect(&self) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(engine) = inner.engine.as_mut() {
            let _ = engine.send(Command::CancelDetect);
        }
    }

    /// Emergency path used by the backend's own hotkey registration.
    pub fn emergency_disable(&self) {
        let mut inner = self.inner.lock().unwrap();
        let was_active = inner.engine.is_some();
        inner.enabled = false;
        self.stop(&mut inner);
        if was_active {
            inner.last_error = Some("Emergency shortcut pressed. All mappings are disabled.".into());
        }
        self.publish(&inner);
    }

    pub fn shutdown(&self) {
        let mut inner = self.inner.lock().unwrap();
        self.stop(&mut inner);
    }

    fn stop(&self, inner: &mut Inner) {
        inner.detecting = false;
        if let Some(engine) = inner.engine.take() {
            inner.generation += 1;
            engine.stop(STOP_GRACE);
        }
    }

    fn start(self: &Arc<Self>, inner: &mut Inner, with_mappings: bool) -> Result<(), String> {
        self.stop(inner);
        let exe = self.ahk_path().ok_or(
            "AutoHotkey v2 was not found. Reinstall KeyForge, or install AutoHotkey v2 from autohotkey.com.",
        )?;
        let empty = Vec::new();
        let mappings = if with_mappings { &inner.mappings } else { &empty };
        let (report, resolved) = validate::validate(mappings);
        if !report.ok() {
            let messages: Vec<_> = report.errors.iter().map(|i| i.message.as_str()).collect();
            return Err(messages.join("\n"));
        }
        write_atomic(&self.script_path, &ahk::generate(&resolved)).map_err(|e| format!("Could not write the engine script: {e}"))?;

        let (engine, rx) = Engine::spawn(&Launch::autohotkey(&exe, &self.script_path)).map_err(|e| format!("Could not start AutoHotkey: {e}"))?;
        let ready = wait_ready(&rx, READY_TIMEOUT).map_err(|e| e.to_string())?;
        inner.generation += 1;
        inner.ahk_version = Some(ready.ahk_version);
        inner.elevated_engine = ready.admin;
        inner.engine = Some(engine);
        let host = Arc::clone(self);
        let generation = inner.generation;
        thread::Builder::new()
            .name("kf-engine-events".into())
            .spawn(move || host.forward(generation, rx))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn forward(&self, generation: u64, rx: Receiver<EngineEvent>) {
        let mut exit_reason = None;
        for event in rx {
            let mut inner = self.inner.lock().unwrap();
            if inner.generation != generation {
                // Replaced or stopped on purpose; nothing to recover.
                continue;
            }
            match event {
                EngineEvent::Protocol(Event::Key(key)) => {
                    let _ = self.app.emit::<DetectedKey>("kf://key-detected", key);
                }
                EngineEvent::Protocol(Event::DetectEnded { reason }) => {
                    inner.detecting = false;
                    if reason != "done" {
                        let _ = self.app.emit("kf://detect-ended", reason);
                    }
                    if !inner.enabled {
                        self.stop(&mut inner);
                    }
                    self.publish(&inner);
                }
                EngineEvent::Protocol(Event::Exit { reason }) => exit_reason = Some(reason),
                EngineEvent::Protocol(Event::Error { message }) => {
                    let _ = self.app.emit("kf://log", format!("engine error: {message}"));
                }
                EngineEvent::Stderr(line) | EngineEvent::Unparsed(line) => {
                    let _ = self.app.emit("kf://log", line);
                }
                EngineEvent::Protocol(_) => {}
                EngineEvent::Exited { code } => {
                    // The engine exited without being asked to.
                    inner.engine = None;
                    inner.detecting = false;
                    inner.enabled = false;
                    #[cfg(windows)]
                    keyforge_engine::win::release_stuck_modifiers();
                    inner.last_error = Some(if code == Some(exit_code::EMERGENCY) || exit_reason.as_deref() == Some("emergency") {
                        "Emergency shortcut pressed. All mappings are disabled.".into()
                    } else {
                        format!("The engine stopped unexpectedly (exit code {code:?}). Mappings are disabled and the keyboard is back to normal.")
                    });
                    self.publish(&inner);
                }
            }
        }
    }
}

/// Write via a temporary file and rename, so a crash never leaves a
/// half-written script. The UTF-8 BOM makes AutoHotkey read it as UTF-8.
fn write_atomic(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    let mut bytes = Vec::with_capacity(contents.len() + 3);
    bytes.extend_from_slice(b"\xEF\xBB\xBF");
    bytes.extend_from_slice(contents.as_bytes());
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}
