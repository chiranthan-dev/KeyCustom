# Configuration file

KeyForge stores everything in `%LOCALAPPDATA%\dev.keyforge.app\`:

| File | Purpose |
|---|---|
| `config.json` | The current configuration (schema below). |
| `config.last-good.json` | The configuration that was in effect before the last change. |
| `config.corrupt-<time>.json` | A damaged `config.json` that KeyForge set aside during recovery. |
| `backups/config-<time>-<reason>.json` | Automatic backups (before import, restore and reset) and manual ones. The newest 20 are kept. |
| `engine/keyforge.ahk` | The generated engine script. It is rewritten on every apply; do not edit it. |
| `scripts/<sha256>.<ext>` | Private copies of confirmed scripts. Only these copies are ever run. |

Every write is atomic: KeyForge writes a temporary file, then renames it over the target.

At startup, KeyForge recovers from problems in this order:

1. If `config.json` is missing (first run), it writes the defaults.
2. If the file can't be parsed, fails validation, or comes from a newer schema version, it is renamed to `config.corrupt-<time>.json`.
3. KeyForge then loads `config.last-good.json`. If that is unusable too, it loads the defaults.
4. A notice explaining what happened is shown on the dashboard.

`cargo run -p keyforge-core --example generate -- path\to\config.json` prints the engine script for any configuration file. This is useful for checking what a file does.

## Schema (version 1)

Unknown fields are ignored. Optional fields can be left out. A complete example is in [`examples/sample-config.json`](examples/sample-config.json).

```jsonc
{
  "version": 1,                       // required; files from newer versions are refused
  "activeProfile": "productivity",    // profile id
  "settings": {
    "tapHoldMs": 200,                 // 100–2000: hold longer than this to run a hold action
    "doubleTapMs": 250,               // 100–1000: window for the second tap
    "closeToTray": true,
    "enableOnLaunch": false,          // opt-in; mappings start off by default
    "ahkPath": null                   // optional custom AutoHotkey64.exe
  },
  "profiles": [ /* 1–32 profiles */ ]
}
```

### Profile

| Field | Type | Notes |
|---|---|---|
| `id` | string | Unique. Letters, digits, `-` and `_`, up to 64 characters. |
| `name` | string | 1–64 characters. |
| `switchKey` | chord, optional | Global shortcut that activates this profile. Needs at least one modifier. |
| `autoApps` | string[] | Process names, e.g. `code.exe`. While one of them is in front, this profile applies instead of the active one. An app can belong to only one profile. |
| `mappings` | mapping[] | At most 500. |
| `macros` | macro[] | |
| `appRules` | app rule[] | |

### Chord

`{ "key": "F13", "ctrl": false, "shift": false, "alt": false, "win": false }`. The modifier flags are optional and default to false. Modifiers match either the left or the right key.

`key` takes one of two forms:

- An id from the key table (`crates/core/src/keys.rs`), for example `F1`–`F24`, `PrintScreen`, `Pause`, `Insert`, `ScrollLock`, `Home`, `PgUp`, `Left`, `RCtrl`, `CapsLock`, `AppsKey`, `A`–`Z`, `D0`–`D9`, `Semicolon`, `Numpad0`, `Media_Play_Pause` or `Volume_Up`.
- A raw code for keys without a name: `vkXXscYYY` (hex virtual-key code and scan code) or `scYYY` for keys that report only a scan code. Detect fills these in automatically.

### Mapping

| Field | Type | Notes |
|---|---|---|
| `id` | string | Unique within the profile; same character rules as profile ids. |
| `name` | string | Optional label. |
| `enabled` | bool | Default true. |
| `trigger` | chord | The key, plus any modifiers that must be held. |
| `action` | action | Runs on a press, or on a tap when `hold` or `doubleTap` is set. |
| `hold` | action, optional | Runs when the key is held longer than `holdMs` (or `settings.tapHoldMs`). |
| `doubleTap` | action, optional | Runs on two taps within `settings.doubleTapMs`. A single tap then waits for that window. |
| `holdMs` | number, optional | 100–2000. |
| `app` | string, optional | Process name. When set, the mapping applies only while that app is in front, and it overrides a global mapping with the same trigger. |
| `edited` | number | Unix milliseconds; used for "recently edited". |

### Actions

| `type` | Fields | Behaviour |
|---|---|---|
| `remap` | `to`: key id | Acts as a different key, including hold and auto-repeat. Modifiers you hold are passed through. With a modifier trigger, or with a `hold`/`doubleTap` set, it sends one press instead. |
| `shortcut` | `keys`: chord | Sends the combination once per press. Modifiers held on the trigger are released first. |
| `launch` | `path`, `args` | Starts a program, document or shortcut file. |
| `url` | `url` | `http://` or `https://` only. Opens in the default browser. |
| `text` | `text` | Types the text literally (up to 10 000 characters). |
| `macro` | `macro`: macro id | Runs a macro from the same profile. |
| `media` | `action`: `playPause` `next` `previous` `stop` `volumeUp` `volumeDown` `mute` | |
| `system` | `action`: see below | Opens a fixed target; nothing user-supplied is executed. |
| `script` | `path`, `sha256`, `confirmEachRun` | Runs `.ps1` (PowerShell), `.bat` or `.cmd`. `sha256` must match KeyForge's private copy, which is made when you confirm the script in the app. With `confirmEachRun`, every run asks first. |

`system` actions:

- **Settings pages:** `settingsHome` `display` `sound` `bluetooth` `network` `wifi` `personalization` `apps` `power` `windowsUpdate` `notifications` `privacy` `mouse`.
- **Utilities:** `taskManager` `calculator` `snippingTool` `fileExplorer` `notepad` `controlPanel`.
- **Windows:** `lockScreen` `showDesktop` `emojiPanel`.

### Macro

`{ "id": "standup", "name": "Stand-up note", "steps": [ … ] }`, with 1–200 steps and at most 300 seconds of total delay.

| Step `type` | Fields |
|---|---|
| `keys` | `keys`: chord |
| `text` | `text` |
| `delay` | `ms` (1–60 000) |
| `launch` | `path`, `args` |
| `url` | `url` |

Keys that a macro sends never trigger other mappings.

### App rule

`{ "id": "game", "process": "game.exe", "name": "", "mode": "override" | "disable", "enabled": true }`

- `override` lists the app so it can have app-specific mappings. Mappings use the app's process name in `mapping.app`.
- `disable` turns this profile's global mappings off while the app is in front. App-specific mappings still work.

## Validation rules

KeyForge rejects a configuration (and changes nothing) when any of these is true:

- **Unknown or reserved keys:**
  - unknown keys, or ids with invalid characters;
  - Backspace used as a trigger, or Ctrl+Alt+Shift+Backspace sent by an action (that is the emergency shortcut);
  - a remap, within one profile, of both the left and the right key of Ctrl, Shift or Alt (one of each must stay free for the emergency shortcut);
  - Win+L as a trigger (Windows reserves it).
- **Conflicts:**
  - two enabled mappings with the same trigger in the same scope;
  - a trigger equal to a profile shortcut, or a bare key that is part of one;
  - the same app listed in the automatic apps of two profiles.
- **Invalid fields:**
  - empty launch paths, URLs that are not http(s), empty text, missing macros, unconfirmed scripts, out-of-range timings or delays;
  - a modifier key used as a trigger together with other modifiers.

KeyForge warns, but still applies, when:

- a typing key (letters, digits, punctuation, Space, Enter, Tab) is remapped without Ctrl, Alt or Win;
- a remap targets a key that is itself remapped. Mappings never trigger each other, so chains do not cascade.
