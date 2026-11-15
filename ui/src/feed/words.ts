import type { FeedEntry } from "../api/types";
import type { Translate } from "../i18n";

// What an entry of the feed says (docs/features/auto-feed.md "The UI"): the type's label in the
// viewer's words, from a closed table; a type no table knows is shown as stored. The labels a
// screen already says are its words (the Board's task events, the Inbox's states, the Tasks
// screen's), so one event reads alike wherever it shows.

/** Each known type's label, read through the catalog. */
const TYPE_LABELS: Readonly<Record<string, (t: Translate) => string>> = {
  "proposal.created": (t) => t("board.activity.created"),
  "proposal.approved": (t) => t("inbox.status.approved"),
  "proposal.applied": (t) => t("inbox.status.applied"),
  "proposal.rejected": (t) => t("inbox.status.rejected"),
  "proposal.apply_failed": (t) => t("board.activity.apply_failed"),
  "proposal.staged": (t) => t("board.activity.staged"),
  "proposal.unstaged": (t) => t("board.activity.unstaged"),
  "task.created": (t) => t("board.event.created"),
  "task.planned": (t) => t("feed.type.planned"),
  "task.approved": (t) => t("inbox.status.approved"),
  "task.changes_requested": (t) => t("tasks.status.changes_requested"),
  "task.claimed": (t) => t("tasks.runs.claimed"),
  "task.run_reported": (t) => t("feed.type.run_reported"),
  "task.completed": (t) => t("tasks.outcome.completed"),
  "task.cancelled": (t) => t("tasks.status.cancelled"),
  "task.refreshed": (t) => t("board.event.refreshed"),
  "card.created": (t) => t("board.event.created"),
  "card.moved": (t) => t("board.event.moved"),
  "card.accepted": (t) => t("board.event.accepted"),
  "card.edited": (t) => t("board.event.edited"),
  "card.assigned": (t) => t("board.event.assigned"),
  "card.cancelled": (t) => t("board.event.cancelled"),
  "card.cleared": (t) => t("board.event.cleared"),
  "card.reestimated": (t) => t("board.event.reestimated"),
  "card.refreshed": (t) => t("board.event.refreshed"),
  "auto.started": (t) => t("feed.type.started"),
  "auto.bound": (t) => t("feed.type.bound"),
  "auto.held": (t) => t("feed.type.held"),
  "auto.landed": (t) => t("feed.type.landed"),
  "gate.observed": (t) => t("feed.type.observed"),
};

/** A type's label and whether it is the stored type itself (no table knows it: shown raw, tagged English). */
export function typeLabel(type: string, t: Translate): { label: string; raw: boolean } {
  const label = Object.hasOwn(TYPE_LABELS, type) ? TYPE_LABELS[type] : undefined;
  return label === undefined ? { label: type, raw: true } : { label: label(t), raw: false };
}

/**
 * Who acted besides a person, `by` as served (docs/features/auto-feed.md "Data"): a closed table, its
 * one word the auto rule's; a word no table knows is said raw. Its key is spelled once, here, as the
 * served word, never compared in a string.
 */
const BY_LABELS: Readonly<Record<string, (t: Translate) => string>> = {
  rule: (t) => t("feed.byRule"),
};

/** The By badge's words: By rule for the rule, the served word raw for another; null when a person acted. */
export function byLabel(by: string | null, t: Translate): { label: string; raw: boolean } | null {
  if (by === null) {
    return null;
  }
  const label = Object.hasOwn(BY_LABELS, by) ? BY_LABELS[by] : undefined;
  return label === undefined ? { label: by, raw: true } : { label: label(t), raw: false };
}

/** Where a subject links: an agent task to its Tasks detail, a proposal to its view, a task of the Board to its modal; a line is text. */
export type Subject = { to: "tasks" | "inbox" | "board"; id: string } | { to: "line"; id: string } | null;

/** The entry's subject: the agent task, else the proposal, else the Board's task, else the line (docs/features/auto-feed.md "Description and interactions"). */
export function subjectOf(entry: Pick<FeedEntry, "task" | "proposal" | "card" | "line">): Subject {
  if (entry.task !== null) {
    return { to: "tasks", id: entry.task };
  }
  if (entry.proposal !== null) {
    return { to: "inbox", id: entry.proposal };
  }
  if (entry.card !== null) {
    return { to: "board", id: entry.card };
  }
  return entry.line === null ? null : { to: "line", id: entry.line };
}

/** A commit's first twelve hex digits, as the CLI's words cut it; the whole is the element's title. */
export function shortCommit(commit: string): string {
  return commit.slice(0, 12);
}

/** A rule hash's twelve hex digits after `b3:`, as the auto head and Applied by rule show it. */
export function shortRule(rule: string): string {
  return rule.startsWith("b3:") ? `b3:${rule.slice(3, 15)}` : rule.slice(0, 15);
}

/** The time between two entries the list marks as a quiet stretch: two hours or more with nothing (a usage limit's pause shows so). */
export const QUIET_MS = 7_200_000;

/** A quiet stretch's length in whole hours and minutes; null when shorter than QUIET_MS, or a time is none or does not parse. */
export function quietBetween(earlier: string | null, later: string | null): { hours: number; minutes: number } | null {
  if (earlier === null || later === null) {
    return null;
  }
  const span = Date.parse(later) - Date.parse(earlier);
  if (Number.isNaN(span) || span < QUIET_MS) {
    return null;
  }
  const minutes = Math.floor(span / 60_000);
  return { hours: Math.floor(minutes / 60), minutes: minutes % 60 };
}

/** A served code's words: the catalog's label for a known one, else the code itself, said raw. */
export interface CodeLabel {
  label: string;
  raw: boolean;
}

function labelIn(table: Readonly<Record<string, (t: Translate) => string>>, code: string, t: Translate): CodeLabel {
  const label = Object.hasOwn(table, code) ? table[code] : undefined;
  return label === undefined ? { label: code, raw: true } : { label: label(t), raw: false };
}

/** Why auto mode held an item: the closed set of `docs/canon/auto-mode.md` "Judging an item", the land's and the runner's reasons too. */
const REASONS: Readonly<Record<string, (t: Translate) => string>> = {
  off: (t) => t("feed.reason.off"),
  team: (t) => t("feed.reason.team"),
  stopped: (t) => t("feed.reason.stopped"),
  "not-in-pack": (t) => t("feed.reason.not_in_pack"),
  author: (t) => t("feed.reason.author"),
  until: (t) => t("feed.reason.until"),
  status: (t) => t("feed.reason.status"),
  "run-outcome": (t) => t("feed.reason.run_outcome"),
  "proposal-kind": (t) => t("feed.reason.proposal_kind"),
  staged: (t) => t("feed.reason.staged"),
  "config-target": (t) => t("feed.reason.config_target"),
  "excluded-zone": (t) => t("feed.reason.excluded_zone"),
  "excluded-kind": (t) => t("feed.reason.excluded_kind"),
  uncovered: (t) => t("feed.reason.uncovered"),
  findings: (t) => t("feed.reason.findings"),
  rebases: (t) => t("feed.reason.rebases"),
  cap: (t) => t("feed.reason.cap"),
  place: (t) => t("feed.reason.place"),
  "merge-conflict": (t) => t("feed.reason.merge_conflict"),
  check: (t) => t("feed.reason.check"),
  "branch-checked-out": (t) => t("feed.reason.branch_checked_out"),
  "branch-moved": (t) => t("feed.reason.branch_moved"),
  "agent-error": (t) => t("feed.reason.agent_error"),
  "denied-tool": (t) => t("feed.reason.denied_tool"),
  "no-mcp": (t) => t("feed.reason.no_mcp"),
  "max-turns": (t) => t("feed.reason.max_turns"),
  "max-budget": (t) => t("feed.reason.max_budget"),
  "not-planned": (t) => t("feed.reason.not_planned"),
  "not-done": (t) => t("feed.reason.not_done"),
};

/** The act a hold stopped (`auto.held`'s `act`). */
const ACTS: Readonly<Record<string, (t: Translate) => string>> = {
  approve: (t) => t("feed.act.approve"),
  apply: (t) => t("feed.act.apply"),
  land: (t) => t("feed.act.land"),
  run: (t) => t("feed.act.run"),
};

/** A proposal's kind: the Delivery tab's words for the three it names, the feed's for the rest of the queue's own; another raw. */
const KINDS: Readonly<Record<string, (t: Translate) => string>> = {
  question: (t) => t("roadmap.delivery.item.question"),
  discrepancy: (t) => t("roadmap.delivery.item.discrepancy"),
  decision: (t) => t("roadmap.delivery.item.decision"),
  update: (t) => t("feed.kind.update"),
  create: (t) => t("feed.kind.create"),
  interpretation: (t) => t("feed.kind.interpretation"),
  amendment: (t) => t("feed.kind.amendment"),
  drift: (t) => t("feed.kind.drift"),
};

/** A hold's reason in the viewer's words; an unknown code raw; none (a payload without it) null. */
export function reasonLabel(code: string | null, t: Translate): CodeLabel | null {
  return code === null ? null : labelIn(REASONS, code, t);
}

/** A held act in the viewer's words; an unknown one raw; none null. */
export function actLabel(code: string | null, t: Translate): CodeLabel | null {
  return code === null ? null : labelIn(ACTS, code, t);
}

/** An open proposal's kind in the viewer's words; one the queue adds later raw; none null. */
export function proposalKindLabel({ kind }: { kind: string | null }, t: Translate): CodeLabel | null {
  return kind === null ? null : labelIn(KINDS, kind, t);
}
