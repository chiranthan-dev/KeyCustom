import { useState } from "react";
import { ChordPicker } from "../components/pickers";
import { Badge, Button, Card, Field, Input, Kbd, PageHeader } from "../components/ui";
import { chordLabel, isValidProcess, slug } from "../lib/model";
import type { Profile } from "../lib/types";
import { useStore } from "../store";

export function Profiles() {
  const { state, edit, dispatch } = useStore();
  const draft = state.draft!;
  const [name, setName] = useState("");

  const add = (from?: Profile) => {
    const label = from ? `${from.name} copy` : name.trim() || "New profile";
    const id = slug(label, draft.profiles.map((p) => p.id));
    edit((c) =>
      c.profiles.push(
        from
          ? { ...structuredClone(from), id, name: label, switchKey: null, autoApps: [] }
          : { id, name: label, switchKey: null, autoApps: [], mappings: [], macros: [], appRules: [] },
      ),
    );
    setName("");
  };

  return (
    <>
      <PageHeader
        title="Profiles"
        description="Each profile has its own mappings, macros and app rules. Switch with a shortcut, from the dashboard, or automatically when an app is in front."
      >
        <Input aria-label="New profile name" placeholder="New profile name" value={name} onChange={(e) => setName(e.target.value)} />
        <Button variant="primary" onClick={() => add()}>
          Add profile
        </Button>
      </PageHeader>
      <div className="grid gap-4 lg:grid-cols-2">
        {draft.profiles.map((p) => (
          <ProfileCard key={p.id} p={p} onDuplicate={() => add(p)} onOpen={() => dispatch({ t: "nav", page: "studio", profileId: p.id })} />
        ))}
      </div>
    </>
  );
}

function ProfileCard({ p, onDuplicate, onOpen }: { p: Profile; onDuplicate: () => void; onOpen: () => void }) {
  const { state, edit, setActiveProfile } = useStore();
  const draft = state.draft!;
  const savedExists = state.saved!.profiles.some((x) => x.id === p.id);
  const isActive = state.status!.activeProfile === p.id;
  const [app, setApp] = useState("");
  const update = (fn: (x: Profile) => void) => edit((c) => fn(c.profiles.find((x) => x.id === p.id)!));

  return (
    <Card
      title={p.name}
      aside={
        <div className="flex items-center gap-2">
          {isActive ? <Badge tone="emerald">active</Badge> : <Button disabled={!savedExists || state.busy} title={savedExists ? undefined : "Apply changes first"} onClick={() => setActiveProfile(p.id)}>Activate</Button>}
        </div>
      }
    >
      <div className="grid gap-4">
        <Field label="Name">{(id) => <Input id={id} value={p.name} maxLength={64} onChange={(e) => update((x) => (x.name = e.target.value))} />}</Field>
        <Field label="Switch shortcut" hint="Works everywhere while mappings are on. Needs at least one modifier.">
          {() =>
            p.switchKey ? (
              <div className="grid gap-2">
                <ChordPicker value={p.switchKey} onChange={(c) => update((x) => (x.switchKey = c))} label="Profile shortcut" />
                <div>
                  <Button variant="ghost" onClick={() => update((x) => (x.switchKey = null))}>
                    Remove shortcut
                  </Button>
                </div>
              </div>
            ) : (
              <div>
                <Button onClick={() => update((x) => (x.switchKey = { key: `D${Math.min(draft.profiles.indexOf(p) + 1, 9)}`, ctrl: true, alt: true }))}>Add shortcut</Button>
              </div>
            )
          }
        </Field>
        <Field label="Activate automatically in these apps" hint="While one of these programs is in front, this profile's mappings apply instead of the active profile.">
          {(id) => (
            <div className="grid gap-2">
              <div className="flex flex-wrap gap-1.5">
                {p.autoApps.length === 0 && <span className="text-sm text-zinc-500">None</span>}
                {p.autoApps.map((a) => (
                  <span key={a} className="inline-flex items-center gap-1 rounded-md bg-zinc-800 px-2 py-0.5 text-xs">
                    {a}
                    <button type="button" aria-label={`Remove ${a}`} className="text-zinc-400 hover:text-zinc-100" onClick={() => update((x) => (x.autoApps = x.autoApps.filter((y) => y !== a)))}>
                      ✕
                    </button>
                  </span>
                ))}
              </div>
              <form
                className="flex gap-2"
                onSubmit={(e) => {
                  e.preventDefault();
                  const v = app.trim().toLowerCase();
                  if (!isValidProcess(v) || p.autoApps.includes(v)) return;
                  update((x) => x.autoApps.push(v));
                  setApp("");
                }}
              >
                <Input id={id} placeholder="code.exe" value={app} onChange={(e) => setApp(e.target.value)} />
                <Button type="submit" disabled={!isValidProcess(app)}>
                  Add
                </Button>
              </form>
            </div>
          )}
        </Field>
        <p className="text-sm text-zinc-400">
          {p.mappings.length} mappings · {p.macros.length} macros · {p.appRules.length} app rules
          {p.switchKey && (
            <>
              {" "}· switch with <Kbd>{chordLabel(p.switchKey, state.keys)}</Kbd>
            </>
          )}
        </p>
        <div className="flex flex-wrap gap-2">
          <Button onClick={onOpen}>Edit mappings</Button>
          <Button variant="ghost" onClick={onDuplicate}>
            Duplicate
          </Button>
          <Button
            variant="danger"
            disabled={draft.profiles.length === 1}
            onClick={() =>
              edit((c) => {
                c.profiles = c.profiles.filter((x) => x.id !== p.id);
                if (c.activeProfile === p.id) c.activeProfile = c.profiles[0].id;
              })
            }
          >
            Delete
          </Button>
        </div>
      </div>
    </Card>
  );
}
