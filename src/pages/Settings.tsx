import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { Alert, Badge, Button, Card, Checkbox, Field, Input, Kbd, PageHeader } from "../components/ui";
import { api } from "../lib/api";
import type { BackupInfo } from "../lib/types";
import { useStore } from "../store";

export function Settings() {
  const { state, edit, run, applyOutcome, toast } = useStore();
  const draft = state.draft!;
  const status = state.status!;
  const s = draft.settings;
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [logs, setLogs] = useState<string[]>([]);
  const refresh = () => {
    api.listBackups().then(setBackups, () => {});
    api.logs().then(setLogs, () => {});
  };
  useEffect(refresh, []);
  const setSetting = <K extends keyof typeof s>(k: K, v: (typeof s)[K]) => edit((c) => (c.settings[k] = v));

  return (
    <>
      <PageHeader title="Settings" />
      <div className="grid gap-4 lg:grid-cols-2">
        <Card title="Behaviour">
          <div className="grid gap-4">
            <Field label="Hold threshold (ms)" hint="How long a key must be held for its hold action. 100–2000.">
              {(id) => <Input id={id} type="number" min={100} max={2000} step={10} value={s.tapHoldMs} onChange={(e) => setSetting("tapHoldMs", Number(e.target.value))} className="w-32" />}
            </Field>
            <Field label="Double-tap window (ms)" hint="Time allowed between two taps. 100–1000.">
              {(id) => <Input id={id} type="number" min={100} max={1000} step={10} value={s.doubleTapMs} onChange={(e) => setSetting("doubleTapMs", Number(e.target.value))} className="w-32" />}
            </Field>
            <Checkbox checked={s.closeToTray} onChange={(v) => setSetting("closeToTray", v)} label="Closing the window keeps KeyForge running in the tray" />
            <Checkbox checked={s.enableOnLaunch} onChange={(v) => setSetting("enableOnLaunch", v)} label="Turn mappings on when KeyForge starts (off by default)" />
            <p className="text-xs text-zinc-500">Settings take effect when you apply changes.</p>
          </div>
        </Card>

        <Card title="Safety">
          <div className="grid gap-3 text-sm text-zinc-300">
            <p>
              <Kbd>{status.emergencyShortcut}</Kbd> turns every mapping off immediately. Mappings cannot override it.{" "}
              {status.emergencyRegistered ? <Badge tone="emerald">backup registration active</Badge> : <Badge tone="amber">engine only</Badge>}
            </p>
            <p className="text-zinc-400">
              If the engine stops or crashes, Windows removes its keyboard hook and KeyForge releases any stuck modifier keys. If KeyForge itself closes or
              crashes, Windows ends the engine too. The tray menu can pause and resume mappings at any time.
            </p>
            <p className="text-zinc-400">
              KeyForge runs without administrator rights. Windows does not let it see or send keys while an app running as administrator is in front.
              {status.elevatedEngine && <b className="text-amber-200"> The engine is currently running as administrator.</b>}
            </p>
            <p className="text-zinc-400">
              Privacy: KeyForge never records, stores or sends keystrokes or clipboard contents. Key detection reads one key when you ask, then stops. Nothing
              leaves your PC.
            </p>
          </div>
        </Card>

        <Card title="Engine">
          <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
            <dt className="text-zinc-400">AutoHotkey</dt>
            <dd className="break-all">{status.ahkPath ?? "not found"}</dd>
            <dt className="text-zinc-400">Version</dt>
            <dd>{status.ahkVersion ?? "starts with the engine"}</dd>
            <dt className="text-zinc-400">State</dt>
            <dd>{status.engineRunning ? "running" : "stopped"}</dd>
          </dl>
          <div className="mt-4 grid gap-2">
            <Field label="Custom AutoHotkey v2 executable (optional)" hint="Leave empty to use the copy installed with KeyForge.">
              {(id) => (
                <div className="flex gap-2">
                  <Input id={id} className="flex-1" value={s.ahkPath ?? ""} onChange={(e) => setSetting("ahkPath", e.target.value || null)} />
                  <Button
                    onClick={async () => {
                      const p = await open({ filters: [{ name: "AutoHotkey", extensions: ["exe"] }] });
                      if (typeof p === "string") setSetting("ahkPath", p);
                    }}
                  >
                    Browse…
                  </Button>
                </div>
              )}
            </Field>
          </div>
        </Card>

        <Card title="Configuration and backups">
          <p className="mb-3 text-sm break-all text-zinc-400">Stored in {status.dataDir}</p>
          <div className="flex flex-wrap gap-2">
            <Button
              onClick={async () => {
                const path = await save({ defaultPath: "keyforge-config.json", filters: [{ name: "KeyForge configuration", extensions: ["json"] }] });
                if (path && (await run(() => api.exportConfig(path))) !== undefined) toast("ok", "Configuration exported.");
              }}
            >
              Export…
            </Button>
            <Button
              onClick={async () => {
                const path = await open({ filters: [{ name: "KeyForge configuration", extensions: ["json"] }] });
                if (typeof path !== "string") return;
                if (applyOutcome(await run(() => api.importConfig(path)), "Imported")) refresh();
              }}
            >
              Import…
            </Button>
            <Button
              onClick={async () => {
                const name = await run(() => api.backupNow());
                if (name) {
                  toast("ok", `Backup saved: ${name}`);
                  refresh();
                }
              }}
            >
              Back up now
            </Button>
            <Button
              variant="danger"
              onClick={async () => {
                if (await ask("Replace all profiles, mappings and settings with the defaults? A backup is made first.", { title: "Reset KeyForge", kind: "warning" })) {
                  if (applyOutcome(await run(() => api.resetConfig()), "Reset to defaults")) refresh();
                }
              }}
            >
              Reset to defaults
            </Button>
          </div>
          <h3 className="mt-4 mb-2 text-xs text-zinc-500">Backups (newest first, last 20 kept)</h3>
          {backups.length === 0 ? (
            <p className="text-sm text-zinc-500">No backups yet. One is made automatically before every import, restore and reset.</p>
          ) : (
            <ul className="grid max-h-48 gap-1 overflow-y-auto text-sm">
              {backups.map((b) => (
                <li key={b.name} className="flex items-center justify-between gap-2">
                  <span>
                    {new Date(b.created).toLocaleString()} <span className="text-xs text-zinc-500">{b.name.split("-").pop()?.replace(".json", "")}</span>
                  </span>
                  <Button
                    variant="ghost"
                    onClick={async () => {
                      if (await ask(`Restore the backup from ${new Date(b.created).toLocaleString()}? The current configuration is backed up first.`, { title: "Restore backup" })) {
                        if (applyOutcome(await run(() => api.restoreBackup(b.name)), "Backup restored")) refresh();
                      }
                    }}
                  >
                    Restore
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </Card>

        <Card title="Diagnostics" className="lg:col-span-2" aside={<Button variant="ghost" onClick={refresh}>Refresh</Button>}>
          {logs.length === 0 ? (
            <p className="text-sm text-zinc-500">No engine messages. Errors from actions (for example a program that could not be started) appear here.</p>
          ) : (
            <pre className="max-h-60 overflow-auto rounded-lg bg-zinc-950 p-3 text-xs whitespace-pre-wrap text-zinc-300">{logs.join("\n")}</pre>
          )}
          <div className="mt-3">
            <Alert tone="info">
              A mapping does nothing? Check that it is applied (no “not applied” badge), that mappings are on, and use Test in the Keyboard Studio. Apps running as
              administrator ignore mappings.
            </Alert>
          </div>
        </Card>
      </div>
    </>
  );
}
