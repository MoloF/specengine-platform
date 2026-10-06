import type {
  BundleItem,
  BundleLayers,
  BundleView,
  GraphEdge,
  GraphNode,
  GraphView,
  Proposal,
  SearchHit,
  SearchResults,
  ShownLink,
  ShownNode,
  TaskListEntry,
  TaskPackage,
  TaskProposal,
  TaskRun,
  TreeNode,
  TreeView,
} from "../api/types";

/** A whole proposal for tests; omitted keys null or empty as the review JSON gives them. */
export function aProposal(fields: Partial<Proposal> & Pick<Proposal, "id">): Proposal {
  return {
    project: "alpha",
    kind: "update",
    status: "open",
    target_id: null,
    target_path: null,
    worktree: null,
    branch: null,
    base_commit: null,
    base_hash: null,
    base_text: null,
    new_text: null,
    patch_hash: null,
    rationale: null,
    author: null,
    diagnostics: [],
    diff: null,
    preview: null,
    conflict: null,
    decided_by: null,
    decided_at: null,
    decision_note: null,
    applied_commit: null,
    created_at: "2026-10-01T10:00:00Z",
    updated_at: "2026-10-01T10:00:00Z",
    notes: [],
    severity: "normal",
    gap_type: null,
    task_id: null,
    target_ids: [],
    evidence: [],
    options: [],
    recommendation: null,
    working_answer: null,
    summary: `Summary of ${fields.id}`,
    ...fields,
  };
}

/** A whole shown node for tests. */
export function aNode(fields: Partial<ShownNode> & Pick<ShownNode, "id">): ShownNode {
  return {
    kind: "widget",
    title: `Title of ${fields.id ?? "the node"}`,
    path: "docs/spec/a.md",
    line: 1,
    end_line: 3,
    status: null,
    rev: 1,
    tokens_est: 10,
    archived: false,
    utf8: true,
    sections: [],
    span_hash: "b3:0000000000000000000000000000000000000000000000000000000000000000",
    text: `## ${fields.id ?? "Node"}\n\nCurrent text.\n`,
    truncated: false,
    omitted: null,
    links: null,
    ...fields,
  };
}

/** One tree row for tests: a live document at the given depth. */
export function aTreeNode(fields: Partial<TreeNode> & Pick<TreeNode, "id" | "depth">): TreeNode {
  return {
    kind: "widget",
    title: `Title of ${fields.id ?? "the node"}`,
    path: `docs/spec/${(fields.id ?? "node").toLowerCase()}.md`,
    line: 1,
    parent: null,
    mark: null,
    status: null,
    rev: 1,
    tokens_est: 10,
    archived: false,
    ...fields,
  };
}

/** A whole tree for tests: every key present, the rows as given. */
export function aTreeView(nodes: TreeNode[], fields: Partial<TreeView> = {}): TreeView {
  return {
    ref: null,
    reason: null,
    notes: [],
    depth: null,
    kinds: [],
    archive: false,
    left_out: { generated: 0, tier3: 0 },
    truncated: false,
    nodes,
    ...fields,
  };
}

/** One link of a shown node for tests: resolved unless told otherwise. */
export function aLink(fields: Partial<ShownLink> & Pick<ShownLink, "type" | "written">): ShownLink {
  return {
    origin: "frontmatter",
    at: null,
    name: fields.written,
    path: "docs/spec/a.md",
    line: 2,
    state: "resolved",
    reason: null,
    ...fields,
  };
}

/** One search hit for tests. */
export function aSearchHit(fields: Partial<SearchHit> & Pick<SearchHit, "id">): SearchHit {
  return {
    kind: "widget",
    title: `Title of ${fields.id ?? "the node"}`,
    path: "docs/spec/a.md",
    line: 1,
    ord: 0,
    archived: false,
    snippet: { segments: [{ text: "some ", hit: false }, { text: "text", hit: true }], cut_start: false, cut_end: false },
    ...fields,
  };
}

/** Search results for tests: every key present. */
export function aSearchResults(hits: SearchHit[], fields: Partial<SearchResults> = {}): SearchResults {
  return {
    archive: false,
    hits,
    kinds: [],
    limit: 20,
    notes: [],
    query: "text",
    tier3_left_out: 0,
    truncated: false,
    ...fields,
  };
}

/** One bundle item for tests. */
export function aBundleItem(fields: Partial<BundleItem> & Pick<BundleItem, "name">): BundleItem {
  return {
    kind: "widget",
    title: `Title of ${fields.name}`,
    path: "docs/spec/a.md",
    line: 1,
    form: "text",
    status: null,
    via: null,
    working_answer: null,
    tokens_est: 10,
    archived: false,
    ...fields,
  };
}

/** The nine layers, empty unless given. */
export function someLayers(layers: Partial<BundleLayers> = {}): BundleLayers {
  return {
    targets: [],
    open_questions: [],
    ancestors: [],
    criteria: [],
    bindings: [],
    decisions: [],
    neighbours: [],
    terms: [],
    tests: [],
    ...layers,
  };
}

/** A whole bundle for tests: one target, its body. */
export function aBundle(refs: string[], fields: Partial<BundleView> = {}): BundleView {
  const body = `# Bundle: ${refs.join(", ")}\n\n## Targets\n${refs.join("\n")}\n`;
  return {
    refs,
    reason: null,
    notes: [],
    task: null,
    budget: 2000,
    tokens: 12,
    chars: body.length,
    bytes: body.length,
    bundle_hash: "b3:1111111111111111111111111111111111111111111111111111111111111111",
    body,
    layers: someLayers({ targets: refs.map((name) => aBundleItem({ name })) }),
    tail: [],
    more: 0,
    ...fields,
  };
}

/** One reached node for tests: a live document. */
export function aGraphNode(fields: Partial<GraphNode> & Pick<GraphNode, "id" | "distance">): GraphNode {
  return {
    kind: "widget",
    title: `Title of ${fields.id ?? "the node"}`,
    path: `docs/spec/${(fields.id ?? "node").toLowerCase()}.md`,
    line: 1,
    archived: false,
    ...fields,
  };
}

/** One edge for tests: resolved, written where its source is. */
export function aGraphEdge(fields: Partial<GraphEdge> & Pick<GraphEdge, "src" | "type" | "dst">): GraphEdge {
  return {
    written: fields.dst ?? "R-404",
    path: `docs/spec/${(fields.src ?? "node").toLowerCase()}.md`,
    line: 3,
    state: "resolved",
    reason: null,
    ...fields,
  };
}

/** A whole graph answer for tests: every key present; two invented types followed outgoing. */
export function aGraphView(nodes: GraphNode[], edges: GraphEdge[], fields: Partial<GraphView> = {}): GraphView {
  return {
    ref: nodes[0]?.id ?? "R-1",
    reason: null,
    impact: false,
    types: [
      { type: "zeta_type", direction: "out" },
      { type: "alpha_type", direction: "out" },
    ],
    depth: 2,
    archive: false,
    notes: [],
    left_out: { generated: 0, tier3: 0 },
    truncated: false,
    nodes,
    edges,
    ...fields,
  };
}

/** A task list row for tests. */
export function aTaskEntry(fields: Partial<TaskListEntry> & Pick<TaskListEntry, "id">): TaskListEntry {
  return {
    status: "draft",
    title: `Title of ${fields.id}`,
    targets: [],
    stale: null,
    updated_at: "2026-10-01T10:00:00Z",
    ...fields,
  };
}

/** A whole task package for tests, every key in the documented order; omitted keys null or empty. */
export function aTaskPackage(fields: Partial<TaskPackage> & Pick<TaskPackage, "id">): TaskPackage {
  return {
    schema_version: fields.schema_version ?? 1,
    id: fields.id,
    project: fields.project ?? "alpha",
    status: fields.status ?? "draft",
    title: fields.title === undefined ? `Title of ${fields.id}` : fields.title,
    goal: fields.goal ?? null,
    profile: fields.profile ?? null,
    stale: fields.stale ?? null,
    targets: fields.targets ?? [],
    criteria: fields.criteria ?? [],
    affected_nodes: fields.affected_nodes ?? [],
    plan: fields.plan ?? null,
    assumptions: fields.assumptions ?? [],
    open_proposals: fields.open_proposals ?? [],
    owner_notes: fields.owner_notes ?? [],
    bindings: fields.bindings ?? [],
    spec_snapshot: fields.spec_snapshot ?? null,
    snapshot_diff: fields.snapshot_diff ?? null,
    claim: fields.claim ?? null,
    runs: fields.runs ?? [],
    bundle: fields.bundle ?? null,
    author: fields.author ?? null,
    created_at: fields.created_at ?? "2026-10-01T09:00:00Z",
    updated_at: fields.updated_at ?? "2026-10-01T10:00:00Z",
    notes: fields.notes ?? [],
  };
}

/** A task's open proposal for tests. */
export function aTaskProposal(fields: Partial<TaskProposal> & Pick<TaskProposal, "id">): TaskProposal {
  return { kind: "update", status: "open", target_ids: [], task_id: null, summary: `Summary of ${fields.id}`, ...fields };
}

/** A run for tests; open unless `ended_at` is given. */
export function aTaskRun(fields: Partial<TaskRun> & Pick<TaskRun, "run">): TaskRun {
  return {
    role: "worker",
    started_at: "2026-10-01T11:00:00Z",
    ended_at: null,
    outcome: null,
    summary: null,
    changed_files: [],
    ...fields,
  };
}
