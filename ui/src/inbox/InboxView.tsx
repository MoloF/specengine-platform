import { lazy, Suspense, useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useInbox, useProposal, useStageDecision, withActing, type ProposalQuery } from "../api/queries";
import type { ApiError, Proposal, StageChoice, Team } from "../api/types";
import { replaceHash } from "../app/location";
import { sectionHash } from "../app/routes";
import { useOpenShortcuts } from "../app/shortcuts";
import { actingName, useActing, useAllowed, useConfirmer } from "../board/acting";
import { actOff } from "../board/labels";
import { useTranslate, type Translate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { useAnnounce } from "../ui/announcer";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { hasModifier, isTextField, letterOf } from "../ui/keys";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useFocusLater } from "../ui/useFocusLater";
import { useNow } from "../ui/useNow";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { confirmedLine, DECISION_KEYS, isStaged, stagedAnnouncement, type DecisionKind } from "./decisions";
import { matchesFilter } from "./filter";
import { severityRank } from "./labels";
import { queueOrder } from "./order";
import { ProposalCard } from "./ProposalCard";
import { ProposalList } from "./ProposalList";
import { sharesTarget } from "./targets";

/**
 * The decision dialog arrives in a chunk of its own on the first opening; meanwhile focus stays on
 * what opened it and nothing else opens (the dialog counts as open).
 */
const DecisionDialog = lazy(() => import("./DecisionDialog").then((module) => ({ default: module.DecisionDialog })));

/** The Confirm dialog, a hub's alone, so a local page never loads its chunk. */
const ConfirmDialog = lazy(() => import("./ConfirmDialog").then((module) => ({ default: module.ConfirmDialog })));

/** A routed proposal the inbox no longer lists, shown to read: a chunk of its own, loaded only for such a route. */
const ClosedProposal = lazy(() => import("./ClosedProposal").then((module) => ({ default: module.ClosedProposal })));

const LETTERS = new Map<string, DecisionKind>(DECISION_KEYS.map(([kind, key]) => [key, kind]));

/** A line the view shows after a write, worded in the language shown when it renders. */
type Said = (t: Translate) => string;

/**
 * The open decision dialog: the proposal's review document as it was when opened, so a re-read
 * cannot pull the dialog away; the dialog itself is given the current read and says when it changed.
 */
interface OpenDecision {
  id: string;
  proposal: Proposal;
  kind: DecisionKind;
}

/**
 * Where a routed proposal the inbox does not list stands: a skeleton while it is read, the daemon's
 * words and Retry when the read fails, its read-only pane once its review document is in; nothing for
 * the exit-1 document, which the note under the filter names. It reads nothing of its own: `review` is
 * the view's one query for the ID.
 */
function ClosedRead({ project, id, review }: { project: string; id: string; review: ProposalQuery }) {
  const t = useTranslate();
  const failure = useRetainedFailure(review.error, review.isFetching);
  const place = useRef<HTMLDivElement>(null);
  const retried = useRetryFocus(review.data, () => place.current);
  const reading = (
    <div aria-busy="true">
      <Skeleton label={t("inbox.closed.reading", { id })} lines={4} />
    </div>
  );
  let shown;
  if (review.data === undefined) {
    shown =
      failure === null ? (
        reading
      ) : (
        <ErrorPanel
          title={t("inbox.card.reviewFailed", { id })}
          message={apiErrorOf(failure).message}
          retrying={review.isFetching}
          attempt={review.errorUpdateCount}
          onRetry={() => {
            retried();
            void review.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else if (review.data.id === null) {
    return null;
  } else {
    shown = (
      <Suspense fallback={reading}>
        <ClosedProposal project={project} proposal={review.data} />
      </Suspense>
    );
  }
  return (
    <div ref={place} className="closed-read" tabIndex={-1}>
      {shown}
    </div>
  );
}

/**
 * The owner's queue: the inbox's entries by severity then age, the selected one's card from its
 * review document, the four decisions. Accept and Reject stage the owner's choice, confirmed only
 * by `spec approve|reject PR` on a terminal (`docs/canon/decision-staging.md` "UI", ADR-0035); a
 * staged proposal stays listed, its card showing the command and Unstage. Needs clarification and
 * Defer send nothing. On a hub a login `[team]` entitles also has Confirm on a staged choice, one
 * dialog per proposal (`docs/features/hub-confirm.md`); the dialog sits outside the queue, so its
 * keys never reach the queue's.
 */
export function InboxView({ project, selectedId }: { project: string; selectedId: string | null }) {
  const t = useTranslate();
  const inbox = useInbox(project);
  const failure = useRetainedFailure(inbox.error, inbox.isFetching);
  const decide = useStageDecision(project);
  // The decision dialog stages under the acting name when one is known, the login on a hub (docs/features/card-questions.md "Acceptance criteria", L4).
  const [acting] = useActing();
  const canStage = useAllowed(project)("stage");
  const confirmer = useConfirmer(project);
  const now = useNow();
  const openShortcuts = useOpenShortcuts();
  const focusLater = useFocusLater();
  const announce = useAnnounce();
  const filterId = useId();
  const queue = useRef<HTMLDivElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const returnFocus = useRef<HTMLElement | null>(null);
  /** The open dialog as event handlers and async replies see it, ahead of the next render. */
  const openDecision = useRef<OpenDecision | null>(null);
  /** A decision is on its way to the daemon: nothing opens or closes a dialog until it answers. */
  const deciding = useRef(false);
  /** The view is on screen. A reply after the owner left (browser Back) is only spoken. */
  const mounted = useRef(false);
  /** Retry was pressed on the inbox's error panel; the answer takes away the Retry that had focus. */
  const retried = useRef(false);
  const [query, setQuery] = useState("");
  const [dialog, setDialog] = useState<OpenDecision | null>(null);
  /** The open Confirm dialog: its proposal and the `[team]` row it opened under. */
  const [confirming, setConfirming] = useState<{ id: string; team: Team } | null>(null);
  const [result, setResult] = useState<Said | null>(null);
  /** The daemon's words refusing the last unstage, by proposal. */
  const [unstageRefusal, setUnstageRefusal] = useState<{ id: string; message: string } | null>(null);

  const ordered = inbox.data === undefined ? [] : queueOrder(inbox.data.proposals);
  const visible = ordered.filter((proposal) => matchesFilter(proposal, query));
  // A routed ID the inbox lists, or none: today's selection, the first visible item for none or one the
  // filter hides. One it does not list selects nothing: it is read on its own and shown to read
  // (docs/features/inbox-closed-link.md "Data"), and the route stays.
  const listed = selectedId !== null && ordered.some((proposal) => proposal.id === selectedId);
  const selected = selectedId === null || listed ? (visible.find((proposal) => proposal.id === selectedId) ?? visible[0] ?? null) : null;
  const closedId = selectedId !== null && inbox.data !== undefined && !listed ? selectedId : null;
  // One query per ID: the selected item's review document, or the closed one's, which the same events read again.
  const review = useProposal(project, selected?.id ?? closedId);
  /** The selected proposal's review document, once read (not the exit-1 document). */
  const reviewed = review.data !== undefined && review.data.id !== null && review.data.id === selected?.id ? review.data : null;

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      // No dialog of this view is open any more: a late reply must not select or focus through it.
      openDecision.current = null;
    };
  }, []);

  // After a successful Retry: focus the selected item (the queue's keys act from there), else the heading.
  useEffect(() => {
    if (!retried.current || inbox.data === undefined) {
      return;
    }
    retried.current = false;
    if (focusIsLost()) {
      (queue.current?.querySelector<HTMLElement>('[role="option"][aria-selected="true"]') ?? heading.current)?.focus();
    }
  }, [inbox.data]);

  function itemElement(id: string): HTMLElement | null {
    const items = queue.current?.querySelectorAll<HTMLElement>("[data-proposal]") ?? [];
    return Array.from(items).find((item) => item.dataset.proposal === id) ?? null;
  }

  function select(id: string, focus: boolean) {
    replaceHash(sectionHash(project, "inbox", id));
    if (focus) {
      focusLater(() => itemElement(id));
    }
  }

  function move(step: number, to?: "first" | "last") {
    if (visible.length === 0) {
      return;
    }
    const at = selected === null ? -1 : visible.indexOf(selected);
    const index =
      to === "first" ? 0 : to === "last" ? visible.length - 1 : Math.min(visible.length - 1, Math.max(0, at + step));
    const target = visible[index];
    if (target !== undefined) {
      select(target.id, true);
    }
  }

  function showDialog(next: OpenDecision | null) {
    openDecision.current = next;
    setDialog(next);
  }

  /**
   * Opens a decision on the selected proposal once its review document is read (the options come
   * from it, and the `updated_at` a stage is sent against); nothing while a dialog is open or a
   * stage pending. A drift item opens none: decided on a terminal only, it is never staged
   * (`docs/features/drift-queue.md` "Review, inbox, round, stage"); its key only says so. Nor does a
   * decision that stages where a hub's `allowed` omits `stage`: its key says that too.
   */
  function openDialog(kind: DecisionKind) {
    if (selected === null && closedId !== null) {
      // Nothing selected: a proposal out of the inbox is there to read, with nothing to decide.
      announce(t("inbox.closed.noDecision", { id: closedId }));
      return;
    }
    if (selected === null || reviewed === null || reviewed.updated_at === null || openDecision.current !== null || confirming !== null || deciding.current || decide.isPending) {
      return;
    }
    if (reviewed.drift !== null) {
      announce(t("inbox.driftKey", { id: selected.id }));
      return;
    }
    if (isStaged(kind) && !canStage) {
      announce(actOff("stage", t));
      return;
    }
    returnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    showDialog({ id: selected.id, proposal: reviewed, kind });
  }

  function closeDialog() {
    if (deciding.current) {
      return;
    }
    showDialog(null);
    const back = returnFocus.current;
    focusLater(() => (back?.isConnected === true ? back : null));
  }

  /** Closes the dialog deciding `id` and no other; false when that dialog is not the open one. */
  function closeOwnDialog(id: string): boolean {
    if (openDecision.current?.id !== id) {
      return false;
    }
    showDialog(null);
    return true;
  }

  /** After a stage leaves the dialog: focus back on what opened it, else the selected item. */
  function focusAfterStage(id: string) {
    const back = returnFocus.current;
    focusLater(() => (back?.isConnected === true ? back : itemElement(id)));
  }

  /**
   * Stages the owner's choice once, against the `updated_at` the dialog shows. Only the call is
   * guarded: what follows a success runs outside the catch, so it can never be shown as a refusal.
   * The proposal stays in the queue. Resolves to the refusal for the dialog to show (a 409 too: the
   * inbox and the proposal are read again, and the dialog shows a changed proposal as changed), or
   * null when the dialog is closed (or gone with the view).
   */
  async function submitStage(id: string, stage: StageChoice, updatedAt: string): Promise<string | null> {
    deciding.current = true;
    let review: Proposal;
    try {
      review = await decide.mutateAsync(withActing({ id, stage, updatedAt }, actingName(acting)));
    } catch (error) {
      deciding.current = false;
      return refused(id, apiErrorOf(error));
    }
    deciding.current = false;
    const said: Said = (shown) => stagedAnnouncement(id, stage.decision, review, shown);
    announce(said(t));
    if (!mounted.current) {
      return null;
    }
    setUnstageRefusal(null);
    setResult(() => said);
    if (closeOwnDialog(id)) {
      focusAfterStage(id);
    }
    return null;
  }

  /** A refusal stays in the dialog, or is spoken when the view is gone. */
  function refused(id: string, refusal: ApiError): string | null {
    if (!mounted.current) {
      announce(t("inbox.stageRefusedSaid", { id, message: refusal.message }), "assertive");
      return null;
    }
    return refusal.message;
  }

  /** Opens the Confirm dialog on `id`, nothing while another dialog is open or a stage is on its way. */
  function openConfirm(id: string) {
    if (confirmer === null || confirming !== null || openDecision.current !== null || deciding.current || decide.isPending) {
      return;
    }
    returnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setConfirming({ id, team: confirmer });
  }

  /** Cancel keeps the stage: nothing is sent, focus goes back to what opened the dialog. */
  function closeConfirm() {
    setConfirming(null);
    const back = returnFocus.current;
    focusLater(() => (back?.isConnected === true ? back : null));
  }

  /**
   * The daemon's answer, spoken and shown; the item has left the queue, so the route drops its ID (a
   * routed ID out of the inbox would show it closed) and focus goes to the item now selected.
   */
  function confirmed(id: string, answered: Proposal) {
    const said: Said = (shown) => confirmedLine(id, answered, shown);
    announce(said(t));
    if (!mounted.current) {
      return;
    }
    replaceHash(sectionHash(project, "inbox"));
    setUnstageRefusal(null);
    setResult(() => said);
    setConfirming(null);
    focusLater(() => queue.current?.querySelector<HTMLElement>('[role="option"][aria-selected="true"]') ?? heading.current);
  }

  /** Drops the selected proposal's staged choice: one call; a refusal shows on the card in the daemon's words. */
  async function unstage(id: string) {
    if (deciding.current || openDecision.current !== null || confirming !== null) {
      return;
    }
    deciding.current = true;
    try {
      await decide.mutateAsync({ id, stage: null });
    } catch (error) {
      deciding.current = false;
      const refusal = apiErrorOf(error);
      announce(t("inbox.unstageRefusedSaid", { id, message: refusal.message }), "assertive");
      if (mounted.current) {
        setUnstageRefusal({ id, message: refusal.message });
      }
      return;
    }
    deciding.current = false;
    const said: Said = (shown) => shown("inbox.unstagedSaid", { id });
    announce(said(t));
    if (!mounted.current) {
      return;
    }
    setUnstageRefusal(null);
    setResult(() => said);
    // The Unstage button that had focus is gone with the staged choice.
    focusLater(() => itemElement(id));
  }

  function onQueueKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (dialog !== null || confirming !== null || decide.isPending || hasModifier(event) || isTextField(event.target)) {
      return;
    }
    const inList = event.target instanceof Element && event.target.closest('[role="listbox"]') !== null;
    const letter = letterOf(event);
    const kind = letter === null ? undefined : LETTERS.get(letter);
    if (kind !== undefined) {
      openDialog(kind);
    } else if (letter === "j" || (inList && event.key === "ArrowDown")) {
      move(1);
    } else if (letter === "k" || (inList && event.key === "ArrowUp")) {
      move(-1);
    } else if (inList && event.key === "Home") {
      move(0, "first");
    } else if (inList && event.key === "End") {
      move(0, "last");
    } else if (event.key === "?") {
      openShortcuts();
    } else {
      return;
    }
    event.preventDefault();
  }

  // The review's exit-1 document for a routed ID the inbox does not list: no proposal by it, so no pane.
  const unknown = closedId !== null && review.data !== undefined && review.data.id === null && (
    <p className="note" role="status">
      <Icon name="info" />
      <span>{t("inbox.unknown", { id: closedId })}</span>
    </p>
  );

  let body;
  if (inbox.data === undefined) {
    body =
      failure === null ? (
        <div className="queue-loading" aria-busy="true">
          <Skeleton label={t("inbox.loading", { project })} lines={4} />
        </div>
      ) : (
        <ErrorPanel
          title={t("inbox.failed")}
          message={apiErrorOf(failure).message}
          retrying={inbox.isFetching}
          attempt={inbox.errorUpdateCount}
          onRetry={() => {
            retried.current = true;
            void inbox.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else if (ordered.length === 0) {
    const clear = (
      <EmptyState
        level={2}
        title={t("inbox.clearTitle")}
        description={
          <>
            <p>{t("inbox.clearText", { project })}</p>
            <p>{fill(t("inbox.clearNext", { link: slot("link") }), { link: <a href={sectionHash(project, "tasks")}>{t("inbox.clearNextLink")}</a> })}</p>
          </>
        }
      />
    );
    // A clear queue still shows a routed proposal it no longer lists, beside the empty state.
    body =
      closedId === null ? (
        clear
      ) : (
        <div className="queue" ref={queue} onKeyDown={onQueueKeyDown}>
          <div className="queue-pane">
            {clear}
            {unknown}
          </div>
          <ClosedRead project={project} id={closedId} review={review} />
        </div>
      );
  } else {
    const high = ordered.filter((proposal) => severityRank(proposal.severity) === 0).length;
    body = (
      <div className="queue" ref={queue} onKeyDown={onQueueKeyDown}>
        <div className="queue-pane">
          <p className="queue-summary">{t("inbox.queueSummary", { count: ordered.length, high })}</p>
          <label className="field-label" htmlFor={filterId}>
            {t("inbox.filter")}
          </label>
          <input
            id={filterId}
            className="field"
            type="search"
            autoComplete="off"
            spellCheck={false}
            value={query}
            placeholder={t("inbox.filterPlaceholder")}
            onChange={(event) => {
              setQuery(event.target.value);
            }}
          />
          {unknown}
          {inbox.data.notes.length > 0 && (
            <ul className="note-list" aria-label={t("inbox.notes")}>
              {inbox.data.notes.map((note) => (
                <li key={note} lang="en">
                  {note}
                </li>
              ))}
            </ul>
          )}
          {visible.length === 0 ? (
            // Under the view's h1: no proposal is shown, so no h2 stands between.
            <EmptyState
              level={2}
              title={t("inbox.noMatch", { query })}
              primary={
                <Button
                  appearance="primary"
                  onClick={() => {
                    setQuery("");
                  }}
                >
                  {t("inbox.clearFilter")}
                </Button>
              }
            />
          ) : (
            <ProposalList
              proposals={visible}
              selectedId={selected?.id ?? null}
              now={now}
              onSelect={(id) => {
                select(id, true);
              }}
            />
          )}
          {inbox.error !== null && (
            <p className="note" role="alert">
              {fill(t("inbox.failedAgain", { message: slot("message") }), { message: <span lang="en">{apiErrorOf(inbox.error).message}</span> })}
            </p>
          )}
        </div>
        {selected !== null && (
          <ProposalCard
            project={project}
            entry={selected}
            review={review}
            others={ordered.filter((other) => sharesTarget(other, selected))}
            now={now}
            deciding={decide.isPending}
            canStage={canStage}
            unstageRefusal={unstageRefusal !== null && unstageRefusal.id === selected.id ? unstageRefusal.message : null}
            onDecide={openDialog}
            onUnstage={() => {
              void unstage(selected.id);
            }}
            onConfirm={
              confirmer === null
                ? undefined
                : () => {
                    openConfirm(selected.id);
                  }
            }
            onSelect={(id) => {
              select(id, true);
            }}
          />
        )}
        {closedId !== null && <ClosedRead project={project} id={closedId} review={review} />}
      </div>
    );
  }

  return (
    <section className="view inbox" aria-labelledby="inbox-title" aria-busy={inbox.data === undefined && failure === null}>
      <header className="view-head">
        <h1 id="inbox-title" ref={heading} tabIndex={-1}>
          {t("inbox.title")}
        </h1>
        <p className="view-about">{t("inbox.about")}</p>
      </header>
      {/* A visible copy; screen readers hear it through the page's live region (useAnnounce). */}
      {result !== null && <p className="decision-result">{result(t)}</p>}
      {body}
      {dialog !== null && (
        <Suspense fallback={null}>
          <DecisionDialog
            key={`${dialog.id}-${dialog.kind}`}
            id={dialog.id}
            proposal={reviewed !== null && reviewed.id === dialog.id ? reviewed : dialog.proposal}
            kind={dialog.kind}
            onCancel={closeDialog}
            onSubmit={(stage, updatedAt) => submitStage(dialog.id, stage, updatedAt)}
          />
        </Suspense>
      )}
      {confirming !== null && (
        <Suspense fallback={null}>
          <ConfirmDialog
            key={confirming.id}
            project={project}
            id={confirming.id}
            team={confirming.team}
            onClose={closeConfirm}
            onConfirmed={(answered) => {
              confirmed(confirming.id, answered);
            }}
          />
        </Suspense>
      )}
    </section>
  );
}
