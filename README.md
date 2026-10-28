# KeyForge

Turn unused keys on a Windows 11 laptop into remaps, shortcuts, launchers and macros. KeyForge is a small Tauri app (Rust and React). One supervised AutoHotkey v2 engine does all keyboard interception.

> **Status: Stage 1 of 6, engine proof of concept.** You can detect a physical key and remap it to another key, globally. Mappings are not saved yet (Stage 2). See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the design, the AutoHotkey evaluation, the safety model and the stage plan.

## What works in Stage 1

- **Key detection.** Press a key and see its name, virtual-key code, scan code and held modifiers. The engine captures exactly one key and then stops listening.
- **Key remapping**, including Ctrl/Shift/Alt/Win trigger combinations. Down, up and auto-repeat are handled correctly, and the remap works while any (non-elevated) app has focus.
- **Validation** before anything is applied: unknown keys, duplicate triggers, self-remaps and reserved keys are rejected, and chains get a warning.
- **Safety:**
  - Mappings start disabled.
  - **Ctrl+Alt+Shift+Backspace** disables everything and cannot be overridden.
  - The engine dies with KeyForge, even if KeyForge crashes.
  - Stuck modifiers are released after an abnormal exit.

## Install

Download the `keyforge-windows-installer` artifact from the latest successful [CI run](../../actions/workflows/ci.yml). Then run `KeyForge_0.1.0_x64-setup.exe`. The installer installs per user (no administrator rights) and bundles AutoHotkey v2. WebView2 ships with Windows 11.

## Build from source

Requirements: Windows 11 x64, [Rust](https://rustup.rs) stable (MSVC), Node.js 22, and AutoHotkey v2 (either installed, or `AutoHotkey64.exe` copied into `src-tauri/resources/engine/`).

```bash
npm ci
```

```bash
npm run tauri dev
```

To build the installer:

```bash
npm run tauri build
```

The installer is written to `target/release/bundle/nsis/`.

## Tests

On Linux, macOS or Windows, run the unit tests and the engine lifecycle tests (a fake engine is used on Unix):

```bash
cargo test
```

On Windows, run the end-to-end tests against real AutoHotkey. They inject keys with `SendInput`, so do not type while they run (about 10 s):

```powershell
$env:KEYFORGE_AHK = "C:\Program Files\AutoHotkey\v2\AutoHotkey64.exe"
cargo test -p keyforge-engine -- --test-threads=1
```

The end-to-end tests check that:

- every key in the key table loads in AutoHotkey;
- remap down and up both arrive, the original key is blocked, and the key is normal again after stop;
- detection reports the physical key;
- the emergency shortcut ends the engine with no stuck modifiers;
- the engine exits when its stdin closes, and when its supervisor is killed.

CI runs all of this on `windows-latest` and builds the installer.

### Manual test checklist (Stage 1)

1. Start KeyForge. The header says **Mappings off**. Typing anywhere is unaffected.
2. Click **Detect key** and press **F13**, or any unused key. The name, VK, SC and modifiers appear and the trigger field is filled in. Press **Detect key** and wait 15 s: you see a timeout message.
3. Choose **Play/Pause** as the target, click **Add remap**, then turn on **Enable mappings**. Focus another app and press the key: media play/pause toggles.
4. Hold the key down: the target repeats and stops when you release it, with no stuck key.
5. Press **Ctrl+Alt+Shift+Backspace**. The banner says mappings are disabled, and the key behaves normally again.
6. Enable mappings, then end `AutoHotkey64.exe` in Task Manager. KeyForge reports that the engine stopped and the keyboard is normal.
7. Enable mappings, then end `keyforge.exe` in Task Manager. `AutoHotkey64.exe` disappears too and the key is normal.
8. Add a mapping for Backspace, or for both LCtrl and RCtrl, and click **Apply**. It is rejected with an explanation.

## Limitations (honest list)

- **Fn key.** On almost every laptop, Fn is handled by the keyboard's embedded controller and never reaches Windows. It cannot be detected or remapped by any Windows software. Combinations like Fn+F5 reach Windows only as whatever key the firmware sends, if any. Use **Detect key** to see what your hardware actually reports.
- **Some OEM keys** (vendor hotkeys, airplane mode, brightness on many models) go through vendor drivers or ACPI and are invisible as key events. Keys that do report a scan code but have no name show up as `vkXXscYYY` or `SCxxx`, and can still be mapped.
- **Elevated apps.** While an app that runs as administrator has focus, Windows (UIPI) prevents the non-elevated engine from seeing or sending keys. KeyForge runs without elevation by default.
- **Secure desktop.** The sign-in screen, UAC prompts and Ctrl+Alt+Del are never affected.

## Troubleshooting

| Symptom | Fix |
|---|---|
| "AutoHotkey v2 was not found" | Reinstall KeyForge. For dev builds, install AutoHotkey v2 or set `KEYFORGE_AHK`. |
| "AutoHotkey v2 is required, found 1.x" | `KEYFORGE_AHK` points at v1. Point it at the v2 `AutoHotkey64.exe`. |
| Keys feel wrong or stuck | Press **Ctrl+Alt+Shift+Backspace**, then press and release Ctrl, Alt, Shift and Win once. |
| Mapping does nothing in one app | That app is probably elevated (see Limitations). |
| Engine error shown on Apply | The previous mappings stay active. The message is the engine's own error output. |

## Project layout

```
crates/core      key table, config model, validation, AHK generation, protocol (pure Rust)
crates/engine    engine supervisor + Windows safety mechanisms + e2e tests
src-tauri        Tauri app (commands, engine host)
src              React UI
docs             architecture and design notes
```

AutoHotkey is © its authors and licensed under GPL-2.0. KeyForge runs it as a separate, unmodified program; source code: https://github.com/AutoHotkey/AutoHotkey.
