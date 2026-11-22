import type { ReactNode } from "react";
import type { CommentDocument, CommentItem } from "../api/types";
import { sectionHash } from "../app/routes";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { byAgent, statusLozenge } from "../inbox/labels";
import { Prose } from "../markdown/Prose";
import { Lozenge } from "../ui/Lozenge";
import { shownSlot } from "../ui/text";
import { formatUtc } from "../ui/time";

// A comment as the thread and the Mentions view draw it (docs/features/comments.md "UI"): its head
// (who, an agent's mark and role, when, an edit's time and version, a node's revision, a reply's
// comment when its own is not drawn above it, its ID), then its body rendered only through
// src/markdown/ as people's words (never an anchor the daemon did not resolve, raw HTML as text), or
// its tombstone; then the items raised from it, each an Inbox link with its state as served.

/** A stored UTC time, as stored to the minute. */
export function At({ at }: { at: string }) {
  const t = useTranslate();
  return <time dateTime={at}>{formatUtc(at, t)}</time>;
}

/** The head line: `headingId` names the comment's article; `replyTo` is said when the comment it answers is not drawn above it. */
export function CommentHead({ comment, headingId, replyTo = false, children }: { comment: CommentDocument; headingId: string; replyTo?: boolean; children?: ReactNode }) {
  const t = useTranslate();
  const { author } = comment;
  return (
    <p className="comment-head">
      <bdi id={headingId} className="comment-who">
        {shownSlot(comment.actor)}
      </bdi>
      {byAgent(author) && (
        <>
          <span className="plain-tag">{t("comments.agent")}</span>
          {author.role !== null && (
            <span className="kind-tag" lang="en">
              {shownSlot(author.role)}
            </span>
          )}
        </>
      )}
      <span className="muted">
        <At at={comment.created_at} />
      </span>
      {comment.version > 1 && <span className="muted">{fill(t("comments.edited", { at: slot("at"), version: comment.version }), { at: <At at={comment.updated_at} /> })}</span>}
      {comment.on_rev !== null && <span className="muted">{t("comments.onRev", { rev: comment.on_rev })}</span>}
      {replyTo && comment.reply_to !== null && <span className="muted">{t("comments.replyTo", { id: comment.reply_to })}</span>}
      {children}
      <span className="mono muted comment-id">{shownSlot(comment.id)}</span>
    </p>
  );
}

/** The body as people wrote it, rendered as markdown at `level` (its headings one below), or the tombstone. */
export function CommentText({ project, comment, level }: { project: string; comment: CommentDocument; level: 3 | 4 }) {
  const t = useTranslate();
  if (comment.deleted !== null) {
    return (
      <p className="comment-deleted muted">
        {fill(t("comments.deleted", { by: slot("by"), at: slot("at") }), { by: <bdi>{shownSlot(comment.deleted.by)}</bdi>, at: <At at={comment.deleted.at} /> })}
      </p>
    );
  }
  return (
    <div className="comment-text" dir="auto">
      <Prose source={comment.body ?? ""} links={null} project={project} baseLevel={level} frontMatter={false} />
    </div>
  );
}

/** The items raised from it, by ID number as stored: each an Inbox link, its kind raw, its state looked up. */
export function CommentItems({ project, items }: { project: string; items: readonly CommentItem[] }) {
  const t = useTranslate();
  if (items.length === 0) {
    return null;
  }
  return (
    <ul className="comment-items" aria-label={t("comments.items")}>
      {items.map((item) => (
        <li key={item.id}>
          <a className="mono" href={sectionHash(project, "inbox", item.id)}>
            {shownSlot(item.id)}
          </a>
          <span className="kind-tag" lang="en">
            {shownSlot(item.kind)}
          </span>
          <Lozenge {...statusLozenge(item.status, t)} />
        </li>
      ))}
    </ul>
  );
}
