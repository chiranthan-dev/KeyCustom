// Sidebar, the unsaved-changes bar with preview, and notifications.
import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { diffConfigs } from "../lib/model";
import type { Preview } from "../lib/types";
import { useStore, type Page } from "../store";
import { Alert, Button, Dialog, Toggle } from "./ui";

const NAV: { page: Page; label: string; icon: string }[] = [
  { page: "dashboard", label: "Dashboard", icon: "◧" },
  { page: "studio", label: "Keyboard Studio", icon: "⌨" },
  { page: "macros", label: "Macro Builder", icon: "≡" },
  { page: "profiles", label: "Profiles", icon: "◎" },
  { page: "apps", label: "Application Rules", icon: "▣" },
  { page: "settings", label: "Settings", icon: "⚙" },
];

export function Sidebar() {
  const { state, dispatch, setEnabled } = useStore();
  const status = state.status;
  const active = state.saved?.profiles.find((p) => p.id === status?.activeProfile);
  return (
    <nav aria-label="Main" className="flex w-56 shrink-0 flex-col gap-1 border-r border-zinc-800 bg-zinc-950 p-3">
      <div className="mb-4 flex items-center gap-2 px-2 pt-1">
        <span aria-hidden className="grid h-8 w-8 place-items-center rounded-lg bg-sky-500 font-bold text-zinc-950">K</span>
        <span className="font-semibold">KeyForge</span>
      </div>
      {NAV.map((n) => (
        <button
          key={n.page}
          type="button"
          aria-current={state.page === n.page ? "page" : undefined}
          onClick={() => dispatch({ t: "nav", page: n.page })}
          className={`flex items-center gap-3 rounded-lg px-3 py-2 text-left text-sm transition-colors ${
            state.page === n.page ? "bg-zinc-800 text-zinc-50" : "text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100"
          }`}
        >
          <span aria-hidden className="w-4 text-center">
            {n.icon}
          </span>
          {n.label}
        </button>
      ))}
      <div className="mt-auto grid gap-3 rounded-lg border border-zinc-800 p-3 text-xs">
        <div className="flex items-center justify-between gap-2">
          <span className={status?.enabled ? "font-medium text-emerald-400" : "text-zinc-400"}>{status?.enabled ? "Mappings on" : "Mappings off"}</span>
          <Toggle label="Mappings on or off" checked={!!status?.enabled} disabled={state.busy || !status?.ahkPath} onChange={setEnabled} />
        </div>
        <div className="text-zinc-500">Profile: <span className="text-zinc-300">{active?.name ?? "—"}</span></div>
        <div className="text-zinc-500">Emergency off: <span className="text-zinc-300">Ctrl+Alt+Shift+Backspace</span></div>
      </div>
    </nav>
  );
}

export function ChangesBar() {
  const { state, dispatch, apply, dirty } = useStore();
  const [preview, setPreview] = useState<Preview | null>(null);
  const [showScript, setShowScript] = useState(false);
  const changes = dirty && state.saved && state.draft ? diffConfigs(state.saved, state.draft, state.keys) : [];

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // App-window shortcuts only; nothing here sees keys outside KeyForge.
      const target = e.target as HTMLElement;
      const typing = target.tagName === "INPUT" || target.tagName === "TEXTAREA";
      if (!e.ctrlKey || e.altKey) return;
      if (e.key.toLowerCase() === "z" && !typing) {
        e.preventDefault();
        dispatch({ t: e.shiftKey ? "redo" : "undo" });
      } else if (e.key.toLowerCase() === "y" && !typing) {
        e.preventDefault();
        dispatch({ t: "redo" });
      } else if (e.key.toLowerCase() === "s") {
        e.preventDefault();
        if (dirty) void apply();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dirty, apply, dispatch]);

  const errors = state.report?.errors ?? [];
  if (!dirty && errors.length === 0 && state.past.length === 0) return null;
  return (
    <div className="sticky bottom-0 z-10 border-t border-zinc-800 bg-zinc-950/95 px-6 py-3 backdrop-blur">
      {errors.length > 0 && (
        <div className="mb-2">
          <Alert tone="error">
            Not applied. Fix these first:
            <ul className="mt-1 list-disc pl-5">
              {errors.map((e, i) => (
                <li key={i}>{e.message}</li>
              ))}
            </ul>
          </Alert>
        </div>
      )}
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-sm text-zinc-300">{dirty ? `${changes.length || 1} unsaved change${changes.length === 1 ? "" : "s"}` : "All changes saved"}</span>
        <Button variant="ghost" disabled={state.past.length === 0} onClick={() => dispatch({ t: "undo" })} title="Undo (Ctrl+Z)">
          Undo
        </Button>
        <Button variant="ghost" disabled={state.future.length === 0} onClick={() => dispatch({ t: "redo" })} title="Redo (Ctrl+Y)">
          Redo
        </Button>
        <div className="ml-auto flex gap-2">
          <Button variant="ghost" disabled={!dirty} onClick={() => dispatch({ t: "discard" })}>
            Discard
          </Button>
          <Button
            disabled={!dirty || state.busy}
            onClick={async () => {
              setShowScript(false);
              setPreview(await api.preview(state.draft!));
            }}
          >
            Preview
          </Button>
          <Button variant="primary" disabled={!dirty || state.busy} onClick={() => apply()} title="Apply (Ctrl+S)">
            Apply changes
          </Button>
        </div>
      </div>
      <Dialog
        title="Preview changes"
        wide
        open={!!preview}
        onClose={() => setPreview(null)}
        footer={
          <>
            <Button variant="ghost" onClick={() => setPreview(null)}>
              Keep editing
            </Button>
            <Button
              variant="primary"
              disabled={!preview?.report || preview.report.errors.length > 0}
              onClick={async () => {
                setPreview(null);
                await apply();
              }}
            >
              Apply changes
            </Button>
          </>
        }
      >
        {preview && (
          <div className="grid gap-4 text-sm">
            <section>
              <h3 className="mb-2 font-medium">What changes</h3>
              <ul className="grid gap-1">
                {changes.map((c, i) => (
                  <li key={i} className="rounded-md bg-zinc-800/60 px-3 py-1.5">
                    {c}
                  </li>
                ))}
              </ul>
            </section>
            {preview.report.errors.length > 0 && (
              <Alert tone="error">
                {preview.report.errors.map((e) => e.message).join("\n")}
              </Alert>
            )}
            {preview.report.warnings.length > 0 && <Alert tone="warn">{preview.report.warnings.map((e) => e.message).join("\n")}</Alert>}
            {preview.report.errors.length === 0 && (
              <p className="text-zinc-400">
                Applying saves the current configuration as the last known-good copy first. If the engine cannot load the new one, the previous mappings stay active.
              </p>
            )}
            {preview.script && (
              <section>
                <Button variant="ghost" onClick={() => setShowScript(!showScript)} aria-expanded={showScript}>
                  {showScript ? "Hide" : "Show"} generated engine script
                </Button>
                {showScript && <pre className="mt-2 max-h-80 overflow-auto rounded-lg bg-zinc-950 p-3 text-xs text-zinc-300">{preview.script.slice(preview.script.indexOf("; --- Generated configuration"))}</pre>}
              </section>
            )}
          </div>
        )}
      </Dialog>
    </div>
  );
}

export function Toast() {
  const { state, dispatch, setEnabled } = useStore();
  const toast = state.toast;
  useEffect(() => {
    if (!toast || toast.tone === "error" || toast.action) return;
    const t = setTimeout(() => dispatch({ t: "toast", toast: null }), 5000);
    return () => clearTimeout(t);
  }, [toast, dispatch]);
  if (!toast) return null;
  const tone = toast.tone === "ok" ? "ok" : toast.tone === "error" ? "error" : "info";
  return (
    <div className="fixed right-6 bottom-20 z-20 w-96 max-w-[calc(100%-3rem)] shadow-xl">
      <Alert tone={tone} onClose={() => dispatch({ t: "toast", toast: null })}>
        {toast.text}
        {toast.action === "enable" && (
          <div className="mt-2">
            <Button variant="primary" onClick={() => setEnabled(true)}>
              Turn mappings on
            </Button>
          </div>
        )}
      </Alert>
    </div>
  );
}

export function StatusBanners() {
  const { state } = useStore();
  const s = state.status;
  if (!s) return null;
  return (
    <div className="grid gap-2">
      {s.notice && (
        <Alert tone="warn" onClose={() => api.dismissNotice()}>
          {s.notice}
        </Alert>
      )}
      {s.lastError && (
        <Alert tone="error" onClose={() => api.dismissNotice()}>
          {s.lastError}
        </Alert>
      )}
      {!s.ahkPath && <Alert tone="error">AutoHotkey v2 was not found. Reinstall KeyForge, or set a custom AutoHotkey path in Settings.</Alert>}
    </div>
  );
}
