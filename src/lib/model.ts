// Pure helpers for the configuration model. Unit tested in model.test.ts.
import type { Action, ActionType, Chord, Config, KeyDef, Mapping, MediaAction, Profile, Step, SystemAction } from "./types";

export const MOD_NAMES = ["ctrl", "shift", "alt", "win"] as const;

export function newId(prefix: string): string {
  const rand = Math.random().toString(36).slice(2, 8);
  return `${prefix}-${Date.now().toString(36)}${rand}`;
}

export function slug(name: string, taken: string[]): string {
  const base = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40) || "profile";
  let id = base;
  for (let i = 2; taken.includes(id); i++) id = `${base}-${i}`;
  return id;
}

export const ACTION_TYPES: { type: ActionType; label: string; hint: string }[] = [
  { type: "remap", label: "Remap key", hint: "Act as a different key" },
  { type: "shortcut", label: "Shortcut", hint: "Send a combination like Ctrl+Shift+S" },
  { type: "launch", label: "Open app", hint: "Start a program or file" },
  { type: "url", label: "Open website", hint: "Open a URL in the default browser" },
  { type: "text", label: "Type text", hint: "Insert a saved snippet" },
  { type: "macro", label: "Macro", hint: "Run a sequence of steps" },
  { type: "media", label: "Media", hint: "Play/pause, tracks, volume" },
  { type: "system", label: "Windows", hint: "Settings pages and utilities" },
  { type: "script", label: "Script", hint: "Run a confirmed .ps1/.bat/.cmd file" },
];

export const MEDIA_LABELS: Record<MediaAction, string> = {
  playPause: "Play / pause",
  next: "Next track",
  previous: "Previous track",
  stop: "Stop",
  volumeUp: "Volume up",
  volumeDown: "Volume down",
  mute: "Mute",
};

export const SYSTEM_GROUPS: { label: string; items: [SystemAction, string][] }[] = [
  {
    label: "Settings pages",
    items: [
      ["settingsHome", "Settings home"],
      ["display", "Display"],
      ["sound", "Sound"],
      ["bluetooth", "Bluetooth & devices"],
      ["network", "Network status"],
      ["wifi", "Wi-Fi"],
      ["personalization", "Personalization"],
      ["apps", "Installed apps"],
      ["power", "Power & sleep"],
      ["windowsUpdate", "Windows Update"],
      ["notifications", "Notifications"],
      ["privacy", "Privacy & security"],
      ["mouse", "Mouse & touchpad"],
    ],
  },
  {
    label: "Utilities",
    items: [
      ["taskManager", "Task Manager"],
      ["calculator", "Calculator"],
      ["snippingTool", "Screen snip"],
      ["fileExplorer", "File Explorer"],
      ["notepad", "Notepad"],
      ["controlPanel", "Control Panel"],
    ],
  },
  {
    label: "Windows",
    items: [
      ["lockScreen", "Lock the PC"],
      ["showDesktop", "Show desktop"],
      ["emojiPanel", "Emoji panel"],
    ],
  },
];

export const SYSTEM_LABELS: Record<string, string> = Object.fromEntries(SYSTEM_GROUPS.flatMap((g) => g.items));

export function defaultAction(type: ActionType, profile?: Profile): Action {
  switch (type) {
    case "remap":
      return { type, to: "F13" };
    case "shortcut":
      return { type, keys: { key: "S", ctrl: true } };
    case "launch":
      return { type, path: "", args: "" };
    case "url":
      return { type, url: "https://" };
    case "text":
      return { type, text: "" };
    case "macro":
      return { type, macro: profile?.macros[0]?.id ?? "" };
    case "media":
      return { type, action: "playPause" };
    case "system":
      return { type, action: "taskManager" };
    case "script":
      return { type, path: "", sha256: "", confirmEachRun: true };
  }
}

export function keyLabel(id: string, keys: KeyDef[]): string {
  return keys.find((k) => k.id.toLowerCase() === id.toLowerCase())?.label ?? id;
}

export function chordLabel(chord: Chord, keys: KeyDef[] = []): string {
  const parts: string[] = [];
  if (chord.ctrl) parts.push("Ctrl");
  if (chord.shift) parts.push("Shift");
  if (chord.alt) parts.push("Alt");
  if (chord.win) parts.push("Win");
  parts.push(keyLabel(chord.key, keys));
  return parts.join(" + ");
}

function basename(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

export function describeAction(action: Action, keys: KeyDef[] = [], profile?: Profile): string {
  switch (action.type) {
    case "remap":
      return `Acts as ${keyLabel(action.to, keys)}`;
    case "shortcut":
      return `Sends ${chordLabel(action.keys, keys)}`;
    case "launch":
      return action.path ? `Opens ${basename(action.path)}` : "Opens an app";
    case "url":
      return `Opens ${action.url.replace(/^https?:\/\//, "")}`;
    case "text": {
      const t = action.text.replace(/\s+/g, " ");
      return `Types "${t.length > 30 ? t.slice(0, 30) + "…" : t}"`;
    }
    case "macro":
      return `Runs macro ${profile?.macros.find((m) => m.id === action.macro)?.name ?? action.macro}`;
    case "media":
      return MEDIA_LABELS[action.action];
    case "system":
      return SYSTEM_LABELS[action.action] ?? action.action;
    case "script":
      return action.path ? `Runs script ${basename(action.path)}` : "Runs a script";
  }
}

export function describeMapping(m: Mapping, keys: KeyDef[] = [], profile?: Profile): string {
  let s = describeAction(m.action, keys, profile);
  if (m.hold) s += ` · hold: ${describeAction(m.hold, keys, profile)}`;
  if (m.doubleTap) s += ` · double tap: ${describeAction(m.doubleTap, keys, profile)}`;
  return s;
}

export function describeStep(step: Step, keys: KeyDef[] = []): string {
  switch (step.type) {
    case "keys":
      return `Press ${chordLabel(step.keys, keys)}`;
    case "text":
      return `Type "${step.text.length > 30 ? step.text.slice(0, 30) + "…" : step.text}"`;
    case "delay":
      return `Wait ${step.ms} ms`;
    case "launch":
      return step.path ? `Open ${basename(step.path)}` : "Open an app";
    case "url":
      return `Open ${step.url}`;
  }
}

export function sameTrigger(a: Chord, b: Chord): boolean {
  return a.key.toLowerCase() === b.key.toLowerCase() && MOD_NAMES.every((m) => !!a[m] === !!b[m]);
}

/** Human-readable list of differences between the saved and edited config. */
export function diffConfigs(before: Config, after: Config, keys: KeyDef[] = []): string[] {
  const out: string[] = [];
  const byId = <T extends { id: string }>(xs: T[]) => new Map(xs.map((x) => [x.id, x]));
  const prevProfiles = byId(before.profiles);
  const nextProfiles = byId(after.profiles);
  for (const p of after.profiles) {
    const old = prevProfiles.get(p.id);
    if (!old) {
      out.push(`Add profile "${p.name}"`);
      continue;
    }
    if (old.name !== p.name) out.push(`Rename profile "${old.name}" to "${p.name}"`);
    if (JSON.stringify(old.switchKey ?? null) !== JSON.stringify(p.switchKey ?? null))
      out.push(`${p.name}: shortcut ${p.switchKey ? chordLabel(p.switchKey, keys) : "removed"}`);
    if (JSON.stringify(old.autoApps) !== JSON.stringify(p.autoApps)) out.push(`${p.name}: automatic apps changed`);
    const oldMaps = byId(old.mappings);
    const newMaps = byId(p.mappings);
    for (const m of p.mappings) {
      const prev = oldMaps.get(m.id);
      const where = `${p.name}: ${chordLabel(m.trigger, keys)}${m.app ? ` in ${m.app}` : ""}`;
      if (!prev) out.push(`${where} → ${describeMapping(m, keys, p)} (new)`);
      else if (JSON.stringify({ ...prev, edited: 0 }) !== JSON.stringify({ ...m, edited: 0 })) {
        if (prev.enabled !== m.enabled && JSON.stringify({ ...prev, enabled: m.enabled, edited: 0 }) === JSON.stringify({ ...m, edited: 0 }))
          out.push(`${where} ${m.enabled ? "enabled" : "disabled"}`);
        else out.push(`${where} → ${describeMapping(m, keys, p)} (changed)`);
      }
    }
    for (const m of old.mappings)
      if (!newMaps.has(m.id)) out.push(`${p.name}: ${chordLabel(m.trigger, keys)}${m.app ? ` in ${m.app}` : ""} restored to normal`);
    const oldMacros = byId(old.macros);
    for (const m of p.macros) {
      const prev = oldMacros.get(m.id);
      if (!prev) out.push(`${p.name}: new macro "${m.name}"`);
      else if (JSON.stringify(prev) !== JSON.stringify(m)) out.push(`${p.name}: macro "${m.name}" changed`);
    }
    for (const m of old.macros) if (!p.macros.some((x) => x.id === m.id)) out.push(`${p.name}: macro "${m.name}" deleted`);
    if (JSON.stringify(old.appRules) !== JSON.stringify(p.appRules)) out.push(`${p.name}: application rules changed`);
  }
  for (const p of before.profiles) if (!nextProfiles.has(p.id)) out.push(`Delete profile "${p.name}"`);
  if (JSON.stringify(before.settings) !== JSON.stringify(after.settings)) out.push("Settings changed");
  return out;
}

export function isValidProcess(name: string): boolean {
  return /^[^\\/:*?"<>|\x00-\x1f]+\.exe$/i.test(name.trim()) && name.trim().length <= 260;
}
