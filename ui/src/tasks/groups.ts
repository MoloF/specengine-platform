import type { KnownTaskStatus, TaskListEntry, TaskStatus } from "../api/types";
import { isKnownTaskStatus, taskStatusLook } from "./labels";

// The list's groups (docs/features/ui-tasks.md "Description and interactions", List), taken from
// `status` and `stale` as the daemon sent them; nothing is computed or held here (ADR-0012).

/**
 * Where each state's group stands. A plan in review always waits for the owner; a ready task waits
 * when its spec changed since approval. `done` and `cancelled` share Closed; the three reserved
 * states come after it, each under its own label; an unknown state last.
 */
const RANK: Record<KnownTaskStatus, number> = {
  review: 0,
  draft: 1,
  changes_requested: 2,
  ready: 3,
  in_progress: 4,
  done: 5,
  cancelled: 5,
  analysis: 6,
  in_review: 7,
  accepted: 8,
};

const WAITING_RANK = 0;
const CLOSED_RANK = 5;
const UNKNOWN_RANK = 9;

/** What waits for the owner now: a plan to review, or a ready task whose spec changed since approval. */
export function waitsForYou(entry: Pick<TaskListEntry, "status" | "stale">): boolean {
  return entry.status === "review" || (entry.status === "ready" && entry.stale === true);
}

/** `done` and `cancelled`: behind the Closed chip. */
export function isClosed(status: TaskStatus): boolean {
  return status === "done" || status === "cancelled";
}

function rankOf(entry: TaskListEntry): number {
  if (waitsForYou(entry)) {
    return WAITING_RANK;
  }
  return isKnownTaskStatus(entry.status) ? RANK[entry.status as KnownTaskStatus] : UNKNOWN_RANK;
}

/** Task IDs by number: T-0999 before T-1000, any other text after them as text. */
export function byNumber(a: string, b: string): number {
  return a.localeCompare(b, "en", { numeric: true });
}

export interface TaskGroup {
  key: string;
  title: string;
  waiting: boolean;
  entries: TaskListEntry[];
}

function titleOf(rank: number, entry: TaskListEntry): string {
  if (rank === WAITING_RANK) {
    return "Waiting for you";
  }
  if (rank === CLOSED_RANK) {
    return "Closed";
  }
  if (rank === UNKNOWN_RANK) {
    return "Other states";
  }
  return taskStatusLook(entry.status).label;
}

/** The entries in their groups, groups in order, entries by number within each. */
export function groupsOf(entries: readonly TaskListEntry[]): TaskGroup[] {
  const ranked = new Map<number, TaskListEntry[]>();
  for (const entry of entries) {
    const rank = rankOf(entry);
    const members = ranked.get(rank) ?? [];
    members.push(entry);
    ranked.set(rank, members);
  }
  return [...ranked.entries()]
    .sort(([a], [b]) => a - b)
    .map(([rank, members]) => {
      const sorted = [...members].sort((a, b) => byNumber(a.id, b.id));
      const first = sorted[0];
      return {
        key: `group-${String(rank)}`,
        title: first === undefined ? "" : titleOf(rank, first),
        waiting: rank === WAITING_RANK,
        entries: sorted,
      };
    });
}

/** The rank of a state's chip: as its group, the waiting one at review's place. */
export function chipRank(status: TaskStatus): number {
  return isKnownTaskStatus(status) ? RANK[status as KnownTaskStatus] : UNKNOWN_RANK;
}
