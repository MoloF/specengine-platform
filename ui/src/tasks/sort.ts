import type { TaskListEntry } from "../api/types";
import type { Translate } from "../i18n";
import { byNumber, type TaskGroup } from "./groups";

// The task list's Sort (docs/features/auto-feed.md "The UI", Tasks): By state, today's groups and
// the default, or Last changed, one list by `updated_at`, newest first, ties by ID number, highest
// first. Kept per viewer and project, `specengine.tasks.sort.<p>`; storage that refuses keeps it for
// the page alone.

/** The list's two orders. */
export type TaskSort = "state" | "changed";

/** Where a project's order is kept, per viewer. */
export function sortKey(project: string): string {
  return `specengine.tasks.sort.${project}`;
}

/** The order kept for `project`; anything else, none, or storage that refuses: By state. */
export function recallSort(project: string): TaskSort {
  try {
    return window.localStorage.getItem(sortKey(project)) === "changed" ? "changed" : "state";
  } catch {
    return "state";
  }
}

/** Keeps the order; storage that refuses keeps it in the list's state until a reload. */
export function rememberSort(project: string, sort: TaskSort): void {
  try {
    window.localStorage.setItem(sortKey(project), sort);
  } catch {
    // Held in the list's state alone.
  }
}

/** Newest change first; equal or unreadable times by ID number, the highest first. */
function byLastChange(a: TaskListEntry, b: TaskListEntry): number {
  const later = Date.parse(b.updated_at) - Date.parse(a.updated_at);
  return Number.isNaN(later) || later === 0 ? byNumber(b.id, a.id) : later;
}

/** Last changed: every entry in one group, newest change first; none for no entry. */
export function changedGroups(entries: readonly TaskListEntry[], t: Translate): TaskGroup[] {
  return entries.length === 0 ? [] : [{ key: "group-changed", title: t("tasks.sort.group"), waiting: false, entries: [...entries].sort(byLastChange) }];
}
