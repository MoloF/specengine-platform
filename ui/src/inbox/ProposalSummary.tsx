import { useTranslate } from "../i18n";
import { shownSlot } from "../ui/text";
import { leadingGist, summaryOf, type Summarised } from "./summary";

/**
 * The line a proposal is listed by, the one component every surface draws it with, so none can drift
 * (docs/features/release-hardening.md "Acceptance criteria", RH-U4): a held gist as the lead draws it,
 * agent text verbatim in a `<bdi>` through shownSlot; else the summary's line as before. The Inbox's
 * row and card title, the card's others, the Spec tree's Proposals tab, Health's queue and the palette.
 */
export function ProposalSummary({ proposal }: { proposal: Summarised }) {
  const t = useTranslate();
  const gist = leadingGist(proposal);
  return gist === null ? summaryOf(proposal, t) : <bdi>{shownSlot(gist)}</bdi>;
}
