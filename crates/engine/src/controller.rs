//! Application logic between the UI and the engine: configuration
//! persistence, atomic apply with rollback, detection, profiles, testing,
//! health checks and recovery. It has no UI dependency; the Tauri app and
//! the tests drive it the same way.

use crate::{locate, wait_ready, Engine, EngineEvent, Launch};
use keyforge_core::ahk::{self, script_copy_name, GenOptions};
use keyforge_core::config::{Action, Config, Profile};
use keyforge_core::protocol::{exit_code, Command, Event};
use keyforge_core::store::{self, BackupInfo, Store};
use keyforge_core::validate::{self, script_kind, Report, EMERGENCY_LABEL};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::thread;
use std::time::{Duration, Instant};

const READY_TIMEOUT: Duration = Duration::from_secs(5);
const STOP_GRACE: Duration = Duration::from_millis(1500);
const MAX_LOG: usize = 100;
const MAX_SCRIPT_BYTES: u64 = 1024 * 1024;

/// Receives everything the UI needs to know. `event` names: `status`,
/// `key-detected`, `detect-ended`, `fired`, `profile`, `log`.
pub trait Sink: Send + Sync + 'static {
    fn emit(&self, event: &str, payload: Value);
}

pub type Launcher = Box<dyn Fn(&Path, &Path) -> Launch + Send + Sync>;

pub struct Options {
    pub data_dir: PathBuf,
    /// Folder holding the AutoHotkey copy shipped with the installer.
    pub bundled_ahk_dir: Option<PathBuf>,
    /// How to start the engine for (exe, script). Defaults to AutoHotkey.
    pub launcher: Option<Launcher>,
    /// Interval of engine health pings. The engine is considered hung after
    /// three intervals without an answer.
    pub health_interval: Duration,
}

impl Options {
    pub fn new(data_dir: impl Into<PathBuf>) -> Options {
        Options { data_dir: data_dir.into(), bundled_ahk_dir: None, launcher: None, health_interval: Duration::from_secs(5) }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub ahk_path: Option<String>,
    pub ahk_version: Option<String>,
    pub engine_running: bool,
    pub enabled: bool,
    pub detecting: bool,
    pub active_profile: String,
    pub elevated_engine: bool,
    pub last_error: Option<String>,
    pub notice: Option<String>,
    pub emergency_shortcut: &'static str,
    pub emergency_registered: bool,
    pub data_dir: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub config: Config,
    pub status: Status,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOutcome {
    /// Validation result. When it has errors nothing was changed.
    pub report: Report,
    pub applied: bool,
    pub config: Config,
    pub status: Status,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub report: Report,
    pub script: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScriptInfo {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Default)]
struct Inner {
    config: Config,
    engine: Option<Engine>,
    /// Incremented for every engine start and stop, so events from an engine
    /// that was replaced or stopped on purpose are ignored.
    generation: u64,
    enabled: bool,
    detecting: bool,
    ahk_version: Option<String>,
    elevated_engine: bool,
    last_error: Option<String>,
    notice: Option<String>,
    emergency_registered: bool,
    last_pong: Option<Instant>,
    pending_test: Option<(String, mpsc::Sender<()>)>,
    log: VecDeque<String>,
}

pub struct Controller {
    store: Store,
    bundled_ahk_dir: Option<PathBuf>,
    launcher: Launcher,
    health_interval: Duration,
    sink: Arc<dyn Sink>,
    inner: Mutex<Inner>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Configuration for an engine that only does key detection.
fn detection_config() -> Config {
    let mut config = Config::default();
    config.profiles.truncate(1);
    config
}

impl Controller {
    pub fn new(options: Options, sink: Arc<dyn Sink>) -> Arc<Controller> {
        let store = Store::new(&options.data_dir);
        let loaded = store.load();
        let launcher = options.launcher.unwrap_or_else(|| Box::new(|exe: &Path, script: &Path| Launch::autohotkey(exe, script)));
        let controller = Arc::new(Controller {
            store,
            bundled_ahk_dir: options.bundled_ahk_dir,
            launcher,
            health_interval: options.health_interval,
            sink,
            inner: Mutex::new(Inner { config: loaded.config, notice: loaded.notice, ..Inner::default() }),
        });
        let weak = Arc::downgrade(&controller);
        let interval = options.health_interval;
        let _ = thread::Builder::new().name("kf-health".into()).spawn(move || health_loop(weak, interval));
        controller
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn data_dir(&self) -> &Path {
        self.store.dir()
    }

    fn script_path(&self) -> PathBuf {
        self.store.dir().join("engine").join("keyforge.ahk")
    }

    fn scripts_dir(&self) -> PathBuf {
        self.store.dir().join("scripts")
    }

    fn ahk_path(&self, config: &Config) -> Option<PathBuf> {
        if let Some(custom) = config.settings.ahk_path.as_deref().filter(|p| !p.trim().is_empty()) {
            return Some(PathBuf::from(custom)).filter(|p| p.is_file());
        }
        locate::find(self.bundled_ahk_dir.as_deref())
    }

    fn status_of(&self, inner: &Inner) -> Status {
        Status {
            ahk_path: self.ahk_path(&inner.config).map(|p| p.display().to_string()),
            ahk_version: inner.ahk_version.clone(),
            engine_running: inner.engine.is_some(),
            enabled: inner.enabled,
            detecting: inner.detecting,
            active_profile: inner.config.active_profile.clone(),
            elevated_engine: inner.elevated_engine,
            last_error: inner.last_error.clone(),
            notice: inner.notice.clone(),
            emergency_shortcut: EMERGENCY_LABEL,
            emergency_registered: inner.emergency_registered,
            data_dir: self.store.dir().display().to_string(),
        }
    }

    fn publish(&self, inner: &Inner) {
        self.sink.emit("status", json!(self.status_of(inner)));
    }

    fn log(&self, inner: &mut Inner, line: String) {
        if inner.log.len() == MAX_LOG {
            inner.log.pop_front();
        }
        inner.log.push_back(line.clone());
        self.sink.emit("log", json!(line));
    }

    pub fn status(&self) -> Status {
        self.status_of(&self.lock())
    }

    pub fn snapshot(&self) -> Snapshot {
        let inner = self.lock();
        Snapshot { config: inner.config.clone(), status: self.status_of(&inner) }
    }

    /// Never blocks: used from the window's close handler on the main thread.
    pub fn close_to_tray(&self) -> bool {
        self.inner.try_lock().map(|i| i.config.settings.close_to_tray).unwrap_or(true)
    }

    pub fn logs(&self) -> Vec<String> {
        self.lock().log.iter().cloned().collect()
    }

    pub fn dismiss_notice(&self) {
        let mut inner = self.lock();
        inner.notice = None;
        inner.last_error = None;
        self.publish(&inner);
    }

    pub fn set_emergency_registered(&self, registered: bool) {
        let mut inner = self.lock();
        inner.emergency_registered = registered;
        self.publish(&inner);
    }

    /// Checks that need the file system: confirmed scripts must still match.
    fn check_files(&self, config: &Config) -> Result<(), String> {
        for p in &config.profiles {
            for m in p.mappings.iter().filter(|m| m.enabled) {
                for action in [Some(&m.action), m.hold.as_ref(), m.double_tap.as_ref()].into_iter().flatten() {
                    if let Action::Script { path, sha256, .. } = action {
                        let copy = self.scripts_dir().join(script_copy_name(sha256, path));
                        let ok = std::fs::read(&copy).map(|b| sha256_hex(&b) == sha256.to_ascii_lowercase()).unwrap_or(false);
                        if !ok {
                            return Err(format!("The confirmed copy of {path} is missing or changed. Confirm the script again."));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn preview(&self, config: &Config) -> Preview {
        let (mut report, plan) = validate::validate(config);
        if let Err(e) = self.check_files(config) {
            report.errors.push(validate::Issue { profile: None, mapping: None, message: e });
        }
        let script = plan.filter(|_| report.ok()).map(|plan| {
            let scripts = self.scripts_dir().display().to_string();
            ahk::generate(&plan, &GenOptions { scripts_dir: &scripts })
        });
        Preview { report, script }
    }

    /// Validate, save and (if mappings are on) switch the engine to `config`.
    /// The previous configuration is kept as last-known-good first, and is
    /// restored if the new engine fails to start.
    pub fn apply(self: &Arc<Self>, config: Config) -> Result<ApplyOutcome, String> {
        let preview = self.preview(&config);
        let mut inner = self.lock();
        if !preview.report.ok() {
            return Ok(ApplyOutcome { report: preview.report, applied: false, config: inner.config.clone(), status: self.status_of(&inner) });
        }
        if config != inner.config {
            self.store.save_last_good().map_err(|e| format!("Could not back up the current configuration: {e}"))?;
            self.store.save(&config).map_err(|e| format!("Could not save the configuration: {e}"))?;
            let previous = std::mem::replace(&mut inner.config, config);
            if inner.enabled {
                if let Err(e) = self.start(&mut inner, true) {
                    inner.config = previous;
                    let _ = self.store.save(&inner.config);
                    let restored = self.start(&mut inner, true);
                    let msg = match restored {
                        Ok(()) => format!("{e}\nThe previous configuration is still active."),
                        Err(_) => {
                            inner.enabled = false;
                            format!("{e}\nMappings are disabled.")
                        }
                    };
                    inner.last_error = Some(msg.clone());
                    self.publish(&inner);
                    return Err(msg);
                }
            } else {
                let profile = inner.config.active_profile.clone();
                if let Some(engine) = inner.engine.as_mut() {
                    let _ = engine.send(Command::Profile(profile));
                }
            }
        }
        inner.last_error = None;
        self.publish(&inner);
        Ok(ApplyOutcome { report: preview.report, applied: true, config: inner.config.clone(), status: self.status_of(&inner) })
    }

    pub fn set_enabled(self: &Arc<Self>, enabled: bool) -> Result<Status, String> {
        let mut inner = self.lock();
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

    pub fn set_profile(&self, id: &str) -> Result<Status, String> {
        let mut inner = self.lock();
        if inner.config.profile(id).is_none() {
            return Err(format!("There is no profile \"{id}\"."));
        }
        if inner.config.active_profile != id {
            inner.config.active_profile = id.to_string();
            self.store.save(&inner.config).map_err(|e| format!("Could not save the configuration: {e}"))?;
        }
        if inner.enabled {
            if let Some(engine) = inner.engine.as_mut() {
                let _ = engine.send(Command::Profile(id.to_string()));
            }
        }
        self.sink.emit("profile", json!(id));
        self.publish(&inner);
        Ok(self.status_of(&inner))
    }

    /// Ask the engine to report the next key press. Starts a mapping-free
    /// engine when mappings are off; it stops again after detection.
    pub fn start_detect(self: &Arc<Self>) -> Result<Status, String> {
        let mut inner = self.lock();
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
        let mut inner = self.lock();
        if let Some(engine) = inner.engine.as_mut() {
            let _ = engine.send(Command::CancelDetect);
        }
    }

    /// Run a mapping as if its key were pressed. When the key can be sent
    /// (global mapping of the active profile, on Windows), the real key press
    /// is injected so the whole path is checked; otherwise the engine runs the
    /// action directly.
    pub fn test_mapping(&self, profile: &str, mapping: &str) -> Result<String, String> {
        let tag = ahk::mapping_tag(profile, mapping);
        let (rx, key_path) = {
            let mut inner = self.lock();
            if !inner.enabled || inner.engine.is_none() {
                return Err("Turn on mappings first; the test runs through the engine.".into());
            }
            let Some(m) = inner.config.profile(profile).and_then(|p| p.mappings.iter().find(|m| m.id == mapping)) else {
                return Err("Apply your changes first; this mapping is not active yet.".into());
            };
            if !m.enabled {
                return Err("This mapping is turned off.".into());
            }
            let key_path = cfg!(windows) && m.app.is_none() && inner.config.active_profile == profile;
            let trigger = m.trigger.clone();
            let (tx, rx) = mpsc::channel();
            inner.pending_test = Some((tag.clone(), tx));
            if !key_path {
                let _ = inner.engine.as_mut().unwrap().send(Command::Test(tag.clone()));
            }
            drop(inner);
            if key_path {
                #[cfg(windows)]
                if let Some(key) = keyforge_core::keys::resolve(&trigger.key) {
                    thread::sleep(Duration::from_millis(150));
                    crate::win::send_chord(key.vk, key.sc, &trigger.mods);
                }
                #[cfg(not(windows))]
                let _ = trigger;
            }
            (rx, key_path)
        };
        let fired = rx.recv_timeout(Duration::from_millis(1500)).is_ok();
        if fired && key_path {
            return Ok("The key press reached the engine and the mapping ran.".into());
        }
        if fired {
            return Ok("The action ran. App-specific mappings respond to the key only while that app is in front.".into());
        }
        if key_path {
            // Fall back to running the action directly to tell the cases apart.
            let rx = {
                let mut inner = self.lock();
                let (tx, rx) = mpsc::channel();
                inner.pending_test = Some((tag.clone(), tx));
                if let Some(engine) = inner.engine.as_mut() {
                    let _ = engine.send(Command::Test(tag));
                }
                rx
            };
            if rx.recv_timeout(Duration::from_millis(1500)).is_ok() {
                return Err("The action works, but the simulated key press did not reach the engine. Another keyboard tool may be intercepting this key.".into());
            }
        }
        Err("The engine did not run the mapping. Check the log in Settings.".into())
    }

    /// Copy a script into KeyForge's data folder and return its hash. Only the
    /// copy is ever run, so later edits to the original need a new
    /// confirmation.
    pub fn confirm_script(&self, path: &str) -> Result<ScriptInfo, String> {
        if script_kind(path).is_none() {
            return Err("Scripts must be .ps1, .bat or .cmd files.".into());
        }
        let meta = std::fs::metadata(path).map_err(|e| format!("Cannot read {path}: {e}"))?;
        if meta.len() > MAX_SCRIPT_BYTES {
            return Err("The script is larger than 1 MB.".into());
        }
        let bytes = std::fs::read(path).map_err(|e| format!("Cannot read {path}: {e}"))?;
        let sha256 = sha256_hex(&bytes);
        store::write_atomic(&self.scripts_dir().join(script_copy_name(&sha256, path)), &bytes).map_err(|e| e.to_string())?;
        Ok(ScriptInfo { path: path.to_string(), sha256, size: meta.len() })
    }

    pub fn export(&self, path: &Path) -> Result<(), String> {
        let config = self.lock().config.clone();
        self.store.export(&config, path).map_err(|e| e.to_string())
    }

    pub fn import(self: &Arc<Self>, path: &Path) -> Result<ApplyOutcome, String> {
        let config = store::read_file(path).map_err(|e| format!("This file cannot be imported: it {e}."))?;
        self.store.backup("import").map_err(|e| e.to_string())?;
        self.apply(config)
    }

    pub fn backup_now(&self) -> Result<String, String> {
        self.store.backup("manual").map_err(|e| e.to_string())?.ok_or_else(|| "There is no saved configuration yet.".into())
    }

    pub fn list_backups(&self) -> Vec<BackupInfo> {
        self.store.list_backups()
    }

    pub fn restore_backup(self: &Arc<Self>, name: &str) -> Result<ApplyOutcome, String> {
        let config = self.store.read_backup(name).map_err(|e| format!("This backup cannot be restored: it {e}."))?;
        self.store.backup("restore").map_err(|e| e.to_string())?;
        self.apply(config)
    }

    pub fn reset(self: &Arc<Self>) -> Result<ApplyOutcome, String> {
        self.store.backup("reset").map_err(|e| e.to_string())?;
        self.apply(Config::default())
    }

    /// Emergency path for the backend's own hotkey registration.
    pub fn emergency_disable(&self) {
        let mut inner = self.lock();
        let was_running = inner.engine.is_some();
        inner.enabled = false;
        self.stop(&mut inner);
        #[cfg(windows)]
        crate::win::release_stuck_modifiers();
        if was_running {
            inner.last_error = Some(format!("Emergency shortcut ({EMERGENCY_LABEL}) pressed. All mappings are off."));
        }
        self.publish(&inner);
    }

    pub fn shutdown(&self) {
        let mut inner = self.lock();
        self.stop(&mut inner);
    }

    fn stop(&self, inner: &mut Inner) {
        inner.detecting = false;
        inner.generation += 1;
        if let Some(engine) = inner.engine.take() {
            engine.stop(STOP_GRACE);
        }
    }

    fn start(self: &Arc<Self>, inner: &mut Inner, with_mappings: bool) -> Result<(), String> {
        self.stop(inner);
        let exe = self.ahk_path(&inner.config).ok_or(
            "AutoHotkey v2 was not found. Reinstall KeyForge, set a custom AutoHotkey path in Settings, or install AutoHotkey v2.",
        )?;
        let detect_only;
        let config = if with_mappings {
            &inner.config
        } else {
            detect_only = detection_config();
            &detect_only
        };
        let preview = self.preview(config);
        let script = preview.script.ok_or_else(|| preview.report.error_text())?;
        let path = self.script_path();
        let mut bytes = b"\xEF\xBB\xBF".to_vec();
        bytes.extend_from_slice(script.as_bytes());
        store::write_atomic(&path, &bytes).map_err(|e| format!("Could not write the engine script: {e}"))?;

        let (engine, rx) = Engine::spawn(&(self.launcher)(&exe, &path)).map_err(|e| format!("Could not start AutoHotkey: {e}"))?;
        let ready = wait_ready(&rx, READY_TIMEOUT).map_err(|e| e.to_string())?;
        inner.generation += 1;
        inner.ahk_version = Some(ready.ahk_version);
        inner.elevated_engine = ready.admin;
        inner.engine = Some(engine);
        inner.last_pong = Some(Instant::now());
        let this = Arc::clone(self);
        let generation = inner.generation;
        thread::Builder::new().name("kf-engine-events".into()).spawn(move || this.forward(generation, rx)).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn forward(&self, generation: u64, rx: Receiver<EngineEvent>) {
        let mut exit_reason = None;
        for event in rx {
            let mut inner = self.lock();
            if inner.generation != generation {
                continue; // replaced or stopped on purpose
            }
            match event {
                EngineEvent::Protocol(Event::Key(key)) => self.sink.emit("key-detected", json!(key)),
                EngineEvent::Protocol(Event::DetectEnded { reason }) => {
                    inner.detecting = false;
                    if reason != "done" {
                        self.sink.emit("detect-ended", json!(reason));
                    }
                    if !inner.enabled {
                        self.stop(&mut inner);
                    }
                    self.publish(&inner);
                }
                EngineEvent::Protocol(Event::Fired { id }) => {
                    if let Some((tag, tx)) = inner.pending_test.take() {
                        if tag == id {
                            let _ = tx.send(());
                        } else {
                            inner.pending_test = Some((tag, tx));
                        }
                    }
                    let (profile, mapping) = id.split_once('/').unwrap_or(("", &id));
                    self.sink.emit("fired", json!({ "profile": profile, "mapping": mapping }));
                }
                EngineEvent::Protocol(Event::Profile { id }) => {
                    if inner.config.active_profile != id && inner.config.profile(&id).is_some() {
                        inner.config.active_profile = id.clone();
                        let _ = self.store.save(&inner.config);
                    }
                    self.sink.emit("profile", json!(id));
                    self.publish(&inner);
                }
                EngineEvent::Protocol(Event::Pong) => inner.last_pong = Some(Instant::now()),
                EngineEvent::Protocol(Event::Exit { reason }) => exit_reason = Some(reason),
                EngineEvent::Protocol(Event::Error { message }) => self.log(&mut inner, format!("Engine: {message}")),
                EngineEvent::Stderr(line) | EngineEvent::Unparsed(line) => self.log(&mut inner, line),
                EngineEvent::Protocol(Event::Ready { .. } | Event::DetectStarted) => {}
                EngineEvent::Exited { code } => {
                    // The engine exited without being asked to.
                    inner.engine = None;
                    inner.detecting = false;
                    let was_enabled = std::mem::replace(&mut inner.enabled, false);
                    #[cfg(windows)]
                    crate::win::release_stuck_modifiers();
                    let emergency = code == Some(exit_code::EMERGENCY) || exit_reason.as_deref() == Some("emergency");
                    let msg = if emergency {
                        format!("Emergency shortcut ({EMERGENCY_LABEL}) pressed. All mappings are off.")
                    } else {
                        format!("The engine stopped unexpectedly (exit code {code:?}). Mappings are off and the keyboard is back to normal.")
                    };
                    if was_enabled || emergency {
                        inner.last_error = Some(msg.clone());
                    }
                    self.log(&mut inner, msg);
                    self.publish(&inner);
                }
            }
        }
    }

    fn health_check(&self) {
        let mut inner = self.lock();
        let Some(last) = inner.last_pong else { return };
        if inner.engine.is_none() {
            return;
        }
        if last.elapsed() > self.health_interval * 3 {
            // Hung: Windows already bypasses its hook, so keys work, but its
            // mappings do not. Kill it and say so.
            inner.generation += 1;
            drop(inner.engine.take());
            inner.enabled = false;
            inner.detecting = false;
            #[cfg(windows)]
            crate::win::release_stuck_modifiers();
            let msg = "The engine stopped responding and was stopped. Mappings are off.".to_string();
            inner.last_error = Some(msg.clone());
            self.log(&mut inner, msg);
            self.publish(&inner);
        } else if let Some(engine) = inner.engine.as_mut() {
            let _ = engine.send(Command::Ping);
        }
    }

    pub fn active_profile(&self) -> Option<Profile> {
        let inner = self.lock();
        inner.config.profile(&inner.config.active_profile).cloned()
    }
}

fn health_loop(controller: Weak<Controller>, interval: Duration) {
    loop {
        thread::sleep(interval);
        match controller.upgrade() {
            Some(c) => c.health_check(),
            None => return,
        }
    }
}

#[cfg(windows)]
impl Controller {
    /// Register the emergency shortcut with Windows as a fallback for when
    /// the engine is not running or not responding.
    pub fn register_emergency_hotkey(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        let ok = crate::win::listen_emergency_hotkey(move || {
            if let Some(c) = weak.upgrade() {
                c.emergency_disable();
            }
        })
        .is_ok();
        self.set_emergency_registered(ok);
    }
}

#[cfg(all(test, unix))]
mod tests {
    //! Controller logic against a shell script that speaks the engine
    //! protocol. `tests/windows_e2e.rs` repeats the important flows with real
    //! AutoHotkey.
    use super::*;
    use keyforge_core::config::{Chord, Mapping};

    /// Fails to load when the script contains KF_FAIL; never answers pings
    /// when it contains KF_NOPONG; otherwise answers like the real engine.
    const FAKE: &str = r#"
script="$1"
if grep -q KF_FAIL "$script"; then echo "Error: simulated load failure" >&2; exit 2; fi
nopong=0; grep -q KF_NOPONG "$script" && nopong=1
echo '{"event":"ready","ahk":"2.0.19","admin":false,"pid":1,"profile":"productivity"}'
while read -r line; do
  case "$line" in
    ping) [ $nopong = 1 ] || echo '{"event":"pong"}' ;;
    exit) echo '{"event":"exit","reason":"requested"}'; exit 0 ;;
    detect) echo '{"event":"detect-started"}'
            echo '{"event":"key","name":"F13","vk":124,"sc":100}'
            echo '{"event":"detect-ended","reason":"done"}' ;;
    "profile "*) echo "{\"event\":\"profile\",\"id\":\"${line#profile }\"}" ;;
    "test "*) echo "{\"event\":\"fired\",\"id\":\"${line#test }\"}" ;;
    crash) exit 7 ;;
  esac
done
exit 2
"#;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<(String, Value)>>);

    impl Sink for Recorder {
        fn emit(&self, event: &str, payload: Value) {
            self.0.lock().unwrap().push((event.to_string(), payload));
        }
    }

    impl Recorder {
        fn wait_for(&self, event: &str) -> Value {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some((_, v)) = self.0.lock().unwrap().iter().find(|(e, _)| e == event) {
                    return v.clone();
                }
                assert!(Instant::now() < deadline, "no {event} event");
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    fn controller(name: &str, health: Duration) -> (Arc<Controller>, Arc<Recorder>) {
        let dir = std::env::temp_dir().join(format!("kf-ctl-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let fake_exe = dir.join("AutoHotkey64.exe");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&fake_exe, "").unwrap();
        let mut options = Options::new(&dir);
        options.bundled_ahk_dir = Some(dir.clone());
        options.health_interval = health;
        options.launcher = Some(Box::new(|_exe: &Path, script: &Path| Launch {
            program: "sh".into(),
            args: vec!["-c".into(), FAKE.into(), "sh".into(), script.into()],
        }));
        let sink = Arc::new(Recorder::default());
        (Controller::new(options, sink.clone()), sink)
    }

    fn remap(id: &str, from: &str, to: &str) -> Mapping {
        Mapping {
            id: id.into(),
            name: String::new(),
            enabled: true,
            trigger: Chord::new(from),
            action: Action::Remap { to: to.into() },
            hold: None,
            double_tap: None,
            hold_ms: None,
            app: None,
            edited: 0,
        }
    }

    fn with(mappings: Vec<Mapping>) -> Config {
        let mut c = Config::default();
        c.profiles[0].mappings = mappings;
        c
    }

    fn text(id: &str, key: &str, text: &str) -> Mapping {
        Mapping { action: Action::Text { text: text.into() }, ..remap(id, key, "F1") }
    }

    #[test]
    fn starts_disabled_and_persists_applied_config() {
        if std::env::var_os(locate::ENV_OVERRIDE).is_some() {
            return; // the override would replace the fake executable
        }
        let (c, _) = controller("persist", Duration::from_secs(60));
        let status = c.status();
        assert!(!status.enabled && !status.engine_running);
        let config = with(vec![remap("a", "F13", "F14")]);
        let outcome = c.apply(config.clone()).unwrap();
        assert!(outcome.applied && outcome.report.ok());
        assert!(!c.status().engine_running, "applying does not enable");
        let reopened = Store::new(c.data_dir()).load();
        assert_eq!(reopened.config, config);
        assert_eq!(Store::new(c.data_dir()).read_last_good().unwrap(), Config::default());

        let status = c.set_enabled(true).unwrap();
        assert!(status.enabled && status.engine_running);
        let script = std::fs::read_to_string(c.data_dir().join("engine/keyforge.ahk")).unwrap();
        assert!(script.contains("*F13::KF_RemapDown(\"productivity/a\", \"F13\", \"F14\")"));
        c.set_enabled(false).unwrap();
        assert!(!c.status().engine_running);
    }

    #[test]
    fn invalid_config_is_not_saved() {
        if std::env::var_os(locate::ENV_OVERRIDE).is_some() {
            return;
        }
        let (c, _) = controller("invalid", Duration::from_secs(60));
        let outcome = c.apply(with(vec![remap("a", "Backspace", "F14")])).unwrap();
        assert!(!outcome.applied && !outcome.report.ok());
        assert_eq!(Store::new(c.data_dir()).load().config, Config::default());
    }

    #[test]
    fn failed_engine_start_rolls_back() {
        if std::env::var_os(locate::ENV_OVERRIDE).is_some() {
            return;
        }
        let (c, _) = controller("rollback", Duration::from_secs(60));
        let good = with(vec![remap("a", "F13", "F14")]);
        c.apply(good.clone()).unwrap();
        c.set_enabled(true).unwrap();
        let err = c.apply(with(vec![text("b", "F15", "KF_FAIL")])).unwrap_err();
        assert!(err.contains("simulated load failure") && err.contains("previous configuration"), "{err}");
        let snap = c.snapshot();
        assert_eq!(snap.config, good);
        assert!(snap.status.enabled && snap.status.engine_running);
        assert_eq!(Store::new(c.data_dir()).load().config, good, "disk rolled back too");
        c.shutdown();
    }

    #[test]
    fn detection_uses_temporary_engine() {
        if std::env::var_os(locate::ENV_OVERRIDE).is_some() {
            return;
        }
        let (c, sink) = controller("detect", Duration::from_secs(60));
        c.start_detect().unwrap();
        let key = sink.wait_for("key-detected");
        assert_eq!(key["id"], "F13");
        let deadline = Instant::now() + Duration::from_secs(5);
        while c.status().engine_running {
            assert!(Instant::now() < deadline, "detection engine kept running");
            thread::sleep(Duration::from_millis(10));
        }
        assert!(!c.status().enabled);
    }

    #[test]
    fn profiles_switch_and_persist() {
        if std::env::var_os(locate::ENV_OVERRIDE).is_some() {
            return;
        }
        let (c, sink) = controller("profile", Duration::from_secs(60));
        c.set_enabled(true).unwrap();
        c.set_profile("coding").unwrap();
        assert_eq!(sink.wait_for("profile"), json!("coding"));
        assert_eq!(Store::new(c.data_dir()).load().config.active_profile, "coding");
        assert!(c.set_profile("nope").is_err());
        c.shutdown();
    }

    #[test]
    fn test_command_runs_action() {
        if std::env::var_os(locate::ENV_OVERRIDE).is_some() {
            return;
        }
        let (c, _) = controller("test", Duration::from_secs(60));
        assert!(c.test_mapping("productivity", "a").is_err(), "mappings off");
        let mut m = remap("a", "F13", "F14");
        m.app = Some("code.exe".into());
        c.apply(with(vec![m])).unwrap();
        c.set_enabled(true).unwrap();
        assert!(c.test_mapping("productivity", "a").unwrap().contains("action ran"));
        assert!(c.test_mapping("productivity", "missing").is_err());
        c.shutdown();
    }

    #[test]
    fn hung_engine_is_stopped() {
        if std::env::var_os(locate::ENV_OVERRIDE).is_some() {
            return;
        }
        let (c, _) = controller("hung", Duration::from_millis(100));
        c.apply(with(vec![text("a", "F13", "KF_NOPONG")])).unwrap();
        c.set_enabled(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while c.status().engine_running {
            assert!(Instant::now() < deadline, "hung engine not stopped");
            thread::sleep(Duration::from_millis(20));
        }
        let status = c.status();
        assert!(!status.enabled);
        assert!(status.last_error.unwrap().contains("stopped responding"));
    }

    #[test]
    fn unexpected_exit_turns_mappings_off() {
        if std::env::var_os(locate::ENV_OVERRIDE).is_some() {
            return;
        }
        let (c, _) = controller("crash", Duration::from_secs(60));
        c.set_enabled(true).unwrap();
        c.lock().engine.as_mut().unwrap().send_raw("crash\n").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while c.status().engine_running {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(10));
        }
        let status = c.status();
        assert!(!status.enabled);
        assert!(status.last_error.unwrap().contains("exit code Some(7)"));
    }

    #[test]
    fn scripts_must_match_confirmed_copy() {
        if std::env::var_os(locate::ENV_OVERRIDE).is_some() {
            return;
        }
        let (c, _) = controller("script", Duration::from_secs(60));
        let original = c.data_dir().join("hello.ps1");
        std::fs::write(&original, "Write-Output hi").unwrap();
        assert!(c.confirm_script(c.data_dir().join("x.exe").to_str().unwrap()).is_err());
        let info = c.confirm_script(original.to_str().unwrap()).unwrap();
        let mut m = remap("a", "F13", "F14");
        m.action = Action::Script { path: info.path.clone(), sha256: info.sha256.clone(), confirm_each_run: true };
        let config = with(vec![m]);
        assert!(c.apply(config.clone()).unwrap().applied);
        // Tampering with the private copy is detected.
        std::fs::write(c.data_dir().join("scripts").join(script_copy_name(&info.sha256, &info.path)), "evil").unwrap();
        let mut changed = config.clone();
        changed.settings.tap_hold_ms = 300;
        let outcome = c.apply(changed).unwrap();
        assert!(!outcome.applied);
        assert!(outcome.report.error_text().contains("Confirm the script again"));
    }

    #[test]
    fn import_export_backup_reset() {
        if std::env::var_os(locate::ENV_OVERRIDE).is_some() {
            return;
        }
        let (c, _) = controller("io", Duration::from_secs(60));
        let config = with(vec![remap("a", "F13", "F14")]);
        c.apply(config.clone()).unwrap();
        let file = c.data_dir().join("export.json");
        c.export(&file).unwrap();
        c.reset().unwrap();
        assert_eq!(c.snapshot().config, Config::default());
        assert!(c.list_backups().iter().any(|b| b.name.ends_with("-reset.json")));
        c.import(&file).unwrap();
        assert_eq!(c.snapshot().config, config);
        std::fs::write(&file, "{\"version\":1}").unwrap();
        assert!(c.import(&file).is_err());
        let name = c.backup_now().unwrap();
        c.restore_backup(&name).unwrap();
        assert_eq!(c.snapshot().config, config);
    }
}
