// Stored text from the queue or the code shown as the CLI prints it (`docs/canon/round.md` "Sheet",
// `docs/canon/health-code.md` "Text"): verbatim, never markdown; every character the queue escapes
// written `\u{..}` (lower-case hex, as `escape_controls` writes it), U+2028 and U+2029 too, so a
// stored U+202E reorders nothing around it. A multi-line text keeps its line feeds and tabs; a
// one-line slot (an option's label, effect, price; a title) writes LF and CR as `\n` and `\r`.
// Shared by the Inbox, the Questions screen and Health; the round's caps stay in `round/text.ts`.

/**
 * A character the CLI escapes: C0 controls but LF and TAB (CR included), DEL, C1, the
 * bidirectional marks, embeddings, overrides and isolates (core `record::is_escaped`), and the line
 * and paragraph separators.
 */
const ESCAPED = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069\u2028\u2029]/gu;

function escaped(char: string): string {
  return `\\u{${(char.codePointAt(0) ?? 0).toString(16)}}`;
}

/** A multi-line text as the CLI prints it: line feeds and tabs kept, every other control `\u{..}`. */
export function shownText(text: string): string {
  return text.replace(ESCAPED, escaped);
}

/** A one-line slot as the CLI prints it: LF `\n`, CR `\r`, then as shownText. */
export function shownSlot(text: string): string {
  return shownText(text.replaceAll("\n", "\\n").replaceAll("\r", "\\r"));
}
