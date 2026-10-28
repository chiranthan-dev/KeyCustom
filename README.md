# KeyForge

Turn unused keys on a Windows 11 laptop into shortcuts, app launchers, text snippets, macros, media controls and more. Mappings work in every app, persist across restarts and can be switched per profile or per application.

KeyForge is a small Tauri app (Rust and React, using Windows' built-in WebView2). The installer is about 2 MB. One supervised AutoHotkey v2 process does all keyboard interception, and KeyForge generates its script from your configuration. The design, safety model and AutoHotkey evaluation are in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Features

- **Keyboard Studio:**
  - a clickable laptop keyboard plus F13–F24, media, navigation and numpad keys;
  - mapped keys are highlighted;
  - **Detect** shows the name, virtual-key code, scan code and held modifiers of any key the hardware reports;
  - a searchable key list, per-key enable/disable, **Test** and **Restore original**.
- **Actions:**
  - key remap and shortcut (e.g. Ctrl+Shift+S);
  - open app and open website;
  - type text and macro;
  - media (play/pause, next, previous, volume, mute);
  - Windows settings pages and utilities;
  - confirmed `.ps1`/`.bat`/`.cmd` scripts.
- **Behaviour:**
  - modifier combinations as triggers;
  - tap vs hold with a configurable threshold;
  - double tap;
  - each mapping fires once per press, without auto-repeat triggers or stuck keys.
- **Macro Builder:** add, reorder, duplicate, edit and delete key, text, delay, app and URL steps without code.
- **Profiles:** Productivity, Coding, Study and Gaming to start with; add your own. Switch with a shortcut, from the dashboard or tray, or automatically while a chosen app is in front.
- **Application rules:** app-specific mappings that override global ones, or all mappings off in an app (e.g. games).
- **Changes:**
  - undo/redo (Ctrl+Z/Ctrl+Y);
  - **Preview** of every change and of the generated engine script before applying;
  - conflict detection with clear errors.
- **Configuration:** a documented JSON schema ([docs/CONFIG.md](docs/CONFIG.md)) with import, export, backups, reset and automatic recovery from damaged files.

## Safety

- Mappings are **off** when KeyForge starts, unless you opt in.
- **Ctrl+Alt+Shift+Backspace** turns everything off at once, and no mapping can override it. Windows also delivers it to KeyForge directly if the engine is not responding.
- The tray menu offers Pause, Resume, Open settings and Exit.
- If the engine crashes or is killed, Windows removes its keyboard hook. KeyForge then releases stuck modifiers and turns mappings off. If KeyForge itself crashes, Windows ends the engine as well; this is tested.
- Changes are validated first and applied atomically. The previous configuration is kept as last-known-good and restored if the new one fails to load.
- No keystroke logging, clipboard access or network use. Detection reads one key on request.
- Scripts must be confirmed. KeyForge runs a private copy pinned by SHA-256 and, by default, asks before each run.
- It runs without administrator rights.

## Install

Download the `keyforge-windows-installer` artifact from the latest successful [CI run](../../actions/workflows/ci.yml), unzip it and run `KeyForge_0.1.0_x64-setup.exe`. The installer:

- installs per user (no administrator rights);
- bundles AutoHotkey v2;
- uses the WebView2 runtime that ships with Windows 11.

The installer is not code-signed, so SmartScreen may ask you to confirm (**More info → Run anyway**).

## Quick start

1. Open **Keyboard Studio** and click a key, or click **Detect key** and press it.
2. Click **Add mapping**, choose what it does, and click **Save mapping**.
3. Click **Apply changes** at the bottom of the window. This saves your mappings.
4. Turn **mappings on** using the prompt, the sidebar switch or the tray menu. Then use the key in any app. **Test** next to the mapping checks the full path.

## Build from source

Requirements:

- Windows 11 x64;
- [Rust](https://rustup.rs) stable (MSVC);
- Node.js 22;
- AutoHotkey v2, either installed or with `AutoHotkey64.exe` copied into `src-tauri/resources/engine/`.

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

On any OS, run the core and controller tests:

```bash
cargo test
```

Run the UI unit tests:

```bash
npm test
```

On Windows, the end-to-end tests use real AutoHotkey and inject keys:

```powershell
$env:KEYFORGE_AHK = "C:\Program Files\AutoHotkey\v2\AutoHotkey64.exe"
cargo test -p keyforge-engine --test windows_e2e -- --test-threads=1
```

[docs/TESTING.md](docs/TESTING.md) lists what each suite covers and gives a 30-step manual test plan.

## Limitations

- **Fn key.** On almost every laptop, Fn is handled by the keyboard's embedded controller and never reaches Windows. No Windows software can detect or remap it. Fn combinations reach Windows only as whatever key the firmware sends, if any; use **Detect** to see what your hardware reports.
- **Vendor keys.** Brightness, airplane mode, keyboard backlight and similar keys usually go through firmware or vendor drivers, and are invisible as key events. Keys that report a scan code but have no name appear as `vkXXscYYY` or `SCxxx` and can be mapped.
- **Keys that arrive as combinations.** The Copilot key is one example: the firmware sends Win+Shift+F23. Detect captures the combination, and the mapping releases those modifiers before running its action. Such keys behave as a single press, so remapping one to a key you want to *hold* (such as Ctrl) is not possible.
- **Elevated apps.** While an app running as administrator is in front, Windows (UIPI) prevents the non-elevated engine from seeing or sending keys. The sign-in screen, UAC prompts and Ctrl+Alt+Del are never affected.
- **Win+L and Ctrl+Alt+Del** are reserved by Windows. **Backspace** is reserved for the emergency shortcut.

## Troubleshooting

| Symptom | Fix |
|---|---|
| A mapping does nothing | 1. Did you click **Apply changes**? A "not applied" badge means no. 2. Are mappings **on** (sidebar)? 3. Is the right profile active, or is an automatic app or a *mappings off* rule in effect? 4. Click **Test** on the mapping: it tells you whether the key reaches the engine. 5. Is the app in front running as administrator? |
| Detect shows nothing for a key | The key is handled by firmware or a vendor driver and never reaches Windows (see Limitations). |
| A laptop key detects as a combination (e.g. Win+Shift+F23) | This is normal for some keys; keep the detected modifiers in the trigger. |
| Keys feel wrong or stuck | Press **Ctrl+Alt+Shift+Backspace**, then press and release Ctrl, Alt, Shift and Win once. |
| "AutoHotkey v2 was not found" | Reinstall KeyForge, or set a custom path in **Settings → Engine**. |
| "AutoHotkey v2 is required, found 1.x" | The custom path or `KEYFORGE_AHK` points at v1. Point it at the v2 `AutoHotkey64.exe`. |
| "The engine stopped unexpectedly" | Mappings were turned off to keep the keyboard safe. **Settings → Diagnostics** shows the engine's messages. Turn mappings back on after fixing the cause. |
| A notice says the configuration was restored | `config.json` was damaged and set aside (named in the notice). The last working configuration is active. |
| An action fails (app won't open, etc.) | **Settings → Diagnostics** shows the error from the engine. |

## Project layout

```
crates/core      key table, config schema, validation, AutoHotkey generation, protocol, config store
crates/engine    engine supervisor, controller (app logic), Windows safety mechanisms, e2e tests
src-tauri        Tauri shell: commands, tray, single instance, dialogs
src              React UI (pages/, components/, lib/)
docs             architecture, configuration schema, testing
```

AutoHotkey is © its authors and licensed under GPL-2.0. KeyForge runs it as a separate, unmodified program, and the installer includes its license. Source code: https://github.com/AutoHotkey/AutoHotkey.
