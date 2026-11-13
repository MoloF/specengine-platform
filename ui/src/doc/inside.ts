import type { TreeView, WorkspaceModule } from "../api/types";
import { isDirection } from "../workspace/choice";

// What a document consists of (docs/features/doc-modal.md "Acceptance criteria", DM-13): the rows of
// a rooted tree read two deep, grouped by the holder they are listed under (the document, then each
// of its sections), then by direction and raw kind. A child's own children and sections show in its
// own Inside. A direction, a module listing kinds, gathers its kinds' children under its title
// (docs/features/departments.md "Acceptance criteria", DP-13); a kind no direction lists stays raw.

/** A kind's entries shown before "Show all". */
export const INSIDE_SHOWN = 50;

/** One child: its name (ID, else path), title, raw kind and file. */
export interface InsideEntry {
  name: string;
  title: string | null;
  kind: string | null;
  path: string;
}

/** A direction as Inside groups by it: its served name and title, and the kinds it lists. */
export interface InsideDirection {
  name: string;
  title: string;
  kinds: readonly string[];
}

/** One set of entries: a direction's, labelled by its title, or a raw kind's (null: no kind). */
export type KindSet<T> = { by: "direction"; name: string; title: string; entries: T[] } | { by: "kind"; kind: string | null; entries: T[] };

/** One set of a holder's children. */
export type InsideSet = KindSet<InsideEntry>;

/** The children listed under one holder: `direct` the document itself, else one of its sections, by its title (else ID). */
export interface InsideGroup {
  holder: string;
  title: string | null;
  direct: boolean;
  sets: InsideSet[];
}

/** The served modules that are directions, in served order: a rooted module groups nothing. */
export function insideDirections(modules: readonly WorkspaceModule[]): InsideDirection[] {
  return modules.filter(isDirection).map((module) => ({ name: module.name, title: module.title, kinds: module.kinds }));
}

/** Code-point order, a raw kind's: never a locale's. */
function byCodePoint(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

/**
 * Entries as sets by their kind: each entry whose kind a direction lists joins the first such
 * direction in served order; directions first, in served order; then the kinds no direction lists,
 * by code point, no kind last. Entries keep the order given. An entry with no kind matches no
 * direction. Inside groups a holder's children so, a document's Links its users
 * (docs/features/usage-orphans.md "Acceptance criteria", UO10), Health its orphans (UO11).
 */
export function kindSets<T extends { kind: string | null }>(named: readonly T[], directions: readonly InsideDirection[]): KindSet<T>[] {
  const byDirection = new Map<InsideDirection, T[]>();
  const byKind = new Map<string | null, T[]>();
  const add = <K>(held: Map<K, T[]>, key: K, entry: T) => {
    const list = held.get(key);
    if (list === undefined) {
      held.set(key, [entry]);
    } else {
      list.push(entry);
    }
  };
  for (const entry of named) {
    const { kind } = entry;
    const direction = kind === null ? undefined : directions.find((candidate) => candidate.kinds.includes(kind));
    if (direction === undefined) {
      add(byKind, kind, entry);
    } else {
      add(byDirection, direction, entry);
    }
  }
  const sets: KindSet<T>[] = [];
  for (const direction of directions) {
    const entries = byDirection.get(direction);
    if (entries !== undefined) {
      sets.push({ by: "direction", name: direction.name, title: direction.title, entries });
    }
  }
  const kinds = [...byKind.entries()].sort(([a], [b]) => (a === null ? (b === null ? 0 : 1) : b === null ? -1 : byCodePoint(a, b)));
  for (const [kind, entries] of kinds) {
    sets.push({ by: "kind", kind, entries });
  }
  return sets;
}

/**
 * The read's groups, the document's first, then its sections' in tree order, each holding a child;
 * within each, the directions' sets, then the raw kinds' (`kindSets`). Children without an ID are
 * counted in `noId`, for the Spec tree. Own nodes are the rows sharing a depth-0 row's file;
 * children, the other rows whose `parent` is an own node; anything deeper is dropped.
 */
export function insideOf(tree: TreeView, directions: readonly InsideDirection[] = []): { groups: InsideGroup[]; noId: number } {
  const own = new Set(tree.nodes.filter((row) => row.depth === 0).map((row) => row.path));
  const owners = tree.nodes.filter((row) => own.has(row.path));
  const ownNames = new Set(owners.map((row) => row.id ?? row.path));
  const children = tree.nodes.filter((row) => !own.has(row.path) && row.parent !== null && ownNames.has(row.parent));
  let noId = 0;
  const groups: InsideGroup[] = [];
  for (const holder of owners) {
    const name = holder.id ?? holder.path;
    const under = children.filter((row) => row.parent === name);
    const named = under.filter((row) => row.id !== null);
    noId += under.length - named.length;
    if (named.length === 0) {
      continue;
    }
    const entries = named.map((row): InsideEntry => ({ name: row.id ?? row.path, title: row.title, kind: row.kind, path: row.path }));
    groups.push({ holder: name, title: holder.title, direct: holder.depth === 0, sets: kindSets(entries, directions) });
  }
  return { groups, noId };
}
