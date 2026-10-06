import { describe, expect, it } from "vitest";
import { chipsOf, DEFAULT_SETTINGS, graphOptions, patternOrder, toggleChip, typesToSend, withBase, withMode, type GraphSettings } from "./settings";

// The Controls of docs/features/ui-graph.md: chips from the mode's latest unfiltered answer, in its
// order, then `mentions` released; all but `mentions` pressed sends no `types`; the last pressed
// one stays; a mode switch resets them; depth 2 by default, All sends none.

const BASE = [
  { type: "zeta_type", direction: "out" },
  { type: "alpha_type", direction: "in" },
];

function learned(): GraphSettings {
  return withBase(DEFAULT_SETTINGS, false, BASE);
}

describe("the type chips", () => {
  it("are the base in its order, then mentions released; none before an answer", () => {
    expect(chipsOf(DEFAULT_SETTINGS, [])).toEqual([]);
    expect(chipsOf(learned(), [])).toEqual([
      { type: "zeta_type", direction: "out", pressed: true },
      { type: "alpha_type", direction: "in", pressed: true },
      { type: "mentions", direction: null, pressed: false },
    ]);
  });

  it("send no types with all but mentions pressed, else the pressed ones in chip order", () => {
    const settings = learned();
    expect(typesToSend(settings)).toBeUndefined();
    const withMentions = toggleChip(settings, "mentions");
    expect(typesToSend(withMentions)).toEqual(["zeta_type", "alpha_type", "mentions"]);
    expect(typesToSend(toggleChip(withMentions, "zeta_type"))).toEqual(["alpha_type", "mentions"]);
    expect(typesToSend(toggleChip(settings, "alpha_type"))).toEqual(["zeta_type"]);
  });

  it("keep the last pressed one pressed", () => {
    const one = toggleChip(learned(), "alpha_type");
    expect(toggleChip(one, "zeta_type")).toBe(one);
  });

  it("go back to the mode's default on a mode switch; the same mode changes nothing", () => {
    const changed = toggleChip(toggleChip(learned(), "mentions"), "zeta_type");
    const impact = withMode(changed, true);
    expect([impact.impact, impact.released, impact.mentions]).toEqual([true, [], false]);
    expect(withMode(changed, false)).toBe(changed);
  });

  it("learn a base once; the same base again keeps the settings", () => {
    const settings = learned();
    expect(withBase(settings, false, BASE)).toBe(settings);
    expect(withBase(settings, true, BASE).bases.impact).toEqual(BASE);
  });
});

describe("the pattern order", () => {
  it("is the mode's chip order, the same whichever chips the answer followed", () => {
    const settings = learned();
    const order = ["zeta_type", "alpha_type", "mentions"];
    expect(patternOrder(settings.bases, false, BASE)).toEqual(order);
    expect(patternOrder(settings.bases, false, [{ type: "alpha_type", direction: "in" }])).toEqual(order);
    expect(patternOrder(settings.bases, false, [{ type: "mentions", direction: "out" }])).toEqual(order);
  });

  it("follows the answer while the mode has no chips yet, then mentions; a type the chips lack goes last", () => {
    expect(patternOrder(DEFAULT_SETTINGS.bases, true, BASE)).toEqual(["zeta_type", "alpha_type", "mentions"]);
    expect(patternOrder(learned().bases, false, [{ type: "beta_type", direction: "out" }])).toEqual(["zeta_type", "alpha_type", "mentions", "beta_type"]);
  });
});

describe("the read", () => {
  it("asks depth 2 by default, omits what is absent", () => {
    expect(graphOptions("R", DEFAULT_SETTINGS)).toEqual({ ref: "R", depth: 2 });
    expect(graphOptions("R", { ...learned(), depth: null, impact: true, archive: true })).toEqual({ ref: "R", impact: true, archive: true });
  });
});
