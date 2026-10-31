import { useId, type KeyboardEvent, type MouseEvent, type ReactNode, type Ref } from "react";
import { apiErrorOf } from "../api/client";
import type { CardQuery } from "../api/queries";
import type { Card, CardEvent, CardFlow, CardNotFound } from "../api/types";
import { sectionHash } from "../app/routes";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { hasModifier } from "../ui/keys";
import { ErrorPanel, Skeleton } from "../ui/states";
import { shownSlot, shownText } from "../ui/text";
import { formatUtc } from "../ui/time";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { isOverdue } from "./due";
import { ACCEPTED, BY_HAND, CANCELLED, columnTitle, eventWords, OVERDUE, UNASSIGNED } from "./labels";

/** The panel's four writes, each a dialog. */
export type CardAction = "edit" | "assign" | "cancel" | "accept";

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
  return <time dateTime={at}>{formatUtc(at)}</time>;
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
function eventDetail(payload: unknown, flow: CardFlow | undefined): string {
  const from = payloadWord(payload, "from");
  const to = payloadWord(payload, "to");
  const column = payloadWord(payload, "column");
  const parts: string[] = [];
  if (from !== null && to !== null) {
    parts.push(from === to ? `within ${columnTitle(flow, to)}` : `from ${columnTitle(flow, from)} to ${columnTitle(flow, to)}`);
  } else if (column !== null) {
    parts.push(`in ${columnTitle(flow, column)}`);
  }
  for (const [key, word] of [
    ["fields", "changed"],
    ["assignee", "assignee"],
    ["reviewer", "reviewer"],
  ] as const) {
    const value = payloadWord(payload, key);
    if (value !== null) {
      parts.push(`${word} ${value}`);
    }
  }
  return parts.join(", ");
}

/** One event by `seq`: time, its words, who, from and to; an unknown type raw. */
function HistoryLine({ event, flow }: { event: CardEvent; flow: CardFlow | undefined }) {
  const by = payloadWord(event.payload, "as");
  const detail = eventDetail(event.payload, flow);
  return (
    <li className="board-history-line">
      <At at={event.at} />
      <span>
        <strong>{eventWords(event.type)}</strong>
        {by !== null && (
          <>
            {" "}
            by <bdi>{shownSlot(by)}</bdi>
          </>
        )}
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
  return (
    <>
      {card.acceptance !== null && (
        <Part title="Acceptance">
          <p>
            By <bdi>{shownSlot(card.acceptance.by)}</bdi>, <At at={card.acceptance.at} />
          </p>
          {card.acceptance.note !== null && <p className="board-text">{shownText(card.acceptance.note)}</p>}
        </Part>
      )}
      {card.cancellation !== null && (
        <Part title="Cancellation">
          <p>
            By <bdi>{shownSlot(card.cancellation.by)}</bdi>, <At at={card.cancellation.at} />
          </p>
          <p className="board-text">{shownText(card.cancellation.reason)}</p>
        </Part>
      )}
    </>
  );
}

function Actions({ card, flow, onAct }: { card: Card; flow: CardFlow | undefined; onAct: (action: CardAction, card: Card, trigger: HTMLElement) => void }) {
  function act(action: CardAction) {
    return (event: MouseEvent<HTMLButtonElement>) => {
      onAct(action, card, event.currentTarget);
    };
  }
  return (
    <div className="board-panel-actions" role="group" aria-label="Actions">
      {card.column === flow?.accept.from && (
        <button type="button" className="button button-primary" onClick={act("accept")}>
          Accept
        </button>
      )}
      <button type="button" className="button" onClick={act("edit")}>
        Edit
      </button>
      <button type="button" className="button" onClick={act("assign")}>
        Assign
      </button>
      <button type="button" className="button button-danger" onClick={act("cancel")}>
        Cancel card
      </button>
    </div>
  );
}

function Details({
  project,
  card,
  flow,
  today,
  onAct,
}: {
  project: string;
  card: Card;
  flow: CardFlow | undefined;
  today: string;
  onAct: (action: CardAction, card: Card, trigger: HTMLElement) => void;
}) {
  const accepted = card.acceptance !== null;
  const cancelled = card.cancellation !== null;
  const history = [...(card.history ?? [])].sort((a, b) => a.seq - b.seq);
  return (
    <>
      <p className="board-panel-state">
        <span>
          In <bdi>{columnTitle(flow, card.column)}</bdi>
        </span>
        {accepted && <Badge look={ACCEPTED} />}
        {cancelled && <Badge look={CANCELLED} />}
        {isOverdue({ due: card.due, accepted, cancelled }, today) && <Badge look={OVERDUE} />}
      </p>
      {card.notes.length > 0 && (
        <ul className="note-list" aria-label="Notes from the daemon on the card">
          {card.notes.map((note, index) => (
            <li key={`${String(index)}-${note}`}>{note}</li>
          ))}
        </ul>
      )}
      {accepted || cancelled ? <Closure card={card} /> : <Actions card={card} flow={flow} onAct={onAct} />}
      {card.goal !== null && (
        <Part title="Goal">
          <p className="board-text">{shownText(card.goal)}</p>
        </Part>
      )}
      {card.criteria.length > 0 && (
        <Part title="Criteria">
          <ul className="board-panel-list">
            {card.criteria.map((criterion, index) => (
              <li key={`${String(index)}-${criterion}`} className="board-text">
                {shownText(criterion)}
              </li>
            ))}
          </ul>
        </Part>
      )}
      <Part title="Source">
        {card.targets.length === 0 ? (
          <p className="muted">{BY_HAND}</p>
        ) : (
          <ul className="board-panel-list">
            {card.targets.map((target) => (
              <li key={target}>
                <a href={sectionHash(project, "tree", target)}>
                  <bdi className="mono">{shownSlot(target)}</bdi>
                </a>
              </li>
            ))}
          </ul>
        )}
      </Part>
      <Part title="Details">
        <dl className="facts">
          <Fact term="Discipline">
            <Value text={card.discipline} />
          </Fact>
          <Fact term="Estimate">
            <Value text={card.estimate} />
          </Fact>
          <Fact term="Due">
            <Value text={card.due} />
          </Fact>
          <Fact term="Assignee">{card.assignee === null ? UNASSIGNED : <Value text={card.assignee} />}</Fact>
          <Fact term="Reviewer">
            <Value text={card.reviewer} />
          </Fact>
          <Fact term="Parent">
            <CardLink project={project} id={card.parent} />
          </Fact>
          <Fact term="Corrects">
            <CardLink project={project} id={card.corrects} />
          </Fact>
          <Fact term="Created by">
            <Value text={card.created_by} />
          </Fact>
          <Fact term="Created at">
            <At at={card.created_at} />
          </Fact>
        </dl>
      </Part>
      <Part title="Evidence">
        {card.evidence.length === 0 ? (
          <p className="muted">None yet.</p>
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
      <Part title="History">
        {history.length === 0 ? (
          <p className="muted">None yet.</p>
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
  headingRef,
  onAct,
  onClose,
}: {
  project: string;
  id: string;
  query: CardQuery;
  flowOf: (name: string) => CardFlow | undefined;
  today: string;
  headingRef: Ref<HTMLHeadingElement>;
  onAct: (action: CardAction, card: Card, trigger: HTMLElement) => void;
  onClose: () => void;
}) {
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
          <Skeleton label={`Loading ${id}`} lines={5} />
        </div>
      ) : (
        <ErrorPanel
          title={`${id} could not be read`}
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
        <p className="verbatim">{isCard(answer) ? "" : answer.reason}</p>
        <p>
          Next: pick a card on the board. <a href={sectionHash(project, "board")}>Back to the board</a>
        </p>
      </div>
    );
  } else {
    body = <Details project={project} card={card} flow={flowOf(card.flow)} today={today} onAct={onAct} />;
  }

  return (
    <aside className="board-panel" aria-labelledby={titleId} onKeyDown={onKeyDown}>
      <header className="board-panel-head">
        <p className="mono board-card-id">{id}</p>
        <button type="button" className="button button-quiet" onClick={onClose}>
          <Icon name="rejected" />
          <span>
            Close<span className="sr-only"> the panel</span>
          </span>
        </button>
        <h2 id={titleId} ref={headingRef} tabIndex={-1} className="board-panel-title">
          <bdi>{card === undefined ? id : shownSlot(card.title)}</bdi>
        </h2>
      </header>
      {answer !== undefined && query.error !== null && (
        <p className="note" role="alert">
          <Icon name="alert" />
          <span>
            {id} could not be read again: {apiErrorOf(query.error).message}
          </span>
        </p>
      )}
      {body}
    </aside>
  );
}
