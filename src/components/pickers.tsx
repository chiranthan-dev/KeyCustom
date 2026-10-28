// Key and chord pickers shared by the Studio, Macro Builder and Profiles.
import { useMemo, useState } from "react";
import { MOD_NAMES, keyLabel } from "../lib/model";
import type { Chord, KeyDef } from "../lib/types";
import { useStore } from "../store";
import { Button, Checkbox, Select } from "./ui";

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

export function useKeyGroups(keys: KeyDef[]) {
  return useMemo(() => {
    const map = new Map<string, KeyDef[]>();
    for (const k of keys) map.set(k.group, [...(map.get(k.group) ?? []), k]);
    return [...map.entries()].map(([g, ks]) => ({ group: g, label: GROUP_LABELS[g] ?? g, keys: ks }));
  }, [keys]);
}

export function KeySelect({ id, value, onChange, exclude = [] }: { id?: string; value: string; onChange: (id: string) => void; exclude?: string[] }) {
  const { state } = useStore();
  const groups = useKeyGroups(state.keys);
  const known = state.keys.some((k) => k.id.toLowerCase() === value.toLowerCase());
  return (
    <Select id={id} value={known ? state.keys.find((k) => k.id.toLowerCase() === value.toLowerCase())!.id : value} onChange={(e) => onChange(e.target.value)}>
      {!known && <option value={value}>{value} (detected)</option>}
      {groups.map((g) => (
        <optgroup key={g.group} label={g.label}>
          {g.keys
            .filter((k) => !exclude.includes(k.id))
            .map((k) => (
              <option key={k.id} value={k.id}>
                {k.label}
              </option>
            ))}
        </optgroup>
      ))}
    </Select>
  );
}

/** A key with modifiers, chosen from the list or by pressing it. */
export function ChordPicker({ value, onChange, allowDetect = true, label }: { value: Chord; onChange: (c: Chord) => void; allowDetect?: boolean; label?: string }) {
  const { detect, state } = useStore();
  const [waiting, setWaiting] = useState(false);
  return (
    <div className="grid gap-2" role="group" aria-label={label ?? "Key combination"}>
      <div className="flex flex-wrap items-center gap-2">
        <KeySelect value={value.key} onChange={(key) => onChange({ ...value, key })} />
        {allowDetect && (
          <Button
            disabled={waiting || !state.status?.ahkPath}
            onClick={async () => {
              setWaiting(true);
              const k = await detect();
              setWaiting(false);
              if (k) onChange({ key: k.id, ctrl: k.ctrl, shift: k.shift, alt: k.alt, win: k.win });
            }}
          >
            {waiting ? "Press a key…" : "Detect"}
          </Button>
        )}
      </div>
      <div className="flex flex-wrap gap-3">
        {MOD_NAMES.map((m) => (
          <Checkbox key={m} checked={!!value[m]} onChange={(on) => onChange({ ...value, [m]: on })} label={m[0].toUpperCase() + m.slice(1)} />
        ))}
      </div>
      <span className="sr-only">Selected: {keyLabel(value.key, state.keys)}</span>
    </div>
  );
}
