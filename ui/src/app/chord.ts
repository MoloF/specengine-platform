// The palette's chord, Cmd-K or Ctrl-K (ui/README.md "Screen rules", Keyboard): the one key that
// acts anywhere, a text field included. The shell's single `document` keydown listener (capture)
// reads it; every other key stays with the region that has focus.
import { letterOf } from "../ui/keys";

/** The key fields the chord is read from. */
export type ChordEvent = Pick<KeyboardEvent, "key" | "code" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey" | "isComposing">;

/**
 * Cmd or Ctrl with K, without Alt or Shift, outside an IME composition. K is the letter `letterOf`
 * reads: the `key` when it is one ASCII letter; on a layout whose `key` is not (Cyrillic), the
 * physical key `KeyK`. A Latin layout where `KeyK` types another letter (Dvorak: Cmd-T) is that
 * letter, not K.
 */
export function isJumpChord(event: ChordEvent): boolean {
  if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey || event.isComposing) {
    return false;
  }
  return letterOf(event)?.toLowerCase() === "k";
}

/** The chord as `aria-keyshortcuts` names it. */
export const JUMP_KEYSHORTCUTS = "Meta+K Control+K";
