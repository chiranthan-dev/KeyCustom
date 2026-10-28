// Mirrors the Rust configuration schema (crates/core/src/config.rs).

export interface Modifiers {
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
  win?: boolean;
}

export interface Chord extends Modifiers {
  key: string;
}

export type MediaAction = "playPause" | "next" | "previous" | "stop" | "volumeUp" | "volumeDown" | "mute";

export type SystemAction =
  | "settingsHome" | "display" | "sound" | "bluetooth" | "network" | "wifi" | "personalization" | "apps"
  | "power" | "windowsUpdate" | "notifications" | "privacy" | "mouse" | "taskManager" | "calculator"
  | "snippingTool" | "fileExplorer" | "notepad" | "controlPanel" | "lockScreen" | "showDesktop" | "emojiPanel";

export type Action =
  | { type: "remap"; to: string }
  | { type: "shortcut"; keys: Chord }
  | { type: "launch"; path: string; args?: string }
  | { type: "url"; url: string }
  | { type: "text"; text: string }
  | { type: "macro"; macro: string }
  | { type: "media"; action: MediaAction }
  | { type: "system"; action: SystemAction }
  | { type: "script"; path: string; sha256: string; confirmEachRun: boolean };

export type ActionType = Action["type"];

export type Step =
  | { type: "keys"; keys: Chord }
  | { type: "text"; text: string }
  | { type: "delay"; ms: number }
  | { type: "launch"; path: string; args?: string }
  | { type: "url"; url: string };

export interface Mapping {
  id: string;
  name?: string;
  enabled: boolean;
  trigger: Chord;
  action: Action;
  hold?: Action | null;
  doubleTap?: Action | null;
  holdMs?: number | null;
  app?: string | null;
  edited?: number;
}

export interface Macro {
  id: string;
  name: string;
  steps: Step[];
}

export interface AppRule {
  id: string;
  process: string;
  name?: string;
  mode: "override" | "disable";
  enabled: boolean;
}

export interface Profile {
  id: string;
  name: string;
  switchKey?: Chord | null;
  autoApps: string[];
  mappings: Mapping[];
  macros: Macro[];
  appRules: AppRule[];
}

export interface Settings {
  tapHoldMs: number;
  doubleTapMs: number;
  closeToTray: boolean;
  enableOnLaunch: boolean;
  ahkPath?: string | null;
}

export interface Config {
  version: number;
  activeProfile: string;
  settings: Settings;
  profiles: Profile[];
}

export interface KeyDef {
  id: string;
  label: string;
  vk: number;
  sc: number;
  group: string;
}

export interface Status {
  ahkPath: string | null;
  ahkVersion: string | null;
  engineRunning: boolean;
  enabled: boolean;
  detecting: boolean;
  activeProfile: string;
  elevatedEngine: boolean;
  lastError: string | null;
  notice: string | null;
  emergencyShortcut: string;
  emergencyRegistered: boolean;
  dataDir: string;
}

export interface Issue {
  profile: string | null;
  mapping: string | null;
  message: string;
}

export interface Report {
  errors: Issue[];
  warnings: Issue[];
}

export interface Snapshot {
  config: Config;
  status: Status;
}

export interface ApplyOutcome {
  report: Report;
  applied: boolean;
  config: Config;
  status: Status;
}

export interface Preview {
  report: Report;
  script: string | null;
}

export interface DetectedKey {
  id: string;
  name: string;
  vk: number;
  sc: number;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  win: boolean;
}

export interface BackupInfo {
  name: string;
  created: number;
  size: number;
}

export interface ScriptInfo {
  path: string;
  sha256: string;
  size: number;
}
