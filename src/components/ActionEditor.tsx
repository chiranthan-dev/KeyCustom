import { ask, open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { api, errorText } from "../lib/api";
import { ACTION_TYPES, MEDIA_LABELS, SYSTEM_GROUPS, defaultAction } from "../lib/model";
import type { Action, ActionType, MediaAction, Profile, SystemAction } from "../lib/types";
import { useStore } from "../store";
import { ChordPicker, KeySelect } from "./pickers";
import { Badge, Button, Checkbox, Field, Input, Select, TextArea } from "./ui";

export async function browseFile(filters: { name: string; extensions: string[] }[]): Promise<string | null> {
  const picked = await open({ multiple: false, directory: false, filters });
  return typeof picked === "string" ? picked : null;
}

export function ActionEditor({ value, onChange, profile, label = "Action" }: { value: Action; onChange: (a: Action) => void; profile: Profile; label?: string }) {
  const { dispatch } = useStore();
  return (
    <div className="grid gap-3">
      <Field label={label}>
        {(id) => (
          <Select id={id} value={value.type} onChange={(e) => onChange(defaultAction(e.target.value as ActionType, profile))}>
            {ACTION_TYPES.map((t) => (
              <option key={t.type} value={t.type}>
                {t.label} — {t.hint}
              </option>
            ))}
          </Select>
        )}
      </Field>
      {value.type === "remap" && (
        <Field label="Acts as this key" hint="Holds and repeats like the real key. Modifiers you hold are passed through.">
          {(id) => <KeySelect id={id} value={value.to} onChange={(to) => onChange({ ...value, to })} />}
        </Field>
      )}
      {value.type === "shortcut" && <ChordPicker label="Shortcut to send" value={value.keys} onChange={(keys) => onChange({ ...value, keys })} />}
      {value.type === "launch" && (
        <>
          <Field label="Program, document or shortcut">
            {(id) => (
              <div className="flex gap-2">
                <Input id={id} className="flex-1" value={value.path} placeholder="C:\Program Files\App\app.exe" onChange={(e) => onChange({ ...value, path: e.target.value })} />
                <Button
                  onClick={async () => {
                    const path = await browseFile([{ name: "Programs", extensions: ["exe", "lnk", "bat", "cmd", "msc"] }, { name: "All files", extensions: ["*"] }]);
                    if (path) onChange({ ...value, path });
                  }}
                >
                  Browse…
                </Button>
              </div>
            )}
          </Field>
          <Field label="Arguments (optional)">{(id) => <Input id={id} value={value.args ?? ""} onChange={(e) => onChange({ ...value, args: e.target.value })} />}</Field>
        </>
      )}
      {value.type === "url" && (
        <Field label="Web address" hint="Opens in your default browser. Must start with https:// or http://.">
          {(id) => <Input id={id} value={value.url} onChange={(e) => onChange({ ...value, url: e.target.value.trim() })} />}
        </Field>
      )}
      {value.type === "text" && (
        <Field label="Text to type" hint="Typed exactly as written, including line breaks. Stored in your configuration file, so do not save passwords here.">
          {(id) => <TextArea id={id} value={value.text} onChange={(e) => onChange({ ...value, text: e.target.value })} />}
        </Field>
      )}
      {value.type === "macro" && (
        <Field label="Macro">
          {(id) =>
            profile.macros.length === 0 ? (
              <div className="flex items-center gap-2 text-sm text-zinc-400">
                This profile has no macros yet.
                <Button onClick={() => dispatch({ t: "nav", page: "macros", profileId: profile.id })}>Create one</Button>
              </div>
            ) : (
              <Select id={id} value={value.macro} onChange={(e) => onChange({ ...value, macro: e.target.value })}>
                {!profile.macros.some((m) => m.id === value.macro) && <option value={value.macro}>Choose a macro</option>}
                {profile.macros.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.name} ({m.steps.length} steps)
                  </option>
                ))}
              </Select>
            )
          }
        </Field>
      )}
      {value.type === "media" && (
        <Field label="Media control">
          {(id) => (
            <Select id={id} value={value.action} onChange={(e) => onChange({ ...value, action: e.target.value as MediaAction })}>
              {Object.entries(MEDIA_LABELS).map(([k, v]) => (
                <option key={k} value={k}>
                  {v}
                </option>
              ))}
            </Select>
          )}
        </Field>
      )}
      {value.type === "system" && (
        <Field label="Windows action">
          {(id) => (
            <Select id={id} value={value.action} onChange={(e) => onChange({ ...value, action: e.target.value as SystemAction })}>
              {SYSTEM_GROUPS.map((g) => (
                <optgroup key={g.label} label={g.label}>
                  {g.items.map(([k, v]) => (
                    <option key={k} value={k}>
                      {v}
                    </option>
                  ))}
                </optgroup>
              ))}
            </Select>
          )}
        </Field>
      )}
      {value.type === "script" && <ScriptEditor value={value} onChange={onChange} />}
    </div>
  );
}

function ScriptEditor({ value, onChange }: { value: Extract<Action, { type: "script" }>; onChange: (a: Action) => void }) {
  const [error, setError] = useState<string | null>(null);
  const confirmed = value.sha256.length === 64;
  async function confirm(path: string) {
    setError(null);
    const yes = await ask(
      `KeyForge will run this script when the key is pressed:\n\n${path}\n\nOnly confirm scripts you wrote or trust. KeyForge stores a private copy and runs that copy; if you change the file later, confirm it again.`,
      { title: "Confirm script", kind: "warning", okLabel: "Trust this script", cancelLabel: "Cancel" },
    );
    if (!yes) return;
    try {
      const info = await api.confirmScript(path);
      onChange({ ...value, path, sha256: info.sha256 });
    } catch (e) {
      setError(errorText(e));
    }
  }
  return (
    <div className="grid gap-3">
      <Field label="Script file (.ps1, .bat or .cmd)">
        {(id) => (
          <div className="flex gap-2">
            <Input id={id} className="flex-1" value={value.path} readOnly placeholder="Choose a script" />
            <Button
              onClick={async () => {
                const path = await browseFile([{ name: "Scripts", extensions: ["ps1", "bat", "cmd"] }]);
                if (path) {
                  onChange({ ...value, path, sha256: "" });
                  await confirm(path);
                }
              }}
            >
              Browse…
            </Button>
          </div>
        )}
      </Field>
      <div className="flex flex-wrap items-center gap-2 text-sm">
        {confirmed ? <Badge tone="emerald">Confirmed · {value.sha256.slice(0, 12)}…</Badge> : <Badge tone="amber">Not confirmed</Badge>}
        {value.path && <Button onClick={() => confirm(value.path)}>{confirmed ? "Re-confirm" : "Confirm"}</Button>}
      </div>
      <Checkbox checked={value.confirmEachRun} onChange={(confirmEachRun) => onChange({ ...value, confirmEachRun })} label="Ask before every run" />
      {error && <p className="text-sm text-red-300">{error}</p>}
    </div>
  );
}
