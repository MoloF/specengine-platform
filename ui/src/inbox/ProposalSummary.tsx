import { useTranslate } from "../i18n";
import { CitedText, type CitedLinks } from "../markdown/CitedText";
import { leadingGist, summaryOf, type Summarised } from "./summary";

/**
 * The line a proposal is listed by, the one component every surface draws it with, so none can drift
 * (docs/features/release-hardening.md "Acceptance criteria", RH-U4): a held gist as the lead draws it,
 * agent text verbatim in a `<bdi>` through shownSlot; else the summary's line as before. The Inbox's
 * row and card title, the card's others, the Spec tree's Proposals tab, Health's queue and the palette.
 * Given the review's served citations (the Inbox card's title alone), their strings are document
 * links (docs/features/department-practices.md "Acceptance criteria", DP21); without, the line is exactly as before.
 */
export function ProposalSummary({ proposal, links = null }: { proposal: Summarised; links?: CitedLinks | null }) {
  const t = useTranslate();
  const gist = leadingGist(proposal);
  return gist === null ? (
    <CitedText text={summaryOf(proposal, t)} links={links} raw />
  ) : (
    <bdi>
      <CitedText text={gist} links={links} slot />
    </bdi>
  );
}
