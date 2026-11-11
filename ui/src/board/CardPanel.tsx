import { lazy, Suspense, useId, type MouseEvent, type ReactNode } from "react";
import { apiErrorOf } from "../api/client";
import { useChildren, useProjects, type CardQuery, type ChildrenQuery } from "../api/queries";
import type { Card, CardEvent, CardFlow, CardItem, CardNotFound } from "../api/types";
import { RegionBoundary } from "../app/RegionBoundary";
import { DocLink } from "../doc/DocLink";
import { homeHash, sectionHash } from "../app/routes";
import { rollupLook } from "../flow/labels";
import { Badge } from "../ui/Badge";
import { Breadcrumbs, type Crumb } from "../ui/Breadcrumbs";
import { Button } from "../ui/Button";
import { Disclosure } from "../ui/Disclosure";
import { Icon, type IconName } from "../ui/Icon";
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
import { flowTitleOf } from "./filter";
import { useTranslate, type Translate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import {
  acceptedLook,
  byHand,
  canClear,
  cancelledLook,
  columnTitle,
  eventWords,
  isDecision,
  isItemEvent,
  itemEventWords,
  knowsEvent,
  leadActs,
  mandateAbout,
  mandateMark,
  mandateWords,
  markLine,
  markLook,
  needsLine,
  overdueLook,
  raisesItem,
  returnedLook,
  type CardMark,
  type LeadAct,
} from "./labels";
import { returnReasonOf } from "./rollup";
import { CardLink, type CardLinks } from "./CardLink";
import { CardProgress } from "./CardProgress";
import { CardTasks, ownNotes } from "./CardTasks";

export type { CardLinks } from "./CardLink";

// A card as a work item (`docs/canon/cards.md` "Documents and events";
// docs/features/design-foundation.md "The work item: the task modal and page"): one IssueView for
// the Board's modal and the card's page alike. The head holds the breadcrumbs, the caller's tools and
// title, and the acts (Accept, primary, in `accept.from` alone; Hand off; Return…; Edit; Assign; Cancel
// card, subtle and last). The main column holds the card's blocks (its mark and Clear, why it came
// back, Accept's verdict, the cancellation), Goal and Criteria at the document's size, Source,
// Subtasks (once Hand-offs; its progress, CardProgress.tsx, from the card's own children read), Evidence and Activity, the card's events and its items' in one list (docs/canon/card-questions.md "The rule"). The side column holds the status (its column's
// lozenge and the Move menu), the people's fields with the empty ones folded, and Development, the
// linked `T-` tasks (`docs/canon/card-tasks.md` "The rule"), folded. Nothing here judges a column: the
// return column and the accept column are the flow's, looked up (`docs/canon/card-flows.md` "The rule").

/**
 * An item's block, in a chunk of its own with the dialogs that raise and answer items
 * (docs/features/card-questions.md "Acceptance criteria", U1, U7): until it arrives, the item's ID.
 */
const ItemBlock = lazy(() => import("./CardItems").then((module) => ({ default: module.ItemBlock })));

/**
 * A parent's Decisions from subtasks, in the items' chunk (docs/features/task-mandate.md
 * "Acceptance criteria", V3): drawn only when the card's `subtask_items` holds a decision, its
 * heading standing until the chunk arrives.
 */
const SubtaskDecisions = lazy(() => import("./CardItems").then((module) => ({ default: module.SubtaskDecisions })));

/** The card's acts, each a dialog but Move, which opens the Move menu; `ask` and `decide` raise an item on it. */
export type CardAction = "move" | "handoff" | "split" | "return" | "edit" | "assign" | "cancel" | "accept" | "clear" | "ask" | "decide";

/** Whether a card's read found one, rather than `{id, reason}`. */
export function isCard(answer: Card | CardNotFound): answer is Card {
  return "title" in answer;
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
  // A mandate the payload holds, named or changed (docs/features/task-mandate.md "Acceptance criteria", V2): its word; a value this build does not know raw, under lang="en".
  const named = payloadWord(event.payload, "mandate");
  const words = named === null ? null : mandateWords(named, t);
  return (
    <li className="board-history-line">
      <At at={event.at} />
      <span>
        {/* A type this build has no words for, raw: the daemon's word, under lang="en". */}
        <strong lang={knowsEvent(event.type) ? undefined : "en"}>{eventWords(event.type, t)}</strong>
        {by !== null && fill(t("board.history.by", { by: slot("by") }), { by: <bdi>{shownSlot(by)}</bdi> })}
        {detail !== "" && <span className="muted"> {shownSlot(detail)}</span>}
        {words !== null && (
          <span className="muted">
            {detail === "" ? " " : ", "}
            {fill(t("board.history.mandate", { value: slot("value") }), { value: <span lang={words.served ? "en" : undefined}>{shownSlot(words.text)}</span> })}
          </span>
        )}
      </span>
      {note !== null && <span className="board-text">{fill(t("board.history.why", { note: slot("note") }), { note: <bdi>{shownText(note)}</bdi> })}</span>}
    </li>
  );
}

/** Who an item's event names: its `as`, else the item's author's login, else its role; none when it has none. */
function itemBy(event: CardEvent, item: CardItem | undefined): string | null {
  return payloadWord(event.payload, "as") ?? item?.author.login ?? item?.author.role ?? null;
}

/** An item's block, the chunk's, its ID standing in until it arrives. */
function Item({ project, item }: { project: string; item: CardItem }) {
  return (
    <Suspense fallback={<p className="mono muted">{shownSlot(item.id)}</p>}>
      <ItemBlock project={project} item={item} />
    </Suspense>
  );
}

/**
 * An item's event in the Activity (docs/features/card-questions.md "Acceptance criteria", U1): time,
 * its word (labels.ts; an unknown type raw), the item's ID and who; the event that raised it opens
 * the item's block under it.
 */
function ItemLine({ project, event, items }: { project: string; event: CardEvent; items: readonly CardItem[] }) {
  const t = useTranslate();
  const id = payloadWord(event.payload, "id");
  const item = items.find((candidate) => candidate.id === id);
  const by = itemBy(event, item);
  return (
    <li className="board-history-line board-activity-item">
      <At at={event.at} />
      <span>
        <strong lang={knowsEvent(event.type) ? undefined : "en"}>{itemEventWords(event.type, t)}</strong>
        {id !== null && <span className="mono"> {shownSlot(id)}</span>}
        {by !== null && fill(t("board.history.by", { by: slot("by") }), { by: <bdi>{shownSlot(by)}</bdi> })}
      </span>
      {raisesItem(event.type) && item !== undefined && <Item project={project} item={item} />}
    </li>
  );
}

/**
 * Activity, where History stood (docs/canon/card-questions.md "The rule"; U1 of
 * docs/features/card-questions.md "Acceptance criteria"): the items raised before the history's last
 * events first, by ID, under Earlier items; then every event by `seq`, the card's as History drew
 * them, its items' by their words, each item's block at the event that raised it. Nothing is
 * reordered by time.
 */
function Activity({ project, card, flow }: { project: string; card: Card; flow: CardFlow | undefined }) {
  const t = useTranslate();
  const history = [...(card.history ?? [])].sort((a, b) => a.seq - b.seq);
  const raised = new Set(history.filter((event) => raisesItem(event.type)).map((event) => payloadWord(event.payload, "id")));
  const earlier = card.items.filter((item) => !raised.has(item.id));
  return (
    <Part title={t("board.panel.activity")}>
      {history.length === 0 && earlier.length === 0 && <p className="muted">{t("board.panel.noneYet")}</p>}
      {earlier.length > 0 && (
        <>
          <h4 className="board-activity-earlier">{t("board.activity.earlier")}</h4>
          <ul className="board-panel-list" aria-label={t("board.activity.earlier")}>
            {earlier.map((item) => (
              <li key={item.id}>
                <Item project={project} item={item} />
              </li>
            ))}
          </ul>
        </>
      )}
      {history.length > 0 && (
        <ol className="board-panel-list board-activity">
          {history.map((event) =>
            isItemEvent(event.type) ? <ItemLine key={event.seq} project={project} event={event} items={card.items} /> : <HistoryLine key={event.seq} event={event} flow={flow} />,
          )}
        </ol>
      )}
    </Part>
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
            aria-haspopup="dialog"
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

/** An act of the head, by its key: the acts a mandate leads with among them (labels.ts `leadActs`). */
type HeadAct = [LeadAct | "handoff" | "split" | "edit" | "assign" | "cancel", ReactNode];

/**
 * The acts in the head's order: the ones the card's mandate leads with, those shown, right after
 * Accept and in its order (binding: Return, then Ask a question…; open: Record a decision…); every
 * other act in today's order. A card with no mandate keeps today's order, act for act
 * (docs/features/task-mandate.md "Acceptance criteria", V2). The order is looked up, never judged.
 */
function ordered(acts: readonly HeadAct[], card: Pick<Card, "mandate">): ReactNode[] {
  const lead = leadActs(card.mandate);
  const first = lead.flatMap((wanted) => acts.filter(([key]) => key === wanted));
  return [...first, ...acts.filter(([key]) => !first.some(([led]) => led === key))].map(([, node]) => node);
}

/**
 * An open card's acts, the head's, in this order: Accept, the one primary, in `accept.from` alone and
 * idle where a hub's `allowed` omits it; then the acts its mandate leads with; then the rest: Hand off;
 * Split to departments…, the card the parent (docs/features/department-split.md "Acceptance criteria",
 * U1); Return when its flow names a return column it is not in and `allowed` holds the move there; Ask
 * a question… and Record a decision… on a card with a target; Edit; Assign; Cancel card, subtle and
 * last. Move stands in the side column's Status. Each opens a dialog and says so (`aria-haspopup`). A
 * `by` the acting name lacks reads "needs <names>", as in the Move menu; the daemon judges either way.
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
  const acts: HeadAct[] = [
    [
      "handoff",
      <Button key="handoff" icon="cardHandOff" aria-haspopup="dialog" onClick={act("handoff")}>
        <span>{t("board.handoff.action")}</span>
      </Button>,
    ],
    [
      "split",
      <Button key="split" icon="workspace" aria-haspopup="dialog" onClick={act("split")}>
        <span>{t("board.split.act")}</span>
      </Button>,
    ],
    ...(returning === undefined
      ? []
      : ([
          [
            "return",
            <Button key="return" icon="cardReturned" aria-haspopup="dialog" onClick={act("return")}>
              <span>{t("board.return.action")}</span>
            </Button>,
          ],
        ] satisfies HeadAct[])),
    ...itemActs(card, onAct, t),
    [
      "edit",
      <Button key="edit" aria-haspopup="dialog" onClick={act("edit")}>
        {t("board.panel.edit")}
      </Button>,
    ],
    [
      "assign",
      <Button key="assign" aria-haspopup="dialog" onClick={act("assign")}>
        {t("board.panel.assign")}
      </Button>,
    ],
    [
      "cancel",
      <Button key="cancel" appearance="subtle" aria-haspopup="dialog" onClick={act("cancel")}>
        {t("board.panel.cancel")}
      </Button>,
    ],
  ];
  return (
    <>
      <div className="board-panel-actions" role="group" aria-label={t("board.panel.actions")}>
        {inAcceptColumn && (
          <Button appearance="primary" aria-haspopup="dialog" aria-disabled={!accepts} onClick={accepts ? act("accept") : undefined}>
            {t("board.panel.accept")}
          </Button>
        )}
        {ordered(acts, card)}
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

/**
 * The two acts that raise an item on a card with a target, open or closed (docs/canon/card-questions.md
 * "The rule"; docs/features/card-questions.md "Acceptance criteria", U3): Ask a question… and Record a
 * decision…; a card with no target has none.
 */
function itemActs(card: Card, onAct: (action: CardAction, card: Card, trigger: HTMLElement) => void, t: Translate): HeadAct[] {
  if (card.targets.length === 0) {
    return [];
  }
  return [
    [
      "ask",
      <Button
        key="ask"
        aria-haspopup="dialog"
        onClick={(event) => {
          onAct("ask", card, event.currentTarget);
        }}
      >
        {t("board.ask.action")}
      </Button>,
    ],
    [
      "decide",
      <Button
        key="decide"
        aria-haspopup="dialog"
        onClick={(event) => {
          onAct("decide", card, event.currentTarget);
        }}
      >
        {t("board.decide.action")}
      </Button>,
    ],
  ];
}

/** A closed card's acts: only those that raise an item, when it has a target (U3 of docs/features/card-questions.md "Acceptance criteria"), in its mandate's order. */
function ClosedActs({ card, onAct }: { card: Card; onAct: (action: CardAction, card: Card, trigger: HTMLElement) => void }) {
  const t = useTranslate();
  return (
    <div className="board-panel-actions" role="group" aria-label={t("board.panel.actions")}>
      {ordered(itemActs(card, onAct, t), card)}
    </div>
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

/** The main column of a card read: its blocks, then Goal, Criteria, Source, Subtasks, Evidence and Activity. */
function MainParts({ project, card, flow, mark, rollups, subtasks, markFailure, stateFailure, links, onAct }: Omit<CardViewProps, "id" | "query" | "today"> & { card: Card; flow: CardFlow | undefined; subtasks: ChildrenQuery }) {
  const t = useTranslate();
  const allows = useAllowed(project);
  const cancelled = card.cancellation !== null;
  const back = flow?.return ?? null;
  const notes = ownNotes(card);
  const about = mandateAbout(card.mandate, t);
  // Its subtasks' decisions, served with the card in its one read; their questions stand on each subtask's Activity.
  const decisions = card.subtask_items.filter(isDecision);
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
      {about !== null && (
        // What the task's mandate asks of its department (docs/features/task-mandate.md "Acceptance criteria", V2).
        <p className="note board-mandate-note">
          <Icon name="info" />
          <span>{about}</span>
        </p>
      )}
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
      <Part title={t("board.modal.handOffs")}>
        <CardProgress card={card} query={subtasks} links={links} />
      </Part>
      {decisions.length > 0 && (
        <Part title={t("board.fromSubtasks.heading")}>
          <Suspense
            fallback={
              <ul className="board-panel-list">
                {decisions.map((item) => (
                  <li key={item.id} className="mono muted">
                    {shownSlot(item.id)}
                  </li>
                ))}
              </ul>
            }
          >
            <SubtaskDecisions project={project} decisions={decisions} subtasks={subtasks} links={links} />
          </Suspense>
        </Part>
      )}
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
      <Activity project={project} card={card} flow={flow} />
    </>
  );
}

/**
 * The main column as the card's read stands: a skeleton while out, the daemon's words and Retry when
 * it failed, its `{id, reason}` verbatim when there is no such card, else every part. A read failing
 * again keeps what it last read, saying so.
 */
function CardMain(props: CardViewProps & { subtasks: ChildrenQuery }) {
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

/** A mandate in Details: its icon and word, never colour alone (docs/features/task-mandate.md "Acceptance criteria", V2). */
function MandateValue({ icon, label }: { icon: IconName; label: string }) {
  return (
    <span className="board-mandate">
      <Icon name={icon} />
      <span>{label}</span>
    </span>
  );
}

/** "Agent tasks · 2 linked", or "Agent tasks · none linked": named by its content, every link counted, read here or not. */
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
  const shown = mandateMark(card.mandate, t);
  const details: Property[] = [
    { key: "assignee", label: t("board.panel.assignee"), value: text(card.assignee) },
    { key: "reviewer", label: t("board.panel.reviewer"), value: text(card.reviewer) },
    { key: "mandate", label: t("board.mandate.label"), value: shown === null ? null : <MandateValue icon={shown.icon} label={shown.label} /> },
    { key: "flow", label: t("board.item.flow"), value: <Value text={flowTitleOf(flow, card.flow)} /> },
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
  // The card's children, cancelled ones too, read beside the card while it is shown (docs/features/handoff-progress.md "Acceptance criteria", HP-1).
  const subtasks = useChildren(project, id);
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
        ) : card !== undefined && card.targets.length > 0 ? (
          <RegionBoundary name={t("board.view.boundaryCard")}>
            <ClosedActs card={card} onAct={onAct} />
          </RegionBoundary>
        ) : null
      }
      main={
        <>
          {notices}
          <RegionBoundary name={t("board.view.boundaryCard")}>
            <CardMain {...props} subtasks={subtasks} />
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
