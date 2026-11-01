// Ready-made mappings and macros, shared with the Rust tests through
// templates.json (crates/core validates and generates every one of them).
// Choosing a template adds it to a key in the current draft; nothing is
// applied until the user clicks Apply.
import data from "./templates.json";
import type { Action, Chord, Step } from "./types";

export type TemplateGroup = "Windows" | "Editing" | "Browser" | "Text & dates" | "Routines" | "Media" | "Gaming";

export interface Template {
  id: string;
  group: TemplateGroup;
  name: string;
  description: string;
  /** Key suggested when the template is used. */
  key?: Chord;
  /** Single action, or a macro that is created alongside the mapping. */
  action?: Action;
  macro?: { name: string; steps: Step[] };
  /** Ask the user to fill in fields (text, address, website) before adding. */
  personalize?: string;
}

export const TEMPLATE_GROUPS = data.groups as { group: TemplateGroup; blurb: string }[];
export const TEMPLATES = data.templates as Template[];
