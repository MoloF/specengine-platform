import { describe, expect, it } from "vitest";
import appCss from "../styles/app.css?raw";
import tokensCss from "../styles/tokens.css?raw";
import { enT } from "../test/text";
import { isCodeLayer, layerHeading, layerHeadings, syncLook } from "./labels";
import { ru } from "../i18n/ru";
import { ruT } from "../test/text";
import * as tree from "./labels";

const LAYER_HEADINGS = layerHeadings(enT);

// UI-2 of docs/features/bundle-bindings.md: a code row's sync is verify's state labelled, with its
// own colour role and icon; a row not judged and cannot_verify never look like a row in sync; any
// other word raw, neutral. Layers 5 and 9 hold rows, every other layer nodes.

/** verify's nine states, in its table's order (`docs/canon/spec-verify.md` "States"). */
const STATES = ["cannot_verify", "predated", "bad_marker", "broken_binding", "unlocked", "spec_ahead", "code_ahead", "conflict", "ok"];

describe("a binding row's sync (UI-2 of bundle-bindings)", () => {
  it("labels verify's nine states distinctly, each with its own tone and icon, none by its raw word", () => {
    const looks = STATES.map((state) => syncLook(state, enT));
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
    expect(syncLook("cannot_verify", enT)).toEqual({ label: "Cannot verify", tone: "sync-cannot-verify", icon: "cannotCheck" });
    expect(syncLook("cannot_verify", enT).tone).not.toBe(syncLook("ok", enT).tone);
  });

  it("says a row was not judged when its sync is null: its own neutral role and icon, never ok's nor cannot-verify's", () => {
    const listed = syncLook(null, enT);
    expect(listed).toEqual({ label: "Not judged", tone: "sync-not-judged", icon: "stub" });
    expect([listed.tone, listed.icon]).not.toEqual([syncLook("ok", enT).tone, syncLook("ok", enT).icon]);
    expect(listed.tone).not.toBe(syncLook("cannot_verify", enT).tone);
  });

  it("shows any other word raw in a neutral badge, an own property name too", () => {
    expect(syncLook("drifting", enT)).toEqual({ label: "drifting", tone: "sync-unknown", icon: "unknown", served: true });
    expect(syncLook("OK", enT).tone).toBe("sync-unknown");
    expect(syncLook("toString", enT)).toEqual({ label: "toString", tone: "sync-unknown", icon: "unknown", served: true });
  });

  it("gives every tone a badge rule in app.css and a colour role in tokens.css", () => {
    const tones = [...STATES.map((state) => syncLook(state, enT).tone), syncLook(null, enT).tone, syncLook("drifting", enT).tone];
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
    expect(["bindings", "tests"].map((key) => layerHeading(key, enT))).toEqual(["Bindings", "Tests"]);
  });

  it("knows no other key as one, an unknown or inherited name included", () => {
    expect(["targets", "criteria", "terms", "Bindings", "code", "constructor", ""].filter((key) => isCodeLayer(key))).toEqual([]);
  });
});

// LG-3 of docs/features/ui-languages.md: under `ruT` every known value's label is ru.ts's value at its key (M9).
describe("the tree's tables in Russian (LG-3)", () => {
  it("labels each link state, mark, form, direction, layer and sync state from ru.ts, an unknown one raw", () => {
    for (const state of tree.KNOWN_LINK_STATES) {
      expect([state, tree.linkStateLook(state, ruT).label]).toEqual([state, ru.tree.linkState[state]]);
    }
    expect(tree.KNOWN_TREE_MARKS.map((mark) => tree.markLook(mark, ruT).label)).toEqual([ru.tree.mark.danglingParent, ru.tree.mark.parentCycle]);
    for (const form of tree.KNOWN_FORMS) {
      expect([form, tree.formLabel(form, ruT)]).toEqual([form, ru.tree.form[form]]);
    }
    expect([tree.directionLabel("in", ruT), tree.directionLabel("out", ruT)]).toEqual([ru.tree.direction.in, ru.tree.direction.out]);
    for (const [key, heading] of tree.layerHeadings(ruT)) {
      expect([key, heading, tree.layerHeading(key, ruT)]).toEqual([key, ru.tree.layer[key], ru.tree.layer[key]]);
    }
    expect(tree.KNOWN_SYNC_STATES).toHaveLength(9);
    for (const sync of tree.KNOWN_SYNC_STATES) {
      expect([sync, tree.syncLook(sync, ruT).label]).toEqual([sync, ru.tree.sync[sync]]);
    }
    expect(tree.syncLook(null, ruT).label).toBe(ru.tree.notJudged);
    expect([tree.linkStateLook("lost", ruT).label, tree.markLook("orphan", ruT).label, tree.formLabel("digest", ruT), tree.directionLabel("both", ruT), tree.layerHeading("extra", ruT), tree.syncLook("stale", ruT).label]).toEqual([
      "lost",
      "orphan",
      "digest",
      "both",
      "extra",
      "stale",
    ]);
  });
});
