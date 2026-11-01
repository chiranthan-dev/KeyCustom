//! Test fixtures shared by the core and engine test suites.

use crate::config::{Action, Chord, Config, Macro, Mapping, Modifiers, Step};
use serde::Deserialize;

/// The UI's template catalogue (`src/lib/templates.json`).
pub const TEMPLATES_JSON: &str = include_str!("../../../src/lib/templates.json");

#[derive(Deserialize)]
struct Catalogue {
    templates: Vec<Template>,
}

#[derive(Deserialize)]
struct Template {
    id: String,
    name: String,
    action: Option<Action>,
    #[serde(rename = "macro")]
    macro_def: Option<TemplateMacro>,
}

#[derive(Deserialize)]
struct TemplateMacro {
    name: String,
    steps: Vec<Step>,
}

/// A configuration that uses every template once, each on its own trigger,
/// exactly as the UI would add them.
pub fn all_templates_config() -> Config {
    let catalogue: Catalogue = serde_json::from_str(TEMPLATES_JSON).expect("templates.json matches the schema");
    let mods = [
        Modifiers::default(),
        Modifiers { ctrl: true, ..Default::default() },
        Modifiers { shift: true, ..Default::default() },
        Modifiers { alt: true, ..Default::default() },
        Modifiers { ctrl: true, shift: true, ..Default::default() },
        Modifiers { ctrl: true, alt: true, ..Default::default() },
        Modifiers { alt: true, shift: true, ..Default::default() },
    ];
    let mut config = Config::default();
    let profile = &mut config.profiles[0];
    for (i, t) in catalogue.templates.into_iter().enumerate() {
        let trigger = Chord { key: format!("F{}", 13 + i % 12), mods: mods[i / 12].clone() };
        let action = match (t.action, t.macro_def) {
            (Some(action), None) => action,
            (None, Some(m)) => {
                let id = format!("mac-{}", t.id);
                profile.macros.push(Macro { id: id.clone(), name: m.name, steps: m.steps });
                Action::Macro { macro_id: id }
            }
            _ => panic!("template {} needs exactly one of action or macro", t.id),
        };
        profile.mappings.push(Mapping {
            id: format!("t-{}", t.id),
            name: t.name,
            enabled: true,
            trigger,
            action,
            hold: None,
            double_tap: None,
            hold_ms: None,
            app: None,
            edited: 0,
        });
    }
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ahk, validate};

    #[test]
    fn every_template_validates_and_generates() {
        let config = all_templates_config();
        assert!(config.profiles[0].mappings.len() >= 60);
        let (report, plan) = validate::validate(&config);
        assert!(report.errors.is_empty(), "{:#?}", report.errors);
        let script = ahk::generate(&plan.unwrap(), &ahk::GenOptions { scripts_dir: "C:\\s" });
        assert!(script.contains("SendText FormatTime(, \"yyyy-MM-dd\")"));
        assert!(script.contains("DllCall(\"LockWorkStation\")"));
    }
}
