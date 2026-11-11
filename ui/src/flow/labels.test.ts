import { describe, expect, it } from "vitest";
import { aFlowEdge, aFlowGap, aFlowNode, aFlowView, tenGapRows } from "../test/builders";
import { allClear } from "./labels";
import { en } from "../i18n/en";
import { ru } from "../i18n/ru";
import { enT, ruT } from "../test/text";
import * as flow from "./labels";

// AC-06 of docs/features/screen-fit.md: the rows have nothing to say only when each was measured at 0.

describe("allClear (AC-06)", () => {
  it("holds for every row measured at 0, whatever the number served", () => {
    expect(allClear(tenGapRows())).toBe(true);
    expect(allClear([aFlowGap("no-start")])).toBe(true);
  });

  it("holds for no list: no row served says no None", () => {
    expect(allClear([])).toBe(false);
  });

  const notNone = [
    ["a row found", { state: "measured", count: 1 }],
    ["an unknown state", { state: "weighed", count: 0 }],
    ["a null count", { state: "measured", count: null }],
    ["Not configured", { state: "not_configured", count: null }],
    ["Cannot measure", { state: "cannot", count: null }],
  ] as const;

  // The first row and the last: one row at 0, or the first alone, is not all of them.
  it.each(notNone.flatMap(([name, fields]) => ["no-start", "supply-unfed"].map((code) => [name, code, fields] as const)))("does not hold with %s at %s", (_name, code, fields) => {
    expect(allClear(tenGapRows([aFlowGap(code, [], fields)]))).toBe(false);
  });
});

// LG-3 of docs/features/ui-languages.md: under `ruT` every known value's label is ru.ts's value at its key (M9).
describe("the Flow's words in Russian (LG-3)", () => {
  it("means each gap code and words each gap state from ru.ts, another code nothing", () => {
    const keys = ["noStart", "twoStarts", "unreachable", "noWayToEnd", "trapCycle", "unjoinedParallel", "cycleNoRepeat", "definitionUnused", "definitionOnce", "supplyUnfed"] as const;
    expect(flow.GAP_CODES.map((code) => flow.gapMeaning(code, ruT))).toEqual(keys.map((key) => ru.flow.gapMeaning[key]));
    expect(flow.gapMeaning("odd-code", ruT)).toBeNull();
    expect(flow.gapLook(aFlowGap("no-start", [], { state: "measured", count: 0 }), ruT).label).toBe(ru.flow.gap.none);
    expect(flow.gapLook(aFlowGap("no-start", [], { state: "measured", count: 3 }), ruT).label).toBe(ru.flow.gap.found_few.replace("{{count}}", "3"));
    expect(flow.gapLook(aFlowGap("no-start", [], { state: "not_configured", count: null }), ruT).label).toBe(ru.flow.gap.notConfigured);
    expect(flow.gapLook(aFlowGap("no-start", [], { state: "cannot", count: null }), ruT).label).toBe(ru.flow.gap.cannotMeasure);
    expect(flow.gapLook(aFlowGap("no-start", [], { state: "paused", count: null }), ruT).label).toBe("paused");
    expect(flow.gapNotRead(ruT).label).toBe(ru.flow.gap.notRead);
  });

  it("labels each rollup and its meaning from ru.ts, an unknown one raw", () => {
    const keys = ["confirmed", "inWork", "suspect", "departed", "noCard"] as const;
    expect(flow.ROLLUP_WORDS.map((rollup) => flow.rollupLook(rollup, ruT).label)).toEqual(keys.map((key) => ru.flow.rollup[key]));
    expect(flow.rollupMeanings(ruT).map(({ meaning }) => meaning)).toEqual(keys.map((key) => ru.flow.rollupMeaning[key]));
    expect(flow.rollupLook("parked", ruT).label).toBe("parked");
    expect(flow.countParts({ confirmed: 1, in_work: 2, suspect: 5, departed: 0, no_card: 21 }, ruT).map((part) => part.words)).toEqual([
      ru.flow.count.confirmed_one.replace("{{count}}", "1"),
      ru.flow.count.inWork_few.replace("{{count}}", "2"),
      ru.flow.count.suspect_many.replace("{{count}}", "5"),
      ru.flow.count.noCard_one.replace("{{count}}", "21"),
    ]);
  });

  it("words lanes, starts, holdings and the lines from ru.ts, English unchanged", () => {
    expect([flow.stageWords(null, ruT), flow.stageWords("draft", ruT)]).toEqual([ru.flow.noStage, "draft"]);
    expect([flow.startWord(false, ruT), flow.startWord(true, ruT)]).toEqual([ru.flow.start, ru.flow.entry]);
    expect([flow.holdsText(1, ruT), flow.holdsText(3, ruT), flow.holdsText(5, ruT)]).toEqual([
      ru.flow.holds_one.replace("{{count}}", "1"),
      ru.flow.holds_few.replace("{{count}}", "3"),
      ru.flow.holds_many.replace("{{count}}", "5"),
    ]);
    expect([flow.lastReadWords(ruT), flow.statesHiddenWords(ruT)]).toEqual([ru.flow.lastRead, ru.flow.statesHidden]);
    expect([flow.notDrawnLine(2, 9, enT), flow.confirmedLine(1, 4, enT), flow.withGapsText(1, enT)]).toEqual([
      "2 of 9 boxes not drawn: choose a stage, enter a group, or read the List",
      "1 of 4 confirmed",
      "1 with gaps",
    ]);
  });
});

// docs/features/writer-studio.md: the studio's words and keys, the hand-off's title.
describe("the studio's words (writer-studio)", () => {
  const edge = { from: "A", to: "B", type: "alpha_link", kind: null, when: null, count: 1, path: "docs/a.md", line: 3 };

  it("keys an arrow by from, to, type and kind: one text, kind null apart from any kind", () => {
    expect(flow.edgeKeyOf({ ...edge, kind: "repeat" })).toEqual({ from: "A", to: "B", type: "alpha_link", kind: "repeat" });
    expect(flow.edgeKeyText(flow.edgeKeyOf(edge))).toBe(flow.edgeKeyText({ from: "A", to: "B", type: "alpha_link", kind: null }));
    expect(flow.edgeKeyText(flow.edgeKeyOf(edge))).not.toBe(flow.edgeKeyText({ from: "A", to: "B", type: "alpha_link", kind: "repeat" }));
  });

  it("words a List row `to <to title>, <type>[, <kind>][, when <when>][, \u00d7<count>]`, the name with no title, never `path:line` (E-1 of handoff-progress)", () => {
    expect(flow.edgeRowWords(edge, null, enT)).toBe("to B, alpha_link");
    expect(flow.edgeRowWords({ ...edge, kind: "alternative", when: "after dark", count: 3 }, "The berth", enT)).toBe("to The berth, alpha_link, alternative, when after dark, \u00d73");
    expect(flow.edgeRowWords({ ...edge, when: "x" }, null, ruT)).toBe(`${ru.flow.edgeRow.replace("{{to}}", "B")}, alpha_link, ${ru.flow.edgeWhen.replace("{{when}}", "x")}`);
    expect(flow.edgeRowWords(edge, "T", enT)).not.toContain("docs/a.md");
  });

  it("titles a gap's card by its code and members, cut past 120 characters to the first and a count", () => {
    expect(flow.handTitle("cycle-no-repeat", ["MEC-FAIRWAY", "MEC-PILOTAGE"], enT)).toBe("Gap cycle-no-repeat: MEC-FAIRWAY + MEC-PILOTAGE");
    const many = Array.from({ length: 12 }, (_, index) => `MEC-LONG-NAME-${String(index)}`);
    expect(flow.handTitle("trap-cycle", many, enT)).toBe("Gap trap-cycle: MEC-LONG-NAME-0 and 11 more");
    expect(flow.handTitle("trap-cycle", many, ruT)).toBe(ru.flow.handTitleCut_many.replace("{{code}}", "trap-cycle").replace("{{first}}", "MEC-LONG-NAME-0").replace("{{count}}", "11"));
  });

  it("reads `-` as No stage and any other asked value as served; a stage of 0 nodes says why", () => {
    expect([flow.askedStageWords("-", enT), flow.askedStageWords("quayside", enT)]).toEqual(["No stage", "quayside"]);
    expect([flow.emptyStageWords("-", enT), flow.emptyStageWords("quayside", enT)]).toEqual(["Every step here has a stage.", "No step here carries quayside."]);
  });

  it("says what a read left out, in the catalog's words; nothing, nothing", () => {
    expect(flow.leftOutWords({ generated: 1, tier3: 2 }, enT)).toBe("Left out: 1 generated, 2 archived");
    expect(flow.leftOutWords({ generated: 0, tier3: 0 }, enT)).toBeNull();
  });

  it("hands a measured row's items alone", () => {
    expect(flow.isMeasured(aFlowGap("cycle-no-repeat", [["A", "B"]]))).toBe(true);
    expect(flow.isMeasured(aFlowGap("no-start", [], { state: "not_configured", count: null }))).toBe(false);
  });
});

// flow-scenario FS-16, FS-17, FS-18 (RM-0094): the served types, no count on the canvas, the Gaps
// toggle's look, the records by kind.
describe("the scenario's words (FS-16 to FS-18)", () => {
  const types = ["first_link", "second_link", "third_link"];
  const edge = { from: "A", to: "B", type: "second_link", kind: null, when: null, count: 5, path: "docs/a.md", line: 3 };

  it("takes the types the read serves, in their order: a picture lacking a type dashes the others as one holding all three", () => {
    const lacking = aFlowView([aFlowNode({ id: "A", distance: 0 })], [aFlowEdge({ from: "A", to: "A", type: "third_link" })], { types });
    expect(flow.typesOf(lacking)).toEqual(types);
    expect(flow.typeColour("third_link", flow.typesOf(lacking))).toBe("var(--flow-edge-2)");
    expect(flow.typeDash("second_link", flow.typesOf(lacking))).toBe(flow.typeDash("second_link", types));
    // M: types from the read's edges would place the third type first.
    expect(flow.typeColour("third_link", flow.typesOf(lacking))).not.toBe("var(--flow-edge-0)");
  });

  it("puts no count on the canvas; the List keeps it", () => {
    expect(flow.edgeWords(edge)).toEqual([]);
    expect(flow.edgeWords({ ...edge, kind: "alternative", when: "cracked" })).toEqual(["alternative", "cracked"]);
  });

  // docs/features/handoff-progress.md "Acceptance criteria", E-1: an arrow's name reads forwards.
  it("names an arrow from its source's words to its target's, the type raw after them (E-1)", () => {
    // M: `from` and `to` swapped.
    expect(flow.edgeName(edge, "Kiln bay", "Glaze room", enT)).toBe("From Kiln bay to Glaze room, second_link");
    expect(flow.edgeName({ ...edge, kind: "repeat" }, "Kiln bay", "Glaze room", enT)).toBe("From Kiln bay to Glaze room, second_link, repeat");
    expect(flow.edgeName({ ...edge, kind: "alternative", when: "the glaze has crawled over the foot ring" }, "Kiln bay", "Glaze room", enT)).toBe(
      "From Kiln bay to Glaze room, second_link, alternative, the glaze has crawled ov\u2026",
    );
    // A box with no title gives its name.
    expect(flow.edgeName(edge, null, null, enT)).toBe("From A to B, second_link");
    expect(flow.edgeRowWords(edge, "Glaze room", enT)).toBe("to Glaze room, second_link, \u00d75");
  });

  it("names the closed edge kinds in the viewer's language, the canvas, the List and the name alike; an unknown kind raw", () => {
    const parallel = { ...edge, kind: "parallel" };
    expect(flow.edgeShownWords(parallel, ruT)).toEqual([ru.flow.edgeKind.parallel]);
    expect(flow.edgeRowWords(parallel, "Kiln bay", ruT)).toBe(`${ru.flow.edgeRow.replace("{{to}}", "Kiln bay")}, second_link, ${ru.flow.edgeKind.parallel}, \u00d75`);
    expect(flow.edgeName({ ...edge, kind: "repeat" }, "Kiln bay", "Glaze room", ruT)).toBe(`${ru.flow.edgeName.replace("{{from}}", "Kiln bay").replace("{{to}}", "Glaze room")}, second_link, ${ru.flow.edgeKind.repeat}`);
    expect(flow.FLOW_KINDS.map((kind) => flow.kindWords(kind, ruT))).toEqual([ru.flow.edgeKind.alternative, ru.flow.edgeKind.parallel, ru.flow.edgeKind.repeat]);
    expect(flow.kindWords("detour", ruT)).toBe("detour");
    expect(flow.edgeShownWords({ ...edge, kind: "alternative", when: "cracked" }, enT)).toEqual(["alternative", "cracked"]);
  });

  it("looks the Gaps toggle up: rows found, warning; any not measured, neutral; else None; a failed read, Not read", () => {
    const found = tenGapRows([aFlowGap("two-starts", [["A", "B"]]), aFlowGap("definition-once", [], { state: "not_configured", count: null })]);
    expect(flow.gapsLook(found, false, enT)).toEqual({ appearance: "warning", icon: "flowGap", label: "1 found" });
    expect(flow.gapsLook(tenGapRows([aFlowGap("definition-once", [], { state: "not_configured", count: null })]), false, enT)).toEqual({ appearance: "neutral", icon: "info", label: "not all measured" });
    expect(flow.gapsLook(tenGapRows(), false, enT)).toEqual({ appearance: "success", icon: "checkClean", label: "None" });
    expect(flow.gapsLook(found, true, enT).label).toBe("Not read");
    const sorted = flow.gapsSorted(found);
    expect([sorted.found.map((gap) => gap.code), sorted.unmeasured.map((gap) => gap.code), sorted.zero]).toEqual([["two-starts"], ["definition-once"], 8]);
  });

  it("lists the records a read leaves out of the flow: by raw kind, sorted, no kind last, served order; ID-less ones counted apart; the scope's own node never", () => {
    const nodes = [
      aFlowNode({ id: "SCOPE", distance: null, in_flow: false }),
      aFlowNode({ id: "Z-2", kind: "zeta", distance: null, in_flow: false }),
      aFlowNode({ id: "A-1", kind: "alpha", distance: null, in_flow: false }),
      aFlowNode({ id: "Z-1", kind: "zeta", distance: null, in_flow: false }),
      aFlowNode({ id: "N-1", kind: null, distance: null, in_flow: false }),
      aFlowNode({ id: null, kind: null, distance: null, path: "docs/notes/a.md", in_flow: false }),
      aFlowNode({ id: "STEP", kind: "alpha", distance: 0 }),
    ];
    const listed = flow.recordsOf(nodes, "SCOPE");
    expect(listed.groups.map((group) => [group.kind, group.nodes.map((node) => node.id)])).toEqual([
      ["alpha", ["A-1"]],
      ["zeta", ["Z-2", "Z-1"]],
      [null, ["N-1"]],
    ]);
    expect([listed.withId, listed.noId]).toEqual([4, 1]);
  });

  it("joins a divider's stages with a middle dot, raw", () => {
    expect(flow.dividerWords(["glazing"])).toBe("glazing");
    expect(flow.dividerWords(["one", "two"])).toBe("one · two");
  });
});

// docs/features/handoff-progress.md "Acceptance criteria", F-6: the words say left to right.
describe("the walk's words (F-6)", () => {
  it("says left to right in the legend's about text and the canvas's hint, never top or bottom; the Russian its own", () => {
    // M: the about text left at top to bottom.
    expect(en.flow.legend.about).toMatch(/^Left to right/);
    expect(en.flow.canvasHint).toContain("The walk runs left to right.");
    for (const words of [en.flow.legend.about, en.flow.canvasHint]) {
      expect(words).not.toMatch(/\btop\b|\bbottom\b/i);
    }
    expect(ru.flow.legend.about).not.toBe(en.flow.legend.about);
    expect(ru.flow.canvasHint).not.toBe(en.flow.canvasHint);
  });
});
