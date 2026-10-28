// App state: the saved configuration, the edited draft with undo/redo,
// engine status and transient UI state.
import { createContext, useCallback, useContext, useEffect, useMemo, useReducer, useRef, type ReactNode } from "react";
import { api, errorText, onEvent } from "./lib/api";
import type { ApplyOutcome, Config, DetectedKey, KeyDef, Report, Status } from "./lib/types";

export type Page = "dashboard" | "studio" | "macros" | "profiles" | "apps" | "settings";

interface Fired {
  profile: string;
  mapping: string;
  at: number;
}

interface State {
  keys: KeyDef[];
  saved: Config | null;
  draft: Config | null;
  past: Config[];
  future: Config[];
  status: Status | null;
  report: Report | null;
  fired: Fired[];
  toast: { tone: "ok" | "error" | "info"; text: string; action?: "enable" } | null;
  busy: boolean;
  page: Page;
  /** Profile being edited in Studio, Macros and App Rules. */
  profileId: string | null;
  /** App scope pre-selected in the Studio ("" = global). */
  scope: string;
  selectedKey: string | null;
}

type Msg =
  | { t: "init"; config: Config; status: Status; keys: KeyDef[] }
  | { t: "edit"; config: Config }
  | { t: "undo" }
  | { t: "redo" }
  | { t: "discard" }
  | { t: "applied"; outcome: ApplyOutcome }
  | { t: "status"; status: Status }
  | { t: "report"; report: Report | null }
  | { t: "fired"; fired: Fired }
  | { t: "toast"; toast: State["toast"] }
  | { t: "busy"; busy: boolean }
  | { t: "nav"; page: Page; profileId?: string; scope?: string; selectedKey?: string | null }
  | { t: "profile-saved"; id: string };

const HISTORY = 100;

function reduce(s: State, m: Msg): State {
  switch (m.t) {
    case "init":
      return { ...s, keys: m.keys, saved: m.config, draft: m.config, status: m.status, profileId: m.config.activeProfile };
    case "edit":
      if (!s.draft) return s;
      return { ...s, past: [...s.past.slice(-HISTORY + 1), s.draft], future: [], draft: m.config, report: null };
    case "undo": {
      if (!s.draft || s.past.length === 0) return s;
      const prev = s.past[s.past.length - 1];
      return { ...s, past: s.past.slice(0, -1), future: [s.draft, ...s.future], draft: prev, report: null };
    }
    case "redo": {
      if (!s.draft || s.future.length === 0) return s;
      return { ...s, past: [...s.past, s.draft], future: s.future.slice(1), draft: s.future[0], report: null };
    }
    case "discard":
      return s.saved ? { ...s, past: s.draft ? [...s.past, s.draft] : s.past, future: [], draft: s.saved, report: null } : s;
    case "applied": {
      const { outcome } = m;
      if (!outcome.applied) return { ...s, report: outcome.report, status: outcome.status };
      return { ...s, saved: outcome.config, draft: outcome.config, report: outcome.report, status: outcome.status };
    }
    case "status": {
      // Profile switches by shortcut change the saved active profile.
      const sync = (c: Config | null) => (c && c.activeProfile !== m.status.activeProfile ? { ...c, activeProfile: m.status.activeProfile } : c);
      return { ...s, status: m.status, saved: sync(s.saved), draft: sync(s.draft) };
    }
    case "profile-saved": {
      const sync = (c: Config | null) => (c ? { ...c, activeProfile: m.id } : c);
      return { ...s, saved: sync(s.saved), draft: sync(s.draft) };
    }
    case "report":
      return { ...s, report: m.report };
    case "fired":
      return { ...s, fired: [m.fired, ...s.fired].slice(0, 20) };
    case "toast":
      return { ...s, toast: m.toast };
    case "busy":
      return { ...s, busy: m.busy };
    case "nav":
      return {
        ...s,
        page: m.page,
        profileId: m.profileId ?? s.profileId,
        scope: m.scope ?? (m.profileId ? "" : s.scope),
        selectedKey: m.selectedKey !== undefined ? m.selectedKey : s.selectedKey,
      };
  }
}

const initial: State = {
  keys: [],
  saved: null,
  draft: null,
  past: [],
  future: [],
  status: null,
  report: null,
  fired: [],
  toast: null,
  busy: false,
  page: "dashboard",
  profileId: null,
  scope: "",
  selectedKey: null,
};

function useStoreValue() {
  const [state, dispatch] = useReducer(reduce, initial);
  const draftRef = useRef<Config | null>(null);
  draftRef.current = state.draft;
  const detectWaiter = useRef<((k: DetectedKey | null) => void) | null>(null);

  useEffect(() => {
    Promise.all([api.snapshot(), api.keys()]).then(
      ([snap, keys]) => dispatch({ t: "init", config: snap.config, status: snap.status, keys }),
      (e) => dispatch({ t: "toast", toast: { tone: "error", text: errorText(e) } }),
    );
    const subs = [
      onEvent("status", (status) => dispatch({ t: "status", status })),
      onEvent("fired", (f) => dispatch({ t: "fired", fired: { ...f, at: Date.now() } })),
      onEvent("key-detected", (k) => {
        detectWaiter.current?.(k);
        detectWaiter.current = null;
      }),
      onEvent("detect-ended", (reason) => {
        detectWaiter.current?.(null);
        detectWaiter.current = null;
        if (reason === "timeout") dispatch({ t: "toast", toast: { tone: "info", text: "No key was pressed within 15 seconds." } });
      }),
    ];
    return () => subs.forEach((p) => p.then((unlisten) => unlisten()));
  }, []);

  const toast = useCallback((tone: "ok" | "error" | "info", text: string, action?: "enable") => dispatch({ t: "toast", toast: { tone, text, action } }), []);

  /** Run a backend call with busy state and error toast. */
  const run = useCallback(
    async <T,>(fn: () => Promise<T>): Promise<T | undefined> => {
      dispatch({ t: "busy", busy: true });
      try {
        return await fn();
      } catch (e) {
        toast("error", errorText(e));
        return undefined;
      } finally {
        dispatch({ t: "busy", busy: false });
      }
    },
    [toast],
  );

  const edit = useCallback((mutate: (c: Config) => void) => {
    if (!draftRef.current) return;
    const next = structuredClone(draftRef.current);
    mutate(next);
    dispatch({ t: "edit", config: next });
  }, []);

  const applyOutcome = useCallback(
    (outcome: ApplyOutcome | undefined, verb: string) => {
      if (!outcome) return false;
      dispatch({ t: "applied", outcome });
      if (!outcome.applied) {
        toast("error", `Not applied: fix ${outcome.report.errors.length} problem(s) first.`);
        return false;
      }
      if (outcome.status.enabled) toast("ok", `${verb}. Mappings are active.`);
      else toast("info", `${verb}. Mappings are off, so keys behave normally until you turn them on.`, "enable");
      return true;
    },
    [toast],
  );

  const apply = useCallback(async () => {
    if (!draftRef.current) return false;
    const draft = draftRef.current;
    return applyOutcome(await run(() => api.apply(draft)), "Saved");
  }, [run, applyOutcome]);

  const setEnabled = useCallback(
    async (on: boolean) => {
      const status = await run(() => api.setEnabled(on));
      if (status) {
        dispatch({ t: "status", status });
        dispatch({ t: "toast", toast: null });
      }
    },
    [run],
  );

  const setActiveProfile = useCallback(
    async (id: string) => {
      const status = await run(() => api.setProfile(id));
      if (status) {
        dispatch({ t: "profile-saved", id });
        dispatch({ t: "status", status });
      }
    },
    [run],
  );

  /** Ask the engine for the next key press. Resolves null on timeout or cancel. */
  const detect = useCallback(async (): Promise<DetectedKey | null> => {
    detectWaiter.current?.(null);
    const result = new Promise<DetectedKey | null>((resolve) => (detectWaiter.current = resolve));
    const status = await run(() => api.startDetect());
    if (!status) {
      detectWaiter.current = null;
      return null;
    }
    dispatch({ t: "status", status });
    return result;
  }, [run]);

  const dirty = useMemo(() => !!state.draft && !!state.saved && JSON.stringify(state.draft) !== JSON.stringify(state.saved), [state.draft, state.saved]);

  return { state, dispatch, edit, apply, applyOutcome, run, toast, setEnabled, setActiveProfile, detect, dirty };
}

type Store = ReturnType<typeof useStoreValue>;
const StoreContext = createContext<Store | null>(null);

export function StoreProvider({ children }: { children: ReactNode }) {
  const value = useStoreValue();
  return <StoreContext.Provider value={value}>{children}</StoreContext.Provider>;
}

export function useStore(): Store {
  const s = useContext(StoreContext);
  if (!s) throw new Error("useStore outside StoreProvider");
  return s;
}

/** The profile being edited, falling back to the active one. */
export function useEditedProfile() {
  const { state } = useStore();
  const draft = state.draft!;
  const profile = draft.profiles.find((p) => p.id === state.profileId) ?? draft.profiles.find((p) => p.id === draft.activeProfile) ?? draft.profiles[0];
  return profile;
}
