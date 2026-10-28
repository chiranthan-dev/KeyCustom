# KeyForge architecture

## Goals that shape the design

- **One interception engine.** Exactly one process hooks the keyboard: an AutoHotkey v2 engine that KeyForge generates and supervises. The UI never captures global input, and the Rust backend does not install a second keyboard hook.
- **Fail toward a normal keyboard.** Every failure path ends with the engine gone. Windows removes a low-level hook when its process exits, so a missing engine means a normal keyboard.
- **Lightweight.** Tauri (the system WebView2, no bundled browser), a single small AutoHotkey process (~3 MB RAM idle), and no polling of the keyboard. The release build is size-optimised and stripped.

## Components

```
┌──────────────────────────── KeyForge.exe (Tauri, non-elevated) ────────────────────────────┐
│  React UI (WebView2)  ──invoke/events──▶  Host (src-tauri/src/host.rs)                     │
│                                             │ validate → generate → write script atomically │
│                                             │ start / stop / detect / recover                │
│                                             ▼                                              │
│                                 keyforge-engine (supervisor)                               │
│                                   • job object: KILL_ON_JOB_CLOSE                          │
│                                   • RegisterHotKey fallback for the emergency shortcut     │
│                                   • releases stuck modifiers after abnormal exits          │
└──────────────────────────────────────────┬─────────────────────────────────────────────────┘
                     stdin: commands        │        stdout: JSON events
                                            ▼
                 AutoHotkey64.exe keyforge.ahk  (the only keyboard hook)
                   runtime.ahk + generated hotkeys
```

| Crate / folder | Role | Platform |
|---|---|---|
| `crates/core` | Key table, config model, validation, AutoHotkey code generation, protocol parsing. Pure logic, fully unit tested. | any |
| `crates/core` (also) | Configuration store: atomic saves, last-known-good copy, recovery, backups, import and export. | any |
| `crates/engine` | Process supervisor (spawn, protocol, stop, kill), AutoHotkey discovery, Windows safety mechanisms, and the **controller**: the whole application logic (apply with rollback, detection, profiles, mapping test, health ping, script confirmation) with no UI dependency. | Windows (logic also tested on Unix with a fake engine) |
| `src-tauri` | Thin Tauri shell: commands that call the controller, the tray menu, close to tray, single instance, file dialogs. | Windows |
| `src` | React + TypeScript + Tailwind UI: dashboard, keyboard studio, macro builder, profiles, application rules, settings. | WebView2 |

## Why AutoHotkey v2, and where it is not used

Each requirement checked against AutoHotkey v2:

| Requirement | AutoHotkey v2 | Notes |
|---|---|---|
| Global interception while other apps have focus | ✅ `WH_KEYBOARD_LL` hook (`#UseHook`) | Same mechanism a native hook would use. |
| Key remap with correct down/up and auto-repeat | ✅ `a::b` remap syntax | AutoHotkey expands it into separate down and up hotkeys. |
| Shortcuts, text insertion, app/URL launch, media keys | ✅ `Send`, `SendText`, `Run`, `Media_*` keys | Stage 3–5. |
| Macros with delays | ✅ | Stage 4. |
| Tap vs hold, double tap | ✅ `KeyWait`, `A_TimeSincePriorHotkey`, timers | Stage 5. |
| Per-application rules | ✅ `#HotIf WinActive("ahk_exe …")` | Evaluated by the engine on each key press, so no foreground polling. |
| No recursion between mappings | ✅ | The engine's own output never triggers its hotkeys (hook hotkeys, SendLevel 0). Chains do not cascade; validation warns about them. |
| Key detection (name, VK, SC, modifiers) | ✅ `InputHook` with `OnKeyDown` | One key, then it stops. Runs inside the same engine, with mappings suspended. |
| Elevated (admin) windows | ⚠️ | UIPI blocks a non-elevated hook and `SendInput` for elevated windows. No non-elevated tool can avoid this. KeyForge says so in the UI. |
| Fn key and some OEM keys | ❌ | Usually handled by the embedded controller and never reported to Windows. No software on Windows can see them. |

Conclusion: AutoHotkey v2 covers every action and event type. The backend uses native Win32 calls only where AutoHotkey cannot help, because those cases happen when the engine is gone or unresponsive:

- **Job object** with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`: when KeyForge exits or crashes, Windows kills the engine.
- **`RegisterHotKey`** for the emergency shortcut. It is a fallback, not a second hook. It fires only if the engine is not running, or has stopped responding and Windows has bypassed its hook.
- **`SendInput` key-ups** for modifiers that are still logically down after the engine was killed.

## Engine protocol

The engine reads commands on stdin, one per line: `ping`, `detect`, `cancel-detect`, `exit`, `profile <id>` and `test <profile>/<mapping>`. It writes one JSON object per line to stdout:

```json
{"event":"ready","ahk":"2.0.19","admin":false,"pid":1234,"profile":"coding"}
{"event":"detect-started"}
{"event":"key","name":"F13","vk":124,"sc":100,"ctrl":false,"shift":false,"alt":false,"win":false}
{"event":"detect-ended","reason":"done|timeout|cancelled"}
{"event":"fired","id":"coding/m-1"}
{"event":"profile","id":"gaming"}
{"event":"error","message":"..."}
{"event":"exit","reason":"requested|emergency|supervisor-lost"}
```

`fired` names only the configured mapping that ran, never the key, and it is not stored. It drives the dashboard's activity list and the mapping test.

## How the generated script works

One script holds every profile, so switching profiles never restarts the engine or lets two scripts compete for keys.

- **Context.** Every mapping hotkey sits under `#HotIf KF_On(profile, app, excluded)`. `KF_On` takes the foreground process from `WinGetProcessName("A")`, but only when a mapped key is pressed; nothing polls in the background. From that process it works out:
  - the **effective profile**: an automatic-app profile if the app is listed, otherwise the active one;
  - app-specific overrides, which also exclude the global variant of the same trigger in that app;
  - apps where an app rule turns the profile's mappings off.
- **Plain remaps** (no modifiers, no hold or double tap) use AutoHotkey's remap pattern: target down with `{Blind}…DownR` on key-down, so it holds and auto-repeats.
- **Other mappings** call `KF_Run`, which fires once per press. While the key is held, auto-repeat is dropped (`#MaxThreadsPerHotkey 1`).
  - With a hold action, it waits up to the threshold for the release.
  - With a double-tap action, it waits for the second tap before running the single tap.
  - Sends are not blind, so modifiers held on the trigger are released first. This matters for keys like the Copilot key, which the firmware sends as Win+Shift+F23.
- **Key releases.** Every mapped key also gets `*Key up` under `#HotIf KF_IsDown(key)`. This hotkey is exempt from Suspend. It runs whenever the engine handled the key-down, whatever profile or app is active by then. So:
  - a remap target is always released;
  - the trigger's own key-up never leaks to the app;
  - tap/hold timing works the same for physical and injected input.
- **Recursion.** Keys the engine sends never trigger its own hotkeys (hook hotkeys, SendLevel 0), so mappings and macros cannot recurse.

Exit codes: `0` requested, `2` supervisor lost (stdin closed), `3` emergency shortcut. AutoHotkey is started with `/ErrorStdOut`, so load errors arrive on stderr instead of in a dialog box, and KeyForge shows them verbatim.

## Safety model: what actually happens on each failure

| Failure | Mechanism | Result |
|---|---|---|
| User presses **Ctrl+Alt+Shift+Backspace** | Engine hotkey, defined before all user hotkeys and exempt from Suspend. Validation rejects mappings that could shadow it: Backspace as a trigger, or both keys of a modifier pair. | Engine exits with code 3. Host marks mappings disabled. |
| Engine hung (busy, deadlocked) | Windows bypasses an LL hook that misses its timeout, so keys flow again. The `RegisterHotKey` fallback then receives the emergency shortcut and the host kills the engine. | Normal keyboard. |
| Engine crashes or is killed | Windows removes its hook. Host sees `Exited`, sends key-up for stuck modifiers, disables mappings and shows why. | Normal keyboard, mappings off. |
| KeyForge crashes or is killed | The job handle closes, so Windows kills the engine. Independently, the engine sees stdin break within 50 ms and exits. | Normal keyboard. A modifier the engine pressed at that exact moment can stay logically down until you press and release it. |
| Invalid configuration | Validated before anything reaches the engine. Nothing is applied while any error exists. | Previous mappings stay. |
| New script fails to load | Host restarts the previous script; if that also fails, mappings are disabled. | Previous mappings, or none. |
| App start | Mappings are **disabled** until you enable them. | Normal keyboard. |

What is *not* claimed: if Windows itself hangs, nothing user-mode can help. A key held down by injected input during a crash of both processes cannot be released by software that is no longer running.

## Privacy

- `KeyHistory 0` in the engine. Detection captures a single key on request and keeps nothing.
- Nothing is logged except engine errors. Nothing is transmitted; the app makes no network requests. The CSP blocks remote content.
- No clipboard access.

## Applying a configuration

`Controller::apply` is atomic:

1. Validate. This covers the schema rules plus file checks (confirmed scripts must still match their hash). On any error, nothing changes.
2. Copy the current `config.json` to `config.last-good.json`, then write the new file.
3. If mappings are on, stop the engine, write the new script and start it. If it does not report `ready` within 5 seconds, restore the previous configuration on disk and in memory and restart it. If even that fails, mappings turn off. Two engines never run at the same time.

The configuration format, files and validation rules are documented in [CONFIG.md](CONFIG.md).

## Health and recovery

- While mappings are on, the controller pings the engine every 5 seconds. With no answer for 15 seconds, the engine is killed, mappings turn off and the reason is shown. A hung engine's hook has already been bypassed by Windows by then.
- If the engine exits unexpectedly, mappings turn off rather than restarting automatically, so a crashing configuration cannot loop. The dashboard shows why, and the tray or dashboard turns them back on.
- The tray menu (Pause, Resume, Open settings, Exit) works even while the window is hidden. Closing the window hides it to the tray (configurable).
- A second KeyForge instance only focuses the first, so two engines can never compete.

## Delivery stages

All six stages are implemented:

1. **Engine proof of concept:** supervisor, protocol, detection, remap, emergency shortcut, kill-on-close.
2. **Reliability:**
   - persistent config with last-known-good and recovery;
   - import, export, reset and backups;
   - tray menu, health ping and rollback.
3. **Keyboard Studio and basic actions:**
   - visual keyboard, searchable key list, action editor, undo/redo;
   - shortcut, app, URL and text actions;
   - the Test button.
4. **Macros, profiles and app rules:**
   - macro builder and profiles with switch shortcuts and automatic apps;
   - app rules (override or disable), conflict detection, preview.
5. **Advanced behaviour and actions:** tap/hold, double tap, media and system actions, confirmed scripts.
6. **Tests, packaging and docs:**
   - Windows end-to-end tests for every behaviour;
   - NSIS installer that bundles AutoHotkey;
   - manual test plan ([TESTING.md](TESTING.md)) and troubleshooting ([README](../README.md#troubleshooting)).
