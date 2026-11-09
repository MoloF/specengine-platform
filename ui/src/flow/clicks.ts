import type { MouseEvent } from "react";

/**
 * Whether a click on a link is the page's own: the primary button with no modifier. Cmd, Ctrl, Shift
 * or Alt open the link elsewhere, so the studio neither replaces the hash nor stores a choice for it.
 */
export function isPlainClick(event: MouseEvent): boolean {
  return event.button === 0 && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey;
}
