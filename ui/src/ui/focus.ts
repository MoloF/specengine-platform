/**
 * Focus sits nowhere: the element that had it was removed (a Retry that succeeded, a radio of a
 * revised proposal), so the browser fell back to the body, where the queue's keys do not reach.
 */
export function focusIsLost(): boolean {
  return document.activeElement === null || document.activeElement === document.body;
}
