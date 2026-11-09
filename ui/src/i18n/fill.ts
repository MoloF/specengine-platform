// A catalog sentence holding elements (docs/features/ui-languages.md "The catalog, `ui/src/i18n/`":
// a value holds no markup). A placeholder that stands for an element (a link, a `<kbd>`, the
// daemon's words under `lang="en"`) is passed as `slot(name)`; `fill` cuts the translated text at
// each slot and puts the element there, so each language orders its words around it. Data never
// enters the template: a slot's content is a React node, shown as text.
import { createElement, Fragment, type ReactNode } from "react";

/** A character no catalog value holds: it brackets a slot's name in the translated text. */
const MARK = "\u0000";

/** The value a placeholder takes when an element stands in it. */
export function slot(name: string): string {
  return `${MARK}${name}${MARK}`;
}

/** The text with each slot replaced by its element; a slot not given stays as its name. */
export function fill(text: string, slots: Readonly<Record<string, ReactNode>>): ReactNode[] {
  return text.split(MARK).map((piece, index) => createElement(Fragment, { key: index }, index % 2 === 1 ? (slots[piece] ?? piece) : piece));
}

/**
 * The text before and after the first `value` in a translated sentence: a count drawn on its own,
 * the words around it read out (`, 3 cards`: `, ` and ` cards` spoken, `3` shown). Not found: the
 * whole text before.
 */
export function around(text: string, value: string): readonly [string, string] {
  const at = text.indexOf(value);
  return at < 0 ? [text, ""] : [text.slice(0, at), text.slice(at + value.length)];
}
