# Testing KeyForge

## Automated tests

| Suite | Runs on | Command | Covers |
|---|---|---|---|
| Core unit tests | any OS | `cargo test -p keyforge-core` | Key table, schema round-trip, every validation rule, code generation for every action and feature, string escaping, config store (recovery, last-good, backups, import/export). |
| Controller and supervisor | Linux/macOS (fake engine) | `cargo test -p keyforge-engine` | Engine lifecycle, apply with rollback, persistence, detection engine lifecycle, profile switching, test command, hang detection, crash handling, script confirmation and tamper detection, import/export/backup/reset. |
| Windows end-to-end | Windows + AutoHotkey v2 | see below | The real engine with injected keys and a keyboard hook observer (details below). |
| UI model | any OS | `npm test` | Action defaults and descriptions, change summaries for the preview, id and process-name helpers. |

The Windows end-to-end suite checks:

- every key name and every action type loads in AutoHotkey;
- remaps send key-down and key-up, block the original key, and the key is normal again after stop;
- a trigger with modifiers (Copilot-style Win+Shift+F23) sends a clean target;
- shortcut and Unicode text actions;
- tap vs hold vs double tap;
- auto-repeat fires once;
- macro step order and timing;
- profile switching by shortcut and by command;
- a held remap is released after a profile change;
- a launched program runs;
- detection;
- the emergency shortcut leaves no stuck modifiers;
- the engine exits when stdin closes or the supervisor is killed;
- the full app flow through the controller: detect → apply → enable → key works → Test → disable → key restored → restart keeps the mapping and starts disabled.

Run the Windows suite (it injects keys, so don't type for about 15 seconds):

```powershell
$env:KEYFORGE_AHK = "C:\Program Files\AutoHotkey\v2\AutoHotkey64.exe"
cargo test -p keyforge-engine --test windows_e2e -- --test-threads=1
```

CI (`.github/workflows/ci.yml`) runs everything above. On `windows-latest` it also:

- downloads AutoHotkey v2.0.19 and sets `KEYFORGE_REQUIRE_E2E=1`, so a missing AutoHotkey fails the run instead of skipping;
- builds the NSIS installer;
- checks that the release build starts and stays running;
- uploads the installer as an artifact.

## Manual test plan

Use a non-elevated app such as Notepad for typing checks. "Mapped key" means a key you don't otherwise use, such as Insert, Pause, ScrollLock or F13–F24.

### Install and first start
1. Run the installer. It must not ask for administrator rights.
2. Start KeyForge. The dashboard says **Paused**, the tray icon tooltip says "mappings off", and the keyboard is unaffected.
3. Start KeyForge a second time. The existing window comes to the front; no second instance or engine starts.

### Detect and remap
4. In **Keyboard Studio**, click **Detect key** and press Insert. The detected-key panel shows the name, VK 0x2D, SC 0x152 and modifiers "none", and Insert is selected.
5. Click **Detect key** and wait 15 seconds. A timeout message appears.
6. Detect the Fn key. Nothing is reported, which is expected: the firmware handles Fn. The Fn key on the layout is dashed and explains why.
7. On Insert, **Add mapping** → *Remap key* → **Delete**, then **Save mapping**. The key shows a "not applied" badge.
8. Click **Apply changes**. A message says mappings are off and offers **Turn mappings on**. Click it.
9. In Notepad, press Insert. It deletes the character to the right. Hold it: it repeats, and stops when you release.
10. Back in the Studio, click **Test** on the mapping. It reports that the key press reached the engine.
11. Turn mappings off (sidebar toggle). Insert behaves normally again.

### Actions
12. Map keys to the following and check each in Notepad:
    - a **Shortcut** (Ctrl+A selects all);
    - **Type text** with two lines and an accented character;
    - **Open website** (`https://example.com`);
    - **Open app** (`notepad.exe`);
    - **Media** play/pause with music playing;
    - **Windows** → Task Manager and Settings → Sound.
13. **Script:** create `hello.ps1` containing `Start-Process notepad`. Choose it in a Script action. A confirmation dialog appears, and the badge shows "Confirmed". Pressing the key asks before running. Edit the file and apply again: KeyForge refuses until you confirm it again.
14. **Macro:** build "Ctrl+N, wait 300 ms, type Hello" in **Macro Builder**. Reorder the steps with the arrows, delete one, and add it back. Assign the macro to a key and run it in Notepad.

### Tap, hold and double tap
15. On one key, set the tap action to Media play/pause, the hold action to Task Manager and the double-tap action to Calculator. A quick tap toggles playback after a short pause, holding for ½ s opens Task Manager, and two quick taps open Calculator. Each runs once per press.

### Profiles and apps
16. Give **Coding** the shortcut Ctrl+Alt+2 and a mapping for the same key as in Productivity, but with a different action. Press Ctrl+Alt+2: a tooltip shows "KeyForge: Coding" and the key now does the Coding action. The dashboard shows Coding as active, and this survives a restart.
17. Add `notepad.exe` to Coding's automatic apps. With Productivity active, the key does the Coding action only while Notepad is in front.
18. In **Application Rules**, add `notepad.exe` → *Use app-specific mappings*, then add a mapping scoped to it. Notepad uses the override; other apps use the global mapping. Change the rule to *Turn mappings off*: global mappings stop in Notepad only.
19. Open an elevated app (for example Command Prompt → Run as administrator). Mappings do nothing while it is in front, as explained in Settings. Other apps are unaffected.

### Conflicts, preview and undo
20. Map the same key twice in the same scope. The editor warns, and **Apply** is refused with a clear error.
21. Try to map Backspace. It is not selectable.
22. Map both left and right Ctrl. Apply is refused.
23. Make three edits and press Ctrl+Z three times: all are undone. Ctrl+Y redoes them. **Preview** lists the changes and can show the generated engine script.

### Safety and recovery
24. With mappings on, press **Ctrl+Alt+Shift+Backspace**. All mappings turn off immediately and the dashboard says why.
25. With mappings on, end `AutoHotkey64.exe` in Task Manager. KeyForge reports that the engine stopped, mappings turn off, and no key is stuck.
26. With mappings on, end `keyforge.exe` in Task Manager. `AutoHotkey64.exe` disappears too, and all keys are normal.
27. Close the window. KeyForge stays in the tray. From the tray, use **Pause**, **Resume**, **Open settings** and **Exit**. Exit stops the engine.
28. Quit KeyForge, put `{ broken` into `%LOCALAPPDATA%\dev.keyforge.app\config.json`, and start it. A notice says the last working configuration was restored and names the saved damaged file.

### Configuration
29. In **Settings**:
    - **Export** writes a file, and **Import** of that file restores it;
    - importing a non-KeyForge JSON file is refused;
    - **Back up now** adds a backup, and **Restore** brings it back;
    - **Reset to defaults** asks for confirmation and makes a backup first.
30. Restart KeyForge. All mappings are still there and mappings start **off**, unless *Turn mappings on when KeyForge starts* is enabled.
