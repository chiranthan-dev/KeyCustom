//! Events the engine writes to stdout (one JSON object per line) and the
//! commands the supervisor writes to its stdin.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum Event {
    Ready { ahk: String, admin: bool, pid: u32 },
    Pong,
    DetectStarted,
    DetectEnded { reason: String },
    Key(DetectedKey),
    Error { message: String },
    Exit { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedKey {
    /// Name reported by AutoHotkey (layout dependent, for display only).
    pub name: String,
    pub vk: u16,
    pub sc: u16,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub win: bool,
    /// KeyForge key id to use in a mapping. Filled in by the supervisor.
    #[serde(default)]
    pub id: String,
}

pub fn parse_event(line: &str) -> Result<Event, String> {
    let mut event: Event = serde_json::from_str(line.trim()).map_err(|e| format!("{e}: {line}"))?;
    if let Event::Key(key) = &mut event {
        key.id = crate::keys::identify(key.vk, key.sc);
    }
    Ok(event)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Ping,
    Detect,
    CancelDetect,
    Exit,
}

impl Command {
    pub fn line(self) -> &'static str {
        match self {
            Command::Ping => "ping\n",
            Command::Detect => "detect\n",
            Command::CancelDetect => "cancel-detect\n",
            Command::Exit => "exit\n",
        }
    }
}

/// Exit codes used by the runtime script.
pub mod exit_code {
    pub const REQUESTED: i32 = 0;
    pub const SUPERVISOR_LOST: i32 = 2;
    pub const EMERGENCY: i32 = 3;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_events() {
        assert_eq!(
            parse_event(r#"{"event":"ready","ahk":"2.0.18","admin":false,"pid":42}"#).unwrap(),
            Event::Ready { ahk: "2.0.18".into(), admin: false, pid: 42 }
        );
        assert_eq!(parse_event(r#"{"event":"pong"}"#).unwrap(), Event::Pong);
        assert_eq!(parse_event(r#"{"event":"exit","reason":"emergency"}"#).unwrap(), Event::Exit { reason: "emergency".into() });
        let Event::Key(key) = parse_event(r#"{"event":"key","name":"F13","vk":124,"sc":100,"ctrl":true,"shift":false,"alt":false,"win":false}"#).unwrap() else {
            panic!()
        };
        assert_eq!((key.id.as_str(), key.ctrl), ("F13", true));
    }

    #[test]
    fn escaped_names_round_trip() {
        let Event::Key(key) = parse_event(r#"{"event":"key","name":"\\","vk":220,"sc":43}"#).unwrap() else { panic!() };
        assert_eq!((key.name.as_str(), key.id.as_str()), ("\\", "Backslash"));
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_event("Error at line 3").is_err());
        assert!(parse_event(r#"{"event":"surprise"}"#).is_err());
    }
}
