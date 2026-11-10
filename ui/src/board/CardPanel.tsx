import { useId, type MouseEvent, type ReactNode } from "react";
import { apiErrorOf } from "../api/client";
import { useProjects, type CardQuery } from "../api/queries";
import type { Card, CardEvent, CardFlow, CardNotFound } from "../api/types";
import { RegionBoundary } from "../app/RegionBoundary";
import { DocLink } from "../doc/DocLink";
import { cardPageHash, homeHash, sectionHash } from "../app/routes";
import { rollupLook } from "../flow/labels";
import { Badge } from "../ui/Badge";
import { Breadcrumbs, type Crumb } from "../ui/Breadcrumbs";
import { Button } from "../ui/Button";
import { Disclosure } from "../ui/Disclosure";
import { Icon } from "../ui/Icon";
import { IssueView } from "../ui/IssueView";
import { Lozenge } from "../ui/Lozenge";
import { PropertyList, type Property } from "../ui/PropertyList";
import { ErrorPanel, Skeleton } from "../ui/states";
import { shownSlot, shownText } from "../ui/text";
import { formatUtc } from "../ui/time";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { actingName, useActing, useAllowed } from "./acting";
import { ActOff } from "./ActOff";
import { statusLook } from "./category";
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

// A card as a work item (`docs/canon/cards.md` "Documents and events";
// docs/features/design-foundation.md "The work item: the task modal and page"): one IssueView for
// the Board's modal and the card's page alike. The head holds the breadcrumbs, the caller's tools and
// title, and the acts (Accept, primary, in `accept.from` alone; Hand off; Return…; Edit; Assign; Cancel
// card, subtle and last). The main column holds the card's blocks (its mark and Clear, why it came
// back, Accept's verdict, the cancellation), Goal and Criteria at the document's size, Source,
// Hand-offs, Evidence and History, each as before. The side column holds the status (its column's
// lozenge and the Move menu), the people's fields with the empty ones folded, and Development, the
// linked `T-` tasks (`docs/canon/card-tasks.md` "The rule"), folded. Nothing here judges a column: the
// return column and the accept column are the flow's, looked up (`docs/canon/card-flows.md` "The rule").

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

/** What the work item reads and does, the Board's modal and the card's page alike. */
export interface CardViewProps {
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
}

function Value({ text }: { text: string }) {
  return <bdi>{shownSlot(text)}</bdi>;
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
function CardLink({ id, links }: { id: string; links: CardLinks }) {
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

/** A part of the main column: a region named by its heading. */
function Part({ title, className, children }: { title: string; className?: string; children: ReactNode }) {
  return (
    <section className={className === undefined ? "board-panel-part" : `board-panel-part ${className}`} aria-label={title}>
      <h3 className="issue-section-title">{title}</h3>
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
          <Button
            aria-disabled={!clears}
            onClick={(event) => {
              if (clears) {
                onAct("clear", card, event.currentTarget);
              }
            }}
          >
            {t("board.panel.clear")}
          </Button>
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
 * An open card's acts, the head's, in this order: Accept, the one primary, in `accept.from` alone and
 * idle where a hub's `allowed` omits it; Hand off; Return when its flow names a return column it is
 * not in and `allowed` holds the move there; Edit; Assign; Cancel card, subtle and last. Move stands
 * in the side column's Status. A `by` the acting name lacks reads "needs <names>", as in the Move menu;
 * the daemon judges either way.
 */
function HeadActs({ card, flow, accepts, onAct }: { card: Card; flow: CardFlow | undefined; accepts: boolean; onAct: (action: CardAction, card: Card, trigger: HTMLElement) => void }) {
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
        {inAcceptColumn && (
          <Button appearance="primary" aria-disabled={!accepts} onClick={accepts ? act("accept") : undefined}>
            {t("board.panel.accept")}
          </Button>
        )}
        <Button icon="cardHandOff" onClick={act("handoff")}>
          <span>{t("board.handoff.action")}</span>
        </Button>
        {returning !== undefined && (
          <Button icon="cardReturned" onClick={act("return")}>
            <span>{t("board.return.action")}</span>
          </Button>
        )}
        <Button onClick={act("edit")}>{t("board.panel.edit")}</Button>
        <Button onClick={act("assign")}>{t("board.panel.assign")}</Button>
        <Button appearance="subtle" onClick={act("cancel")}>
          {t("board.panel.cancel")}
        </Button>
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

/** The card's children: ID (a link), title, the flow's name, its status lozenge, assignee and word; a child of a flow not served is stranded and has no word. */
function HandOffs({ rollup, links, flowOf }: { rollup: Rollup; links: CardLinks; flowOf: (name: string) => CardFlow | undefined }) {
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
              <bdi className="muted">{shownSlot(row.flow)}</bdi>
              <Lozenge {...statusLook(flowOf(row.flow), row.column)} />
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

/** The main column of a card read: its blocks, then Goal, Criteria, Source, Hand-offs, Evidence and History. */
function MainParts({ project, card, flow, mark, rollups, handOffs, markFailure, stateFailure, links, flowOf, onAct }: Omit<CardViewProps, "id" | "query" | "today"> & { card: Card; flow: CardFlow | undefined }) {
  const t = useTranslate();
  const allows = useAllowed(project);
  const cancelled = card.cancellation !== null;
  const back = flow?.return ?? null;
  const history = [...(card.history ?? [])].sort((a, b) => a.seq - b.seq);
  const notes = ownNotes(card);
  return (
    <>
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
      {back !== null && card.column === back && !cancelled && <ReturnReason card={card} column={back} />}
      <Closure card={card} />
      {card.goal !== null && (
        <Part title={t("board.panel.goal")} className="issue-document">
          <p className="board-text">{shownText(card.goal)}</p>
        </Part>
      )}
      {card.criteria.length > 0 && (
        <Part title={t("board.panel.criteria")} className="issue-document">
          <ul className="board-panel-list issue-criteria">
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
                {/* The document over the card, stacked (docs/features/doc-modal.md "Acceptance criteria", DM-10). */}
                <DocLink project={project} docRef={target}>
                  <bdi className="mono">{shownSlot(target)}</bdi>
                </DocLink>
                <TargetState rollup={rollups.get(target)} />
              </li>
            ))}
          </ul>
        )}
      </Part>
      <HandOffs rollup={handOffs} links={links} flowOf={flowOf} />
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
 * The main column as the card's read stands: a skeleton while out, the daemon's words and Retry when
 * it failed, its `{id, reason}` verbatim when there is no such card, else every part. A read failing
 * again keeps what it last read, saying so.
 */
function CardMain(props: CardViewProps) {
  const { project, id, query, flowOf } = props;
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
    body = <MainParts {...props} card={answer} flow={flowOf(answer.flow)} />;
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

/** "Development · 2 agent tasks", or "No agent tasks": every link counted, read here or not. */
function developmentSummary(card: Card, t: Translate): string {
  const tasks = card.tasks.length === 0 ? t("board.item.noAgentTasks") : t("board.item.agentTasks", { count: card.tasks.length });
  return `${t("board.item.development")} · ${tasks}`;
}

/**
 * The side column: the status (its column's lozenge, the card's own badges, and Move on an open
 * card), the people's fields in a list whose empty ones fold under "More fields", and Development, the
 * card's Tasks part (`docs/canon/card-tasks.md` "Screen"), folded.
 */
function CardSide({ project, card, flow, today, links, onAct }: { project: string; card: Card; flow: CardFlow | undefined; today: string; links: CardLinks; onAct: (action: CardAction, card: Card, trigger: HTMLElement) => void }) {
  const t = useTranslate();
  const statusId = useId();
  const accepted = card.acceptance !== null;
  const cancelled = card.cancellation !== null;
  const text = (value: string | null) => (value === null ? null : <Value text={value} />);
  const details: Property[] = [
    { key: "assignee", label: t("board.panel.assignee"), value: text(card.assignee) },
    { key: "reviewer", label: t("board.panel.reviewer"), value: text(card.reviewer) },
    { key: "flow", label: t("board.item.flow"), value: <Value text={card.flow} /> },
    { key: "due", label: t("board.panel.due"), value: text(card.due) },
    { key: "estimate", label: t("board.panel.estimate"), value: text(card.estimate) },
    { key: "discipline", label: t("board.panel.discipline"), value: text(card.discipline) },
    { key: "parent", label: t("board.panel.parent"), value: card.parent === null ? null : <CardLink id={card.parent} links={links} /> },
    { key: "corrects", label: t("board.panel.corrects"), value: card.corrects === null ? null : <CardLink id={card.corrects} links={links} /> },
    { key: "createdBy", label: t("board.panel.createdBy"), value: <Value text={card.created_by} /> },
    { key: "createdAt", label: t("board.panel.createdAt"), value: <At at={card.created_at} /> },
  ];
  return (
    <>
      <section aria-labelledby={statusId}>
        <div className="property-row">
          <span id={statusId} className="property-label">
            {t("board.item.status")}
          </span>
          <div className="property-value issue-status">
            <Lozenge {...statusLook(flow, card.column)} />
            {accepted && <Badge look={acceptedLook(t)} />}
            {cancelled && <Badge look={cancelledLook(t)} />}
            {isOverdue({ due: card.due, accepted, cancelled }, today) && <Badge look={overdueLook(t)} />}
            {!accepted && !cancelled && (
              <Button
                spacing="compact"
                icon="cardMove"
                aria-haspopup="menu"
                onClick={(event) => {
                  onAct("move", card, event.currentTarget);
                }}
              >
                <span>{t("board.panel.move")}</span>
              </Button>
            )}
          </div>
        </div>
      </section>
      <PropertyList label={t("board.panel.details")} items={details} foldEmpty />
      <Disclosure summary={developmentSummary(card, t)} regionLabel={t("board.tasks.title")} className="issue-development">
        <CardTasks project={project} card={card} />
      </Disclosure>
    </>
  );
}

/**
 * The work item: the IssueView of the card, its head the trail (Project, Board, the parent, the card),
 * the caller's tools and title (the modal's `h2`, the page's `h1`), and the acts of an open card; its
 * main column the card's read as it stands, the caller's notices first; its side column once the card
 * is read. Each column has its own boundary, the title outside them.
 */
export function CardView({ heading, tools, notices, ...props }: CardViewProps & { heading: ReactNode; tools: ReactNode; notices?: ReactNode }) {
  const { project, id, query, flowOf, links, onAct, today } = props;
  const t = useTranslate();
  const projects = useProjects();
  const allows = useAllowed(project);
  const card = query.data !== undefined && isCard(query.data) ? query.data : undefined;
  const flow = card === undefined ? undefined : flowOf(card.flow);
  const listed = projects.data?.find((candidate) => candidate.slug === project);
  const parent = card?.parent ?? null;
  const crumbs: Crumb[] = [
    { key: "project", label: listed?.name ?? project, href: homeHash(project) },
    { key: "board", label: t("app.sections.board.label"), href: sectionHash(project, "board") },
    ...(parent === null
      ? []
      : [
          {
            key: "parent",
            label: parent,
            href: sectionHash(project, "board", parent),
            mono: true,
            onFollow: links.page
              ? undefined
              : () => {
                  links.onFollow(parent);
                },
          },
        ]),
    { key: "card", label: id, mono: true },
  ];
  const open = card !== undefined && card.acceptance === null && card.cancellation === null;
  return (
    <IssueView
      breadcrumbs={<Breadcrumbs items={crumbs} />}
      tools={tools}
      title={heading}
      actions={
        open ? (
          <RegionBoundary name={t("board.view.boundaryCard")}>
            <HeadActs card={card} flow={flow} accepts={allows("accept")} onAct={onAct} />
          </RegionBoundary>
        ) : null
      }
      main={
        <>
          {notices}
          <RegionBoundary name={t("board.view.boundaryCard")}>
            <CardMain {...props} />
          </RegionBoundary>
        </>
      }
      side={
        card === undefined ? null : (
          <RegionBoundary name={t("board.view.boundaryCard")}>
            <CardSide project={project} card={card} flow={flow} today={today} links={links} onAct={onAct} />
          </RegionBoundary>
        )
      }
    />
  );
}

/** The card's title once read, else null: the modal's heading and the page's `h1` fall back to its ID. */
export function titleOf(query: CardQuery): string | null {
  return query.data !== undefined && isCard(query.data) ? query.data.title : null;
}
