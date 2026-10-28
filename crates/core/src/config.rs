//! Configuration model, serialised as JSON. The schema is documented in
//! `docs/CONFIG.md`; field names here are the JSON names in camelCase.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
}

impl Modifiers {
    pub fn is_empty(&self) -> bool {
        !(self.ctrl || self.shift || self.alt || self.win)
    }

    /// AutoHotkey modifier symbols, e.g. `^+` for Ctrl+Shift.
    pub fn ahk_prefix(&self) -> String {
        [(self.ctrl, '^'), (self.shift, '+'), (self.alt, '!'), (self.win, '#')]
            .into_iter()
            .filter_map(|(on, c)| on.then_some(c))
            .collect()
    }
}

/// A key plus the modifiers held with it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chord {
    pub key: String,
    #[serde(default, flatten)]
    pub mods: Modifiers,
}

pub type Trigger = Chord;

impl Chord {
    pub fn new(key: &str) -> Self {
        Chord { key: key.into(), mods: Modifiers::default() }
    }

    pub fn label(&self) -> String {
        let m = &self.mods;
        let mut parts: Vec<&str> = [(m.ctrl, "Ctrl"), (m.shift, "Shift"), (m.alt, "Alt"), (m.win, "Win")]
            .into_iter()
            .filter_map(|(on, s)| on.then_some(s))
            .collect();
        parts.push(&self.key);
        parts.join("+")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaAction {
    PlayPause,
    Next,
    Previous,
    Stop,
    VolumeUp,
    VolumeDown,
    Mute,
}

impl MediaAction {
    pub fn key(self) -> &'static str {
        match self {
            MediaAction::PlayPause => "Media_Play_Pause",
            MediaAction::Next => "Media_Next",
            MediaAction::Previous => "Media_Prev",
            MediaAction::Stop => "Media_Stop",
            MediaAction::VolumeUp => "Volume_Up",
            MediaAction::VolumeDown => "Volume_Down",
            MediaAction::Mute => "Volume_Mute",
        }
    }
}

/// Fixed list of Windows settings pages and utilities. Each maps to a
/// constant target, so nothing user-supplied reaches the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SystemAction {
    SettingsHome,
    Display,
    Sound,
    Bluetooth,
    Network,
    Wifi,
    Personalization,
    Apps,
    Power,
    WindowsUpdate,
    Notifications,
    Privacy,
    Mouse,
    TaskManager,
    Calculator,
    SnippingTool,
    FileExplorer,
    Notepad,
    ControlPanel,
    LockScreen,
    ShowDesktop,
    EmojiPanel,
}

pub enum SystemTarget {
    /// Open with the shell (`Run`).
    Open(&'static str),
    /// Send a Windows shortcut.
    Send(&'static str),
    Lock,
}

impl SystemAction {
    pub const ALL: [SystemAction; 22] = [
        SystemAction::SettingsHome,
        SystemAction::Display,
        SystemAction::Sound,
        SystemAction::Bluetooth,
        SystemAction::Network,
        SystemAction::Wifi,
        SystemAction::Personalization,
        SystemAction::Apps,
        SystemAction::Power,
        SystemAction::WindowsUpdate,
        SystemAction::Notifications,
        SystemAction::Privacy,
        SystemAction::Mouse,
        SystemAction::TaskManager,
        SystemAction::Calculator,
        SystemAction::SnippingTool,
        SystemAction::FileExplorer,
        SystemAction::Notepad,
        SystemAction::ControlPanel,
        SystemAction::LockScreen,
        SystemAction::ShowDesktop,
        SystemAction::EmojiPanel,
    ];

    pub fn target(self) -> SystemTarget {
        use SystemAction::*;
        use SystemTarget::*;
        match self {
            SettingsHome => Open("ms-settings:"),
            Display => Open("ms-settings:display"),
            Sound => Open("ms-settings:sound"),
            Bluetooth => Open("ms-settings:bluetooth"),
            Network => Open("ms-settings:network-status"),
            Wifi => Open("ms-settings:network-wifi"),
            Personalization => Open("ms-settings:personalization"),
            Apps => Open("ms-settings:appsfeatures"),
            Power => Open("ms-settings:powersleep"),
            WindowsUpdate => Open("ms-settings:windowsupdate"),
            Notifications => Open("ms-settings:notifications"),
            Privacy => Open("ms-settings:privacy"),
            Mouse => Open("ms-settings:mousetouchpad"),
            TaskManager => Open("taskmgr.exe"),
            Calculator => Open("calc.exe"),
            SnippingTool => Open("ms-screenclip:"),
            FileExplorer => Open("explorer.exe"),
            Notepad => Open("notepad.exe"),
            ControlPanel => Open("control.exe"),
            LockScreen => Lock,
            ShowDesktop => Send("#d"),
            EmojiPanel => Send("#."),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Action {
    /// Behave as a different key (down, up and auto-repeat).
    Remap { to: String },
    /// Send a key combination such as Ctrl+Shift+S.
    Shortcut { keys: Chord },
    /// Start a program, document or shortcut file.
    Launch {
        path: String,
        #[serde(default)]
        args: String,
    },
    /// Open an http(s) URL in the default browser.
    Url { url: String },
    /// Type a saved snippet.
    Text { text: String },
    /// Run a macro defined in the same profile.
    Macro {
        #[serde(rename = "macro")]
        macro_id: String,
    },
    Media { action: MediaAction },
    System { action: SystemAction },
    /// Run a confirmed script. `sha256` pins the exact file that was
    /// confirmed; KeyForge runs its private copy of that file.
    Script {
        path: String,
        sha256: String,
        #[serde(default = "yes")]
        confirm_each_run: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Step {
    Keys { keys: Chord },
    Text { text: String },
    Delay { ms: u32 },
    Launch {
        path: String,
        #[serde(default)]
        args: String,
    },
    Url { url: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Macro {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mapping {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    pub trigger: Trigger,
    /// Runs on a normal press, or on a tap when `hold`/`doubleTap` is set.
    pub action: Action,
    /// Runs when the key is held longer than the hold threshold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold: Option<Action>,
    /// Runs when the key is tapped twice within the double-tap window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double_tap: Option<Action>,
    /// Per-mapping hold threshold; defaults to `settings.tapHoldMs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_ms: Option<u32>,
    /// Process name (e.g. `code.exe`). When set, the mapping only applies
    /// while that app is in the foreground and overrides global mappings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// Last edit time (Unix milliseconds), for "recently edited".
    #[serde(default)]
    pub edited: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppMode {
    /// The app has its own overrides; global mappings still apply otherwise.
    Override,
    /// Global mappings of this profile are off while the app is focused.
    Disable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppRule {
    pub id: String,
    pub process: String,
    #[serde(default)]
    pub name: String,
    pub mode: AppMode,
    #[serde(default = "yes")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// Global shortcut that activates this profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch_key: Option<Chord>,
    /// Processes that activate this profile automatically while focused.
    #[serde(default)]
    pub auto_apps: Vec<String>,
    #[serde(default)]
    pub mappings: Vec<Mapping>,
    #[serde(default)]
    pub macros: Vec<Macro>,
    #[serde(default)]
    pub app_rules: Vec<AppRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub tap_hold_ms: u32,
    pub double_tap_ms: u32,
    pub close_to_tray: bool,
    /// Opt-in: turn mappings on when KeyForge starts. Off by default.
    pub enable_on_launch: bool,
    /// Custom AutoHotkey v2 executable; the bundled copy is used otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ahk_path: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { tap_hold_ms: 200, double_tap_ms: 250, close_to_tray: true, enable_on_launch: false, ahk_path: None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub version: u32,
    pub active_profile: String,
    #[serde(default)]
    pub settings: Settings,
    pub profiles: Vec<Profile>,
}

fn yes() -> bool {
    true
}

impl Default for Config {
    /// First-run configuration: the four standard profiles, all empty, so
    /// nothing is intercepted until the user adds mappings.
    fn default() -> Self {
        let profile = |id: &str, name: &str| Profile {
            id: id.into(),
            name: name.into(),
            switch_key: None,
            auto_apps: Vec::new(),
            mappings: Vec::new(),
            macros: Vec::new(),
            app_rules: Vec::new(),
        };
        Config {
            version: SCHEMA_VERSION,
            active_profile: "productivity".into(),
            settings: Settings::default(),
            profiles: vec![
                profile("productivity", "Productivity"),
                profile("coding", "Coding"),
                profile("study", "Study"),
                profile("gaming", "Gaming"),
            ],
        }
    }
}

impl Config {
    pub fn profile(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip_uses_documented_names() {
        let mut config = Config::default();
        config.profiles[0].mappings.push(Mapping {
            id: "m1".into(),
            name: String::new(),
            enabled: true,
            trigger: Chord { key: "F13".into(), mods: Modifiers { ctrl: true, ..Default::default() } },
            action: Action::Script { path: "C:\\a.ps1".into(), sha256: "00".into(), confirm_each_run: true },
            hold: Some(Action::Macro { macro_id: "mac".into() }),
            double_tap: None,
            hold_ms: Some(300),
            app: Some("code.exe".into()),
            edited: 1,
        });
        let json = serde_json::to_string(&config).unwrap();
        for needle in [
            r#""activeProfile":"productivity""#,
            r#""trigger":{"key":"F13","ctrl":true,"shift":false,"alt":false,"win":false}"#,
            r#""type":"script""#,
            r#""confirmEachRun":true"#,
            r#""hold":{"type":"macro","macro":"mac"}"#,
            r#""holdMs":300"#,
            r#""tapHoldMs":200"#,
        ] {
            assert!(json.contains(needle), "{needle} missing in {json}");
        }
        assert!(!json.contains("\"doubleTap\""), "None fields are omitted: {json}");
        assert_eq!(serde_json::from_str::<Config>(&json).unwrap(), config);
    }

    #[test]
    fn minimal_json_fills_defaults() {
        let json = r#"{"version":1,"activeProfile":"p","profiles":[{"id":"p","name":"P","mappings":[
            {"id":"a","trigger":{"key":"F13"},"action":{"type":"media","action":"playPause"}}]}]}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        let m = &config.profiles[0].mappings[0];
        assert!(m.enabled);
        assert!(m.trigger.mods.is_empty());
        assert_eq!(config.settings, Settings::default());
    }

    #[test]
    fn labels_and_prefixes() {
        let c = Chord { key: "S".into(), mods: Modifiers { ctrl: true, shift: true, alt: false, win: true } };
        assert_eq!(c.label(), "Ctrl+Shift+Win+S");
        assert_eq!(c.mods.ahk_prefix(), "^+#");
    }
}
