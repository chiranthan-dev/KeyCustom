use keyforge_core::protocol::{self, Command, Event};
use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    Protocol(Event),
    /// A stdout line that is not a protocol event.
    Unparsed(String),
    /// A line on stderr, e.g. an AutoHotkey load-time error.
    Stderr(String),
    /// The process has exited. Always the last event.
    Exited { code: Option<i32> },
}

/// How to start the engine process.
#[derive(Debug, Clone)]
pub struct Launch {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

impl Launch {
    pub fn autohotkey(exe: &Path, script: &Path) -> Self {
        // /ErrorStdOut sends load-time errors to stderr instead of a dialog box.
        Launch { program: exe.into(), args: vec!["/ErrorStdOut=UTF-8".into(), script.into()] }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ready {
    pub ahk_version: String,
    pub admin: bool,
}

#[derive(Debug)]
pub enum StartError {
    Spawn(io::Error),
    /// The engine exited before it was ready. Holds everything it printed.
    Failed { code: Option<i32>, output: String },
    Timeout { output: String },
    UnsupportedVersion(String),
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartError::Spawn(e) => write!(f, "Could not start AutoHotkey: {e}"),
            StartError::Failed { code, output } => {
                write!(f, "The engine stopped during startup (exit code {code:?}).")?;
                if !output.is_empty() {
                    write!(f, "\n{output}")?;
                }
                Ok(())
            }
            StartError::Timeout { output } => write!(f, "The engine did not become ready in time.\n{output}"),
            StartError::UnsupportedVersion(v) => write!(f, "AutoHotkey v2 is required, found {v}."),
        }
    }
}

impl std::error::Error for StartError {}

/// A running engine process. Dropping it kills the process.
pub struct Engine {
    child: Arc<Mutex<Child>>,
    stdin: Option<ChildStdin>,
    pid: u32,
}

impl Engine {
    /// Start the process. Events, ending with `Exited`, arrive on the receiver.
    pub fn spawn(launch: &Launch) -> io::Result<(Engine, mpsc::Receiver<EngineEvent>)> {
        let mut cmd = std::process::Command::new(&launch.program);
        cmd.args(&launch.args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
        }
        let mut child = cmd.spawn()?;
        #[cfg(windows)]
        if let Err(e) = crate::win::assign_to_kill_job(&child) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }

        let pid = child.id();
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let stdin = child.stdin.take();
        let child = Arc::new(Mutex::new(child));
        let (tx, rx) = mpsc::channel();

        let err_tx = tx.clone();
        let stderr_thread = thread::Builder::new().name("kf-engine-stderr".into()).spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                if !line.trim().is_empty() {
                    let _ = err_tx.send(EngineEvent::Stderr(line));
                }
            }
        })?;

        let waiter = Arc::clone(&child);
        thread::Builder::new().name("kf-engine-stdout".into()).spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if line.trim().is_empty() {
                    continue;
                }
                let event = match protocol::parse_event(&line) {
                    Ok(event) => EngineEvent::Protocol(event),
                    Err(_) => EngineEvent::Unparsed(line),
                };
                let _ = tx.send(event);
            }
            let _ = stderr_thread.join();
            let code = wait_or_kill(&waiter, Duration::from_secs(3));
            let _ = tx.send(EngineEvent::Exited { code });
        })?;

        Ok((Engine { child, stdin, pid }, rx))
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn send(&mut self, command: Command) -> io::Result<()> {
        self.send_raw(&command.line())
    }

    pub fn send_raw(&mut self, line: &str) -> io::Result<()> {
        let stdin = self.stdin.as_mut().ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "engine stdin closed"))?;
        stdin.write_all(line.as_bytes())?;
        stdin.flush()
    }

    pub fn is_running(&self) -> bool {
        matches!(self.child.lock().unwrap().try_wait(), Ok(None))
    }

    /// Ask the engine to exit, and kill it if it has not exited after `grace`.
    pub fn stop(mut self, grace: Duration) -> Option<i32> {
        let _ = self.send(Command::Exit);
        self.stdin = None;
        wait_or_kill(&self.child, grace)
    }

    /// Close stdin without sending a command (used to test the watchdog).
    pub fn close_stdin(&mut self) {
        self.stdin = None;
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.stdin = None;
        let mut child = self.child.lock().unwrap();
        if let Ok(None) = child.try_wait() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn wait_or_kill(child: &Mutex<Child>, grace: Duration) -> Option<i32> {
    let deadline = Instant::now() + grace;
    loop {
        {
            let mut child = child.lock().unwrap();
            match child.try_wait() {
                Ok(Some(status)) => return status.code(),
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    return child.wait().ok().and_then(|s| s.code());
                }
                Ok(None) => {}
                Err(_) => return None,
            }
        }
        thread::sleep(Duration::from_millis(15));
    }
}

/// Wait for the `ready` event. Any output seen before it is returned in the
/// error, so AutoHotkey syntax errors reach the user verbatim.
pub fn wait_ready(rx: &mpsc::Receiver<EngineEvent>, timeout: Duration) -> Result<Ready, StartError> {
    let deadline = Instant::now() + timeout;
    let mut output = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(EngineEvent::Protocol(Event::Ready { ahk, admin, .. })) => {
                if !ahk.starts_with("2.") {
                    return Err(StartError::UnsupportedVersion(ahk));
                }
                return Ok(Ready { ahk_version: ahk, admin });
            }
            Ok(EngineEvent::Protocol(Event::Error { message })) => output.push(message),
            Ok(EngineEvent::Stderr(line) | EngineEvent::Unparsed(line)) => output.push(line),
            Ok(EngineEvent::Protocol(_)) => {}
            Ok(EngineEvent::Exited { code }) => return Err(StartError::Failed { code, output: output.join("\n") }),
            Err(mpsc::RecvTimeoutError::Timeout) => return Err(StartError::Timeout { output: output.join("\n") }),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(StartError::Failed { code: None, output: output.join("\n") }),
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    //! Lifecycle tests against a shell script that speaks the engine protocol.
    //! The real AutoHotkey engine is exercised by `tests/windows_e2e.rs`.
    use super::*;

    const FAKE: &str = r#"
echo '{"event":"ready","ahk":"2.0.18","admin":false,"pid":1}'
while read -r line; do
  case "$line" in
    ping) echo '{"event":"pong"}' ;;
    exit) echo '{"event":"exit","reason":"requested"}'; exit 0 ;;
  esac
done
exit 2
"#;

    fn sh(script: &str) -> Launch {
        Launch { program: "sh".into(), args: vec!["-c".into(), script.into()] }
    }

    fn next(rx: &mpsc::Receiver<EngineEvent>) -> EngineEvent {
        rx.recv_timeout(Duration::from_secs(5)).expect("event")
    }

    #[test]
    fn ready_ping_and_clean_stop() {
        let (mut engine, rx) = Engine::spawn(&sh(FAKE)).unwrap();
        assert_eq!(wait_ready(&rx, Duration::from_secs(5)).unwrap().ahk_version, "2.0.18");
        engine.send(Command::Ping).unwrap();
        assert_eq!(next(&rx), EngineEvent::Protocol(Event::Pong));
        assert_eq!(engine.stop(Duration::from_secs(2)), Some(0));
        assert_eq!(next(&rx), EngineEvent::Protocol(Event::Exit { reason: "requested".into() }));
        assert_eq!(next(&rx), EngineEvent::Exited { code: Some(0) });
    }

    #[test]
    fn exits_when_supervisor_closes_stdin() {
        let (mut engine, rx) = Engine::spawn(&sh(FAKE)).unwrap();
        wait_ready(&rx, Duration::from_secs(5)).unwrap();
        engine.close_stdin();
        assert_eq!(next(&rx), EngineEvent::Exited { code: Some(2) });
        assert!(!engine.is_running());
    }

    #[test]
    fn startup_errors_are_reported() {
        let (_engine, rx) = Engine::spawn(&sh("echo 'Error: bad hotkey' >&2; exit 2")).unwrap();
        match wait_ready(&rx, Duration::from_secs(5)) {
            Err(StartError::Failed { code: Some(2), output }) => assert!(output.contains("bad hotkey")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn rejects_autohotkey_v1() {
        let (_engine, rx) = Engine::spawn(&sh(r#"echo '{"event":"ready","ahk":"1.1.37","admin":false,"pid":1}'; sleep 5"#)).unwrap();
        assert!(matches!(wait_ready(&rx, Duration::from_secs(5)), Err(StartError::UnsupportedVersion(_))));
    }

    #[test]
    fn hung_engine_is_killed() {
        let (engine, rx) = Engine::spawn(&sh("trap '' TERM; while true; do sleep 1; done")).unwrap();
        let started = Instant::now();
        assert_eq!(engine.stop(Duration::from_millis(300)), None, "killed by signal");
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(matches!(next(&rx), EngineEvent::Exited { .. }));
    }

    #[test]
    fn drop_kills_engine() {
        let (engine, rx) = Engine::spawn(&sh("exec sleep 30")).unwrap();
        drop(engine);
        assert!(matches!(next(&rx), EngineEvent::Exited { .. }));
    }
}
