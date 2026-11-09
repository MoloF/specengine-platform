import { useId, type KeyboardEvent, type MouseEvent, type ReactNode, type Ref } from "react";
import { apiErrorOf } from "../api/client";
import type { CardQuery } from "../api/queries";
import type { Card, CardEvent, CardFlow, CardNotFound } from "../api/types";
import { sectionHash } from "../app/routes";
import { rollupLook } from "../flow/labels";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { hasModifier } from "../ui/keys";
import { ErrorPanel, Skeleton } from "../ui/states";
import { shownSlot, shownText } from "../ui/text";
import { formatUtc } from "../ui/time";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useAllowed } from "./acting";
import { ActOff } from "./ActOff";
import { isOverdue } from "./due";
import { useTranslate, type Translate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { acceptedLook, byHand, canClear, cancelledLook, columnTitle, eventWords, markLine, markLook, overdueLook, unassigned, type CardMark } from "./labels";

/** The panel's five writes, each a dialog. */
export type CardAction = "edit" | "assign" | "cancel" | "accept" | "clear";

/** Whether a card's read found one, rather than `{id, reason}`. */
export function isCard(answer: Card | CardNotFound): answer is Card {
  return "title" in answer;
}

/** One fact; an absent value `-`, as `spec card show`. */
function Fact({ term, children }: { term: string; children: ReactNode }) {
  return (
    <div className="fact">
      <dt>{term}</dt>
      <dd>{children}</dd>
    </div>
  );
}

function Value({ text }: { text: string | null }) {
  return text === null ? "-" : <bdi>{shownSlot(text)}</bdi>;
}

/** A stored UTC time, as stored to the minute. */
function At({ at }: { at: string }) {
  const t = useTranslate();
  return <time dateTime={at}>{formatUtc(at, t)}</time>;
}

/** A link to another card; the ID as stored. */
function CardLink({ project, id }: { project: string; id: string | null }) {
  return id === null ? "-" : <a href={sectionHash(project, "board", id)}>{shownSlot(id)}</a>;
}

/** A key of an event's payload as text: its string, a list joined; anything else null. */
function payloadWord(payload: unknown, key: string): string | null {
  if (typeof payload !== "object" || payload === null) {
    return null;
  }
  const value = (payload as Record<string, unknown>)[key];
  if (typeof value === "string") {
    return value;
  }
  return Array.isArray(value) && value.every((one) => typeof one === "string") ? value.join(", ") : null;
}

/** What an event says besides its words, by the keys its payload holds: from and to, the column, the fields, the names. */
function eventDetail(payload: unknown, flow: CardFlow | undefined, t: Translate): string {
  const from = payloadWord(payload, "from");
  const to = payloadWord(payload, "to");
  const column = payloadWord(payload, "column");
  const parts: string[] = [];
  if (from !== null && to !== null) {
    parts.push(from === to ? t("board.history.within", { column: columnTitle(flow, to) }) : t("board.history.fromTo", { from: columnTitle(flow, from), to: columnTitle(flow, to) }));
  } else if (column !== null) {
    parts.push(t("board.history.in", { column: columnTitle(flow, column) }));
  }
  for (const [key, word] of [
    ["fields", "board.history.changed"],
    ["assignee", "board.history.assignee"],
    ["reviewer", "board.history.reviewer"],
  ] as const) {
    const value = payloadWord(payload, key);
    if (value !== null) {
      parts.push(t(word, { value }));
    }
  }
  return parts.join(", ");
}

/** One event by `seq`: time, its words, who, from and to; an unknown type raw. */
function HistoryLine({ event, flow }: { event: CardEvent; flow: CardFlow | undefined }) {
  const t = useTranslate();
  const by = payloadWord(event.payload, "as");
  const detail = eventDetail(event.payload, flow, t);
  return (
    <li className="board-history-line">
      <At at={event.at} />
      <span>
        <strong>{eventWords(event.type, t)}</strong>
        {by !== null && fill(t("board.history.by", { by: slot("by") }), { by: <bdi>{shownSlot(by)}</bdi> })}
        {detail !== "" && <span className="muted"> {shownSlot(detail)}</span>}
      </span>
    </li>
  );
}

function Part({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="board-panel-part" aria-label={title}>
      <h3 className="card-section-title">{title}</h3>
      {children}
    </section>
  );
}

/** A closed card has no actions: who closed it, when and what they said. */
function Closure({ card }: { card: Card }) {
  const t = useTranslate();
  return (
    <>
      {card.acceptance !== null && (
        <Part title={t("board.panel.acceptance")}>
          <p>{fill(t("board.panel.byAt", { by: slot("by"), at: slot("at") }), { by: <bdi>{shownSlot(card.acceptance.by)}</bdi>, at: <At at={card.acceptance.at} /> })}</p>
          {card.acceptance.verdict === "departs" && (
            <p>
              {card.acceptance.discrepancy === null
                ? fill(t("board.panel.departs", { from: slot("from") }), { from: <bdi>{shownSlot(card.acceptance.departs.join(", "))}</bdi> })
                : fill(t("board.panel.departsWith", { from: slot("from"), discrepancy: slot("discrepancy") }), {
                    from: <bdi>{shownSlot(card.acceptance.departs.join(", "))}</bdi>,
                    discrepancy: <bdi>{shownSlot(card.acceptance.discrepancy)}</bdi>,
                  })}
            </p>
          )}
          {card.acceptance.note !== null && <p className="board-text">{shownText(card.acceptance.note)}</p>}
        </Part>
      )}
      {card.cancellation !== null && (
        <Part title={t("board.panel.cancellation")}>
          <p>{fill(t("board.panel.byAt", { by: slot("by"), at: slot("at") }), { by: <bdi>{shownSlot(card.cancellation.by)}</bdi>, at: <At at={card.cancellation.at} /> })}</p>
          <p className="board-text">{shownText(card.cancellation.reason)}</p>
        </Part>
      )}
    </>
  );
}

/**
 * What a read says of the card against its brief, closed or not: a suspect card's Clear stands here,
 * an accepted card's too; idle, as a hub's `allowed` says, it stays where it is and does nothing.
 */
function BriefMark({ card, mark, clears, onAct }: { card: Card; mark: CardMark; clears: boolean; onAct: (action: CardAction, card: Card, trigger: HTMLElement) => void }) {
  const t = useTranslate();
  const look = markLook(mark, t);
  if (look === null) {
    return null;
  }
  return (
    <Part title={t("board.panel.brief")}>
      <p className="board-panel-state">
        <Badge look={look} />
        <span>
          <bdi>{shownSlot(markLine(mark, t))}</bdi>
        </span>
      </p>
      {canClear(mark) && (
        <div className="board-panel-actions">
          <button
            type="button"
            className="button"
            aria-disabled={!clears}
            onClick={(event) => {
              if (clears) {
                onAct("clear", card, event.currentTarget);
              }
            }}
          >
            {t("board.panel.clear")}
          </button>
        </div>
      )}
      {canClear(mark) && !clears && <ActOff act="clear" />}
    </Part>
  );
}

/** A target's word, when the trace read holds it; a target gone has none. */
function TargetState({ rollup }: { rollup: string | undefined }) {
  const t = useTranslate();
  return rollup === undefined ? null : <Badge look={rollupLook(rollup, t)} />;
}

function Actions({ card, flow, accepts, onAct }: { card: Card; flow: CardFlow | undefined; accepts: boolean; onAct: (action: CardAction, card: Card, trigger: HTMLElement) => void }) {
  const t = useTranslate();
  const inAcceptColumn = card.column === flow?.accept.from;
  function act(action: CardAction) {
    return (event: MouseEvent<HTMLButtonElement>) => {
      onAct(action, card, event.currentTarget);
    };
  }
  return (
    <>
      <div className="board-panel-actions" role="group" aria-label={t("board.panel.actions")}>
        {inAcceptColumn && (
          <button type="button" className="button button-primary" aria-disabled={!accepts} onClick={accepts ? act("accept") : undefined}>
            {t("board.panel.accept")}
          </button>
        )}
        <button type="button" className="button" onClick={act("edit")}>
          {t("board.panel.edit")}
        </button>
        <button type="button" className="button" onClick={act("assign")}>
          {t("board.panel.assign")}
        </button>
        <button type="button" className="button button-danger" onClick={act("cancel")}>
          {t("board.panel.cancel")}
        </button>
      </div>
      {inAcceptColumn && !accepts && <ActOff act="accept" />}
    </>
  );
}

function Details({
  project,
  card,
  flow,
  today,
  mark,
  rollups,
  markFailure,
  stateFailure,
  onAct,
}: {
  project: string;
  card: Card;
  flow: CardFlow | undefined;
  today: string;
  mark: CardMark | null;
  rollups: ReadonlyMap<string, string>;
  markFailure: string | null;
  stateFailure: string | null;
  onAct: (action: CardAction, card: Card, trigger: HTMLElement) => void;
}) {
  const t = useTranslate();
  const allows = useAllowed(project);
  const accepted = card.acceptance !== null;
  const cancelled = card.cancellation !== null;
  const history = [...(card.history ?? [])].sort((a, b) => a.seq - b.seq);
  return (
    <>
      <p className="board-panel-state">
        <span>{fill(t("board.panel.in", { column: slot("column") }), { column: <bdi>{columnTitle(flow, card.column)}</bdi> })}</span>
        {accepted && <Badge look={acceptedLook(t)} />}
        {cancelled && <Badge look={cancelledLook(t)} />}
        {isOverdue({ due: card.due, accepted, cancelled }, today) && <Badge look={overdueLook(t)} />}
      </p>
      {card.notes.length > 0 && (
        <ul className="note-list" aria-label={t("board.panel.notes")}>
          {card.notes.map((note, index) => (
            <li key={`${String(index)}-${note}`} lang="en">
              {note}
            </li>
          ))}
        </ul>
      )}
      {mark !== null && <BriefMark card={card} mark={mark} clears={allows("clear")} onAct={onAct} />}
      {markFailure !== null && (
        // Not known, never read as clean: no mark is shown for a read that failed.
        <p className="note">
          <Icon name="alert" />
          <span>{fill(t("board.panel.markFailed", { message: slot("message") }), { message: <span lang="en">{markFailure}</span> })}</span>
        </p>
      )}
      {stateFailure !== null && (
        <p className="note">
          <Icon name="alert" />
          <span>{fill(t("board.panel.stateFailed", { message: slot("message") }), { message: <span lang="en">{stateFailure}</span> })}</span>
        </p>
      )}
      {accepted || cancelled ? <Closure card={card} /> : <Actions card={card} flow={flow} accepts={allows("accept")} onAct={onAct} />}
      {card.goal !== null && (
        <Part title={t("board.panel.goal")}>
          <p className="board-text">{shownText(card.goal)}</p>
        </Part>
      )}
      {card.criteria.length > 0 && (
        <Part title={t("board.panel.criteria")}>
          <ul className="board-panel-list">
            {card.criteria.map((criterion, index) => (
              <li key={`${String(index)}-${criterion}`} className="board-text">
                {shownText(criterion)}
              </li>
            ))}
          </ul>
        </Part>
      )}
      <Part title={t("board.panel.source")}>
        {card.targets.length === 0 ? (
          <p className="muted">{byHand(t)}</p>
        ) : (
          <ul className="board-panel-list">
            {card.targets.map((target) => (
              <li key={target} className="board-panel-target">
                <a href={sectionHash(project, "tree", target)}>
                  <bdi className="mono">{shownSlot(target)}</bdi>
                </a>
                <TargetState rollup={rollups.get(target)} />
              </li>
            ))}
          </ul>
        )}
      </Part>
      <Part title={t("board.panel.details")}>
        <dl className="facts">
          <Fact term={t("board.panel.discipline")}>
            <Value text={card.discipline} />
          </Fact>
          <Fact term={t("board.panel.estimate")}>
            <Value text={card.estimate} />
          </Fact>
          <Fact term={t("board.panel.due")}>
            <Value text={card.due} />
          </Fact>
          <Fact term={t("board.panel.assignee")}>{card.assignee === null ? unassigned(t) : <Value text={card.assignee} />}</Fact>
          <Fact term={t("board.panel.reviewer")}>
            <Value text={card.reviewer} />
          </Fact>
          <Fact term={t("board.panel.parent")}>
            <CardLink project={project} id={card.parent} />
          </Fact>
          <Fact term={t("board.panel.corrects")}>
            <CardLink project={project} id={card.corrects} />
          </Fact>
          <Fact term={t("board.panel.createdBy")}>
            <Value text={card.created_by} />
          </Fact>
          <Fact term={t("board.panel.createdAt")}>
            <At at={card.created_at} />
          </Fact>
        </dl>
      </Part>
      <Part title={t("board.panel.evidence")}>
        {card.evidence.length === 0 ? (
          <p className="muted">{t("board.panel.noneYet")}</p>
        ) : (
          <ul className="board-panel-list">
            {card.evidence.map((entry, index) => (
              <li key={`${String(index)}-${entry.at}`} className="board-evidence">
                <span className="plain-tag">{shownSlot(entry.kind)}</span>
                {/* A reference as stored: a url is text here, never a link. */}
                <span className="mono board-text">{shownSlot(entry.value)}</span>
                <span className="muted">
                  <bdi>{shownSlot(entry.by)}</bdi>, <At at={entry.at} />
                </span>
              </li>
            ))}
          </ul>
        )}
      </Part>
      <Part title={t("board.panel.history")}>
        {history.length === 0 ? (
          <p className="muted">{t("board.panel.noneYet")}</p>
        ) : (
          <ol className="board-panel-list">
            {history.map((event) => (
              <HistoryLine key={event.seq} event={event} flow={flow} />
            ))}
          </ol>
        )}
      </Part>
    </>
  );
}

/**
 * The panel beside the board, routed by ID (docs/features/board-ui.md "Panel"): the card with its
 * history, text as stored, its actions; an accepted or cancelled card has none and shows why. Esc
 * or Close puts focus back on the card.
 */
export function CardPanel({
  project,
  id,
  query,
  flowOf,
  today,
  mark,
  rollups,
  markFailure,
  stateFailure,
  headingRef,
  onAct,
  onClose,
}: {
  project: string;
  id: string;
  query: CardQuery;
  flowOf: (name: string) => CardFlow | undefined;
  today: string;
  mark: CardMark | null;
  /** Each target's rollup, as the trace read has it. */
  rollups: ReadonlyMap<string, string>;
  /** Why the list, which says the mark, could not be read: no mark is shown. */
  markFailure: string | null;
  /** Why the trace, which says the targets' states, could not be read. */
  stateFailure: string | null;
  headingRef: Ref<HTMLHeadingElement>;
  onAct: (action: CardAction, card: Card, trigger: HTMLElement) => void;
  onClose: () => void;
}) {
  const t = useTranslate();
  const titleId = useId();
  const failure = useRetainedFailure(query.error, query.isFetching);
  const answer = query.data;
  const card = answer !== undefined && isCard(answer) ? answer : undefined;

  function onKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Escape" && !hasModifier(event)) {
      event.preventDefault();
      event.stopPropagation();
      onClose();
    }
  }

  let body;
  if (answer === undefined) {
    body =
      failure === null ? (
        <div aria-busy="true">
          <Skeleton label={t("board.panel.loading", { id })} lines={5} />
        </div>
      ) : (
        <ErrorPanel
          title={t("board.panel.failed", { id })}
          message={apiErrorOf(failure).message}
          retrying={query.isFetching}
          attempt={query.errorUpdateCount}
          onRetry={() => {
            void query.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else if (card === undefined) {
    body = (
      <div className="empty-state">
        <p className="verbatim" lang="en">
          {isCard(answer) ? "" : answer.reason}
        </p>
        <p>{fill(t("board.panel.next", { link: slot("link") }), { link: <a href={sectionHash(project, "board")}>{t("board.panel.back")}</a> })}</p>
      </div>
    );
  } else {
    body = <Details project={project} card={card} flow={flowOf(card.flow)} today={today} mark={mark} rollups={rollups} markFailure={markFailure} stateFailure={stateFailure} onAct={onAct} />;
  }

  return (
    <aside className="board-panel" aria-labelledby={titleId} onKeyDown={onKeyDown}>
      <header className="board-panel-head">
        <p className="mono board-card-id">{id}</p>
        <button type="button" className="button button-quiet" onClick={onClose}>
          <Icon name="rejected" />
          <span>{fill(t("board.panel.close", { what: slot("what") }), { what: <span className="sr-only">{t("board.panel.closeWhat")}</span> })}</span>
        </button>
        <h2 id={titleId} ref={headingRef} tabIndex={-1} className="board-panel-title">
          <bdi>{card === undefined ? id : shownSlot(card.title)}</bdi>
        </h2>
      </header>
      {answer !== undefined && query.error !== null && (
        <p className="note" role="alert">
          <Icon name="alert" />
          <span>{fill(t("board.panel.failedAgain", { id, message: slot("message") }), { message: <span lang="en">{apiErrorOf(query.error).message}</span> })}</span>
        </p>
      )}
      {body}
    </aside>
  );
}
