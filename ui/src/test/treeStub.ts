import type { NodeView, Proposal, ShownLinks, ShownNode, TreeNode } from "../api/types";
import { aLink, aNode, aTreeNode, aTreeView } from "./builders";
import { stubClient } from "./stubClient";

// A small corpus on the stub client for the Spec tree tests: two roots with children, nested
// sections, an ID-less document, one ID with two holders (the second with a child), a dangling
// parent, a two-document cycle, an archived document. Kinds and statuses are invented here.

export const TREE: TreeNode[] = [
  aTreeNode({ id: "DOC-A", depth: 0, path: "docs/spec/a.md", line: 1, kind: "widget", status: "alpha-live", rev: 2 }),
  aTreeNode({ id: "SEC-A1", depth: 1, parent: "DOC-A", path: "docs/spec/a.md", line: 12, kind: "gadget" }),
  aTreeNode({ id: "SEC-A1X", depth: 2, parent: "SEC-A1", path: "docs/spec/a.md", line: 16, kind: "gadget" }),
  aTreeNode({ id: "SEC-DUP", depth: 1, parent: "DOC-A", path: "docs/spec/a.md", line: 30, kind: "gadget" }),
  aTreeNode({ id: null, depth: 1, parent: "DOC-A", path: "docs/spec/b.md", line: 1, kind: null, title: "Bee notes" }),
  aTreeNode({ id: "SEC-DUP", depth: 2, parent: "docs/spec/b.md", path: "docs/spec/b.md", line: 9, kind: "gadget" }),
  aTreeNode({ id: "SEC-DUP-KID", depth: 3, parent: "SEC-DUP", path: "docs/spec/b.md", line: 14, kind: "gadget" }),
  aTreeNode({ id: "DOC-C", depth: 0, path: "docs/spec/c.md", mark: "dangling-parent" }),
  aTreeNode({ id: "DOC-D", depth: 0, path: "docs/spec/d.md", mark: "parent-cycle" }),
  aTreeNode({ id: "DOC-E", depth: 1, parent: "DOC-D", path: "docs/spec/e.md" }),
  aTreeNode({ id: "DOC-F", depth: 0, path: "docs/spec/f.md", archived: true }),
];

/** The rows shown at the start: the roots expanded, nothing below them. */
export const START_ROWS = ["DOC-A", "SEC-A1", "SEC-DUP", "docs/spec/b.md", "DOC-C", "DOC-D", "DOC-E", "DOC-F"];

/** A node's text: a heading and nine lines, so its span is ten lines from its first. */
export function textOf(row: { id: string | null; path: string; title: string | null }): string {
  const name = row.id ?? row.path;
  const lines = [`## ${name}: ${row.title ?? ""}`, ...Array.from({ length: 9 }, (_, at) => `Line ${String(at + 2)} of ${name}.`)];
  return `${lines.join("\n")}\n`;
}

/** The links every stub node shows: four states, a weak one, both directions, reasons. */
export const LINKS: ShownLinks = {
  outgoing: [
    aLink({ type: "zeta_type", written: "R-9", name: "R-9", line: 2 }),
    aLink({ type: "alpha_type", written: "[[R-8]]", name: "R-8", line: 3, at: "SEC-A1" }),
    aLink({
      type: "mentions",
      origin: "inline",
      written: "R-404",
      name: null,
      line: 4,
      state: "dangling",
      reason: "`R-404` resolves to no ID and no alias",
    }),
    aLink({ type: "beta_type", written: "other:R-1", name: null, line: 5, state: "skipped" }),
    aLink({
      type: "gamma_type",
      written: "docs/canon/x.md",
      name: null,
      line: 6,
      state: "unchecked",
      reason: "`canon:` names docs/canon/x.md; not checked",
    }),
  ],
  incoming: [aLink({ type: "delta_type", written: "DOC-A", name: "DOC-X", path: "docs/spec/x.md", line: 7 })],
  left_out: { generated: 1, tier3: 2 },
  omitted: 0,
};

export function nodeFor(row: TreeNode, links: ShownLinks | null = null): ShownNode {
  return aNode({
    id: row.id,
    kind: row.kind,
    title: row.title,
    path: row.path,
    line: row.line,
    end_line: row.line + 9,
    status: row.status,
    archived: row.archived,
    text: textOf(row),
    links,
  });
}

/** What the stub's getNode answers for a REF: the rows named so, else the exit-1 document. */
export function viewOf(ref: string, links: boolean): NodeView {
  const holders = TREE.filter((row) => (row.id ?? row.path) === ref);
  if (holders.length === 0) {
    return { ref, reason: `\`${ref}\` resolves to no ID and no alias`, notes: [], nodes: [] };
  }
  return { ref, reason: null, notes: [], nodes: holders.map((row) => nodeFor(row, links ? LINKS : null)) };
}

/** A stub client serving the corpus above. */
export function treeClient(proposals: Proposal[] = []) {
  const client = stubClient(proposals);
  client.getTree.mockImplementation(() => Promise.resolve(aTreeView(TREE.map((row) => ({ ...row })))));
  client.getNode.mockImplementation((_project, ref, options) =>
    Promise.resolve(viewOf(ref, options?.with?.includes("links") === true)),
  );
  return client;
}
