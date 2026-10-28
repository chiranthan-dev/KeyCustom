import { useState } from "react";
import { browseFile } from "../components/ActionEditor";
import { Badge, Button, Card, Empty, Field, Input, PageHeader, Select, Toggle } from "../components/ui";
import { chordLabel, describeMapping, isValidProcess, newId } from "../lib/model";
import type { AppRule } from "../lib/types";
import { useEditedProfile, useStore } from "../store";

export function AppRules() {
  const { state, dispatch, edit } = useStore();
  const profile = useEditedProfile();
  const [process, setProcess] = useState("");
  const [mode, setMode] = useState<AppRule["mode"]>("override");
  const proc = process.trim().toLowerCase();
  const exists = profile.appRules.some((r) => r.process.toLowerCase() === proc);

  const add = (name: string) =>
    edit((c) => c.profiles.find((p) => p.id === profile.id)!.appRules.push({ id: newId("app"), process: name, name: "", mode, enabled: true }));

  const update = (id: string, fn: (r: AppRule) => void) => edit((c) => fn(c.profiles.find((p) => p.id === profile.id)!.appRules.find((r) => r.id === id)!));

  return (
    <>
      <PageHeader
        title="Application Rules"
        description="Give an app its own mappings that override the global ones, or turn this profile's mappings off while an app (such as a game) is in front."
      >
        <Select aria-label="Profile" value={profile.id} onChange={(e) => dispatch({ t: "nav", page: "apps", profileId: e.target.value })}>
          {state.draft!.profiles.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </Select>
      </PageHeader>
      <Card title="Add a rule">
        <form
          className="flex flex-wrap items-end gap-3"
          onSubmit={(e) => {
            e.preventDefault();
            if (!isValidProcess(proc) || exists) return;
            add(proc);
            setProcess("");
          }}
        >
          <Field label="Program file name" hint={exists ? "This app already has a rule." : "As shown in Task Manager's Details tab, e.g. code.exe."}>
            {(id) => <Input id={id} placeholder="code.exe" value={process} onChange={(e) => setProcess(e.target.value)} />}
          </Field>
          <Button
            onClick={async () => {
              const path = await browseFile([{ name: "Programs", extensions: ["exe"] }]);
              if (path) setProcess(path.split(/[\\/]/).pop()!.toLowerCase());
            }}
          >
            Browse…
          </Button>
          <Field label="Rule">
            {(id) => (
              <Select id={id} value={mode} onChange={(e) => setMode(e.target.value as AppRule["mode"])}>
                <option value="override">Use app-specific mappings</option>
                <option value="disable">Turn mappings off in this app</option>
              </Select>
            )}
          </Field>
          <Button variant="primary" type="submit" disabled={!isValidProcess(proc) || exists}>
            Add rule
          </Button>
        </form>
      </Card>
      {profile.appRules.length === 0 ? (
        <Empty>No rules in profile “{profile.name}”.</Empty>
      ) : (
        <div className="grid gap-3">
          {profile.appRules.map((r) => {
            const maps = profile.mappings.filter((m) => (m.app ?? "").toLowerCase() === r.process.toLowerCase());
            return (
              <Card key={r.id}>
                <div className="flex flex-wrap items-center gap-3">
                  <Toggle label={`Rule for ${r.process} on or off`} checked={r.enabled} onChange={(on) => update(r.id, (x) => (x.enabled = on))} />
                  <span className="font-medium">{r.name || r.process}</span>
                  <Badge tone={r.mode === "disable" ? "amber" : "sky"}>{r.mode === "disable" ? "mappings off" : "app-specific mappings"}</Badge>
                  <Select aria-label={`Rule for ${r.process}`} value={r.mode} onChange={(e) => update(r.id, (x) => (x.mode = e.target.value as AppRule["mode"]))} className="ml-auto">
                    <option value="override">Use app-specific mappings</option>
                    <option value="disable">Turn global mappings off</option>
                  </Select>
                  <Button
                    variant="danger"
                    onClick={() =>
                      edit((c) => {
                        const p = c.profiles.find((x) => x.id === profile.id)!;
                        p.appRules = p.appRules.filter((x) => x.id !== r.id);
                        p.mappings = p.mappings.filter((m) => (m.app ?? "").toLowerCase() !== r.process.toLowerCase());
                      })
                    }
                    title={maps.length ? `Also deletes ${maps.length} app-specific mapping(s)` : undefined}
                  >
                    Delete
                  </Button>
                </div>
                <div className="mt-3 grid gap-1.5 text-sm">
                  {r.mode === "disable" && <p className="text-zinc-400">While {r.process} is in front, this profile's global mappings are off. Its app-specific mappings below still work.</p>}
                  {maps.length === 0 ? (
                    <p className="text-zinc-500">No app-specific mappings yet.</p>
                  ) : (
                    maps.map((m) => (
                      <p key={m.id} className="text-zinc-300">
                        <span className="font-mono text-xs">{chordLabel(m.trigger, state.keys)}</span> — {describeMapping(m, state.keys, profile)}
                      </p>
                    ))
                  )}
                  <div className="mt-1">
                    <Button onClick={() => dispatch({ t: "nav", page: "studio", profileId: profile.id, scope: r.process.toLowerCase() })}>Edit mappings for {r.process}</Button>
                  </div>
                </div>
              </Card>
            );
          })}
        </div>
      )}
      <p className="text-xs text-zinc-500">
        Programs running as administrator (elevated) cannot be controlled while KeyForge runs without elevation. This is a Windows security boundary.
      </p>
    </>
  );
}
