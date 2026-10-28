//! End-to-end tests against a real AutoHotkey v2 engine on Windows.
//!
//! They inject keys with SendInput and observe the result with a low-level
//! keyboard hook, so they need an interactive desktop session (GitHub's
//! Windows runners have one). Set KEYFORGE_AHK to AutoHotkey64.exe. Without
//! it the tests are skipped, unless KEYFORGE_REQUIRE_E2E=1, which turns the
//! skip into a failure (CI sets it).
#![cfg(windows)]

use keyforge_core::config::{Action, Mapping, Modifiers, Trigger};
use keyforge_core::protocol::{exit_code, Command, Event};
use keyforge_core::{ahk, keys, validate};
use keyforge_engine::{locate, wait_ready, Engine, EngineEvent, Launch};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{CloseHandle, LPARAM, LRESULT, WAIT_OBJECT_0, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{GetCurrentThreadId, OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_BACK, VK_F19, VK_F20,
    VK_F21, VK_LCONTROL, VK_LMENU, VK_LSHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG,
    WH_KEYBOARD_LL, WM_KEYUP, WM_QUIT, WM_SYSKEYUP,
};

/// The tests share the system keyboard, so they must not overlap.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

fn ahk_exe() -> Option<PathBuf> {
    let found = locate::find(None);
    if found.is_none() {
        assert!(std::env::var_os("KEYFORGE_REQUIRE_E2E").is_none(), "AutoHotkey v2 not found; set KEYFORGE_AHK");
        eprintln!("skipping: AutoHotkey v2 not found (set KEYFORGE_AHK)");
    }
    found
}

fn remap(id: &str, from: &str, to: &str) -> Mapping {
    Mapping { id: id.into(), enabled: true, trigger: Trigger { key: from.into(), mods: Modifiers::default() }, action: Action::Remap { to: to.into() } }
}

fn start(exe: &PathBuf, name: &str, mappings: &[Mapping]) -> (Engine, Receiver<EngineEvent>) {
    let (report, resolved) = validate::validate(mappings);
    assert!(report.ok(), "{report:?}");
    let dir = std::env::temp_dir().join("keyforge-e2e");
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join(format!("{name}.ahk"));
    std::fs::write(&script, format!("\u{FEFF}{}", ahk::generate(&resolved))).unwrap();
    let (engine, rx) = Engine::spawn(&Launch::autohotkey(exe, &script)).unwrap();
    let ready = wait_ready(&rx, Duration::from_secs(10)).unwrap_or_else(|e| panic!("engine did not start: {e}"));
    assert!(ready.ahk_version.starts_with("2."));
    (engine, rx)
}

fn key(vk: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: 0 } },
    }
}

fn send(inputs: &[INPUT]) {
    let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    assert_eq!(sent as usize, inputs.len(), "SendInput was blocked; an interactive desktop is required");
}

fn tap(vk: u16) {
    send(&[key(vk, false), key(vk, true)]);
}

fn wait_for(rx: &Receiver<EngineEvent>, timeout: Duration, mut pred: impl FnMut(&EngineEvent) -> bool) -> EngineEvent {
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let event = rx.recv_timeout(left).expect("timed out waiting for engine event");
        if pred(&event) {
            return event;
        }
    }
}

// --- Keyboard observer --------------------------------------------------------

static SEEN: Mutex<Vec<(u32, bool)>> = Mutex::new(Vec::new());

unsafe extern "system" fn observe_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
        let up = wparam as u32 == WM_KEYUP || wparam as u32 == WM_SYSKEYUP;
        SEEN.lock().unwrap().push((kb.vkCode, up));
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

/// A low-level hook installed *before* the engine. Windows calls the most
/// recently installed hook first, so this sees what the engine lets through.
struct Observer {
    thread_id: u32,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Observer {
    fn start() -> Observer {
        SEEN.lock().unwrap().clear();
        let (tx, rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || unsafe {
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(observe_proc), GetModuleHandleW(std::ptr::null()), 0);
            assert!(!hook.is_null(), "SetWindowsHookExW failed");
            tx.send(GetCurrentThreadId()).unwrap();
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
            UnhookWindowsHookEx(hook);
        });
        Observer { thread_id: rx.recv().unwrap(), thread: Some(thread) }
    }

    fn seen(&self) -> Vec<(u32, bool)> {
        SEEN.lock().unwrap().clone()
    }
}

impl Drop for Observer {
    fn drop(&mut self) {
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
        let _ = self.thread.take().unwrap().join();
    }
}

fn process_exits(pid: u32, timeout: Duration) -> bool {
    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            return true; // already gone
        }
        let done = WaitForSingleObject(handle, timeout.as_millis() as u32) == WAIT_OBJECT_0;
        CloseHandle(handle);
        done
    }
}

// --- Tests -----------------------------------------------------------------------

#[test]
fn every_table_key_loads_in_autohotkey() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    // Map every key the table offers (as trigger and as target) so AutoHotkey
    // itself checks every key name we can generate.
    let mut maps = Vec::new();
    for (i, def) in keys::KEYS.iter().enumerate() {
        if matches!(def.id, "Backspace" | "RCtrl" | "RShift" | "RAlt") {
            continue;
        }
        let to = if def.id == "F24" { "F23" } else { "F24" };
        maps.push(remap(&format!("m{i}"), def.id, to));
    }
    maps.push(remap("raw", "vkE9sc0F1", "Backspace"));
    maps.push(remap("sc", "SC0F2", "Media_Play_Pause"));
    maps.push(remap("vk", "vkE8sc000", "vkE9sc0F1"));
    let mut ctrl = remap("ctrl", "F13", "F14");
    ctrl.trigger.mods = Modifiers { ctrl: true, shift: true, alt: false, win: true };
    maps.push(ctrl);
    let (engine, _rx) = start(&exe, "all-keys", &maps);
    assert_eq!(engine.stop(Duration::from_secs(3)), Some(exit_code::REQUESTED));
}

#[test]
fn ping_and_clean_exit() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    let (mut engine, rx) = start(&exe, "ping", &[]);
    engine.send(Command::Ping).unwrap();
    wait_for(&rx, Duration::from_secs(3), |e| *e == EngineEvent::Protocol(Event::Pong));
    assert_eq!(engine.stop(Duration::from_secs(3)), Some(exit_code::REQUESTED));
    wait_for(&rx, Duration::from_secs(3), |e| matches!(e, EngineEvent::Exited { .. }));
}

#[test]
fn remaps_key_down_and_up() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    let observer = Observer::start();
    let (engine, _rx) = start(&exe, "remap", &[remap("r", "F20", "F21")]);
    tap(VK_F20);
    std::thread::sleep(Duration::from_millis(400));
    let seen = observer.seen();
    assert!(seen.contains(&(VK_F21 as u32, false)), "F21 down not seen: {seen:?}");
    assert!(seen.contains(&(VK_F21 as u32, true)), "F21 up not seen: {seen:?}");
    assert!(!seen.iter().any(|&(vk, _)| vk == VK_F20 as u32), "F20 leaked through: {seen:?}");

    // After the engine stops, the key behaves normally again.
    engine.stop(Duration::from_secs(3));
    SEEN.lock().unwrap().clear();
    tap(VK_F20);
    std::thread::sleep(Duration::from_millis(300));
    let seen = observer.seen();
    assert!(seen.contains(&(VK_F20 as u32, false)), "F20 not restored: {seen:?}");
    assert!(!seen.iter().any(|&(vk, _)| vk == VK_F21 as u32), "remap still active: {seen:?}");
}

#[test]
fn detects_a_key() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    // F19 is mapped, but detection reports the physical key, not its mapping.
    let (mut engine, rx) = start(&exe, "detect", &[remap("r", "F19", "F21")]);
    engine.send(Command::Detect).unwrap();
    wait_for(&rx, Duration::from_secs(3), |e| *e == EngineEvent::Protocol(Event::DetectStarted));
    tap(VK_F19);
    let EngineEvent::Protocol(Event::Key(k)) = wait_for(&rx, Duration::from_secs(3), |e| matches!(e, EngineEvent::Protocol(Event::Key(_)))) else {
        unreachable!()
    };
    assert_eq!((k.id.as_str(), k.vk, k.ctrl), ("F19", VK_F19, false), "{k:?}");
    wait_for(&rx, Duration::from_secs(3), |e| matches!(e, EngineEvent::Protocol(Event::DetectEnded { .. })));
    engine.stop(Duration::from_secs(3));
}

#[test]
fn emergency_shortcut_stops_engine() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    let (engine, rx) = start(&exe, "emergency", &[remap("r", "F20", "F21")]);
    send(&[key(VK_LCONTROL, false), key(VK_LMENU, false), key(VK_LSHIFT, false), key(VK_BACK, false), key(VK_BACK, true)]);
    let exited = wait_for(&rx, Duration::from_secs(5), |e| matches!(e, EngineEvent::Exited { .. }));
    send(&[key(VK_LSHIFT, true), key(VK_LMENU, true), key(VK_LCONTROL, true)]);
    assert_eq!(exited, EngineEvent::Exited { code: Some(exit_code::EMERGENCY) });
    assert!(!engine.is_running());
    keyforge_engine::win::release_stuck_modifiers();
    for vk in [VK_LCONTROL, VK_LMENU, VK_LSHIFT] {
        assert!(unsafe { GetAsyncKeyState(vk as i32) } >= 0, "modifier {vk:#x} stuck");
    }
}

#[test]
fn engine_exits_when_stdin_closes() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    let (mut engine, rx) = start(&exe, "stdin", &[]);
    engine.close_stdin();
    let exited = wait_for(&rx, Duration::from_secs(3), |e| matches!(e, EngineEvent::Exited { .. }));
    assert_eq!(exited, EngineEvent::Exited { code: Some(exit_code::SUPERVISOR_LOST) });
}

/// Helper for `engine_dies_with_supervisor`: run in a child process, starts
/// an engine, prints its PID and waits to be killed.
#[test]
#[ignore]
fn helper_hold_engine() {
    if std::env::var_os("KF_HELPER").is_none() {
        return;
    }
    let exe = locate::find(None).unwrap();
    let (engine, _rx) = start(&exe, "helper", &[remap("r", "F20", "F21")]);
    println!("ENGINE_PID={}", engine.pid());
    std::thread::sleep(Duration::from_secs(60));
}

#[test]
fn engine_dies_with_supervisor() {
    let _g = serial();
    if ahk_exe().is_none() {
        return;
    }
    let mut helper = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "helper_hold_engine", "--ignored", "--nocapture"])
        .env("KF_HELPER", "1")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut pid = None;
    for line in BufReader::new(helper.stdout.take().unwrap()).lines() {
        if let Some(p) = line.unwrap().strip_prefix("ENGINE_PID=") {
            pid = Some(p.trim().parse::<u32>().unwrap());
            break;
        }
    }
    let pid = pid.expect("helper did not start an engine");
    assert!(!process_exits(pid, Duration::from_millis(200)), "engine should be running");
    // Simulate a supervisor crash: TerminateProcess, no cleanup code runs.
    helper.kill().unwrap();
    helper.wait().unwrap();
    assert!(process_exits(pid, Duration::from_secs(3)), "engine outlived its supervisor");
}
