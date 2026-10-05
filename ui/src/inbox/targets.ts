import type { Proposal } from "../api/types";

/** The nodes a proposal targets: `target_ids` when the queue fields name them, else its one `target_id`. */
export function targetsOf(proposal: Proposal): string[] {
  if (proposal.target_ids.length > 0) {
    return proposal.target_ids;
  }
  return proposal.target_id === null ? [] : [proposal.target_id];
}

/** Whether two different proposals share a target node (information only: nothing waits on it). */
export function sharesTarget(a: Proposal, b: Proposal): boolean {
  if (a.id === b.id) {
    return false;
  }
  const theirs = targetsOf(b);
  return targetsOf(a).some((id) => theirs.includes(id));
}
