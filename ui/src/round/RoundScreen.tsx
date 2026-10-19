import { useCallback, useEffect, useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import { useProjects, useRoundPosting, useStageDecision, useStageRoundAnswers, type RoundQuery } from "../api/queries";
import type { RoundAnswersOutcome, RoundItem, RoundSheet } from "../api/types";
import { sectionHash } from "../app/routes";
import { useAnnounce } from "../ui/announcer";
import { CopyButton } from "../ui/CopyButton";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { shownSlot } from "../ui/text";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { answerOf, stateOf, withDraft, type Draft } from "./drafts";
import { useRoundEntry, type RoundEntry, type RoundMemory } from "./memory";
import { RoundItemCard } from "./RoundItemCard";
import { ROUND_COMMAND, ROUND_LIMIT_MAX } from "./text";

const COPY_FAILED = "Copy failed: select the command and copy it";

/** The outcome of an answer the daemon staged. */
const STAGED = "staged";

/** `1 answer`, `2 answers`: a count and its noun, with an `s` past one. */
function counted(count: number, noun: string): string {
  return `${String(count)} ${noun}${count === 1 ? "" : "s"}`;
}

/** `a, b and c`. */
function listed(ids: readonly string[]): string {
  return ids.length <= 1 ? ids.join("") : `${ids.slice(0, -1).join(", ")} and ${ids.at(-1) ?? ""}`;
}

/** What a POST's 200 says, spoken politely and shown: what was staged, what was not, the next step. */
function summaryOf(document: RoundAnswersOutcome): string {
  const staged = document.answers.filter((answer) => answer.outcome === STAGED).map((answer) => answer.id);
  const others = document.answers.filter((answer) => answer.outcome !== STAGED).map((answer) => answer.id);
  const notStaged = others.length === 0 ? "" : `; ${counted(others.length, "answer")} not staged: ${listed(others)}`;
  const next =
    staged.length === 0
      ? "Nothing new is staged; each refusal is shown on its item."
      : `Nothing is applied yet. Confirm on a terminal: ${ROUND_COMMAND}; each answer is asked again there, one at a time.`;
  return `Staged ${counted(staged.length, "answer")}${notStaged}. ${next}`;
}

/**
 * The round's terminal step (`docs/features/ui-round.md` "Description and interactions"): the
 * staged answers are confirmed only on a terminal, one `[y/N]` each, in the project's root. Copy
 * takes the command alone, fixed words; the root is shown, never copied.
 */
function ConfirmOnTerminal({ root, count }: { root: string | null; count: number }) {
  return (
    <section className="owner-commands round-confirm" aria-labelledby="round-confirm-title">
      <h2 id="round-confirm-title" className="owner-commands-title">
        <Icon name="terminal" />
        <span>Confirm on a terminal</span>
      </h2>
      <p className="owner-commands-note">{counted(count, "item")} {count === 1 ? "is" : "are"} staged on this sheet; nothing is applied until you confirm.</p>
      <p className="round-confirm-line">
        Confirm on a terminal in <span className="mono">{root === null ? "the project's root" : <bdi>{root}</bdi>}</span>:{" "}
        <code className="command-text">{ROUND_COMMAND}</code>; each answer is asked again there, one at a time.
      </p>
      <CopyButton text={ROUND_COMMAND} context={`the command: ${ROUND_COMMAND}`} failed={COPY_FAILED} />
    </section>
  );
}

/** The map less `id`'s entry; the same map when it holds none. */
function without<T>(map: ReadonlyMap<string, T>, id: string): ReadonlyMap<string, T> {
  if (!map.has(id)) {
    return map;
  }
  const next = new Map(map);
  next.delete(id);
  return next;
}

/**
 * The Questions screen's body, its own chunk: the round sheet, the owner's drafts and their one
 * POST. The drafts, the limit and the last POST's outcome live in the shell's store per project
 * (`memory`), not here: a POST that settles after the owner left and came back clears its drafts
 * and leaves its outcome for whichever screen is on view, and while one is on its way no screen
 * sends again. A re-read drops the drafts of items no longer on the sheet, saying so, and marks
 * those whose item changed since.
 */
export function RoundScreen({ project, round, memory }: { project: string; round: RoundQuery; memory: RoundMemory }) {
  const stage = useStageRoundAnswers(project);
  const posting = useRoundPosting(project);
  const decide = useStageDecision(project);
  const projects = useProjects();
  const announce = useAnnounce();
  const focusLater = useFocusLater();
  const failure = useRetainedFailure(round.error, round.isFetching);
  const top = useRef<HTMLDivElement>(null);
  /** What the last Stage said at length, above the bar: the bar's outcome line brings it into view. */
  const atLength = useRef<HTMLElement>(null);
  const retried = useRetryFocus(round.data, () => top.current);
  /** This screen sent a write not yet answered: a second click before the next render sends nothing. */
  const sending = useRef(false);
  /** An item just unstaged: its heading takes the focus once its Unstage is gone with the stage. */
  const unstaged = useRef<string | null>(null);
  /** The last sheet whose items the drafts were held against. */
  const seen = useRef<RoundSheet | null>(null);
  const [unstageRefusal, setUnstageRefusal] = useState<{ id: string; message: string } | null>(null);
  const entry = useRoundEntry(memory, project);
  const { drafts, refused, result, failure: postFailure, dropped, limit } = entry;

  const update = useCallback(
    (change: (current: RoundEntry) => RoundEntry) => {
      memory.update(project, change);
    },
    [memory, project],
  );

  // This project's Questions screen is on view: a POST settling now is shown here, not only spoken.
  useEffect(() => memory.attach(project), [memory, project]);

  const sheet = round.data;
  // A new sheet drops the drafts of items it no longer lists, and says so for those holding an answer.
  useEffect(() => {
    if (sheet === undefined || round.isPlaceholderData || sheet === seen.current) {
      return;
    }
    seen.current = sheet;
    const onSheet = new Set(sheet.items.map((item) => item.id));
    update((current) => {
      const gone = [...current.drafts.keys()].filter((id) => !onSheet.has(id));
      if (gone.length === 0) {
        return current;
      }
      const answered = gone.filter((id) => current.drafts.get(id)?.pick !== "open");
      return {
        ...current,
        drafts: gone.reduce((next, id) => withDraft(next, id, undefined), current.drafts),
        dropped: answered.length > 0 ? answered : current.dropped,
      };
    });
  }, [sheet, round.isPlaceholderData, update]);

  // After an Unstage the button that had focus goes with the stage: focus its item's heading.
  useEffect(() => {
    const id = unstaged.current;
    const item = sheet?.items.find((candidate) => candidate.id === id);
    if (id === null || item === undefined || item.staged !== null) {
      return;
    }
    unstaged.current = null;
    if (focusIsLost()) {
      const items = top.current?.querySelectorAll<HTMLElement>("[data-item]") ?? [];
      Array.from(items)
        .find((element) => element.dataset.item === id)
        ?.querySelector<HTMLElement>("h2")
        ?.focus();
    }
  }, [sheet]);

  const items = sheet?.items ?? [];
  const ready = items.flatMap((item) => {
    const answer = answerOf(item, drafts.get(item.id));
    return answer === null ? [] : [{ item, answer }];
  });
  const changed = items.filter((item) => stateOf(item, drafts.get(item.id)) === "changed").map((item) => item.id);
  const unwritten = items.filter((item) => stateOf(item, drafts.get(item.id)) === "unwritten").map((item) => item.id);
  const replacing = ready.filter(({ item }) => item.staged_at !== null).map(({ item }) => item);
  const staged = items.filter((item) => item.staged !== null && item.staged_at !== null).length;
  const busy = posting || stage.isPending || decide.isPending;
  const root = projects.data?.find((candidate) => candidate.slug === project)?.root ?? null;

  const changeDraft = useCallback(
    (id: string, draft: Draft) => {
      update((current) => ({ ...current, drafts: withDraft(current.drafts, id, draft), refused: without(current.refused, id) }));
    },
    [update],
  );

  /**
   * The ready drafts in one POST, in sheet order; nothing while one is on its way, from here or
   * from a screen the owner left. A 200 clears the drafts it staged (unless edited meanwhile) and
   * keeps the refused ones with the daemon's reason; a failure keeps every draft, its message
   * verbatim; no answer at all reads the sheet again (useStageRoundAnswers). The outcome goes to
   * the store whether or not this screen is still on view. A failure speaks only when no Questions
   * screen of the project is on view (one that is shows it as an alert); said after the owner left,
   * the words name the project.
   */
  async function submit() {
    if (sending.current || busy || ready.length === 0) {
      return;
    }
    sending.current = true;
    const sent = new Map(ready.map(({ item }) => [item.id, drafts.get(item.id)]));
    update((current) => ({ ...current, failure: null, result: null }));
    let document: RoundAnswersOutcome;
    try {
      document = await stage.mutateAsync(ready.map(({ answer }) => answer));
    } catch (error) {
      sending.current = false;
      const refusal = apiErrorOf(error);
      update((current) => ({ ...current, failure: refusal }));
      if (!memory.onView(project)) {
        const what = refusal.status === 0 ? "no answer from the daemon; some answers may have been staged" : "the daemon refused the answers; nothing was staged";
        announce(`Questions of ${project}: ${what}. ${refusal.message}`, "assertive");
      }
      return;
    }
    sending.current = false;
    const stagedIds = new Set(document.answers.filter((answer) => answer.outcome === STAGED).map((answer) => answer.id));
    const said = summaryOf(document);
    announce(memory.onView(project) ? said : `Questions of ${project}: ${said}`);
    update((current) => ({
      ...current,
      drafts: [...stagedIds].reduce((next, id) => (current.drafts.get(id) === sent.get(id) ? withDraft(next, id, undefined) : next), current.drafts),
      refused: new Map(document.answers.filter((answer) => answer.outcome !== STAGED).map((answer) => [answer.id, answer])),
      result: { text: said, notes: document.notes, staged: stagedIds.size, sent: document.answers.length },
    }));
  }

  /**
   * Drops an item's staged choice (the decision route's DELETE), tracked from the stage this sheet
   * shows and the time it was read; nothing while the sheet shown is the last limit's, kept up while
   * another is read (it has no read time of its own).
   */
  async function unstage(item: RoundItem) {
    if (sending.current || busy || round.isPlaceholderData) {
      return;
    }
    sending.current = true;
    try {
      await decide.mutateAsync({ id: item.id, stage: null, shown: { staged: item.staged !== null, readAt: round.dataUpdatedAt } });
    } catch (error) {
      sending.current = false;
      const refusal = apiErrorOf(error);
      announce(`The daemon refused to unstage ${item.id}; nothing changed. ${refusal.message}`, "assertive");
      if (top.current !== null) {
        setUnstageRefusal({ id: item.id, message: refusal.message });
      }
      return;
    }
    sending.current = false;
    announce(`Unstaged the answer on ${item.id}; nothing is staged for it now.`);
    if (top.current !== null) {
      setUnstageRefusal(null);
      unstaged.current = item.id;
    }
  }

  // The cards are memoised: they get one stable callback, which calls this render's unstage.
  const latestUnstage = useRef(unstage);
  useEffect(() => {
    latestUnstage.current = unstage;
  });
  const onUnstage = useCallback((item: RoundItem) => {
    void latestUnstage.current(item);
  }, []);

  if (sheet === undefined) {
    return (
      <div className="round" ref={top} tabIndex={-1}>
        {failure === null ? (
          <div aria-busy="true">
            <Skeleton label={`Loading the question round of ${project}`} lines={5} />
          </div>
        ) : (
          <ErrorPanel
            title="The question round could not be loaded"
            message={apiErrorOf(failure).message}
            retrying={round.isFetching}
            attempt={round.errorUpdateCount}
            onRetry={() => {
              retried();
              void round.refetch({ cancelRefetch: false });
            }}
          />
        )}
      </div>
    );
  }

  const notes =
    sheet.notes.length === 0 ? null : (
      <ul className="note-list" aria-label="Notes from the daemon">
        {sheet.notes.map((note, index) => (
          <li key={`${String(index)}-${note}`}>{shownSlot(note)}</li>
        ))}
      </ul>
    );
  const droppedNotice =
    dropped.length === 0 ? null : (
      <div className="notice round-dropped" role="status">
        <p className="notice-title">
          <Icon name="info" />
          <span>
            {dropped.length === 1 ? `Your answer to ${listed(dropped)} was dropped` : `Your answers to ${listed(dropped)} were dropped`}
          </span>
        </p>
        <p>No longer on the sheet: decided elsewhere, or moved past the sheet's limit.</p>
        <button
          type="button"
          className="button button-quiet"
          onClick={() => {
            update((current) => ({ ...current, dropped: [] }));
            focusLater(() => top.current);
          }}
        >
          Dismiss
        </button>
      </div>
    );
  const rereadFailure =
    round.error === null ? null : (
      <p className="note" role="alert">
        The sheet could not be read again: {apiErrorOf(round.error).message}
      </p>
    );

  if (items.length === 0) {
    return (
      <div className="round" ref={top} tabIndex={-1}>
        {droppedNotice}
        <div className="empty-state">
          <h2>No open questions or discrepancies.</h2>
          {notes}
          <p>
            Agents raise a question or a discrepancy when the spec leaves something open, and keep working on their working
            answer meanwhile.
          </p>
          <p>
            Next: <a href={sectionHash(project, "inbox")}>review the change proposals in the Inbox</a>, or{" "}
            <a href={sectionHash(project, "tasks")}>approve the tasks that are ready</a>.
          </p>
        </div>
        {rereadFailure}
      </div>
    );
  }

  const all = Math.min(items.length + sheet.more, ROUND_LIMIT_MAX);
  const answered = ready.length;
  const details = replacing.length > 0 || changed.length > 0 || unwritten.length > 0 || postFailure !== null || result !== null;
  return (
    <div className="round" ref={top} tabIndex={-1}>
      <div className="round-intro">
        <p className="round-count">{`${counted(items.length, "item")}, about ${String(sheet.minutes)} min of reading`}</p>
        {notes}
        {sheet.more > 0 && all > items.length && all !== limit && (
          <button
            type="button"
            className="button"
            aria-disabled={round.isFetching}
            onClick={() => {
              if (!round.isFetching) {
                update((current) => ({ ...current, limit: all }));
              }
            }}
          >
            {all === items.length + sheet.more ? `Show all ${String(all)}` : `Show ${String(all)}, the most one sheet holds`}
          </button>
        )}
        <p className="note">
          <Icon name="info" />
          <span>
            Your answers stay in this tab while it is open, across sections; a reload drops them. Nothing is staged until you
            press Stage, and nothing is applied here.
          </span>
        </p>
      </div>
      {droppedNotice}
      {rereadFailure}
      {staged > 0 && <ConfirmOnTerminal root={root} count={staged} />}
      <ol className="round-list" aria-label="Open questions and discrepancies, in round order" aria-busy={round.isPlaceholderData}>
        {items.map((item) => (
          <RoundItemCard
            key={item.id}
            project={project}
            item={item}
            draft={drafts.get(item.id)}
            result={refused.get(item.id)}
            busy={busy || round.isPlaceholderData}
            unstageRefusal={unstageRefusal !== null && unstageRefusal.id === item.id ? unstageRefusal.message : null}
            onDraft={changeDraft}
            onUnstage={onUnstage}
          />
        ))}
      </ol>
      {details && (
        // Out of the sticky bar, so the bar stays short and never covers the control in focus.
        <section ref={atLength} className="round-stage-details" aria-label="The answers to stage" tabIndex={-1}>
          {replacing.length > 0 && (
            <p className="round-stage-note">
              Replaces the choice staged on{" "}
              {replacing.map((item, index) => (
                <span key={item.id}>
                  {index > 0 && (index === replacing.length - 1 ? " and " : ", ")}
                  <span className="mono">{item.id}</span> (<time dateTime={item.staged_at ?? ""}>{item.staged_at}</time>)
                </span>
              ))}
              .
            </p>
          )}
          {changed.length > 0 && (
            <p className="round-stage-note">Left out, changed since you answered: {listed(changed)}. Pick again to include them.</p>
          )}
          {unwritten.length > 0 && <p className="round-stage-note">Left out, nothing written yet: {listed(unwritten)}.</p>}
          {postFailure !== null && (
            <div className="refusal" role="alert">
              <p className="refusal-title">
                <Icon name="alert" />
                <span>
                  {postFailure.status === 0
                    ? "No answer from the daemon: some answers may have been staged. The sheet is read again; your answers are kept."
                    : "The daemon refused the answers; nothing was staged. Your answers are kept."}
                </span>
              </p>
              <p className="verbatim">{postFailure.message}</p>
            </div>
          )}
          {/* A visible copy; screen readers hear it through the page's live region (useAnnounce). */}
          {result !== null && (
            <div className="decision-result">
              <p>{result.text}</p>
              {result.notes.length > 0 && (
                <ul className="note-list" aria-label="Notes from the daemon on the answers">
                  {result.notes.map((note, index) => (
                    <li key={`${String(index)}-${note}`}>{shownSlot(note)}</li>
                  ))}
                </ul>
              )}
            </div>
          )}
        </section>
      )}
      <section className="round-stage" aria-labelledby="round-stage-title" aria-busy={busy}>
        <h2 id="round-stage-title" className="sr-only">
          Stage your answers
        </h2>
        <p className="round-stage-count">
          {posting
            ? "Staging the answers sent; nothing else is sent until they are answered."
            : answered === 0
              ? "No answer picked yet."
              : `${counted(answered, "answer")} ready to stage.`}
        </p>
        {!posting && (postFailure !== null || result !== null) && (
          // The last Stage's outcome in one line, here where it was pressed; at length above the bar.
          <p className="round-stage-outcome">
            <button
              type="button"
              className="link-button"
              onClick={() => {
                atLength.current?.focus();
              }}
            >
              {postFailure !== null
                ? postFailure.status === 0
                  ? "No answer from the daemon: see below"
                  : "Not staged: see why below"
                : `Staged ${String(result?.staged ?? 0)} of ${String(result?.sent ?? 0)}: see the outcome below`}
            </button>
          </p>
        )}
        <button
          type="button"
          className="button button-primary"
          aria-disabled={busy || answered === 0}
          onClick={() => {
            void submit();
          }}
        >
          {posting ? "Staging" : `Stage ${counted(answered, "answer")}`}
        </button>
      </section>
    </div>
  );
}
