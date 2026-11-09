import { memo, type DragEvent, type MouseEvent } from "react";
import type { CardRow } from "../api/types";
import { sectionHash } from "../app/routes";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { hasModifier } from "../ui/keys";
import { shownSlot } from "../ui/text";
import { isOverdue } from "./due";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { acceptedLook, byHand, cancelledLook, markLook, markNames, overdueLook, rowMark, unassigned } from "./labels";

/** What a card asks of its board. */
export interface CardActions {
  onOpen: (id: string) => void;
  /** Its Move button was pressed, or `m` on the card (button null). */
  onMove: (row: CardRow, button: HTMLElement | null) => void;
  onFocus: (id: string) => void;
  onDragStart: (row: CardRow) => void;
  onDragEnd: () => void;
}

/**
 * The card's source: its first target, a link to the Spec tree, `+<n>` for the rest; no target: made
 * by hand.
 */
function Source({ project, targets }: { project: string; targets: readonly string[] }) {
  const t = useTranslate();
  const first = targets[0];
  if (first === undefined) {
    return <span className="board-card-source muted">{byHand(t)}</span>;
  }
  return (
    <span className="board-card-source">
      <span className="sr-only">{t("board.card.source")}</span>
      <a href={sectionHash(project, "tree", first)} tabIndex={-1} draggable={false}>
        <bdi className="mono">{shownSlot(first)}</bdi>
      </a>
      {targets.length > 1 && <span className="muted"> +{targets.length - 1}</span>}
    </span>
  );
}

/**
 * One card (docs/features/board-ui.md "Board"): title, assignee, due, estimate, its source. A mark
 * stands first and names what it is on, the list's own, so a card on a gone target has it too; an
 * accepted card's word yields to it (docs/features/trace-views.md "The board tile"). The board holds
 * one tab stop: every card but the roving one, and the controls inside each, are out of
 * the Tab order, and keys or a click reach them; `m` is the Move button, Enter the title's link, the
 * panel the source. A click anywhere on it but a control opens the panel. A closed card neither
 * moves nor drags: accepted and cancelled take no write, and say so. Memoised: an arrow key or a drag
 * changes two cards' props, and the rest of a full board stays as drawn, `actions` being stable.
 */
export const BoardCard = memo(function BoardCard({
  project,
  row,
  today,
  tabbable,
  selected,
  menuOpen,
  dropAbove,
  actions,
}: {
  project: string;
  row: CardRow;
  today: string;
  tabbable: boolean;
  selected: boolean;
  menuOpen: boolean;
  /** A drag would put the card it carries right above this one. */
  dropAbove: boolean;
  actions: CardActions;
}) {
  const t = useTranslate();
  const closed = row.accepted || row.cancelled;
  const mark = rowMark(row);
  const markShown = mark === null ? null : markLook(mark, t);
  const named = mark === null ? null : markNames(mark);

  function onClick(event: MouseEvent<HTMLElement>) {
    if (!(event.target instanceof Element) || event.target.closest("a, button") === null) {
      actions.onOpen(row.id);
    }
  }

  function onTitleClick(event: MouseEvent<HTMLAnchorElement>) {
    // Cmd-, Ctrl-, Shift- or Alt-click opens it elsewhere, as the browser does.
    if (event.button === 0 && !hasModifier(event) && !event.shiftKey) {
      event.preventDefault();
      actions.onOpen(row.id);
    }
  }

  function onDragStart(event: DragEvent<HTMLElement>) {
    // Firefox starts a drag only with data; jsdom's events carry no transfer.
    const transfer = event.dataTransfer as DataTransfer | undefined;
    transfer?.setData("text/plain", row.id);
    if (transfer !== undefined) {
      transfer.effectAllowed = "move";
    }
    actions.onDragStart(row);
  }

  const classes = ["board-card", selected && "board-card-selected", closed && "board-card-closed"].filter(Boolean).join(" ");
  return (
    <li className={dropAbove ? "board-slot board-slot-drop" : "board-slot"}>
      <article
        className={classes}
        data-card={row.id}
        data-column={row.column}
        tabIndex={tabbable ? 0 : -1}
        aria-current={selected ? "true" : undefined}
        aria-label={`${row.id}: ${shownSlot(row.title)}${markShown === null ? "" : `, ${markShown.label}`}`}
        draggable={!closed}
        onClick={onClick}
        onFocus={() => {
          actions.onFocus(row.id);
        }}
        onDragStart={onDragStart}
        onDragEnd={actions.onDragEnd}
      >
        {markShown !== null && (
          <p className="board-card-mark">
            <Badge look={markShown} />
            {named !== null && <bdi className="mono board-card-marked">{shownSlot(named)}</bdi>}
          </p>
        )}
        <div className="board-card-top">
          <span className="mono board-card-id">{row.id}</span>
          {row.accepted && markShown === null && <Badge look={acceptedLook(t)} />}
          {row.cancelled && <Badge look={cancelledLook(t)} />}
          {isOverdue(row, today) && <Badge look={overdueLook(t)} />}
        </div>
        <h3 className="board-card-title">
          <a href={sectionHash(project, "board", row.id)} tabIndex={-1} draggable={false} onClick={onTitleClick}>
            <bdi>{shownSlot(row.title)}</bdi>
          </a>
        </h3>
        <p className={row.assignee === null ? "board-card-who muted" : "board-card-who"}>
          <span className="sr-only">{t("board.card.assignee")}</span>
          <bdi>{row.assignee === null ? unassigned(t) : shownSlot(row.assignee)}</bdi>
        </p>
        {(row.due !== null || row.estimate !== null) && (
          <p className="board-card-when">
            {row.due !== null && <span>{t("board.card.due", { due: shownSlot(row.due) })}</span>}
            {row.estimate !== null && <span>{t("board.card.estimate", { estimate: shownSlot(row.estimate) })}</span>}
          </p>
        )}
        <div className="board-card-foot">
          <Source project={project} targets={row.targets} />
          {!closed && (
            <button
              type="button"
              className="button button-quiet board-move"
              data-card-move=""
              tabIndex={-1}
              aria-haspopup="menu"
              aria-expanded={menuOpen}
              onClick={(event) => {
                actions.onMove(row, event.currentTarget);
              }}
            >
              <Icon name="cardMove" />
              <span>{fill(t("board.card.move", { id: slot("id") }), { id: <span className="sr-only">{t("board.card.moveId", { id: row.id })}</span> })}</span>
            </button>
          )}
        </div>
      </article>
    </li>
  );
});
