import type { NodeView, Proposal, ShownLink, ShownLinks, TreeNode, WorkspaceDocument } from "../api/types";
import { aDirection, aLink, aModule, aNode, aTrace, aTraceCard, aTraceNode, aTreeNode, aTreeView, aWorkspace, aWorkspaces } from "./builders";
import { stubClient } from "./stubClient";

// A river ferry for the document view's tests (docs/features/doc-modal.md "Acceptance criteria",
// DM-9 to DM-13), invented: one document with an ancestor, a section, links of every kind the view
// sorts (strong out and in, a mention, a dangling one), a trace row with a card, a rooted tree read
// two deep, and a workspaces document with two directions. Kinds, statuses, link types and module
// names are this stub's data, never app code's.

export const FERRY = "DOM-FERRY";
export const FERRY_PATH = "docs/ferry/ferry.md";

/** The ferry's file, its front-matter first: the text the daemon serves for the document. */
export const FERRY_LINES = [
  "---",
  "id: DOM-FERRY",
  "kind: domain",
  "status: accepted",
  "parent: DOM-RIVER",
  "links: {uses: [MEC-BOARDING, MEC-GANGWAY]}",
  "---",
  "",
  "# The ferry",
  "",
  "A flat boat crosses the river on a cable.",
  "",
  "It runs from dawn to dusk.",
  "",
  "## Tides {#RULE-FERRY-TIDES}",
  "",
  "The ferry waits for slack water.",
];

/** The front-matter block as written, through its closing line. */
export const FERRY_FRONT_MATTER = `${FERRY_LINES.slice(0, 7).join("\n")}\n`;

const link = (fields: Partial<ShownLink> & Pick<ShownLink, "type" | "written">): ShownLink => aLink({ path: FERRY_PATH, line: 6, ...fields });

/** Its links: out `uses` Boarding, a mention of the river, a `uses` that dangles; in `follows` from Landing. */
export const FERRY_LINKS: ShownLinks = {
  outgoing: [
    link({ type: "uses", written: "MEC-BOARDING", name: "MEC-BOARDING", title: "Boarding", end_kind: "mechanic" }),
    link({ type: "mentions", origin: "inline", written: "DOM-RIVER", name: "DOM-RIVER", title: "The river", end_kind: "domain", line: 11 }),
    link({ type: "uses", written: "MEC-GANGWAY", name: null, state: "dangling", reason: "`MEC-GANGWAY` resolves to no ID and no alias" }),
  ],
  incoming: [link({ type: "follows", written: "DOM-FERRY", name: "MEC-LANDING", title: "Landing", end_kind: "mechanic", path: "docs/ferry/landing.md", line: 5 })],
  left_out: { generated: 0, tier3: 0 },
  omitted: 0,
};

/** `getNode(DOM-FERRY, {with: ["links"]})`. */
export function ferryView(): NodeView {
  return {
    ref: FERRY,
    reason: null,
    notes: [],
    nodes: [
      aNode({
        id: FERRY,
        kind: "domain",
        title: "The ferry",
        path: FERRY_PATH,
        line: 1,
        end_line: FERRY_LINES.length,
        status: "accepted",
        rev: 3,
        tokens_est: 42,
        sections: ["RULE-FERRY-TIDES"],
        text: `${FERRY_LINES.join("\n")}\n`,
        ancestors: [{ id: "DOM-RIVER", kind: "domain", title: "The river", path: "docs/river.md" }],
        links: structuredClone(FERRY_LINKS),
      }),
    ],
  };
}

/** The other documents the links name, each with a plain text, the river at the top. */
const OTHERS: Readonly<Record<string, { title: string; kind: string; path: string; ancestors: string[] }>> = {
  "DOM-RIVER": { title: "The river", kind: "domain", path: "docs/river.md", ancestors: [] },
  "MEC-BOARDING": { title: "Boarding", kind: "mechanic", path: "docs/ferry/boarding.md", ancestors: ["DOM-RIVER", FERRY] },
  "MEC-LANDING": { title: "Landing", kind: "mechanic", path: "docs/ferry/landing.md", ancestors: ["DOM-RIVER"] },
  "RULE-FERRY-TIDES": { title: "Tides", kind: "rule", path: FERRY_PATH, ancestors: ["DOM-RIVER", FERRY] },
};

/** What the stub's getNode answers: the ferry, the documents it links, else the exit-1 document. */
export function docView(ref: string): NodeView {
  if (ref === FERRY) {
    return ferryView();
  }
  const other = OTHERS[ref];
  if (other === undefined) {
    return { ref, reason: `\`${ref}\` resolves to no ID and no alias`, notes: [], nodes: [] };
  }
  const ancestors = other.ancestors.map((name) => ({ id: name, kind: name === FERRY ? "domain" : (OTHERS[name]?.kind ?? null), title: name === FERRY ? "The ferry" : (OTHERS[name]?.title ?? null), path: name === FERRY ? FERRY_PATH : (OTHERS[name]?.path ?? "") }));
  return {
    ref,
    reason: null,
    notes: [],
    nodes: [aNode({ id: ref, kind: other.kind, title: other.title, path: other.path, text: `## ${other.title}\n\nThe ${other.title.toLowerCase()} as written.\n`, ancestors, links: { outgoing: [], incoming: [], left_out: { generated: 0, tier3: 0 }, omitted: 0 } })],
  };
}

/** `getTree({root: DOM-FERRY, depth: 2})`: the document and its section; three under the section; Boarding with its own child; a note without an ID. */
export function ferryTree(): TreeNode[] {
  return [
    aTreeNode({ id: FERRY, depth: 0, kind: "domain", title: "The ferry", path: FERRY_PATH, line: 1 }),
    aTreeNode({ id: "RULE-FERRY-TIDES", depth: 1, parent: FERRY, kind: "rule", title: "Tides", path: FERRY_PATH, line: 15 }),
    aTreeNode({ id: "ASM-0001", depth: 2, parent: "RULE-FERRY-TIDES", kind: "assumption", title: "The tide table is right", path: "docs/ferry/asm-0001.md" }),
    aTreeNode({ id: "ASM-0002", depth: 2, parent: "RULE-FERRY-TIDES", kind: "assumption", title: "Fog stops the ferry", path: "docs/ferry/asm-0002.md" }),
    aTreeNode({ id: "QST-0001", depth: 2, parent: "RULE-FERRY-TIDES", kind: "question", title: "Who rings the bell?", path: "docs/ferry/qst-0001.md" }),
    aTreeNode({ id: "MEC-BOARDING", depth: 1, parent: FERRY, kind: "mechanic", title: "Boarding", path: "docs/ferry/boarding.md" }),
    aTreeNode({ id: "ASM-0003", depth: 2, parent: "MEC-BOARDING", kind: "assumption", title: "A plank is enough", path: "docs/ferry/asm-0003.md" }),
    aTreeNode({ id: null, depth: 1, parent: FERRY, kind: null, title: "Ferry notes", path: "docs/ferry/notes.md" }),
  ];
}

/**
 * The ferry's workspaces: a rooted module, which groups nothing, then two directions, `records` by
 * `assumption` and `asked` by `question` and `assumption`, so an assumption joins the first in served
 * order (docs/features/departments.md "Acceptance criteria", DP-13). Names and kinds are this stub's.
 */
export function ferryWorkspaces(): WorkspaceDocument {
  return aWorkspaces(
    [aWorkspace({ name: "crossing" })],
    [aModule({ name: "river", title: "The river", root: "DOM-RIVER" }), aDirection({ name: "records", title: "Records", kinds: ["assumption"] }), aDirection({ name: "asked", title: "Asked", kinds: ["question", "assumption"] })],
  );
}

/** The ferry's trace row: in work, its one card. */
export const FERRY_CARD = aTraceCard({ id: "C-0007", title: "Paint the ferry", mark: "open" });

/** A stub client serving the ferry: its node reads, its trace, its rooted tree; any other root none. */
export function docClient(proposals: Proposal[] = []) {
  const client = stubClient(proposals);
  client.getNode.mockImplementation((_project, ref) => Promise.resolve(docView(ref)));
  client.getTrace.mockImplementation((_project, options) =>
    Promise.resolve(options?.ref === FERRY ? aTrace([aTraceNode(FERRY, [FERRY_CARD], { title: "The ferry", rollup: "in work" })], { ref: FERRY }) : aTrace([], { ref: options?.ref ?? null })),
  );
  client.getTree.mockImplementation((_project, options) =>
    Promise.resolve(options?.root === FERRY ? aTreeView(ferryTree(), { ref: FERRY, depth: 2 }) : aTreeView([], { ref: options?.root ?? null, depth: options?.depth ?? null })),
  );
  client.getWorkspaces.mockImplementation(() => Promise.resolve(ferryWorkspaces()));
  return client;
}
