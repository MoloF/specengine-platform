/** Whether a key event comes from a place that takes typed text, where letter hotkeys must not act. */
export function isTextField(target: EventTarget | null): boolean {
  if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) {
    return true;
  }
  if (target instanceof HTMLInputElement) {
    return !["checkbox", "radio", "button", "submit", "reset", "range", "color", "file"].includes(target.type);
  }
  return target instanceof HTMLElement && target.isContentEditable;
}

/** Whether Ctrl, Alt or Cmd is held: single-key shortcuts act only without them (WCAG 2.1.4). */
export function hasModifier(event: { altKey: boolean; ctrlKey: boolean; metaKey: boolean }): boolean {
  return event.altKey || event.ctrlKey || event.metaKey;
}
