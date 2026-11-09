import { useId, type DragEvent, type ReactNode } from "react";
import { useTranslate } from "../i18n";
import { around } from "../i18n/fill";
import { COLUMN_SHOWN, type BoardLane } from "./filter";

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

/**
 * One column of a board: its title as stored and how many cards stand in it, the cards in the
 * daemon's order, "Show all <n>" past the cap. A column takes drops, the lane of cards outside the
 * flow none. A drop is the same `moveCard`: the daemon judges it, so any column takes one.
 */
export function BoardColumn({
  lane,
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
  lane: BoardLane;
  expanded: boolean;
  /** A card is being dragged. */
  dropping: boolean;
  /** A drag would put its card last here. */
  dropAtEnd: boolean;
  /** What the daemon says of this lane. */
  note: ReactNode;
  onShowAll: (lane: BoardLane) => void;
  onDragOver: (lane: BoardLane, before: string | null) => void;
  onDragLeave: (lane: BoardLane) => void;
  onDrop: (lane: BoardLane, before: string | null) => void;
  children: ReactNode;
}) {
  const t = useTranslate();
  // The count drawn, the words around it read out: `, 3 cards`.
  const countSaid = around(t("board.column.count", { count: lane.count }), String(lane.count));
  const titleId = useId();
  const accepts = dropping && lane.column !== null;

  function over(event: DragEvent<HTMLElement>) {
    if (!accepts) {
      return;
    }
    event.preventDefault();
    onDragOver(lane, beforeAt(event.currentTarget, event.clientY));
  }

  function left(event: DragEvent<HTMLElement>) {
    if (!(event.relatedTarget instanceof Node) || !event.currentTarget.contains(event.relatedTarget)) {
      onDragLeave(lane);
    }
  }

  function dropped(event: DragEvent<HTMLElement>) {
    if (accepts) {
      event.preventDefault();
      onDrop(lane, beforeAt(event.currentTarget, event.clientY));
    }
  }

  return (
    <section
      className={lane.column === null ? "board-column board-column-outside" : "board-column"}
      aria-labelledby={titleId}
      data-lane={lane.key}
      onDragOver={over}
      onDragLeave={left}
      onDrop={dropped}
    >
      <h2 id={titleId} className="board-column-title">
        <bdi>{lane.title}</bdi>
        <span className="board-column-count">
          <span className="sr-only">{countSaid[0]}</span>
          {lane.count}
          <span className="sr-only">{countSaid[1]}</span>
        </span>
      </h2>
      {note}
      <ul className="board-list">{children}</ul>
      {dropAtEnd && <div className="board-drop-end" />}
      {!expanded && lane.rows.length > COLUMN_SHOWN && (
        <button
          type="button"
          className="button button-quiet"
          onClick={() => {
            onShowAll(lane);
          }}
        >
          {t("board.column.showAll", { count: lane.rows.length })}
        </button>
      )}
    </section>
  );
}
