import { describe, expect, it } from "vitest";
import { aFlowGap, tenGapRows } from "../test/builders";
import { allClear } from "./labels";
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

  it("words a List row `<type> <to>, <title>[, <kind>][, when <when>][, ×<count>]`, never `path:line`", () => {
    expect(flow.edgeRowWords(edge, null, enT)).toBe("alpha_link B");
    expect(flow.edgeRowWords({ ...edge, kind: "alternative", when: "after dark", count: 3 }, "The berth", enT)).toBe("alpha_link B, The berth, alternative, when after dark, ×3");
    expect(flow.edgeRowWords({ ...edge, when: "x" }, null, ruT)).toBe(`alpha_link B, ${ru.flow.edgeWhen.replace("{{when}}", "x")}`);
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
