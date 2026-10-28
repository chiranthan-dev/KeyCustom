import { chordLabel, describeMapping } from "../lib/model";
import { useStore } from "../store";
import { Badge, Button, Card, Empty, Kbd, PageHeader, Select, Toggle } from "../components/ui";

export function Dashboard() {
  const { state, dispatch, setEnabled, setActiveProfile } = useStore();
  const status = state.status!;
  const saved = state.saved!;
  const active = saved.profiles.find((p) => p.id === status.activeProfile) ?? saved.profiles[0];
  const enabledCount = active.mappings.filter((m) => m.enabled).length;
  const recent = saved.profiles
    .flatMap((p) => p.mappings.map((m) => ({ p, m })))
    .filter(({ m }) => m.edited)
    .sort((a, b) => (b.m.edited ?? 0) - (a.m.edited ?? 0))
    .slice(0, 6);
  const mappingName = (profile: string, id: string) => {
    const p = saved.profiles.find((x) => x.id === profile);
    const m = p?.mappings.find((x) => x.id === id);
    return m ? chordLabel(m.trigger, state.keys) : id;
  };

  return (
    <>
      <PageHeader title="Dashboard" description="Your keyboard at a glance." />
      <div className="grid gap-4 md:grid-cols-3">
        <Card title="Mappings" className="md:col-span-2">
          <div className="flex flex-wrap items-center gap-4">
            <Toggle label="Pause or resume all mappings" checked={status.enabled} disabled={state.busy || !status.ahkPath} onChange={setEnabled} />
            <div>
              <p className={`text-lg font-semibold ${status.enabled ? "text-emerald-400" : "text-zinc-300"}`}>{status.enabled ? "Active" : "Paused"}</p>
              <p className="text-sm text-zinc-400">
                {status.enabled
                  ? "Your mappings work in every app that is not running as administrator."
                  : "Keys behave normally. Turn mappings on to use them. KeyForge always starts paused unless you change that in Settings."}
              </p>
            </div>
          </div>
          <p className="mt-4 text-xs text-zinc-500">
            Emergency off at any time: <Kbd>{status.emergencyShortcut}</Kbd>. It cannot be remapped.
          </p>
        </Card>
        <Card title="Active profile">
          <Select aria-label="Active profile" value={active.id} disabled={state.busy} onChange={(e) => setActiveProfile(e.target.value)} className="w-full">
            {saved.profiles.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </Select>
          <dl className="mt-4 grid grid-cols-2 gap-y-1 text-sm">
            <dt className="text-zinc-400">Enabled mappings</dt>
            <dd className="text-right font-medium">{enabledCount}</dd>
            <dt className="text-zinc-400">Macros</dt>
            <dd className="text-right font-medium">{active.macros.length}</dd>
            <dt className="text-zinc-400">App rules</dt>
            <dd className="text-right font-medium">{active.appRules.length}</dd>
            <dt className="text-zinc-400">Shortcut</dt>
            <dd className="text-right">{active.switchKey ? <Kbd>{chordLabel(active.switchKey, state.keys)}</Kbd> : "—"}</dd>
          </dl>
        </Card>
      </div>

      {saved.profiles.every((p) => p.mappings.length === 0) && (
        <Card title="Get started">
          <ol className="grid list-decimal gap-1.5 pl-5 text-sm text-zinc-300">
            <li>Open <b>Keyboard Studio</b> and click a key, or use <b>Detect</b> and press the key you want to use.</li>
            <li>Choose what it should do and click <b>Save mapping</b>.</li>
            <li>Click <b>Apply changes</b> at the bottom. This saves your mappings.</li>
            <li>Turn <b>mappings on</b> (sidebar or above), then press the key in any app. Use <b>Test</b> in the Studio to check it.</li>
          </ol>
          <div className="mt-4">
            <Button variant="primary" onClick={() => dispatch({ t: "nav", page: "studio" })}>
              Open Keyboard Studio
            </Button>
          </div>
        </Card>
      )}

      <div className="grid gap-4 md:grid-cols-2">
        <Card title="Recently edited">
          {recent.length === 0 ? (
            <Empty>Mappings you edit appear here.</Empty>
          ) : (
            <ul className="grid gap-2">
              {recent.map(({ p, m }) => (
                <li key={`${p.id}/${m.id}`}>
                  <button
                    type="button"
                    className="flex w-full items-center gap-3 rounded-lg px-2 py-1.5 text-left text-sm hover:bg-zinc-800"
                    onClick={() => dispatch({ t: "nav", page: "studio", profileId: p.id, scope: m.app ?? "", selectedKey: m.trigger.key })}
                  >
                    <Kbd>{chordLabel(m.trigger, state.keys)}</Kbd>
                    <span className="min-w-0 flex-1 truncate text-zinc-300">{describeMapping(m, state.keys, p)}</span>
                    <Badge>{p.name}</Badge>
                    {!m.enabled && <Badge tone="amber">off</Badge>}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </Card>
        <Card title="Recent activity">
          {state.fired.length === 0 ? (
            <Empty>When a mapping runs, it shows up here. Only configured mappings are listed; KeyForge never records other keys.</Empty>
          ) : (
            <ul className="grid gap-1.5 text-sm">
              {state.fired.map((f, i) => (
                <li key={i} className="flex items-center gap-3">
                  <span className="w-20 text-xs text-zinc-500">{new Date(f.at).toLocaleTimeString()}</span>
                  <Kbd>{mappingName(f.profile, f.mapping)}</Kbd>
                  <span className="text-zinc-500">{saved.profiles.find((p) => p.id === f.profile)?.name}</span>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </div>
      {state.report && state.report.warnings.length > 0 && (
        <Card title="Warnings">
          <ul className="grid list-disc gap-1 pl-5 text-sm text-amber-200">
            {state.report.warnings.map((w, i) => (
              <li key={i}>{w.message}</li>
            ))}
          </ul>
        </Card>
      )}
    </>
  );
}
