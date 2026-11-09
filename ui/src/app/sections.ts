import type { Translate } from "../i18n";

/**
 * The nine sections of the nav, in order (07 §3 "Web UI — screens"). Each one's label and what it
 * shows are the catalog's `app.sections.<id>` (docs/features/ui-languages.md "Description and interactions").
 */
export const SECTIONS = [
  { id: "inbox" },
  { id: "tasks" },
  { id: "board" },
  { id: "roadmap" },
  { id: "flow" },
  { id: "tree" },
  { id: "graph" },
  { id: "health" },
  { id: "questions" },
] as const;

export type SectionId = (typeof SECTIONS)[number]["id"];

/** A section's name in the nav, the palette and its view's heading. */
export function sectionLabel(id: SectionId, t: Translate): string {
  return t(`app.sections.${id}.label`);
}

/** What a section shows, as the nav's table words it. */
export function aboutOf(id: SectionId, t: Translate): string {
  return t(`app.sections.${id}.about`);
}

export function isSectionId(value: string): value is SectionId {
  return SECTIONS.some((section) => section.id === value);
}
