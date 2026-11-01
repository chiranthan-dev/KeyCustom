// Physical keyboard layouts for the Keyboard Studio, in key units (1 unit =
// one letter key). Keys are placed absolutely so tall keys (numpad + and
// Enter) and the gaps between blocks are exact.

export type LayoutId = "full" | "laptop" | "gk61";

export interface PlacedKey {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
  label?: string;
  /** Why the key cannot be mapped; it is shown dashed. */
  disabled?: string;
  /** Secondary legend, e.g. the Fn-layer key a GK61 key sends with Fn. */
  sub?: string;
}

export interface Layout {
  id: LayoutId;
  name: string;
  width: number;
  height: number;
  keys: PlacedKey[];
  /** Key groups shown below the keyboard. */
  extras: { label: string; ids: string[] }[];
  note?: string;
}

const FN_FIRMWARE = "Fn is handled by the keyboard's firmware and never reaches Windows, so no software can detect or remap it.";
const BACKSPACE = "Backspace is reserved for the emergency shortcut (Ctrl+Alt+Shift+Backspace).";

const LABELS: Record<string, string> = {
  Escape: "Esc",
  PrintScreen: "PrtSc",
  ScrollLock: "ScrLk",
  Insert: "Ins",
  Delete: "Del",
  PgUp: "PgUp",
  PgDn: "PgDn",
  CapsLock: "Caps",
  LShift: "Shift",
  RShift: "Shift",
  LCtrl: "Ctrl",
  RCtrl: "Ctrl",
  LAlt: "Alt",
  RAlt: "Alt",
  LWin: "Win",
  RWin: "Win",
  AppsKey: "Menu",
  Left: "←",
  Up: "↑",
  Down: "↓",
  Right: "→",
  NumLock: "Num",
  NumpadDiv: "/",
  NumpadMult: "*",
  NumpadSub: "−",
  NumpadAdd: "+",
  NumpadEnter: "Enter",
  NumpadDot: ".",
  Fn: "Fn",
  ...Object.fromEntries(Array.from({ length: 10 }, (_, i) => [`Numpad${i}`, String(i)])),
};

/**
 * Place one row. Tokens: `id`, `id:w`, `id:w:h`, and `_:w` for a gap.
 * Letters and digits use their key ids (`Q`, `D1`).
 */
function row(y: number, x0: number, spec: string, unit = 1): PlacedKey[] {
  const keys: PlacedKey[] = [];
  let x = x0;
  for (const token of spec.split(" ")) {
    const [id, w = "1", h = "1"] = token.split(":");
    const width = Number(w) * unit;
    if (id !== "_") {
      const key: PlacedKey = { id, x, y, w: width, h: Number(h) };
      if (LABELS[id]) key.label = LABELS[id];
      if (id === "Fn") key.disabled = FN_FIRMWARE;
      if (id === "Backspace") key.disabled = BACKSPACE;
      if (/^D\d$/.test(id)) key.label = id.slice(1);
      keys.push(key);
    }
    x += width;
  }
  return keys;
}

const letters = (s: string) => s.split("").join(" ");
const R_NUM = "Backquote D1 D2 D3 D4 D5 D6 D7 D8 D9 D0 Minus Equal Backspace:2";
const R_TOP = `Tab:1.5 ${letters("QWERTYUIOP")} BracketLeft BracketRight Backslash:1.5`;
const R_HOME = `CapsLock:1.75 ${letters("ASDFGHJKL")} Semicolon Quote Enter:2.25`;
const R_LOW = `LShift:2.25 ${letters("ZXCVBNM")} Comma Period Slash RShift:2.75`;

const PUNCT_LABELS: Record<string, string> = {
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
};
for (const [id, label] of Object.entries(PUNCT_LABELS)) LABELS[id] = label;

const MEDIA = { label: "Media", ids: ["Media_Play_Pause", "Media_Prev", "Media_Next", "Media_Stop", "Volume_Mute", "Volume_Down", "Volume_Up"] };
const BROWSER = { label: "Browser & launch", ids: ["Browser_Back", "Browser_Forward", "Browser_Refresh", "Browser_Search", "Browser_Home", "Launch_Mail", "Launch_App1", "Launch_App2"] };
const F13_24 = { label: "Extra function keys (F13–F24)", ids: Array.from({ length: 12 }, (_, i) => `F${i + 13}`) };

const full: Layout = {
  id: "full",
  name: "Full-size",
  width: 22.5,
  height: 6.25,
  keys: [
    ...row(0, 0, "Escape _ F1 F2 F3 F4 _:0.5 F5 F6 F7 F8 _:0.5 F9 F10 F11 F12"),
    ...row(0, 15.25, "PrintScreen ScrollLock Pause"),
    ...row(1.25, 0, R_NUM),
    ...row(2.25, 0, R_TOP),
    ...row(3.25, 0, R_HOME),
    ...row(4.25, 0, R_LOW),
    ...row(5.25, 0, "LCtrl:1.25 LWin:1.25 LAlt:1.25 Space:6.25 RAlt:1.25 RWin:1.25 AppsKey:1.25 RCtrl:1.25"),
    ...row(1.25, 15.25, "Insert Home PgUp"),
    ...row(2.25, 15.25, "Delete End PgDn"),
    ...row(4.25, 16.25, "Up"),
    ...row(5.25, 15.25, "Left Down Right"),
    ...row(1.25, 18.5, "NumLock NumpadDiv NumpadMult NumpadSub"),
    ...row(2.25, 18.5, "Numpad7 Numpad8 Numpad9 NumpadAdd:1:2"),
    ...row(3.25, 18.5, "Numpad4 Numpad5 Numpad6"),
    ...row(4.25, 18.5, "Numpad1 Numpad2 Numpad3 NumpadEnter:1:2"),
    ...row(5.25, 18.5, "Numpad0:2 NumpadDot"),
  ],
  extras: [F13_24, MEDIA, BROWSER],
};

const laptop: Layout = {
  id: "laptop",
  name: "Laptop",
  width: 15,
  height: 6,
  keys: [
    ...row(0, 0, "Escape F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12 PrintScreen Insert Delete", 15 / 16),
    ...row(1, 0, R_NUM),
    ...row(2, 0, R_TOP),
    ...row(3, 0, R_HOME),
    ...row(4, 0, R_LOW),
    ...row(5, 0, "LCtrl:1.25 Fn LWin LAlt:1.25 Space:4.5 RAlt AppsKey RCtrl Left"),
    { id: "Up", x: 13, y: 5, w: 1, h: 0.5, label: "↑" },
    { id: "Down", x: 13, y: 5.5, w: 1, h: 0.5, label: "↓" },
    ...row(5, 14, "Right"),
  ],
  extras: [
    F13_24,
    { label: "Navigation & system", ids: ["Home", "End", "PgUp", "PgDn", "Pause", "ScrollLock", "NumLock"] },
    MEDIA,
    BROWSER,
    {
      label: "Numpad",
      ids: ["Numpad0", "Numpad1", "Numpad2", "Numpad3", "Numpad4", "Numpad5", "Numpad6", "Numpad7", "Numpad8", "Numpad9", "NumpadDot", "NumpadDiv", "NumpadMult", "NumpadSub", "NumpadAdd", "NumpadEnter"],
    },
  ],
};

const FN_ROW = ["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"];
const gk61Keys = [
  ...row(0, 0, "Escape D1 D2 D3 D4 D5 D6 D7 D8 D9 D0 Minus Equal Backspace:2"),
  ...row(1, 0, R_TOP),
  ...row(2, 0, R_HOME),
  ...row(3, 0, R_LOW),
  ...row(4, 0, "LCtrl:1.25 LWin:1.25 LAlt:1.25 Space:6.25 RAlt:1.25 Fn:1.25 AppsKey:1.25 RCtrl:1.25"),
];
// The number row doubles as F1–F12 with Fn.
["D1", "D2", "D3", "D4", "D5", "D6", "D7", "D8", "D9", "D0", "Minus", "Equal"].forEach((id, i) => {
  const key = gk61Keys.find((k) => k.id === id);
  if (key) key.sub = FN_ROW[i];
});

const gk61: Layout = {
  id: "gk61",
  name: "GK61 (60%)",
  width: 15,
  height: 5,
  keys: gk61Keys,
  extras: [
    { label: "Fn layer: keys your GK61 sends with Fn", ids: [...FN_ROW, "Backquote", "Up", "Down", "Left", "Right", "Delete", "Insert", "Home", "End", "PgUp", "PgDn", "PrintScreen", "ScrollLock", "Pause"] },
    F13_24,
    MEDIA,
  ],
  note: "On the GK61, Fn combinations are produced by the keyboard itself, so Windows sees the resulting key (for example Fn+1 arrives as F1). Map that key, or press the combination with Detect to see exactly what yours sends; layers you changed in the keyboard's own software are respected.",
};

export const LAYOUTS: Record<LayoutId, Layout> = { full, laptop, gk61 };
export const DEFAULT_LAYOUT: LayoutId = "full";

const STORAGE_KEY = "keyforge.layout";

export function loadLayoutId(): LayoutId {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v && v in LAYOUTS) return v as LayoutId;
  } catch {
    // Storage unavailable: use the default.
  }
  return DEFAULT_LAYOUT;
}

export function saveLayoutId(id: LayoutId) {
  try {
    localStorage.setItem(STORAGE_KEY, id);
  } catch {
    // Not persisted; the choice still applies for this session.
  }
}

/** Ids on the layout or in its extra groups. */
export function layoutIds(layout: Layout): Set<string> {
  return new Set([...layout.keys.map((k) => k.id), ...layout.extras.flatMap((g) => g.ids)].map((s) => s.toLowerCase()));
}
