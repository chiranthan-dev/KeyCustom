//! Validation that runs before any configuration reaches the engine.

use crate::config::{Action, Mapping};
use crate::keys::{self, KeyRef};
use serde::Serialize;
use std::collections::HashMap;

pub const MAX_MAPPINGS: usize = 500;

/// Reserved emergency shortcut: Ctrl+Alt+Shift+Backspace. The engine handles
/// it before any user mapping and the backend registers it too as a fallback.
pub const EMERGENCY_LABEL: &str = "Ctrl+Alt+Shift+Backspace";
pub const EMERGENCY_KEY: &str = "Backspace";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    /// Mapping the issue refers to, if any.
    pub mapping: Option<String>,
    pub message: String,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Report {
    pub errors: Vec<Issue>,
    pub warnings: Vec<Issue>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }

    fn error(&mut self, mapping: &Mapping, message: impl Into<String>) {
        self.errors.push(Issue { mapping: Some(mapping.id.clone()), message: message.into() });
    }

    fn warn(&mut self, mapping: &Mapping, message: impl Into<String>) {
        self.warnings.push(Issue { mapping: Some(mapping.id.clone()), message: message.into() });
    }
}

/// A mapping whose keys have been resolved and checked.
#[derive(Debug, Clone)]
pub struct Resolved<'a> {
    pub mapping: &'a Mapping,
    pub key: KeyRef,
    pub target: KeyRef,
}

pub fn validate(mappings: &[Mapping]) -> (Report, Vec<Resolved<'_>>) {
    let mut report = Report::default();
    let mut resolved = Vec::new();

    if mappings.len() > MAX_MAPPINGS {
        report.errors.push(Issue { mapping: None, message: format!("At most {MAX_MAPPINGS} mappings are supported.") });
    }
    let mut ids = HashMap::new();
    for m in mappings {
        if m.id.trim().is_empty() || ids.insert(m.id.as_str(), ()).is_some() {
            report.error(m, "Mapping ids must be unique and non-empty.");
        }
    }

    let mut triggers: HashMap<(String, String), &str> = HashMap::new();
    for m in mappings.iter().filter(|m| m.enabled) {
        let Some(key) = keys::resolve(&m.trigger.key) else {
            report.error(m, format!("Unknown trigger key \"{}\".", m.trigger.key));
            continue;
        };
        let Action::Remap { to } = &m.action;
        let Some(target) = keys::resolve(to) else {
            report.error(m, format!("Unknown target key \"{to}\"."));
            continue;
        };
        if key.modifier && !m.trigger.mods.is_empty() {
            report.error(m, "A modifier key cannot be combined with other modifiers as a trigger.");
            continue;
        }
        if key.id == EMERGENCY_KEY {
            report.error(m, format!("Backspace is reserved for the emergency shortcut ({EMERGENCY_LABEL})."));
            continue;
        }
        if key.id == target.id && m.trigger.mods.is_empty() {
            report.error(m, "A key cannot be remapped to itself.");
            continue;
        }
        let slot = (key.id.clone(), m.trigger.mods.ahk_prefix());
        if let Some(other) = triggers.insert(slot, &m.id) {
            report.error(m, format!("{} is already used by mapping \"{other}\".", m.trigger.label()));
            continue;
        }
        resolved.push(Resolved { mapping: m, key, target });
    }

    // At least one key of each modifier pair must keep working, otherwise the
    // emergency shortcut can no longer be typed.
    for (left, right, name) in [("LCtrl", "RCtrl", "Ctrl"), ("LShift", "RShift", "Shift"), ("LAlt", "RAlt", "Alt")] {
        let remapped = |id: &str| resolved.iter().any(|r| r.key.id == id && r.mapping.trigger.mods.is_empty());
        if remapped(left) && remapped(right) {
            report.errors.push(Issue {
                mapping: None,
                message: format!("Left and right {name} cannot both be remapped; the emergency shortcut needs one of them."),
            });
        }
    }

    for r in &resolved {
        let chained = resolved.iter().any(|o| o.key.id == r.target.id && o.mapping.trigger.mods.is_empty());
        if chained {
            report.warn(
                r.mapping,
                format!("{} sends {}, which is also remapped. Mappings never trigger each other, so it sends the original {}.", r.mapping.trigger.label(), r.target.id, r.target.id),
            );
        }
    }

    if !report.ok() {
        resolved.clear();
    }
    (report, resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Modifiers, Trigger};

    fn remap(id: &str, from: &str, to: &str) -> Mapping {
        Mapping { id: id.into(), enabled: true, trigger: Trigger { key: from.into(), mods: Modifiers::default() }, action: Action::Remap { to: to.into() } }
    }

    #[test]
    fn accepts_simple_remap() {
        let maps = [remap("a", "F13", "Media_Play_Pause"), remap("b", "RCtrl", "AppsKey")];
        let (report, resolved) = validate(&maps);
        assert!(report.ok(), "{report:?}");
        assert!(report.warnings.is_empty());
        assert_eq!(resolved.len(), 2);
    }

    #[test]
    fn rejects_unknown_self_and_duplicate() {
        let maps = [remap("a", "Nope", "F1"), remap("b", "F1", "F1"), remap("c", "F2", "F3"), remap("d", "f2", "F4")];
        let (report, resolved) = validate(&maps);
        assert_eq!(report.errors.len(), 3, "{report:?}");
        assert!(resolved.is_empty(), "nothing is applied when anything is invalid");
    }

    #[test]
    fn modifier_variants_are_distinct_triggers() {
        let mut ctrl = remap("b", "F13", "F14");
        ctrl.trigger.mods.ctrl = true;
        let (report, _) = validate(&[remap("a", "F13", "F15"), ctrl]);
        assert!(report.ok(), "{report:?}");
    }

    #[test]
    fn protects_emergency_shortcut() {
        let (report, _) = validate(&[remap("a", "Backspace", "Delete")]);
        assert!(!report.ok());
        let (report, _) = validate(&[remap("a", "LCtrl", "F13"), remap("b", "RCtrl", "F14")]);
        assert!(!report.ok());
        let (report, _) = validate(&[remap("a", "LCtrl", "F13")]);
        assert!(report.ok());
    }

    #[test]
    fn disabled_mappings_are_ignored() {
        let mut off = remap("b", "F13", "F15");
        off.enabled = false;
        let maps = [remap("a", "F13", "F14"), off];
        let (report, resolved) = validate(&maps);
        assert!(report.ok());
        assert_eq!(resolved.len(), 1);
    }

    #[test]
    fn swaps_are_allowed_but_chains_warn() {
        let (report, _) = validate(&[remap("a", "F13", "F14"), remap("b", "F14", "F13")]);
        assert!(report.ok());
        assert_eq!(report.warnings.len(), 2);
    }
}
