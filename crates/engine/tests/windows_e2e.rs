//! End-to-end tests against a real AutoHotkey v2 engine on Windows.
//!
//! They inject keys with SendInput and observe the result with a low-level
//! keyboard hook, so they need an interactive desktop session (GitHub's
//! Windows runners have one). Set KEYFORGE_AHK to AutoHotkey64.exe. Without
//! it the tests are skipped, unless KEYFORGE_REQUIRE_E2E=1, which turns the
//! skip into a failure (CI sets it). Run with --test-threads=1.
#![cfg(windows)]

use keyforge_core::ahk::{self, GenOptions};
use keyforge_core::config::{Action, AppMode, AppRule, Chord, Config, Macro, MediaAction, Mapping, Modifiers, Step, SystemAction};
use keyforge_core::keys;
use keyforge_core::protocol::{exit_code, Command, Event};
use keyforge_core::validate;
use keyforge_engine::controller::{Controller, Options, Sink};
use keyforge_engine::{locate, wait_ready, Engine, EngineEvent, Launch};
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{CloseHandle, LPARAM, LRESULT, WAIT_OBJECT_0, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{GetCurrentThreadId, OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, VK_BACK,
    VK_F19, VK_F20, VK_F21, VK_F22, VK_F23, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG,
    WH_KEYBOARD_LL, WM_KEYUP, WM_QUIT, WM_SYSKEYUP,
};

const VK_PACKET: u32 = 0xE7;

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

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("keyforge-e2e").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn mapping(id: &str, key: &str, action: Action) -> Mapping {
    Mapping {
        id: id.into(),
        name: String::new(),
        enabled: true,
        trigger: Chord::new(key),
        action,
        hold: None,
        double_tap: None,
        hold_ms: None,
        app: None,
        edited: 0,
    }
}

fn remap(id: &str, from: &str, to: &str) -> Mapping {
    mapping(id, from, Action::Remap { to: to.into() })
}

fn chord(key: &str, ctrl: bool, shift: bool, alt: bool, win: bool) -> Chord {
    Chord { key: key.into(), mods: Modifiers { ctrl, shift, alt, win } }
}

fn config_with(mappings: Vec<Mapping>) -> Config {
    let mut c = Config::default();
    c.profiles[0].mappings = mappings;
    c
}

fn start_config(exe: &PathBuf, name: &str, config: &Config) -> (Engine, Receiver<EngineEvent>) {
    let (report, plan) = validate::validate(config);
    assert!(report.ok(), "{report:?}");
    let dir = temp_dir(name);
    let scripts = dir.display().to_string();
    let script = dir.join("engine.ahk");
    std::fs::write(&script, format!("\u{FEFF}{}", ahk::generate(&plan.unwrap(), &GenOptions { scripts_dir: &scripts }))).unwrap();
    let (engine, rx) = Engine::spawn(&Launch::autohotkey(exe, &script)).unwrap();
    let ready = wait_ready(&rx, Duration::from_secs(10)).unwrap_or_else(|e| panic!("engine did not start: {e}"));
    assert!(ready.ahk_version.starts_with("2."));
    (engine, rx)
}

fn start(exe: &PathBuf, name: &str, mappings: Vec<Mapping>) -> (Engine, Receiver<EngineEvent>) {
    start_config(exe, name, &config_with(mappings))
}

fn key(vk: u16, up: bool) -> INPUT {
    let ext = if vk == VK_LWIN { KEYEVENTF_EXTENDEDKEY } else { 0 };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: ext | if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: 0 } },
    }
}

fn send(inputs: &[INPUT]) {
    let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    assert_eq!(sent as usize, inputs.len(), "SendInput was blocked; an interactive desktop is required");
}

fn tap(vk: u16) {
    send(&[key(vk, false), key(vk, true)]);
}

fn settle(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
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

/// (vk, key-up, timestamp ms)
static SEEN: Mutex<Vec<(u32, bool, u32)>> = Mutex::new(Vec::new());

unsafe extern "system" fn observe_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
        let up = wparam as u32 == WM_KEYUP || wparam as u32 == WM_SYSKEYUP;
        SEEN.lock().unwrap().push((kb.vkCode, up, kb.time));
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

    fn clear(&self) {
        SEEN.lock().unwrap().clear();
    }

    fn seen(&self) -> Vec<(u32, bool, u32)> {
        SEEN.lock().unwrap().clone()
    }

    fn saw(&self, vk: u16, up: bool) -> bool {
        self.seen().iter().any(|&(v, u, _)| v == vk as u32 && u == up)
    }

    fn saw_any(&self, vk: u16) -> bool {
        self.seen().iter().any(|&(v, _, _)| v == vk as u32)
    }

    fn index(&self, vk: u16, up: bool) -> Option<usize> {
        self.seen().iter().position(|&(v, u, _)| v == vk as u32 && u == up)
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

fn assert_no_stuck_modifiers() {
    keyforge_engine::win::release_stuck_modifiers();
    for vk in [VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN] {
        assert!(unsafe { GetAsyncKeyState(vk as i32) } >= 0, "modifier {vk:#x} stuck");
    }
}

// --- Script validity: everything we can generate must load ---------------------

#[test]
fn every_table_key_loads_in_autohotkey() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
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
    let (engine, _rx) = start(&exe, "all-keys", maps);
    assert_eq!(engine.stop(Duration::from_secs(3)), Some(exit_code::REQUESTED));
}

#[test]
fn every_action_and_feature_loads_in_autohotkey() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    let mut c = Config::default();
    let mut maps = vec![
        mapping("sc", "F13", Action::Shortcut { keys: chord("S", true, true, false, false) }),
        mapping("la", "F14", Action::Launch { path: "C:\\Windows\\System32\\notepad.exe".into(), args: "\"C:\\a b.txt\"".into() }),
        mapping("ur", "F15", Action::Url { url: "https://example.com/?a=1&b=%20".into() }),
        mapping("tx", "F16", Action::Text { text: "Quote \" tick ` semi ; percent % brace {x} newline\nend é😀".into() }),
        mapping("mc", "F17", Action::Macro { macro_id: "mac".into() }),
        mapping("sp", "F18", Action::Script { path: "C:\\x\\go.ps1".into(), sha256: "ab".repeat(32), confirm_each_run: true }),
        mapping("bt", "F19", Action::Script { path: "C:\\x\\go.cmd".into(), sha256: "cd".repeat(32), confirm_each_run: false }),
    ];
    for (i, action) in [MediaAction::PlayPause, MediaAction::Next, MediaAction::Previous, MediaAction::Stop, MediaAction::VolumeUp, MediaAction::VolumeDown, MediaAction::Mute]
        .into_iter()
        .enumerate()
    {
        maps.push(mapping(&format!("media{i}"), &format!("Numpad{i}"), Action::Media { action }));
    }
    let letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    for (i, action) in SystemAction::ALL.into_iter().enumerate() {
        let mut m = mapping(&format!("sys{i}"), &letters[i..i + 1], Action::System { action });
        m.trigger.mods.ctrl = true;
        m.trigger.mods.alt = true;
        maps.push(m);
    }
    maps[0].hold = Some(Action::Media { action: MediaAction::Mute });
    maps[0].double_tap = Some(Action::Remap { to: "F22".into() });
    maps[0].hold_ms = Some(400);
    let mut app = remap("app", "F13", "F21");
    app.app = Some("notepad.exe".into());
    maps.push(app);
    c.profiles[0].mappings = maps;
    c.profiles[0].macros.push(Macro {
        id: "mac".into(),
        name: "All steps".into(),
        steps: vec![
            Step::Keys { keys: chord("C", true, false, false, false) },
            Step::Delay { ms: 10 },
            Step::Text { text: "x\"y".into() },
            Step::Launch { path: "notepad.exe".into(), args: String::new() },
            Step::Url { url: "https://example.org".into() },
        ],
    });
    c.profiles[0].app_rules.push(AppRule { id: "g".into(), process: "game.exe".into(), name: String::new(), mode: AppMode::Disable, enabled: true });
    c.profiles[1].switch_key = Some(chord("D2", true, false, true, false));
    c.profiles[1].auto_apps = vec!["code.exe".into()];
    c.profiles[1].mappings = vec![remap("x", "F13", "F14")];
    let (engine, _rx) = start_config(&exe, "all-actions", &c);
    assert_eq!(engine.stop(Duration::from_secs(3)), Some(exit_code::REQUESTED));
}

// --- Engine behaviour -------------------------------------------------------------

#[test]
fn ping_and_clean_exit() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    let (mut engine, rx) = start(&exe, "ping", vec![]);
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
    let (engine, rx) = start(&exe, "remap", vec![remap("r", "F20", "F21")]);
    tap(VK_F20);
    settle(400);
    assert!(observer.saw(VK_F21, false) && observer.saw(VK_F21, true), "F21 not seen: {:?}", observer.seen());
    assert!(!observer.saw_any(VK_F20), "F20 leaked through: {:?}", observer.seen());
    wait_for(&rx, Duration::from_secs(2), |e| *e == EngineEvent::Protocol(Event::Fired { id: "productivity/r".into() }));

    engine.stop(Duration::from_secs(3));
    observer.clear();
    tap(VK_F20);
    settle(300);
    assert!(observer.saw(VK_F20, false), "F20 not restored: {:?}", observer.seen());
    assert!(!observer.saw_any(VK_F21), "remap still active: {:?}", observer.seen());
}

#[test]
fn modifier_trigger_sends_clean_target() {
    // Keys like the Copilot key arrive as a combination (Win+Shift+F23). The
    // target must be sent without those modifiers held.
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    let observer = Observer::start();
    let mut m = remap("copilot", "F23", "F21");
    m.trigger.mods = Modifiers { shift: true, win: true, ..Default::default() };
    let (engine, _rx) = start(&exe, "mod-trigger", vec![m]);
    send(&[key(VK_LWIN, false), key(VK_LSHIFT, false), key(VK_F23, false), key(VK_F23, true), key(VK_LSHIFT, true), key(VK_LWIN, true)]);
    settle(500);
    let f21 = observer.index(VK_F21, false).unwrap_or_else(|| panic!("F21 not sent: {:?}", observer.seen()));
    let released = observer.seen()[..f21].iter().any(|&(vk, up, _)| up && (vk == VK_LSHIFT as u32 || vk == VK_LWIN as u32));
    assert!(released, "modifiers were still held when F21 was sent: {:?}", observer.seen());
    assert!(!observer.saw_any(VK_F23), "F23 leaked through");
    engine.stop(Duration::from_secs(3));
    assert_no_stuck_modifiers();
}

#[test]
fn shortcut_and_text_actions() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    let observer = Observer::start();
    let (engine, _rx) = start(
        &exe,
        "shortcut-text",
        vec![
            mapping("s", "F20", Action::Shortcut { keys: chord("F22", true, true, false, false) }),
            mapping("t", "F21", Action::Text { text: "é→".into() }),
        ],
    );
    tap(VK_F20);
    settle(400);
    let f22 = observer.index(VK_F22, false).unwrap_or_else(|| panic!("F22 not sent: {:?}", observer.seen()));
    assert!(observer.seen()[..f22].iter().any(|&(vk, up, _)| !up && (vk == VK_LCONTROL as u32 || vk == 0x11)), "Ctrl not held for shortcut");
    observer.clear();
    tap(VK_F21);
    settle(400);
    let packets = observer.seen().iter().filter(|&&(vk, _, _)| vk == VK_PACKET).count();
    assert!(packets >= 4, "expected Unicode text input, saw {:?}", observer.seen());
    engine.stop(Duration::from_secs(3));
    assert_no_stuck_modifiers();
}

#[test]
fn tap_hold_and_double_tap() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    let observer = Observer::start();
    let mut m = remap("th", "F20", "F21");
    m.hold = Some(Action::Shortcut { keys: Chord::new("F22") });
    m.double_tap = Some(Action::Shortcut { keys: Chord::new("F23") });
    let (engine, _rx) = start(&exe, "tap-hold", vec![m]);

    tap(VK_F20);
    settle(700); // single tap fires after the double-tap window
    assert!(observer.saw(VK_F21, false) && !observer.saw_any(VK_F22) && !observer.saw_any(VK_F23), "tap: {:?}", observer.seen());

    observer.clear();
    send(&[key(VK_F20, false)]);
    settle(450);
    send(&[key(VK_F20, true)]);
    settle(400);
    assert!(observer.saw(VK_F22, false) && !observer.saw_any(VK_F21) && !observer.saw_any(VK_F23), "hold: {:?}", observer.seen());

    observer.clear();
    tap(VK_F20);
    settle(60);
    tap(VK_F20);
    settle(700);
    assert!(observer.saw(VK_F23, false) && !observer.saw_any(VK_F21) && !observer.saw_any(VK_F22), "double: {:?}", observer.seen());
    engine.stop(Duration::from_secs(3));
}

#[test]
fn held_key_auto_repeat_fires_action_once() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    let observer = Observer::start();
    let (engine, rx) = start(&exe, "repeat", vec![mapping("s", "F20", Action::Shortcut { keys: Chord::new("F21") })]);
    // Simulate auto-repeat: several downs, then one up.
    send(&[key(VK_F20, false)]);
    for _ in 0..5 {
        settle(40);
        send(&[key(VK_F20, false)]);
    }
    send(&[key(VK_F20, true)]);
    settle(400);
    let downs = observer.seen().iter().filter(|&&(vk, up, _)| vk == VK_F21 as u32 && !up).count();
    assert_eq!(downs, 1, "{:?}", observer.seen());
    let fired = rx.try_iter().filter(|e| matches!(e, EngineEvent::Protocol(Event::Fired { .. }))).count();
    assert_eq!(fired, 1);
    engine.stop(Duration::from_secs(3));
}

#[test]
fn macro_runs_steps_in_order() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    let observer = Observer::start();
    let mut c = config_with(vec![mapping("m", "F20", Action::Macro { macro_id: "mac".into() })]);
    c.profiles[0].macros.push(Macro {
        id: "mac".into(),
        name: "Two keys".into(),
        steps: vec![Step::Keys { keys: Chord::new("F21") }, Step::Delay { ms: 200 }, Step::Keys { keys: Chord::new("F22") }],
    });
    let (engine, _rx) = start_config(&exe, "macro", &c);
    tap(VK_F20);
    settle(700);
    let seen = observer.seen();
    let a = seen.iter().find(|&&(vk, up, _)| vk == VK_F21 as u32 && !up).expect("F21");
    let b = seen.iter().find(|&&(vk, up, _)| vk == VK_F22 as u32 && !up).expect("F22");
    assert!(b.2.wrapping_sub(a.2) >= 150, "delay not honoured: {seen:?}");
    engine.stop(Duration::from_secs(3));
}

#[test]
fn profile_shortcut_and_command_switch_mappings() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    let observer = Observer::start();
    let mut c = config_with(vec![remap("a", "F20", "F21")]);
    c.profiles[1].mappings = vec![remap("b", "F20", "F22")];
    c.profiles[1].switch_key = Some(chord("F19", true, false, true, false));
    let (mut engine, rx) = start_config(&exe, "profiles", &c);

    send(&[key(VK_LCONTROL, false), key(VK_LMENU, false), key(VK_F19, false), key(VK_F19, true), key(VK_LMENU, true), key(VK_LCONTROL, true)]);
    wait_for(&rx, Duration::from_secs(3), |e| *e == EngineEvent::Protocol(Event::Profile { id: "coding".into() }));
    observer.clear();
    tap(VK_F20);
    settle(300);
    assert!(observer.saw(VK_F22, false) && !observer.saw_any(VK_F21), "coding profile: {:?}", observer.seen());

    engine.send(Command::Profile("productivity".into())).unwrap();
    wait_for(&rx, Duration::from_secs(3), |e| *e == EngineEvent::Protocol(Event::Profile { id: "productivity".into() }));
    observer.clear();
    tap(VK_F20);
    settle(300);
    assert!(observer.saw(VK_F21, false) && !observer.saw_any(VK_F22), "productivity profile: {:?}", observer.seen());
    engine.stop(Duration::from_secs(3));
    assert_no_stuck_modifiers();
}

#[test]
fn remap_is_released_when_profile_changes_while_held() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    let observer = Observer::start();
    let mut c = config_with(vec![remap("a", "F20", "F21")]);
    c.profiles[1].mappings = vec![remap("b", "F22", "F23")];
    let (mut engine, rx) = start_config(&exe, "held", &c);
    send(&[key(VK_F20, false)]);
    settle(150);
    engine.send(Command::Profile("coding".into())).unwrap();
    wait_for(&rx, Duration::from_secs(3), |e| matches!(e, EngineEvent::Protocol(Event::Profile { .. })));
    send(&[key(VK_F20, true)]);
    settle(300);
    assert!(observer.saw(VK_F21, true), "F21 left down: {:?}", observer.seen());
    engine.stop(Duration::from_secs(3));
}

#[test]
fn launch_action_starts_program() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    let dir = temp_dir("launch-out");
    let out = dir.join("launched.txt");
    let action = Action::Launch {
        path: "C:\\Windows\\System32\\cmd.exe".into(),
        args: format!("/c echo ok> \"{}\"", out.display()),
    };
    let (mut engine, rx) = start(&exe, "launch", vec![mapping("l", "F20", action)]);
    tap(VK_F20);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !out.exists() {
        if Instant::now() > deadline {
            let log: Vec<_> = rx.try_iter().collect();
            panic!("program was not launched; engine said {log:?}");
        }
        settle(100);
    }
    // The test command runs the same action without a key press.
    std::fs::remove_file(&out).unwrap();
    engine.send(Command::Test("productivity/l".into())).unwrap();
    wait_for(&rx, Duration::from_secs(3), |e| *e == EngineEvent::Protocol(Event::Fired { id: "productivity/l".into() }));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !out.exists() {
        assert!(Instant::now() < deadline, "test command did not launch");
        settle(100);
    }
    engine.stop(Duration::from_secs(3));
}

#[test]
fn detects_a_key() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    // F19 is mapped, but detection reports the physical key, not its mapping.
    let (mut engine, rx) = start(&exe, "detect", vec![remap("r", "F19", "F21")]);
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
    let (engine, rx) = start(&exe, "emergency", vec![remap("r", "F20", "F21")]);
    send(&[key(VK_LCONTROL, false), key(VK_LMENU, false), key(VK_LSHIFT, false), key(VK_BACK, false), key(VK_BACK, true)]);
    let exited = wait_for(&rx, Duration::from_secs(5), |e| matches!(e, EngineEvent::Exited { .. }));
    send(&[key(VK_LSHIFT, true), key(VK_LMENU, true), key(VK_LCONTROL, true)]);
    assert_eq!(exited, EngineEvent::Exited { code: Some(exit_code::EMERGENCY) });
    assert!(!engine.is_running());
    assert_no_stuck_modifiers();
}

#[test]
fn engine_exits_when_stdin_closes() {
    let _g = serial();
    let Some(exe) = ahk_exe() else { return };
    let (mut engine, rx) = start(&exe, "stdin", vec![]);
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
    let (engine, _rx) = start(&exe, "helper", vec![remap("r", "F20", "F21")]);
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

// --- The app's own flow, through the controller -------------------------------

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
            settle(10);
        }
    }
}

#[test]
fn controller_detect_apply_enable_test_disable() {
    let _g = serial();
    if ahk_exe().is_none() {
        return;
    }
    let observer = Observer::start();
    let sink = Arc::new(Recorder::default());
    let controller = Controller::new(Options::new(temp_dir("controller")), sink.clone());
    assert!(!controller.status().enabled, "mappings start off");

    // 1. Detect the key, exactly like the Keyboard Studio does.
    controller.start_detect().unwrap();
    settle(200);
    tap(VK_F20);
    assert_eq!(sink.wait_for("key-detected")["id"], "F20");

    // 2. Add a remap and apply it.
    let outcome = controller.apply(config_with(vec![remap("r", "F20", "F21")])).unwrap();
    assert!(outcome.applied, "{:?}", outcome.report);

    // 3. Enable and press the key.
    controller.set_enabled(true).unwrap();
    observer.clear();
    tap(VK_F20);
    settle(400);
    assert!(observer.saw(VK_F21, false) && !observer.saw_any(VK_F20), "remap through controller: {:?}", observer.seen());

    // 4. The built-in test injects the trigger and confirms it fired.
    let result = controller.test_mapping("productivity", "r").unwrap();
    assert!(result.contains("reached the engine"), "{result}");

    // 5. Disabling restores the key.
    controller.set_enabled(false).unwrap();
    observer.clear();
    tap(VK_F20);
    settle(300);
    assert!(observer.saw(VK_F20, false) && !observer.saw_any(VK_F21), "after disable: {:?}", observer.seen());

    // 6. Restart: the mapping persisted and mappings start off again.
    controller.shutdown();
    drop(controller);
    let reopened = Controller::new(Options::new(std::env::temp_dir().join("keyforge-e2e").join("controller")), Arc::new(Recorder::default()));
    let snapshot = reopened.snapshot();
    assert_eq!(snapshot.config.profiles[0].mappings.len(), 1);
    assert!(!snapshot.status.enabled);
    reopened.shutdown();
}
