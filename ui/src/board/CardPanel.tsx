import { type MouseEvent, type ReactNode } from "react";
import { apiErrorOf } from "../api/client";
import type { CardQuery } from "../api/queries";
import type { Card, CardEvent, CardFlow, CardNotFound } from "../api/types";
import { cardPageHash, sectionHash } from "../app/routes";
import { rollupLook } from "../flow/labels";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { shownSlot, shownText } from "../ui/text";
import { formatUtc } from "../ui/time";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { actingName, useActing, useAllowed } from "./acting";
import { ActOff } from "./ActOff";
import { isOverdue } from "./due";
import { useTranslate, type Translate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import {
  acceptedLook,
  byHand,
  canClear,
  cancelledLook,
  columnTitle,
  eventWords,
  handOffLook,
  markLine,
  markLook,
  needsLine,
  overdueLook,
  returnedLook,
  unassigned,
  type CardMark,
} from "./labels";
import { returnReasonOf, type Rollup } from "./rollup";
import { CardTasks, ownNotes } from "./CardTasks";

// A card's body (`docs/canon/cards.md` "Documents and events"): every field, its mark and Clear,
// Accept's verdict, its history with each move's note, its links, its hand-offs, its linked `T-`
// tasks (`docs/canon/card-tasks.md` "The rule"; their notes stand on their rows) and, standing in its
// flow's return column, why it came back. The Board's modal and the card's page show it alike; the
// acts open their dialogs over it (`docs/canon/card-flows.md` "The rule"). Nothing here judges a
// column: the return column and the accept column are the flow's, looked up.

/** The card's acts, each a dialog but Move, which opens the Move menu. */
export type CardAction = "move" | "handoff" | "return" | "edit" | "assign" | "cancel" | "accept" | "clear";

/** Whether a card's read found one, rather than `{id, reason}`. */
export function isCard(answer: Card | CardNotFound): answer is Card {
  return "title" in answer;
}

/** How a link to another card goes: to its page or to it on the board, and what a plain click does instead (the modal replaces its card). */
export interface CardLinks {
  project: string;
  page: boolean;
  onFollow: (id: string) => void;
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

/** A plain click with the primary button: a modified one, or another button, is the browser's. */
function isPlainClick(event: MouseEvent<HTMLElement>): boolean {
  return event.button === 0 && !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
}

/** A link to another card; the ID as stored. A plain click follows it in place, any other is the browser's. */
function CardLink({ id, links }: { id: string | null; links: CardLinks }) {
  if (id === null) {
    return "-";
  }
  const onClick = (event: MouseEvent<HTMLAnchorElement>) => {
    if (isPlainClick(event)) {
      event.preventDefault();
      links.onFollow(id);
    }
  };
  return links.page ? (
    <a className="mono" href={cardPageHash(links.project, id)} onClick={onClick}>
      {shownSlot(id)}
    </a>
  ) : (
    <a className="mono" href={sectionHash(links.project, "board", id)} onClick={onClick}>
      {shownSlot(id)}
    </a>
  );
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

/** One event by `seq`: time, its words, who, from and to, and a move's note as written; an unknown type raw. */
function HistoryLine({ event, flow }: { event: CardEvent; flow: CardFlow | undefined }) {
  const t = useTranslate();
  const by = payloadWord(event.payload, "as");
  const detail = eventDetail(event.payload, flow, t);
  const note = payloadWord(event.payload, "note");
  return (
    <li className="board-history-line">
      <At at={event.at} />
      <span>
        <strong>{eventWords(event.type, t)}</strong>
        {by !== null && fill(t("board.history.by", { by: slot("by") }), { by: <bdi>{shownSlot(by)}</bdi> })}
        {detail !== "" && <span className="muted"> {shownSlot(detail)}</span>}
      </span>
      {note !== null && <span className="board-text">{fill(t("board.history.why", { note: slot("note") }), { note: <bdi>{shownText(note)}</bdi> })}</span>}
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

/**
 * An open card's acts: Move, Hand off, Return when its flow names a return column it is not in and
 * `allowed` holds the move there, Edit, Assign, Cancel card, and Accept in `accept.from` alone. A
 * `by` the acting name lacks reads "needs <names>", as in the Move menu; the daemon judges either way.
 */
function Actions({ card, flow, accepts, onAct }: { card: Card; flow: CardFlow | undefined; accepts: boolean; onAct: (action: CardAction, card: Card, trigger: HTMLElement) => void }) {
  const t = useTranslate();
  const [acting] = useActing();
  const inAcceptColumn = card.column === flow?.accept.from;
  const back = flow?.return ?? null;
  const returning = back === null || card.column === back ? undefined : card.allowed.find((move) => move.to === back);
  const name = actingName(acting);
  const needs = returning === undefined || returning.by.length === 0 || (name !== null && returning.by.includes(name)) ? null : needsLine(returning.by, t);
  function act(action: CardAction) {
    return (event: MouseEvent<HTMLButtonElement>) => {
      onAct(action, card, event.currentTarget);
    };
  }
  return (
    <>
      <div className="board-panel-actions" role="group" aria-label={t("board.panel.actions")}>
        <button type="button" className="button" aria-haspopup="menu" onClick={act("move")}>
          <Icon name="cardMove" />
          <span>{t("board.panel.move")}</span>
        </button>
        <button type="button" className="button" onClick={act("handoff")}>
          <Icon name="cardHandOff" />
          <span>{t("board.handoff.action")}</span>
        </button>
        {returning !== undefined && (
          <button type="button" className="button" onClick={act("return")}>
            <Icon name="cardReturned" />
            <span>{t("board.return.action")}</span>
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
        {inAcceptColumn && (
          <button type="button" className="button button-primary" aria-disabled={!accepts} onClick={accepts ? act("accept") : undefined}>
            {t("board.panel.accept")}
          </button>
        )}
      </div>
      {needs !== null && (
        <p className="note">
          <Icon name="info" />
          <span>{fill(t("board.panel.returnNeeds", { needs: slot("needs") }), { needs: <bdi>{needs}</bdi> })}</span>
        </p>
      )}
      {inAcceptColumn && !accepts && <ActOff act="accept" />}
    </>
  );
}

/** Why a card standing in its flow's return column came back: the latest reason its history keeps, or that it keeps none now. */
function ReturnReason({ card, column }: { card: Card; column: string }) {
  const t = useTranslate();
  const reason = returnReasonOf(card.history ?? [], column);
  return (
    <Part title={t("board.return.badge")}>
      <p className="board-panel-state">
        <Badge look={returnedLook(t)} />
      </p>
      {reason === null ? (
        <p className="note">
          <Icon name="info" />
          <span>{t("board.modal.reasonGone")}</span>
        </p>
      ) : (
        <>
          <p>{fill(t("board.return.latest", { by: slot("by"), at: slot("at") }), { by: <bdi>{reason.by === null ? "-" : shownSlot(reason.by)}</bdi>, at: <At at={reason.at} /> })}</p>
          <p className="board-text">{shownText(reason.note)}</p>
        </>
      )}
    </Part>
  );
}

/** The card's children: ID (a link), title, `<flow>/<column>`, assignee and word; a child of a flow not served has no word. */
function HandOffs({ rollup, links }: { rollup: Rollup; links: CardLinks }) {
  const t = useTranslate();
  return (
    <Part title={t("board.modal.handOffs")}>
      {rollup.children.length === 0 ? (
        <p className="muted">{t("board.panel.noneYet")}</p>
      ) : (
        <ul className="board-panel-list">
          {rollup.children.map(({ row, word }) => (
            <li key={row.id} className="board-handoff">
              <CardLink id={row.id} links={links} />
              <bdi className="board-handoff-title">{shownSlot(row.title)}</bdi>
              <span className="mono muted">
                {shownSlot(row.flow)}/{shownSlot(row.column)}
              </span>
              <span className={row.assignee === null ? "muted" : undefined}>
                <bdi>{row.assignee === null ? unassigned(t) : shownSlot(row.assignee)}</bdi>
              </span>
              {word !== null && <Badge look={handOffLook(word, t)} />}
            </li>
          ))}
        </ul>
      )}
    </Part>
  );
}

function Details({
  project,
  card,
  flow,
  today,
  mark,
  rollups,
  handOffs,
  markFailure,
  stateFailure,
  links,
  onAct,
}: {
  project: string;
  card: Card;
  flow: CardFlow | undefined;
  today: string;
  mark: CardMark | null;
  rollups: ReadonlyMap<string, string>;
  handOffs: Rollup;
  markFailure: string | null;
  stateFailure: string | null;
  links: CardLinks;
  onAct: (action: CardAction, card: Card, trigger: HTMLElement) => void;
}) {
  const t = useTranslate();
  const allows = useAllowed(project);
  const accepted = card.acceptance !== null;
  const cancelled = card.cancellation !== null;
  const back = flow?.return ?? null;
  const history = [...(card.history ?? [])].sort((a, b) => a.seq - b.seq);
  const notes = ownNotes(card);
  return (
    <>
      <p className="board-panel-state">
        <span>{fill(t("board.panel.in", { column: slot("column") }), { column: <bdi>{columnTitle(flow, card.column)}</bdi> })}</span>
        {accepted && <Badge look={acceptedLook(t)} />}
        {cancelled && <Badge look={cancelledLook(t)} />}
        {isOverdue({ due: card.due, accepted, cancelled }, today) && <Badge look={overdueLook(t)} />}
      </p>
      {notes.length > 0 && (
        <ul className="note-list" aria-label={t("board.panel.notes")}>
          {notes.map((note, index) => (
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
      {back !== null && card.column === back && !cancelled && <ReturnReason card={card} column={back} />}
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
      <HandOffs rollup={handOffs} links={links} />
      <Part title={t("board.tasks.title")}>
        <CardTasks project={project} card={card} />
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
            <CardLink id={card.parent} links={links} />
          </Fact>
          <Fact term={t("board.panel.corrects")}>
            <CardLink id={card.corrects} links={links} />
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
 * The card's body as its read stands: a skeleton while out, the daemon's words and Retry when it
 * failed, its `{id, reason}` verbatim when there is no such card, else every part. A read failing
 * again keeps what it last read, saying so.
 */
export function CardBody({
  project,
  id,
  query,
  flowOf,
  today,
  mark,
  rollups,
  handOffs,
  markFailure,
  stateFailure,
  links,
  onAct,
}: {
  project: string;
  id: string;
  query: CardQuery;
  flowOf: (name: string) => CardFlow | undefined;
  today: string;
  mark: CardMark | null;
  /** Each target's rollup, as the trace read has it. */
  rollups: ReadonlyMap<string, string>;
  /** The card's children in the list read. */
  handOffs: Rollup;
  /** Why the list, which says the mark, could not be read: no mark is shown. */
  markFailure: string | null;
  /** Why the trace, which says the targets' states, could not be read. */
  stateFailure: string | null;
  links: CardLinks;
  onAct: (action: CardAction, card: Card, trigger: HTMLElement) => void;
}) {
  const t = useTranslate();
  const failure = useRetainedFailure(query.error, query.isFetching);
  const answer = query.data;
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
  } else if (!isCard(answer)) {
    body = (
      <div className="empty-state">
        <p className="verbatim" lang="en">
          {answer.reason}
        </p>
        <p>{fill(t("board.panel.next", { link: slot("link") }), { link: <a href={sectionHash(project, "board")}>{t("board.panel.back")}</a> })}</p>
      </div>
    );
  } else {
    body = (
      <Details
        project={project}
        card={answer}
        flow={flowOf(answer.flow)}
        today={today}
        mark={mark}
        rollups={rollups}
        handOffs={handOffs}
        markFailure={markFailure}
        stateFailure={stateFailure}
        links={links}
        onAct={onAct}
      />
    );
  }
  return (
    <>
      {answer !== undefined && query.error !== null && (
        <p className="note" role="alert">
          <Icon name="alert" />
          <span>{fill(t("board.panel.failedAgain", { id, message: slot("message") }), { message: <span lang="en">{apiErrorOf(query.error).message}</span> })}</span>
        </p>
      )}
      {body}
    </>
  );
}

/** The card's title once read, else null: the modal's heading and the page's `h1` fall back to its ID. */
export function titleOf(query: CardQuery): string | null {
  return query.data !== undefined && isCard(query.data) ? query.data.title : null;
}
