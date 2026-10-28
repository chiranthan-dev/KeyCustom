// Typed wrappers around backend commands and events. The UI never listens to
// the keyboard globally; detection happens in the engine.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ApplyOutcome, BackupInfo, Config, DetectedKey, KeyDef, Preview, ScriptInfo, Snapshot, Status } from "./types";

export const api = {
  snapshot: () => invoke<Snapshot>("get_snapshot"),
  keys: () => invoke<KeyDef[]>("list_keys"),
  preview: (config: Config) => invoke<Preview>("preview_config", { config }),
  apply: (config: Config) => invoke<ApplyOutcome>("apply_config", { config }),
  setEnabled: (enabled: boolean) => invoke<Status>("set_enabled", { enabled }),
  setProfile: (id: string) => invoke<Status>("set_profile", { id }),
  startDetect: () => invoke<Status>("start_detect"),
  cancelDetect: () => invoke<void>("cancel_detect"),
  testMapping: (profile: string, mapping: string) => invoke<string>("test_mapping", { profile, mapping }),
  confirmScript: (path: string) => invoke<ScriptInfo>("confirm_script", { path }),
  importConfig: (path: string) => invoke<ApplyOutcome>("import_config", { path }),
  exportConfig: (path: string) => invoke<void>("export_config", { path }),
  backupNow: () => invoke<string>("backup_now"),
  listBackups: () => invoke<BackupInfo[]>("list_backups"),
  restoreBackup: (name: string) => invoke<ApplyOutcome>("restore_backup", { name }),
  resetConfig: () => invoke<ApplyOutcome>("reset_config"),
  logs: () => invoke<string[]>("get_logs"),
  dismissNotice: () => invoke<void>("dismiss_notice"),
};

export interface Events {
  status: Status;
  "key-detected": DetectedKey;
  "detect-ended": string;
  fired: { profile: string; mapping: string };
  profile: string;
  log: string;
}

export function onEvent<K extends keyof Events>(name: K, handler: (payload: Events[K]) => void): Promise<UnlistenFn> {
  return listen<Events[K]>(`kf://${name}`, (e) => handler(e.payload));
}

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}
