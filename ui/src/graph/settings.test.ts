import { describe, expect, it } from "vitest";
import { chipsOf, DEFAULT_SETTINGS, graphOptions, modeOf, patternOrder, toggleChip, typesToSend, withBase, withMode, type GraphSettings } from "./settings";

// The Controls of docs/features/ui-graph.md: chips from the mode's latest unfiltered answer, in its
// order, then `mentions` released; all but `mentions` pressed sends no `types`; the last pressed
// one stays; a mode switch resets them; depth 2 by default, All sends none.

const BASE = [
  { type: "zeta_type", direction: "out" },
  { type: "alpha_type", direction: "in" },
];

function learned(): GraphSettings {
  return withBase(DEFAULT_SETTINGS, "out", BASE);
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
    const impact = withMode(changed, "impact");
    expect([impact.mode, impact.released, impact.mentions]).toEqual(["impact", [], false]);
    expect(withMode(changed, "out")).toBe(changed);
  });

  it("learn a base once; the same base again keeps the settings", () => {
    const settings = learned();
    expect(withBase(settings, "out", BASE)).toBe(settings);
    expect(withBase(settings, "impact", BASE).bases.impact).toEqual(BASE);
  });
});

describe("the pattern order", () => {
  it("is the mode's chip order, the same whichever chips the answer followed", () => {
    const settings = learned();
    const order = ["zeta_type", "alpha_type", "mentions"];
    expect(patternOrder(settings.bases, "out", BASE)).toEqual(order);
    expect(patternOrder(settings.bases, "out", [{ type: "alpha_type", direction: "in" }])).toEqual(order);
    expect(patternOrder(settings.bases, "out", [{ type: "mentions", direction: "out" }])).toEqual(order);
  });

  it("follows the answer while the mode has no chips yet, then mentions; a type the chips lack goes last", () => {
    expect(patternOrder(DEFAULT_SETTINGS.bases, "impact", BASE)).toEqual(["zeta_type", "alpha_type", "mentions"]);
    expect(patternOrder(learned().bases, "out", [{ type: "beta_type", direction: "out" }])).toEqual(["zeta_type", "alpha_type", "mentions", "beta_type"]);
  });
});

describe("the read", () => {
  it("asks depth 2 by default, omits what is absent", () => {
    expect(graphOptions("R", DEFAULT_SETTINGS)).toEqual({ ref: "R", depth: 2 });
    expect(graphOptions("R", { ...learned(), depth: null, mode: "impact", archive: true })).toEqual({ ref: "R", impact: true, archive: true });
  });
});

// docs/features/corpus-map.md "Acceptance criteria", CM10: the local graph's Both ways mode. Its read
// sends `both`, never `impact`; its chips name each (type, direction) pair as its answer gives them,
// a chip pressing or releasing the type; each mode keeps its own base; the home's view defaults to
// the Map. M: Both ways sending `impact`.
describe("Both ways (CM10)", () => {
  const BOTH = [
    { type: "zeta_type", direction: "out" },
    { type: "zeta_type", direction: "in" },
    { type: "alpha_type", direction: "out" },
    { type: "alpha_type", direction: "in" },
  ];

  it("sends both=true and never impact; Links sends neither (M: Both ways sending impact)", () => {
    const both = withMode(DEFAULT_SETTINGS, "both");
    expect(graphOptions("R", both)).toEqual({ ref: "R", both: true, depth: 2 });
    expect(graphOptions("R", both).impact).toBeUndefined();
    expect(graphOptions("R", DEFAULT_SETTINGS)).toEqual({ ref: "R", depth: 2 });
  });

  it("names each (type, direction) pair as a chip, in the answer's order, then mentions; a chip presses its type's pairs together", () => {
    const both = withBase(withMode(DEFAULT_SETTINGS, "both"), "both", BOTH);
    expect(chipsOf(both, []).map((chip) => `${chip.type} ${String(chip.direction)} ${String(chip.pressed)}`)).toEqual([
      "zeta_type out true",
      "zeta_type in true",
      "alpha_type out true",
      "alpha_type in true",
      "mentions null false",
    ]);
    const released = toggleChip(both, "zeta_type");
    expect(chipsOf(released, []).filter((chip) => !chip.pressed).map((chip) => `${chip.type} ${String(chip.direction)}`)).toEqual(["zeta_type out", "zeta_type in", "mentions null"]);
    expect(typesToSend(released)).toEqual(["alpha_type"]);
    // The last type pressed stays, both its pairs.
    expect(toggleChip(released, "alpha_type")).toBe(released);
    expect(patternOrder(both.bases, "both", BOTH)).toEqual(["zeta_type", "alpha_type", "mentions"]);
  });

  it("keeps each mode's base apart: Both ways learns its own, Links and Impact keep theirs", () => {
    const out = withBase(DEFAULT_SETTINGS, "out", BASE);
    const impact = withBase(withMode(out, "impact"), "impact", [{ type: "depends_on", direction: "in" }]);
    const both = withBase(withMode(impact, "both"), "both", BOTH);
    expect(both.bases).toEqual({ out: BASE, impact: [{ type: "depends_on", direction: "in" }], both: BOTH });
    expect(chipsOf(withMode(both, "out"), []).map((chip) => chip.type)).toEqual(["zeta_type", "alpha_type", "mentions"]);
    expect(chipsOf(withMode(both, "impact"), []).map((chip) => chip.type)).toEqual(["depends_on", "mentions"]);
  });

  it("reads an answer's mode from its `both` and `impact`", () => {
    expect([modeOf({ both: false, impact: false }), modeOf({ both: true, impact: false }), modeOf({ both: false, impact: true })]).toEqual(["out", "both", "impact"]);
  });

  it("opens the home on the Map", () => {
    expect(DEFAULT_SETTINGS.home).toBe("map");
  });
});
