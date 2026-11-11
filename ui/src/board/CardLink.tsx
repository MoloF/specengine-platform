import type { MouseEvent } from "react";
import { cardPageHash, sectionHash } from "../app/routes";
import { shownSlot } from "../ui/text";

// A link from a work item to another card (CardPanel.tsx, CardProgress.tsx): to its page or to it on
// the board, a plain click following it in place.

/** How a link to another card goes: to its page or to it on the board, and what a plain click does instead (the modal replaces its card). */
export interface CardLinks {
  project: string;
  page: boolean;
  onFollow: (id: string) => void;
}

/** A plain click with the primary button: a modified one, or another button, is the browser's. */
function isPlainClick(event: MouseEvent<HTMLElement>): boolean {
  return event.button === 0 && !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
}

/** A link to another card; the ID as stored. A plain click follows it in place, any other is the browser's. */
export function CardLink({ id, links }: { id: string; links: CardLinks }) {
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
