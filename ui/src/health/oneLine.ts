import { shownText } from "../ui/text";

/**
 * A field of rows 4-6 as `spec health` prints it (docs/canon/health-code.md "Text"): on one line, a
 * line break written as a space, every other control character and bidirectional mark as `\u{..}`,
 * so a stored text can neither break the row nor reorder what is around it. A leaf module: the
 * label maps (`health/labels.ts`, `tree/labels.ts`) write an unlisted word through it too, and
 * `codeRows.tsx`, which reads those maps, re-exports it.
 */
export function oneLine(text: string): string {
  return shownText(text.replaceAll(/\r\n|\r|\n/g, " "));
}
