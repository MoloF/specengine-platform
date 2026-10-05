import { isSectionId, type SectionId } from "./sections";

/** Where the hash points: `#/<project>/<section>[/<id>]`, `#/` (home) or nowhere known. */
export type Route =
  | { type: "home" }
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
  if (
    project === undefined ||
    project === null ||
    project === "" ||
    section === undefined ||
    section === null ||
    !isSectionId(section) ||
    id === null ||
    rest.length > 0
  ) {
    return { type: "not_found" };
  }
  return { type: "section", project, section, id: id === undefined || id === "" ? null : id };
}

export function sectionHash(project: string, section: SectionId, id: string | null = null): string {
  const base = `#/${encodeURIComponent(project)}/${section}`;
  return id === null ? base : `${base}/${encodeURIComponent(id)}`;
}
