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
| `crates/engine` | Process supervisor (spawn, protocol, stop, kill), AutoHotkey discovery, Windows safety mechanisms. | Windows (lifecycle logic also tested on Unix with a fake engine) |
| `src-tauri` | Tauri app: commands, events, engine host. Tray and persistence arrive in Stage 2. | Windows |
| `src` | React + TypeScript + Tailwind UI. | WebView2 |

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

The engine reads commands on stdin, one per line: `ping`, `detect`, `cancel-detect`, `exit`. It writes one JSON object per line to stdout:

```json
{"event":"ready","ahk":"2.0.19","admin":false,"pid":1234}
{"event":"detect-started"}
{"event":"key","name":"F13","vk":124,"sc":100,"ctrl":false,"shift":false,"alt":false,"win":false}
{"event":"detect-ended","reason":"done|timeout|cancelled"}
{"event":"error","message":"..."}
{"event":"exit","reason":"requested|emergency|supervisor-lost"}
```

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

## Configuration (Stage 2)

`%LOCALAPPDATA%\dev.keyforge.app\config.json`, versioned (`"version": 1`) and documented in `docs/CONFIG.md`. It is written atomically (temporary file, then rename) and keeps a `config.last-good.json`. A corrupt file is moved aside and the last-good copy is loaded. Schema validation is the same `keyforge-core::validate` that gates the engine.

## Delivery stages

1. **Engine proof of concept** (this stage): supervisor, protocol, detection, remap, emergency shortcut, kill-on-close, Windows e2e tests, installer pipeline.
2. Persistent config, last-known-good, import/export/reset/backup, tray (Pause/Resume/Open/Exit), engine health ping and restart policy.
3. Sidebar UI, Keyboard Studio (visual keyboard, searchable key list, action editor, undo), shortcut/app/URL/text actions, test button.
4. Macro builder, profiles (Coding, Study, Productivity, Gaming), per-app rules via `#HotIf`, profile switch shortcuts, conflict view, preview of the generated mapping.
5. Tap/hold and double tap, media and system actions, custom scripts with validation and confirmation.
6. Edge-case tests, installer polish, full manual test plan, troubleshooting guide.
