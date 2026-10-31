import type { CardFlow, CardList, CardRow } from "../api/types";
import { folded } from "../tasks/filter";
import { NOT_IN_THE_FLOW, UNASSIGNED } from "./labels";

// The Board's arrangement of the one answer already read: the filters, the tabs, the columns, where
// Up, Down and a drop put a card. Rows keep the daemon's order; no word of a project is read here.

/** A filter showing everything. */
export const ANY = "";

/** The Assignee filter's "Unassigned": it has capitals, so no folded name can equal it. */
export const NOBODY = "(Unassigned)";

/** The cards a column shows before "Show all <n>". */
export const COLUMN_SHOWN = 200;

/** The filters, per project: each a folded value, ANY, or for the assignee NOBODY. */
export interface BoardFilters {
  discipline: string;
  assignee: string;
}

export const NO_FILTERS: BoardFilters = { discipline: ANY, assignee: ANY };

/** The two filters ANDed, compared folded (`ui/README.md` "Screen rules", Text). */
export function matchesFilters(row: CardRow, filters: BoardFilters): boolean {
  const discipline = filters.discipline === ANY || (row.discipline !== null && folded(row.discipline) === filters.discipline);
  const assignee = filters.assignee === ANY || (filters.assignee === NOBODY ? row.assignee === null : row.assignee !== null && folded(row.assignee) === filters.assignee);
  return discipline && assignee;
}

/** One entry of a filter's list: the folded key, the first spelling read. */
export interface Choice {
  key: string;
  label: string;
}

/** The values read, each once by its folded form, in the order read; a value `kept` no card holds now stays listed. */
function choicesOf(values: readonly (string | null)[], kept: string): Choice[] {
  const choices = new Map<string, string>();
  for (const value of values) {
    if (value !== null && !choices.has(folded(value))) {
      choices.set(folded(value), value);
    }
  }
  if (kept !== ANY && kept !== NOBODY && !choices.has(kept)) {
    choices.set(kept, kept);
  }
  return Array.from(choices, ([key, label]) => ({ key, label }));
}

export function disciplineChoices(rows: readonly CardRow[], kept: string): Choice[] {
  return choicesOf(
    rows.map((row) => row.discipline),
    kept,
  );
}

/** The names read, then "Unassigned". */
export function assigneeChoices(rows: readonly CardRow[], kept: string): Choice[] {
  return [
    ...choicesOf(
      rows.map((row) => row.assignee),
      kept,
    ),
    { key: NOBODY, label: UNASSIGNED },
  ];
}

/** One board's tab: a flow served, or one only a row still names. */
export interface BoardTab {
  name: string;
  flow: CardFlow | undefined;
}

/** A tab per `flows[]` in served order, then any flow `[card]` no longer holds, so no row is lost. */
export function tabsOf(list: CardList): BoardTab[] {
  const tabs: BoardTab[] = list.flows.map((flow) => ({ name: flow.name, flow }));
  for (const row of list.cards) {
    if (!tabs.some((tab) => tab.name === row.flow)) {
      tabs.push({ name: row.flow, flow: undefined });
    }
  }
  return tabs;
}

/** One column on a board: `column` null for the lane of cards outside `columns[]`; `key` holds the flow's name, so two flows' columns of one name stay apart. */
export interface BoardLane {
  key: string;
  column: string | null;
  title: string;
  /** Standing in it, a cancelled card having left: as `spec card list` counts. */
  count: number;
  rows: CardRow[];
}

/** The rows that stand in a column: the moves and drops place a card among these. */
export function openRows(rows: readonly CardRow[]): CardRow[] {
  return rows.filter((row) => !row.cancelled);
}

function laneOf(key: string, column: string | null, title: string, rows: CardRow[]): BoardLane {
  return { key, column, title, count: openRows(rows).length, rows };
}

/** A board's columns in served order, empty ones too, then a last lane for the rows outside them. */
export function lanesOf({ name, flow }: BoardTab, rows: readonly CardRow[]): BoardLane[] {
  const own = rows.filter((row) => row.flow === name);
  const columns = flow?.columns ?? [];
  const lanes = columns.map((column) =>
    laneOf(
      `${name}/column:${column.id}`,
      column.id,
      column.title,
      own.filter((row) => row.column === column.id),
    ),
  );
  const outside = own.filter((row) => !columns.some((column) => column.id === row.column));
  return outside.length === 0 ? lanes : [...lanes, laneOf(`${name}/outside`, null, NOT_IN_THE_FLOW, outside)];
}

/** Where Up and Down put a card; `before` null: last in the column. */
export interface Reorder {
  up: { before: string } | null;
  down: { before: string | null } | null;
}

/**
 * Up takes the place of the card above as shown; Down goes after the card below as shown, so
 * `before` is the card after that one in the whole column, shown or not, none: last.
 */
export function reorderOf(id: string, shown: readonly CardRow[], whole: readonly CardRow[]): Reorder {
  const at = shown.findIndex((row) => row.id === id);
  const above = shown[at - 1];
  const below = at < 0 ? undefined : shown[at + 1];
  const after = below === undefined ? undefined : whole[whole.findIndex((row) => row.id === below.id) + 1];
  return {
    up: above === undefined ? null : { before: above.id },
    down: below === undefined ? null : { before: after?.id ?? null },
  };
}

/** Where a drag lands among `standing`: a cancelled card has left its column and takes no `before`, so past the last that stands: null. */
export function standingBefore(before: string | null, standing: readonly CardRow[]): string | null {
  return standing.some((row) => row.id === before) ? before : null;
}

/** What a drop asks, `standing` the column's open rows: `null` when the card would stay where it stands. */
export function dropMove(row: CardRow, column: string, before: string | null, standing: readonly CardRow[]): { to: string; before?: string } | null {
  const place = standingBefore(before, standing);
  if (row.column === column) {
    const at = standing.findIndex((candidate) => candidate.id === row.id);
    if (place === row.id || place === (standing[at + 1]?.id ?? null)) {
      return null;
    }
  }
  return place === null ? { to: column } : { to: column, before: place };
}
