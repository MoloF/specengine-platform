import type { MouseEvent, ReactNode, Ref } from "react";
import { openDocFrom } from "../app/location";
import { docPageHash } from "../app/routes";

/** A plain click: the primary button, no modifier, not taken by anyone before. Any other is the browser's. */
function isPlainClick(event: MouseEvent<HTMLAnchorElement>): boolean {
  return event.button === 0 && !event.defaultPrevented && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey;
}

/**
 * A link to a document (docs/features/doc-modal.md "Acceptance criteria", DM-7): its `href` the
 * document's page, so a Cmd, Ctrl, Shift or middle click and the context menu open it as the browser
 * does; a plain click opens the document modal over the screen instead, one history entry, or
 * replaces the open one's document, and records this link as the place focus comes back to. Enter on
 * the focused link is a plain click.
 */
export function DocLink({
  project,
  docRef,
  children,
  className,
  tabIndex,
  draggable,
  ref,
}: {
  project: string;
  docRef: string;
  children: ReactNode;
  className?: string;
  tabIndex?: number;
  draggable?: boolean;
  ref?: Ref<HTMLAnchorElement>;
}) {
  return (
    <a
      ref={ref}
      className={className}
      href={docPageHash(project, docRef)}
      tabIndex={tabIndex}
      draggable={draggable}
      onClick={(event) => {
        if (isPlainClick(event)) {
          event.preventDefault();
          openDocFrom(docRef, event.currentTarget);
        }
      }}
    >
      {children}
    </a>
  );
}
