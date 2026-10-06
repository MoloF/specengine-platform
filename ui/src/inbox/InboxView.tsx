import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { apiErrorOf, DECIDED_ELSEWHERE } from "../api/client";
import { useDecideProposal, useInbox, useProposal } from "../api/queries";
import type { ApiError, Decision, DecisionResult, Proposal } from "../api/types";
import { replaceHash } from "../app/location";
import { sectionHash } from "../app/routes";
import { useOpenShortcuts } from "../app/shortcuts";
import { useAnnounce } from "../ui/announcer";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { hasModifier, isTextField } from "../ui/keys";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useFocusLater } from "../ui/useFocusLater";
import { useNow } from "../ui/useNow";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { DecisionDialog } from "./DecisionDialog";
import { DECISION_KEYS, type DecisionKind } from "./decisions";
import { matchesFilter } from "./filter";
import { severityRank } from "./labels";
import { queueOrder } from "./order";
import { ProposalCard } from "./ProposalCard";
import { ProposalList } from "./ProposalList";
import { sharesTarget } from "./targets";

const LETTERS = new Map<string, DecisionKind>(DECISION_KEYS.map(([kind, key]) => [key, kind]));

function announcementOf(id: string, result: DecisionResult, decision: Decision): string {
  switch (decision.decision) {
    case "accept":
      return result.commit === null
        ? `Accepted ${id}.`
        : `Accepted ${id}: committed ${result.commit.sha} "${result.commit.subject}".`;
    case "reject":
      return `Rejected ${id}.`;
    case "needs_clarification":
      return `Sent ${id} back: needs clarification.`;
    case "defer":
      return `Deferred ${id}.`;
  }
}

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
 * The owner's queue: the inbox's entries by severity then age, the selected one's card from its
 * review document, the four decisions (the daemon refuses each, naming the terminal command).
 */
export function InboxView({ project, selectedId }: { project: string; selectedId: string | null }) {
  const inbox = useInbox(project);
  const failure = useRetainedFailure(inbox.error, inbox.isFetching);
  const decide = useDecideProposal(project);
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
  const [result, setResult] = useState("");
  const [problem, setProblem] = useState<{ id: string; message: string } | null>(null);

  const ordered = inbox.data === undefined ? [] : queueOrder(inbox.data.proposals);
  const visible = ordered.filter((proposal) => matchesFilter(proposal, query));
  const selected = visible.find((proposal) => proposal.id === selectedId) ?? visible[0] ?? null;
  const absentId =
    selectedId !== null && inbox.data !== undefined && !ordered.some((proposal) => proposal.id === selectedId)
      ? selectedId
      : null;
  const review = useProposal(project, selected?.id ?? null);
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
   * from it); nothing while a dialog is open or a decision pending.
   */
  function openDialog(kind: DecisionKind) {
    if (selected === null || reviewed === null || openDecision.current !== null || deciding.current || decide.isPending) {
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

  /** After a decision leaves the dialog: select and focus the next item, else the heading. */
  function focusNextAfter(id: string) {
    const at = visible.findIndex((candidate) => candidate.id === id);
    const next = visible[at + 1] ?? visible[at - 1];
    if (next === undefined) {
      focusLater(() => heading.current);
      return;
    }
    select(next.id, true);
  }

  /**
   * Sends the owner's decision once. Only the call is guarded: what follows a success runs outside
   * the catch, so it can never be shown as a refusal. Resolves to the refusal for the dialog to show,
   * or null when the dialog is closed (or gone with the view).
   */
  async function submitDecision(id: string, decision: Decision): Promise<string | null> {
    deciding.current = true;
    let decided: DecisionResult;
    try {
      decided = await decide.mutateAsync({ id, decision });
    } catch (error) {
      deciding.current = false;
      return refused(id, apiErrorOf(error));
    }
    deciding.current = false;
    const said = announcementOf(id, decided, decision);
    announce(said);
    if (!mounted.current) {
      return null;
    }
    setProblem(null);
    setResult(said);
    if (closeOwnDialog(id)) {
      focusNextAfter(id);
    }
    return null;
  }

  /** A refusal: 409 closes the dialog; any other stays in it, or is spoken when the view is gone. */
  function refused(id: string, refusal: ApiError): string | null {
    if (refusal.status === DECIDED_ELSEWHERE) {
      announce(`${id} was decided elsewhere; the inbox is read again. ${refusal.message}`, "assertive");
      if (mounted.current) {
        setResult("");
        setProblem({ id, message: refusal.message });
        if (closeOwnDialog(id)) {
          focusNextAfter(id);
        }
      }
      return null;
    }
    if (!mounted.current) {
      announce(`The daemon refused the decision on ${id}; nothing changed. ${refusal.message}`, "assertive");
      return null;
    }
    return refusal.message;
  }

  function onQueueKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (dialog !== null || decide.isPending || hasModifier(event) || isTextField(event.target)) {
      return;
    }
    const inList = event.target instanceof Element && event.target.closest('[role="listbox"]') !== null;
    const kind = LETTERS.get(event.key);
    if (kind !== undefined) {
      openDialog(kind);
    } else if (event.key === "j" || (inList && event.key === "ArrowDown")) {
      move(1);
    } else if (event.key === "k" || (inList && event.key === "ArrowUp")) {
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

  let body;
  if (inbox.data === undefined) {
    body =
      failure === null ? (
        <div className="queue-loading" aria-busy="true">
          <Skeleton label={`Loading the inbox of ${project}`} lines={4} />
        </div>
      ) : (
        <ErrorPanel
          title="The inbox could not be loaded"
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
    body = (
      <div className="empty-state">
        <h2>The queue is clear</h2>
        <p>
          No proposal in {project} waits for your decision. Agents add one when the code and the spec disagree or a
          section needs an edit, and keep working on their working answer meanwhile.
        </p>
        <p>
          Next: <a href={sectionHash(project, "tasks")}>approve the tasks that are ready for development</a>.
        </p>
      </div>
    );
  } else {
    const high = ordered.filter((proposal) => severityRank(proposal.severity) === 0).length;
    body = (
      <div className="queue" ref={queue} onKeyDown={onQueueKeyDown}>
        <div className="queue-pane">
          <p className="queue-summary">
            {ordered.length} in the queue, {high} high severity
          </p>
          <label className="field-label" htmlFor={filterId}>
            Filter
          </label>
          <input
            id={filterId}
            className="field"
            type="search"
            autoComplete="off"
            spellCheck={false}
            value={query}
            placeholder="ID, words, kind, branch or target"
            onChange={(event) => {
              setQuery(event.target.value);
            }}
          />
          {absentId !== null && (
            <p className="note" role="status">
              <Icon name="info" />
              <span>
                {absentId} is not in this inbox; it may have been decided. The first proposal is shown.
              </span>
            </p>
          )}
          {inbox.data.notes.length > 0 && (
            <ul className="note-list" aria-label="Notes from the daemon">
              {inbox.data.notes.map((note) => (
                <li key={note}>{note}</li>
              ))}
            </ul>
          )}
          {selected === null ? (
            <div className="empty-filter">
              <p>No proposal matches &ldquo;{query}&rdquo;.</p>
              <button
                type="button"
                className="button"
                onClick={() => {
                  setQuery("");
                }}
              >
                Clear the filter
              </button>
            </div>
          ) : (
            <ProposalList
              proposals={visible}
              selectedId={selected.id}
              now={now}
              onSelect={(id) => {
                select(id, true);
              }}
            />
          )}
          {inbox.error !== null && (
            <p className="note" role="alert">
              The inbox could not be read again: {apiErrorOf(inbox.error).message}
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
            onDecide={openDialog}
            onSelect={(id) => {
              select(id, true);
            }}
          />
        )}
      </div>
    );
  }

  return (
    <section className="view inbox" aria-labelledby="inbox-title" aria-busy={inbox.data === undefined && failure === null}>
      <header className="view-head">
        <h1 id="inbox-title" ref={heading} tabIndex={-1}>
          Inbox
        </h1>
        <p className="view-about">
          Proposals waiting for your decision. Nothing waits on them: agents work on with their working answer, and
          your control point is approving tasks.
        </p>
      </header>
      {/* Visible copies; screen readers hear both through the page's live regions (useAnnounce). */}
      {result !== "" && <p className="decision-result">{result}</p>}
      {problem !== null && (
        <div className="notice notice-problem">
          <p>{problem.id} was decided elsewhere; the inbox is read again.</p>
          <p className="verbatim">{problem.message}</p>
        </div>
      )}
      {body}
      {dialog !== null && (
        <DecisionDialog
          key={`${dialog.id}-${dialog.kind}`}
          id={dialog.id}
          proposal={reviewed !== null && reviewed.id === dialog.id ? reviewed : dialog.proposal}
          kind={dialog.kind}
          onCancel={closeDialog}
          onSubmit={(decision) => submitDecision(dialog.id, decision)}
        />
      )}
    </section>
  );
}
