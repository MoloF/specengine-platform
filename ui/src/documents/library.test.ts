import { describe, expect, it } from "vitest";
import type { LibraryRow } from "../api/types";
import { aDirection, aLibraryChange, aLibraryDecision, aLibraryRow, aModule } from "../test/builders";
import { enT } from "../test/text";
import {
  ALL_CHOSEN,
  atAll,
  decisionCounts,
  decisionCountText,
  facetChoices,
  groupLabel,
  groupsOf,
  isFiltered,
  keptRows,
  knownGroup,
  moduleChoices,
  NO_FILTERS,
  pressed,
  sortedRows,
  startingFilters,
  toggleCount,
  toggled,
  valuesOf,
  type LibraryFilters,
} from "./library";

// DL18 of docs/features/doc-library.md "Acceptance criteria", the pure half: facets AND across, OR
// within; each value's count is the rows every other filter keeps; Class starts at canon only when a
// row is canon; search over title, name, path and tags; the catalog's sort; the served groups read
// through one table. The view's tests check their counts against these functions.

const collator = new Intl.Collator("en", { numeric: true });

/** Four rows over two classes, two kinds, tags, owners, tiers and scopes, two modules, a behind row and an orphan. */
function rows(): LibraryRow[] {
  return [
    aLibraryRow({ name: "DOC-A", class: "canon", kind: "mechanic", status: "accepted", tags: ["tides", "tuning"], owner: "Ann", tier: 2, scope: ["sprint"], modules: ["waters"], behind: true }),
    aLibraryRow({ name: "DOC-B", class: "canon", kind: "rule", status: "draft", tags: ["tides"], owner: null, tier: 1, modules: ["waters", "rules"] }),
    aLibraryRow({ name: "DOC-C", class: "spec", kind: "mechanic", status: "accepted", tags: [], owner: "Bob", tier: null, modules: [], orphan: true }),
    aLibraryRow({ name: "docs/notes/cats.md", id: null, title: "\u041a\u043e\u0442\u044b \u0433\u0430\u0432\u0430\u043d\u0438", class: null, kind: null, status: null, tags: ["\u6f6e"], path: "docs/notes/cats.md", modules: ["rules"] }),
  ];
}

function chosen(fields: Partial<LibraryFilters["chosen"]>): LibraryFilters {
  return { ...NO_FILTERS, chosen: { ...ALL_CHOSEN, ...fields } };
}

function names(list: readonly LibraryRow[]): string[] {
  return list.map((row) => row.name);
}

describe("the start (DL18)", () => {
  it("Class starts at canon when a row is canon, every other facet at All", () => {
    expect(startingFilters(rows())).toEqual(chosen({ class: ["canon"] }));
  });

  it("starts at All when no row is canon (M: canon chosen over no canon row empties the table)", () => {
    const none = rows().map((row) => ({ ...row, class: row.class === "canon" ? "spec" : row.class }));
    expect(startingFilters(none)).toEqual(NO_FILTERS);
    expect(keptRows(none, startingFilters(none), null)).toHaveLength(4);
  });
});

describe("the facets (DL18)", () => {
  it("the Discipline facet's values: each kept entry once, null where none (DP21 of department-practices)", () => {
    expect(valuesOf(aLibraryRow({ name: "PRA-ART", disciplines: ["art", "development", "art"] }), "disciplines")).toEqual(["art", "development"]);
    expect(valuesOf(aLibraryRow({ name: "DOC-X" }), "disciplines")).toEqual([null]);
    const practices = [aLibraryRow({ name: "PRA-ART", disciplines: ["art"] }), aLibraryRow({ name: "PRA-SHARED", disciplines: ["art", "development"] }), aLibraryRow({ name: "DOC-X" })];
    expect(names(keptRows(practices, chosen({ disciplines: ["development"] }), null))).toEqual(["PRA-SHARED"]);
    expect(facetChoices(practices, NO_FILTERS, null, "disciplines", collator)).toEqual([
      { value: "art", count: 2 },
      { value: "development", count: 1 },
      { value: null, count: 1 },
    ]);
  });

  it("a facet's values: one for a field, each entry for a list, null where none", () => {
    const [a, , c, cats] = rows();
    expect(a === undefined || c === undefined || cats === undefined).toBe(false);
    if (a === undefined || c === undefined || cats === undefined) {
      return;
    }
    expect(valuesOf(a, "tags")).toEqual(["tides", "tuning"]);
    expect(valuesOf(c, "tags")).toEqual([null]);
    expect(valuesOf(a, "tier")).toEqual(["2"]);
    expect(valuesOf(c, "tier")).toEqual([null]);
    expect(valuesOf(a, "scope")).toEqual(["sprint"]);
    expect(valuesOf(cats, "class")).toEqual([null]);
  });

  it("two values of one facet OR; two facets AND (M: OR across facets)", () => {
    expect(names(keptRows(rows(), chosen({ kind: ["mechanic", "rule"] }), null))).toEqual(["DOC-A", "DOC-B", "DOC-C"]);
    expect(names(keptRows(rows(), chosen({ class: ["canon"], kind: ["mechanic"] }), null))).toEqual(["DOC-A"]);
    expect(names(keptRows(rows(), chosen({ class: ["spec"], tags: ["tides"] }), null))).toEqual([]);
    expect(names(keptRows(rows(), chosen({ tags: [null] }), null))).toEqual(["DOC-C"]);
    expect(names(keptRows(rows(), chosen({ class: [null] }), null))).toEqual(["docs/notes/cats.md"]);
  });

  it("each value's count is the rows every other filter keeps, by count descending, then collation, None last", () => {
    const filters = chosen({ class: ["canon"], kind: ["mechanic"] });
    // Class counts over Kind's mechanic alone: canon DOC-A, spec DOC-C.
    expect(facetChoices(rows(), filters, null, "class", collator)).toEqual([
      { value: "canon", count: 1 },
      { value: "spec", count: 1 },
      { value: null, count: 0 },
    ]);
    // Kind counts over Class's canon alone: mechanic DOC-A, rule DOC-B; the no-kind row's own class is none.
    expect(facetChoices(rows(), filters, null, "kind", collator)).toEqual([
      { value: "mechanic", count: 1 },
      { value: "rule", count: 1 },
      { value: null, count: 0 },
    ]);
    // Tags under both: DOC-A's two.
    expect(facetChoices(rows(), filters, null, "tags", collator)).toEqual([
      { value: "tides", count: 1 },
      { value: "tuning", count: 1 },
      { value: "\u6f6e", count: 0 },
      { value: null, count: 0 },
    ]);
    expect(facetChoices(rows(), NO_FILTERS, null, "tags", collator).map((choice) => [choice.value, choice.count])).toEqual([
      ["tides", 2],
      ["tuning", 1],
      ["\u6f6e", 1],
      [null, 1],
    ]);
  });

  it("a chosen value no row holds stays listed, at 0", () => {
    expect(facetChoices(rows(), chosen({ owner: ["Gone"] }), null, "owner", collator).find((choice) => choice.value === "Gone")).toEqual({ value: "Gone", count: 0 });
  });

  it("Module keeps the rows it holds; its counts are over the other filters, All modules first, then by title", () => {
    expect(names(keptRows(rows(), NO_FILTERS, "waters"))).toEqual(["DOC-A", "DOC-B"]);
    const modules = [aModule({ name: "waters", title: "Waters", root: "DOM-WATER" }), aDirection({ name: "rules", title: "Rulebook", kinds: ["rule"] })];
    const { all, choices } = moduleChoices(rows(), chosen({ class: ["canon"] }), modules, collator);
    expect(all).toBe(2);
    expect(choices.map((choice) => [choice.module.name, choice.count])).toEqual([
      ["rules", 1],
      ["waters", 2],
    ]);
  });

  it("the toggles keep the rows behind their decision and the orphans; their counts are over the other filters", () => {
    expect(names(keptRows(rows(), { ...NO_FILTERS, behind: true }, null))).toEqual(["DOC-A"]);
    expect(names(keptRows(rows(), { ...NO_FILTERS, orphans: true }, null))).toEqual(["DOC-C"]);
    expect(toggleCount(rows(), chosen({ class: ["canon"] }), null, "behind")).toBe(1);
    expect(toggleCount(rows(), chosen({ class: ["canon"] }), null, "orphans")).toBe(0);
  });

  it("search folds case and covers title, name, path and tags, in any script", () => {
    const search = (text: string) => names(keptRows(rows(), { ...NO_FILTERS, search: text }, null));
    expect(search("doc-b")).toEqual(["DOC-B"]);
    expect(search("NOTES/")).toEqual(["docs/notes/cats.md"]);
    expect(search("\u043a\u043e\u0442\u044b")).toEqual(["docs/notes/cats.md"]);
    expect(search("TUNING")).toEqual(["DOC-A"]);
    expect(search("\u6f6e")).toEqual(["docs/notes/cats.md"]);
    expect(search("  ")).toHaveLength(4);
  });

  it("a press toggles a value; All empties the facet; anything chosen is filtered", () => {
    const once = toggled(NO_FILTERS, "tags", "tides");
    expect(once.chosen.tags).toEqual(["tides"]);
    expect(toggled(once, "tags", "tides").chosen.tags).toEqual([]);
    expect(atAll(chosen({ class: ["canon", null] }), "class")).toEqual(NO_FILTERS);
    expect(isFiltered(NO_FILTERS, null)).toBe(false);
    expect(isFiltered(NO_FILTERS, "waters")).toBe(true);
    expect(isFiltered({ ...NO_FILTERS, search: " x " }, null)).toBe(true);
    expect(isFiltered(once, null)).toBe(true);
  });
});

describe("the order (DL18: the catalog's sort)", () => {
  const changed = (at: string | null) => (at === null ? null : aLibraryChange({ at }));
  const sortable = [
    aLibraryRow({ name: "B", title: "beta", rev: 2, reviewed: "2026-02-01", last_change: changed("2026-03-01T00:00:00Z"), used_by: 1 }),
    aLibraryRow({ name: "A", title: "Alpha", rev: null, reviewed: null, last_change: changed(null), used_by: 5 }),
    aLibraryRow({ name: "C", title: "gamma", rev: 2, reviewed: "2026-01-01", last_change: changed("2026-01-01T00:00:00Z"), used_by: 1 }),
  ];

  it("Title ascends first; Version, Reviewed, Last change and Used by descend first; a second press reverses", () => {
    expect(pressed(null, "title")).toEqual({ column: "title", descending: false });
    expect(pressed(null, "class")).toEqual({ column: "class", descending: false });
    for (const column of ["version", "reviewed", "changed", "used"] as const) {
      expect(pressed(null, column)).toEqual({ column, descending: true });
    }
    expect(pressed({ column: "title", descending: false }, "title")).toEqual({ column: "title", descending: true });
  });

  it("missing values sort last both ways; ties keep served order both ways", () => {
    expect(names(sortedRows(sortable, null, collator))).toEqual(["B", "A", "C"]);
    expect(names(sortedRows(sortable, { column: "title", descending: false }, collator))).toEqual(["A", "B", "C"]);
    expect(names(sortedRows(sortable, { column: "version", descending: true }, collator))).toEqual(["B", "C", "A"]);
    expect(names(sortedRows(sortable, { column: "version", descending: false }, collator))).toEqual(["B", "C", "A"]);
    expect(names(sortedRows(sortable, { column: "reviewed", descending: true }, collator))).toEqual(["B", "C", "A"]);
    expect(names(sortedRows(sortable, { column: "changed", descending: false }, collator))).toEqual(["C", "B", "A"]);
    expect(names(sortedRows(sortable, { column: "used", descending: true }, collator))).toEqual(["A", "B", "C"]);
    expect(names(sortedRows(sortable, { column: "used", descending: false }, collator))).toEqual(["B", "C", "A"]);
  });
});

describe("the decisions (DL18)", () => {
  const decisions = [
    aLibraryDecision({ name: "DEC-3", group: "accepted" }),
    aLibraryDecision({ name: "DEC-2", group: "superseded", status: "superseded-by DEC-3" }),
    aLibraryDecision({ name: "DEC-1", group: "superseded", status: "superseded-by DEC-2" }),
    aLibraryDecision({ name: "DEC-0", group: "withdrawn" }),
  ];

  it("groups in served order, each once, entries in served order", () => {
    expect(groupsOf(decisions).map(({ group, entries }) => [group, entries.map((entry) => entry.name)])).toEqual([
      ["accepted", ["DEC-3"]],
      ["superseded", ["DEC-2", "DEC-1"]],
      ["withdrawn", ["DEC-0"]],
    ]);
  });

  it("reads a group through one table: the four known, another raw", () => {
    expect(["accepted", "superseded", "rejected", "other", "withdrawn"].map(knownGroup)).toEqual(["accepted", "superseded", "rejected", "other", null]);
    expect(groupLabel("rejected", enT)).toEqual({ label: "Rejected", raw: false });
    expect(groupLabel("withdrawn", enT)).toEqual({ label: "withdrawn", raw: true });
  });

  it("counts a row's decisions by group: N accepted · M superseded …", () => {
    const parts = decisionCounts(decisions);
    expect(parts).toEqual([
      { group: "accepted", count: 1, known: "accepted" },
      { group: "superseded", count: 2, known: "superseded" },
      { group: "withdrawn", count: 1, known: null },
    ]);
    // The unknown group's words around its slot: the caller fills it with the raw word under lang="en".
    expect(parts.map((part) => decisionCountText(part, enT, "<group>"))).toEqual(["1 accepted", "2 superseded", "1 <group>"]);
    expect(decisionCounts([aLibraryDecision({ name: "DEC-9", group: "other" })]).map((part) => decisionCountText(part, enT, ""))).toEqual(["1 other"]);
  });
});
