import type { KnownDecisionGroup, LibraryDecision, LibraryRow, WorkspaceModule } from "../api/types";
import type { Translate } from "../i18n";
import { folded } from "../tasks/filter";

// Documents' arrangement of the one answer read (docs/features/doc-library.md "Description and interactions", DL18):
// the facets, their counts, the search, the order a header's press gives and a row's decision
// words. Facets AND, a facet's values OR; each value's count is the rows every other filter keeps.
// Nothing here counts a use, resolves a link, reads git or judges a decision: the served rows are
// filtered, ordered and paged, never recomputed (docs/canon/architecture.md#ui). The one word read
// here is the class the table starts at (A3) and the served decision groups, told through
// GROUP_WORDS; a class, a kind, a status, a tag, an owner and a scope are data, compared only with
// what a person chose among the served values.

/** The rows shown before "Show 200 more", and how many more each press shows. */
export const LIBRARY_SHOWN = 200;

/** The facets a person picks any number of values in, ORed. Module is apart: one choice, the module bar's stored one. */
export const MULTI_FACETS = ["class", "kind", "status", "tags", "owner", "tier", "scope"] as const;

export type MultiFacet = (typeof MULTI_FACETS)[number];

/** The bar's order: Class, Kind, Status, Tags, Module, Owner, Tier, Scope. */
export const FACET_ORDER = ["class", "kind", "status", "tags", "module", "owner", "tier", "scope"] as const;

export type Facet = (typeof FACET_ORDER)[number];

/** A facet's value: a served word as written, or null for none ("No class"). */
export type FacetValue = string | null;

/** What narrows the table, the Module facet aside: each facet's chosen values (none: All), the two toggles, the search. */
export interface LibraryFilters {
  chosen: Readonly<Record<MultiFacet, readonly FacetValue[]>>;
  behind: boolean;
  orphans: boolean;
  search: string;
}

/** Every facet at All. */
export const ALL_CHOSEN: Readonly<Record<MultiFacet, readonly FacetValue[]>> = { class: [], kind: [], status: [], tags: [], owner: [], tier: [], scope: [] };

/** Every facet at All, both toggles off, no search. */
export const NO_FILTERS: LibraryFilters = { chosen: ALL_CHOSEN, behind: false, orphans: false, search: "" };

/**
 * The class the table starts at when a row has it (the analyst's A3: wish 1 is about the canon, "how
 * it works now"); with none, every class. One press on All classes gives the rest (ADR-0073).
 */
const START_CLASS = "canon";

/** The filters a library starts at: Class at canon when a row is canon, every other facet at All. */
export function startingFilters(rows: readonly LibraryRow[]): LibraryFilters {
  return rows.some((row) => row.class === START_CLASS) ? { ...NO_FILTERS, chosen: { ...ALL_CHOSEN, class: [START_CLASS] } } : NO_FILTERS;
}

/** One value, as a facet holds it. */
function one(value: FacetValue): FacetValue[] {
  return [value];
}

/** A list's entries, each once; none (absent or empty): the facet's "No …". */
function entriesOf(values: readonly string[] | null): FacetValue[] {
  return values === null || values.length === 0 ? [null] : [...new Set(values)];
}

/** The values a row holds in a facet: one for a field, each entry for a list, null where it has none. */
export function valuesOf(row: LibraryRow, facet: MultiFacet): FacetValue[] {
  switch (facet) {
    case "class":
      return one(row.class);
    case "kind":
      return one(row.kind);
    case "status":
      return one(row.status);
    case "tags":
      return entriesOf(row.tags);
    case "owner":
      return one(row.owner);
    case "tier":
      return one(row.tier === null ? null : String(row.tier));
    case "scope":
      return entriesOf(row.scope);
  }
}

/** A search as compared: trimmed, NFC, lower case (`ui/README.md` "Screen rules", Text). */
export function searchKey(search: string): string {
  return folded(search.trim());
}

/** Each served row's searchable text, folded once per row object: a new answer's rows fold anew, a keystroke folds none. */
const searchable = new WeakMap<LibraryRow, readonly string[]>();

/** A row's title, name, path and tags, folded as the search compares them. */
function searchTextOf(row: LibraryRow): readonly string[] {
  const known = searchable.get(row);
  if (known !== undefined) {
    return known;
  }
  const texts = [row.title ?? "", row.name, row.path, ...row.tags].map(folded);
  searchable.set(row, texts);
  return texts;
}

/** Whether a row's title, name, path or one of its tags holds the folded search; an empty search holds every row. */
export function searchHits(row: LibraryRow, key: string): boolean {
  return key === "" || searchTextOf(row).some((text) => text.includes(key));
}

/** A filter left out when a facet counts its own values: the facet, Module or a toggle. */
type Apart = MultiFacet | "module" | "behind" | "orphans" | null;

/** Whether every filter but `apart` keeps a row; `module` the chosen module's name, null for All modules. */
function keeps(row: LibraryRow, filters: LibraryFilters, module: string | null, key: string, apart: Apart): boolean {
  for (const facet of MULTI_FACETS) {
    const chosen = filters.chosen[facet];
    if (facet !== apart && chosen.length > 0 && !valuesOf(row, facet).some((value) => chosen.includes(value))) {
      return false;
    }
  }
  if (apart !== "module" && module !== null && !row.modules.includes(module)) {
    return false;
  }
  if (apart !== "behind" && filters.behind && row.behind !== true) {
    return false;
  }
  if (apart !== "orphans" && filters.orphans && row.orphan !== true) {
    return false;
  }
  return searchHits(row, key);
}

/** The rows every filter keeps, in served order. */
export function keptRows(rows: readonly LibraryRow[], filters: LibraryFilters, module: string | null): readonly LibraryRow[] {
  const key = searchKey(filters.search);
  return rows.filter((row) => keeps(row, filters, module, key, null));
}

/** Whether anything narrows the table: a facet off All, a toggle, a search, a module. */
export function isFiltered(filters: LibraryFilters, module: string | null): boolean {
  return module !== null || filters.behind || filters.orphans || searchKey(filters.search) !== "" || MULTI_FACETS.some((facet) => filters.chosen[facet].length > 0);
}

/** One value of a facet and the rows the other filters keep that hold it. */
export interface FacetChoice {
  value: FacetValue;
  count: number;
}

/**
 * A facet's values: each value a row holds, and each chosen one, with the number of rows every other
 * filter keeps that hold it; by that count descending, then by the viewer's collation, "No …" last.
 */
export function facetChoices(rows: readonly LibraryRow[], filters: LibraryFilters, module: string | null, facet: MultiFacet, collator: Intl.Collator): FacetChoice[] {
  const key = searchKey(filters.search);
  const counts = new Map<FacetValue, number>();
  for (const row of rows) {
    const kept = keeps(row, filters, module, key, facet);
    for (const value of valuesOf(row, facet)) {
      counts.set(value, (counts.get(value) ?? 0) + (kept ? 1 : 0));
    }
  }
  for (const value of filters.chosen[facet]) {
    if (!counts.has(value)) {
      counts.set(value, 0);
    }
  }
  return Array.from(counts, ([value, count]) => ({ value, count })).sort((a, b) => {
    if (a.value === null || b.value === null) {
      return a.value === b.value ? 0 : a.value === null ? 1 : -1;
    }
    return b.count - a.count || collator.compare(a.value, b.value);
  });
}

/** One served module and the rows the other filters keep that it holds. */
export interface ModuleChoice {
  module: WorkspaceModule;
  count: number;
}

/** The Module facet: All modules' count (every row the other filters keep), then the served modules by title, each with its count. */
export function moduleChoices(rows: readonly LibraryRow[], filters: LibraryFilters, modules: readonly WorkspaceModule[], collator: Intl.Collator): { all: number; choices: ModuleChoice[] } {
  const key = searchKey(filters.search);
  const kept = rows.filter((row) => keeps(row, filters, null, key, "module"));
  const choices = modules.map((module) => ({ module, count: kept.filter((row) => row.modules.includes(module.name)).length }));
  return { all: kept.length, choices: choices.sort((a, b) => collator.compare(a.module.title, b.module.title)) };
}

/** The rows a toggle would keep given every other filter: those behind their decision, or the orphans. */
export function toggleCount(rows: readonly LibraryRow[], filters: LibraryFilters, module: string | null, toggle: "behind" | "orphans"): number {
  const key = searchKey(filters.search);
  return rows.filter((row) => keeps(row, filters, module, key, toggle) && (toggle === "behind" ? row.behind === true : row.orphan === true)).length;
}

/** A facet's choice with `value` pressed: added when it was not chosen, removed when it was. */
export function toggled(filters: LibraryFilters, facet: MultiFacet, value: FacetValue): LibraryFilters {
  const chosen = filters.chosen[facet];
  const next = chosen.includes(value) ? chosen.filter((each) => each !== value) : [...chosen, value];
  return { ...filters, chosen: { ...filters.chosen, [facet]: next } };
}

/** A facet back at All. */
export function atAll(filters: LibraryFilters, facet: MultiFacet): LibraryFilters {
  return { ...filters, chosen: { ...filters.chosen, [facet]: [] } };
}

/** What a row's link says: its title, else its name. */
export function titleOf(row: LibraryRow): string {
  return row.title ?? row.name;
}

/** A decision's link words: its title, else its name. */
export function decisionTitle(decision: LibraryDecision): string {
  return decision.title ?? decision.name;
}

// ---------------------------------------------------------------- order

/** The table's columns, in order; each header is `documents.column.<name>`. */
export const LIBRARY_COLUMNS = ["title", "class", "kind", "status", "version", "owner", "reviewed", "changed", "used", "decisions", "tags"] as const;

export type LibraryColumn = (typeof LIBRARY_COLUMNS)[number];

/** The columns a press orders the rows by: every one but Decisions and Tags, which hold lists. */
export const SORTED_COLUMNS = ["title", "class", "kind", "status", "version", "owner", "reviewed", "changed", "used"] as const;

export type SortedColumn = (typeof SORTED_COLUMNS)[number];

export function isSorted(column: LibraryColumn): column is SortedColumn {
  return SORTED_COLUMNS.some((each) => each === column);
}

/** The columns whose first press orders descending: the newest, the highest version, the most used first. */
const DESCENDING_FIRST: readonly SortedColumn[] = ["version", "reviewed", "changed", "used"];

/** A press's order; null: as served, until a press. */
export interface LibrarySort {
  column: SortedColumn;
  descending: boolean;
}

/** The order after a press on `column`: the same column reversed, else its first order. */
export function pressed(sort: LibrarySort | null, column: SortedColumn): LibrarySort {
  if (sort !== null && sort.column === column) {
    return { column, descending: !sort.descending };
  }
  return { column, descending: DESCENDING_FIRST.includes(column) };
}

/** A header's `aria-sort` while its column orders the rows. */
export function ariaSort(sort: LibrarySort | null, column: SortedColumn): "ascending" | "descending" | undefined {
  if (sort === null || sort.column !== column) {
    return undefined;
  }
  return sort.descending ? "descending" : "ascending";
}

/** A row's value in a column: words collate, numbers by size, a written date and a stored time by their text; null where none. */
function keyOf(row: LibraryRow, column: SortedColumn): { text: string } | { words: string } | { number: number } | null {
  const words = (value: string | null) => (value === null ? null : { words: value });
  switch (column) {
    case "title":
      return { words: titleOf(row) };
    case "class":
      return words(row.class);
    case "kind":
      return words(row.kind);
    case "status":
      return words(row.status);
    case "owner":
      return words(row.owner);
    case "version":
      return row.rev === null ? null : { number: row.rev };
    case "reviewed":
      return row.reviewed === null ? null : { text: row.reviewed };
    case "changed":
      return row.last_change === null ? null : { text: row.last_change.at };
    case "used":
      return { number: row.used_by };
  }
}

/**
 * The rows in a press's order, or as served before any press. Stable: ties keep served order in
 * either direction; a missing value stands last in either direction.
 */
export function sortedRows(rows: readonly LibraryRow[], sort: LibrarySort | null, collator: Intl.Collator): readonly LibraryRow[] {
  if (sort === null) {
    return rows;
  }
  const sign = sort.descending ? -1 : 1;
  const keyed = rows.map((row) => ({ row, key: keyOf(row, sort.column) }));
  keyed.sort((a, b) => {
    if (a.key === null || b.key === null) {
      return a.key === b.key ? 0 : a.key === null ? 1 : -1;
    }
    if ("number" in a.key && "number" in b.key) {
      return sign * (a.key.number - b.key.number);
    }
    if ("words" in a.key && "words" in b.key) {
      return sign * collator.compare(a.key.words, b.key.words);
    }
    const left = "text" in a.key ? a.key.text : "";
    const right = "text" in b.key ? b.key.text : "";
    return sign * (left < right ? -1 : left > right ? 1 : 0);
  });
  return keyed.map((entry) => entry.row);
}

// ---------------------------------------------------------------- decisions

/**
 * The core's four groups (`decision_group`) as the keys of the one table a served group is read
 * through (docs/canon/doc-view.md "Rules"); one is also a spec status of the mocks, never quoted here.
 */
const GROUP_WORDS: Readonly<Record<KnownDecisionGroup, true>> = { accepted: true, superseded: true, rejected: true, other: true };

/** A served group this build knows; another word is shown raw. */
export function knownGroup(group: string): KnownDecisionGroup | null {
  return Object.hasOwn(GROUP_WORDS, group) ? (group as KnownDecisionGroup) : null;
}

/** A row's decisions by served group, each group once in the order first served, entries in served order. */
export function groupsOf(decisions: readonly LibraryDecision[]): { group: string; entries: LibraryDecision[] }[] {
  const groups = new Map<string, LibraryDecision[]>();
  for (const decision of decisions) {
    groups.set(decision.group, [...(groups.get(decision.group) ?? []), decision]);
  }
  return Array.from(groups, ([group, entries]) => ({ group, entries }));
}

/** A group's heading: the known group's word, else the served word raw (`raw` true, shown under `lang="en"`). */
export function groupLabel(group: string, t: Translate): { label: string; raw: boolean } {
  const known = knownGroup(group);
  return known === null ? { label: group, raw: true } : { label: t(`documents.group.${known}`), raw: false };
}

/** One part of a row's Decisions cell: a served group, its count, and the group as this build knows it (null: shown raw under `lang="en"`). */
export interface DecisionCount {
  group: string;
  count: number;
  known: KnownDecisionGroup | null;
}

/** A row's Decisions cell's parts: each served group's count, in served order. */
export function decisionCounts(decisions: readonly LibraryDecision[]): DecisionCount[] {
  return groupsOf(decisions).map(({ group, entries }) => ({ group, count: entries.length, known: knownGroup(group) }));
}

/** A known group's count in words, "2 accepted"; an unknown one's chrome around its slot `group`, filled by the caller with the raw word. */
export function decisionCountText(part: DecisionCount, t: Translate, group: string): string {
  return part.known === null ? t("documents.decisions.unknown", { number: part.count, group }) : t(`documents.decisions.${part.known}`, { count: part.count });
}
