import type { CardRow } from "../api/types";
import { folded } from "../tasks/filter";
import { cellsOf, openRows, type BoardColumnCell, type BoardColumnHead, type BoardTab } from "./filter";

// The Board's swimlanes (`ui/README.md` "Screen rules", Shell): one more way to cut the one list
// read, never a read of its own. None draws one lane; Assignee one per name and then Unassigned;
// Parent one per parent card and then everything else. A lane's place comes from what heads it,
// never from its first row, so a move that changes a lane's rows never reorders the lanes. A drop
// in any lane sends its column alone: a lane neither assigns nor re-parents.

/** How the rows are cut into lanes; stored per viewer and project. */
export type LaneMode = "none" | "assignee" | "parent";

/** The modes in the select's order. */
export const LANE_MODES: readonly LaneMode[] = ["none", "assignee", "parent"];

function isLaneMode(value: string | null): value is LaneMode {
  return value !== null && (LANE_MODES as readonly string[]).includes(value);
}

/** Where a project's lane mode is kept, per viewer. */
export function laneModeKey(project: string): string {
  return `specengine.board.lanes.${project}`;
}

/** The mode stored for `project`; an unknown or missing value, or storage that refuses, reads as None. */
export function recallLaneMode(project: string): LaneMode {
  try {
    const stored = window.localStorage.getItem(laneModeKey(project));
    return isLaneMode(stored) ? stored : "none";
  } catch {
    return "none";
  }
}

/** Keeps the mode; storage that refuses keeps it for this page alone (the caller's state). */
export function rememberLaneMode(project: string, mode: LaneMode): void {
  try {
    window.localStorage.setItem(laneModeKey(project), mode);
  } catch {
    // The choice holds in the Board's state until a reload.
  }
}

/** What heads a lane: nothing (None), a person, nobody, a parent card (its row when the list holds it), or the rest. */
export type LaneHead =
  | { cut: "all" }
  | { cut: "assignee"; name: string }
  | { cut: "unassigned" }
  | { cut: "parent"; id: string; card: CardRow | undefined }
  | { cut: "rest" };

/** One swimlane: its head, a cell per column of the board, and the cards standing in it. */
export interface Swimlane {
  key: string;
  head: LaneHead;
  cells: BoardColumnCell[];
  count: number;
}

/** The rows by one of their values, each group in the rows' order: one pass, however many lanes. */
function groupOf(rows: readonly CardRow[], valueOf: (row: CardRow) => string | null): Map<string | null, CardRow[]> {
  const groups = new Map<string | null, CardRow[]>();
  for (const row of rows) {
    const value = valueOf(row);
    const group = groups.get(value);
    if (group === undefined) {
      groups.set(value, [row]);
    } else {
      group.push(row);
    }
  }
  return groups;
}

/** A card ID's number: its digits; none sorts last. */
function numberOf(id: string): number {
  const digits = /\d+/.exec(id)?.[0];
  return digits === undefined ? Number.POSITIVE_INFINITY : Number(digits);
}

function byText(a: string, b: string): number {
  if (a === b) {
    return 0;
  }
  return a < b ? -1 : 1;
}

/** Names folded as the filters fold them, ties by the raw name. */
function byName(a: string, b: string): number {
  return byText(folded(a), folded(b)) || byText(a, b);
}

/** Parent IDs by their number, ties by the raw ID. */
function byNumber(a: string, b: string): number {
  return numberOf(a) - numberOf(b) || byText(a, b);
}

/**
 * The lanes of one board: `rows` the tab's rows as shown (filters applied), `all` the whole list, for
 * a parent standing in another flow, a hidden one too. A mode that finds no lane draws the one lane
 * of None, so the columns still take a drop.
 */
export function swimlanesOf(tab: BoardTab, heads: readonly BoardColumnHead[], rows: readonly CardRow[], mode: LaneMode, all: readonly CardRow[]): Swimlane[] {
  const own = rows.filter((row) => row.flow === tab.name);
  const lane = (suffix: string, head: LaneHead, members: readonly CardRow[]): Swimlane => {
    const key = `${tab.name}/lane:${suffix}`;
    return { key, head, cells: cellsOf(tab, heads, members, `${key}|`), count: openRows(members).length };
  };
  const lanes: Swimlane[] = [];
  if (mode === "assignee") {
    // One pass groups the rows by name, in the daemon's order; the names then sort.
    const byPerson = groupOf(own, (row) => row.assignee);
    for (const name of [...byPerson.keys()].flatMap((key) => (key === null ? [] : [key])).sort(byName)) {
      lanes.push(lane(`assignee:${name}`, { cut: "assignee", name }, byPerson.get(name) ?? []));
    }
    const nobody = byPerson.get(null);
    if (nobody !== undefined) {
      lanes.push(lane("unassigned", { cut: "unassigned" }, nobody));
    }
  } else if (mode === "parent") {
    const byParent = groupOf(own, (row) => row.parent);
    const cards = new Map(all.map((row) => [row.id, row] as const));
    for (const id of [...byParent.keys()].flatMap((key) => (key === null ? [] : [key])).sort(byNumber)) {
      lanes.push(lane(`parent:${id}`, { cut: "parent", id, card: cards.get(id) }, byParent.get(id) ?? []));
    }
    const rest = byParent.get(null);
    if (rest !== undefined) {
      lanes.push(lane("rest", { cut: "rest" }, rest));
    }
  }
  return lanes.length > 0 ? lanes : [lane("all", { cut: "all" }, own)];
}
