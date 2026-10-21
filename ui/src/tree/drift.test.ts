import { describe, expect, it, vi } from "vitest";
import type { DriftRow, HealthDrift, HealthReport } from "../api/types";
import { driftBadgeLooks } from "../health/labels";
import { aDriftIn, aDriftOff, aDriftRow, aHealthReport, aTreeNode, measuredDrift } from "../test/builders";
import { TREE } from "../test/treeStub";
import { driftOf, driftWords, measuredRows, recallShowDrift, rememberShowDrift, treeDrift, type NodeDrift } from "./drift";
import { buildRows, type TreeRow } from "./rows";

// UI-1 of docs/features/ui-tree-drift.md: Health's row 6 rows grouped by `node` as sent, matched to
// the tree's lines by `namesOf` (`<ID>`, `<stem>/<ID>`, an ID-less document's path) as the inbox
// counter matches; each counted on its own line only, never on an ancestor's; two holders each
// count; `""`, an unresolved ID and a look-alike in `unmatched`; an unlisted class apart; an
// unchanged line keeps its object; `measuredRows`. UI-6: the badges' looks. UI-7: the storage.

const ROWS = buildRows(TREE);

/** The tree line at `path:line`. */
function at(place: string): TreeRow {
  const found = ROWS.find((row) => row.key === place);
  if (found === undefined) {
    throw new Error(`no line ${place}`);
  }
  return found;
}

const DOC_A = at("docs/spec/a.md:1");
const SEC_A1 = at("docs/spec/a.md:12");
const SEC_A1X = at("docs/spec/a.md:16");
const DUP_IN_A = at("docs/spec/a.md:30");
const B_DOC = at("docs/spec/b.md:1");
const DUP_IN_B = at("docs/spec/b.md:9");
const DOC_C = at("docs/spec/c.md:1");

function counts(fields: Partial<NodeDrift>): NodeDrift {
  return { fix: 0, check: 0, cannot: 0, unlisted: 0, ...fields };
}

/** Each tree line's counts by its key, in tree order. */
function shown(byRow: ReadonlyMap<string, NodeDrift>): [string, NodeDrift][] {
  return ROWS.flatMap((row) => {
    const own = byRow.get(row.key);
    return own === undefined ? [] : [[row.key, own] as [string, NodeDrift]];
  });
}

describe("row 6 over the tree's lines (UI-1)", () => {
  it("counts each line's own rows by class; a section's rows never on its parent or an ancestor", () => {
    const drift = treeDrift(
      ROWS,
      [
        aDriftRow({ node: "DOC-A", class: "fix", state: "conflict" }),
        aDriftRow({ node: "DOC-A", class: "check" }),
        aDriftRow({ node: "DOC-A", class: "check", relation: "verifies" }),
        aDriftRow({ node: "SEC-A1", class: "cannot", state: "cannot_verify" }),
        aDriftRow({ node: "SEC-A1X", class: "check" }),
      ],
      null,
    );
    expect(shown(drift.byRow)).toEqual([
      [DOC_A.key, counts({ fix: 1, check: 2 })],
      [SEC_A1.key, counts({ cannot: 1 })],
      [SEC_A1X.key, counts({ check: 1 })],
    ]);
    expect(drift.unmatched).toBe(0);
  });

  it("matches by ID, by <stem>/<ID> and an ID-less document by its path; two holders of an ID each count it", () => {
    const drift = treeDrift(
      ROWS,
      [
        aDriftRow({ node: "a/SEC-A1", class: "fix" }),
        aDriftRow({ node: "docs/spec/b.md", class: "check" }),
        aDriftRow({ node: "SEC-DUP", class: "check" }),
        aDriftRow({ node: "b/SEC-DUP", class: "fix" }),
      ],
      null,
    );
    expect(shown(drift.byRow)).toEqual([
      [SEC_A1.key, counts({ fix: 1 })],
      [DUP_IN_A.key, counts({ check: 1 })],
      [B_DOC.key, counts({ check: 1 })],
      [DUP_IN_B.key, counts({ check: 1, fix: 1 })],
    ]);
    expect(drift.unmatched).toBe(0);
  });

  it("counts row by row those naming no line: a bare marker, an unresolved ID, a look-alike as written, another file's stem", () => {
    const drift = treeDrift(
      ROWS,
      [
        aDriftRow({ node: "", class: "fix", state: "bad_marker" }),
        aDriftRow({ node: "", class: "fix", state: "bad_marker", file: "src/other.rs" }),
        aDriftRow({ node: "R-404", class: "fix", state: "broken_binding" }),
        aDriftRow({ node: "DOC-\u0410", class: "fix", state: "bad_marker", category: "homoglyph" }),
        aDriftRow({ node: "b/SEC-A1", class: "check" }),
        aDriftRow({ node: "DOC-A", class: "check" }),
      ],
      null,
    );
    expect(shown(drift.byRow)).toEqual([[DOC_A.key, counts({ check: 1 })]]);
    expect(drift.unmatched).toBe(5);
  });

  it("takes `node` as sent: no trimming, case folding or normal form; a `node` that is no string unmatched, never thrown on", () => {
    const odd = { ...aDriftRow({ node: "DOC-A" }), node: 7 } as unknown as DriftRow;
    const drift = treeDrift(
      ROWS,
      [aDriftRow({ node: "doc-a" }), aDriftRow({ node: " DOC-A" }), aDriftRow({ node: "DOC-A\n" }), odd, aDriftRow({ node: "DOC-A", class: "fix" })],
      null,
    );
    expect(shown(drift.byRow)).toEqual([[DOC_A.key, counts({ fix: 1 })]]);
    expect(drift.unmatched).toBe(4);
  });

  it("names no line `\"\"`: a malformed line with an empty ID never takes the bare markers", () => {
    const lines = buildRows([aTreeNode({ id: "", depth: 0, path: "docs/spec/odd.md" })]);
    const drift = treeDrift(lines, [aDriftRow({ node: "", class: "fix" })], null);
    expect(drift.byRow.size).toBe(0);
    expect(drift.unmatched).toBe(1);
  });

  it("counts a class row 6 does not list, or none, apart from the three, never as one of them", () => {
    const drift = treeDrift(ROWS, [aDriftRow({ node: "DOC-A", class: "advisory" }), aDriftRow({ node: "DOC-A", class: null }), aDriftRow({ node: "DOC-A", class: "toString" })], null);
    expect(drift.byRow.get(DOC_A.key)).toEqual(counts({ unlisted: 3 }));
  });

  it("keeps an unchanged line's object from the previous answer; a changed or new line gets a new one; rows gone, no entry", () => {
    const first = treeDrift(ROWS, [aDriftRow({ node: "DOC-A", class: "fix" }), aDriftRow({ node: "SEC-A1", class: "check" })], null);
    const second = treeDrift(
      ROWS,
      [aDriftRow({ node: "DOC-A", class: "fix" }), aDriftRow({ node: "SEC-A1", class: "check" }), aDriftRow({ node: "SEC-A1", class: "check" }), aDriftRow({ node: "DOC-C", class: "cannot" })],
      first,
    );
    expect(second.byRow.get(DOC_A.key)).toBe(first.byRow.get(DOC_A.key));
    expect(second.byRow.get(SEC_A1.key)).not.toBe(first.byRow.get(SEC_A1.key));
    expect(shown(second.byRow)).toEqual([
      [DOC_A.key, counts({ fix: 1 })],
      [SEC_A1.key, counts({ check: 2 })],
      [DOC_C.key, counts({ cannot: 1 })],
    ]);
    const third = treeDrift(ROWS, [aDriftRow({ node: "DOC-A", class: "check" })], second);
    expect(third.byRow.get(DOC_A.key)).not.toBe(second.byRow.get(DOC_A.key));
    expect([...third.byRow.keys()]).toEqual([DOC_A.key]);
  });

  it("with every line unchanged returns the previous map itself; a line changed, gone or new, a new map", () => {
    const rows = [aDriftRow({ node: "DOC-A", class: "fix" }), aDriftRow({ node: "SEC-A1", class: "check" })];
    const first = treeDrift(ROWS, rows, null);
    // Other rows, other words, the same counts per line; another unmatched count too.
    const reworded = [aDriftRow({ node: "DOC-A", class: "fix", detail: "body" }), aDriftRow({ node: "SEC-A1", class: "check", relation: "verifies" }), aDriftRow({ node: "R-404" })];
    const second = treeDrift(ROWS, reworded, first);
    expect(second.byRow).toBe(first.byRow);
    expect(second.unmatched).toBe(1);
    expect(treeDrift(ROWS, [rows[0] ?? aDriftRow({ node: "DOC-A" })], first).byRow).not.toBe(first.byRow);
    expect(treeDrift(ROWS, [...rows, aDriftRow({ node: "DOC-C", class: "cannot" })], first).byRow).not.toBe(first.byRow);
    expect(treeDrift(ROWS, [...rows, aDriftRow({ node: "DOC-A", class: "fix" })], first).byRow).not.toBe(first.byRow);
    expect(treeDrift(ROWS, rows, null).byRow).not.toBe(first.byRow);
  });

  it("degrades on a malformed answer: no row 6, rows or counts of another shape, a row that is no object; never throws", () => {
    const report = { ...aHealthReport(), drift: undefined } as unknown as HealthReport;
    expect(driftOf(report)).toBeUndefined();
    expect(measuredRows(driftOf(report))).toBeNull();
    expect(driftWords(driftOf(report))).toEqual({ state: null, causes: [] });
    const rows = [aDriftRow({ node: "DOC-A", class: "fix" })];
    expect(measuredRows({ ...measuredDrift(rows), rows: "rows" } as unknown as HealthDrift)).toBeNull();
    expect(measuredRows({ ...measuredDrift(rows), counts: 3 } as unknown as HealthDrift)).toBeNull();
    expect(measuredRows({ ...measuredDrift(rows), state: 7 } as unknown as HealthDrift)).toBeNull();
    expect(driftWords({ ...aDriftIn("cannot", []), state: 7, causes: ["a cause", 3, null] } as unknown as HealthDrift)).toEqual({ state: null, causes: ["a cause"] });
    const odd = [null, 5, "DOC-A", { ...aDriftRow({ node: "DOC-A" }), class: 9 }, aDriftRow({ node: "DOC-A", class: "fix" })] as unknown as DriftRow[];
    const drift = treeDrift(ROWS, odd, null);
    expect(shown(drift.byRow)).toEqual([[DOC_A.key, counts({ fix: 1, unlisted: 1 })]]);
    expect(drift.unmatched).toBe(0);
  });

  it("reads rows only from a measured row 6 holding counts and rows: off, cannot, a state not listed or no counts give none", () => {
    const rows = [aDriftRow({ node: "DOC-A" })];
    expect(measuredRows(measuredDrift(rows))).toBe(rows);
    expect(measuredRows(measuredDrift([]))).toEqual([]);
    expect(measuredRows(aDriftOff())).toBeNull();
    expect(measuredRows(aDriftIn("cannot", ["spec.lock:1: unclosed array, expected `]`"]))).toBeNull();
    expect(measuredRows(aDriftIn("partly", []))).toBeNull();
    expect(measuredRows({ ...measuredDrift(rows), counts: null })).toBeNull();
    expect(measuredRows({ ...measuredDrift(rows), rows: null })).toBeNull();
  });
});

describe("a line's drift badges (UI-6)", () => {
  it("one per class with rows, in verify's order, then Another class; each its look and icon, the label with the count; none at 0", () => {
    expect(driftBadgeLooks(counts({ unlisted: 1, cannot: 3, check: 2, fix: 1 }))).toEqual([
      { label: "Fix 1", tone: "finding-error", icon: "findingError" },
      { label: "Check 2", tone: "finding-warning", icon: "findingWarning" },
      { label: "Cannot verify 3", tone: "sync-cannot-verify", icon: "cannotCheck" },
      { label: "Another class 1", tone: "sync-unknown", icon: "unknown" },
    ]);
    expect(driftBadgeLooks(counts({ check: 4 }))).toEqual([{ label: "Check 4", tone: "finding-warning", icon: "findingWarning" }]);
    expect(driftBadgeLooks(counts({}))).toEqual([]);
  });
});

describe("Show drift's storage (UI-7)", () => {
  it("is off until turned on, per project, under specengine.tree.show-drift.<project>; off again, the key gone", () => {
    expect(recallShowDrift("alpha")).toBe(false);
    rememberShowDrift("alpha", true);
    expect(window.localStorage.getItem("specengine.tree.show-drift.alpha")).toBe("on");
    expect(recallShowDrift("alpha")).toBe(true);
    expect(recallShowDrift("beta")).toBe(false);
    rememberShowDrift("alpha", false);
    expect(recallShowDrift("alpha")).toBe(false);
    expect(window.localStorage.length).toBe(0);
    window.localStorage.setItem("specengine.tree.show-drift.alpha", "true");
    expect(recallShowDrift("alpha")).toBe(false);
  });

  it("works without storage: every access refused reads off and keeps nothing, never throws", () => {
    const refused = () => {
      throw new DOMException("The operation is insecure.", "SecurityError");
    };
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(refused);
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(refused);
    vi.spyOn(Storage.prototype, "removeItem").mockImplementation(refused);
    expect(() => {
      rememberShowDrift("alpha", true);
      rememberShowDrift("alpha", false);
    }).not.toThrow();
    expect(recallShowDrift("alpha")).toBe(false);
  });

  it("works where the page may not reach storage at all (a sandboxed frame)", () => {
    vi.spyOn(window, "localStorage", "get").mockImplementation(() => {
      throw new DOMException("The document is sandboxed.", "SecurityError");
    });
    expect(() => {
      rememberShowDrift("alpha", true);
    }).not.toThrow();
    expect(recallShowDrift("alpha")).toBe(false);
  });
});
