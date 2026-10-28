// Typed wrappers around the backend commands and events. The UI never
// listens to the keyboard itself; key detection happens in the engine.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface Modifiers {
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
  win?: boolean;
}

export interface Mapping {
  id: string;
  enabled: boolean;
  trigger: { key: string } & Modifiers;
  action: { type: "remap"; to: string };
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
  elevatedEngine: boolean;
  lastError: string | null;
  emergencyShortcut: string;
  emergencyRegistered: boolean;
  mappings: Mapping[];
}

export interface DetectedKey extends Required<Modifiers> {
  id: string;
  name: string;
  vk: number;
  sc: number;
}

export interface Issue {
  mapping: string | null;
  message: string;
}

export interface ApplyResult {
  report: { errors: Issue[]; warnings: Issue[] };
  status: Status;
}

export const api = {
  status: () => invoke<Status>("get_status"),
  keys: () => invoke<KeyDef[]>("list_keys"),
  apply: (mappings: Mapping[]) => invoke<ApplyResult>("apply_mappings", { mappings }),
  setEnabled: (enabled: boolean) => invoke<Status>("set_enabled", { enabled }),
  startDetect: () => invoke<Status>("start_detect"),
  cancelDetect: () => invoke<void>("cancel_detect"),
};

export function onEvent<T>(name: "status" | "key-detected" | "detect-ended" | "log", handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(`kf://${name}`, (e) => handler(e.payload));
}

export const hex = (n: number, width = 2) => "0x" + n.toString(16).toUpperCase().padStart(width, "0");

export function triggerLabel(t: Mapping["trigger"]): string {
  return [t.ctrl && "Ctrl", t.shift && "Shift", t.alt && "Alt", t.win && "Win", t.key].filter(Boolean).join(" + ");
}
