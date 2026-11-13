import { describe, expect, it } from "vitest";
import type { CatalogRow } from "../api/types";
import { enT, ruT } from "../test/text";
import { aCatalogChange, aCatalogRow } from "../test/builders";
import { ariaSort, CATALOG_COLUMNS, CATALOG_FILTERS, CATALOG_SHOWN, filteredRows, pressed, sortedRows, tasksText, titleOf, versionText, type CatalogColumn } from "./catalog";

// DC-10 of docs/features/direction-catalog.md "Acceptance criteria": what a press on each header
// orders the served rows by. Title and State ascending first, the rest descending first; a second
// press reverses; ties keep served order in either direction; a missing value stands last either way;
// Title by the viewer's collation, State by the rollups' order, an unknown word last. UO11 of
// docs/features/usage-orphans.md "Acceptance criteria": Used by, after Tasks, by the served count,
// most first; No declared use keeps the rows whose served `used_by` is 0, Not cited those whose
// `used_by` and `mentioned_by` are both 0 (the orchestrator's working answer, 2026-11-13). Named
// mutations: No declared use on `mentioned_by`; Not cited on `used_by` alone.

const EN = new Intl.Collator("en", { numeric: true });

function ids(rows: readonly CatalogRow[]): (string | null)[] {
  return rows.map((row) => row.id);
}

function sortBy(rows: readonly CatalogRow[], column: CatalogColumn, presses = 1, collator = EN): readonly CatalogRow[] {
  let sort = pressed(null, column);
  for (let press = 1; press < presses; press += 1) {
    sort = pressed(sort, column);
  }
  return sortedRows(rows, sort, collator);
}

describe("a press's order (DC-10)", () => {
  it("names seven columns in the table's order, Used by after Tasks, and shows 50 at a time", () => {
    expect(CATALOG_COLUMNS).toEqual(["title", "version", "state", "open", "tasks", "used", "changed"]);
    expect(CATALOG_SHOWN).toBe(50);
  });

  it("orders Title and State ascending first, the rest descending first; the same column again reverses; another column starts at its own first", () => {
    expect(CATALOG_COLUMNS.map((column) => [column, pressed(null, column).descending])).toEqual([
      ["title", false],
      ["version", true],
      ["state", false],
      ["open", true],
      ["tasks", true],
      ["used", true],
      ["changed", true],
    ]);
    expect(pressed({ column: "version", descending: true }, "version")).toEqual({ column: "version", descending: false });
    expect(pressed({ column: "version", descending: false }, "version")).toEqual({ column: "version", descending: true });
    expect(pressed({ column: "version", descending: false }, "title")).toEqual({ column: "title", descending: false });
  });

  it("says aria-sort on the ordering column alone", () => {
    const sort = pressed(null, "version");
    expect(CATALOG_COLUMNS.map((column) => ariaSort(sort, column))).toEqual([undefined, "descending", undefined, undefined, undefined, undefined, undefined]);
    expect(ariaSort(pressed(sort, "version"), "version")).toBe("ascending");
    expect(ariaSort(null, "title")).toBeUndefined();
  });

  it("keeps the served order before any press, the same array", () => {
    const rows = [aCatalogRow({ id: "B" }), aCatalogRow({ id: "A" })];
    expect(sortedRows(rows, null, EN)).toBe(rows);
  });
});

describe("each column's order (DC-10)", () => {
  it("Version: most first, ties in served order, no version last; reversed, least first, ties still served, no version still last", () => {
    const rows = [
      aCatalogRow({ id: "A", rev: 2 }),
      aCatalogRow({ id: "NONE", rev: null }),
      aCatalogRow({ id: "B", rev: 3 }),
      aCatalogRow({ id: "C", rev: 2 }),
      aCatalogRow({ id: "D", rev: 1 }),
    ];
    expect(ids(sortBy(rows, "version"))).toEqual(["B", "A", "C", "D", "NONE"]);
    expect(ids(sortBy(rows, "version", 2))).toEqual(["D", "A", "C", "B", "NONE"]);
  });

  it("Title: the title, else the ID, else the path, by the viewer's collation, numbers as numbers; reversed", () => {
    const rows = [
      aCatalogRow({ id: "S-10", title: "Scene 10" }),
      aCatalogRow({ id: "S-2", title: "Scene 2" }),
      aCatalogRow({ id: "ZED", title: null }),
      aCatalogRow({ id: "S-E", title: "\u00e9cluse" }),
      aCatalogRow({ id: null, title: null, path: "docs/spec/aaa.md" }),
      aCatalogRow({ id: "S-B", title: "Beacon" }),
    ];
    expect(rows.map(titleOf)).toEqual(["Scene 10", "Scene 2", "ZED", "\u00e9cluse", "docs/spec/aaa.md", "Beacon"]);
    expect(sortBy(rows, "title").map(titleOf)).toEqual(["Beacon", "docs/spec/aaa.md", "\u00e9cluse", "Scene 2", "Scene 10", "ZED"]);
    expect(sortBy(rows, "title", 2).map(titleOf)).toEqual(["ZED", "Scene 10", "Scene 2", "\u00e9cluse", "docs/spec/aaa.md", "Beacon"]);
  });

  it("Title: a collation of another language orders by its own rules (M: the code points compared)", () => {
    // Swedish puts \u00e4 after z; English beside a.
    const rows = [aCatalogRow({ id: "Z", title: "zon" }), aCatalogRow({ id: "AE", title: "\u00e4c" }), aCatalogRow({ id: "A", title: "ab" })];
    expect(ids(sortBy(rows, "title", 1, new Intl.Collator("en")))).toEqual(["A", "AE", "Z"]);
    expect(ids(sortBy(rows, "title", 1, new Intl.Collator("sv")))).toEqual(["A", "Z", "AE"]);
    // Cyrillic orders by its own alphabet in Russian: \u0430 (a), \u0431 (be), \u044f (ya).
    const cyrillic = [aCatalogRow({ id: "YA", title: "\u044f\u043a\u043e\u0440\u044c" }), aCatalogRow({ id: "A", title: "\u0430\u0440\u043a\u0430" }), aCatalogRow({ id: "BE", title: "\u0431\u0443\u0439" })];
    expect(ids(sortBy(cyrillic, "title", 1, new Intl.Collator("ru")))).toEqual(["A", "BE", "YA"]);
  });

  it("State: the rollups' order, done first, an unknown word last in either direction, ties in served order", () => {
    const rows = [
      aCatalogRow({ id: "NC", state: "no card" }),
      aCatalogRow({ id: "PARKED", state: "parked" }),
      aCatalogRow({ id: "IW1", state: "in work" }),
      aCatalogRow({ id: "CF", state: "confirmed" }),
      aCatalogRow({ id: "DP", state: "departed" }),
      aCatalogRow({ id: "SU", state: "suspect" }),
      aCatalogRow({ id: "IW2", state: "in work" }),
    ];
    expect(ids(sortBy(rows, "state"))).toEqual(["CF", "IW1", "IW2", "SU", "DP", "NC", "PARKED"]);
    expect(ids(sortBy(rows, "state", 2))).toEqual(["NC", "DP", "SU", "IW1", "IW2", "CF", "PARKED"]);
  });

  it("Open items and Tasks: most first, ties in served order", () => {
    const rows = [aCatalogRow({ id: "A", open_items: 1, tasks: 2 }), aCatalogRow({ id: "B", open_items: 3, tasks: 0 }), aCatalogRow({ id: "C", open_items: 1, tasks: 2 })];
    expect(ids(sortBy(rows, "open"))).toEqual(["B", "A", "C"]);
    expect(ids(sortBy(rows, "tasks"))).toEqual(["A", "C", "B"]);
    expect(ids(sortBy(rows, "tasks", 2))).toEqual(["B", "A", "C"]);
  });

  it("Used by: the served count, most first, ties in served order; reversed, least first (UO11)", () => {
    const rows = [
      aCatalogRow({ id: "A", used_by: 2, mentioned_by: 0 }),
      aCatalogRow({ id: "B", used_by: 0, mentioned_by: 9 }),
      aCatalogRow({ id: "C", used_by: 5, mentioned_by: 1 }),
      aCatalogRow({ id: "D", used_by: 2, mentioned_by: 4 }),
    ];
    expect(ids(sortBy(rows, "used"))).toEqual(["C", "A", "D", "B"]);
    expect(ids(sortBy(rows, "used", 2))).toEqual(["B", "A", "D", "C"]);
  });

  it("Last change: the newest stored time first, none read last in either direction", () => {
    const rows = [
      aCatalogRow({ id: "OLD", last_change: aCatalogChange({ at: "2026-01-02T10:00:00Z" }) }),
      aCatalogRow({ id: "NONE", last_change: null }),
      aCatalogRow({ id: "NEW", last_change: aCatalogChange({ at: "2026-03-01T08:00:00Z" }) }),
      aCatalogRow({ id: "MID", last_change: aCatalogChange({ at: "2026-02-10T23:59:00Z" }) }),
    ];
    expect(ids(sortBy(rows, "changed"))).toEqual(["NEW", "MID", "OLD", "NONE"]);
    expect(ids(sortBy(rows, "changed", 2))).toEqual(["OLD", "MID", "NEW", "NONE"]);
  });
});

describe("the Used by filters (UO11)", () => {
  const ROWS = [
    aCatalogRow({ id: "USED", used_by: 1, mentioned_by: 0 }),
    aCatalogRow({ id: "MENTIONED", used_by: 0, mentioned_by: 3 }),
    aCatalogRow({ id: "ORPHAN", used_by: 0, mentioned_by: 0, orphan: true }),
    aCatalogRow({ id: "SECTION", used_by: 0, mentioned_by: 0, orphan: null }),
    aCatalogRow({ id: "BOTH", used_by: 2, mentioned_by: 2 }),
  ];

  it("names two, No declared use and Not cited; none held keeps every row, the same array", () => {
    expect(CATALOG_FILTERS).toEqual(["undeclared", "uncited"]);
    expect(filteredRows(ROWS, null)).toBe(ROWS);
  });

  it("No declared use: the rows whose served used_by is 0, in the order given, a mentioned one too, a section alike (M: the filter on mentioned_by)", () => {
    expect(ids(filteredRows(ROWS, "undeclared"))).toEqual(["MENTIONED", "ORPHAN", "SECTION"]);
    expect(filteredRows([aCatalogRow({ id: "A", used_by: 1 })], "undeclared")).toEqual([]);
  });

  it("Not cited: the rows whose served used_by and mentioned_by are both 0, never a mentioned one (M: Not cited on used_by alone)", () => {
    expect(ids(filteredRows(ROWS, "uncited"))).toEqual(["ORPHAN", "SECTION"]);
    expect(filteredRows([aCatalogRow({ id: "A", used_by: 0, mentioned_by: 1 })], "uncited")).toEqual([]);
  });
});

describe("a row's words (DC-10)", () => {
  it("Version N, or No version", () => {
    expect(versionText(aCatalogRow({ id: "A", rev: 3 }), enT)).toBe("Version 3");
    expect(versionText(aCatalogRow({ id: "A", rev: null }), enT)).toBe("No version");
    expect(versionText(aCatalogRow({ id: "A", rev: 3 }), ruT)).not.toBe("Version 3");
  });

  it("the served tasks and how many of them are open, compact, in both languages", () => {
    expect(tasksText(aCatalogRow({ id: "A", tasks: 3, tasks_open: 1 }), enT)).toBe("3 · 1 open");
    expect(tasksText(aCatalogRow({ id: "A", tasks: 1, tasks_open: 0 }), enT)).toBe("1 · 0 open");
    // Russian through its key, ru.ts's own words (no Cyrillic outside ru.ts): the pair, compact, not the English.
    const russian = tasksText(aCatalogRow({ id: "A", tasks: 5, tasks_open: 2 }), ruT);
    expect(russian).toBe(ruT("tree.catalog.tasks", { count: 5, open: 2 }));
    expect(russian).toMatch(/^5 · \S.*2$/);
    expect(russian).not.toBe(enT("tree.catalog.tasks", { count: 5, open: 2 }));
    expect(tasksText(aCatalogRow({ id: "A", tasks: 1, tasks_open: 1 }), ruT)).toBe(ruT("tree.catalog.tasks", { count: 1, open: 1 }));
  });

  it("a bare 0 where both are 0, in either language; any served open count keeps the pair (M: 0 for any row with no task)", () => {
    expect(tasksText(aCatalogRow({ id: "A", tasks: 0, tasks_open: 0 }), enT)).toBe("0");
    expect(tasksText(aCatalogRow({ id: "A", tasks: 0, tasks_open: 0 }), ruT)).toBe("0");
    // Never served by the daemon (open tasks are among its tasks), but drawn as served, not hidden.
    expect(tasksText(aCatalogRow({ id: "A", tasks: 0, tasks_open: 1 }), enT)).toBe("0 · 1 open");
  });
});
