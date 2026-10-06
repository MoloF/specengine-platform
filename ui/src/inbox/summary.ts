import type { InboxEntry, Proposal } from "../api/types";

/**
 * The line a proposal is listed by: its summary, else its rationale's first line, else kind and
 * target. An inbox entry's summary and rationale are already their first line, cut at 80 characters.
 */
export function summaryOf(proposal: Pick<InboxEntry | Proposal, "summary" | "rationale" | "kind" | "target_id">): string {
  const rationale = proposal.rationale?.split("\n")[0]?.trim();
  if (proposal.summary !== null && proposal.summary.trim() !== "") {
    return proposal.summary;
  }
  if (rationale !== undefined && rationale !== "") {
    return rationale;
  }
  return `${proposal.kind ?? "proposal"} on ${proposal.target_id ?? "no target"}`;
}
