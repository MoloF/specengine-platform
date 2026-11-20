import type { ReactNode } from "react";
import type { Author } from "../api/types";
import { useTranslate } from "../i18n";
import { CitedText, type CitedLinks } from "../markdown/CitedText";
import { Disclosure } from "../ui/Disclosure";
import { Icon } from "../ui/Icon";
import { recommendedBy } from "./labels";

// A question's lead (docs/features/plain-questions.md "Acceptance criteria", PQ12): for an
// item holding a `gist`, the gist on one line, then why it matters when held, then the item's options or
// controls, which its surface draws; everything else it showed (its text, working answer, price of
// another answer, why) folded below, collapsed at first. Agent text is data: the gist and any one-line
// slot through shownSlot, a text of several lines through shownText, each in its own `<bdi>`, never
// rewritten or cut. An item without a gist uses none of this and renders as it did. Who made a
// recommendation is labels.ts's to say: nothing here reads an author. Given the item's served
// citations (`links`), their strings are document links (docs/features/department-practices.md "Acceptance criteria", DP21).

/** No served citation: the text exactly as before. */
type Links = { links?: CitedLinks | null };

/** The gist on one line, then "Why it matters" with its text when held. */
export function ItemLead({ gist, matters, links = null }: { gist: string; matters: string | null } & Links) {
  return (
    <>
      <p className="lead-gist">
        <bdi>
          <CitedText text={gist} links={links} slot />
        </bdi>
      </p>
      {matters !== null && <LeadMatters matters={matters} links={links} />}
    </>
  );
}

/** "Why it matters" and its text, wrapped, each line in its own direction. */
export function LeadMatters({ matters, links = null }: { matters: string } & Links) {
  const t = useTranslate();
  return (
    <div className="lead-matters">
      <p className="lead-label">{t("inbox.lead.matters")}</p>
      <p className="lead-text">
        <bdi>
          <CitedText text={matters} links={links} />
        </bdi>
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
export function AskedText({ text, links = null }: { text: string } & Links) {
  return (
    <p className="lead-text">
      <bdi>
        <CitedText text={text} links={links} />
      </bdi>
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
