// The raw sizes the scale's guard lets through (docs/features/design-foundation.md "Exceptions"):
// each a declaration of a style file under `ui/src`, as written, with why it is not a token; both
// are kept for good (docs/features/design-sweep-b.md "Acceptance criteria", DB7), and a new entry is
// a kept one only. src/styles/scale.test.ts fails on any other raw font size, line height, space or
// radius, and on an entry here that matches nothing.

export interface ScaleException {
  /** The style file, from `ui/`: "src/styles/app.css". */
  file: string;
  /** The rule's selector, whitespace collapsed. */
  selector: string;
  /** "font-size", "line-height", "gap", "margin-bottom", "border-radius", ... */
  property: string;
  /** As written. */
  value: string;
  /** Why it is not a token. */
  reason: string;
  /** Kept: no item removes it. */
  until: null;
}

const APP = "src/styles/app.css";

export const SCALE_EXCEPTIONS: readonly ScaleException[] = [
  { file: APP, selector: ".sr-only", property: "margin", value: "-1px", reason: "the visually hidden pattern", until: null },
  { file: APP, selector: ".tab", property: "margin-bottom", value: "-1px", reason: "the selected tab's border laid over the list's: a border, not space", until: null },
];
