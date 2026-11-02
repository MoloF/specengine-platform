// Stored text from the queue or the code shown as the CLI prints it (`docs/canon/round.md` "Sheet",
// `docs/canon/health-code.md` "Text"): verbatim, never markdown; every character the queue escapes
// written `\u{..}` (lower-case hex, as `escape_controls` writes it), U+2028 and U+2029 too, so a
// stored U+202E reorders nothing around it. A multi-line text keeps its line feeds and tabs; a
// one-line slot (an option's label, effect, price; a title) writes LF and CR as `\n` and `\r`.
// Shared by the Inbox, the Questions screen and Health; the round's caps stay in `round/text.ts`.
// A Confirm dialog escapes the format characters (Unicode Cf) too, the hub's confirm question's form.

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

/**
 * Unicode's general category Cf, the format characters, as a table so every browser reads one set:
 * the ranges of core `card.rs` `is_format_character` (the same in Unicode 15.1 and 17.0). U+200C
 * and U+200D are among them, so a confirm shows Persian, Indic and emoji text with their joiners
 * written out.
 */
const FORMAT =
  /[\u00ad\u0600-\u0605\u061c\u06dd\u070f\u0890-\u0891\u08e2\u180e\u200b-\u200f\u202a-\u202e\u2060-\u2064\u2066-\u206f\ufeff\ufff9-\ufffb\u{110bd}\u{110cd}\u{13430}-\u{1343f}\u{1bca0}-\u{1bca3}\u{1d173}-\u{1d17a}\u{e0001}\u{e0020}-\u{e007f}]/u;

/** What a confirm escapes: what the CLI escapes and every format character. */
const CONFIRM_ESCAPED = new RegExp(`${ESCAPED.source}|${FORMAT.source}`, "gu");

/**
 * A text a Confirm dialog shows (diff, record, question; Escaping in `docs/canon/hub-confirm.md` "Rules"):
 * as shownText, every format character (Cf: U+00AD, U+200B, U+2060, U+FEFF and the rest)
 * written `\u{..}` too, as the daemon's confirm question writes them, so nothing a person consents
 * to is invisible. A literal `\u{200b}` and the character read alike, as on a terminal. The
 * terminal's own escape, and so shownText, stays as it is.
 */
export function shownConfirmText(text: string): string {
  return text.replace(CONFIRM_ESCAPED, escaped);
}

/** A one-line slot of a Confirm dialog: LF `\n`, CR `\r`, then as shownConfirmText. */
export function shownConfirmSlot(text: string): string {
  return shownConfirmText(text.replaceAll("\n", "\\n").replaceAll("\r", "\\r"));
}
