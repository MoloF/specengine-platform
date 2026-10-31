import { useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import { Icon, type IconName } from "../ui/Icon";

/** One line of the menu. */
export interface MoveChoice {
  key: string;
  label: string;
  /** Who a move is for: "needs <names>". */
  detail: string | null;
  icon: IconName;
}

/** The gap kept between the menu and the window's edge, and its Move button. */
const MARGIN = 8;

/** Where the menu stands: under its button, kept inside the window; above it when there is no room below. */
function placed(anchor: DOMRect, menu: DOMRect): { left: number; top: number } {
  const left = Math.max(MARGIN, Math.min(anchor.left, window.innerWidth - menu.width - MARGIN));
  const below = anchor.bottom + MARGIN / 2;
  const room = below + menu.height + MARGIN <= window.innerHeight;
  return { left, top: room ? below : Math.max(MARGIN, anchor.top - menu.height - MARGIN / 2) };
}

/**
 * The Move menu (docs/features/board-ui.md "Moving"): its choices in the order given, keys alone or
 * pointer. Up and Down (wrapping), Home and End move among them, Enter or Space takes one, Esc or Tab
 * closes with nothing done, as a click off it does. Fixed to the window, so no column clips it; it
 * lists and never judges: the daemon answers what is chosen.
 */
export function MoveMenu<Choice extends MoveChoice>({
  label,
  anchor,
  choices,
  empty,
  onChoose,
  onClose,
}: {
  label: string;
  anchor: DOMRect;
  choices: readonly Choice[];
  /** Said when there is nothing to choose. */
  empty: string;
  onChoose: (choice: Choice) => void;
  onClose: () => void;
}) {
  const menu = useRef<HTMLDivElement>(null);
  const [place, setPlace] = useState<{ left: number; top: number }>({ left: anchor.left, top: anchor.bottom });

  useLayoutEffect(() => {
    const root = menu.current;
    if (root === null) {
      return;
    }
    setPlace(placed(anchor, root.getBoundingClientRect()));
    (root.querySelector<HTMLElement>('[role="menuitem"]') ?? root).focus();
  }, [anchor]);

  function items(): HTMLElement[] {
    return Array.from(menu.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? []);
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    // The board's keys never hear the menu's.
    event.stopPropagation();
    if (event.key === "Escape" || event.key === "Tab") {
      event.preventDefault();
      onClose();
      return;
    }
    const all = items();
    const at = all.findIndex((item) => item === document.activeElement);
    let next: number;
    switch (event.key) {
      case "ArrowDown":
        next = (at + 1) % all.length;
        break;
      case "ArrowUp":
        next = (at - 1 + all.length) % all.length;
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = all.length - 1;
        break;
      case "Enter":
      case " ":
        event.preventDefault();
        if (choices[at] !== undefined) {
          onChoose(choices[at]);
        }
        return;
      default:
        return;
    }
    event.preventDefault();
    all[next]?.focus();
  }

  return (
    <>
      <div className="move-menu-scrim" data-menu-scrim="" onClick={onClose} />
      <div ref={menu} className="move-menu" role="menu" aria-label={label} tabIndex={-1} style={place} onKeyDown={onKeyDown}>
        {choices.length === 0 && <p className="move-menu-empty">{empty}</p>}
        {choices.map((choice) => (
          <button
            key={choice.key}
            type="button"
            role="menuitem"
            tabIndex={-1}
            className="move-menu-item"
            onClick={() => {
              onChoose(choice);
            }}
          >
            <Icon name={choice.icon} />
            <span className="move-menu-label">
              <bdi>{choice.label}</bdi>
            </span>
            {choice.detail !== null && <span className="move-menu-detail">{choice.detail}</span>}
          </button>
        ))}
      </div>
    </>
  );
}
