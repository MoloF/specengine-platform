import type { TaskListEntry, TaskStatus } from "../api/types";
import { byNumber, chipRank, isClosed } from "./groups";
import { taskStatusLook } from "./labels";

// The list's filters, over the one answer already read: a chip per state present (`done` and
// `cancelled` one Closed chip), "Spec changed", and a text field over ID, title and targets. No
// filter reads again.

const CLOSED_CHIP = "closed";

/** A chip's key: `closed`, else `state:<status>` (a state named closed stays its own chip). */
export function chipKeyOf(status: TaskStatus): string {
  return isClosed(status) ? CLOSED_CHIP : `state:${status}`;
}

export interface TaskFilters {
  /** Chips the owner toggled away from their default; absent: pressed, but Closed released. */
  toggled: ReadonlyMap<string, boolean>;
  /** "Spec changed" pressed: only tasks whose spec changed since approval. */
  changedOnly: boolean;
  query: string;
}

export const DEFAULT_FILTERS: TaskFilters = { toggled: new Map(), changedOnly: false, query: "" };

export function isPressed(filters: TaskFilters, key: string): boolean {
  return filters.toggled.get(key) ?? key !== CLOSED_CHIP;
}

export function toggleChip(filters: TaskFilters, key: string): TaskFilters {
  const toggled = new Map(filters.toggled);
  toggled.set(key, !isPressed(filters, key));
  return { ...filters, toggled };
}

/** Whether the filters differ from the defaults (Clear filters has something to do). */
export function isFiltered(filters: TaskFilters, keys: readonly string[]): boolean {
  return filters.changedOnly || filters.query !== "" || keys.some((key) => isPressed(filters, key) !== (key !== CLOSED_CHIP));
}

/** Text as the filter compares it: NFC, lower case. */
export function folded(text: string): string {
  return text.normalize("NFC").toLowerCase();
}

function matchesQuery(entry: TaskListEntry, query: string): boolean {
  const wanted = folded(query.trim());
  if (wanted === "") {
    return true;
  }
  return [entry.id, entry.title ?? "", ...entry.targets].some((text) => folded(text).includes(wanted));
}

export function matchesFilters(entry: TaskListEntry, filters: TaskFilters): boolean {
  return (
    isPressed(filters, chipKeyOf(entry.status)) &&
    (!filters.changedOnly || entry.stale === true) &&
    matchesQuery(entry, filters.query)
  );
}

export interface Chip {
  key: string;
  label: string;
  /** Tasks of the whole answer under this chip. */
  count: number;
}

/** One chip per state present, in the groups' order; unknown states raw, after the known ones. */
export function chipsOf(entries: readonly TaskListEntry[]): Chip[] {
  const chips = new Map<string, { chip: Chip; rank: number; raw: string }>();
  for (const entry of entries) {
    const key = chipKeyOf(entry.status);
    const known = chips.get(key);
    if (known !== undefined) {
      known.chip.count += 1;
      continue;
    }
    const label = isClosed(entry.status) ? "Closed" : taskStatusLook(entry.status).label;
    chips.set(key, { chip: { key, label, count: 1 }, rank: chipRank(entry.status), raw: entry.status });
  }
  return [...chips.values()].sort((a, b) => a.rank - b.rank || byNumber(a.raw, b.raw)).map(({ chip }) => chip);
}
