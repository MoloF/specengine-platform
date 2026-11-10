import type { TreeView } from "../api/types";

// What a document consists of (docs/features/doc-modal.md "Acceptance criteria", DM-13): the rows of
// a rooted tree read two deep, grouped by the holder they are listed under (the document, then each
// of its sections), then by raw kind. A child's own children and sections show in its own Inside.

/** A kind's entries shown before "Show all". */
export const INSIDE_SHOWN = 50;

/** One child: its name (ID, else path), title, raw kind and file. */
export interface InsideEntry {
  name: string;
  title: string | null;
  kind: string | null;
  path: string;
}

/** The children listed under one holder: `direct` the document itself, else one of its sections, by its title (else ID). */
export interface InsideGroup {
  holder: string;
  title: string | null;
  direct: boolean;
  kinds: { kind: string | null; entries: InsideEntry[] }[];
}

/** Code-point order, a raw kind's: never a locale's. */
function byCodePoint(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

/**
 * The read's groups, the document's first, then its sections' in tree order, each holding a child;
 * kinds by code point, no kind last; entries in tree order. Children without an ID are counted in
 * `noId`, for the Spec tree. Own nodes are the rows sharing a depth-0 row's file; children, the other
 * rows whose `parent` is an own node; anything deeper is dropped.
 */
export function insideOf(tree: TreeView): { groups: InsideGroup[]; noId: number } {
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
    const byKind = new Map<string | null, InsideEntry[]>();
    for (const row of named) {
      const entry: InsideEntry = { name: row.id ?? row.path, title: row.title, kind: row.kind, path: row.path };
      const held = byKind.get(row.kind);
      if (held === undefined) {
        byKind.set(row.kind, [entry]);
      } else {
        held.push(entry);
      }
    }
    const kinds = [...byKind.entries()]
      .sort(([a], [b]) => (a === null ? (b === null ? 0 : 1) : b === null ? -1 : byCodePoint(a, b)))
      .map(([kind, entries]) => ({ kind, entries }));
    groups.push({ holder: name, title: holder.title, direct: holder.depth === 0, kinds });
  }
  return { groups, noId };
}
