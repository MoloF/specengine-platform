import type { Proposal, RevisionChange } from "../api/types";

// What approval does to the revisions of the nodes a proposal changes, as the review document's
// `revisions` says it (docs/features/rev-discipline.md "Review document, stage, queue", "UI"). The
// numbers are the daemon's: `to` is shown as sent, never worked out from `rev`; a raise the daemon
// refused shows as refused, with its reason, never as a number.

/**
 * The nodes whose meaning the change alters, when there are any: only then does Accept offer
 * "Editorial: keep the revision", a refused raise among them too. Null (the kind writes no existing
 * node, the preview conflicts or is unavailable) and `[]` (no meaning changes) offer nothing.
 * Decided by the key, never the kind.
 */
export function revisionsToKeep(review: Pick<Proposal, "revisions">): readonly RevisionChange[] | null {
  return review.revisions !== null && review.revisions.length > 0 ? review.revisions : null;
}

/** A meaning approve cannot raise this node: the daemon refused the raise (apply's step 6), or sent no `to`. */
export function isRefused(entry: RevisionChange): boolean {
  return entry.refused !== null || entry.to === null;
}

/** The lock entries an editorial approval refreshes for one node: a count, or unknown when the lock is unreadable. */
export function lockEntriesText(count: number | null): string {
  if (count === null) {
    return "lock entries unknown";
  }
  return count === 1 ? "1 lock entry" : `${String(count)} lock entries`;
}

/**
 * One node's line as `spec review` writes it (`R-12 3 -> 4 (2 lock entries)`, a refused raise
 * `ADR-0007 1 -> refused (0 lock entries)`), `rev` named and the arrow drawn (U+2192).
 */
export function revisionText(entry: RevisionChange): string {
  const to = isRefused(entry) ? "refused" : String(entry.to);
  return `${entry.id} rev ${String(entry.rev)} → ${to} (${lockEntriesText(entry.lock_entries)})`;
}
