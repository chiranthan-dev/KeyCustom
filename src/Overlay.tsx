// The cheat sheet: a small panel in the corner of the screen, shown while
// the cheat-sheet key is held. It lives in its own transparent, click-through
// window that never takes focus.
import { useEffect, useState } from "react";
import { api, onEvent } from "./lib/api";
import { chordLabel, cheatSheetRows } from "./lib/model";
import type { Config, KeyDef } from "./lib/types";

const MAX_ROWS = 12;

export function Overlay() {
  const [config, setConfig] = useState<Config | null>(null);
  const [keys, setKeys] = useState<KeyDef[]>([]);
  const [ctx, setCtx] = useState<{ app: string; profile: string } | null>(null);

  useEffect(() => {
    api.keys().then(setKeys, () => {});
    api.snapshot().then((s) => setConfig(s.config), () => {});
    const sub = onEvent("overlay", (e) => {
      if (!e.show) {
        setCtx(null);
        return;
      }
      setCtx({ app: e.app ?? "", profile: e.profile ?? "" });
      // Show the cached configuration at once, then refresh it.
      api.snapshot().then((s) => setConfig(s.config), () => {});
    });
    return () => {
      sub.then((unlisten) => unlisten());
    };
  }, []);

  if (!ctx || !config) return null;
  const { profile, rows, globalOff } = cheatSheetRows(config, ctx.profile, ctx.app, keys);
  const shown = rows.slice(0, MAX_ROWS);
  return (
    <div className="fixed inset-0 flex items-end justify-end">
      <section
        aria-label="Cheat sheet"
        className="w-[340px] border border-white/10 bg-neutral-950/90 p-4 font-mono text-neutral-100 shadow-2xl backdrop-blur"
      >
        <header className="mb-3 flex items-center justify-between">
          <span className="text-sm font-semibold">{profile?.name ?? "KeyForge"}</span>
          <span className="rounded-full bg-lime-400/15 px-2 py-0.5 text-[11px] text-lime-300">on</span>
        </header>
        {shown.length === 0 ? (
          <p className="text-xs text-neutral-400">{globalOff ? `Mappings are off in ${ctx.app}.` : "No mappings in this profile yet."}</p>
        ) : (
          <ul className="grid grid-cols-1 gap-2">
            {shown.map((r, i) => (
              <li key={i} className="flex min-w-0 items-center justify-between gap-3 text-[12.5px]">
                <span className="shrink-0 border border-white/15 px-1.5 py-0.5 text-[11px] whitespace-nowrap text-neutral-200">{chordLabel(r.trigger, keys)}</span>
                <span className={`min-w-0 truncate text-right ${r.appOnly ? "text-amber-300" : "text-neutral-200"}`}>{r.action}</span>
              </li>
            ))}
          </ul>
        )}
        <footer className="mt-3 border-t border-white/10 pt-2 text-[10.5px] text-neutral-400">
          {rows.length > MAX_ROWS ? `+${rows.length - MAX_ROWS} more · ` : ""}
          {ctx.app ? `keys for ${ctx.app}` : "all apps"}
          {globalOff && rows.length > 0 ? " · global mappings off here" : ""} · release to hide
        </footer>
      </section>
    </div>
  );
}
