import { describe, expect, it, vi } from "vitest";
import { aCheckFinding, anEntry } from "../test/builders";
import {
  budgetRowsOf,
  codeChip,
  codeChipsOf,
  debtRowsOf,
  groupsOf,
  isSpecPath,
  NO_FILTERS,
  rowsOf,
  searchableRows,
  severityChip,
  severityChipsOf,
  severityCountsOf,
  toggled,
  visibleRows,
  worstRank,
} from "./findings";

// docs/features/ui-health.md AC-06 (groups), AC-07 (which paths link), AC-08 (debt order,
// budgets), AC-05 (severity counts sum to the queue): arranged, never judged.

const FINDINGS = [
  aCheckFinding({ code: "id-width", severity: "warning", path: "docs/a.md", line: 3 }),
  aCheckFinding({ code: "ref-dangling", severity: "error", path: "docs/b.md", line: 1 }),
  aCheckFinding({ code: "zeta", severity: "triage", path: "docs/c.md", line: 2 }),
  aCheckFinding({ code: "ref-dangling", severity: "warning", path: "docs/d.md", line: 4 }),
  aCheckFinding({ code: "budget", severity: "error", path: "docs/canon/x.md", line: 1, subject: "canon", message: "12950 bytes, over the canon cap of 12288" }),
  aCheckFinding({ code: "id-width", severity: "warning", path: "docs/e.md", line: 9 }),
  aCheckFinding({ code: "ref-dangling", severity: "error", path: "docs/f.md", line: 5 }),
];

describe("groups (AC-06)", () => {
  it("by code, a group holding an error first, by code within a rank; unknown severities last", () => {
    const groups = groupsOf(rowsOf(FINDINGS));
    expect(groups.map((group) => group.code)).toEqual(["budget", "ref-dangling", "id-width", "zeta"]);
    expect(groups.reduce((total, group) => total + group.rows.length, 0)).toBe(FINDINGS.length);
  });

  it("errors first within a group, the report's order kept among equals", () => {
    const dangling = groupsOf(rowsOf(FINDINGS)).find((group) => group.code === "ref-dangling");
    expect(dangling?.rows.map((row) => [row.key, row.finding.severity])).toEqual([
      [1, "error"],
      [6, "error"],
      [3, "warning"],
    ]);
  });

  it("chips count over the whole report: severities errors first, an unknown one raw and last; codes in the groups' order", () => {
    const rows = rowsOf(FINDINGS);
    expect(severityChipsOf(rows).map((chip) => [chip.label, chip.count])).toEqual([
      ["Error", 3],
      ["Warning", 3],
      ["triage", 1],
    ]);
    expect(codeChipsOf(rows).map((chip) => [chip.label, chip.count])).toEqual([
      ["budget", 1],
      ["ref-dangling", 3],
      ["id-width", 2],
      ["zeta", 1],
    ]);
  });

  it("filters by chip and by text, NFC and lower case, over path, subject, message and code", () => {
    const rows = searchableRows(rowsOf([
      ...FINDINGS,
      aCheckFinding({ code: "ref-dangling", subject: "Zel\u00ebn", message: "x" }),
    ]));
    const shown = (filters = NO_FILTERS) => visibleRows(rows, filters).map((row) => row.key);
    expect(shown()).toHaveLength(8);
    expect(shown(toggled(NO_FILTERS, severityChip("warning")))).toEqual([1, 2, 4, 6, 7]);
    expect(shown(toggled(NO_FILTERS, codeChip("ref-dangling")))).toEqual([0, 2, 4, 5]);
    expect(shown(toggled(toggled(NO_FILTERS, codeChip("zeta")), codeChip("zeta")))).toHaveLength(8);
    expect(shown({ ...NO_FILTERS, query: "ZELE\u0308N" })).toEqual([7]);
    expect(shown({ ...NO_FILTERS, query: "zelen" })).toEqual([]);
    expect(shown({ ...NO_FILTERS, query: " CANON/X " })).toEqual([4]);
  });
});

describe("the text filter's cost", () => {
  it("folds each row's text once per report and the query once per filtering, whatever the rows", () => {
    const many = Array.from({ length: 500 }, (_, index) => aCheckFinding({ code: "c", path: `docs/r${String(index)}.md`, message: "m" }));
    const rows = searchableRows(rowsOf(many));
    const normalize = vi.spyOn(String.prototype, "normalize");
    expect(visibleRows(rows, { ...NO_FILTERS, query: "R499" }).map((row) => row.key)).toEqual([499]);
    expect(normalize).toHaveBeenCalledTimes(1);
  });

  it("finds the worst severity of a million rows without a spread's argument limit", () => {
    const one = aCheckFinding({ code: "c", severity: "warning" });
    const rows = Array.from({ length: 1_000_000 }, (_, key) => ({ key, finding: key === 999_999 ? aCheckFinding({ code: "c" }) : one }));
    expect(worstRank(rows)).toBe(0);
    expect(worstRank(rows.slice(0, 10))).toBe(1);
  });
});

describe("debt and budgets (AC-08)", () => {
  it("debt: expired first, then by the stored expiry as written, then the report's order", () => {
    const rows = rowsOf([
      aCheckFinding({ code: "a", debt: { reason: "r", expires: "2027-01-01", expired: false } }),
      aCheckFinding({ code: "b" }),
      aCheckFinding({ code: "c", debt: { reason: "r", expires: "2026-12-31", expired: false } }),
      aCheckFinding({ code: "d", debt: { reason: "r", expires: "2026-12-31", expired: true } }),
      aCheckFinding({ code: "e", debt: { reason: "r", expires: "2025-01-01", expired: false } }),
      aCheckFinding({ code: "f", debt: { reason: "r", expires: "2026-12-31", expired: false } }),
    ]);
    expect(debtRowsOf(rows).map((row) => row.finding.code)).toEqual(["d", "e", "c", "f", "a"]);
  });

  it("budgets: exactly the `budget` findings, whatever their message says", () => {
    const rows = rowsOf([
      ...FINDINGS,
      aCheckFinding({ code: "budget-ish", message: "99 bytes, over the tier1 cap of 10" }),
      aCheckFinding({ code: "budget", message: "a message with no number" }),
    ]);
    expect(budgetRowsOf(rows).map((row) => row.key)).toEqual([4, 8]);
  });
});

describe("paths (AC-07)", () => {
  it("links only an .md path", () => {
    expect([isSpecPath("docs/a.md"), isSpecPath(""), isSpecPath("specengine.toml"), isSpecPath("docs/a.md.bak")]).toEqual([
      true,
      false,
      false,
      false,
    ]);
  });
});

describe("the queue's severities (AC-05)", () => {
  it("count each severity once, null apart, in the Inbox's order, summing to the queue", () => {
    const queue = [
      anEntry({ id: "PR-1", severity: "low" }),
      anEntry({ id: "PR-2", severity: "high" }),
      anEntry({ id: "PR-3", severity: null }),
      anEntry({ id: "PR-4", severity: "urgent" }),
      anEntry({ id: "PR-5", severity: "high" }),
      anEntry({ id: "PR-6", severity: "normal" }),
    ];
    const counts = severityCountsOf(queue);
    expect(counts).toEqual([
      { severity: "high", count: 2 },
      { severity: "normal", count: 1 },
      { severity: "low", count: 1 },
      { severity: null, count: 1 },
      { severity: "urgent", count: 1 },
    ]);
    expect(counts.reduce((total, { count }) => total + count, 0)).toBe(queue.length);
  });
});
