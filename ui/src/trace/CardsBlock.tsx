import { useRef, useState, type ReactNode } from "react";
import { apiErrorOf } from "../api/client";
import { useCard, useTrace } from "../api/queries";
import type { CardWrite, TraceCard, TraceDocument, TraceNode, TraceUse } from "../api/types";
import { sectionHash } from "../app/routes";
import { useAllowed } from "../board/acting";
import { ActOff } from "../board/ActOff";
import { ClearCardDialog, ClearGroupDialog, NewCardDialog } from "../board/CardDialogs";
import { isCard } from "../board/CardPanel";
import { canClear, cardStateLook, isMarked } from "../board/labels";
import { countsText, lastReadWords, rollupLook } from "../flow/labels";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { useAnnounce } from "../ui/announcer";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { shownSlot } from "../ui/text";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";

/** A Clear asked for: the card, the button to go back to; `asked` counts the presses, so a read that failed is read again. */
interface Clearing {
  card: TraceCard;
  trigger: HTMLElement;
  asked: number;
}

/** The row of the node asked, never another's: a read of a group holds what it holds besides, by (path, position). */
function rowOf(trace: TraceDocument, nodeRef: string): TraceNode | undefined {
  return trace.nodes.find((node) => node.id === nodeRef);
}

/** Each card of a read once: a card on two rows keeps the mark that says something, one a person can clear before any. */
function cardsOf(trace: TraceDocument): TraceCard[] {
  const held = new Map<string, TraceCard>();
  for (const card of trace.nodes.flatMap((node) => node.cards)) {
    const seen = held.get(card.id);
    if (seen === undefined || (isMarked(card) && !isMarked(seen)) || (canClear(card) && !canClear(seen))) {
      held.set(card.id, card);
    }
  }
  return [...held.values()];
}

/** Each node using this one once, what it holds using it included. */
function usesOf(trace: TraceDocument): TraceUse[] {
  return [...new Map(trace.nodes.flatMap((node) => node.uses).map((use) => [use.id, use])).values()];
}

/** A dialog's keys stay in the dialog: the view around the block has Esc and `?` of its own. */
function OwnKeys({ children }: { children: ReactNode }) {
  return (
    <div
      onKeyDown={(event) => {
        event.stopPropagation();
      }}
    >
      {children}
    </div>
  );
}

/** One card: its title, a link to the Board, the word for where it stands, where it is, Clear on a suspect one, idle as a hub's `allowed` says. */
function CardLine({ project, card, clears, onClear }: { project: string; card: TraceCard; clears: boolean; onClear: (card: TraceCard, trigger: HTMLElement) => void }) {
  const t = useTranslate();
  return (
    <li className="cards-line">
      <a className="cards-line-title" href={sectionHash(project, "board", card.id)}>
        <span className="mono cards-line-id">{card.id}</span> <bdi>{shownSlot(card.title)}</bdi>
      </a>
      <Badge look={cardStateLook(card, t)} />
      <span className="muted cards-line-place">
        <bdi>{shownSlot(`${card.flow}/${card.column}`)}</bdi>
      </span>
      {canClear(card) && (
        <button
          type="button"
          className="button"
          aria-disabled={!clears}
          onClick={(event) => {
            if (clears) {
              onClear(card, event.currentTarget);
            }
          }}
        >
          {fill(t("trace.clear", { id: slot("id") }), { id: <span className="sr-only">{card.id}</span> })}
        </button>
      )}
    </li>
  );
}

/** A Clear reads its card first: the dialog goes on the revision read now. A card that cannot be read says so, in the daemon's words. */
function ClearCard({ project, clearing, onClose, onDone }: { project: string; clearing: Clearing; onClose: () => void; onDone: (answer: CardWrite) => void }) {
  const t = useTranslate();
  const read = useCard(project, clearing.card.id);
  const answer = read.data;
  if (answer === undefined || !isCard(answer)) {
    const reason = read.error === null ? answer?.reason : apiErrorOf(read.error).message;
    return reason === undefined ? (
      <Skeleton label={t("trace.cardReading", { id: clearing.card.id })} lines={1} />
    ) : (
      <p className="note" role="alert">
        <Icon name="alert" />
        <span>{fill(t("trace.cardFailed", { id: clearing.card.id, reason: slot("reason") }), { reason: <span lang="en">{reason}</span> })}</span>
      </p>
    );
  }
  return (
    <OwnKeys>
      <ClearCardDialog project={project} card={answer} on={clearing.card.on} onClose={onClose} onDone={onDone} />
    </OwnKeys>
  );
}

/**
 * A node's cards (docs/features/trace-views.md "A node's cards"), beside it in the Flow's panel and the
 * Spec tree: `getTrace(ref)`'s rollup, each card with its mark, linked to the Board, the nodes using
 * it, New card. A suspect card takes its Clear here, the dialog of the Board; a group's block adds
 * Clear all, no effect only. Nothing here is counted: a group says the counts served for it. On a hub
 * Clear and Clear all stand idle where `allowed` omits them (`docs/features/hub-team.md`).
 */
export function CardsBlock({ project, nodeRef, section, heading: Heading }: { project: string; nodeRef: string; section: "flow" | "tree"; heading: "h2" | "h3" }) {
  const t = useTranslate();
  const read = useTrace(project, { ref: nodeRef });
  const failure = useRetainedFailure(read.error, read.isFetching);
  const allows = useAllowed(project);
  const announce = useAnnounce();
  const focusLater = useFocusLater();
  const [creating, setCreating] = useState(false);
  const [clearing, setClearing] = useState<Clearing | null>(null);
  const [clearingAll, setClearingAll] = useState(false);
  const [made, setMade] = useState<string | null>(null);
  const [notes, setNotes] = useState<readonly string[]>([]);
  const presses = useRef(0);
  const title = useRef<HTMLHeadingElement>(null);
  const newButton = useRef<HTMLButtonElement>(null);
  const allButton = useRef<HTMLButtonElement>(null);
  const retried = useRetryFocus(read.dataUpdatedAt === 0 ? undefined : read.dataUpdatedAt, () => title.current);

  function retry() {
    retried();
    void read.refetch({ cancelRefetch: false });
  }

  function backTo(target: HTMLElement | null) {
    focusLater(() => (target?.isConnected === true ? target : title.current));
  }

  function afterClear(answer: CardWrite, trigger: HTMLElement) {
    setClearing(null);
    setNotes(answer.notes);
    backTo(trigger);
    announce(answer.id === null ? t("trace.clearedCard") : t("trace.cleared", { id: answer.id }));
  }

  let body: ReactNode;
  const trace = read.data;
  if (trace === undefined) {
    body =
      failure === null ? (
        <Skeleton label={t("trace.reading", { ref: nodeRef })} lines={2} />
      ) : (
        <ErrorPanel
          title={t("trace.failed", { ref: nodeRef })}
          message={apiErrorOf(failure).message}
          retrying={read.isFetching}
          attempt={read.errorUpdateCount}
          onRetry={retry}
        />
      );
  } else if (trace.reason !== null) {
    body = (
      <p className="verbatim" lang="en">
        {trace.reason}
      </p>
    );
  } else {
    const row = rowOf(trace, nodeRef);
    const cards = cardsOf(trace);
    const uses = usesOf(trace);
    const group = trace.nodes.length > 1;
    const clearable = cards.filter(canClear).length;
    const clearsAll = allows("group_clear");
    body = (
      <>
        {/* A state read before a failure is no state now: the failure line below speaks for what stands under it. */}
        {read.error === null &&
          (row === undefined ? (
            <p className="cards-state muted">{t("trace.noRow", { ref: nodeRef })}</p>
          ) : (
            <p className="cards-state">{group ? countsText(row.counts, t) : <Badge look={rollupLook(row.rollup, t)} />}</p>
          ))}
        {trace.notes.length > 0 && (
          <ul className="note-list" aria-label={t("trace.notes", { ref: nodeRef })}>
            {trace.notes.map((note, index) => (
              <li key={`${String(index)}-${note}`} lang="en">
                {note}
              </li>
            ))}
          </ul>
        )}
        {read.error !== null && (
          <div className="flow-failed">
            <p className="note" role="alert">
              <Icon name="alert" />
              <span>
                {fill(t("trace.failedAgain", { ref: nodeRef, message: slot("message") }), { message: <span lang="en">{apiErrorOf(read.error).message}</span> })}
                <br />
                {lastReadWords(t)}
              </span>
            </p>
            <button type="button" className="button" aria-disabled={read.isFetching} onClick={read.isFetching ? undefined : retry}>
              <Icon name="retry" />
              <span>{read.isFetching ? t("ui.retrying") : t("ui.retry")}</span>
            </button>
          </div>
        )}
        {cards.length === 0 && row !== undefined && <p className="muted">{t("trace.none")}</p>}
        {cards.length > 0 && (
          <ul className="cards-lines" aria-label={t("trace.cardsOf", { ref: nodeRef })}>
            {cards.map((shown) => (
              <CardLine
                key={shown.id}
                project={project}
                card={shown}
                clears={allows("clear")}
                onClear={(card, trigger) => {
                  presses.current += 1;
                  setClearing({ card, trigger, asked: presses.current });
                }}
              />
            ))}
          </ul>
        )}
        {clearable > 0 && !allows("clear") && <ActOff act="clear" />}
        {uses.length > 0 && (
          <>
            <p className="field-label">{t("trace.usedBy")}</p>
            <ul className="cards-lines" aria-label={t("trace.usingOf", { ref: nodeRef })}>
              {uses.map((use) => (
                <li key={use.id} className="cards-line">
                  <a className="mono cards-line-title" href={sectionHash(project, section, use.id)}>
                    <bdi>{shownSlot(use.id)}</bdi>
                  </a>
                  <Badge look={rollupLook(use.rollup, t)} />
                  <span className="muted cards-line-place">{t("trace.usesCards", { count: use.cards })}</span>
                </li>
              ))}
            </ul>
          </>
        )}
        {group && clearable > 0 && (
          <p className="cards-actions">
            <button
              ref={allButton}
              type="button"
              className="button"
              aria-haspopup="dialog"
              aria-disabled={!clearsAll}
              onClick={() => {
                if (clearsAll) {
                  setClearingAll(true);
                }
              }}
            >
              {t("trace.clearAll")}
            </button>
          </p>
        )}
        {group && clearable > 0 && !clearsAll && <ActOff act="group_clear" />}
        {clearingAll && (
          <OwnKeys>
            <ClearGroupDialog
              project={project}
              group={nodeRef}
              cards={clearable}
              onClose={() => {
                setClearingAll(false);
                backTo(allButton.current);
              }}
              onDone={(answer) => {
                setClearingAll(false);
                setNotes(answer.notes);
                backTo(allButton.current);
                announce(t("trace.clearedGroup", { count: answer.cleared.length, group: answer.group }));
              }}
            />
          </OwnKeys>
        )}
      </>
    );
  }

  return (
    <section className="cards-block" aria-label={t("trace.title")}>
      <div className="cards-head">
        <Heading ref={title} tabIndex={-1} className="card-section-title">
          {t("trace.title")}
        </Heading>
        <button
          ref={newButton}
          type="button"
          className="button button-quiet"
          aria-haspopup="dialog"
          onClick={() => {
            setCreating(true);
          }}
        >
          <Icon name="cardNew" />
          <span>{t("trace.newCard")}</span>
        </button>
      </div>
      {body}
      {made !== null && (
        <p className="note">
          <Icon name="info" />
          <span>
            {fill(t("trace.made", { id: slot("id"), link: slot("link") }), {
              id: <span className="mono">{made}</span>,
              link: <a href={sectionHash(project, "board", made)}>{t("trace.openOnBoard")}</a>,
            })}
          </span>
        </p>
      )}
      {notes.length > 0 && (
        <ul className="note-list" aria-label={t("trace.writeNotes")}>
          {notes.map((note, index) => (
            <li key={`${String(index)}-${note}`} lang="en">
              {note}
            </li>
          ))}
        </ul>
      )}
      {clearing !== null && (
        <ClearCard
          key={clearing.asked}
          project={project}
          clearing={clearing}
          onClose={() => {
            setClearing(null);
            backTo(clearing.trigger);
          }}
          onDone={(answer) => {
            afterClear(answer, clearing.trigger);
          }}
        />
      )}
      {creating && (
        <OwnKeys>
          <NewCardDialog
            project={project}
            target={nodeRef}
            onClose={() => {
              setCreating(false);
              backTo(newButton.current);
            }}
            onDone={({ id }) => {
              setCreating(false);
              backTo(newButton.current);
              if (id !== null) {
                setMade(id);
                announce(t("trace.created", { id }));
              }
            }}
          />
        </OwnKeys>
      )}
    </section>
  );
}
