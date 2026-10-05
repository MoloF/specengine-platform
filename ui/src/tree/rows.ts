import type { TreeNode } from "../api/types";

/**
 * One row of the tree as the view walks it. The hierarchy comes from `depth` alone: a row's
 * parent is the nearest earlier row one level up, never looked up by name (two holders share
 * one name; a dangling parent names nothing).
 */
export interface TreeRow {
  node: TreeNode;
  /** `path:line`, unique per row: expansion and focus survive a re-read. */
  key: string;
  index: number;
  parent: number | null;
  children: number[];
  /** Among its siblings (WAI-ARIA tree: aria-setsize, aria-posinset). */
  setSize: number;
  posInSet: number;
}

/** A row's name: its ID, else its path. */
export function rowName(node: { id: string | null; path: string }): string {
  return node.id ?? node.path;
}

/** A node's place, the same for a tree row and a shown node. */
export function placeKey(node: { path: string; line: number }): string {
  return `${node.path}:${String(node.line)}`;
}

/** The rows of a flat pre-order tree, each with its parent, children and place among siblings. */
export function buildRows(nodes: readonly TreeNode[]): TreeRow[] {
  const rows: TreeRow[] = [];
  const seen = new Set<string>();
  const open: TreeRow[] = [];
  for (const node of nodes) {
    while (open.length > 0 && (open[open.length - 1]?.node.depth ?? -1) >= node.depth) {
      open.pop();
    }
    const parent = open[open.length - 1] ?? null;
    let key = placeKey(node);
    if (seen.has(key)) {
      key = `${key}@${String(rows.length)}`;
    }
    seen.add(key);
    const row: TreeRow = { node, key, index: rows.length, parent: parent?.index ?? null, children: [], setSize: 1, posInSet: 1 };
    rows.push(row);
    parent?.children.push(row.index);
    open.push(row);
  }
  const place = (siblings: readonly number[]) => {
    siblings.forEach((index, position) => {
      const row = rows[index];
      if (row !== undefined) {
        row.setSize = siblings.length;
        row.posInSet = position + 1;
      }
    });
  };
  place(rows.filter((row) => row.parent === null).map((row) => row.index));
  for (const row of rows) {
    place(row.children);
  }
  return rows;
}

/** The rows shown: every row whose ancestors are all expanded, in order. */
export function visibleRows(rows: readonly TreeRow[], expanded: ReadonlySet<string>): TreeRow[] {
  const shown: TreeRow[] = [];
  let hiddenBelow: number | null = null;
  for (const row of rows) {
    if (hiddenBelow !== null) {
      if (row.node.depth > hiddenBelow) {
        continue;
      }
      hiddenBelow = null;
    }
    shown.push(row);
    if (row.children.length > 0 && !expanded.has(row.key)) {
      hiddenBelow = row.node.depth;
    }
  }
  return shown;
}

/** A row's ancestors, nearest first. */
export function ancestorsOf(rows: readonly TreeRow[], row: TreeRow): TreeRow[] {
  const chain: TreeRow[] = [];
  for (let at = row.parent; at !== null; at = rows[at]?.parent ?? null) {
    const ancestor = rows[at];
    if (ancestor === undefined) {
      break;
    }
    chain.push(ancestor);
  }
  return chain;
}

/** The file name without `.md`: the `slug` of a `slug/ID` REF. */
export function stemOf(path: string): string {
  return (path.split("/").pop() ?? path).replace(/\.md$/, "");
}

/**
 * The rows a REF opens. Once the node is read, its holders' places say it exactly; before that,
 * the rows whose name, `slug/ID` or section matches the REF as written.
 */
export function rowsForRef(rows: readonly TreeRow[], ref: string, holders: readonly { path: string; line: number }[] | null): TreeRow[] {
  if (holders !== null) {
    const places = new Set(holders.map(placeKey));
    return rows.filter((row) => places.has(placeKey(row.node)));
  }
  const hash = ref.indexOf("#");
  const wanted = hash < 0 ? ref : ref.slice(hash + 1);
  return rows.filter((row) => {
    const { id, path } = row.node;
    if (id === null) {
      return path === wanted;
    }
    return id === wanted || `${stemOf(path)}/${id}` === wanted;
  });
}
