import { describe, expect, it } from "vitest";
import { chordLabel, defaultAction, describeAction, diffConfigs, isValidProcess, sameTrigger, slug, ACTION_TYPES } from "./model";
import type { Config } from "./types";

const base = (): Config => ({
  version: 1,
  activeProfile: "p",
  settings: { tapHoldMs: 200, doubleTapMs: 250, closeToTray: true, launchMappings: "restore", cheatSheetKey: "AppsKey" },
  profiles: [{ id: "p", name: "Work", autoApps: [], mappings: [], macros: [], appRules: [] }],
});

describe("model", () => {
  it("labels chords in a fixed modifier order", () => {
    expect(chordLabel({ key: "S", shift: true, ctrl: true })).toBe("Ctrl + Shift + S");
    expect(chordLabel({ key: "PgUp" }, [{ id: "PgUp", label: "Page Up", vk: 33, sc: 0x149, group: "navigation" }])).toBe("Page Up");
  });

  it("creates a valid default for every action type", () => {
    for (const { type } of ACTION_TYPES) {
      const action = defaultAction(type);
      expect(action.type).toBe(type);
      expect(describeAction(action).length).toBeGreaterThan(0);
    }
  });

  it("compares triggers case-insensitively with modifiers", () => {
    expect(sameTrigger({ key: "f13" }, { key: "F13", ctrl: false })).toBe(true);
    expect(sameTrigger({ key: "F13", ctrl: true }, { key: "F13" })).toBe(false);
  });

  it("makes unique slugs", () => {
    expect(slug("Deep Work!", [])).toBe("deep-work");
    expect(slug("Deep Work", ["deep-work"])).toBe("deep-work-2");
    expect(slug("***", [])).toBe("profile");
  });

  it("validates process names like the backend", () => {
    expect(isValidProcess("Code.exe")).toBe(true);
    expect(isValidProcess("C:\\x\\code.exe")).toBe(false);
    expect(isValidProcess("code")).toBe(false);
  });

  it("summarises changes for the preview", () => {
    const before = base();
    const after = base();
    after.profiles[0].mappings.push({ id: "m", enabled: true, trigger: { key: "F13" }, action: { type: "media", action: "mute" } });
    expect(diffConfigs(before, after)).toEqual(["Work: F13 → Mute (new)"]);
    const disabled = structuredClone(after);
    disabled.profiles[0].mappings[0].enabled = false;
    expect(diffConfigs(after, disabled)).toEqual(["Work: F13 disabled"]);
    expect(diffConfigs(after, before)).toEqual(["Work: F13 restored to normal"]);
    const renamed = base();
    renamed.profiles[0].name = "Focus";
    renamed.settings.tapHoldMs = 300;
    expect(diffConfigs(before, renamed)).toEqual(['Rename profile "Work" to "Focus"', "Settings changed"]);
  });
});

describe("templates", () => {
  it("have unique ids, known groups and exactly one of action or macro", async () => {
    const { TEMPLATES, TEMPLATE_GROUPS } = await import("./templates");
    const groups = new Set(TEMPLATE_GROUPS.map((g) => g.group));
    const ids = new Set<string>();
    for (const t of TEMPLATES) {
      expect(ids.has(t.id), t.id).toBe(false);
      ids.add(t.id);
      expect(groups.has(t.group), t.group).toBe(true);
      expect(!!t.action !== !!t.macro, t.id).toBe(true);
    }
  });
});

describe("cheat sheet", () => {
  it("lists app overrides first and hides what they override", async () => {
    const { cheatSheetRows } = await import("./model");
    const c = base();
    c.profiles[0].mappings = [
      { id: "g", enabled: true, trigger: { key: "F13" }, action: { type: "media", action: "mute" } },
      { id: "a", enabled: true, trigger: { key: "F13" }, action: { type: "block" }, app: "notepad.exe" },
      { id: "h", enabled: true, trigger: { key: "F14" }, action: { type: "media", action: "next" } },
      { id: "off", enabled: false, trigger: { key: "F15" }, action: { type: "media", action: "next" } },
    ];
    const inNotepad = cheatSheetRows(c, "p", "Notepad.exe");
    expect(inNotepad.rows.map((r) => [r.trigger.key, r.appOnly])).toEqual([["F13", true], ["F14", false]]);
    const elsewhere = cheatSheetRows(c, "p", "other.exe");
    expect(elsewhere.rows.map((r) => r.action)).toEqual(["Mute", "Next track"]);
    c.profiles[0].appRules = [{ id: "r", process: "game.exe", mode: "disable", enabled: true }];
    const inGame = cheatSheetRows(c, "p", "game.exe");
    expect(inGame.globalOff).toBe(true);
    expect(inGame.rows).toEqual([]);
  });
});

describe("layouts", () => {
  it("only use known key ids and do not overlap", async () => {
    const { LAYOUTS } = await import("./layout");
    // The Rust key table is the source of truth for key ids.
    const { default: keysRs } = await import("../../crates/core/src/keys.rs?raw");
    const table = [...keysRs.matchAll(/^\s*k\("([^"]+)"/gm)].map((m) => m[1]);
    expect(table.length).toBeGreaterThan(100);
    const known = new Set([...table, "Fn"].map((s) => s.toLowerCase()));
    for (const layout of Object.values(LAYOUTS)) {
      for (const k of layout.keys) {
        expect(known.has(k.id.toLowerCase()), `${layout.id}: ${k.id}`).toBe(true);
        expect(k.x + k.w, `${layout.id}: ${k.id} fits`).toBeLessThanOrEqual(layout.width + 1e-9);
        expect(k.y + k.h, `${layout.id}: ${k.id} fits`).toBeLessThanOrEqual(layout.height + 1e-9);
      }
      for (const a of layout.keys)
        for (const b of layout.keys)
          if (a !== b) {
            const overlap = a.x < b.x + b.w - 1e-9 && b.x < a.x + a.w - 1e-9 && a.y < b.y + b.h - 1e-9 && b.y < a.y + a.h - 1e-9;
            expect(overlap, `${layout.id}: ${a.id} overlaps ${b.id}`).toBe(false);
          }
      for (const g of layout.extras) for (const id of g.ids) expect(known.has(id.toLowerCase()), `${layout.id} extra ${id}`).toBe(true);
    }
    expect(LAYOUTS.gk61.keys.length).toBe(61);
  });
});
