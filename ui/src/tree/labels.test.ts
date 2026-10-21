import { describe, expect, it } from "vitest";
import appCss from "../styles/app.css?raw";
import tokensCss from "../styles/tokens.css?raw";
import { isCodeLayer, LAYER_HEADINGS, layerHeading, syncLook } from "./labels";

// UI-2 of docs/features/bundle-bindings.md: a code row's sync is verify's state labelled, with its
// own colour role and icon; a row not judged and cannot_verify never look like a row in sync; any
// other word raw, neutral. Layers 5 and 9 hold rows, every other layer nodes.

/** verify's nine states, in its table's order (`docs/canon/spec-verify.md` "States"). */
const STATES = ["cannot_verify", "predated", "bad_marker", "broken_binding", "unlocked", "spec_ahead", "code_ahead", "conflict", "ok"];

describe("a binding row's sync (UI-2 of bundle-bindings)", () => {
  it("labels verify's nine states distinctly, each with its own tone and icon, none by its raw word", () => {
    const looks = STATES.map(syncLook);
    expect(looks.map((look) => look.label)).toEqual([
      "Cannot verify",
      "Predated",
      "Bad marker",
      "Broken binding",
      "Unlocked",
      "Spec ahead",
      "Code ahead",
      "Conflict",
      "In sync",
    ]);
    expect(new Set(looks.map((look) => look.tone)).size).toBe(9);
    expect(new Set(looks.map((look) => look.icon)).size).toBe(9);
    expect(looks.every((look) => look.tone.startsWith("sync-") && look.tone !== "sync-unknown")).toBe(true);
  });

  it("draws cannot_verify in cannot-verify's role and icon, as Health does, never ok's", () => {
    expect(syncLook("cannot_verify")).toEqual({ label: "Cannot verify", tone: "sync-cannot-verify", icon: "cannotCheck" });
    expect(syncLook("cannot_verify").tone).not.toBe(syncLook("ok").tone);
  });

  it("says a row was not judged when its sync is null: its own neutral role and icon, never ok's nor cannot-verify's", () => {
    const listed = syncLook(null);
    expect(listed).toEqual({ label: "Not judged", tone: "sync-not-judged", icon: "stub" });
    expect([listed.tone, listed.icon]).not.toEqual([syncLook("ok").tone, syncLook("ok").icon]);
    expect(listed.tone).not.toBe(syncLook("cannot_verify").tone);
  });

  it("shows any other word raw in a neutral badge, an own property name too", () => {
    expect(syncLook("drifting")).toEqual({ label: "drifting", tone: "sync-unknown", icon: "unknown" });
    expect(syncLook("OK").tone).toBe("sync-unknown");
    expect(syncLook("toString")).toEqual({ label: "toString", tone: "sync-unknown", icon: "unknown" });
  });

  it("gives every tone a badge rule in app.css and a colour role in tokens.css", () => {
    const tones = [...STATES.map((state) => syncLook(state).tone), syncLook(null).tone, syncLook("drifting").tone];
    for (const tone of tones) {
      expect([tone, appCss.includes(`.tone-${tone} { --badge-color: var(--${tone}); }`), tokensCss.includes(`--${tone}:`)]).toEqual([tone, true, true]);
    }
  });
});

describe("the layers of code rows (UI-2 of bundle-bindings)", () => {
  it("are layers 5 and 9 alone, keeping their CLI headings", () => {
    expect(LAYER_HEADINGS.filter(([key]) => isCodeLayer(key)).map(([key, heading]) => `${String(LAYER_HEADINGS.findIndex(([known]) => known === key) + 1)} ${heading}`)).toEqual([
      "5 Bindings",
      "9 Tests",
    ]);
    expect(["bindings", "tests"].map(layerHeading)).toEqual(["Bindings", "Tests"]);
  });

  it("knows no other key as one, an unknown or inherited name included", () => {
    expect(["targets", "criteria", "terms", "Bindings", "code", "constructor", ""].filter((key) => isCodeLayer(key))).toEqual([]);
  });
});
