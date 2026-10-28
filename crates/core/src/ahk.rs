//! Generates the single AutoHotkey script the engine runs.

use crate::validate::Resolved;
use crate::config::Action;

pub const RUNTIME: &str = include_str!("runtime.ahk");

/// Build the complete engine script. Only validated mappings are accepted, so
/// every token written here comes from the key table or a checked raw id.
pub fn generate(mappings: &[Resolved<'_>]) -> String {
    let mut out = String::with_capacity(RUNTIME.len() + mappings.len() * 48);
    out.push_str(RUNTIME);
    for r in mappings {
        let hotkey = format!("{}{}", r.mapping.trigger.mods.ahk_prefix(), r.key.ahk);
        match &r.mapping.action {
            // AutoHotkey's remap syntax handles key-down, key-up and
            // auto-repeat, and the engine's own output never triggers hotkeys.
            Action::Remap { .. } => {
                out.push_str(&format!("{hotkey}::{} ; {}\n", r.target.ahk, sanitize_comment(&r.mapping.id)));
            }
        }
    }
    out
}

fn sanitize_comment(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(80).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Mapping, Modifiers, Trigger};
    use crate::validate::validate;

    fn remap(id: &str, from: &str, to: &str, ctrl: bool) -> Mapping {
        Mapping {
            id: id.into(),
            enabled: true,
            trigger: Trigger { key: from.into(), mods: Modifiers { ctrl, ..Default::default() } },
            action: Action::Remap { to: to.into() },
        }
    }

    #[test]
    fn generates_remaps_after_runtime() {
        let maps = [remap("play", "F13", "Media_Play_Pause", false), remap("semi", "Semicolon", "vkE9sc0F1", true)];
        let (report, resolved) = validate(&maps);
        assert!(report.ok());
        let script = generate(&resolved);
        assert!(script.starts_with(RUNTIME));
        let tail = &script[RUNTIME.len()..];
        assert_eq!(tail, "F13::Media_Play_Pause ; play\n^SC027::vkE9sc0F1 ; semi\n");
    }

    #[test]
    fn empty_config_is_runtime_only() {
        assert_eq!(generate(&[]), RUNTIME);
    }

    #[test]
    fn ids_cannot_break_out_of_comments() {
        let maps = [remap("x\nRun \"calc\"", "F13", "F14", false)];
        let (_, resolved) = validate(&maps);
        let script = generate(&resolved);
        assert!(!script[RUNTIME.len()..].contains("\nRun"));
    }

    #[test]
    fn runtime_keeps_safety_features() {
        for needle in ["#Requires AutoHotkey v2.0", "KeyHistory 0", "#SuspendExempt true\n^!+Backspace::", "PeekNamedPipe", "KF_ReleaseModifiers"] {
            assert!(RUNTIME.contains(needle), "{needle}");
        }
        // Emergency hotkey must precede the user mapping section.
        assert!(RUNTIME.find("^!+Backspace::").unwrap() < RUNTIME.find("; --- User mappings").unwrap());
        assert!(RUNTIME.ends_with('\n'));
    }
}
