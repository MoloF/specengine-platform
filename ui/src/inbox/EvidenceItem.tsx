import type { Evidence } from "../api/types";
import { useTranslate } from "../i18n";
import { shownSlot, shownText } from "../ui/text";

/**
 * One evidence entry as an agent sent it: its file with the lines, the code path when given, then
 * what the code does against what the spec says, each quote verbatim in its own direction. The
 * Inbox's flat list and both sides of a collision pair draw it (docs/features/collision-pairs.md
 * "Acceptance criteria", PP9); the caller keys it. Every slot is agent text, so its controls and
 * bidi characters are written `\u{..}` as the CLI prints them (`docs/canon/round-ui.md` "Screen"):
 * the file, lines and code path as one-line slots, the two quotes keeping their line feeds.
 */
export function EvidenceItem({ item }: { item: Evidence }) {
  const t = useTranslate();
  return (
    <li className="evidence">
      <p className="mono">
        <bdi>{shownSlot(item.file)}</bdi>
        {item.lines !== null && (
          <>
            :<bdi>{shownSlot(item.lines)}</bdi>
          </>
        )}
      </p>
      {item.qpath !== null && (
        <p className="mono muted">
          <bdi>{shownSlot(item.qpath)}</bdi>
        </p>
      )}
      <dl className="pairs">
        <dt>{t("inbox.card.observed")}</dt>
        <dd>
          <bdi>{shownText(item.observed)}</bdi>
        </dd>
        <dt>{t("inbox.card.documented")}</dt>
        <dd>
          <bdi>{shownText(item.documented)}</bdi>
        </dd>
      </dl>
    </li>
  );
}
