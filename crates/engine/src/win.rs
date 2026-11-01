//! Windows-only safety mechanisms around the engine process.

use std::ffi::c_void;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::process::Child;
use std::sync::{mpsc, OnceLock};
use std::thread;
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HANDLE, WIN32_ERROR};
use windows_sys::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_BINARY, RRF_RT_REG_SZ,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use keyforge_core::config::Modifiers;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, RegisterHotKey, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, VK_BACK, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN,
    VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};

/// Process-wide job object with KILL_ON_JOB_CLOSE. Its handle is never
/// closed explicitly: when KeyForge exits for any reason, including a crash,
/// Windows closes it and terminates every engine assigned to it. Windows also
/// removes a process's keyboard hook when the process ends.
fn kill_job() -> io::Result<HANDLE> {
    static JOB: OnceLock<usize> = OnceLock::new();
    if let Some(&job) = JOB.get() {
        return Ok(job as HANDLE);
    }
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if ok == 0 {
            let err = io::Error::last_os_error();
            CloseHandle(job);
            return Err(err);
        }
        if JOB.set(job as usize).is_err() {
            CloseHandle(job);
        }
        Ok(*JOB.get().unwrap() as HANDLE)
    }
}

pub fn assign_to_kill_job(child: &Child) -> io::Result<()> {
    let job = kill_job()?;
    if unsafe { AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn key_input(vk: u16, flags: u32) -> INPUT {
    key_input_sc(vk, 0, flags)
}

fn key_input_sc(vk: u16, sc: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: sc, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

/// Inject one press and release of a key with modifiers, as the mapping
/// test does. Both the virtual-key and scan code are set, so hotkeys that
/// match either see the key.
pub fn send_chord(vk: u16, sc: u16, mods: &Modifiers) {
    let held: Vec<u16> = [(mods.ctrl, VK_LCONTROL), (mods.shift, VK_LSHIFT), (mods.alt, VK_LMENU), (mods.win, VK_LWIN)]
        .into_iter()
        .filter_map(|(on, vk)| on.then_some(vk))
        .collect();
    let mut flags = if sc & 0x100 != 0 { KEYEVENTF_EXTENDEDKEY } else { 0 };
    if vk == 0 {
        flags |= KEYEVENTF_SCANCODE;
    }
    let mut inputs: Vec<INPUT> = held.iter().map(|&m| key_input(m, if m == VK_LWIN { KEYEVENTF_EXTENDEDKEY } else { 0 })).collect();
    inputs.push(key_input_sc(vk, sc & 0xFF, flags));
    inputs.push(key_input_sc(vk, sc & 0xFF, flags | KEYEVENTF_KEYUP));
    for &m in held.iter().rev() {
        inputs.push(key_input(m, KEYEVENTF_KEYUP | if m == VK_LWIN { KEYEVENTF_EXTENDEDKEY } else { 0 }));
    }
    unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
}

/// Send key-up for every modifier Windows still considers down. Used after
/// the engine ended abnormally (killed or crashed) and could not clean up.
/// Returns the number of modifiers released.
pub fn release_stuck_modifiers() -> usize {
    const MODS: [u16; 8] = [VK_LCONTROL, VK_RCONTROL, VK_LSHIFT, VK_RSHIFT, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN];
    let down: Vec<u16> = MODS.into_iter().filter(|&vk| unsafe { GetAsyncKeyState(vk as i32) } < 0).collect();
    if down.is_empty() {
        return 0;
    }
    let mut inputs = Vec::with_capacity(down.len() + 2);
    if down.iter().any(|&vk| matches!(vk, VK_LWIN | VK_RWIN | VK_LMENU | VK_RMENU)) {
        // An unassigned key (0xE8) between Win/Alt down and up stops the
        // release from opening the Start menu or activating a menu bar.
        inputs.push(key_input(0xE8, 0));
        inputs.push(key_input(0xE8, KEYEVENTF_KEYUP));
    }
    for &vk in &down {
        let extended = if matches!(vk, VK_RCONTROL | VK_RMENU | VK_LWIN | VK_RWIN) { KEYEVENTF_EXTENDEDKEY } else { 0 };
        inputs.push(key_input(vk, KEYEVENTF_KEYUP | extended));
    }
    unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    down.len()
}

/// Register Ctrl+Alt+Shift+Backspace with `RegisterHotKey` on a dedicated
/// thread. The engine handles this shortcut itself while it runs. This
/// registration is the fallback: it fires when the engine is not running or
/// has stopped responding (Windows bypasses an unresponsive keyboard hook).
pub fn listen_emergency_hotkey(on_press: impl Fn() + Send + 'static) -> io::Result<()> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new().name("kf-emergency-hotkey".into()).spawn(move || unsafe {
        let ok = RegisterHotKey(std::ptr::null_mut(), 1, MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_NOREPEAT, VK_BACK as u32);
        let registered = ok != 0;
        let _ = tx.send(if registered { Ok(()) } else { Err(io::Error::last_os_error()) });
        if !registered {
            return;
        }
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if msg.message == WM_HOTKEY {
                on_press();
            }
        }
    })?;
    rx.recv().unwrap_or_else(|_| Err(io::Error::other("emergency hotkey thread ended")))
}

// --- Start with Windows ------------------------------------------------------
// The per-user Run key needs no administrator rights. Task Manager's Startup
// tab can disable an entry without removing it; that state lives under
// StartupApproved, where an odd first byte means "disabled".

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn reg_result(err: WIN32_ERROR) -> io::Result<()> {
    if err == ERROR_SUCCESS || err == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(err as i32))
    }
}

/// Register (`Some(command)`) or remove (`None`) a program that starts when
/// the current user signs in. Registering also clears a Task Manager
/// "disabled" flag.
pub fn set_run_at_login(name: &str, command: Option<&str>) -> io::Result<()> {
    let (run, approved, value) = (wide(RUN_KEY), wide(APPROVED_KEY), wide(name));
    unsafe {
        if let Some(command) = command {
            let data = wide(command);
            let err = RegSetKeyValueW(HKEY_CURRENT_USER, run.as_ptr(), value.as_ptr(), REG_SZ, data.as_ptr() as *const c_void, (data.len() * 2) as u32);
            if err != ERROR_SUCCESS {
                return Err(io::Error::from_raw_os_error(err as i32));
            }
        } else {
            reg_result(RegDeleteKeyValueW(HKEY_CURRENT_USER, run.as_ptr(), value.as_ptr()))?;
        }
        reg_result(RegDeleteKeyValueW(HKEY_CURRENT_USER, approved.as_ptr(), value.as_ptr()))
    }
}

/// The registered start-up command for `name`, or `None` if it is not
/// registered or was disabled in Task Manager.
pub fn run_at_login(name: &str) -> Option<String> {
    let (run, approved, value) = (wide(RUN_KEY), wide(APPROVED_KEY), wide(name));
    unsafe {
        let mut len = 0u32;
        let no_type = std::ptr::null_mut();
        if RegGetValueW(HKEY_CURRENT_USER, run.as_ptr(), value.as_ptr(), RRF_RT_REG_SZ, no_type, std::ptr::null_mut(), &mut len) != ERROR_SUCCESS {
            return None;
        }
        let mut buf = vec![0u16; (len as usize).div_ceil(2)];
        if RegGetValueW(HKEY_CURRENT_USER, run.as_ptr(), value.as_ptr(), RRF_RT_REG_SZ, no_type, buf.as_mut_ptr() as *mut c_void, &mut len) != ERROR_SUCCESS {
            return None;
        }
        let command = String::from_utf16_lossy(&buf).trim_end_matches('\0').to_string();
        let mut flags = [0u8; 32];
        let mut flags_len = flags.len() as u32;
        let found = RegGetValueW(HKEY_CURRENT_USER, approved.as_ptr(), value.as_ptr(), RRF_RT_REG_BINARY, no_type, flags.as_mut_ptr() as *mut c_void, &mut flags_len);
        if found == ERROR_SUCCESS && flags_len > 0 && flags[0] & 1 == 1 {
            return None;
        }
        Some(command)
    }
}
