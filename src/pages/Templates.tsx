import { useMemo, useState } from "react";
import { ActionEditor } from "../components/ActionEditor";
import { ChordPicker } from "../components/pickers";
import { Alert, Badge, Button, Card, Dialog, Field, Input, PageHeader, Select } from "../components/ui";
import { chordLabel, describeAction, describeStep, newId, sameTrigger } from "../lib/model";
import { TEMPLATES, TEMPLATE_GROUPS, type Template, type TemplateGroup } from "../lib/templates";
import type { Action, Chord } from "../lib/types";
import { useEditedProfile, useStore } from "../store";

export function Templates() {
  const [group, setGroup] = useState<TemplateGroup | "All">("All");
  const [query, setQuery] = useState("");
  const [using, setUsing] = useState<Template | null>(null);
  const list = useMemo(() => {
    const q = query.trim().toLowerCase();
    return TEMPLATES.filter((t) => (group === "All" || t.group === group) && (!q || `${t.name} ${t.description} ${t.group}`.toLowerCase().includes(q)));
  }, [group, query]);

  return (
    <>
      <PageHeader title="Templates" description="Common shortcuts, snippets and routines. Pick one, choose a key, and adjust it if you like.">
        <Input aria-label="Search templates" placeholder="Search templates" value={query} onChange={(e) => setQuery(e.target.value)} />
      </PageHeader>
      <div className="flex flex-wrap gap-1.5" role="tablist" aria-label="Template groups">
        {(["All", ...TEMPLATE_GROUPS.map((g) => g.group)] as const).map((g) => (
          <button
            key={g}
            type="button"
            role="tab"
            aria-selected={group === g}
            onClick={() => setGroup(g)}
            className={`rounded-full px-3 py-1 text-sm ${group === g ? "bg-lime-500 text-neutral-950" : "bg-neutral-800 text-neutral-300 hover:bg-neutral-700"}`}
          >
            {g}
          </button>
        ))}
      </div>
      {TEMPLATE_GROUPS.filter((g) => group === "All" || g.group === group).map((g) => {
        const items = list.filter((t) => t.group === g.group);
        if (items.length === 0) return null;
        return (
          <section key={g.group} aria-label={g.group} className="grid gap-2">
            <h2 className="mt-2 text-sm font-medium text-neutral-300">
              {g.group} <span className="font-normal text-neutral-500">· {g.blurb}</span>
            </h2>
            <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
              {items.map((t) => (
                <button
                  key={t.id}
                  type="button"
                  onClick={() => setUsing(t)}
                  className="group flex flex-col gap-1 rounded-xl border border-neutral-800 bg-neutral-900/60 p-4 text-left transition-colors hover:border-lime-700 hover:bg-neutral-900"
                >
                  <span className="flex items-center justify-between gap-2">
                    <span className="font-medium text-neutral-100">{t.name}</span>
                    {t.macro ? <Badge tone="accent">macro · {t.macro.steps.length} steps</Badge> : null}
                  </span>
                  <span className="text-sm text-neutral-400">{t.description}</span>
                  <span className="mt-2 text-xs text-lime-400 opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100">Use this →</span>
                </button>
              ))}
            </div>
          </section>
        );
      })}
      {list.length === 0 && <Alert tone="info">No templates match “{query}”.</Alert>}
      {using && <UseTemplate template={using} onClose={() => setUsing(null)} />}
    </>
  );
}

function UseTemplate({ template, onClose }: { template: Template; onClose: () => void }) {
  const { state, edit, dispatch, toast } = useStore();
  const edited = useEditedProfile();
  const draft = state.draft!;
  const [profileId, setProfileId] = useState(edited.id);
  const profile = draft.profiles.find((p) => p.id === profileId)!;
  const preselected = state.selectedKey && state.selectedKey !== "Backspace" ? { key: state.selectedKey } : null;
  const [trigger, setTrigger] = useState<Chord>(template.key ?? preselected ?? { key: "F13" });
  const [app, setApp] = useState("");
  const [action, setAction] = useState<Action | undefined>(template.action ? structuredClone(template.action) : undefined);
  const existing = profile.mappings.find((m) => m.enabled && sameTrigger(m.trigger, trigger) && (m.app ?? "") === app);

  const add = () => {
    edit((c) => {
      const p = c.profiles.find((x) => x.id === profileId)!;
      let mapped: Action;
      if (template.macro) {
        const id = newId("mac");
        p.macros.push({ id, name: template.macro.name, steps: structuredClone(template.macro.steps) });
        mapped = { type: "macro", macro: id };
      } else {
        mapped = action!;
      }
      if (existing) p.mappings = p.mappings.filter((m) => m.id !== existing.id);
      p.mappings.push({ id: newId("m"), name: template.name, enabled: true, trigger, action: mapped, app: app || null, edited: Date.now() });
    });
    onClose();
    dispatch({ t: "nav", page: "studio", profileId, scope: app, selectedKey: trigger.key });
    toast("info", `“${template.name}” added to ${chordLabel(trigger, state.keys)}. Click Apply changes to use it.`);
  };

  return (
    <Dialog
      title={template.name}
      open
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={add} disabled={trigger.key === "Backspace"}>
            {existing ? "Replace mapping" : "Add to key"}
          </Button>
        </>
      }
    >
      <div className="grid gap-4">
        <p className="text-sm text-neutral-400">{template.description}</p>
        <Field label="Key">{() => <ChordPicker value={trigger} onChange={setTrigger} label="Key for this template" />}</Field>
        <div className="grid gap-3 sm:grid-cols-2">
          <Field label="Profile">
            {(id) => (
              <Select id={id} value={profileId} onChange={(e) => setProfileId(e.target.value)}>
                {draft.profiles.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </Select>
            )}
          </Field>
          <Field label="Where">
            {(id) => (
              <Select id={id} value={app} onChange={(e) => setApp(e.target.value)}>
                <option value="">All apps</option>
                {profile.appRules.map((r) => (
                  <option key={r.id} value={r.process.toLowerCase()}>
                    Only in {r.name || r.process}
                  </option>
                ))}
              </Select>
            )}
          </Field>
        </div>
        {existing && <Alert tone="warn">{chordLabel(trigger, state.keys)} already {describeAction(existing.action, state.keys, profile).toLowerCase()}. Adding replaces it.</Alert>}
        {template.macro ? (
          <Card title={`Macro “${template.macro.name}”`}>
            <ol className="grid list-decimal gap-1 pl-5 text-sm text-neutral-300">
              {template.macro.steps.map((s, i) => (
                <li key={i}>{describeStep(s, state.keys)}</li>
              ))}
            </ol>
            {template.personalize && <p className="mt-2 text-xs text-neutral-500">{template.personalize}.</p>}
          </Card>
        ) : (
          action &&
          (template.personalize ? (
            <div className="grid gap-2">
              <p className="text-sm text-amber-200">{template.personalize}:</p>
              <ActionEditor value={action} onChange={setAction} profile={profile} label="Action" />
            </div>
          ) : (
            <p className="text-sm text-neutral-300">Does: {describeAction(action, state.keys, profile)}</p>
          ))
        )}
      </div>
    </Dialog>
  );
}
