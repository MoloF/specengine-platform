// The palette's chord, Cmd-K or Ctrl-K (docs/features/ui-home.md "Chord"): the one key that acts
// anywhere, a text field included. The shell's single `document` keydown listener (capture) reads
// it; every other key stays with the region that has focus.

/** The key fields the chord is read from. */
export type ChordEvent = Pick<KeyboardEvent, "key" | "code" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey" | "isComposing">;

/** One ASCII letter: a Latin layout's own K is read from `key`, never from the physical key. */
const ASCII_LETTER = /^[A-Za-z]$/;

/**
 * Cmd or Ctrl with K, without Alt or Shift, outside an IME composition. K is the `key`; on a layout
 * whose `key` is not one ASCII letter (Cyrillic), the physical key `KeyK` stands in. A Latin layout
 * where `KeyK` types another letter (Dvorak: Cmd-T) is that letter, not K.
 */
export function isJumpChord(event: ChordEvent): boolean {
  if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey || event.isComposing) {
    return false;
  }
  if (event.key.toLowerCase() === "k") {
    return true;
  }
  return event.code === "KeyK" && !ASCII_LETTER.test(event.key);
}

/** The chord as `aria-keyshortcuts` names it. */
export const JUMP_KEYSHORTCUTS = "Meta+K Control+K";
