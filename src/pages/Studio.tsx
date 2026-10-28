import { useMemo, useState } from "react";
import { ActionEditor } from "../components/ActionEditor";
import { Alert, Badge, Button, Card, Checkbox, Empty, Field, Input, Kbd, PageHeader, Select, Toggle } from "../components/ui";
import { api, errorText } from "../lib/api";
import { EXTRA_GROUPS, MAIN_ROWS, type LayoutKey } from "../lib/layout";
import { MOD_NAMES, chordLabel, defaultAction, describeAction, describeMapping, keyLabel, newId, sameTrigger } from "../lib/model";
import type { DetectedKey, KeyDef, Mapping, Profile } from "../lib/types";
import { useEditedProfile, useStore } from "../store";

const hex = (n: number, w = 2) => "0x" + n.toString(16).toUpperCase().padStart(w, "0");

export function Studio() {
  const { state, dispatch, detect } = useStore();
  const profile = useEditedProfile();
  const [detected, setDetected] = useState<DetectedKey | null>(null);
  const [detecting, setDetecting] = useState(false);
  const [query, setQuery] = useState("");
  const selected = state.selectedKey;
  const scope = state.scope;
  const select = (id: string | null) => dispatch({ t: "nav", page: "studio", selectedKey: id });

  const mappedIn = (id: string) => profile.mappings.filter((m) => m.trigger.key.toLowerCase() === id.toLowerCase());
  const extraIds = useMemo(() => {
    const layoutIds = new Set([...MAIN_ROWS.flat().map((k) => k.id), ...EXTRA_GROUPS.flatMap((g) => g.ids)].map((s) => s.toLowerCase()));
    const ids = new Set(profile.mappings.map((m) => m.trigger.key).filter((id) => !layoutIds.has(id.toLowerCase())));
    if (detected && !layoutIds.has(detected.id.toLowerCase())) ids.add(detected.id);
    return [...ids];
  }, [profile, detected]);

  const results = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return [];
    const all: { id: string; label: string }[] = [...state.keys.map((k) => ({ id: k.id, label: k.label })), ...extraIds.map((id) => ({ id, label: id }))];
    return all.filter((k) => k.id.toLowerCase().includes(q) || k.label.toLowerCase().includes(q)).slice(0, 30);
  }, [query, state.keys, extraIds]);

  async function runDetect() {
    setDetecting(true);
    const k = await detect();
    setDetecting(false);
    if (k) {
      setDetected(k);
      select(k.id);
    }
  }

  const keyButton = (k: LayoutKey) => {
    const maps = k.disabled ? [] : mappedIn(k.id);
    const inScope = maps.filter((m) => (m.app ?? "") === scope);
    const enabled = inScope.some((m) => m.enabled);
    const isSel = selected?.toLowerCase() === k.id.toLowerCase();
    const label = k.label ?? keyLabel(k.id, state.keys);
    return (
      <button
        key={k.id}
        type="button"
        disabled={!!k.disabled}
        title={k.disabled ?? (inScope.length ? `${label}: ${inScope.map((m) => describeMapping(m, state.keys, profile)).join("; ")}` : label)}
        aria-label={`${label}${inScope.length ? `, mapped (${inScope.length})` : ""}`}
        aria-pressed={isSel}
        onClick={() => select(k.id)}
        style={{ flexGrow: k.w ?? 1, flexBasis: 0 }}
        className={`relative h-11 min-w-0 truncate rounded-md border px-1 text-xs transition-colors disabled:cursor-not-allowed disabled:border-dashed disabled:opacity-40 ${
          isSel
            ? "border-sky-400 bg-sky-500/20 text-sky-100"
            : enabled
              ? "border-sky-700 bg-sky-950 text-sky-200 hover:border-sky-500"
              : inScope.length
                ? "border-zinc-600 bg-zinc-800/60 text-zinc-400"
                : "border-zinc-800 bg-zinc-900 text-zinc-300 hover:border-zinc-600"
        }`}
      >
        {label}
        {inScope.length > 0 && <span aria-hidden className={`absolute top-1 right-1 h-1.5 w-1.5 rounded-full ${enabled ? "bg-sky-400" : "bg-zinc-500"}`} />}
        {maps.length > inScope.length && <span aria-hidden className="absolute top-1 left-1 h-1.5 w-1.5 rounded-full bg-amber-400" />}
      </button>
    );
  };

  return (
    <>
      <PageHeader title="Keyboard Studio" description="Click a key, or press Detect and then the key, to choose what it does.">
        <ProfileScope profile={profile} />
        <Button variant="primary" disabled={detecting || !state.status?.ahkPath} onClick={runDetect}>
          {detecting ? "Press any key…" : "Detect key"}
        </Button>
        {detecting && <Button onClick={() => api.cancelDetect()}>Cancel</Button>}
      </PageHeader>

      <Card>
        <div className="grid gap-1.5" role="group" aria-label="Keyboard">
          {MAIN_ROWS.map((row, i) => (
            <div key={i} className="flex gap-1.5">
              {row.map(keyButton)}
            </div>
          ))}
        </div>
        <div className="mt-3 flex flex-wrap gap-x-5 gap-y-1 text-xs text-zinc-500">
          <span><span className="mr-1 inline-block h-2 w-2 rounded-full bg-sky-400" />Mapped here</span>
          <span><span className="mr-1 inline-block h-2 w-2 rounded-full bg-zinc-500" />Mapped, turned off</span>
          <span><span className="mr-1 inline-block h-2 w-2 rounded-full bg-amber-400" />Mapped in another scope</span>
          <span>Dashed keys cannot be mapped (hover for why).</span>
        </div>
      </Card>

      <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.3fr)]">
        <div className="grid content-start gap-4">
          {detected && (
            <Card title="Detected key" aside={<Button variant="ghost" onClick={() => setDetected(null)}>Clear</Button>}>
              <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm" aria-live="polite">
                <dt className="text-zinc-400">Name</dt>
                <dd><Kbd>{detected.name || detected.id}</Kbd> <span className="text-zinc-500">({detected.id})</span></dd>
                <dt className="text-zinc-400">Virtual key</dt>
                <dd className="font-mono">{hex(detected.vk)}</dd>
                <dt className="text-zinc-400">Scan code</dt>
                <dd className="font-mono">{hex(detected.sc, 3)}</dd>
                <dt className="text-zinc-400">Modifiers held</dt>
                <dd>{MOD_NAMES.filter((m) => detected[m]).map((m) => m[0].toUpperCase() + m.slice(1)).join(" + ") || "none"}</dd>
              </dl>
              {(detected.shift || detected.win || detected.ctrl || detected.alt) && (
                <p className="mt-3 text-xs text-zinc-400">
                  This key arrived together with modifiers. Some laptop keys (such as the Copilot key, which sends Win+Shift+F23) do this in firmware. New
                  mappings for it include those modifiers automatically.
                </p>
              )}
            </Card>
          )}
          <Card title="Find a key">
            <Input aria-label="Search keys" placeholder="Search: F13, Pause, media…" value={query} onChange={(e) => setQuery(e.target.value)} className="w-full" />
            {results.length > 0 && (
              <ul className="mt-2 grid max-h-56 gap-0.5 overflow-y-auto">
                {results.map((k) => (
                  <li key={k.id}>
                    <button type="button" className="flex w-full items-center justify-between rounded px-2 py-1 text-left text-sm hover:bg-zinc-800" onClick={() => select(k.id)}>
                      <span>{k.label}</span>
                      {mappedIn(k.id).length > 0 && <Badge tone="sky">{mappedIn(k.id).length} mapped</Badge>}
                    </button>
                  </li>
                ))}
              </ul>
            )}
            <div className="mt-4 grid gap-3">
              {[...EXTRA_GROUPS, ...(extraIds.length ? [{ label: "Detected and other keys", ids: extraIds }] : [])].map((g) => (
                <div key={g.label}>
                  <h3 className="mb-1.5 text-xs text-zinc-500">{g.label}</h3>
                  <div className="flex flex-wrap gap-1.5">
                    {g.ids.map((id) => {
                      const n = mappedIn(id).filter((m) => (m.app ?? "") === scope).length;
                      const sel = selected?.toLowerCase() === id.toLowerCase();
                      return (
                        <button
                          key={id}
                          type="button"
                          aria-pressed={sel}
                          onClick={() => select(id)}
                          className={`rounded-md border px-2 py-1 text-xs ${sel ? "border-sky-400 bg-sky-500/20" : n ? "border-sky-700 bg-sky-950 text-sky-200" : "border-zinc-800 bg-zinc-900 text-zinc-300 hover:border-zinc-600"}`}
                        >
                          {keyLabel(id, state.keys)}
                        </button>
                      );
                    })}
                  </div>
                </div>
              ))}
            </div>
          </Card>
          <Alert tone="info">
            The Fn key and most brightness, Wi-Fi and vendor keys are handled by the laptop's firmware or drivers and never reach Windows. If Detect shows nothing
            for a key, no Windows software can remap it.
          </Alert>
        </div>
        {selected ? <KeyPanel keyId={selected} profile={profile} detected={detected?.id === selected ? detected : null} /> : <Card><Empty>Select a key to see and edit what it does.</Empty></Card>}
      </div>
    </>
  );
}

function ProfileScope({ profile }: { profile: Profile }) {
  const { state, dispatch } = useStore();
  const draft = state.draft!;
  return (
    <>
      <Select aria-label="Profile to edit" value={profile.id} onChange={(e) => dispatch({ t: "nav", page: state.page, profileId: e.target.value, scope: "" })}>
        {draft.profiles.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name}
            {p.id === draft.activeProfile ? " (active)" : ""}
          </option>
        ))}
      </Select>
      <Select aria-label="Scope" value={state.scope} onChange={(e) => dispatch({ t: "nav", page: state.page, scope: e.target.value })}>
        <option value="">All apps (global)</option>
        {profile.appRules.map((r) => (
          <option key={r.id} value={r.process.toLowerCase()}>
            Only in {r.name || r.process}
          </option>
        ))}
      </Select>
    </>
  );
}

function KeyPanel({ keyId, profile, detected }: { keyId: string; profile: Profile; detected: DetectedKey | null }) {
  const { state, edit } = useStore();
  const def: KeyDef | undefined = state.keys.find((k) => k.id.toLowerCase() === keyId.toLowerCase());
  const maps = profile.mappings.filter((m) => m.trigger.key.toLowerCase() === keyId.toLowerCase());
  const [editing, setEditing] = useState<Mapping | null>(null);

  const startNew = () => {
    setEditing({
      id: newId("m"),
      enabled: true,
      trigger: { key: def?.id ?? keyId, ctrl: !!detected?.ctrl, shift: !!detected?.shift, alt: !!detected?.alt, win: !!detected?.win },
      action: defaultAction("shortcut", profile),
      app: state.scope || null,
    });
  };

  const save = (m: Mapping) => {
    edit((c) => {
      const p = c.profiles.find((x) => x.id === profile.id)!;
      const i = p.mappings.findIndex((x) => x.id === m.id);
      const next = { ...m, edited: Date.now() };
      if (i >= 0) p.mappings[i] = next;
      else p.mappings.push(next);
    });
    setEditing(null);
  };

  const remove = (ids: string[]) =>
    edit((c) => {
      const p = c.profiles.find((x) => x.id === profile.id)!;
      p.mappings = p.mappings.filter((m) => !ids.includes(m.id));
    });

  return (
    <Card
      title={`${def?.label ?? keyId}${def ? ` · VK ${hex(def.vk)} · SC ${hex(def.sc, 3)}` : ""}`}
      aside={
        maps.length > 0 && (
          <Button variant="ghost" onClick={() => remove(maps.map((m) => m.id))} title="Remove every mapping of this key in this profile">
            Restore original
          </Button>
        )
      }
    >
      {maps.length === 0 && !editing && <Empty>This key works normally in profile “{profile.name}”.</Empty>}
      <ul className="grid gap-2">
        {maps.map((m) =>
          editing?.id === m.id ? null : (
            <MappingRow key={m.id} m={m} profile={profile} onEdit={() => setEditing(structuredClone(m))} onRemove={() => remove([m.id])} />
          ),
        )}
      </ul>
      {editing ? (
        <MappingEditor key={editing.id} initial={editing} profile={profile} onSave={save} onCancel={() => setEditing(null)} />
      ) : (
        <div className="mt-4">
          <Button variant="primary" onClick={startNew} disabled={keyId === "Backspace"}>
            Add mapping
          </Button>
        </div>
      )}
    </Card>
  );
}

function MappingRow({ m, profile, onEdit, onRemove }: { m: Mapping; profile: Profile; onEdit: () => void; onRemove: () => void }) {
  const { state, edit, toast } = useStore();
  const [testing, setTesting] = useState(false);
  const savedProfile = state.saved?.profiles.find((p) => p.id === profile.id);
  const applied = JSON.stringify(savedProfile?.mappings.find((x) => x.id === m.id)) === JSON.stringify(m);
  const issue = state.report?.errors.find((i) => i.mapping === m.id && i.profile === profile.id) ?? state.report?.warnings.find((i) => i.mapping === m.id && i.profile === profile.id);
  return (
    <li className="rounded-lg border border-zinc-800 bg-zinc-900 p-3">
      <div className="flex flex-wrap items-center gap-2">
        <Toggle
          label={`Mapping ${chordLabel(m.trigger, state.keys)} on or off`}
          checked={m.enabled}
          onChange={(on) =>
            edit((c) => {
              const x = c.profiles.find((p) => p.id === profile.id)!.mappings.find((y) => y.id === m.id)!;
              x.enabled = on;
              x.edited = Date.now();
            })
          }
        />
        <Kbd>{chordLabel(m.trigger, state.keys)}</Kbd>
        {m.app ? <Badge tone="amber">only in {m.app}</Badge> : <Badge>all apps</Badge>}
        {!applied && <Badge tone="sky">not applied</Badge>}
        <div className="ml-auto flex gap-1">
          <Button
            variant="ghost"
            disabled={!applied || !state.status?.enabled || testing}
            title={!applied ? "Apply changes first" : !state.status?.enabled ? "Turn mappings on first" : "Send this key through the engine and check that the mapping runs"}
            onClick={async () => {
              setTesting(true);
              try {
                toast("ok", await api.testMapping(profile.id, m.id));
              } catch (e) {
                toast("error", errorText(e));
              } finally {
                setTesting(false);
              }
            }}
          >
            {testing ? "Testing…" : "Test"}
          </Button>
          <Button variant="ghost" onClick={onEdit}>
            Edit
          </Button>
          <Button variant="ghost" onClick={onRemove} aria-label={`Delete mapping ${chordLabel(m.trigger, state.keys)}`}>
            Delete
          </Button>
        </div>
      </div>
      <p className={`mt-2 text-sm ${m.enabled ? "text-zinc-300" : "text-zinc-500"}`}>
        {m.name ? <b className="mr-1">{m.name}:</b> : null}
        {describeMapping(m, state.keys, profile)}
      </p>
      {issue && <p className={`mt-1 text-xs ${state.report?.errors.includes(issue) ? "text-red-300" : "text-amber-300"}`}>{issue.message}</p>}
    </li>
  );
}

function MappingEditor({ initial, profile, onSave, onCancel }: { initial: Mapping; profile: Profile; onSave: (m: Mapping) => void; onCancel: () => void }) {
  const { state } = useStore();
  const [m, setM] = useState<Mapping>(initial);
  const settings = state.draft!.settings;
  const conflict = profile.mappings.find((o) => o.id !== m.id && o.enabled && sameTrigger(o.trigger, m.trigger) && (o.app ?? "") === (m.app ?? ""));
  const set = (patch: Partial<Mapping>) => setM({ ...m, ...patch });

  return (
    <form
      className="mt-4 grid gap-4 rounded-lg border border-sky-900 bg-zinc-950 p-4"
      onSubmit={(e) => {
        e.preventDefault();
        onSave(m);
      }}
    >
      <div className="grid gap-3 sm:grid-cols-2">
        <Field label="Trigger" hint="Hold these modifiers together with the key.">
          {() => (
            <div className="flex flex-wrap gap-3 pt-1">
              {MOD_NAMES.map((mod) => (
                <Checkbox key={mod} checked={!!m.trigger[mod]} onChange={(on) => set({ trigger: { ...m.trigger, [mod]: on } })} label={mod[0].toUpperCase() + mod.slice(1)} />
              ))}
            </div>
          )}
        </Field>
        <Field label="Where">
          {(id) => (
            <Select id={id} value={m.app ?? ""} onChange={(e) => set({ app: e.target.value || null })}>
              <option value="">All apps (global)</option>
              {profile.appRules.map((r) => (
                <option key={r.id} value={r.process.toLowerCase()}>
                  Only in {r.name || r.process}
                </option>
              ))}
            </Select>
          )}
        </Field>
      </div>
      <p className="text-sm text-zinc-400">
        When you press <Kbd>{chordLabel(m.trigger, state.keys)}</Kbd>: <span className="text-zinc-200">{describeAction(m.action, state.keys, profile)}</span>
      </p>
      {conflict && <Alert tone="warn">{chordLabel(conflict.trigger, state.keys)} is already mapped here ({describeMapping(conflict, state.keys, profile)}). Turn one off or change the trigger.</Alert>}
      <ActionEditor value={m.action} onChange={(action) => set({ action })} profile={profile} label="What it does" />

      <fieldset className="grid gap-3 rounded-lg border border-zinc-800 p-3">
        <legend className="px-1 text-xs text-zinc-400">Advanced behaviour</legend>
        <Checkbox checked={!!m.hold} onChange={(on) => set({ hold: on ? defaultAction("media", profile) : null })} label="Do something else when the key is held" />
        {m.hold && (
          <div className="grid gap-3 pl-6">
            <Field label="Hold longer than (ms)" hint={`Default ${settings.tapHoldMs} ms (Settings).`}>
              {(id) => (
                <Input id={id} type="number" min={100} max={2000} step={50} value={m.holdMs ?? settings.tapHoldMs} onChange={(e) => set({ holdMs: Number(e.target.value) || null })} className="w-32" />
              )}
            </Field>
            <ActionEditor value={m.hold} onChange={(hold) => set({ hold })} profile={profile} label="When held" />
          </div>
        )}
        <Checkbox checked={!!m.doubleTap} onChange={(on) => set({ doubleTap: on ? defaultAction("media", profile) : null })} label="Do something else on a double tap" />
        {m.doubleTap && (
          <div className="grid gap-3 pl-6">
            <p className="text-xs text-zinc-500">A single tap then waits {settings.doubleTapMs} ms (Settings) to see whether a second tap follows.</p>
            <ActionEditor value={m.doubleTap} onChange={(doubleTap) => set({ doubleTap })} profile={profile} label="On double tap" />
          </div>
        )}
      </fieldset>

      <div className="grid gap-3 sm:grid-cols-2">
        <Field label="Name (optional)">{(id) => <Input id={id} value={m.name ?? ""} onChange={(e) => set({ name: e.target.value })} />}</Field>
        <div className="flex items-end">
          <Checkbox checked={m.enabled} onChange={(enabled) => set({ enabled })} label="Enabled" />
        </div>
      </div>
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Button variant="primary" type="submit">
          Save mapping
        </Button>
      </div>
      <p className="text-xs text-zinc-500">Saved mappings take effect when you click Apply changes.</p>
    </form>
  );
}

