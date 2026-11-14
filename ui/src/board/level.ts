import type { CardRow } from "../api/types";

// The Board's Show select (docs/features/board-subtasks.md "Description and interactions"): one
// more cut of the one list read, ANDed with Discipline and Assignee where the rows shown are cut.
// It reads `parent` alone: All cards, the tasks without a parent, or the subtasks at any depth.
// Stored per viewer and project, as the lane mode is; never in the URL or the Shell's screen state.

/** Which cards the Board draws, by whether they have a parent. */
export type BoardLevel = "all" | "tasks" | "subtasks";

/** The levels in the select's order. */
export const BOARD_LEVELS: readonly BoardLevel[] = ["all", "tasks", "subtasks"];

function isBoardLevel(value: string | null): value is BoardLevel {
  return value !== null && (BOARD_LEVELS as readonly string[]).includes(value);
}

/** Where a project's level is kept, per viewer. */
export function levelKey(project: string): string {
  return `specengine.board.level.${project}`;
}

/** The level stored for `project`; an unknown or missing value, or storage that refuses, reads as All cards. */
export function recallLevel(project: string): BoardLevel {
  try {
    const stored = window.localStorage.getItem(levelKey(project));
    return isBoardLevel(stored) ? stored : "all";
  } catch {
    return "all";
  }
}

/** Keeps the level; storage that refuses keeps it for this page alone (the caller's state). */
export function rememberLevel(project: string, level: BoardLevel): void {
  try {
    window.localStorage.setItem(levelKey(project), level);
  } catch {
    // The choice holds in the Board's state until a reload.
  }
}

/** Whether a row is drawn at `level`: Tasks only keeps the rows with no parent, Subtasks only the rest. */
export function matchesLevel(row: Pick<CardRow, "parent">, level: BoardLevel): boolean {
  switch (level) {
    case "tasks":
      return row.parent === null;
    case "subtasks":
      return row.parent !== null;
    default:
      return true;
  }
}
