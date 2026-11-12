import { useEffect, useId, useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import { useConfirmProposal, useConfirmRead, useProposal } from "../api/queries";
import type { ApiError, Proposal, Team } from "../api/types";
import { useTranslate, type Translate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { useAnnounce } from "../ui/announcer";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { DiffView } from "../ui/DiffView";
import { Icon } from "../ui/Icon";
import { Skeleton } from "../ui/states";
import { shownConfirmSlot, shownConfirmText } from "../ui/text";
import { useRetainedFailure } from "../ui/useRetainedFailure";

/**
 * A request with no answer, status 0: a one-factor session's `fetch` meeting the login door's
 * redirect (`docs/canon/hub-confirm.md` "Known limits"), or a connection dropped, after the commit
 * too, so it never says that nothing was confirmed (`docs/features/hub-confirm.md` "Description and interactions").
 */

/** The heading of a refusal; the daemon's own words stand under it. */
function headingOf({ status }: ApiError, t: Translate): string {
  return status === 0 ? t("inbox.confirm.noAnswer") : t("inbox.confirm.refused");
}

/**
 * A kind that applies with no diff to show (`docs/features/hub-confirm.md` "Description and interactions"):
 * nothing to consent to, so Confirm stays off. Read by the keys, never the kind: an applying kind's
 * document carries `new_text`, a deciding kind's and a drift item's never.
 */
function lacksDiff(review: Proposal): boolean {
  return review.new_text !== null && review.diff === null;
}

/**
 * What the confirm does to the staged choice, the page's words; the question says it in the terminal's.
 * True on every hub: the click pushes nothing, though a `--push` hub's sync sends the commit later
 * (`docs/features/hub-git.md`).
 */
function effectOf(review: Proposal, t: Translate): string | null {
  const stage = review.staged;
  if (stage === null) {
    return null;
  }
  return stage.decision === "reject" ? t("inbox.confirm.effectReject") : t("inbox.confirm.effectApprove");
}

/**
 * The change itself: the section diff, else the record, each written as shownConfirmText. A deciding
 * kind's record to come stands in the question. An applying kind with no diff: the review's notes
 * in its place, which say why.
 */
function ChangeShown({ review }: { review: Proposal }) {
  const t = useTranslate();
  if (review.diff !== null) {
    return <DiffView diff={shownConfirmText(review.diff)} />;
  }
  if (lacksDiff(review)) {
    return (
      <div className="notice">
        <p className="notice-title">
          <Icon name="alert" />
          <span>{t("inbox.confirm.noDiff")}</span>
        </p>
        {review.notes.length > 0 && (
          <ul className="note-list" aria-label={t("inbox.confirm.reviewNotes")}>
            {review.notes.map((note, index) => (
              <li key={`${String(index)}-${note}`} className="agent-text" dir="auto" lang="en">
                {shownConfirmText(note)}
              </li>
            ))}
          </ul>
        )}
      </div>
    );
  }
  if (review.record_text === null) {
    return null;
  }
  return (
    <figure className="diff-figure" aria-label={review.record_id === null ? t("inbox.confirm.recordBare") : t("inbox.confirm.record", { id: review.record_id })}>
      <pre className="node-text agent-text" dir="auto">
        {shownConfirmText(review.record_text)}
      </pre>
    </figure>
  );
}

/**
 * One staged choice, one Confirm (`docs/features/hub-confirm.md` "Description and interactions"): the
 * diff or record, the question a terminal would ask and the login, read when the dialog opens. Focus
 * starts on Cancel, which keeps the stage. The POST carries the read's `question` and `updated_at`
 * and nothing else: this page never sends a choice, and no control here reaches a second item.
 * Confirm stands on only while the document on screen is the version the question was read on, so a
 * person consents to what is shown; a version that moved says so. An applying kind with no diff
 * shows the review's notes in its place and keeps Confirm off.
 *
 * The question stays as read until Read again: a live event never swaps the words under a click. A
 * submit is one call; while it is out, a second click, Cancel and Esc do nothing. Any refusal (a read
 * or a POST; a 403, a 409, status 0) keeps the dialog, in the daemon's words, and leaves Confirm off
 * until Read again, so the same words are not sent twice; status 0 may follow a commit, so it says
 * to read again, never that nothing was confirmed. Everything of an agent's or the daemon's shows as
 * text, written as the CLI prints it with every format character escaped too (shownConfirmText in
 * src/ui/text.ts), never as markup.
 *
 * A 200 whose review document holds notes (an apply's gap warning, `docs/features/apply-outputs.md`;
 * a reject's follow-up) keeps the dialog open (`docs/features/hub-git.md` "Status and the screen"):
 * `Confirmed.` and the notes as text, Confirm gone, focus on Close; Close, or Esc, hands the document
 * on, once. A clean confirm's document holds none, so it is handed on at once, as before.
 */
export function ConfirmDialog({
  project,
  id,
  team,
  onClose,
  onConfirmed,
}: {
  project: string;
  id: string;
  /** This login's row of `[team]`, as read when the dialog opened: a later read cannot pull it away. */
  team: Team;
  onClose: () => void;
  /** The daemon confirmed it: its answer, the review document, handed on once. The caller closes the dialog. */
  onConfirmed: (review: Proposal) => void;
}) {
  const t = useTranslate();
  const read = useConfirmRead(project, id);
  const proposal = useProposal(project, id);
  const write = useConfirmProposal(project);
  const announce = useAnnounce();
  const failedRead = useRetainedFailure(read.error ?? proposal.error, read.isFetching || proposal.isFetching);
  const [refusal, setRefusal] = useState<ApiError | null>(null);
  const [pending, setPending] = useState(false);
  const inFlight = useRef(false);
  /** The view is on screen: a refusal after the owner left (browser Back) is only spoken. */
  const mounted = useRef(false);
  const cancelButton = useRef<HTMLButtonElement>(null);
  /** A 200 whose document holds notes: shown here until Close hands it on. */
  const [done, setDone] = useState<Proposal | null>(null);
  /** The document went to onConfirmed: never twice, whatever is pressed after. */
  const handedOn = useRef(false);
  const closeButton = useRef<HTMLButtonElement>(null);
  const doneId = useId();

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  // Confirm went with its answer: focus moves to Close, never left on the page's body.
  useEffect(() => {
    if (done !== null) {
      closeButton.current?.focus();
    }
  }, [done]);

  /** The daemon's answer to the caller, once. */
  function handOn(review: Proposal) {
    if (!handedOn.current) {
      handedOn.current = true;
      onConfirmed(review);
    }
  }

  const failure = refusal ?? (failedRead === null ? null : apiErrorOf(failedRead));
  const asked = failedRead === null ? read.data : undefined;
  const review = proposal.data?.id === id ? proposal.data : undefined;
  const sameVersion = asked !== undefined && review !== undefined && review.updated_at === asked.updated_at;
  // Never while a POST or a read is out: the POST's own commit moves the document, and its notice
  // says nothing was confirmed.
  const moved = asked !== undefined && proposal.data !== undefined && !sameVersion && !proposal.isFetching && !read.isFetching && !pending;
  const noDiff = review !== undefined && lacksDiff(review);
  const ready = sameVersion && failure === null && !noDiff && !read.isFetching && !pending;
  const stale = !pending && !read.isFetching && (failure !== null || moved || noDiff);

  function cancel() {
    if (done !== null) {
      handOn(done);
    } else if (!inFlight.current) {
      onClose();
    }
  }

  /** Reads the question and the document anew; the next click answers what then stands on screen. */
  function readAgain() {
    setRefusal(null);
    void read.refetch({ cancelRefetch: false });
    void proposal.refetch({ cancelRefetch: false });
    cancelButton.current?.focus();
  }

  /** One POST. Only the call is guarded: what follows a success can never show as a refusal. */
  async function submit() {
    if (asked === undefined || !ready || inFlight.current) {
      return;
    }
    inFlight.current = true;
    setPending(true);
    let answered: Proposal;
    try {
      answered = await write.mutateAsync({ id, body: { question: asked.question, updated_at: asked.updated_at } });
    } catch (error) {
      inFlight.current = false;
      const refused = apiErrorOf(error);
      if (mounted.current) {
        setPending(false);
        setRefusal(refused);
      } else {
        announce(t("inbox.confirm.refusedSaid", { heading: headingOf(refused, t), id, message: refused.message }), "assertive");
      }
      return;
    }
    // Notes to read: the dialog stays, Confirm gone; the POST is spent, so nothing sends it again.
    if (answered.notes.length > 0 && mounted.current) {
      setPending(false);
      setDone(answered);
      return;
    }
    handOn(answered);
  }

  if (done !== null) {
    return (
      <Dialog title={t("inbox.confirm.title", { id })} onClose={cancel} className="confirm-dialog">
        <div id={doneId} className="confirm-body" role="group" aria-label={t("inbox.confirm.did", { id })} tabIndex={0}>
          <p className="dialog-text confirm-done">
            {/* A check, no state's icon: a staged reject is confirmed too. */}
            <Icon name="selected" />
            <span>{t("inbox.confirm.done")}</span>
          </p>
          <ul className="note-list" aria-label={t("inbox.confirm.daemonNotes")}>
            {done.notes.map((note, index) => (
              <li key={`${String(index)}-${note}`} className="agent-text" dir="auto" lang="en">
                {shownConfirmText(note)}
              </li>
            ))}
          </ul>
        </div>
        <div className="dialog-actions">
          <Button ref={closeButton} data-autofocus="" aria-describedby={doneId} onClick={cancel}>
            {t("inbox.dialog.close")}
          </Button>
        </div>
      </Dialog>
    );
  }

  const effect = review === undefined ? null : effectOf(review, t);
  return (
    <Dialog title={t("inbox.confirm.title", { id })} onClose={cancel} className="confirm-dialog">
      <div className="confirm-body" role="group" aria-label={t("inbox.confirm.does", { id })} tabIndex={0}>
        {review !== undefined && (
          <>
            <p className="dialog-text agent-text" dir="auto">
              {shownConfirmText(review.summary ?? review.target_id ?? review.kind ?? id)}
            </p>
            <ChangeShown review={review} />
          </>
        )}
        {effect !== null && <p className="dialog-text dialog-effect">{effect}</p>}
        <dl className="pairs">
          <dt>{t("inbox.confirm.as")}</dt>
          <dd>
            <span className="mono">
              <bdi>{shownConfirmSlot(team.login)}</bdi>
            </span>
            {team.name !== null && (
              <>
                {" "}
                (<bdi>{shownConfirmSlot(team.name)}</bdi>)
              </>
            )}
          </dd>
        </dl>
        {asked === undefined
          ? failedRead === null && (
              <div aria-busy="true">
                <Skeleton label={t("inbox.confirm.reading", { id })} lines={2} />
              </div>
            )
          : (
              <figure className="diff-figure" aria-label={t("inbox.confirm.question")}>
                <pre className="node-text agent-text" dir="auto" lang="en">
                  {shownConfirmText(asked.question)}
                </pre>
              </figure>
            )}
      </div>
      {failure !== null && (
        <div className="refusal" role="alert">
          <p className="refusal-title">
            <Icon name="alert" />
            <span>{headingOf(failure, t)}</span>
          </p>
          <p className="verbatim" lang="en">
            {failure.message}
          </p>
        </div>
      )}
      {moved && failure === null && (
        <div className="notice" role="alert">
          <p className="notice-title">
            <Icon name="alert" />
            <span>{t("inbox.confirm.moved")}</span>
          </p>
          <p>{t("inbox.confirm.movedText")}</p>
        </div>
      )}
      <div className="dialog-actions">
        <Button
          appearance={review?.staged?.decision === "reject" ? "danger" : "primary"}
          aria-disabled={!ready}
          onClick={() => {
            void submit();
          }}
        >
          {fill(t(pending ? "inbox.confirm.confirming" : "inbox.confirm.confirm", { id: slot("id") }), { id: <span className="sr-only">{id}</span> })}
        </Button>
        {stale && <Button onClick={readAgain}>{t("inbox.confirm.readAgain")}</Button>}
        <Button ref={cancelButton} data-autofocus="" aria-disabled={pending} onClick={cancel}>
          {t("inbox.dialog.cancel")}
        </Button>
      </div>
    </Dialog>
  );
}
