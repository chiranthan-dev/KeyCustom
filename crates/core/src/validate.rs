//! Validation that runs before any configuration is saved or reaches the
//! engine. Nothing is applied while a single error exists.

use crate::config::{Action, AppMode, Chord, Config, Mapping, Modifiers, Profile, Step, SCHEMA_VERSION};
use crate::keys::{self, KeyGroup, KeyRef};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

pub const MAX_MAPPINGS: usize = 500;
pub const MAX_PROFILES: usize = 32;
pub const MAX_STEPS: usize = 200;
pub const MAX_TEXT: usize = 10_000;
pub const MAX_DELAY_MS: u32 = 60_000;
pub const MAX_MACRO_MS: u64 = 300_000;
pub const SCRIPT_EXTENSIONS: [&str; 3] = ["ps1", "bat", "cmd"];

/// Reserved emergency shortcut: Ctrl+Alt+Shift+Backspace. The engine handles
/// it before any user mapping and the backend registers it as a fallback.
pub const EMERGENCY_LABEL: &str = "Ctrl+Alt+Shift+Backspace";
pub const EMERGENCY_KEY: &str = "Backspace";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub profile: Option<String>,
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

    pub fn error_text(&self) -> String {
        self.errors.iter().map(|i| i.message.as_str()).collect::<Vec<_>>().join("\n")
    }

    fn push(list: &mut Vec<Issue>, profile: Option<&str>, mapping: Option<&str>, message: String) {
        list.push(Issue { profile: profile.map(Into::into), mapping: mapping.map(Into::into), message });
    }
}

/// Validated, resolved configuration ready for code generation.
pub struct Plan<'a> {
    pub config: &'a Config,
    pub active: &'a str,
    pub profiles: Vec<PlanProfile<'a>>,
}

pub struct PlanProfile<'a> {
    pub profile: &'a Profile,
    pub switch: Option<(KeyRef, &'a Modifiers)>,
    pub mappings: Vec<PlanMapping<'a>>,
    /// Lowercase process names where this profile's global mappings are off.
    pub disabled_apps: Vec<String>,
}

pub struct PlanMapping<'a> {
    pub mapping: &'a Mapping,
    pub key: KeyRef,
    /// For a global mapping: apps that override the same trigger.
    pub overridden_in: Vec<String>,
    /// Plain remap that uses AutoHotkey's wildcard remap form.
    pub wildcard_remap: bool,
}

pub fn valid_id(s: &str) -> bool {
    (1..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn valid_process(s: &str) -> bool {
    s.len() > 4
        && s.len() <= 260
        && s.to_ascii_lowercase().ends_with(".exe")
        && !s.chars().any(|c| c.is_control() || "\\/:*?\"<>|".contains(c))
}

pub fn valid_url(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://"))
        && s.len() > 8
        && s.len() <= 2048
        && !s.chars().any(|c| c.is_whitespace() || c.is_control() || c == '"' || c == '`')
}

fn valid_path(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 1024 && !s.chars().any(|c| c.is_control() || c == '"')
}

fn valid_args(s: &str) -> bool {
    s.len() <= 2048 && !s.chars().any(|c| c.is_control())
}

pub fn script_kind(path: &str) -> Option<&'static str> {
    let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
    SCRIPT_EXTENSIONS.into_iter().find(|e| *e == ext)
}

fn check_text(text: &str) -> Result<(), String> {
    if text.is_empty() {
        Err("Text to type is empty.".into())
    } else if text.chars().count() > MAX_TEXT {
        Err(format!("Text is longer than {MAX_TEXT} characters."))
    } else {
        Ok(())
    }
}

fn is_emergency(chord: &Chord) -> bool {
    chord.key.eq_ignore_ascii_case(EMERGENCY_KEY) && chord.mods.ctrl && chord.mods.alt && chord.mods.shift
}

fn check_chord(chord: &Chord, what: &str) -> Result<KeyRef, String> {
    let key = keys::resolve(&chord.key).ok_or_else(|| format!("Unknown {what} key \"{}\".", chord.key))?;
    if is_emergency(chord) {
        return Err(format!("{EMERGENCY_LABEL} is reserved for the emergency shortcut."));
    }
    Ok(key)
}

fn check_action(action: &Action, profile: &Profile) -> Result<(), String> {
    match action {
        Action::Remap { to } => keys::resolve(to).map(|_| ()).ok_or_else(|| format!("Unknown target key \"{to}\".")),
        Action::Shortcut { keys } => check_chord(keys, "shortcut").map(|_| ()),
        Action::Launch { path, args } => {
            if !valid_path(path) {
                Err("Choose a program to launch.".into())
            } else if !valid_args(args) {
                Err("Launch arguments contain invalid characters.".into())
            } else {
                Ok(())
            }
        }
        Action::Url { url } => valid_url(url).then_some(()).ok_or_else(|| "Enter a full http:// or https:// address without spaces.".into()),
        Action::Text { text } => check_text(text),
        Action::Macro { macro_id } => profile
            .macros
            .iter()
            .any(|m| &m.id == macro_id)
            .then_some(())
            .ok_or_else(|| "The selected macro does not exist in this profile.".into()),
        Action::Media { .. } | Action::System { .. } => Ok(()),
        Action::Script { path, sha256, .. } => {
            if script_kind(path).is_none() {
                Err("Scripts must be .ps1, .bat or .cmd files.".into())
            } else if !valid_path(path) {
                Err("Choose a script file.".into())
            } else if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
                Err("Confirm the script before using it.".into())
            } else {
                Ok(())
            }
        }
    }
}

fn check_macro(m: &crate::config::Macro) -> Result<(), String> {
    if m.steps.is_empty() {
        return Err("Add at least one step.".into());
    }
    if m.steps.len() > MAX_STEPS {
        return Err(format!("A macro can have at most {MAX_STEPS} steps."));
    }
    let mut total: u64 = 0;
    for (i, step) in m.steps.iter().enumerate() {
        let n = i + 1;
        match step {
            Step::Keys { keys } => {
                check_chord(keys, "step").map_err(|e| format!("Step {n}: {e}"))?;
            }
            Step::Text { text } => check_text(text).map_err(|e| format!("Step {n}: {e}"))?,
            Step::Delay { ms } => {
                if *ms == 0 || *ms > MAX_DELAY_MS {
                    return Err(format!("Step {n}: delays must be between 1 and {MAX_DELAY_MS} ms."));
                }
                total += u64::from(*ms);
            }
            Step::Launch { path, args } => {
                if !valid_path(path) || !valid_args(args) {
                    return Err(format!("Step {n}: choose a program to launch."));
                }
            }
            Step::Url { url } => {
                if !valid_url(url) {
                    return Err(format!("Step {n}: enter a full http:// or https:// address."));
                }
            }
        }
    }
    if total > MAX_MACRO_MS {
        return Err(format!("The delays add up to more than {} seconds.", MAX_MACRO_MS / 1000));
    }
    Ok(())
}

fn is_typing_key(key: &KeyRef) -> bool {
    keys::find(&key.id).is_some_and(|d| matches!(d.group, KeyGroup::Alphanumeric | KeyGroup::Punctuation))
        || matches!(key.id.as_str(), "Space" | "Enter" | "Tab")
}

pub fn validate(config: &Config) -> (Report, Option<Plan<'_>>) {
    let mut report = Report::default();
    let errors = &mut report.errors;
    let warnings = &mut report.warnings;

    if config.version != SCHEMA_VERSION {
        Report::push(errors, None, None, format!("Unsupported configuration version {} (expected {SCHEMA_VERSION}).", config.version));
    }
    if config.profiles.is_empty() || config.profiles.len() > MAX_PROFILES {
        Report::push(errors, None, None, format!("Between 1 and {MAX_PROFILES} profiles are required."));
    }
    let s = &config.settings;
    if !(100..=2000).contains(&s.tap_hold_ms) {
        Report::push(errors, None, None, "The hold threshold must be between 100 and 2000 ms.".into());
    }
    if !(100..=1000).contains(&s.double_tap_ms) {
        Report::push(errors, None, None, "The double-tap window must be between 100 and 1000 ms.".into());
    }

    let mut profile_ids = HashSet::new();
    let mut auto_apps: HashMap<String, &str> = HashMap::new();
    let mut switch_keys: HashMap<(String, String), &str> = HashMap::new();
    for p in &config.profiles {
        let pid = Some(p.id.as_str());
        if !valid_id(&p.id) || !profile_ids.insert(p.id.as_str()) {
            Report::push(errors, pid, None, format!("Profile \"{}\" needs a unique id made of letters, digits, - or _.", p.name));
        }
        if p.name.trim().is_empty() || p.name.len() > 64 {
            Report::push(errors, pid, None, "Profile names must be 1 to 64 characters.".into());
        }
        for app in &p.auto_apps {
            if !valid_process(app) {
                Report::push(errors, pid, None, format!("\"{app}\" is not a valid program name (example: code.exe)."));
            } else if let Some(other) = auto_apps.insert(app.to_ascii_lowercase(), &p.name) {
                Report::push(errors, pid, None, format!("{app} already activates profile \"{other}\"."));
            }
        }
        if let Some(chord) = &p.switch_key {
            match check_chord(chord, "profile shortcut") {
                Err(e) => Report::push(errors, pid, None, e),
                Ok(_) if chord.mods.is_empty() => {
                    Report::push(errors, pid, None, "Profile shortcuts need at least one modifier (Ctrl, Shift, Alt or Win).".into());
                }
                Ok(key) => {
                    if let Some(other) = switch_keys.insert((key.id.clone(), chord.mods.ahk_prefix()), &p.name) {
                        Report::push(errors, pid, None, format!("{} already switches to profile \"{other}\".", chord.label()));
                    }
                }
            }
        }
    }
    let active = match config.profile(&config.active_profile) {
        Some(p) => p.id.as_str(),
        None => {
            if let Some(first) = config.profiles.first() {
                Report::push(warnings, None, None, format!("The active profile was missing; \"{}\" is used instead.", first.name));
            }
            config.profiles.first().map(|p| p.id.as_str()).unwrap_or("")
        }
    };

    let mut plan_profiles = Vec::new();
    for p in &config.profiles {
        let pid = Some(p.id.as_str());
        if p.mappings.len() > MAX_MAPPINGS {
            Report::push(errors, pid, None, format!("A profile can have at most {MAX_MAPPINGS} mappings."));
        }

        let mut macro_ids = HashSet::new();
        for m in &p.macros {
            if !valid_id(&m.id) || !macro_ids.insert(m.id.as_str()) {
                Report::push(errors, pid, None, format!("Macro \"{}\" needs a unique id.", m.name));
            }
            if let Err(e) = check_macro(m) {
                Report::push(errors, pid, None, format!("Macro \"{}\": {e}", m.name));
            }
        }

        let mut rule_apps = HashSet::new();
        let mut disabled_apps = Vec::new();
        for r in &p.app_rules {
            if !valid_process(&r.process) {
                Report::push(errors, pid, None, format!("\"{}\" is not a valid program name (example: code.exe).", r.process));
            } else if !rule_apps.insert(r.process.to_ascii_lowercase()) {
                Report::push(errors, pid, None, format!("{} has more than one rule.", r.process));
            } else if r.enabled && r.mode == AppMode::Disable {
                disabled_apps.push(r.process.to_ascii_lowercase());
            }
        }

        let mut mapping_ids = HashSet::new();
        let mut triggers: HashMap<(String, String, String), &str> = HashMap::new();
        let mut mappings: Vec<PlanMapping> = Vec::new();
        for m in &p.mappings {
            let mid = Some(m.id.as_str());
            if !valid_id(&m.id) || !mapping_ids.insert(m.id.as_str()) {
                Report::push(errors, pid, mid, "Mapping ids must be unique (letters, digits, - or _).".into());
                continue;
            }
            if !m.enabled {
                continue;
            }
            let label = m.trigger.label();
            let Some(key) = keys::resolve(&m.trigger.key) else {
                Report::push(errors, pid, mid, format!("Unknown trigger key \"{}\".", m.trigger.key));
                continue;
            };
            if key.id == EMERGENCY_KEY {
                Report::push(errors, pid, mid, format!("Backspace is reserved for the emergency shortcut ({EMERGENCY_LABEL})."));
                continue;
            }
            if key.modifier && !m.trigger.mods.is_empty() {
                Report::push(errors, pid, mid, "A modifier key cannot be combined with other modifiers as a trigger.".into());
                continue;
            }
            if m.trigger.mods.win && key.id == "L" {
                Report::push(errors, pid, mid, "Windows reserves Win+L for locking the PC; it cannot be remapped.".into());
                continue;
            }
            let app = match &m.app {
                Some(a) if !valid_process(a) => {
                    Report::push(errors, pid, mid, format!("\"{a}\" is not a valid program name (example: code.exe)."));
                    continue;
                }
                Some(a) => a.to_ascii_lowercase(),
                None => String::new(),
            };
            let mut action_errors = Vec::new();
            for (what, action) in [("Action", Some(&m.action)), ("Hold action", m.hold.as_ref()), ("Double-tap action", m.double_tap.as_ref())] {
                if let Some(action) = action {
                    if let Err(e) = check_action(action, p) {
                        action_errors.push(format!("{what}: {e}"));
                    }
                }
            }
            if let Action::Remap { to } = &m.action {
                if keys::resolve(to).is_some_and(|t| t.id == key.id) && m.trigger.mods.is_empty() {
                    action_errors.push("A key cannot be remapped to itself.".into());
                }
            }
            if let Some(ms) = m.hold_ms {
                if !(100..=2000).contains(&ms) {
                    action_errors.push("The hold threshold must be between 100 and 2000 ms.".into());
                }
            }
            if !action_errors.is_empty() {
                for e in action_errors {
                    Report::push(errors, pid, mid, e);
                }
                continue;
            }
            let slot = (key.id.clone(), m.trigger.mods.ahk_prefix(), app.clone());
            if let Some(other) = triggers.insert(slot, &m.id) {
                let scope = if app.is_empty() { String::new() } else { format!(" in {app}") };
                Report::push(errors, pid, mid, format!("{label}{scope} is already used by mapping \"{other}\"."));
                continue;
            }
            if let Some(other) = switch_keys.get(&(key.id.clone(), m.trigger.mods.ahk_prefix())) {
                Report::push(errors, pid, mid, format!("{label} is the shortcut for profile \"{other}\"."));
                continue;
            }
            if m.trigger.mods.is_empty() && switch_keys.keys().any(|(k, _)| *k == key.id) {
                Report::push(errors, pid, mid, format!("{} is part of a profile shortcut, so it cannot be mapped on its own.", key.id));
                continue;
            }
            if is_typing_key(&key) && !(m.trigger.mods.ctrl || m.trigger.mods.alt || m.trigger.mods.win) {
                Report::push(warnings, pid, mid, format!("{label} is used for normal typing; this mapping changes what it types."));
            }
            mappings.push(PlanMapping { mapping: m, key, overridden_in: Vec::new(), wildcard_remap: false });
        }

        // Keep at least one key of each modifier pair working so the
        // emergency shortcut can always be typed.
        for (left, right, name) in [("LCtrl", "RCtrl", "Ctrl"), ("LShift", "RShift", "Shift"), ("LAlt", "RAlt", "Alt")] {
            let used = |id: &str| mappings.iter().any(|r| r.key.id == id && r.mapping.app.is_none());
            if used(left) && used(right) {
                Report::push(errors, pid, None, format!("Left and right {name} cannot both be mapped; the emergency shortcut needs one of them."));
            }
        }

        // Global mappings step aside in apps that override the same trigger.
        let overrides: Vec<(String, String, String)> = mappings
            .iter()
            .filter_map(|r| r.mapping.app.as_ref().map(|a| (r.key.id.clone(), r.mapping.trigger.mods.ahk_prefix(), a.to_ascii_lowercase())))
            .collect();
        let with_mods: HashSet<String> = mappings.iter().filter(|r| !r.mapping.trigger.mods.is_empty()).map(|r| r.key.id.clone()).collect();
        for r in &mut mappings {
            if r.mapping.app.is_none() {
                let prefix = r.mapping.trigger.mods.ahk_prefix();
                r.overridden_in = overrides.iter().filter(|(k, m, _)| *k == r.key.id && *m == prefix).map(|(_, _, a)| a.clone()).collect();
            }
            r.wildcard_remap = matches!(r.mapping.action, Action::Remap { .. })
                && r.mapping.trigger.mods.is_empty()
                && r.mapping.hold.is_none()
                && r.mapping.double_tap.is_none()
                && !with_mods.contains(&r.key.id);
        }
        for r in &mappings {
            if let Action::Remap { to } = &r.mapping.action {
                let target = keys::resolve(to).map(|t| t.id).unwrap_or_default();
                if mappings.iter().any(|o| o.key.id == target && o.mapping.trigger.mods.is_empty() && o.mapping.app == r.mapping.app) {
                    Report::push(
                        warnings,
                        pid,
                        Some(&r.mapping.id),
                        format!(
                            "{} sends {target}, which is also mapped. Mappings never trigger each other, so it sends the original {target}.",
                            r.mapping.trigger.label()
                        ),
                    );
                }
            }
        }

        let switch = p.switch_key.as_ref().and_then(|c| keys::resolve(&c.key).map(|k| (k, &c.mods)));
        plan_profiles.push(PlanProfile { profile: p, switch, mappings, disabled_apps });
    }

    if !report.ok() {
        return (report, None);
    }
    (report, Some(Plan { config, active, profiles: plan_profiles }))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::config::{AppRule, MediaAction};

    pub fn mapping(id: &str, key: &str, action: Action) -> Mapping {
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

    pub fn remap(id: &str, from: &str, to: &str) -> Mapping {
        mapping(id, from, Action::Remap { to: to.into() })
    }

    pub fn config_with(mappings: Vec<Mapping>) -> Config {
        let mut c = Config::default();
        c.profiles[0].mappings = mappings;
        c
    }

    fn errors(c: &Config) -> Vec<String> {
        validate(c).0.errors.into_iter().map(|i| i.message).collect()
    }

    #[test]
    fn default_config_is_valid() {
        let c = Config::default();
        let (report, plan) = validate(&c);
        assert!(report.ok() && report.warnings.is_empty());
        assert_eq!(plan.unwrap().active, "productivity");
    }

    #[test]
    fn accepts_simple_remaps() {
        let c = config_with(vec![remap("a", "F13", "Media_Play_Pause"), remap("b", "RCtrl", "AppsKey")]);
        let (report, plan) = validate(&c);
        assert!(report.ok() && report.warnings.is_empty(), "{report:?}");
        assert!(plan.unwrap().profiles[0].mappings.iter().all(|m| m.wildcard_remap));
    }

    #[test]
    fn rejects_unknown_self_and_duplicate() {
        let c = config_with(vec![remap("a", "Nope", "F1"), remap("b", "F1", "F1"), remap("c", "F2", "F3"), remap("d", "f2", "F4")]);
        let (report, plan) = validate(&c);
        assert_eq!(report.errors.len(), 3, "{report:?}");
        assert!(plan.is_none(), "nothing is applied when anything is invalid");
    }

    #[test]
    fn same_trigger_in_different_scopes_is_allowed() {
        let mut ctrl = remap("b", "F13", "F14");
        ctrl.trigger.mods.ctrl = true;
        let mut app = remap("c", "F13", "F16");
        app.app = Some("Code.exe".into());
        let mut c = config_with(vec![remap("a", "F13", "F15"), ctrl, app]);
        c.profiles[1].mappings = vec![remap("x", "F13", "F17")];
        let (report, plan) = validate(&c);
        assert!(report.ok(), "{report:?}");
        let plan = plan.unwrap();
        let global = &plan.profiles[0].mappings[0];
        assert_eq!(global.overridden_in, vec!["code.exe".to_string()]);
        assert!(!global.wildcard_remap, "F13 also has a Ctrl variant");
        assert!(plan.profiles[1].mappings[0].wildcard_remap);
    }

    #[test]
    fn protects_emergency_shortcut() {
        assert!(!errors(&config_with(vec![remap("a", "Backspace", "Delete")])).is_empty());
        assert!(!errors(&config_with(vec![remap("a", "LCtrl", "F13"), remap("b", "RCtrl", "F14")])).is_empty());
        assert!(errors(&config_with(vec![remap("a", "LCtrl", "F13")])).is_empty());
        let send = Chord { key: "Backspace".into(), mods: Modifiers { ctrl: true, alt: true, shift: true, win: false } };
        assert!(!errors(&config_with(vec![mapping("a", "F13", Action::Shortcut { keys: send })])).is_empty());
    }

    #[test]
    fn validates_action_fields() {
        let bad = [
            Action::Url { url: "javascript:alert(1)".into() },
            Action::Url { url: "https://a b".into() },
            Action::Launch { path: " ".into(), args: String::new() },
            Action::Launch { path: "C:\\x\".exe".into(), args: String::new() },
            Action::Text { text: String::new() },
            Action::Macro { macro_id: "missing".into() },
            Action::Script { path: "C:\\run.exe".into(), sha256: "a".repeat(64), confirm_each_run: true },
            Action::Script { path: "C:\\run.ps1".into(), sha256: String::new(), confirm_each_run: true },
            Action::Remap { to: "Nope".into() },
        ];
        for action in bad {
            let c = config_with(vec![mapping("a", "F13", action.clone())]);
            assert_eq!(errors(&c).len(), 1, "{action:?}");
        }
        let good = [
            Action::Url { url: "https://example.com/?q=1".into() },
            Action::Launch { path: "C:\\Program Files\\App\\app.exe".into(), args: "--new-window".into() },
            Action::Text { text: "Hello \"world\" ; `n".into() },
            Action::Media { action: MediaAction::Mute },
            Action::Script { path: "C:\\run.ps1".into(), sha256: "a".repeat(64), confirm_each_run: false },
        ];
        for action in good {
            let c = config_with(vec![mapping("a", "F13", action.clone())]);
            assert!(errors(&c).is_empty(), "{action:?}");
        }
    }

    #[test]
    fn validates_macros() {
        let mut c = config_with(vec![mapping("a", "F13", Action::Macro { macro_id: "m".into() })]);
        c.profiles[0].macros.push(crate::config::Macro {
            id: "m".into(),
            name: "M".into(),
            steps: vec![Step::Keys { keys: Chord::new("C") }, Step::Delay { ms: 100 }, Step::Text { text: "x".into() }],
        });
        assert!(errors(&c).is_empty());
        c.profiles[0].macros[0].steps.push(Step::Delay { ms: 0 });
        assert!(errors(&c)[0].contains("Step 4"));
        c.profiles[0].macros[0].steps = vec![Step::Delay { ms: 60_000 }; 6];
        assert!(errors(&c)[0].contains("seconds"));
    }

    #[test]
    fn validates_profiles_and_rules() {
        let mut c = Config::default();
        c.profiles[1].switch_key = Some(Chord { key: "D1".into(), mods: Modifiers { ctrl: true, alt: true, ..Default::default() } });
        c.profiles[2].switch_key = c.profiles[1].switch_key.clone();
        c.profiles[0].auto_apps = vec!["code.exe".into()];
        c.profiles[1].auto_apps = vec!["Code.exe".into(), "not a program".into()];
        c.profiles[0].app_rules.push(AppRule { id: "r".into(), process: "game.exe".into(), name: String::new(), mode: AppMode::Disable, enabled: true });
        let errs = errors(&c);
        assert_eq!(errs.len(), 3, "{errs:?}");

        let mut c = Config::default();
        c.profiles[1].switch_key = Some(Chord { key: "F13".into(), mods: Modifiers { ctrl: true, ..Default::default() } });
        let mut clash = remap("a", "F13", "F14");
        clash.trigger.mods.ctrl = true;
        c.profiles[0].mappings = vec![clash];
        assert!(errors(&c)[0].contains("shortcut for profile"));
        c.profiles[0].mappings = vec![remap("a", "F13", "F14")];
        assert!(errors(&c)[0].contains("part of a profile shortcut"));
        c.profiles[1].switch_key.as_mut().unwrap().mods = Modifiers::default();
        c.profiles[0].mappings.clear();
        assert!(errors(&c)[0].contains("modifier"));
    }

    #[test]
    fn disabled_mappings_are_ignored() {
        let mut off = remap("b", "F13", "F15");
        off.enabled = false;
        let c = config_with(vec![remap("a", "F13", "F14"), off]);
        let (report, plan) = validate(&c);
        assert!(report.ok());
        assert_eq!(plan.unwrap().profiles[0].mappings.len(), 1);
    }

    #[test]
    fn warnings_for_typing_keys_and_chains() {
        let (report, _) = validate(&config_with(vec![remap("a", "F13", "F14"), remap("b", "F14", "F13"), remap("c", "J", "K")]));
        assert!(report.ok());
        assert_eq!(report.warnings.len(), 3, "{report:?}");
    }

    #[test]
    fn hold_and_double_tap_are_checked() {
        let mut m = mapping("a", "F13", Action::Media { action: MediaAction::PlayPause });
        m.hold = Some(Action::Url { url: "nope".into() });
        m.hold_ms = Some(50);
        let errs = errors(&config_with(vec![m]));
        assert_eq!(errs.len(), 2, "{errs:?}");
    }
}
