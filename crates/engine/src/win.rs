//! Windows-only safety mechanisms around the engine process.

use std::ffi::c_void;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::process::Child;
use std::sync::{mpsc, OnceLock};
use std::thread;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, RegisterHotKey, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, VK_BACK, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN,
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
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
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
