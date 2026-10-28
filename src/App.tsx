import { useEffect, useMemo, useState } from "react";
import { api, hex, onEvent, triggerLabel, type DetectedKey, type Issue, type KeyDef, type Mapping, type Modifiers, type Status } from "./lib/api";
import { Alert, Button, Card, Kbd, Toggle } from "./components/ui";

const GROUP_LABELS: Record<string, string> = {
  function: "Function keys",
  navigation: "Navigation",
  system: "System",
  modifier: "Modifiers",
  alphanumeric: "Letters and digits",
  punctuation: "Punctuation",
  numpad: "Numpad",
  media: "Media",
  browser: "Browser and launch",
};

const MOD_NAMES = ["ctrl", "shift", "alt", "win"] as const;

function errorText(e: unknown) {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}

export default function App() {
  const [status, setStatus] = useState<Status | null>(null);
  const [keys, setKeys] = useState<KeyDef[]>([]);
  const [draft, setDraft] = useState<Mapping[]>([]);
  const [dirty, setDirty] = useState(false);
  const [detected, setDetected] = useState<DetectedKey | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [triggerKey, setTriggerKey] = useState("F13");
  const [mods, setMods] = useState<Modifiers>({});
  const [target, setTarget] = useState("Media_Play_Pause");
  const [issues, setIssues] = useState<{ errors: Issue[]; warnings: Issue[] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.status().then((s) => {
      setStatus(s);
      setDraft(s.mappings);
    }, (e) => setError(errorText(e)));
    api.keys().then(setKeys, (e) => setError(errorText(e)));
    const subs = [
      onEvent<Status>("status", setStatus),
      onEvent<DetectedKey>("key-detected", (k) => {
        setDetected(k);
        setNotice(null);
        setTriggerKey(k.id);
        setMods({ ctrl: k.ctrl, shift: k.shift, alt: k.alt, win: k.win });
      }),
      onEvent<string>("detect-ended", (reason) => setNotice(reason === "timeout" ? "No key was pressed within 15 seconds." : null)),
    ];
    return () => subs.forEach((p) => p.then((unlisten) => unlisten()));
  }, []);

  const groups = useMemo(() => {
    const map = new Map<string, KeyDef[]>();
    for (const k of keys) map.set(k.group, [...(map.get(k.group) ?? []), k]);
    return [...map.entries()];
  }, [keys]);

  const labelOf = (id: string) => keys.find((k) => k.id === id)?.label ?? id;

  async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
    setBusy(true);
    setError(null);
    try {
      return await fn();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  function update(next: Mapping[]) {
    setDraft(next);
    setDirty(true);
    setIssues(null);
  }

  function addMapping() {
    const trigger = { key: triggerKey, ...Object.fromEntries(MOD_NAMES.map((m) => [m, !!mods[m]])) };
    update([...draft, { id: crypto.randomUUID().slice(0, 8), enabled: true, trigger, action: { type: "remap", to: target } }]);
  }

  async function apply(): Promise<boolean> {
    const result = await run(() => api.apply(draft));
    if (!result) return false;
    setIssues(result.report);
    setStatus(result.status);
    const ok = result.report.errors.length === 0;
    if (ok) setDirty(false);
    return ok;
  }

  async function setEnabled(on: boolean) {
    if (on && dirty && !(await apply())) return;
    const s = await run(() => api.setEnabled(on));
    if (s) setStatus(s);
  }

  const detecting = status?.detecting ?? false;
  const issueFor = (id: string) => issues?.errors.find((i) => i.mapping === id) ?? issues?.warnings.find((i) => i.mapping === id);

  return (
    <div className="mx-auto flex min-h-screen max-w-6xl flex-col gap-5 p-6">
      <header className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold">KeyForge</h1>
          <p className="text-sm text-zinc-400">Stage 1: engine proof of concept. Detect a key and remap it.</p>
        </div>
        <div className="flex items-center gap-3">
          <span className={`text-sm ${status?.enabled ? "text-emerald-400" : "text-zinc-400"}`}>{status?.enabled ? "Mappings active" : "Mappings off"}</span>
          <Toggle label="Enable mappings" checked={status?.enabled ?? false} disabled={busy || !status?.ahkPath} onChange={setEnabled} />
        </div>
      </header>

      {error && <Alert tone="error">{error}</Alert>}
      {status?.lastError && !error && <Alert tone="warn">{status.lastError}</Alert>}
      {status && !status.ahkPath && (
        <Alert tone="error">AutoHotkey v2 was not found. The installer bundles it; for a development build, install AutoHotkey v2 or set KEYFORGE_AHK to AutoHotkey64.exe.</Alert>
      )}

      <div className="grid gap-5 md:grid-cols-2">
        <Card title="Detect a key" aside={detecting && <span className="animate-pulse text-xs text-sky-400">Listening…</span>}>
          <p className="mb-4 text-sm text-zinc-400">
            The engine reports the next key you press, then stops listening. Nothing is recorded or stored. The Fn key and some laptop keys are handled by
            the keyboard firmware and never reach Windows, so they cannot be detected or remapped.
          </p>
          <div className="flex gap-2">
            <Button variant="primary" disabled={busy || detecting || !status?.ahkPath} onClick={() => run(async () => { setDetected(null); setNotice(null); setStatus(await api.startDetect()); })}>
              Detect key
            </Button>
            {detecting && <Button onClick={() => api.cancelDetect()}>Cancel</Button>}
          </div>
          {notice && <p className="mt-3 text-sm text-amber-300">{notice}</p>}
          {detected && (
            <dl className="mt-4 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-sm" aria-live="polite">
              <dt className="text-zinc-400">Key</dt>
              <dd><Kbd>{detected.name || labelOf(detected.id)}</Kbd> <span className="ml-1 text-zinc-500">({detected.id})</span></dd>
              <dt className="text-zinc-400">Virtual key</dt>
              <dd className="font-mono">{hex(detected.vk)}</dd>
              <dt className="text-zinc-400">Scan code</dt>
              <dd className="font-mono">{hex(detected.sc, 3)}</dd>
              <dt className="text-zinc-400">Modifiers</dt>
              <dd>{MOD_NAMES.filter((m) => detected[m]).map((m) => m[0].toUpperCase() + m.slice(1)).join(" + ") || "none"}</dd>
            </dl>
          )}
        </Card>

        <Card title="New remap">
          <div className="grid gap-3 text-sm">
            <label className="grid gap-1">
              <span className="text-zinc-400">When I press</span>
              <select className="rounded-lg border border-zinc-700 bg-zinc-800 px-2 py-1.5" value={triggerKey} onChange={(e) => setTriggerKey(e.target.value)}>
                {!keys.some((k) => k.id === triggerKey) && <option value={triggerKey}>{triggerKey} (detected)</option>}
                {groups.map(([g, ks]) => (
                  <optgroup key={g} label={GROUP_LABELS[g] ?? g}>
                    {ks.map((k) => <option key={k.id} value={k.id}>{k.label}</option>)}
                  </optgroup>
                ))}
              </select>
            </label>
            <fieldset className="flex flex-wrap gap-3">
              <legend className="sr-only">Modifiers held with the key</legend>
              {MOD_NAMES.map((m) => (
                <label key={m} className="flex items-center gap-1.5 text-zinc-300">
                  <input type="checkbox" className="accent-sky-500" checked={!!mods[m]} onChange={(e) => setMods({ ...mods, [m]: e.target.checked })} />
                  {m[0].toUpperCase() + m.slice(1)}
                </label>
              ))}
            </fieldset>
            <label className="grid gap-1">
              <span className="text-zinc-400">Send this key instead</span>
              <select className="rounded-lg border border-zinc-700 bg-zinc-800 px-2 py-1.5" value={target} onChange={(e) => setTarget(e.target.value)}>
                {groups.map(([g, ks]) => (
                  <optgroup key={g} label={GROUP_LABELS[g] ?? g}>
                    {ks.map((k) => <option key={k.id} value={k.id}>{k.label}</option>)}
                  </optgroup>
                ))}
              </select>
            </label>
            <div>
              <Button onClick={addMapping}>Add remap</Button>
            </div>
          </div>
        </Card>
      </div>

      <Card
        title="Mappings"
        aside={
          <div className="flex items-center gap-2">
            {dirty && <span className="text-xs text-amber-300">Unsaved changes</span>}
            <Button variant="primary" disabled={!dirty || busy} onClick={apply}>Apply</Button>
          </div>
        }
      >
        {draft.length === 0 ? (
          <p className="text-sm text-zinc-500">No mappings yet. Detect a key or pick one above, then add a remap.</p>
        ) : (
          <ul className="divide-y divide-zinc-800">
            {draft.map((m) => {
              const issue = issueFor(m.id);
              const isError = issues?.errors.includes(issue as Issue);
              return (
                <li key={m.id} className="flex flex-wrap items-center gap-3 py-2.5">
                  <Toggle label={`Enable ${triggerLabel(m.trigger)}`} checked={m.enabled} onChange={(on) => update(draft.map((x) => (x.id === m.id ? { ...x, enabled: on } : x)))} />
                  <span className={`flex items-center gap-2 text-sm ${m.enabled ? "" : "opacity-50"}`}>
                    <Kbd>{triggerLabel({ ...m.trigger, key: labelOf(m.trigger.key) })}</Kbd>
                    <span aria-hidden>→</span>
                    <span className="sr-only">remapped to</span>
                    <Kbd>{labelOf(m.action.to)}</Kbd>
                  </span>
                  {issue && <span className={`text-xs ${isError ? "text-red-300" : "text-amber-300"}`}>{issue.message}</span>}
                  <Button variant="ghost" className="ml-auto" aria-label={`Remove ${triggerLabel(m.trigger)}`} onClick={() => update(draft.filter((x) => x.id !== m.id))}>
                    Remove
                  </Button>
                </li>
              );
            })}
          </ul>
        )}
        {issues?.errors.filter((i) => !i.mapping).map((i) => <Alert key={i.message} tone="error">{i.message}</Alert>)}
      </Card>

      <footer className="mt-auto flex flex-wrap gap-x-6 gap-y-1 text-xs text-zinc-500">
        <span>
          Emergency disable: <Kbd>{status?.emergencyShortcut ?? "Ctrl+Alt+Shift+Backspace"}</Kbd>
          {status && !status.emergencyRegistered && " (handled by the engine only; another app holds the backup registration)"}
        </span>
        <span>Engine: {status?.engineRunning ? `running, AutoHotkey ${status.ahkVersion}` : "stopped"}</span>
        {status?.elevatedEngine && <span className="text-amber-300">Engine runs as administrator</span>}
        <span>Apps running as administrator cannot be controlled while KeyForge runs without elevation.</span>
      </footer>
    </div>
  );
}
