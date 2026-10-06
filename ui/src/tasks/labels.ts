import type { KnownRunOutcome, KnownTaskStatus, RunOutcome, TaskStatus } from "../api/types";
import type { Look } from "../ui/Badge";

// The closed tables of the Tasks screen: the ten task states (05 §3.3) and the four run outcomes
// (docs/features/task-package.md "Data"). Known literals get a label, a colour role and an icon;
// any other string is shown raw in a neutral badge, never mapped to a known value.

function lookup<K extends string, V>(table: Record<K, V>, value: string): V | null {
  return Object.hasOwn(table, value) ? table[value as K] : null;
}

const STATUS: Record<KnownTaskStatus, Look> = {
  draft: { label: "Draft", tone: "task-draft", icon: "taskDraft" },
  analysis: { label: "Analysis", tone: "task-analysis", icon: "search" },
  review: { label: "Plan review", tone: "task-review", icon: "taskReview" },
  changes_requested: { label: "Changes requested", tone: "task-changes-requested", icon: "changes" },
  ready: { label: "Ready", tone: "task-ready", icon: "taskReady" },
  in_progress: { label: "In progress", tone: "task-in-progress", icon: "taskInProgress" },
  in_review: { label: "Work in review", tone: "task-in-review", icon: "taskInReview" },
  done: { label: "Done", tone: "task-done", icon: "applied" },
  accepted: { label: "Accepted", tone: "task-accepted", icon: "taskAccepted" },
  cancelled: { label: "Cancelled", tone: "task-cancelled", icon: "taskCancelled" },
};

/** A task state's look; the raw text in a neutral badge when unknown. */
export function taskStatusLook(status: TaskStatus): Look {
  return lookup(STATUS, status) ?? { label: status, tone: "task-unknown", icon: "unknown" };
}

/** Whether the state is one of the ten. */
export function isKnownTaskStatus(status: TaskStatus): boolean {
  return Object.hasOwn(STATUS, status);
}

const OUTCOME: Record<KnownRunOutcome, Look> = {
  completed: { label: "Completed", tone: "run-completed", icon: "applied" },
  partial: { label: "Partly done", tone: "run-partial", icon: "runPartial" },
  failed: { label: "Failed", tone: "run-failed", icon: "alert" },
  abandoned: { label: "Abandoned", tone: "run-abandoned", icon: "runAbandoned" },
};

/** A run outcome's look; the raw text in a neutral badge when unknown. */
export function runOutcomeLook(outcome: RunOutcome): Look {
  return lookup(OUTCOME, outcome) ?? { label: outcome, tone: "run-unknown", icon: "unknown" };
}

/**
 * The staleness of a package, as the core computed it (never derived here): changed, unchanged,
 * unknown (with a snapshot; its notes say why), or nothing to compare (no snapshot: no badge).
 */
export function stalenessLook(stale: boolean | null, hasSnapshot: boolean): Look | null {
  if (!hasSnapshot) {
    return null;
  }
  if (stale === true) {
    return { label: "Spec changed since approval", tone: "stale-changed", icon: "specChanged" };
  }
  if (stale === false) {
    return { label: "Unchanged since approval", tone: "stale-unchanged", icon: "approved" };
  }
  return { label: "Unknown", tone: "stale-unknown", icon: "unknown" };
}

/** What stands in for the staleness badge before any approval. */
export const FROZEN_AT_APPROVAL = "The spec is frozen at approval.";
