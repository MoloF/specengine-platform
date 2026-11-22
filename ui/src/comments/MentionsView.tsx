import { useEffect, useId, useState, type MouseEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useMentions } from "../api/commentQueries";
import type { CommentDocument, CommentOn } from "../api/types";
import { DocLink } from "../doc/DocLink";
import { useTranslate } from "../i18n";
// The comments' English, merged as this chunk lands (src/i18n/enComments.ts).
import "../i18n/enComments";
import { fill, slot } from "../i18n/fill";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { Lozenge } from "../ui/Lozenge";
import { ErrorPanel, Skeleton } from "../ui/states";
import { shownSlot } from "../ui/text";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useMentionsWho } from "./acting";
import { CommentHead, CommentItems, CommentText } from "./CommentView";
import { placeLink, seenWord } from "./labels";
import { mentionsSeenKey, rememberSeen, seenOf, useSeenText } from "./seen";
import { wantComment } from "./wanted";

// Mentions, `#/<p>/mentions` (docs/features/comments.md "UI", Mentions): the comments naming this
// page's login (a hub's; locally the acting name when it is one, else every mention), newest first,
// apart from the Inbox: talk, never a decision. Each row links to what it is under, where the thread
// brings it into view; Load older pages back. The rows of the first page are kept seen in this browser
// as they are shown, which the nav's badge counts against; those unseen when the view opened are
// marked New for its life. The server keeps no reader state.

/** A plain click: the page follows it here, so the thread it lands on is asked to bring the comment into view. */
function plain(event: MouseEvent): boolean {
  return event.button === 0 && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey;
}

/** What a mention is under, as a link: a task's card on the Board, a proposal in the Inbox, a document in its modal; a place this build does not know, its ID raw (labels.ts `placeLink`). */
function Target({ project, on, id }: { project: string; on: CommentOn; id: string }) {
  const want = (event: MouseEvent) => {
    if (plain(event)) {
      wantComment(project, on.id, id);
    }
  };
  const shown = <bdi className="mono">{shownSlot(on.id)}</bdi>;
  const link = placeLink(project, on);
  if (link === null) {
    return shown;
  }
  return "href" in link ? (
    <a href={link.href} onClick={want}>
      {shown}
    </a>
  ) : (
    <span onClickCapture={want}>
      <DocLink project={project} docRef={link.doc}>
        {shown}
      </DocLink>
    </span>
  );
}

/** One mention: the comment's head, New when unseen at opening, what it is under, its body and its items. */
function Row({ project, comment, fresh }: { project: string; comment: CommentDocument; fresh: boolean }) {
  const t = useTranslate();
  const headingId = useId();
  return (
    <li className="mention" data-new={fresh ? "true" : undefined}>
      <article className="comment" aria-labelledby={headingId}>
        <CommentHead comment={comment} headingId={headingId} replyTo>
          {fresh && <Lozenge appearance="discovery" icon="info" label={t("feed.new")} />}
        </CommentHead>
        <p className="mention-on">{fill(t("comments.mentions.on", { target: slot("target") }), { target: <Target project={project} on={comment.on} id={comment.id} /> })}</p>
        <CommentText project={project} comment={comment} level={3} />
        <CommentItems project={project} items={comment.items} />
      </article>
    </li>
  );
}

export function MentionsView({ project }: { project: string }) {
  const t = useTranslate();
  const titleId = useId();
  const who = useMentionsWho();
  const read = useMentions(project, who.asked, who.known);
  const failure = useRetainedFailure(read.error, read.isFetching);
  const seenText = useSeenText(project, who.seen);
  const first = read.data?.pages[0];
  const key = mentionsSeenKey(project, who.seen);
  // What was seen when the rows were first drawn for this login: their New marks, held for the view's life.
  const [opened, setOpened] = useState<{ key: string; seen: ReadonlySet<string> } | null>(null);
  if (first !== undefined && opened?.key !== key) {
    setOpened({ key, seen: seenOf(seenText) });
  }

  // The first page's rows, as drawn, are kept seen: the badge counts against them.
  useEffect(() => {
    if (first !== undefined) {
      rememberSeen(project, who.seen, first.comments.map(seenWord));
    }
  }, [first, project, who.seen]);

  const rows = read.data?.pages.flatMap((page) => page.comments) ?? [];
  let body;
  if (read.data === undefined) {
    body =
      failure === null ? (
        <div aria-busy="true">
          <Skeleton label={t("app.lazy.mentions")} lines={4} />
        </div>
      ) : (
        <ErrorPanel
          title={t("comments.mentions.failed")}
          message={apiErrorOf(failure).message}
          retrying={read.isFetching}
          attempt={read.errorUpdateCount}
          onRetry={() => {
            void read.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else if (rows.length === 0) {
    body = <EmptyState level={2} title={t("comments.mentions.none")} description={<p>{t("comments.mentions.noneNext")}</p>} />;
  } else {
    body = (
      <>
        <ol className="mentions-list">
          {rows.map((comment) => (
            <Row key={comment.id} project={project} comment={comment} fresh={opened !== null && opened.key === key && !opened.seen.has(seenWord(comment))} />
          ))}
        </ol>
        {read.hasNextPage && (
          <Button
            appearance="subtle"
            aria-disabled={read.isFetchingNextPage}
            onClick={() => {
              if (!read.isFetchingNextPage) {
                void read.fetchNextPage();
              }
            }}
          >
            {read.isFetchingNextPage ? t("comments.loadingOlder") : t("comments.older")}
          </Button>
        )}
      </>
    );
  }

  return (
    <section className="view mentions-view" aria-labelledby={titleId}>
      <header className="view-head">
        <h1 id={titleId} tabIndex={-1}>
          {t("app.mentions")}
        </h1>
        <p className="view-about">{who.seen === null ? t("comments.mentions.aboutEvery") : fill(t("comments.mentions.about", { login: slot("login") }), { login: <bdi className="mono">{shownSlot(who.seen)}</bdi> })}</p>
      </header>
      {read.data !== undefined && read.error !== null && (
        <p className="note" role="alert">
          <span lang="en">{apiErrorOf(read.error).message}</span>
        </p>
      )}
      {body}
    </section>
  );
}
