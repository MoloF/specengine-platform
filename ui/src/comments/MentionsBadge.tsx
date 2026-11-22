import { useMentionsBadge } from "../api/commentQueries";
import { useTranslate } from "../i18n";
// The comments' English, merged as this chunk lands (src/i18n/enComments.ts).
import "../i18n/enComments";
import { useMentionsWho } from "./acting";
import { seenWord } from "./labels";
import { seenOf, useSeenText } from "./seen";

// The Mentions entry's badge (docs/features/comments.md "UI", Mentions): how many rows of the first
// page the browser has not seen, each by its `id@version` (an edit is news again). Its read is that
// first page alone, under a key of its own (the view pages back under another), read again on every
// `comment.*` event; it waits for the identity read,
// which says whose mentions they are. None unseen, a read out or refused: no badge. Information, never
// a hold on anything (ADR-0012).

export function MentionsBadge({ project }: { project: string }) {
  const t = useTranslate();
  const who = useMentionsWho();
  const read = useMentionsBadge(project, who.asked, who.known);
  const seen = seenOf(useSeenText(project, who.seen));
  const first = read.data;
  const unseen = first === undefined ? 0 : first.comments.filter((comment) => !seen.has(seenWord(comment))).length;
  if (unseen === 0) {
    return null;
  }
  return (
    <span className="nav-badge" data-unseen={unseen}>
      <span aria-hidden="true">{unseen}</span>
      <span className="sr-only">{t("comments.mentions.unseen", { count: unseen })}</span>
    </span>
  );
}
