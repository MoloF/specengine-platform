import { describe, expect, it } from "vitest";
import { aProposal } from "../test/builders";
import { fold, matchesFilter } from "./filter";

// The north basin's name in the harbor-sim mock, built from code points so the source stays ASCII.
const ZE = [0x437, 0x435, 0x43b];
const DECOMPOSED = String.fromCodePoint(0x417, 0x435, 0x43b, 0x435, 0x308, 0x43d, 0x44b, 0x438, 0x306);
const COMPOSED_QUERY = String.fromCodePoint(...ZE, 0x451, 0x43d, 0x44b, 0x439);

describe("the filter (AC-18)", () => {
  it("matches a composed query on decomposed text, ignoring case", () => {
    expect(DECOMPOSED).not.toBe(DECOMPOSED.normalize("NFC"));
    const proposal = aProposal({ id: "PR-1", summary: `Keep "${DECOMPOSED}" in plans?` });
    expect(matchesFilter(proposal, COMPOSED_QUERY)).toBe(true);
    expect(matchesFilter(proposal, COMPOSED_QUERY.normalize("NFD").toUpperCase())).toBe(true);
    expect(fold(DECOMPOSED)).toBe(fold(COMPOSED_QUERY.toUpperCase()));
  });

  it("searches ID, summary, kind, task and targets; an empty query matches all", () => {
    const proposal = aProposal({ id: "PR-42", kind: "discrepancy", task_id: "T-7", target_ids: ["RULE-X"], summary: "Tide" });
    for (const query of ["pr-42", "tide", "DISCREP", "t-7", "rule-x", "  "]) {
      expect(matchesFilter(proposal, query)).toBe(true);
    }
    expect(matchesFilter(proposal, "berth")).toBe(false);
  });
});
