import type { InboxEntry, Proposal } from "../api/types";
import type { Translate } from "../i18n";

type Summarised = Pick<InboxEntry | Proposal, "summary" | "rationale" | "kind" | "target_id">;

/**
 * The agent's line a proposal is listed by: its summary, else its rationale's first line; null when
 * it wrote neither. An inbox entry's summary and rationale are already their first line, cut at 80
 * characters.
 */
export function writtenSummary(proposal: Summarised): string | null {
  const rationale = proposal.rationale?.split("\n")[0]?.trim();
  if (proposal.summary !== null && proposal.summary.trim() !== "") {
    return proposal.summary;
  }
  if (rationale !== undefined && rationale !== "") {
    return rationale;
  }
  return null;
}

/** The line a proposal is listed by: the agent's line, else its kind and target in the words of the language shown. */
export function summaryOf(proposal: Summarised, t: Translate): string {
  return (
    writtenSummary(proposal) ??
    t("inbox.summaryFallback", { kind: proposal.kind ?? t("inbox.summaryProposal"), target: proposal.target_id ?? t("inbox.noTarget") })
  );
}
