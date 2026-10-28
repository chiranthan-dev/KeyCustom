//! Generates the single AutoHotkey script the engine runs: the runtime,
//! then the configuration, hotkeys and one function per action.

use crate::config::{Action, Chord, Step, SystemTarget};
use crate::keys;
use crate::validate::{script_kind, Plan};
use std::collections::BTreeSet;
use std::fmt::Write;

pub const RUNTIME: &str = include_str!("runtime.ahk");

pub struct GenOptions<'a> {
    /// Folder holding KeyForge's private copies of confirmed scripts.
    pub scripts_dir: &'a str,
}

/// File name of the private copy of a confirmed script.
pub fn script_copy_name(sha256: &str, path: &str) -> String {
    format!("{}.{}", sha256.to_ascii_lowercase(), script_kind(path).unwrap_or("txt"))
}

/// Tag that identifies a mapping in `fired` events and `test` commands.
pub fn mapping_tag(profile: &str, mapping: &str) -> String {
    format!("{profile}/{mapping}")
}

/// AutoHotkey double-quoted string literal.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '`' => out.push_str("``"),
            '"' => out.push_str("`\""),
            ';' => out.push_str("`;"),
            '\n' => out.push_str("`n"),
            '\r' => out.push_str("`r"),
            '\t' => out.push_str("`t"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `Send` text for a chord, e.g. `^+{s}`. Key tokens never need escaping.
fn chord_send(chord: &Chord) -> String {
    let key = keys::resolve(&chord.key).expect("validated key");
    format!("{}{{{}}}", chord.mods.ahk_prefix(), key.send)
}

struct Gen<'a> {
    opts: &'a GenOptions<'a>,
    functions: String,
    next: usize,
}

impl Gen<'_> {
    fn function(&mut self, body: &[String]) -> String {
        self.next += 1;
        let name = format!("KF_A{}", self.next);
        let _ = writeln!(self.functions, "{name}() {{");
        for line in body {
            let _ = writeln!(self.functions, "    {line}");
        }
        self.functions.push_str("}\n");
        name
    }

    fn action_body(&self, action: &Action, blind: bool, macros: &[(String, String)]) -> Vec<String> {
        let line = match action {
            Action::Remap { to } => {
                let key = keys::resolve(to).expect("validated key");
                format!("Send {}", quote(&format!("{}{{{}}}", if blind { "{Blind}" } else { "" }, key.send)))
            }
            Action::Shortcut { keys } => format!("Send {}", quote(&chord_send(keys))),
            Action::Launch { path, args } => format!("KF_Launch({}, {})", quote(path), quote(args)),
            Action::Url { url } => format!("KF_Open({})", quote(url)),
            Action::Text { text } => format!("SendText {}", quote(text)),
            Action::Macro { macro_id } => {
                let name = macros.iter().find(|(id, _)| id == macro_id).map(|(_, f)| f.as_str()).expect("validated macro");
                format!("{name}()")
            }
            Action::Media { action } => format!("Send {}", quote(&format!("{{{}}}", action.key()))),
            Action::System { action } => match action.target() {
                SystemTarget::Open(t) => format!("KF_Open({})", quote(t)),
                SystemTarget::Send(s) => format!("Send {}", quote(s)),
                SystemTarget::Lock => "DllCall(\"LockWorkStation\")".to_string(),
            },
            Action::Script { path, sha256, confirm_each_run } => {
                let copy = format!("{}\\{}", self.opts.scripts_dir.trim_end_matches('\\'), script_copy_name(sha256, path));
                let kind = script_kind(path).expect("validated script");
                format!("KF_Script({}, {}, {}, {})", quote(&copy), quote(kind), u8::from(*confirm_each_run), quote(path))
            }
        };
        vec![line]
    }

    fn action(&mut self, action: &Action, blind: bool, macros: &[(String, String)]) -> String {
        let body = self.action_body(action, blind, macros);
        self.function(&body)
    }
}

/// Build the complete engine script from a validated plan.
pub fn generate(plan: &Plan<'_>, opts: &GenOptions<'_>) -> String {
    let mut gen = Gen { opts, functions: String::new(), next: 0 };
    let mut init = String::new();
    let mut hotkeys = String::new();
    let mut up_keys = BTreeSet::new();
    let settings = &plan.config.settings;

    let _ = writeln!(init, "KF_Profile := {}", quote(plan.active));
    for pp in &plan.profiles {
        let p = pp.profile;
        let _ = writeln!(init, "KF_Names[{}] := {}", quote(&p.id), quote(&p.name));
        for app in &p.auto_apps {
            let _ = writeln!(init, "KF_AutoApps[{}] := {}", quote(&app.to_ascii_lowercase()), quote(&p.id));
        }
        if !pp.disabled_apps.is_empty() {
            let _ = writeln!(init, "KF_Disabled[{}] := {}", quote(&p.id), quote(&format!("|{}|", pp.disabled_apps.join("|"))));
        }
    }

    hotkeys.push_str("\n; --- Profile shortcuts\n#HotIf\n");
    for pp in &plan.profiles {
        if let Some((key, mods)) = &pp.switch {
            let _ = writeln!(hotkeys, "{}{}::KF_SetProfile({})", mods.ahk_prefix(), key.hotkey, quote(&pp.profile.id));
        }
    }

    for pp in &plan.profiles {
        let p = pp.profile;
        let _ = writeln!(hotkeys, "\n; --- Profile {}", quote(&p.name));
        let mut macros = Vec::new();
        for (i, m) in p.macros.iter().enumerate() {
            gen.next += 1;
            let name = format!("KF_M{}_{}", gen.next, i);
            let _ = writeln!(gen.functions, "{name}() {{");
            for step in &m.steps {
                let line = match step {
                    Step::Keys { keys } => format!("Send {}", quote(&chord_send(keys))),
                    Step::Text { text } => format!("SendText {}", quote(text)),
                    Step::Delay { ms } => format!("Sleep {ms}"),
                    Step::Launch { path, args } => format!("KF_Launch({}, {})", quote(path), quote(args)),
                    Step::Url { url } => format!("KF_Open({})", quote(url)),
                };
                let _ = writeln!(gen.functions, "    {line}");
            }
            gen.functions.push_str("}\n");
            macros.push((m.id.clone(), name));
        }

        // App-specific variants first; global variants also exclude those apps.
        let mut ordered: Vec<_> = pp.mappings.iter().collect();
        ordered.sort_by_key(|m| m.mapping.app.is_none());
        for pm in ordered {
            let m = pm.mapping;
            let tag = mapping_tag(&p.id, &m.id);
            let app = m.app.as_deref().map(str::to_ascii_lowercase).unwrap_or_default();
            let excluded = if pm.overridden_in.is_empty() { String::new() } else { format!("|{}|", pm.overridden_in.join("|")) };
            let _ = writeln!(hotkeys, "#HotIf KF_On({}, {}, {})", quote(&p.id), quote(&app), quote(&excluded));
            let key = &pm.key.hotkey;
            let blind = m.trigger.mods.is_empty();
            let tap = gen.action(&m.action, blind, &macros);
            let _ = writeln!(init, "KF_Tests[{}] := {tap}", quote(&tag));
            if pm.wildcard_remap {
                let Action::Remap { to } = &m.action else { unreachable!() };
                let target = keys::resolve(to).expect("validated key");
                let _ = writeln!(hotkeys, "*{key}::KF_RemapDown({}, {}, {})", quote(&tag), quote(key), quote(&target.send));
                up_keys.insert(key.clone());
            } else {
                let hold = m.hold.as_ref().map(|a| gen.action(a, blind, &macros)).unwrap_or_else(|| "0".into());
                let dbl = m.double_tap.as_ref().map(|a| gen.action(a, blind, &macros)).unwrap_or_else(|| "0".into());
                let hold_ms = m.hold_ms.unwrap_or(settings.tap_hold_ms);
                let dbl_ms = if m.double_tap.is_some() { settings.double_tap_ms } else { 0 };
                let _ = writeln!(
                    hotkeys,
                    "{}{key}::KF_Run({}, {}, {hold_ms}, {dbl_ms}, {tap}, {hold}, {dbl})",
                    m.trigger.mods.ahk_prefix(),
                    quote(&tag),
                    quote(key)
                );
            }
        }
    }
    hotkeys.push_str("#HotIf\n");

    if !up_keys.is_empty() {
        hotkeys.push_str("\n; --- Remap releases (always active, so held keys are never left down)\n#SuspendExempt true\n");
        for key in &up_keys {
            let _ = writeln!(hotkeys, "~*{key} up::KF_KeyUp({})", quote(key));
        }
        hotkeys.push_str("#SuspendExempt false\n");
    }

    let mut out = String::with_capacity(RUNTIME.len() + init.len() + hotkeys.len() + gen.functions.len() + 64);
    out.push_str(RUNTIME);
    out.push_str(&init);
    out.push_str("KF_Start()\n");
    out.push_str(&hotkeys);
    out.push_str("\n; --- Actions\n");
    out.push_str(&gen.functions);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppMode, AppRule, Config, Macro, MediaAction, Modifiers, SystemAction};
    use crate::validate::tests::{config_with, mapping, remap};
    use crate::validate::validate;

    const OPTS: GenOptions = GenOptions { scripts_dir: "C:\\Data\\scripts" };

    fn script(config: &Config) -> String {
        let (report, plan) = validate(config);
        assert!(report.ok(), "{report:?}");
        generate(&plan.unwrap(), &OPTS)
    }

    fn generated(config: &Config) -> String {
        script(config)[RUNTIME.len()..].to_string()
    }

    #[test]
    fn plain_remap_uses_down_and_always_on_up() {
        let out = generated(&config_with(vec![remap("play", "F13", "Media_Play_Pause"), remap("semi", "Semicolon", "vkE9sc0F1")]));
        assert!(out.contains("#HotIf KF_On(\"productivity\", \"\", \"\")\n*F13::KF_RemapDown(\"productivity/play\", \"F13\", \"Media_Play_Pause\")"), "{out}");
        assert!(out.contains("*SC027::KF_RemapDown(\"productivity/semi\", \"SC027\", \"vkE9\")"), "{out}");
        assert!(out.contains("#SuspendExempt true\n~*F13 up::KF_KeyUp(\"F13\")\n~*SC027 up::KF_KeyUp(\"SC027\")\n#SuspendExempt false"), "{out}");
        assert!(out.contains("KF_Tests[\"productivity/play\"] := KF_A1"), "{out}");
        assert!(out.contains("KF_A1() {\n    Send \"{Blind}{Media_Play_Pause}\"\n}"), "{out}");
        assert!(out.find("KF_Start()").unwrap() < out.find("*F13::").unwrap());
    }

    #[test]
    fn modifier_trigger_does_not_keep_modifiers_held() {
        let mut m = remap("a", "F23", "F13");
        m.trigger.mods = Modifiers { shift: true, win: true, ..Default::default() };
        let out = generated(&config_with(vec![m]));
        assert!(out.contains("+#F23::KF_Run(\"productivity/a\", \"F23\", 200, 0, KF_A1, 0, 0)"), "{out}");
        assert!(out.contains("Send \"{F13}\""), "non-blind send releases Shift+Win: {out}");
        assert!(!out.contains("KF_KeyUp"));
    }

    #[test]
    fn every_action_type_generates() {
        let mut maps = vec![
            mapping("sc", "F13", Action::Shortcut { keys: Chord { key: "S".into(), mods: Modifiers { ctrl: true, shift: true, ..Default::default() } } }),
            mapping("la", "F14", Action::Launch { path: "C:\\Program Files\\App\\app.exe".into(), args: "--x \"y\"".into() }),
            mapping("ur", "F15", Action::Url { url: "https://example.com/?a=1&b=2".into() }),
            mapping("tx", "F16", Action::Text { text: "Hi \"there\"; `tick`\nline".into() }),
            mapping("mc", "F17", Action::Macro { macro_id: "mac".into() }),
            mapping("md", "F18", Action::Media { action: MediaAction::VolumeUp }),
            mapping("sy", "F19", Action::System { action: SystemAction::Display }),
            mapping("lk", "F20", Action::System { action: SystemAction::LockScreen }),
            mapping("sd", "F21", Action::System { action: SystemAction::ShowDesktop }),
            mapping("sp", "F22", Action::Script { path: "D:\\tools\\go.ps1".into(), sha256: "AB".repeat(32), confirm_each_run: true }),
        ];
        maps[0].hold = Some(Action::Media { action: MediaAction::Mute });
        maps[0].double_tap = Some(Action::Url { url: "https://a.example".into() });
        maps[0].hold_ms = Some(350);
        let mut c = config_with(maps);
        c.profiles[0].macros.push(Macro {
            id: "mac".into(),
            name: "M".into(),
            steps: vec![
                Step::Keys { keys: Chord { key: "C".into(), mods: Modifiers { ctrl: true, ..Default::default() } } },
                Step::Delay { ms: 120 },
                Step::Text { text: "x".into() },
                Step::Launch { path: "notepad.exe".into(), args: String::new() },
                Step::Url { url: "https://b.example".into() },
            ],
        });
        let out = generated(&c);
        for needle in [
            "F13::KF_Run(\"productivity/sc\", \"F13\", 350, 250, KF_A2, KF_A3, KF_A4)",
            "Send \"^+{s}\"",
            "KF_Launch(\"C:\\Program Files\\App\\app.exe\", \"--x `\"y`\"\")",
            "KF_Open(\"https://example.com/?a=1&b=2\")",
            "SendText \"Hi `\"there`\"`; ``tick```nline\"",
            "Send \"{Volume_Up}\"",
            "KF_Open(\"ms-settings:display\")",
            "DllCall(\"LockWorkStation\")",
            "Send \"#d\"",
            "KF_Script(\"C:\\Data\\scripts\\abababababababababababababababababababababababababababababababab.ps1\", \"ps1\", 1, \"D:\\tools\\go.ps1\")",
            "    Send \"^{c}\"\n    Sleep 120\n    SendText \"x\"\n    KF_Launch(\"notepad.exe\", \"\")\n    KF_Open(\"https://b.example\")",
        ] {
            assert!(out.contains(needle), "missing {needle} in:\n{out}");
        }
    }

    #[test]
    fn profiles_apps_and_switch_keys() {
        let mut c = Config::default();
        c.profiles[1].switch_key = Some(Chord { key: "D1".into(), mods: Modifiers { ctrl: true, alt: true, ..Default::default() } });
        c.profiles[1].auto_apps = vec!["Code.exe".into()];
        let mut app = remap("b", "F13", "F15");
        app.app = Some("Chrome.exe".into());
        c.profiles[1].mappings = vec![remap("a", "F13", "F14"), app];
        c.profiles[1].app_rules.push(AppRule { id: "r".into(), process: "Game.exe".into(), name: String::new(), mode: AppMode::Disable, enabled: true });
        let out = generated(&c);
        for needle in [
            "KF_Profile := \"productivity\"",
            "KF_Names[\"coding\"] := \"Coding\"",
            "KF_AutoApps[\"code.exe\"] := \"coding\"",
            "KF_Disabled[\"coding\"] := \"|game.exe|\"",
            "#HotIf\n^!1::KF_SetProfile(\"coding\")",
            "#HotIf KF_On(\"coding\", \"chrome.exe\", \"\")\n*F13::",
            "#HotIf KF_On(\"coding\", \"\", \"|chrome.exe|\")\n*F13::",
        ] {
            assert!(out.contains(needle), "missing {needle} in:\n{out}");
        }
        assert!(out.find("\"chrome.exe\", \"\"").unwrap() < out.find("\"\", \"|chrome.exe|\"").unwrap(), "app variant first");
    }

    #[test]
    fn empty_config_is_valid_script() {
        let out = generated(&Config::default());
        assert!(out.starts_with("KF_Profile := \"productivity\"\n"));
        assert!(out.contains("KF_Start()"));
        assert!(!out.contains("KF_Run(") && !out.contains("up::"));
    }

    #[test]
    fn quoting_cannot_break_out() {
        assert_eq!(quote("a\"b`c;d\ne\u{7}"), "\"a`\"b``c`;d`ne\"");
        let mut c = Config::default();
        c.profiles[0].name = "x\"\nRun \"calc\"".into();
        let out = generated(&c);
        assert!(!out.contains("\nRun \"calc\""), "{out}");
    }

    #[test]
    fn runtime_keeps_safety_features() {
        for needle in ["#Requires AutoHotkey v2.0", "KeyHistory 0", "#SuspendExempt true\n^!+Backspace::", "PeekNamedPipe", "KF_ReleaseModifiers"] {
            assert!(RUNTIME.contains(needle), "{needle}");
        }
        assert!(RUNTIME.ends_with("; --- Generated configuration ------------------------------------------------\n"));
    }
}
