import { type DragEvent, type ReactNode } from "react";
import type { CardFlow } from "../api/types";
import { useTranslate, type Translate } from "../i18n";
import { around } from "../i18n/fill";
import { Lozenge, type LozengeLook } from "../ui/Lozenge";
import { outsideLook, statusLook } from "./category";
import { COLUMN_SHOWN, type BoardColumnCell, type BoardColumnHead } from "./filter";

/** Where a drag at height `y` would put its card: the first card whose middle lies below it; none: last. */
function beforeAt(list: HTMLElement | null, y: number): string | null {
  const cards = Array.from(list?.querySelectorAll<HTMLElement>("[data-card]") ?? []);
  for (const card of cards) {
    const { top, height } = card.getBoundingClientRect();
    if (y < top + height / 2) {
      return card.dataset.card ?? null;
    }
  }
  return null;
}

/** A count drawn, the words around it read out: `, 3 cards`. */
function Count({ count }: { count: number }) {
  const t = useTranslate();
  const said = around(t("board.column.count", { count }), String(count));
  return (
    <span className="board-column-count">
      <span className="sr-only">{said[0]}</span>
      {count}
      <span className="sr-only">{said[1]}</span>
    </span>
  );
}

/** A head's lozenge: its column's category in the flow, its served title; "Not in the flow" stranded's. */
function headLook(head: BoardColumnHead, flow: CardFlow | undefined, t: Translate): LozengeLook {
  return head.column === null ? outsideLook(t) : statusLook(flow, head.column);
}

/**
 * A column's head in the board's sticky header row: its category's lozenge, its title as stored, and
 * how many cards stand in it across every lane (docs/features/design-foundation.md "The Board"). A
 * filter or a lane never changes which heads stand, so none of them moves.
 */
export function BoardColumnHeader({ head, flow, id, count }: { head: BoardColumnHead; flow: CardFlow | undefined; id: string; count: number }) {
  const t = useTranslate();
  return (
    <h2 id={id} className={head.column === null ? "board-column-title board-column-title-outside" : "board-column-title"} data-column-head={head.key}>
      <Lozenge {...headLook(head, flow, t)} />
      <Count count={count} />
    </h2>
  );
}

/**
 * One cell of the grid: a column within a lane, the cards in the daemon's order, "Show all <n>" past
 * the cap. It is labelled by its column's head and its lane's, and repeats the column's title where
 * the columns stack (the header row is not drawn there). A collapsed lane's cell draws its count and
 * no card. A cell takes drops, the cards outside the flow none; a drop is the same `moveCard`, its
 * column alone sent, whichever lane it lands in: the daemon judges it, so any column takes one.
 */
export function BoardColumn({
  cell,
  flow,
  labelledBy,
  collapsed,
  expanded,
  dropping,
  dropAtEnd,
  note,
  onShowAll,
  onDragOver,
  onDragLeave,
  onDrop,
  children,
}: {
  cell: BoardColumnCell;
  /** The board's flow: the cell's title is its column's lozenge where the columns stack. */
  flow: CardFlow | undefined;
  /** Its column head's ID, then its lane's, if any. */
  labelledBy: string;
  /** Its lane is collapsed: the count stands, no card. */
  collapsed: boolean;
  expanded: boolean;
  /** A card is being dragged. */
  dropping: boolean;
  /** A drag would put its card last here. */
  dropAtEnd: boolean;
  /** What the daemon says of this cell's cards. */
  note: ReactNode;
  onShowAll: (cell: BoardColumnCell) => void;
  onDragOver: (cell: BoardColumnCell, before: string | null) => void;
  onDragLeave: (cell: BoardColumnCell) => void;
  onDrop: (cell: BoardColumnCell, before: string | null) => void;
  children: ReactNode;
}) {
  const t = useTranslate();
  const accepts = dropping && cell.column !== null && !collapsed;

  function over(event: DragEvent<HTMLElement>) {
    if (!accepts) {
      return;
    }
    event.preventDefault();
    onDragOver(cell, beforeAt(event.currentTarget, event.clientY));
  }

  function left(event: DragEvent<HTMLElement>) {
    if (!(event.relatedTarget instanceof Node) || !event.currentTarget.contains(event.relatedTarget)) {
      onDragLeave(cell);
    }
  }

  function dropped(event: DragEvent<HTMLElement>) {
    if (accepts) {
      event.preventDefault();
      onDrop(cell, beforeAt(event.currentTarget, event.clientY));
    }
  }

  const classes = ["board-cell", cell.column === null && "board-cell-outside", collapsed && "board-cell-collapsed"].filter(Boolean).join(" ");
  return (
    <section className={classes} aria-labelledby={labelledBy} data-lane={cell.key} data-cell-column={cell.column ?? ""} onDragOver={over} onDragLeave={left} onDrop={dropped}>
      <p className="board-cell-title" aria-hidden="true">
        <Lozenge {...headLook(cell, flow, t)} />
        <Count count={cell.count} />
      </p>
      {collapsed ? (
        <p className="board-cell-count muted">{t("board.lanes.count", { count: cell.count })}</p>
      ) : (
        <>
          {note}
          <ul className="board-list">{children}</ul>
          {dropAtEnd && <div className="board-drop-end" />}
          {!expanded && cell.rows.length > COLUMN_SHOWN && (
            <button
              type="button"
              className="button button-quiet"
              onClick={() => {
                onShowAll(cell);
              }}
            >
              {t("board.column.showAll", { count: cell.rows.length })}
            </button>
          )}
        </>
      )}
    </section>
  );
}
