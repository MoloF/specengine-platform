import type { MapDocument, MapEdge, MapGhost, MapNode, MapParent } from "../api/types";
import type { MockCorpus } from "./build";
import { treeOf } from "./corpus";
import { HOSTILE_LINK } from "./harbor-sim/fixtures";
import type { Scenario } from "./scenario";

// The map as the daemon's `GET .../map` serves it
// (docs/features/corpus-map.md "Acceptance criteria", CM8, CM12): nodes in (path, ord) order, edges by (src, dst, type), parents by child,
// ghosts by what is written, the uses and the orphan flag counted here as core `usage_of` counts
// them, since the UI never does. harbor-sim: 30 nodes, 26 documents and 4 ID sections, over the
// two directions' kinds (`mechanic`, `rule`: src/mocks/workspaces.ts), an unlisted kind
// (`domain`) and no kind; 2 orphans and 3 ghosts, one of two sources, one a hostile written form.
// Its documents and sections are the corpus's own, read from its tree, so a click opens each, the
// lock keeper, the fog signal and the ID-less note on harbour cats included (src/mocks/harbor-sim/fixtures.ts).
// `large`: 5 000 nodes and 15 000 edges from a fixed generator, never `Math.random`. ledger-api:
// its tree's documents and parents, no link. `empty`: no node. Every word is invented.

/** A node of harbor-sim's map, by its name; its fields read from the corpus's tree. */
interface Picked {
  name: string;
  /** A section's document, by name; absent for a document. */
  of?: string;
}

/** harbor-sim's 26 live documents and 4 of its sections, by name. */
const PICKED: readonly Picked[] = [
  { name: "DEC-0014" },
  { name: "docs/features/deep-berth.md" },
  { name: "DOM-BERTHS" },
  { name: "RULE-BASIN-NAME", of: "DOM-BERTHS" },
  { name: "docs/spec/berths/draft-limits.md" },
  { name: "MEC-MOORING" },
  { name: "RULE-MOOR-CREW", of: "MEC-MOORING" },
  { name: "MEC-CARGO-CRANES" },
  { name: "MEC-FAIRWAY" },
  { name: "MEC-NIGHT-PASSAGE" },
  { name: "DOM-HARBOR" },
  { name: "RULE-HARBOR-CLOCK", of: "DOM-HARBOR" },
  { name: "MEC-LOCK-A" },
  { name: "MEC-LOCK-B" },
  { name: "MEC-LOCK-KEEPER" },
  { name: "docs/spec/notes/harbour-cats.md" },
  { name: "RULE-FOG-SIGNAL" },
  { name: "MEC-PILOTAGE" },
  { name: "DOM-PILOTS" },
  { name: "MEC-PILOT-CALL" },
  { name: "MEC-HARBOUR-LOG" },
  { name: "MEC-PILOT-RETURN" },
  { name: "DEC-0012" },
  { name: "DEC-0013" },
  { name: "RULE-SPRING-WINDOW" },
  { name: "MEC-TIDES" },
  { name: "RULE-TIDE-WINDOW", of: "MEC-TIDES" },
  { name: "MEC-TIDE-TABLES" },
  { name: "DOM-WATER" },
  { name: "RULE-HIGH-WATER" },
];

/** A link by its ends' names and its type. */
type Link = readonly [src: string, dst: string, type: string, places?: number];

/** harbor-sim's resolved links; `mentions` the weak ones. */
const LINKS: readonly Link[] = [
  ["MEC-PILOT-CALL", "RULE-SPRING-WINDOW", "follows"],
  ["MEC-HARBOUR-LOG", "MEC-PILOT-CALL", "follows"],
  ["MEC-PILOT-RETURN", "MEC-MOORING", "follows"],
  ["MEC-TIDES", "DOM-BERTHS", "depends_on"],
  ["MEC-TIDES", "RULE-HIGH-WATER", "uses_term"],
  ["MEC-TIDES", "RULE-HIGH-WATER", "adopts"],
  ["MEC-PILOTAGE", "MEC-TIDES", "depends_on"],
  ["MEC-PILOTAGE", "MEC-MOORING", "precedes"],
  ["MEC-NIGHT-PASSAGE", "RULE-TIDE-WINDOW", "depends_on"],
  ["MEC-TIDE-TABLES", "MEC-TIDES", "depends_on"],
  ["RULE-SPRING-WINDOW", "RULE-TIDE-WINDOW", "working_answer"],
  ["DOM-WATER", "MEC-TIDES", "depends_on"],
  ["docs/features/deep-berth.md", "docs/spec/berths/draft-limits.md", "amends"],
  ["MEC-CARGO-CRANES", "MEC-MOORING", "depends_on"],
  ["RULE-FOG-SIGNAL", "DOM-WATER", "depends_on"],
  ["MEC-LOCK-KEEPER", "MEC-LOCK-A", "depends_on"],
  ["MEC-LOCK-B", "MEC-TIDES", "depends_on"],
  ["MEC-FAIRWAY", "MEC-TIDES", "depends_on", 2],
  ["DEC-0012", "RULE-HARBOR-CLOCK", "amends"],
  ["MEC-MOORING", "DEC-0014", "adopts"],
  ["RULE-MOOR-CREW", "MEC-PILOTAGE", "depends_on"],
  ["RULE-BASIN-NAME", "MEC-TIDES", "mentions"],
  ["MEC-TIDES", "MEC-TIDE-TABLES", "mentions"],
  ["MEC-TIDES", "MEC-PILOTAGE", "mentions"],
  ["DOM-BERTHS", "RULE-TIDE-WINDOW", "mentions"],
  ["DOM-BERTHS", "MEC-TIDES", "mentions"],
  ["RULE-SPRING-WINDOW", "MEC-TIDES", "mentions"],
  ["MEC-HARBOUR-LOG", "MEC-FAIRWAY", "mentions"],
  ["MEC-NIGHT-PASSAGE", "MEC-FAIRWAY", "mentions", 3],
  ["DOM-HARBOR", "DOM-WATER", "mentions"],
  ["MEC-LOCK-A", "DOM-WATER", "mentions"],
  ["RULE-FOG-SIGNAL", "MEC-NIGHT-PASSAGE", "mentions"],
];

/** harbor-sim's parents, child to parent, by name. */
const PARENTS: readonly (readonly [child: string, parent: string])[] = [
  ["DOM-BERTHS", "DOM-HARBOR"],
  ["docs/spec/berths/draft-limits.md", "DOM-BERTHS"],
  ["MEC-MOORING", "DOM-BERTHS"],
  ["MEC-FAIRWAY", "DOM-HARBOR"],
  ["MEC-NIGHT-PASSAGE", "MEC-FAIRWAY"],
  ["MEC-PILOTAGE", "DOM-HARBOR"],
  ["DOM-PILOTS", "DOM-HARBOR"],
  ["MEC-PILOT-CALL", "DOM-PILOTS"],
  ["MEC-HARBOUR-LOG", "DOM-PILOTS"],
  ["MEC-PILOT-RETURN", "DOM-PILOTS"],
  ["DOM-WATER", "DOM-HARBOR"],
  ["MEC-TIDES", "DOM-WATER"],
  ["RULE-SPRING-WINDOW", "MEC-TIDES"],
  ["MEC-TIDE-TABLES", "MEC-TIDES"],
  ["RULE-HIGH-WATER", "DOM-WATER"],
  ["MEC-LOCK-B", "MEC-LOCK-A"],
  ["RULE-FOG-SIGNAL", "DOM-WATER"],
  ["MEC-LOCK-KEEPER", "MEC-LOCK-A"],
];

/** harbor-sim's unresolved targets: what is written, its first place's reason, its sources by name, its places. */
const GHOSTS: readonly (readonly [written: string, reason: string, sources: readonly string[], places: number])[] = [
  ["RULE-QUAY-LIGHTS", "`RULE-QUAY-LIGHTS` resolves to no ID and no alias", ["DEC-0014", "RULE-FOG-SIGNAL"], 2],
  ["TERM-SLACK-WATER", "`TERM-SLACK-WATER` resolves to no ID and no alias", ["MEC-TIDES"], 1],
  [HOSTILE_LINK, `\`${HOSTILE_LINK}\` is no ID, path or URL the reader resolves`, ["MEC-MOORING"], 1],
];

/** Code-point order, as the core sorts. */
function byCodePoint(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

/** A node before its uses are counted. */
type Bare = Omit<MapNode, "used_by" | "mentioned_by" | "orphan">;

/**
 * The served answer over bare nodes in served order and links by index: edges merged per (src, dst,
 * type) and sorted, then each node's uses as core `usage_of` counts them (distinct live sources per
 * strong and per weak link) and each live document's orphan flag: no use, no mention, no resolved
 * link out of its file and no child.
 */
export function answerOf(bare: readonly Bare[], links: readonly MapEdge[], parents: readonly MapParent[], ghosts: readonly MapGhost[], archive: boolean, notes: string[] = []): MapDocument {
  const merged = new Map<string, MapEdge>();
  for (const link of links) {
    if (link.src === link.dst) {
      continue;
    }
    const key = `${String(link.src)}\u0000${String(link.dst)}\u0000${link.type}`;
    const held = merged.get(key);
    merged.set(key, held === undefined ? { ...link } : { ...held, places: held.places + link.places });
  }
  const edges = [...merged.values()].sort((a, b) => a.src - b.src || a.dst - b.dst || byCodePoint(a.type, b.type));
  const strong = bare.map(() => new Set<number>());
  const weak = bare.map(() => new Set<number>());
  const out = bare.map(() => false);
  const documentOf = (index: number) => bare[index]?.of ?? index;
  for (const edge of edges) {
    if (bare[edge.src]?.archived === true) {
      continue;
    }
    (edge.weak ? weak : strong)[edge.dst]?.add(edge.src);
    if (bare[edge.src]?.path !== bare[edge.dst]?.path) {
      out[documentOf(edge.src)] = true;
    }
  }
  const parented = new Set(parents.map((parent) => documentOf(parent.parent)));
  const nodes = bare.map((node, index): MapNode => {
    const usedBy = strong[index]?.size ?? 0;
    const mentionedBy = weak[index]?.size ?? 0;
    const orphan = node.of !== null ? null : !node.archived && usedBy === 0 && mentionedBy === 0 && out[index] !== true && !parented.has(index);
    return { ...node, used_by: usedBy, mentioned_by: mentionedBy, orphan };
  });
  const live = nodes.filter((node) => !node.archived);
  return {
    archive,
    notes,
    counts: {
      documents: live.filter((node) => node.of === null).length,
      sections: live.filter((node) => node.of !== null).length,
      edges: edges.filter((edge) => nodes[edge.src]?.archived !== true && nodes[edge.dst]?.archived !== true).length,
      parents: parents.length,
      ghosts: ghosts.length,
      orphans: live.filter((node) => node.orphan === true).length,
    },
    nodes,
    edges,
    parents: [...parents].sort((a, b) => a.child - b.child),
    ghosts: [...ghosts].sort((a, b) => byCodePoint(a.written, b.written)),
  };
}

/** harbor-sim's map over its corpus: the picked nodes read from its tree, in (path, line) order. */
function harborMap(corpus: MockCorpus): MapDocument {
  const rows = treeOf(corpus, {}).nodes;
  const picked = PICKED.map((pick): Omit<Bare, "of"> & { ofName: string | null } => {
    const row = rows.find((candidate) => (candidate.id ?? candidate.path) === pick.name);
    if (row === undefined) {
      throw new Error(`harbor-sim's tree holds no ${pick.name}`);
    }
    return { name: pick.name, id: row.id, kind: row.kind, title: row.title, path: row.path, line: row.line, status: row.status, archived: false, ofName: pick.of ?? null };
  });
  const ordered = [...picked].sort((a, b) => byCodePoint(a.path, b.path) || a.line - b.line);
  const at = new Map(ordered.map((node, index) => [node.name, index]));
  const index = (name: string): number => {
    const found = at.get(name);
    if (found === undefined) {
      throw new Error(`harbor-sim's map holds no ${name}`);
    }
    return found;
  };
  const bare: Bare[] = ordered.map(({ ofName, ...node }) => ({ ...node, of: ofName === null ? null : index(ofName) }));
  const links = LINKS.map(([src, dst, type, places]): MapEdge => ({ src: index(src), dst: index(dst), type, weak: type === "mentions", places: places ?? 1 }));
  const parents = PARENTS.map(([child, parent]) => ({ child: index(child), parent: index(parent) }));
  const ghosts = GHOSTS.map(([written, reason, sources, places]) => ({ written, reason, sources: sources.map(index).sort((a, b) => a - b), places }));
  return answerOf(bare, links, parents, ghosts, false);
}

/** The large map's node count and edge count (CM12). */
export const LARGE_NODES = 5000;
export const LARGE_EDGES = 15000;

/** Numerical Recipes' LCG: a fixed sequence in [0, 1), the same on every run. */
function generator(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 4294967296;
  };
}

/**
 * The kinds the large map deals in turn, every tenth node a section: the two directions', ten
 * unlisted ones and none, held unevenly so the colouring ranks them (src/map/colours.ts). Over the
 * documents, eight kinds take a colour and `faction` and none the grey; `recipe` and `location` are
 * the sections' alone.
 */
const LARGE_KINDS: readonly (string | null)[] = [
  "mechanic",
  "rule",
  "mechanic",
  "domain",
  "quest",
  "rule",
  "mechanic",
  "item",
  "character",
  "recipe",
  "rule",
  "mechanic",
  "quest",
  "dialogue",
  "event",
  "item",
  "faction",
  null,
  "mechanic",
  "location",
];

/**
 * `large`: 5 000 nodes in 50 folders of 100, every tenth an ID section of the document before it;
 * 15 000 edges, their targets skewed to a few hubs so the sizes differ, one in three a mention; 20
 * ghosts; parents within a folder. A fixed seed: the same answer on every read.
 */
function largeMap(): MapDocument {
  const random = generator(20261113);
  const bare: Bare[] = [];
  for (let index = 0; index < LARGE_NODES; index += 1) {
    const folder = String(Math.floor(index / 100)).padStart(2, "0");
    const id = `MAP-${String(index).padStart(4, "0")}`;
    const section = index % 10 === 9;
    const of = section ? index - 1 : null;
    const documentAt = section ? index - 1 : index;
    bare.push({
      name: id,
      id,
      kind: LARGE_KINDS[index % LARGE_KINDS.length] ?? null,
      title: `Map node ${String(index)}`,
      path: `docs/spec/f${folder}/n${String(documentAt).padStart(4, "0")}.md`,
      line: section ? 12 : 1,
      of,
      status: section ? null : "draft",
      archived: false,
    });
  }
  const seen = new Set<string>();
  const links: MapEdge[] = [];
  while (links.length < LARGE_EDGES) {
    const src = Math.floor(random() * LARGE_NODES);
    const dst = Math.floor(random() ** 3 * LARGE_NODES);
    const weak = random() < 1 / 3;
    const type = weak ? "mentions" : "depends_on";
    const key = `${String(src)}:${String(dst)}:${type}`;
    if (src === dst || seen.has(key)) {
      continue;
    }
    seen.add(key);
    links.push({ src, dst, type, weak, places: 1 });
  }
  const parents: MapParent[] = [];
  for (let index = 0; index < LARGE_NODES; index += 1) {
    if (index % 100 !== 0 && index % 10 !== 9) {
      parents.push({ child: index, parent: index - (index % 100) });
    }
  }
  const ghosts: MapGhost[] = Array.from({ length: 20 }, (_, at) => ({
    written: `MAP-GONE-${String(at).padStart(2, "0")}`,
    reason: `\`MAP-GONE-${String(at).padStart(2, "0")}\` resolves to no ID and no alias`,
    sources: [at * 7, at * 7 + 250].sort((a, b) => a - b),
    places: 2,
  }));
  return answerOf(bare, links, parents, ghosts, false);
}

/** ledger-api's map: its tree's live documents and their parents, no link and no ghost. */
function ledgerMap(corpus: MockCorpus, archive: boolean): MapDocument {
  const rows = treeOf(corpus, { archive }).nodes.filter((row) => row.depth >= 0);
  const ordered = [...rows].sort((a, b) => byCodePoint(a.path, b.path) || a.line - b.line);
  const names = ordered.map((row) => row.id ?? row.path);
  const firstOfPath = new Map<string, number>();
  ordered.forEach((row, index) => {
    if (!firstOfPath.has(row.path)) {
      firstOfPath.set(row.path, index);
    }
  });
  const bare: Bare[] = ordered.map((row, index) => {
    const first = firstOfPath.get(row.path) ?? index;
    return { name: names[index] ?? row.path, id: row.id, kind: row.kind, title: row.title, path: row.path, line: row.line, of: first === index ? null : first, status: row.status, archived: row.archived };
  });
  const parents = ordered.flatMap((row, child) => {
    const parent = row.parent === null || bare[child]?.of !== null ? -1 : names.indexOf(row.parent);
    return parent < 0 ? [] : [{ child, parent }];
  });
  return answerOf(bare, [], parents, [], archive);
}

/** A project's map in a scenario; `archive` as asked. */
export function mockMap(corpus: MockCorpus, slug: string, scenario: Scenario, archive: boolean): MapDocument {
  if (scenario === "empty") {
    return answerOf([], [], [], [], archive);
  }
  if (slug === "harbor-sim") {
    const answer = scenario === "large" ? largeMap() : harborMap(corpus);
    return { ...answer, archive };
  }
  return ledgerMap(corpus, archive);
}
