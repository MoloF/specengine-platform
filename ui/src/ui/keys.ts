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

const ASCII_LETTER = /^[A-Za-z]$/;
const LETTER_CODE = /^Key([A-Z])$/;

/** One character beyond ASCII: a letter of another script, which a Latin hotkey reads by its physical key. */
function isOneNonAscii(key: string): boolean {
  const characters = Array.from(key);
  return characters.length === 1 && (characters[0]?.codePointAt(0) ?? 0) > 0x7f;
}

/**
 * The letter a Latin layout gives (docs/features/ui-languages.md "Keys (`src/ui/keys.ts`)", D11):
 * `key` when one ASCII letter; when one non-ASCII character, `KeyA`..`KeyZ`'s letter, upper case
 * with Shift; else null. Under a Russian layout the physical key acts; a Dvorak or AZERTY layout
 * keeps its own letters, and ASCII punctuation on a letter key (AZERTY's `,` on KeyM) is no letter.
 */
export function letterOf(event: Pick<KeyboardEvent, "key" | "code" | "shiftKey">): string | null {
  if (ASCII_LETTER.test(event.key)) {
    return event.key;
  }
  if (!isOneNonAscii(event.key)) {
    return null;
  }
  const letter = LETTER_CODE.exec(event.code)?.[1];
  if (letter === undefined) {
    return null;
  }
  return event.shiftKey ? letter : letter.toLowerCase();
}

/** The key a hotkey compares: a letter by `letterOf`, any other key (`?`, Escape, Enter, Space, the arrows, Home, End) as `key` names it. */
export function hotkeyOf(event: Pick<KeyboardEvent, "key" | "code" | "shiftKey">): string {
  return letterOf(event) ?? event.key;
}
