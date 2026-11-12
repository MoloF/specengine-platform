import type { KnownRunOutcome, KnownTaskStatus, RunOutcome, TaskStatus } from "../api/types";
import type { Translate } from "../i18n";
import type { IconName } from "../ui/Icon";
import type { Look } from "../ui/Badge";
import type { LozengeAppearance, LozengeLook } from "../ui/Lozenge";

// The closed tables of the Tasks screen: the ten task states (05 §3.3,
// `docs/canon/tasks.md` "Transitions") and the four run outcomes
// (`docs/canon/task-package.md` "Caps"). Known literals get a label from the catalog's `tasks` group,
// a colour role and an icon; any other string is shown raw in a neutral badge, never mapped to a
// known value.

type Mark = Omit<Look, "label">;

function lookup<K extends string, V>(table: Record<K, V>, value: string): V | null {
  return Object.hasOwn(table, value) ? table[value as K] : null;
}

const STATUS: Record<KnownTaskStatus, Mark> = {
  draft: { tone: "task-draft", icon: "taskDraft" },
  analysis: { tone: "task-analysis", icon: "search" },
  review: { tone: "task-review", icon: "taskReview" },
  changes_requested: { tone: "task-changes-requested", icon: "changes" },
  ready: { tone: "task-ready", icon: "taskReady" },
  in_progress: { tone: "task-in-progress", icon: "taskInProgress" },
  in_review: { tone: "task-in-review", icon: "taskInReview" },
  done: { tone: "task-done", icon: "applied" },
  accepted: { tone: "task-accepted", icon: "taskAccepted" },
  cancelled: { tone: "task-cancelled", icon: "taskCancelled" },
};

/** A task state's look; the raw text in a neutral badge when unknown. */
export function taskStatusLook(status: TaskStatus, t: Translate): Look {
  const mark = lookup(STATUS, status);
  return mark === null ? { label: status, tone: "task-unknown", icon: "unknown", served: true } : { label: t(`tasks.status.${status as KnownTaskStatus}`), ...mark };
}

/** Each state's lozenge appearance: waiting on the owner warning, under way information, finished success. */
const STATUS_APPEARANCE: Record<KnownTaskStatus, LozengeAppearance> = {
  draft: "neutral",
  analysis: "neutral",
  review: "warning",
  changes_requested: "warning",
  ready: "information",
  in_progress: "information",
  in_review: "information",
  done: "success",
  accepted: "success",
  cancelled: "neutral",
};

/**
 * A task's state in its head, as a lozenge (docs/features/design-sweep-b.md "Acceptance criteria",
 * DB5): the badge's label and icon, one of the appearances; a state this build does not know raw
 * and neutral, under `lang="en"`, never mapped to a known one. The list's rows keep the badge.
 */
export function taskStatusLozenge(status: TaskStatus, t: Translate): LozengeLook & { lang?: "en" } {
  const mark = lookup(STATUS, status);
  const appearance = lookup(STATUS_APPEARANCE, status);
  if (mark === null || appearance === null) {
    return { appearance: "neutral", icon: "unknown", label: status, lang: "en" };
  }
  return { appearance, icon: mark.icon, label: t(`tasks.status.${status as KnownTaskStatus}`) };
}

/** Whether the state is one of the ten. */
export function isKnownTaskStatus(status: TaskStatus): boolean {
  return Object.hasOwn(STATUS, status);
}

/** The ten states, in the table's order. */
export const KNOWN_TASK_STATUSES = Object.keys(STATUS) as readonly KnownTaskStatus[];

const OUTCOME: Record<KnownRunOutcome, Mark> = {
  completed: { tone: "run-completed", icon: "applied" },
  partial: { tone: "run-partial", icon: "runPartial" },
  failed: { tone: "run-failed", icon: "alert" },
  abandoned: { tone: "run-abandoned", icon: "runAbandoned" },
};

/** The four outcomes, in the table's order. */
export const KNOWN_RUN_OUTCOMES = Object.keys(OUTCOME) as readonly KnownRunOutcome[];

/** A run outcome's look; the raw text in a neutral badge when unknown. */
export function runOutcomeLook(outcome: RunOutcome, t: Translate): Look {
  const mark = lookup(OUTCOME, outcome);
  return mark === null ? { label: outcome, tone: "run-unknown", icon: "unknown", served: true } : { label: t(`tasks.outcome.${outcome as KnownRunOutcome}`), ...mark };
}

const STALENESS: Record<"changed" | "unchanged" | "unknown", { tone: string; icon: IconName }> = {
  changed: { tone: "stale-changed", icon: "specChanged" },
  unchanged: { tone: "stale-unchanged", icon: "approved" },
  unknown: { tone: "stale-unknown", icon: "unknown" },
};

/**
 * The staleness of a package, as the core computed it (never derived here): changed, unchanged,
 * unknown (with a snapshot; its notes say why), or nothing to compare (no snapshot: no badge).
 */
export function stalenessLook(stale: boolean | null, hasSnapshot: boolean, t: Translate): Look | null {
  if (!hasSnapshot) {
    return null;
  }
  const state = stale === true ? "changed" : stale === false ? "unchanged" : "unknown";
  return { label: t(`tasks.staleness.${state}`), ...STALENESS[state] };
}

/** A list row's mark for `stale` true (the Tasks list, the home): the spec changed since approval. */
export function specChanged(t: Translate): Look {
  return { label: t("tasks.specChanged"), tone: "stale-changed", icon: "specChanged" };
}

/** What stands in for the staleness badge before any approval. */
export function frozenAtApproval(t: Translate): string {
  return t("tasks.frozenAtApproval");
}
