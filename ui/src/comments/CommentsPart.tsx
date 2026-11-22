import { useEffect, useId, useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import { useComments, useCommentWrite, useHeldComment, type CommentsQuery } from "../api/commentQueries";
import type { CommentDocument, Mandate } from "../api/types";
import { useAllowed } from "../board/acting";
import { ActOff } from "../board/ActOff";
import { useTranslate } from "../i18n";
// The comments' English, merged as this chunk lands (src/i18n/enComments.ts).
import "../i18n/enComments";
import { useAnnounce } from "../ui/announcer";
import { Button } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useCommentActing } from "./acting";
import { CommentHead, CommentItems, CommentText } from "./CommentView";
import { Composer, type Sent } from "./Composer";
import { DeleteDialog } from "./DeleteDialog";
import { actorOf, canEdit, deleteStands, takesReply, type Acting, type Turn } from "./labels";
import type { CommentsProps } from "./lazy";
import { TurnDialog, TurnMenu } from "./TurnInto";
import { forgetWanted, wantedIn } from "./wanted";

// The Comments part (docs/features/comments.md "UI"; docs/canon/comments.md "Screen"), a chunk of its
// own under a card (modal and page), a document (modal and page) and a proposal (open or closed):
// talk, never truth, and no hold on anything (ADR-0012). The thread oldest first, each reply under
// its comment, Load older paging back by `next_before`; a composer at its foot. Each live comment
// offers Reply (top-level), Edit (one's own), Delete (one's own, any where `allowed` lists
// `delete_comment`, any locally; else off with its line) and Turn into…; a deleted one shows who
// deleted it and offers nothing. Every write is one call; the thread shown is the daemon's, read again
// after each write and on a `comment.*` event. Who acts is the hub's login or the acting name as typed.

/** A refusal by a rule (docs/features/comments.md "HTTP"): a stale version, a reply or alias rule, a deleted comment. */
const REFUSED = 409;

/** The part's heading at its level: the card's and a proposal's `h3`, a document's `h2` on its page. */
function PartHeading({ level, id, children }: { level: 2 | 3; id: string; children: string }) {
  return level === 2 ? (
    <h2 id={id} className="issue-section-title">
      {children}
    </h2>
  ) : (
    <h3 id={id} className="issue-section-title">
      {children}
    </h3>
  );
}

/** The thread's pages as one list, oldest first: the newest page is read first, each older one after it. */
export function threadOf(query: Pick<CommentsQuery, "data">): CommentDocument[] {
  return [...(query.data?.pages ?? [])].reverse().flatMap((page) => page.comments);
}

/** A write's refusal as the composer shows it: refused, in the daemon's words (a stale edit is the edit's own to tell). */
function sentOf(error: unknown): Sent {
  return { refusal: apiErrorOf(error).message, stale: false };
}

/** What every entry of a thread shares. */
interface Thread {
  project: string;
  on: string;
  acting: Acting;
  deletesAny: boolean;
  targets: readonly string[];
  mandate: Mandate | null;
  level: 2 | 3;
}

export function CommentsPart({ project, on, targets, level, mandate = null }: CommentsProps) {
  const t = useTranslate();
  const headingId = useId();
  const section = useRef<HTMLElement>(null);
  const read = useComments(project, on);
  const failure = useRetainedFailure(read.error, read.isFetching);
  const write = useCommentWrite(project);
  const announce = useAnnounce();
  const acting = useCommentActing();
  const allows = useAllowed(project);
  const thread: Thread = { project, on, acting, deletesAny: allows("delete_comment"), targets, mandate, level };
  const all = threadOf(read);
  const first = read.data?.pages[0];
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = read;

  // A Mentions row followed here: its comment brought into view and focused once drawn, older pages
  // read until it is or none is left (docs/features/comments.md "UI", Mentions).
  const wanted = wantedIn(project, on);
  const found = wanted !== null && all.some((comment) => comment.id === wanted);
  useEffect(() => {
    if (wanted === null || read.data === undefined) {
      return;
    }
    const entry = Array.from(section.current?.querySelectorAll<HTMLElement>("[data-comment]") ?? []).find((element) => element.dataset.comment === wanted) ?? null;
    if (entry !== null) {
      forgetWanted();
      entry.scrollIntoView({ block: "center" });
      entry.focus();
    } else if (!found && hasNextPage && !isFetchingNextPage) {
      void fetchNextPage();
    } else if (!found && !hasNextPage) {
      forgetWanted();
    }
  }, [wanted, found, read.data, hasNextPage, isFetchingNextPage, fetchNextPage]);

  async function add(body: string): Promise<Sent> {
    try {
      const made = await write.mutateAsync({ write: "add", body: { on, body, reply_to: null, as: actorName(acting) } });
      announce(t("comments.said.added", { id: made.id }));
      return null;
    } catch (error) {
      return sentOf(error);
    }
  }

  let body;
  if (read.data === undefined) {
    body =
      failure === null ? (
        <div aria-busy="true">
          <Skeleton label={t("app.lazy.comments")} lines={2} />
        </div>
      ) : (
        <ErrorPanel
          title={t("comments.failed")}
          message={apiErrorOf(failure).message}
          retrying={read.isFetching}
          attempt={read.errorUpdateCount}
          onRetry={() => {
            void read.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else {
    // Each comment's replies, grouped once; a reply whose comment is on a page not read stands at its own place, naming it.
    const replies = new Map<string, CommentDocument[]>();
    const ids = new Set(all.map((comment) => comment.id));
    for (const comment of all) {
      if (comment.reply_to !== null && ids.has(comment.reply_to)) {
        replies.set(comment.reply_to, [...(replies.get(comment.reply_to) ?? []), comment]);
      }
    }
    const tops = all.filter((comment) => comment.reply_to === null || !ids.has(comment.reply_to));
    body = (
      <>
        {first !== undefined && first.notes.length > 0 && (
          <ul className="note-list" aria-label={t("comments.notes")}>
            {first.notes.map((note, index) => (
              <li key={`${String(index)}-${note}`} lang="en">
                {note}
              </li>
            ))}
          </ul>
        )}
        {read.error !== null && (
          <p className="note" role="alert">
            <Icon name="alert" />
            <span lang="en">{apiErrorOf(read.error).message}</span>
          </p>
        )}
        {hasNextPage && (
          <Button
            appearance="subtle"
            aria-disabled={isFetchingNextPage}
            onClick={() => {
              if (!isFetchingNextPage) {
                void fetchNextPage();
              }
            }}
          >
            {isFetchingNextPage ? t("comments.loadingOlder") : t("comments.older")}
          </Button>
        )}
        {all.length === 0 ? (
          <p className="muted">{t("comments.none")}</p>
        ) : (
          <ol className="comments-thread">
            {tops.map((comment) => (
              <li key={comment.id}>
                <Entry thread={thread} comment={comment} replies={replies.get(comment.id) ?? []} orphan={comment.reply_to !== null} />
              </li>
            ))}
          </ol>
        )}
      </>
    );
  }

  return (
    <section ref={section} className="comments" aria-label={t("app.comments")} data-on={on}>
      <PartHeading level={level} id={headingId}>
        {first === undefined ? t("app.comments") : t("comments.titleCount", { count: first.total })}
      </PartHeading>
      <p className="comments-about muted">{t("comments.about")}</p>
      {body}
      <Composer project={project} words={{ label: t("comments.new"), send: t("comments.send"), sending: t("comments.sending") }} as={actorOf(acting)} onSend={add} />
    </section>
  );
}

/** The name a write sends as `as`: a hub's login (the acting name there), else as typed; none (null) when none is typed, which the daemon records `local`. */
function actorName({ name }: Acting): string | null {
  const typed = name.trim();
  return typed === "" ? null : typed;
}

/** What an entry has open beside it. */
type Open = { what: "reply" | "edit" } | { what: "delete"; trigger: HTMLElement } | { what: "menu"; anchor: DOMRect; trigger: HTMLElement } | { what: "turn"; turn: Turn; trigger: HTMLElement } | null;

/** One comment, its acts and what they open, its replies under it. */
function Entry({ thread, comment, replies, orphan }: { thread: Thread; comment: CommentDocument; replies: readonly CommentDocument[]; orphan: boolean }) {
  const { project, on, acting, deletesAny, targets, mandate, level } = thread;
  const t = useTranslate();
  const headingId = useId();
  const article = useRef<HTMLElement>(null);
  const replyButton = useRef<HTMLButtonElement>(null);
  const editButton = useRef<HTMLButtonElement>(null);
  const write = useCommentWrite(project);
  const announce = useAnnounce();
  const focusLater = useFocusLater();
  const held = useHeldComment(project, on);
  const [open, setOpen] = useState<Open>(null);
  // The version an edit goes on: the one it opened with, moved to the version stored only once a
  // stale refusal has shown it (`stored`), so a save never overwrites a version its person has not seen.
  const [base, setBase] = useState(comment.version);
  const [stored, setStored] = useState(false);
  const live = comment.deleted === null;
  // An edit stands on a live comment alone: deleted meanwhile (another's delete, a live event), it closes.
  const editing = open?.what === "edit" && live;
  const deletes = deleteStands(comment, acting, deletesAny);
  const as = actorName(acting);

  function close(back: () => HTMLElement | null) {
    setOpen(null);
    focusLater(back);
  }

  async function reply(body: string): Promise<Sent> {
    try {
      const made = await write.mutateAsync({ write: "add", body: { on: comment.on.id, body, reply_to: comment.id, as } });
      announce(t("comments.said.replied", { id: made.id }));
      close(() => replyButton.current);
      return null;
    } catch (error) {
      return sentOf(error);
    }
  }

  function startEdit() {
    setBase(comment.version);
    setStored(false);
    setOpen({ what: "edit" });
  }

  function endEdit() {
    setStored(false);
    close(() => editButton.current);
  }

  /**
   * One save, on the version the edit stands on. A refusal is read against the thread as read again
   * (the write's call settles once it is): the comment deleted meanwhile, the edit closes and the
   * daemon's words are said; still live at another version, the refusal is a stale one, the version
   * now stored shown above the draft, which the next save then goes on; any other, refused as said.
   */
  async function edit(body: string): Promise<Sent> {
    try {
      const saved = await write.mutateAsync({ write: "edit", id: comment.id, body: { body, version: base, as } });
      announce(t("comments.said.edited", { id: saved.id, version: saved.version }));
      endEdit();
      return null;
    } catch (error) {
      const { status, message } = apiErrorOf(error);
      const now = held(comment.id);
      if (status === REFUSED && now !== undefined && now.deleted !== null) {
        announce(message, "assertive");
        setStored(false);
        close(() => article.current);
        return { refusal: message, stale: false };
      }
      if (status === REFUSED && now !== undefined && now.version !== base) {
        setBase(now.version);
        setStored(true);
        return { refusal: message, stale: true };
      }
      return { refusal: message, stale: false };
    }
  }

  return (
    <article ref={article} className="comment" aria-labelledby={headingId} data-comment={comment.id} tabIndex={-1}>
      <CommentHead comment={comment} headingId={headingId} replyTo={orphan} />
      {editing ? (
        <>
          {stored && (
            // What a save would now overwrite, the version stored, to read (docs/features/comments.md "UI", Edit).
            <div className="comment-stored">
              <p className="comment-stored-title muted">{t("comments.stored", { version: comment.version })}</p>
              <CommentText project={project} comment={comment} level={level === 2 ? 3 : 4} />
            </div>
          )}
          <Composer
            project={project}
            words={{ label: t("comments.editLabel", { id: comment.id }), send: t("comments.save"), sending: t("comments.saving") }}
            as={actorOf(acting)}
            initial={comment.body ?? ""}
            focus
            onSend={edit}
            onCancel={endEdit}
          />
        </>
      ) : (
        <CommentText project={project} comment={comment} level={level === 2 ? 3 : 4} />
      )}
      <CommentItems project={project} items={comment.items} />
      {live && !editing && (
        <div className="comment-acts" role="group" aria-label={t("comments.acts", { id: comment.id })}>
          {takesReply(comment) && (
            <Button
              ref={replyButton}
              spacing="compact"
              appearance="subtle"
              aria-expanded={open?.what === "reply"}
              onClick={() => {
                setOpen(open?.what === "reply" ? null : { what: "reply" });
              }}
            >
              {t("comments.reply")}
            </Button>
          )}
          {canEdit(comment, acting) && (
            <Button
              ref={editButton}
              spacing="compact"
              appearance="subtle"
              onClick={startEdit}
            >
              {t("comments.edit")}
            </Button>
          )}
          {targets.length > 0 && (
            <Button
              spacing="compact"
              appearance="subtle"
              aria-haspopup="menu"
              onClick={(event) => {
                setOpen({ what: "menu", anchor: event.currentTarget.getBoundingClientRect(), trigger: event.currentTarget });
              }}
            >
              {t("comments.turn.action")}
            </Button>
          )}
          <Button
            spacing="compact"
            appearance="subtle"
            aria-haspopup="dialog"
            aria-disabled={!deletes}
            onClick={(event) => {
              if (deletes) {
                setOpen({ what: "delete", trigger: event.currentTarget });
              }
            }}
          >
            {t("comments.delete.action")}
          </Button>
        </div>
      )}
      {live && !deletes && <ActOff act="delete_comment" />}
      {open?.what === "reply" && (
        <Composer
          project={project}
          words={{ label: t("comments.replyLabel", { id: comment.id }), send: t("comments.reply"), sending: t("comments.sending") }}
          as={actorOf(acting)}
          focus
          onSend={reply}
          onCancel={() => {
            close(() => replyButton.current);
          }}
        />
      )}
      {replies.length > 0 && (
        <ol className="comments-replies">
          {replies.map((one) => (
            <li key={one.id}>
              <Entry thread={thread} comment={one} replies={[]} orphan={false} />
            </li>
          ))}
        </ol>
      )}
      {open?.what === "menu" && (
        <TurnMenu
          id={comment.id}
          anchor={open.anchor}
          onChoose={(turn) => {
            setOpen({ what: "turn", turn, trigger: open.trigger });
          }}
          onClose={() => {
            const { trigger } = open;
            close(() => trigger);
          }}
        />
      )}
      {open?.what === "turn" && (
        <TurnDialog
          project={project}
          comment={comment}
          turn={open.turn}
          targets={targets}
          mandate={mandate}
          onClose={() => {
            const { trigger } = open;
            close(() => (trigger.isConnected ? trigger : article.current));
          }}
          onRaised={(said) => {
            announce(said);
            const { trigger } = open;
            close(() => (trigger.isConnected ? trigger : article.current));
          }}
        />
      )}
      {open?.what === "delete" && (
        <DeleteDialog
          project={project}
          comment={comment}
          onClose={() => {
            const { trigger } = open;
            close(() => (trigger.isConnected ? trigger : article.current));
          }}
          onDeleted={() => {
            announce(t("comments.said.deleted", { id: comment.id }));
            // The acts go with the text: the tombstone's article takes the focus.
            close(() => article.current);
          }}
        />
      )}
    </article>
  );
}
