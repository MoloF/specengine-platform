import type { InboxEntry, Proposal } from "../api/types";
import type { Translate } from "../i18n";

type Summarised = Pick<InboxEntry | Proposal, "gist" | "summary" | "rationale" | "kind" | "target_id">;

/**
 * The agent's line a proposal is listed by: a question's gist, its one line of plain words
 * (docs/features/plain-questions.md "Acceptance criteria", PQ12), else its summary, else its
 * rationale's first line; null when it wrote none. An inbox entry's summary and rationale are already
 * their first line, cut at 80 characters; a gist is whole, one line already.
 */
export function writtenSummary(proposal: Summarised): string | null {
  const rationale = proposal.rationale?.split("\n")[0]?.trim();
  const gist = leadingGist(proposal);
  if (gist !== null) {
    return gist;
  }
  if (proposal.summary !== null && proposal.summary.trim() !== "") {
    return proposal.summary;
  }
  if (rationale !== undefined && rationale !== "") {
    return rationale;
  }
  return null;
}

/**
 * The gist a proposal is listed by when it leads (held and not blank: writtenSummary's first pick),
 * else null. A surface drawing it shows it as the lead does, verbatim in a `<bdi>` through shownSlot
 * (docs/features/plain-questions.md "The UI"); the summary path stays as it was.
 */
export function leadingGist(proposal: Pick<InboxEntry | Proposal, "gist">): string | null {
  return proposal.gist !== null && proposal.gist.trim() !== "" ? proposal.gist : null;
}

/** The line a proposal is listed by: the agent's line, else its kind and target in the words of the language shown. */
export function summaryOf(proposal: Summarised, t: Translate): string {
  return (
    writtenSummary(proposal) ??
    t("inbox.summaryFallback", { kind: proposal.kind ?? t("inbox.summaryProposal"), target: proposal.target_id ?? t("inbox.noTarget") })
  );
}
