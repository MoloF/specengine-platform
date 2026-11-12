import type { CatalogRow } from "../api/types";
import { ROLLUP_WORDS } from "../flow/labels";
import type { Translate } from "../i18n";
import { rowName } from "./rows";

// A direction's table (docs/features/direction-catalog.md "Acceptance criteria", DC-10): its
// columns, what a press on each orders the served rows by, and a row's words. The one place a state
// word is compared: its place in ROLLUP_WORDS, a word this build does not know last; its look is
// `rollupLook`'s (src/flow/labels.ts). Sorting lays out served values once per press; nothing here
// counts, judges or reads git (docs/canon/architecture.md#ui).

/** The rows shown before Show more, and how many more each press shows. */
export const CATALOG_SHOWN = 50;

/** The columns, in the table's order; each one's header is `tree.catalog.column.<name>`. */
export const CATALOG_COLUMNS = ["title", "version", "state", "open", "tasks", "changed"] as const;

export type CatalogColumn = (typeof CATALOG_COLUMNS)[number];

/** A press's order: its column and its direction. Null: the served order, until a press. */
export interface CatalogSort {
  column: CatalogColumn;
  descending: boolean;
}

/** The order after a press on `column`: the same column reversed, else its first order, Title and State ascending, the rest descending. */
export function pressed(sort: CatalogSort | null, column: CatalogColumn): CatalogSort {
  if (sort !== null && sort.column === column) {
    return { column, descending: !sort.descending };
  }
  return { column, descending: column !== "title" && column !== "state" };
}

/** A header's `aria-sort`: its direction while its column orders the rows, else none. */
export function ariaSort(sort: CatalogSort | null, column: CatalogColumn): "ascending" | "descending" | undefined {
  if (sort === null || sort.column !== column) {
    return undefined;
  }
  return sort.descending ? "descending" : "ascending";
}

/** What a row's link says: its title, else its name (its ID, else its path). */
export function titleOf(row: CatalogRow): string {
  return row.title ?? rowName(row);
}

/** `Version N`, or No version where none is written. */
export function versionText(row: CatalogRow, t: Translate): string {
  return row.rev === null ? t("tree.catalog.noVersion") : t("tree.catalog.version", { rev: row.rev });
}

/**
 * The served pair, compact: `3 · 1 open`, the tasks and how many of them are open; a bare `0` where
 * both are 0, the one case the count says all.
 */
export function tasksText(row: CatalogRow, t: Translate): string {
  if (row.tasks === 0 && row.tasks_open === 0) {
    return String(row.tasks);
  }
  return t("tree.catalog.tasks", { count: row.tasks, open: row.tasks_open });
}

/** A state's place among the rollups, done first; null for a word this build does not know. */
function stateRank(state: string): number | null {
  const at = ROLLUP_WORDS.indexOf(state);
  return at < 0 ? null : at;
}

/** A row's value in a column; null where it has none: no version, no change read, a state unknown here. */
function keyOf(row: CatalogRow, column: CatalogColumn): string | number | null {
  switch (column) {
    case "title":
      return titleOf(row);
    case "version":
      return row.rev;
    case "state":
      return stateRank(row.state);
    case "open":
      return row.open_items;
    case "tasks":
      return row.tasks;
    case "changed":
      return row.last_change?.at ?? null;
  }
}

/** Two present values: numbers by size, a title by the viewer's collation, a stored time by its text. */
function compared(a: string | number, b: string | number, column: CatalogColumn, collator: Intl.Collator): number {
  if (typeof a === "number" && typeof b === "number") {
    return a - b;
  }
  const left = String(a);
  const right = String(b);
  if (column === "title") {
    return collator.compare(left, right);
  }
  return left < right ? -1 : left > right ? 1 : 0;
}

/**
 * The served rows in a press's order, or as served before any press. Stable: ties keep served
 * order, in either direction. A missing value (no version, no change read, a state this build does
 * not know) stands last in either direction.
 */
export function sortedRows(rows: readonly CatalogRow[], sort: CatalogSort | null, collator: Intl.Collator): readonly CatalogRow[] {
  if (sort === null) {
    return rows;
  }
  const sign = sort.descending ? -1 : 1;
  const keyed = rows.map((row) => ({ row, key: keyOf(row, sort.column) }));
  keyed.sort((a, b) => {
    if (a.key === null || b.key === null) {
      return a.key === b.key ? 0 : a.key === null ? 1 : -1;
    }
    return sign * compared(a.key, b.key, sort.column, collator);
  });
  return keyed.map((entry) => entry.row);
}
