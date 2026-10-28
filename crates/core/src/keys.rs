//! Table of keys KeyForge knows by name, plus raw `vkXXscYYY` identifiers for
//! anything else the hardware reports.
//!
//! `ahk` is the token used in generated AutoHotkey code. Punctuation uses scan
//! codes (`SC027`) so hotkeys refer to the physical key and never need escaping.
//! Scan codes follow AutoHotkey's convention: extended keys have 0x100 set.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyGroup {
    Function,
    Navigation,
    System,
    Modifier,
    Alphanumeric,
    Punctuation,
    Numpad,
    Media,
    Browser,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyDef {
    /// Stable identifier stored in configuration files.
    pub id: &'static str,
    pub label: &'static str,
    #[serde(skip)]
    pub ahk: &'static str,
    pub vk: u16,
    pub sc: u16,
    pub group: KeyGroup,
}

const fn k(id: &'static str, label: &'static str, ahk: &'static str, vk: u16, sc: u16, group: KeyGroup) -> KeyDef {
    KeyDef { id, label, ahk, vk, sc, group }
}

use KeyGroup::*;

pub static KEYS: &[KeyDef] = &[
    k("F1", "F1", "F1", 0x70, 0x03B, Function),
    k("F2", "F2", "F2", 0x71, 0x03C, Function),
    k("F3", "F3", "F3", 0x72, 0x03D, Function),
    k("F4", "F4", "F4", 0x73, 0x03E, Function),
    k("F5", "F5", "F5", 0x74, 0x03F, Function),
    k("F6", "F6", "F6", 0x75, 0x040, Function),
    k("F7", "F7", "F7", 0x76, 0x041, Function),
    k("F8", "F8", "F8", 0x77, 0x042, Function),
    k("F9", "F9", "F9", 0x78, 0x043, Function),
    k("F10", "F10", "F10", 0x79, 0x044, Function),
    k("F11", "F11", "F11", 0x7A, 0x057, Function),
    k("F12", "F12", "F12", 0x7B, 0x058, Function),
    k("F13", "F13", "F13", 0x7C, 0x064, Function),
    k("F14", "F14", "F14", 0x7D, 0x065, Function),
    k("F15", "F15", "F15", 0x7E, 0x066, Function),
    k("F16", "F16", "F16", 0x7F, 0x067, Function),
    k("F17", "F17", "F17", 0x80, 0x068, Function),
    k("F18", "F18", "F18", 0x81, 0x069, Function),
    k("F19", "F19", "F19", 0x82, 0x06A, Function),
    k("F20", "F20", "F20", 0x83, 0x06B, Function),
    k("F21", "F21", "F21", 0x84, 0x06C, Function),
    k("F22", "F22", "F22", 0x85, 0x06D, Function),
    k("F23", "F23", "F23", 0x86, 0x06E, Function),
    k("F24", "F24", "F24", 0x87, 0x076, Function),
    k("Escape", "Esc", "Escape", 0x1B, 0x001, System),
    k("Tab", "Tab", "Tab", 0x09, 0x00F, System),
    k("CapsLock", "Caps Lock", "CapsLock", 0x14, 0x03A, System),
    k("Space", "Space", "Space", 0x20, 0x039, System),
    k("Enter", "Enter", "Enter", 0x0D, 0x01C, System),
    k("Backspace", "Backspace", "Backspace", 0x08, 0x00E, System),
    k("PrintScreen", "Print Screen", "PrintScreen", 0x2C, 0x137, System),
    k("ScrollLock", "Scroll Lock", "ScrollLock", 0x91, 0x046, System),
    k("Pause", "Pause", "Pause", 0x13, 0x045, System),
    k("AppsKey", "Menu", "AppsKey", 0x5D, 0x15D, System),
    k("Insert", "Insert", "Insert", 0x2D, 0x152, Navigation),
    k("Delete", "Delete", "Delete", 0x2E, 0x153, Navigation),
    k("Home", "Home", "Home", 0x24, 0x147, Navigation),
    k("End", "End", "End", 0x23, 0x14F, Navigation),
    k("PgUp", "Page Up", "PgUp", 0x21, 0x149, Navigation),
    k("PgDn", "Page Down", "PgDn", 0x22, 0x151, Navigation),
    k("Up", "Up", "Up", 0x26, 0x148, Navigation),
    k("Down", "Down", "Down", 0x28, 0x150, Navigation),
    k("Left", "Left", "Left", 0x25, 0x14B, Navigation),
    k("Right", "Right", "Right", 0x27, 0x14D, Navigation),
    k("LCtrl", "Left Ctrl", "LCtrl", 0xA2, 0x01D, Modifier),
    k("RCtrl", "Right Ctrl", "RCtrl", 0xA3, 0x11D, Modifier),
    k("LShift", "Left Shift", "LShift", 0xA0, 0x02A, Modifier),
    k("RShift", "Right Shift", "RShift", 0xA1, 0x036, Modifier),
    k("LAlt", "Left Alt", "LAlt", 0xA4, 0x038, Modifier),
    k("RAlt", "Right Alt", "RAlt", 0xA5, 0x138, Modifier),
    k("LWin", "Left Win", "LWin", 0x5B, 0x15B, Modifier),
    k("RWin", "Right Win", "RWin", 0x5C, 0x15C, Modifier),
    k("A", "A", "a", 0x41, 0x01E, Alphanumeric),
    k("B", "B", "b", 0x42, 0x030, Alphanumeric),
    k("C", "C", "c", 0x43, 0x02E, Alphanumeric),
    k("D", "D", "d", 0x44, 0x020, Alphanumeric),
    k("E", "E", "e", 0x45, 0x012, Alphanumeric),
    k("F", "F", "f", 0x46, 0x021, Alphanumeric),
    k("G", "G", "g", 0x47, 0x022, Alphanumeric),
    k("H", "H", "h", 0x48, 0x023, Alphanumeric),
    k("I", "I", "i", 0x49, 0x017, Alphanumeric),
    k("J", "J", "j", 0x4A, 0x024, Alphanumeric),
    k("K", "K", "k", 0x4B, 0x025, Alphanumeric),
    k("L", "L", "l", 0x4C, 0x026, Alphanumeric),
    k("M", "M", "m", 0x4D, 0x032, Alphanumeric),
    k("N", "N", "n", 0x4E, 0x031, Alphanumeric),
    k("O", "O", "o", 0x4F, 0x018, Alphanumeric),
    k("P", "P", "p", 0x50, 0x019, Alphanumeric),
    k("Q", "Q", "q", 0x51, 0x010, Alphanumeric),
    k("R", "R", "r", 0x52, 0x013, Alphanumeric),
    k("S", "S", "s", 0x53, 0x01F, Alphanumeric),
    k("T", "T", "t", 0x54, 0x014, Alphanumeric),
    k("U", "U", "u", 0x55, 0x016, Alphanumeric),
    k("V", "V", "v", 0x56, 0x02F, Alphanumeric),
    k("W", "W", "w", 0x57, 0x011, Alphanumeric),
    k("X", "X", "x", 0x58, 0x02D, Alphanumeric),
    k("Y", "Y", "y", 0x59, 0x015, Alphanumeric),
    k("Z", "Z", "z", 0x5A, 0x02C, Alphanumeric),
    k("D0", "0", "0", 0x30, 0x00B, Alphanumeric),
    k("D1", "1", "1", 0x31, 0x002, Alphanumeric),
    k("D2", "2", "2", 0x32, 0x003, Alphanumeric),
    k("D3", "3", "3", 0x33, 0x004, Alphanumeric),
    k("D4", "4", "4", 0x34, 0x005, Alphanumeric),
    k("D5", "5", "5", 0x35, 0x006, Alphanumeric),
    k("D6", "6", "6", 0x36, 0x007, Alphanumeric),
    k("D7", "7", "7", 0x37, 0x008, Alphanumeric),
    k("D8", "8", "8", 0x38, 0x009, Alphanumeric),
    k("D9", "9", "9", 0x39, 0x00A, Alphanumeric),
    k("Minus", "-", "SC00C", 0xBD, 0x00C, Punctuation),
    k("Equal", "=", "SC00D", 0xBB, 0x00D, Punctuation),
    k("BracketLeft", "[", "SC01A", 0xDB, 0x01A, Punctuation),
    k("BracketRight", "]", "SC01B", 0xDD, 0x01B, Punctuation),
    k("Backslash", "\\", "SC02B", 0xDC, 0x02B, Punctuation),
    k("Semicolon", ";", "SC027", 0xBA, 0x027, Punctuation),
    k("Quote", "'", "SC028", 0xDE, 0x028, Punctuation),
    k("Backquote", "`", "SC029", 0xC0, 0x029, Punctuation),
    k("Comma", ",", "SC033", 0xBC, 0x033, Punctuation),
    k("Period", ".", "SC034", 0xBE, 0x034, Punctuation),
    k("Slash", "/", "SC035", 0xBF, 0x035, Punctuation),
    k("NumLock", "Num Lock", "NumLock", 0x90, 0x145, Numpad),
    k("Numpad0", "Num 0", "Numpad0", 0x60, 0x052, Numpad),
    k("Numpad1", "Num 1", "Numpad1", 0x61, 0x04F, Numpad),
    k("Numpad2", "Num 2", "Numpad2", 0x62, 0x050, Numpad),
    k("Numpad3", "Num 3", "Numpad3", 0x63, 0x051, Numpad),
    k("Numpad4", "Num 4", "Numpad4", 0x64, 0x04B, Numpad),
    k("Numpad5", "Num 5", "Numpad5", 0x65, 0x04C, Numpad),
    k("Numpad6", "Num 6", "Numpad6", 0x66, 0x04D, Numpad),
    k("Numpad7", "Num 7", "Numpad7", 0x67, 0x047, Numpad),
    k("Numpad8", "Num 8", "Numpad8", 0x68, 0x048, Numpad),
    k("Numpad9", "Num 9", "Numpad9", 0x69, 0x049, Numpad),
    k("NumpadDot", "Num .", "NumpadDot", 0x6E, 0x053, Numpad),
    k("NumpadDiv", "Num /", "NumpadDiv", 0x6F, 0x135, Numpad),
    k("NumpadMult", "Num *", "NumpadMult", 0x6A, 0x037, Numpad),
    k("NumpadSub", "Num -", "NumpadSub", 0x6D, 0x04A, Numpad),
    k("NumpadAdd", "Num +", "NumpadAdd", 0x6B, 0x04E, Numpad),
    k("NumpadEnter", "Num Enter", "NumpadEnter", 0x0D, 0x11C, Numpad),
    k("Volume_Mute", "Mute", "Volume_Mute", 0xAD, 0x120, Media),
    k("Volume_Down", "Volume Down", "Volume_Down", 0xAE, 0x12E, Media),
    k("Volume_Up", "Volume Up", "Volume_Up", 0xAF, 0x130, Media),
    k("Media_Play_Pause", "Play/Pause", "Media_Play_Pause", 0xB3, 0x122, Media),
    k("Media_Next", "Next Track", "Media_Next", 0xB0, 0x119, Media),
    k("Media_Prev", "Previous Track", "Media_Prev", 0xB1, 0x110, Media),
    k("Media_Stop", "Stop", "Media_Stop", 0xB2, 0x124, Media),
    k("Launch_Mail", "Mail", "Launch_Mail", 0xB4, 0x16C, Browser),
    k("Launch_App1", "My Computer", "Launch_App1", 0xB6, 0x16B, Browser),
    k("Launch_App2", "Calculator", "Launch_App2", 0xB7, 0x121, Browser),
    k("Browser_Back", "Browser Back", "Browser_Back", 0xA6, 0x16A, Browser),
    k("Browser_Forward", "Browser Forward", "Browser_Forward", 0xA7, 0x169, Browser),
    k("Browser_Refresh", "Browser Refresh", "Browser_Refresh", 0xA8, 0x167, Browser),
    k("Browser_Search", "Browser Search", "Browser_Search", 0xAA, 0x165, Browser),
    k("Browser_Home", "Browser Home", "Browser_Home", 0xAC, 0x132, Browser),
];

/// A key reference resolved from a configuration identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyRef {
    pub id: String,
    /// Token used in generated AutoHotkey code.
    pub ahk: String,
    pub vk: u16,
    pub sc: u16,
    pub modifier: bool,
}

pub fn find(id: &str) -> Option<&'static KeyDef> {
    KEYS.iter().find(|k| k.id.eq_ignore_ascii_case(id))
}

/// Resolve a configuration key id: a table id such as `F13`, or a raw
/// `vkXXscYYY` / `scYYY` identifier for keys that are not in the table.
/// `scYYY` is used for keys that report no usable virtual-key code.
pub fn resolve(id: &str) -> Option<KeyRef> {
    if let Some(def) = find(id) {
        return Some(KeyRef {
            id: def.id.to_string(),
            ahk: def.ahk.to_string(),
            vk: def.vk,
            sc: def.sc,
            modifier: def.group == Modifier,
        });
    }
    let (vk, sc) = parse_raw(id)?;
    // A raw id that matches a table key is normalised to the named key.
    if let Some(def) = KEYS.iter().find(|k| k.vk == vk && k.sc == sc) {
        return resolve(def.id);
    }
    Some(KeyRef {
        id: raw_id(vk, sc),
        ahk: raw_id(vk, sc),
        vk,
        sc,
        modifier: is_modifier_vk(vk),
    })
}

pub fn raw_id(vk: u16, sc: u16) -> String {
    if vk == 0 || vk == 0xFF {
        format!("SC{sc:03X}")
    } else {
        format!("vk{vk:02X}sc{sc:03X}")
    }
}

/// Parse `vkXXscYYY` or `scYYY` (hex, case-insensitive). Returns vk 0 for the
/// scan-code-only form. `vk` must be 1..=0xFE and `sc` at most 0x1FF.
pub fn parse_raw(id: &str) -> Option<(u16, u16)> {
    let hex = |s: &str| {
        (!s.is_empty() && s.len() <= 3 && s.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| u16::from_str_radix(s, 16).ok())
            .flatten()
    };
    let lower = id.to_ascii_lowercase();
    if let Some(sc) = lower.strip_prefix("sc") {
        let sc = hex(sc)?;
        return (1..=0x1FF).contains(&sc).then_some((0, sc));
    }
    let (vk, sc) = lower.strip_prefix("vk")?.split_once("sc")?;
    let (vk, sc) = (hex(vk)?, hex(sc)?);
    ((1..=0xFE).contains(&vk) && sc <= 0x1FF).then_some((vk, sc))
}

pub fn is_modifier_vk(vk: u16) -> bool {
    matches!(vk, 0x10 | 0x11 | 0x12 | 0xA0..=0xA5 | 0x5B | 0x5C)
}

/// Best-effort table lookup for a key the engine detected.
pub fn identify(vk: u16, sc: u16) -> String {
    KEYS.iter()
        .find(|k| k.vk == vk && k.sc == sc)
        .or_else(|| KEYS.iter().find(|k| k.sc == sc && sc != 0))
        .map(|k| k.id.to_string())
        .unwrap_or_else(|| raw_id(vk, sc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_and_codes_are_unique() {
        let mut ids = HashSet::new();
        let mut codes = HashSet::new();
        for key in KEYS {
            assert!(ids.insert(key.id.to_ascii_lowercase()), "duplicate id {}", key.id);
            assert!(codes.insert((key.vk, key.sc)), "duplicate vk/sc for {}", key.id);
        }
    }

    #[test]
    fn ahk_tokens_need_no_escaping() {
        for key in KEYS {
            assert!(key.ahk.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'), "{}", key.ahk);
        }
    }

    #[test]
    fn covers_f1_to_f24() {
        for n in 1..=24 {
            let key = find(&format!("F{n}")).unwrap();
            assert_eq!(key.vk, 0x6F + n);
        }
    }

    #[test]
    fn resolves_named_and_raw() {
        assert_eq!(resolve("printscreen").unwrap().ahk, "PrintScreen");
        assert!(resolve("RCtrl").unwrap().modifier);
        // Raw id matching a table entry normalises to the named key.
        assert_eq!(resolve("vk7Csc064").unwrap().id, "F13");
        assert!(resolve("VKFFsc0F1").is_none(), "vk FF is reserved");
        let sc_only = resolve("sc0f1").unwrap();
        assert_eq!((sc_only.vk, sc_only.ahk.as_str()), (0, "SC0F1"));
        let raw = resolve("vkE9sc0F1").unwrap();
        assert_eq!((raw.vk, raw.sc, raw.ahk.as_str()), (0xE9, 0xF1, "vkE9sc0F1"));
    }

    #[test]
    fn rejects_malformed_raw_ids() {
        for bad in ["", "vk", "vksc", "vk1sc", "vkGGsc001", "vk10sc2000", "vk00sc001", "sc000", "sc", "sc001vk10", "vk10sc001;Run"] {
            assert!(parse_raw(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn identifies_detected_keys() {
        assert_eq!(identify(0x2C, 0x137), "PrintScreen");
        assert_eq!(identify(0xFF, 0x0F1), "SC0F1");
        assert_eq!(identify(0xE9, 0x0F2), "vkE9sc0F2");
    }
}
