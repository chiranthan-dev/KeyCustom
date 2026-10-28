// Physical layout of a typical Windows laptop keyboard (US ANSI). Widths are
// in key units. "Fn" is shown but cannot be selected: laptop firmware
// handles it and Windows never sees it.

export interface LayoutKey {
  id: string;
  w?: number;
  label?: string;
  disabled?: string;
}

export const MAIN_ROWS: LayoutKey[][] = [
  [
    { id: "Escape" },
    ...Array.from({ length: 12 }, (_, i) => ({ id: `F${i + 1}` })),
    { id: "PrintScreen", label: "PrtSc" },
    { id: "Insert", label: "Ins" },
    { id: "Delete", label: "Del" },
  ],
  [
    { id: "Backquote" },
    ...["D1", "D2", "D3", "D4", "D5", "D6", "D7", "D8", "D9", "D0"].map((id) => ({ id })),
    { id: "Minus" },
    { id: "Equal" },
    { id: "Backspace", w: 2, disabled: "Backspace is reserved for the emergency shortcut (Ctrl+Alt+Shift+Backspace)." },
  ],
  [
    { id: "Tab", w: 1.5 },
    ..."QWERTYUIOP".split("").map((id) => ({ id })),
    { id: "BracketLeft" },
    { id: "BracketRight" },
    { id: "Backslash", w: 1.5 },
  ],
  [
    { id: "CapsLock", w: 1.75, label: "Caps" },
    ..."ASDFGHJKL".split("").map((id) => ({ id })),
    { id: "Semicolon" },
    { id: "Quote" },
    { id: "Enter", w: 2.25 },
  ],
  [
    { id: "LShift", w: 2.25, label: "Shift" },
    ..."ZXCVBNM".split("").map((id) => ({ id })),
    { id: "Comma" },
    { id: "Period" },
    { id: "Slash" },
    { id: "RShift", w: 2.75, label: "Shift" },
  ],
  [
    { id: "LCtrl", w: 1.25, label: "Ctrl" },
    { id: "Fn", label: "Fn", disabled: "Fn is handled by the keyboard firmware and never reaches Windows, so no software can detect or remap it." },
    { id: "LWin", w: 1.25, label: "Win" },
    { id: "LAlt", w: 1.25, label: "Alt" },
    { id: "Space", w: 5.5 },
    { id: "RAlt", w: 1.25, label: "Alt" },
    { id: "AppsKey", label: "Menu" },
    { id: "RCtrl", w: 1.25, label: "Ctrl" },
    { id: "Left", label: "←" },
    { id: "Up", label: "↑" },
    { id: "Down", label: "↓" },
    { id: "Right", label: "→" },
  ],
];

export const EXTRA_GROUPS: { label: string; ids: string[] }[] = [
  { label: "Extra function keys", ids: Array.from({ length: 12 }, (_, i) => `F${i + 13}`) },
  { label: "Navigation & system", ids: ["Home", "End", "PgUp", "PgDn", "Pause", "ScrollLock", "NumLock"] },
  { label: "Media", ids: ["Media_Play_Pause", "Media_Prev", "Media_Next", "Media_Stop", "Volume_Mute", "Volume_Down", "Volume_Up"] },
  { label: "Browser & launch", ids: ["Browser_Back", "Browser_Forward", "Browser_Refresh", "Browser_Search", "Browser_Home", "Launch_Mail", "Launch_App1", "Launch_App2"] },
  {
    label: "Numpad",
    ids: ["Numpad0", "Numpad1", "Numpad2", "Numpad3", "Numpad4", "Numpad5", "Numpad6", "Numpad7", "Numpad8", "Numpad9", "NumpadDot", "NumpadDiv", "NumpadMult", "NumpadSub", "NumpadAdd", "NumpadEnter"],
  },
];
