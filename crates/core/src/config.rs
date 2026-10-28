//! Configuration model. Serialised as JSON; see `docs/CONFIG.md`.
//!
//! Stage 1 only defines key remapping. Later stages add more `Action`
//! variants, profiles and application rules without changing existing fields.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modifiers {
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub win: bool,
}

impl Modifiers {
    pub fn is_empty(&self) -> bool {
        !(self.ctrl || self.shift || self.alt || self.win)
    }

    /// AutoHotkey hotkey prefix, e.g. `^+` for Ctrl+Shift.
    pub fn ahk_prefix(&self) -> String {
        let mut s = String::new();
        if self.ctrl {
            s.push('^');
        }
        if self.shift {
            s.push('+');
        }
        if self.alt {
            s.push('!');
        }
        if self.win {
            s.push('#');
        }
        s
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trigger {
    pub key: String,
    #[serde(default, flatten)]
    pub mods: Modifiers,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    /// Behave as a different key, including key-down/key-up and auto-repeat.
    Remap { to: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mapping {
    pub id: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    pub trigger: Trigger,
    pub action: Action,
}

fn yes() -> bool {
    true
}

impl Trigger {
    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if self.mods.ctrl {
            parts.push("Ctrl");
        }
        if self.mods.shift {
            parts.push("Shift");
        }
        if self.mods.alt {
            parts.push("Alt");
        }
        if self.mods.win {
            parts.push("Win");
        }
        parts.push(&self.key);
        parts.join("+")
    }
}
