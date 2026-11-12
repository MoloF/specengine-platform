import type { ReactNode } from "react";
import type { Author } from "../api/types";
import { useTranslate } from "../i18n";
import { Disclosure } from "../ui/Disclosure";
import { Icon } from "../ui/Icon";
import { shownSlot, shownText } from "../ui/text";
import { recommendedBy } from "./labels";

// A question's lead (docs/features/plain-questions.md "Acceptance criteria", PQ12): for an
// item holding a `gist`, the gist on one line, then why it matters when held, then the item's options or
// controls, which its surface draws; everything else it showed (its text, working answer, price of
// another answer, why) folded below, collapsed at first. Agent text is data: the gist and any one-line
// slot through shownSlot, a text of several lines through shownText, each in its own `<bdi>`, never
// rewritten or cut. An item without a gist uses none of this and renders as it did. Who made a
// recommendation is labels.ts's to say: nothing here reads an author.

/** The gist on one line, then "Why it matters" with its text when held. */
export function ItemLead({ gist, matters }: { gist: string; matters: string | null }) {
  return (
    <>
      <p className="lead-gist">
        <bdi>{shownSlot(gist)}</bdi>
      </p>
      {matters !== null && <LeadMatters matters={matters} />}
    </>
  );
}

/** "Why it matters" and its text, wrapped, each line in its own direction. */
export function LeadMatters({ matters }: { matters: string }) {
  const t = useTranslate();
  return (
    <div className="lead-matters">
      <p className="lead-label">{t("inbox.lead.matters")}</p>
      <p className="lead-text">
        <bdi>{shownText(matters)}</bdi>
      </p>
    </div>
  );
}

/** What the item showed before it had a gist, folded under "The question as asked", collapsed at first. */
export function AskedFold({ children }: { children: ReactNode }) {
  const t = useTranslate();
  return (
    <Disclosure className="lead-asked" summary={t("inbox.lead.asked")}>
      {children}
    </Disclosure>
  );
}

/** The item's own text, as its author wrote it: verbatim, several lines kept. */
export function AskedText({ text }: { text: string }) {
  return (
    <p className="lead-text">
      <bdi>{shownText(text)}</bdi>
    </p>
  );
}

/** The mark on the recommended option: who made it, in labels.ts's words; it marks, never picks. */
export function RecommendedMark({ author }: { author: Pick<Author, "type"> | null }) {
  const t = useTranslate();
  return (
    <span className="recommended-mark">
      <Icon name="recommended" />
      {recommendedBy(author, t)}
    </span>
  );
}
