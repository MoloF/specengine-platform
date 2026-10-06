import { isSectionId, SECTIONS, type SectionId } from "./sections";

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

/** The home's label in the nav and the palette. */
export const OVERVIEW_LABEL = "Overview";

/** One entry of the nav: the home, then the six sections. */
export interface NavEntry {
  key: "overview" | SectionId;
  label: string;
  hash: string;
}

/** The nav in order: Overview first, then the six sections (07 §3 "Web UI — screens"). */
export function navOf(project: string): NavEntry[] {
  return [
    { key: "overview", label: OVERVIEW_LABEL, hash: homeHash(project) },
    ...SECTIONS.map((section) => ({ key: section.id, label: section.label, hash: sectionHash(project, section.id) })),
  ];
}
