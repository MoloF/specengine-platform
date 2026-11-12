// The raw sizes the scale's guard lets through (docs/features/design-foundation.md "Exceptions"):
// each a declaration of a style file under `ui/src`, as written, with why it is not a token and the
// plan item that removes it; `null` for the two kept for good. src/styles/scale.test.ts fails on any
// other raw font size, line height, space or radius, and on an entry here that matches nothing.

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
  /** The item that removes it; null: kept. */
  until: "RM-0099" | null;
}

const APP = "src/styles/app.css";

/** Graph's canvas likewise. */
const GRAPH_LAYOUT = "Graph's canvas: sized against the hand-written layout's boxes";

/** An icon set against its line's first text by a fraction of the font, which a space step cannot follow. */
const ICON_ALIGNMENT = "an icon aligned to its line's first text in em, beside text of any size";

export const SCALE_EXCEPTIONS: readonly ScaleException[] = [
  // Kept.
  { file: APP, selector: ".sr-only", property: "margin", value: "-1px", reason: "the visually hidden pattern", until: null },
  { file: APP, selector: ".tab", property: "margin-bottom", value: "-1px", reason: "the selected tab's border laid over the list's: a border, not space", until: null },

  // Icons set in em (RM-0099).
  { file: APP, selector: ".waiting-note .icon", property: "margin-top", value: "0.2em", reason: ICON_ALIGNMENT, until: "RM-0099" },
  { file: APP, selector: ".sync-part .icon", property: "margin-top", value: "0.15em", reason: ICON_ALIGNMENT, until: "RM-0099" },
  { file: APP, selector: ".cut-note .icon", property: "margin-top", value: "0.15em", reason: ICON_ALIGNMENT, until: "RM-0099" },
  { file: APP, selector: ".palette-error .icon", property: "margin-top", value: "0.2em", reason: ICON_ALIGNMENT, until: "RM-0099" },

  // The palette (RM-0099).
  { file: APP, selector: ".palette", property: "margin-top", value: "min(10vh, 5rem)", reason: "the palette stands near the viewport's top, by its height", until: "RM-0099" },

  // Graph's canvas (RM-0099).
  { file: APP, selector: ".graph-box", property: "line-height", value: "1.3", reason: GRAPH_LAYOUT, until: "RM-0099" },
  { file: APP, selector: ".graph-edge-label", property: "font-size", value: "0.6875rem", reason: GRAPH_LAYOUT, until: "RM-0099" },
  { file: APP, selector: ".graph-edge-label", property: "line-height", value: "1.5", reason: GRAPH_LAYOUT, until: "RM-0099" },
];
