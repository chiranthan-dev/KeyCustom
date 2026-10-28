import { describe, expect, it } from "vitest";
import { chordLabel, defaultAction, describeAction, diffConfigs, isValidProcess, sameTrigger, slug, ACTION_TYPES } from "./model";
import type { Config } from "./types";

const base = (): Config => ({
  version: 1,
  activeProfile: "p",
  settings: { tapHoldMs: 200, doubleTapMs: 250, closeToTray: true, enableOnLaunch: false },
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
