import { useState } from "react";
import { browseFile } from "../components/ActionEditor";
import { ChordPicker } from "../components/pickers";
import { Badge, Button, Card, Empty, Field, Input, PageHeader, Select, TextArea } from "../components/ui";
import { describeStep, newId } from "../lib/model";
import type { Macro, Profile, Step } from "../lib/types";
import { useEditedProfile, useStore } from "../store";

const STEP_TYPES: { type: Step["type"]; label: string; make: () => Step }[] = [
  { type: "keys", label: "Key or shortcut", make: () => ({ type: "keys", keys: { key: "C", ctrl: true } }) },
  { type: "text", label: "Type text", make: () => ({ type: "text", text: "" }) },
  { type: "delay", label: "Wait", make: () => ({ type: "delay", ms: 200 }) },
  { type: "launch", label: "Open app", make: () => ({ type: "launch", path: "", args: "" }) },
  { type: "url", label: "Open website", make: () => ({ type: "url", url: "https://" }) },
];

export function Macros() {
  const { state, dispatch, edit } = useStore();
  const profile = useEditedProfile();
  const [selectedId, setSelectedId] = useState<string | null>(profile.macros[0]?.id ?? null);
  const macro = profile.macros.find((m) => m.id === selectedId) ?? null;
  const usedBy = (id: string) =>
    profile.mappings.filter((m) => [m.action, m.hold, m.doubleTap].some((a) => a?.type === "macro" && a.macro === id)).length;

  const update = (fn: (m: Macro) => void) =>
    edit((c) => {
      const m = c.profiles.find((p) => p.id === profile.id)!.macros.find((x) => x.id === selectedId)!;
      fn(m);
    });

  return (
    <>
      <PageHeader title="Macro Builder" description="Build sequences of keys, text, waits and launches. Assign a macro to a key in the Keyboard Studio.">
        <Select aria-label="Profile" value={profile.id} onChange={(e) => dispatch({ t: "nav", page: "macros", profileId: e.target.value })}>
          {state.draft!.profiles.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </Select>
        <Button
          variant="primary"
          onClick={() => {
            const id = newId("mac");
            edit((c) => c.profiles.find((p) => p.id === profile.id)!.macros.push({ id, name: `Macro ${profile.macros.length + 1}`, steps: [STEP_TYPES[0].make()] }));
            setSelectedId(id);
          }}
        >
          New macro
        </Button>
      </PageHeader>
      <div className="grid gap-4 md:grid-cols-[16rem_minmax(0,1fr)]">
        <Card title="Macros">
          {profile.macros.length === 0 ? (
            <Empty>No macros in this profile.</Empty>
          ) : (
            <ul className="grid gap-1">
              {profile.macros.map((m) => (
                <li key={m.id}>
                  <button
                    type="button"
                    aria-pressed={m.id === selectedId}
                    onClick={() => setSelectedId(m.id)}
                    className={`flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-sm ${m.id === selectedId ? "bg-zinc-800" : "hover:bg-zinc-800/60"}`}
                  >
                    <span className="truncate">{m.name}</span>
                    <span className="text-xs text-zinc-500">{m.steps.length}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </Card>
        {macro ? <MacroEditor macro={macro} profile={profile} usedBy={usedBy(macro.id)} update={update} onDelete={() => {
          edit((c) => {
            const p = c.profiles.find((x) => x.id === profile.id)!;
            p.macros = p.macros.filter((m) => m.id !== macro.id);
          });
          setSelectedId(null);
        }} /> : <Card><Empty>Select or create a macro.</Empty></Card>}
      </div>
    </>
  );
}

function MacroEditor({ macro, profile, usedBy, update, onDelete }: { macro: Macro; profile: Profile; usedBy: number; update: (fn: (m: Macro) => void) => void; onDelete: () => void }) {
  const { state } = useStore();
  const totalMs = macro.steps.reduce((t, s) => t + (s.type === "delay" ? s.ms : 0), 0);
  return (
    <Card
      title="Edit macro"
      aside={
        <div className="flex items-center gap-2">
          {usedBy > 0 ? <Badge tone="sky">used by {usedBy} mapping{usedBy > 1 ? "s" : ""}</Badge> : <Badge>not used yet</Badge>}
          <Button variant="danger" onClick={onDelete} title={usedBy ? "Mappings that use this macro will need a new action" : undefined}>
            Delete
          </Button>
        </div>
      }
    >
      <Field label="Name">{(id) => <Input id={id} value={macro.name} onChange={(e) => update((m) => (m.name = e.target.value))} />}</Field>
      <ol className="mt-4 grid gap-2" aria-label="Steps">
        {macro.steps.map((step, i) => (
          <li key={i} className="rounded-lg border border-zinc-800 bg-zinc-900 p-3">
            <div className="mb-2 flex items-center gap-2">
              <span className="grid h-6 w-6 place-items-center rounded-full bg-zinc-800 text-xs">{i + 1}</span>
              <span className="text-sm text-zinc-300">{describeStep(step, state.keys)}</span>
              <div className="ml-auto flex gap-1">
                <Button variant="ghost" aria-label={`Move step ${i + 1} up`} disabled={i === 0} onClick={() => update((m) => m.steps.splice(i - 1, 0, m.steps.splice(i, 1)[0]))}>
                  ↑
                </Button>
                <Button
                  variant="ghost"
                  aria-label={`Move step ${i + 1} down`}
                  disabled={i === macro.steps.length - 1}
                  onClick={() => update((m) => m.steps.splice(i + 1, 0, m.steps.splice(i, 1)[0]))}
                >
                  ↓
                </Button>
                <Button variant="ghost" aria-label={`Duplicate step ${i + 1}`} onClick={() => update((m) => m.steps.splice(i + 1, 0, structuredClone(m.steps[i])))}>
                  ⧉
                </Button>
                <Button variant="ghost" aria-label={`Delete step ${i + 1}`} onClick={() => update((m) => m.steps.splice(i, 1))}>
                  ✕
                </Button>
              </div>
            </div>
            <StepEditor step={step} onChange={(s) => update((m) => (m.steps[i] = s))} />
          </li>
        ))}
      </ol>
      <div className="mt-3 flex flex-wrap items-center gap-2">
        <span className="text-sm text-zinc-400">Add step:</span>
        {STEP_TYPES.map((t) => (
          <Button key={t.type} onClick={() => update((m) => m.steps.push(t.make()))}>
            + {t.label}
          </Button>
        ))}
      </div>
      <p className="mt-3 text-xs text-zinc-500">
        {macro.steps.length} steps · {totalMs} ms of waiting. Macros run in the profile “{profile.name}”. Keys a macro sends never trigger other mappings.
      </p>
    </Card>
  );
}

function StepEditor({ step, onChange }: { step: Step; onChange: (s: Step) => void }) {
  switch (step.type) {
    case "keys":
      return <ChordPicker value={step.keys} onChange={(keys) => onChange({ ...step, keys })} label="Key to press" />;
    case "text":
      return <TextArea aria-label="Text to type" value={step.text} onChange={(e) => onChange({ ...step, text: e.target.value })} className="w-full" />;
    case "delay":
      return (
        <label className="flex items-center gap-2 text-sm text-zinc-400">
          Wait
          <Input type="number" min={1} max={60000} step={50} value={step.ms} onChange={(e) => onChange({ ...step, ms: Number(e.target.value) || 1 })} className="w-28" />
          ms
        </label>
      );
    case "launch":
      return (
        <div className="flex flex-wrap gap-2">
          <Input aria-label="Program" className="min-w-60 flex-1" value={step.path} placeholder="C:\Program Files\App\app.exe" onChange={(e) => onChange({ ...step, path: e.target.value })} />
          <Button
            onClick={async () => {
              const path = await browseFile([{ name: "Programs", extensions: ["exe", "lnk", "bat", "cmd"] }]);
              if (path) onChange({ ...step, path });
            }}
          >
            Browse…
          </Button>
          <Input aria-label="Arguments" placeholder="Arguments (optional)" value={step.args ?? ""} onChange={(e) => onChange({ ...step, args: e.target.value })} />
        </div>
      );
    case "url":
      return <Input aria-label="Web address" className="w-full" value={step.url} onChange={(e) => onChange({ ...step, url: e.target.value.trim() })} />;
  }
}
