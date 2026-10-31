import { describe, expect, it } from "vitest";
import { aFlowGap, tenGapRows } from "../test/builders";
import { allClear } from "./labels";

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
