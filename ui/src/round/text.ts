// The round's fixed words and caps (`docs/canon/round.md` "Sheet"): the owner's own text is counted
// in UTF-8 bytes against the stage's caps, never cut (`docs/canon/decision-staging.md` "The stage").
// Agent text is shown through `ui/text.ts`, which the Inbox and Health share; nothing outside
// `src/round/` imports this module.

/** The command that confirms the staged answers on a terminal, one `[y/N]` each: fixed words only. */
export const ROUND_COMMAND = "spec round answer --staged";

/** The caps of a stage's texts, in UTF-8 bytes. */
export const ANSWER_MAX_BYTES = 2048;
export const REASON_MAX_BYTES = 4096;

/** The most items one sheet holds (`--limit`'s range). */
export const ROUND_LIMIT_MAX = 200;

const ENCODER = new TextEncoder();

/** A text's length in UTF-8 bytes, which the stage's caps count (not UTF-16 units, not characters). */
export function utf8Bytes(text: string): number {
  return ENCODER.encode(text).length;
}
