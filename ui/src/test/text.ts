// Fixed `t`s for tests (docs/features/ui-languages.md "Data", "The instance (`index.ts`)"): a label
// function called with `enT` gives today's English, with `ruT` the Russian of ru.ts, whatever the
// page shows. Tests reach Russian through these or ru.ts, never a Cyrillic literal (D13).
import { translateIn } from "../i18n";

export const enT = translateIn("en");
export const ruT = translateIn("ru");

/**
 * A text matcher over an element's whole text, the daemon's words in their own `lang="en"` element
 * included (docs/canon/ui-languages.md "Served text"): the innermost element whose text holds
 * `pattern`. Testing Library's plain text match reads an element's own text nodes only.
 */
export function sentence(pattern: RegExp): (content: string, element: Element | null) => boolean {
  return (_content, element) =>
    element !== null && pattern.test(element.textContent) && !Array.from(element.children).some((child) => pattern.test(child.textContent));
}
