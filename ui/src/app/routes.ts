import type { Translate } from "../i18n";
import { isSectionId, sectionLabel, SECTIONS, type SectionId } from "./sections";

/**
 * Where the hash points: `#/<project>` (its home), `#/<project>/<section>[/<id>]`, a page
 * `#/<project>/board/<C>/page` or `#/<project>/tree/<REF>/page`, `#/` (replaced by the first
 * project's home) or nowhere known. A project or section route may carry the document modal, the
 * `doc` pair after the first raw `?` (docs/features/doc-modal.md "Acceptance criteria", DM-5).
 */
export type Route =
  | { type: "home" }
  | {
      type: "project";
      project: string;
      /** The document modal's REF, decoded; absent with none. */
      doc?: string;
    }
  | {
      type: "section";
      project: string;
      section: SectionId;
      id: string | null;
      /** Flow's alone: what follows the first raw `?`, undecoded, less every `doc` pair; absent with none (docs/features/writer-studio.md D4). */
      query?: string;
      /** A card's page (`board/<C>/page`) or a document's (`tree/<REF>/page`), never a tenth section (`ui/README.md` "Screen rules", Shell). */
      view?: "page";
      /** The document modal's REF, decoded; absent with none. */
      doc?: string;
    }
  | { type: "not_found" };

/** The fourth segment of a page: a card's or a document's. */
const PAGE = "page";

/** The sections whose item has a page of its own: a card's, a document's. */
const PAGED: readonly SectionId[] = ["board", "tree"];

/** The hash key of the document modal. */
const DOC_KEY = "doc";

function decode(segment: string): string | null {
  try {
    return decodeURIComponent(segment);
  } catch {
    return null;
  }
}

/** Whether a `key=value` pair (or a bare key) of the query is a `doc` pair. */
function isDocPair(pair: string): boolean {
  const at = pair.indexOf("=");
  return (at < 0 ? pair : pair.slice(0, at)) === DOC_KEY;
}

/** The query's pairs split: the first `doc` whose value decodes to a non-empty REF, and every other pair as written. */
function splitDoc(query: string): { doc: string | null; rest: string[] } {
  let doc: string | null = null;
  const rest: string[] = [];
  for (const pair of query.split("&")) {
    if (!isDocPair(pair)) {
      rest.push(pair);
      continue;
    }
    const at = pair.indexOf("=");
    const value = at < 0 ? null : decode(pair.slice(at + 1));
    if (doc === null && value !== null && value !== "") {
      doc = value;
    }
  }
  return { doc, rest };
}

/**
 * Reads a hash. The first raw `?` ends the path on every route; what follows is pairs. The first
 * `doc` pair that decodes to a non-empty REF is the modal on a project or section route; every `doc`
 * pair is removed before Flow takes the rest as its query, absent when only `doc` pairs stood there.
 * Other sections ignore other pairs.
 */
export function parseHash(hash: string): Route {
  const raw = hash.startsWith("#") ? hash.slice(1) : hash;
  const mark = raw.indexOf("?");
  const route = parsePath(mark < 0 ? raw : raw.slice(0, mark));
  if (mark < 0 || (route.type !== "project" && route.type !== "section")) {
    return route;
  }
  const { doc, rest } = splitDoc(raw.slice(mark + 1));
  const withQuery = route.type === "section" && route.section === "flow" && rest.length > 0 ? { ...route, query: rest.join("&") } : route;
  return doc === null ? withQuery : { ...withQuery, doc };
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
  if (section === null || !isSectionId(section) || id === null) {
    return { type: "not_found" };
  }
  if (rest.length > 0) {
    // A page: exactly `board`, `<C>`, `page` or `tree`, `<REF>`, `page`; any other fourth segment names nothing.
    return PAGED.includes(section) && id !== undefined && id !== "" && rest.length === 1 && rest[0] === PAGE ? { type: "section", project, section, id, view: PAGE } : { type: "not_found" };
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

/** A card's page, `#/<p>/board/<C>/page`: the card alone, its modal's body and actions, no columns. */
export function cardPageHash(project: string, id: string): string {
  return `${sectionHash(project, "board", id)}/${PAGE}`;
}

/** A document's page, `#/<p>/tree/<REF>/page`: the document view alone, the Spec tree marked in the nav. */
export function docPageHash(project: string, ref: string): string {
  return `${sectionHash(project, "tree", ref)}/${PAGE}`;
}

/** A hash split at its first raw `?`: the part before, and the pairs after (null with no `?`). */
function hashParts(hash: string): { base: string; pairs: string[] | null } {
  const mark = hash.indexOf("?");
  return mark < 0 ? { base: hash, pairs: null } : { base: hash.slice(0, mark), pairs: hash.slice(mark + 1).split("&") };
}

/** The hash with the document modal on `ref`: every `doc` pair removed, `doc=<ref>` appended after `?`, or `&` when pairs remain. */
export function withDoc(hash: string, ref: string): string {
  const { base, pairs } = hashParts(hash);
  const kept = (pairs ?? []).filter((pair) => !isDocPair(pair));
  return `${base}?${[...kept, `${DOC_KEY}=${encodeURIComponent(ref)}`].join("&")}`;
}

/** The hash without the document modal: every `doc` pair removed, and the `?` when none remains. */
export function withoutDoc(hash: string): string {
  const { base, pairs } = hashParts(hash);
  if (pairs === null) {
    return hash;
  }
  const kept = pairs.filter((pair) => !isDocPair(pair));
  return kept.length === 0 ? base : `${base}?${kept.join("&")}`;
}

/** The document a hash opens in the modal: `parseHash`'s `doc`, else null. */
export function docOf(hash: string): string | null {
  const route = parseHash(hash);
  return route.type === "project" || route.type === "section" ? (route.doc ?? null) : null;
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
