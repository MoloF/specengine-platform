// Agent text on the Questions screen as the round sheet prints it (`docs/canon/round.md` "Sheet"):
// verbatim, never markdown; every character the queue escapes written `\u{..}` (lower-case hex, as
// `escape_controls` writes it), U+2028 and U+2029 too, so a stored U+202E reorders nothing around
// it. A multi-line text keeps its line feeds and tabs; a one-line slot (an option's label, effect,
// price; a title) writes LF and CR as `\n` and `\r`. The owner's own text is counted in UTF-8
// bytes against the stage's caps, never cut (`docs/canon/decision-staging.md` "The stage").

/** The command that confirms the staged answers on a terminal, one `[y/N]` each: fixed words only. */
export const ROUND_COMMAND = "spec round answer --staged";

/** The caps of a stage's texts, in UTF-8 bytes. */
export const ANSWER_MAX_BYTES = 2048;
export const REASON_MAX_BYTES = 4096;

/** The most items one sheet holds (`--limit`'s range). */
export const ROUND_LIMIT_MAX = 200;

/**
 * A character the sheet escapes: C0 controls but LF and TAB (CR included), DEL, C1, the
 * bidirectional marks, embeddings, overrides and isolates (core `record::is_escaped`), and the line
 * and paragraph separators.
 */
const ESCAPED = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069\u2028\u2029]/gu;

function escaped(char: string): string {
  return `\\u{${(char.codePointAt(0) ?? 0).toString(16)}}`;
}

/** A multi-line text as the sheet prints it: line feeds and tabs kept, every other control `\u{..}`. */
export function shownText(text: string): string {
  return text.replace(ESCAPED, escaped);
}

/** A one-line slot as the sheet prints it: LF `\n`, CR `\r`, then as shownText. */
export function shownSlot(text: string): string {
  return shownText(text.replaceAll("\n", "\\n").replaceAll("\r", "\\r"));
}

const ENCODER = new TextEncoder();

/** A text's length in UTF-8 bytes, which the stage's caps count (not UTF-16 units, not characters). */
export function utf8Bytes(text: string): number {
  return ENCODER.encode(text).length;
}
