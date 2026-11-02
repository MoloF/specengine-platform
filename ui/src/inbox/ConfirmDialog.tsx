import { useEffect, useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import { useConfirmProposal, useConfirmRead, useProposal } from "../api/queries";
import type { ApiError, Proposal, Team } from "../api/types";
import { useAnnounce } from "../ui/announcer";
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
const NO_ANSWER = "No answer: read again to see whether it was confirmed.";

/** The heading of a refusal; the daemon's own words stand under it. */
function headingOf({ status }: ApiError): string {
  return status === 0 ? NO_ANSWER : "The daemon refused to confirm it.";
}

/**
 * A kind that applies with no diff to show (`docs/features/hub-confirm.md` "Description and interactions"):
 * nothing to consent to, so Confirm stays off. Read by the keys, never the kind: an applying kind's
 * document carries `new_text`, a deciding kind's and a drift item's never.
 */
function lacksDiff(review: Proposal): boolean {
  return review.new_text !== null && review.diff === null;
}

/** What the confirm does to the staged choice, the page's words; the question says it in the terminal's. */
function effectOf(review: Proposal): string | null {
  const stage = review.staged;
  if (stage === null) {
    return null;
  }
  return stage.decision === "reject"
    ? "Confirming rejects this proposal under your login; no spec file changes."
    : "Confirming applies this staged choice as one commit in this hub's checkout, under your login; nothing is pushed.";
}

/**
 * The change itself: the section diff, else the record, each written as shownConfirmText. A deciding
 * kind's record to come stands in the question. An applying kind with no diff: the review's notes
 * in its place, which say why.
 */
function ChangeShown({ review }: { review: Proposal }) {
  if (review.diff !== null) {
    return <DiffView diff={shownConfirmText(review.diff)} />;
  }
  if (lacksDiff(review)) {
    return (
      <div className="notice">
        <p className="notice-title">
          <Icon name="alert" />
          <span>No diff to show: this change cannot be confirmed here</span>
        </p>
        {review.notes.length > 0 && (
          <ul className="note-list" aria-label="The review's notes">
            {review.notes.map((note, index) => (
              <li key={`${String(index)}-${note}`} className="agent-text" dir="auto">
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
    <figure className="diff-figure" aria-label={`Record ${review.record_id ?? ""}`.trim()}>
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
  /** The daemon confirmed it: its answer, the review document. The caller closes the dialog. */
  onConfirmed: (review: Proposal) => void;
}) {
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

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

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
    if (!inFlight.current) {
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
        announce(`${headingOf(refused)} ${id}: ${refused.message}`, "assertive");
      }
      return;
    }
    onConfirmed(answered);
  }

  const effect = review === undefined ? null : effectOf(review);
  return (
    <Dialog title={`Confirm ${id}`} onClose={cancel} className="confirm-dialog">
      <div className="confirm-body" role="group" aria-label={`What confirming ${id} does`} tabIndex={0}>
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
          <dt>Confirming as</dt>
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
                <Skeleton label={`Reading the question of ${id}`} lines={2} />
              </div>
            )
          : (
              <figure className="diff-figure" aria-label="The question a terminal would ask">
                <pre className="node-text agent-text" dir="auto">
                  {shownConfirmText(asked.question)}
                </pre>
              </figure>
            )}
      </div>
      {failure !== null && (
        <div className="refusal" role="alert">
          <p className="refusal-title">
            <Icon name="alert" />
            <span>{headingOf(failure)}</span>
          </p>
          <p className="verbatim">{failure.message}</p>
        </div>
      )}
      {moved && failure === null && (
        <div className="notice" role="alert">
          <p className="notice-title">
            <Icon name="alert" />
            <span>This proposal changed since this dialog read it</span>
          </p>
          <p>Nothing was confirmed. Read it again, check it, then confirm.</p>
        </div>
      )}
      <div className="dialog-actions">
        <button
          type="button"
          className={review?.staged?.decision === "reject" ? "button button-danger" : "button button-primary"}
          aria-disabled={!ready}
          onClick={() => {
            void submit();
          }}
        >
          {pending ? "Confirming" : "Confirm"} <span className="sr-only">{id}</span>
        </button>
        {stale && (
          <button type="button" className="button" onClick={readAgain}>
            Read again
          </button>
        )}
        <button ref={cancelButton} type="button" className="button" data-autofocus="" aria-disabled={pending} onClick={cancel}>
          Cancel
        </button>
      </div>
    </Dialog>
  );
}
