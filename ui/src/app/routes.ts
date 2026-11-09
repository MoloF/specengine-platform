import type { Translate } from "../i18n";
import { isSectionId, sectionLabel, SECTIONS, type SectionId } from "./sections";

/**
 * Where the hash points: `#/<project>` (its home), `#/<project>/<section>[/<id>]`, `#/` (replaced
 * by the first project's home) or nowhere known.
 */
export type Route =
  | { type: "home" }
  | { type: "project"; project: string }
  | {
      type: "section";
      project: string;
      section: SectionId;
      id: string | null;
      /** Flow's alone: what follows the first raw `?`, undecoded; absent with none (docs/features/writer-studio.md D4). */
      query?: string;
    }
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
  // Flow's place follows the first raw `?`; any other section keeps a `?` in its name.
  const mark = path.indexOf("?");
  if (mark >= 0) {
    const route = parsePath(path.slice(0, mark));
    if (route.type === "section" && route.section === "flow") {
      return { ...route, query: path.slice(mark + 1) };
    }
  }
  return parsePath(path);
}

function parsePath(path: string): Route {
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

/** An arrow of a flow read by its served key: no fourth part, `kind` null. */
export interface FlowEdgeKey {
  from: string;
  to: string;
  type: string;
  kind: string | null;
}

/**
 * Where Flow stands (docs/features/writer-studio.md D4): the module, the scope, the stage and what is
 * selected. The hash is the store; this is its reading.
 */
export interface FlowPlace {
  /** null: not in the hash, the viewer's stored choice; `""`: All modules. */
  module: string | null;
  /** null: the lens root (D3). */
  scope: string | null;
  /** null: All stages; `-`: No stage. */
  stage: string | null;
  select: { node: string } | { edge: FlowEdgeKey } | null;
}

/** The lens root, all stages, nothing selected, the stored module. */
export const LENS_ROOT: FlowPlace = { module: null, scope: null, stage: null, select: null };

const FLOW_KEYS = ["module", "scope", "stage", "edge"] as const;

type FlowKey = (typeof FLOW_KEYS)[number];

function isFlowKey(key: string): key is FlowKey {
  return (FLOW_KEYS as readonly string[]).includes(key);
}

/** An arrow's key in the hash: each part encoded, joined by a raw `,`, the kind last when it has one. */
function edgeValue({ from, to, type, kind }: FlowEdgeKey): string {
  return (kind === null ? [from, to, type] : [from, to, type, kind]).map(encodeURIComponent).join(",");
}

/**
 * `#/<p>/flow[/<name>][?module=&scope=&stage=&edge=]`: the name the path segment, as `sectionHash`
 * writes it; then each key that is not null, in that order, its value encoded.
 */
export function flowHash(project: string, place: FlowPlace): string {
  const name = place.select !== null && "node" in place.select ? place.select.node : null;
  const pairs: string[] = [];
  if (place.module !== null) {
    pairs.push(`module=${encodeURIComponent(place.module)}`);
  }
  if (place.scope !== null) {
    pairs.push(`scope=${encodeURIComponent(place.scope)}`);
  }
  if (place.stage !== null) {
    pairs.push(`stage=${encodeURIComponent(place.stage)}`);
  }
  if (place.select !== null && "edge" in place.select) {
    pairs.push(`edge=${edgeValue(place.select.edge)}`);
  }
  const base = sectionHash(project, "flow", name);
  return pairs.length === 0 ? base : `${base}?${pairs.join("&")}`;
}

/** An arrow's key read back: three or four parts, none empty; else nothing. */
function edgeOf(value: string): FlowEdgeKey | null {
  const parts = value.split(",").map(decode);
  if (parts.length < 3 || parts.length > 4 || parts.some((part) => part === null || part === "")) {
    return null;
  }
  const [from = "", to = "", type = "", kind] = parts as string[];
  return { from, to, type, kind: kind ?? null };
}

/**
 * The place a Flow route names. Each value is decoded with `decodeURIComponent` alone (a `+` is no
 * space). A key drops when its value does not decode, when it is unknown, when `scope` or `stage` is
 * empty, or when `edge` is not three or four parts, none empty; the first of a repeated key wins, and
 * a name wins over an `edge`.
 */
export function parseFlowPlace(route: { id: string | null; query?: string }): FlowPlace {
  const found = new Map<Exclude<FlowKey, "edge">, string>();
  let edge: FlowEdgeKey | undefined;
  for (const pair of (route.query ?? "").split("&")) {
    const at = pair.indexOf("=");
    const key = at < 0 ? pair : pair.slice(0, at);
    const raw = pair.slice(at + 1);
    if (at < 0 || !isFlowKey(key)) {
      continue;
    }
    if (key === "edge") {
      edge ??= edgeOf(raw) ?? undefined;
      continue;
    }
    const value = decode(raw);
    if (found.has(key) || value === null || (value === "" && key !== "module")) {
      continue;
    }
    found.set(key, value);
  }
  const select = route.id !== null ? { node: route.id } : edge === undefined ? null : { edge };
  return { module: found.get("module") ?? null, scope: found.get("scope") ?? null, stage: found.get("stage") ?? null, select };
}

/** A route of Flow with no name and no `?`: the nav's link, which the project's last place stands in for. */
export function isBareFlow(route: { id: string | null; query?: string }): boolean {
  return route.id === null && route.query === undefined;
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
