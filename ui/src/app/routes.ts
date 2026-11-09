import type { Translate } from "../i18n";
import { isSectionId, sectionLabel, SECTIONS, type SectionId } from "./sections";

/**
 * Where the hash points: `#/<project>` (its home), `#/<project>/<section>[/<id>]`, `#/` (replaced
 * by the first project's home) or nowhere known.
 */
export type Route =
  | { type: "home" }
  | { type: "project"; project: string }
  | { type: "section"; project: string; section: SectionId; id: string | null }
  | { type: "not_found" };

function decode(segment: string): string | null {
  try {
    return decodeURIComponent(segment);
  } catch {
    return null;
  }
}

export function parseHash(hash: string): Route {
  const path = hash.startsWith("#") ? hash.slice(1) : hash;
  if (path === "" || path === "/") {
    return { type: "home" };
  }
  if (!path.startsWith("/")) {
    return { type: "not_found" };
  }
  const segments = path.slice(1).split("/").map(decode);
  const [project, section, id, ...rest] = segments;
  if (project === undefined || project === null || project === "") {
    return { type: "not_found" };
  }
  // `#/<p>` and `#/<p>/`: the project's home.
  if (section === undefined || (section === "" && id === undefined)) {
    return { type: "project", project };
  }
  if (section === null || !isSectionId(section) || id === null || rest.length > 0) {
    return { type: "not_found" };
  }
  return { type: "section", project, section, id: id === undefined || id === "" ? null : id };
}

/** A project's home, `#/<p>`. */
export function homeHash(project: string): string {
  return `#/${encodeURIComponent(project)}`;
}

export function sectionHash(project: string, section: SectionId, id: string | null = null): string {
  const base = `${homeHash(project)}/${section}`;
  return id === null ? base : `${base}/${encodeURIComponent(id)}`;
}

/** One entry of the nav: the home, then a section. */
export interface NavEntry {
  key: "overview" | SectionId;
  label: string;
  hash: string;
}

/** The nine section IDs in `sections.ts` order: the palette's list, and the nav's when no workspace was read. */
export const ALL_SECTIONS: readonly SectionId[] = SECTIONS.map((section) => section.id);

/**
 * The nav in order: Overview first, then `sections` in their order (07 §3 "Web UI — screens"), each
 * drawn only if it is one of the nine and only once: a workspace's served list
 * (`docs/features/workspaces.md` "The UI"), or ALL_SECTIONS.
 */
export function navOf(project: string, sections: readonly string[], t: Translate): NavEntry[] {
  const shown: SectionId[] = [];
  for (const id of sections) {
    if (isSectionId(id) && !shown.includes(id)) {
      shown.push(id);
    }
  }
  return [
    { key: "overview", label: t("app.overview"), hash: homeHash(project) },
    ...shown.map((id) => ({ key: id, label: sectionLabel(id, t), hash: sectionHash(project, id) })),
  ];
}
