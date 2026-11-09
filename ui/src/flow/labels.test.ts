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

  it("words lanes, steps, starts, holdings and the lines from ru.ts, English unchanged", () => {
    expect([flow.stageWords(null, ruT), flow.stageWords("draft", ruT)]).toEqual([ru.flow.noStage, "draft"]);
    expect([flow.stepWords(null, ruT), flow.stepWords(2, ruT)]).toEqual([ru.flow.stepUnknown, ru.flow.step.replace("{{n}}", "3")]);
    expect([flow.startWord(false, ruT), flow.startWord(true, ruT)]).toEqual([ru.flow.start, ru.flow.entry]);
    expect([flow.holdsText(1, ruT), flow.holdsText(3, ruT), flow.holdsText(5, ruT)]).toEqual([
      ru.flow.holds_one.replace("{{count}}", "1"),
      ru.flow.holds_few.replace("{{count}}", "3"),
      ru.flow.holds_many.replace("{{count}}", "5"),
    ]);
    expect([flow.noCardsYet(ruT), flow.lastReadWords(ruT), flow.statesHiddenWords(ruT), flow.noStagesYet(ruT)]).toEqual([ru.flow.noCardsYet, ru.flow.lastRead, ru.flow.statesHidden, ru.flow.noStagesYet]);
    expect([flow.readCount({ nodes: [], edges: [] }, enT), flow.notDrawnLine(2, 9, enT), flow.confirmedLine(1, 4, enT), flow.withGapsText(1, enT)]).toEqual([
      "0 boxes, 0 edges",
      "2 of 9 boxes not drawn: close a group, or read the List",
      "1 of 4 confirmed",
      "1 with gaps",
    ]);
  });
});
