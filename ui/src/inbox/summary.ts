import type { Proposal } from "../api/types";

/** The line a proposal is listed by: its summary, else its rationale's first line, else kind and target. */
export function summaryOf(proposal: Proposal): string {
  const rationale = proposal.rationale?.split("\n")[0]?.trim();
  if (proposal.summary !== null && proposal.summary.trim() !== "") {
    return proposal.summary;
  }
  if (rationale !== undefined && rationale !== "") {
    return rationale;
  }
  return `${proposal.kind} on ${proposal.target_id ?? "no target"}`;
}
