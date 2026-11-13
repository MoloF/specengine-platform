import type { MapDocument } from "../api/types";
import { byCodePoint, slotOf, type KindColours, type MapDirection } from "./colours";

// What the map draws and lists, over the served answer alone (docs/features/corpus-map.md
// "Acceptance criteria", CM8, CM9). Folding maps a section's end to its document (`of`) and merges
// what then repeats; it counts no use. The node set (the answer and the Sections switch) is laid
// out once; every other filter hides nodes and links and never moves one (D7). At most
// MAP_DRAWN_MAX nodes are drawn, the most used, ties in served order; the List lists all.

/** How many nodes the picture draws at most (D8). */
export const MAP_DRAWN_MAX = 5000;

/** How many rows the List shows at a time, and adds on each "Show more". */
export const MAP_LIST_PAGE = 200;

/** The switches and chips: Sections lays out again; every other one only hides. */
export interface MapFilters {
  /** Sections drawn apart, their links unfolded. */
  sections: boolean;
  /** The parent lines, dotted. */
  parents: boolean;
  /** The weak links, dashed and light; off by default, so a large corpus's mentions never bury its links. */
  mentions: boolean;
  /** Unresolved targets as ghost nodes. */
  ghosts: boolean;
  orphansOnly: boolean;
  /** A ghost's sources and the ghosts alone. */
  brokenOnly: boolean;
  /** The kind chips released, null for no kind. */
  released: ReadonlySet<string | null>;
}

export const DEFAULT_FILTERS: MapFilters = {
  sections: false,
  parents: false,
  mentions: false,
  ghosts: false,
  orphansOnly: false,
  brokenOnly: false,
  released: new Set(),
};

/** A link once folded and merged: its ends as served indexes of the node set. */
export interface FoldedEdge {
  src: number;
  dst: number;
  type: string;
  weak: boolean;
  places: number;
}

/** The answer for one Sections choice: the node set and its links, before the cap and any filter. */
export interface Folded {
  /** The served indexes in the set, in served order: the documents, and the sections when shown. */
  members: readonly number[];
  /** Each served index's place in the set: itself, else its document. */
  home: Int32Array;
  edges: readonly FoldedEdge[];
  parents: readonly { child: number; parent: number }[];
  /** Each ghost's sources, folded, each once, in served order. */
  ghosts: readonly (readonly number[])[];
  /** Each served index the source of how many ghosts: what the List marks "N broken". */
  broken: Int32Array;
}

/** Folds the answer for a Sections choice; duplicates merge with their places summed, an edge folded onto one node goes. */
export function foldMap(answer: MapDocument, sections: boolean): Folded {
  const count = answer.nodes.length;
  const home = new Int32Array(count);
  const members: number[] = [];
  answer.nodes.forEach((node, index) => {
    const of = sections ? null : node.of;
    home[index] = of ?? index;
    if (of === null) {
      members.push(index);
    }
  });
  const merged = new Map<string, FoldedEdge>();
  for (const edge of answer.edges) {
    const src = home[edge.src] ?? edge.src;
    const dst = home[edge.dst] ?? edge.dst;
    if (src === dst) {
      continue;
    }
    const key = `${String(src)}\u0000${String(dst)}\u0000${edge.type}`;
    const held = merged.get(key);
    if (held === undefined) {
      merged.set(key, { src, dst, type: edge.type, weak: edge.weak, places: edge.places });
    } else {
      held.places += edge.places;
    }
  }
  const parentKeys = new Set<string>();
  const parents: { child: number; parent: number }[] = [];
  for (const line of answer.parents) {
    const child = home[line.child] ?? line.child;
    const parent = home[line.parent] ?? line.parent;
    const key = `${String(child)}:${String(parent)}`;
    if (child !== parent && !parentKeys.has(key)) {
      parentKeys.add(key);
      parents.push({ child, parent });
    }
  }
  const broken = new Int32Array(count);
  const ghosts = answer.ghosts.map((ghost) => {
    const sources = [...new Set(ghost.sources.map((source) => home[source] ?? source))].sort((a, b) => a - b);
    for (const source of sources) {
      broken[source] = (broken[source] ?? 0) + 1;
    }
    return sources;
  });
  return { members, home, edges: [...merged.values()], parents, ghosts, broken };
}

/** The members the picture draws: all, or past MAP_DRAWN_MAX the most used, ties in served order, kept in served order. */
export function drawnOf(answer: MapDocument, members: readonly number[], max: number = MAP_DRAWN_MAX): number[] {
  if (members.length <= max) {
    return [...members];
  }
  const used = (index: number) => answer.nodes[index]?.used_by ?? 0;
  return [...members]
    .sort((a, b) => used(b) - used(a) || a - b)
    .slice(0, max)
    .sort((a, b) => a - b);
}

/** A drawn line's look: a strong link solid, a mention dashed, a parent dotted, a ghost's link dashed in the ghost's colour. */
export type LineStyle = "strong" | "weak" | "parent" | "ghost";

/** A point of the picture: a served node, or a ghost by its place in `ghosts`. */
export type ScenePoint = { node: number; ghost: null } | { node: null; ghost: number };

/** What is laid out and drawn: the points, the lines between them by point, each point's lines. */
export interface Scene {
  folded: Folded;
  points: readonly ScenePoint[];
  /** A served index's point, -1 when not drawn (capped out, or a section folded away). */
  pointOf: Int32Array;
  lineFrom: Int32Array;
  lineTo: Int32Array;
  lineStyle: readonly LineStyle[];
  /** Each point's lines by index, both ways. */
  incident: readonly (readonly number[])[];
  /** The node set's size before the cap, and how many of it are not drawn. */
  total: number;
  hidden: number;
}

/** The scene of an answer for a Sections choice: the drawn members, then each ghost with a drawn source; their lines. */
export function sceneOf(answer: MapDocument, sections: boolean, max: number = MAP_DRAWN_MAX): Scene {
  const folded = foldMap(answer, sections);
  const drawn = drawnOf(answer, folded.members, max);
  const pointOf = new Int32Array(answer.nodes.length).fill(-1);
  const points: ScenePoint[] = drawn.map((node, at) => {
    pointOf[node] = at;
    return { node, ghost: null };
  });
  const from: number[] = [];
  const to: number[] = [];
  const styles: LineStyle[] = [];
  const line = (a: number, b: number, style: LineStyle) => {
    if (a >= 0 && b >= 0) {
      from.push(a);
      to.push(b);
      styles.push(style);
    }
  };
  for (const edge of folded.edges) {
    line(pointOf[edge.src] ?? -1, pointOf[edge.dst] ?? -1, edge.weak ? "weak" : "strong");
  }
  for (const parent of folded.parents) {
    line(pointOf[parent.child] ?? -1, pointOf[parent.parent] ?? -1, "parent");
  }
  folded.ghosts.forEach((sources, ghost) => {
    const drawnSources = sources.map((source) => pointOf[source] ?? -1).filter((point) => point >= 0);
    if (drawnSources.length === 0) {
      return;
    }
    const point = points.length;
    points.push({ node: null, ghost });
    for (const source of drawnSources) {
      line(source, point, "ghost");
    }
  });
  const incident: number[][] = points.map(() => []);
  from.forEach((a, index) => {
    incident[a]?.push(index);
    incident[to[index] ?? a]?.push(index);
  });
  return {
    folded,
    points,
    pointOf,
    lineFrom: Int32Array.from(from),
    lineTo: Int32Array.from(to),
    lineStyle: styles,
    incident,
    total: folded.members.length,
    hidden: folded.members.length - drawn.length,
  };
}

/** Whether the node filters keep a served node: its kind's chip pressed, an orphan under Orphans only, a ghost's source under Broken only. */
export function keepsNode(answer: MapDocument, folded: Folded, index: number, filters: MapFilters): boolean {
  const node = answer.nodes[index];
  if (node === undefined) {
    return false;
  }
  const { kind } = node;
  if (filters.released.has(kind)) {
    return false;
  }
  if (filters.orphansOnly && node.orphan !== true) {
    return false;
  }
  return !filters.brokenOnly || (folded.broken[index] ?? 0) > 0;
}

/** What the filters keep of a scene, per point and per line, 1 kept; never a position (D7). */
export interface Kept {
  points: Uint8Array;
  lines: Uint8Array;
}

/** Whether the filters show ghosts at all: Unresolved targets or Broken only on, Orphans only off. */
function showsGhosts(filters: MapFilters): boolean {
  return (filters.ghosts || filters.brokenOnly) && !filters.orphansOnly;
}

export function keptOf(answer: MapDocument, scene: Scene, filters: MapFilters): Kept {
  const points = new Uint8Array(scene.points.length);
  const ghostsShown = showsGhosts(filters);
  scene.points.forEach((point, at) => {
    if (point.node !== null) {
      points[at] = keepsNode(answer, scene.folded, point.node, filters) ? 1 : 0;
    }
  });
  const lines = new Uint8Array(scene.lineStyle.length);
  scene.lineStyle.forEach((style, at) => {
    const shown = style === "weak" ? filters.mentions : style === "parent" ? filters.parents : style === "ghost" ? ghostsShown : true;
    // A ghost's line needs its source kept; the ghost is kept below once one of its lines is.
    const a = scene.lineFrom[at] ?? 0;
    const b = scene.lineTo[at] ?? 0;
    lines[at] = shown && points[a] === 1 && (style === "ghost" || points[b] === 1) ? 1 : 0;
  });
  scene.points.forEach((point, at) => {
    if (point.ghost !== null) {
      points[at] = ghostsShown && (scene.incident[at] ?? []).some((line) => lines[line] === 1) ? 1 : 0;
    }
  });
  return { points, lines };
}

/**
 * What a hover or a lit match lights: the point, its kept neighbours both ways and the kept lines
 * between, as flags and as lists, so a highlight walks only its own points and lines (CM12).
 */
export interface Lit {
  points: Uint8Array;
  lines: Uint8Array;
  pointList: readonly number[];
  lineList: readonly number[];
}

export function litOf(scene: Scene, kept: Kept, point: number): Lit {
  const points = new Uint8Array(scene.points.length);
  const lines = new Uint8Array(scene.lineStyle.length);
  const pointList: number[] = [];
  const lineList: number[] = [];
  const light = (at: number) => {
    if (points[at] !== 1) {
      points[at] = 1;
      pointList.push(at);
    }
  };
  light(point);
  for (const line of scene.incident[point] ?? []) {
    if (kept.lines[line] !== 1) {
      continue;
    }
    lines[line] = 1;
    lineList.push(line);
    light(scene.lineFrom[line] ?? point);
    light(scene.lineTo[line] ?? point);
  }
  return { points, lines, pointList, lineList };
}

/**
 * The points of a laid scene that hold the named nodes, by name, every holder of a shared name, in
 * served order: the picture may still be an older answer's while a new one lays out, and a newer
 * answer's index never names a point of an older answer's scene.
 */
export function pointsByName(answer: MapDocument, scene: Scene, names: readonly string[]): number[] {
  const wanted = new Set(names);
  const points: number[] = [];
  answer.nodes.forEach((node, index) => {
    const point = scene.pointOf[index] ?? -1;
    if (point >= 0 && wanted.has(node.name)) {
      points.push(point);
    }
  });
  return points;
}

/** What a search lights: each matched point of the scene (pointsByName), no line. */
export function litMatches(scene: Scene, matched: readonly number[]): Lit {
  const points = new Uint8Array(scene.points.length);
  const pointList: number[] = [];
  for (const point of matched) {
    if (point >= 0 && point < points.length && points[point] !== 1) {
      points[point] = 1;
      pointList.push(point);
    }
  }
  return { points, lines: new Uint8Array(scene.lineStyle.length), pointList, lineList: [] };
}

/** Text as filters compare it (`ui/README.md` "Screen rules", Text). */
export function folded(text: string): string {
  return text.normalize("NFC").toLowerCase();
}

/** The served indexes of the node set the filters keep whose name or title holds the search, in served order; none for an empty search. */
export function matchesOf(answer: MapDocument, members: readonly number[], keep: (index: number) => boolean, query: string): number[] {
  const wanted = folded(query.trim());
  if (wanted === "") {
    return [];
  }
  return members.filter((index) => {
    const node = answer.nodes[index];
    return node !== undefined && keep(index) && (folded(node.name).includes(wanted) || folded(node.title ?? "").includes(wanted));
  });
}

/** One row of the List: a kept node, its links as drawn out and in, and its ghosts. */
export interface ListRow {
  index: number;
  out: number;
  in: number;
  broken: number;
}

/**
 * The List: every node of the set the filters keep, capped out or not, in served order; its links
 * the folded and merged ones shown (a mention only with Mentions on) whose other end is kept, each
 * counted once.
 */
export function listOf(answer: MapDocument, folded: Folded, filters: MapFilters): ListRow[] {
  const keep = new Uint8Array(answer.nodes.length);
  for (const index of folded.members) {
    keep[index] = keepsNode(answer, folded, index, filters) ? 1 : 0;
  }
  const out = new Int32Array(answer.nodes.length);
  const into = new Int32Array(answer.nodes.length);
  for (const edge of folded.edges) {
    if ((edge.weak && !filters.mentions) || keep[edge.src] !== 1 || keep[edge.dst] !== 1) {
      continue;
    }
    out[edge.src] = (out[edge.src] ?? 0) + 1;
    into[edge.dst] = (into[edge.dst] ?? 0) + 1;
  }
  return folded.members.filter((index) => keep[index] === 1).map((index) => ({ index, out: out[index] ?? 0, in: into[index] ?? 0, broken: folded.broken[index] ?? 0 }));
}

/**
 * The List's Broken links: the ghosts the picture would keep, capped out or not, in served order.
 * None unless Unresolved targets or Broken only is on and Orphans only off; then each ghost one of
 * whose sources, folded, the node filters keep.
 */
export function ghostsOf(answer: MapDocument, folded: Folded, filters: MapFilters): number[] {
  if (!showsGhosts(filters)) {
    return [];
  }
  return answer.ghosts.flatMap((_, ghost) => ((folded.ghosts[ghost] ?? []).some((source) => keepsNode(answer, folded, source, filters)) ? [ghost] : []));
}

/** One kind chip: the kind (null: no kind), its slot, how many nodes of the set hold it. */
export interface KindChip {
  kind: string | null;
  slot: number;
  count: number;
}

/** The kinds of a node set's nodes, in served order, null for none: what its colouring counts. */
export function kindsOf(answer: MapDocument, members: readonly number[]): (string | null)[] {
  return members.map((index) => answer.nodes[index]?.kind ?? null);
}

/**
 * A chip per kind the node set holds: the coloured kinds first, by slot (most held first), then the
 * rest in code-point order, no kind last (CM8).
 */
export function kindChips(answer: MapDocument, members: readonly number[], colours: KindColours): KindChip[] {
  const counts = new Map<string | null, number>();
  for (const kind of kindsOf(answer, members)) {
    counts.set(kind, (counts.get(kind) ?? 0) + 1);
  }
  const chips = [...counts].map(([kind, count]) => ({ kind, slot: slotOf(kind, colours), count }));
  return chips.sort((a, b) => {
    if (a.kind === null || b.kind === null) {
      return a.kind === null ? (b.kind === null ? 0 : 1) : -1;
    }
    if (a.slot !== b.slot) {
      return a.slot === 0 ? 1 : b.slot === 0 ? -1 : a.slot - b.slot;
    }
    return byCodePoint(a.kind, b.kind);
  });
}

/** The chips a direction presses: its kinds; every other chip released. All (null) presses every chip. */
export function releasedFor(chips: readonly KindChip[], direction: MapDirection | null): Set<string | null> {
  if (direction === null) {
    return new Set();
  }
  return new Set(chips.map((chip) => chip.kind).filter((kind) => kind === null || !direction.kinds.includes(kind)));
}
