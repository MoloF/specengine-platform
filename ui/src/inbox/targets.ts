import type { InboxEntry, Proposal } from "../api/types";

/** The nodes a proposal targets: `target_ids` when the queue names them, else its one `target_id`. */
export function targetsOf(proposal: Pick<InboxEntry | Proposal, "target_id" | "target_ids">): string[] {
  if (proposal.target_ids.length > 0) {
    return proposal.target_ids;
  }
  return proposal.target_id === null ? [] : [proposal.target_id];
}

/** Whether two different proposals share a target node (information only: nothing waits on it). */
export function sharesTarget(a: InboxEntry, b: InboxEntry): boolean {
  if (a.id === b.id) {
    return false;
  }
  const theirs = targetsOf(b);
  return targetsOf(a).some((id) => theirs.includes(id));
}
