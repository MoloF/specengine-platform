import type {
  AssumedNode,
  AssumedProposal,
  AssumesRow,
  BindingRow,
  BundleBindingRow,
  BundleItem,
  BundleLayers,
  BundleView,
  Card,
  CardFlow,
  CardList,
  CardRow,
  CardWrite,
  CheckCounts,
  CheckFinding,
  CheckReport,
  DriftRow,
  FlowEdge,
  FlowGap,
  FlowNode,
  FlowView,
  GraphEdge,
  GraphNode,
  GraphView,
  HealthAssumes,
  HealthDrift,
  HealthReport,
  HealthUnbound,
  HistoryEntry,
  InboxEntry,
  Proposal,
  RoadmapDocument,
  RoadmapGroup,
  RoadmapItem,
  RoadmapSum,
  RoundItem,
  RoundSheet,
  SearchHit,
  SearchResults,
  ShownHistory,
  ShownLink,
  ShownNode,
  TaskListEntry,
  TaskPackage,
  TaskProposal,
  TaskRun,
  TestedRow,
  TraceCard,
  TraceDocument,
  TraceNode,
  TreeNode,
  TreeView,
  UnappliedAmendment,
  UnboundMarker,
  UnboundRow,
  Workspace,
  WorkspaceDocument,
  WorkspaceModule,
} from "../api/types";

/** A whole review document for tests; omitted keys null or empty as the review JSON gives them. */
export function aProposal(fields: Partial<Proposal> & { id: string }): Proposal {
  return {
    project: "alpha",
    kind: "update",
    status: "open",
    target_id: null,
    target_path: null,
    worktree: null,
    branch: "main",
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
    revisions: null,
    drift: null,
    decided_by: null,
    decided_at: null,
    decision_note: null,
    applied_commit: null,
    created_at: "2026-10-01T10:00:00Z",
    updated_at: "2026-10-01T10:00:00Z",
    target_ids: [],
    severity: "normal",
    gap_type: null,
    summary: `Summary of ${fields.id}`,
    working_answer: null,
    price_of_other: null,
    evidence: [],
    options: [],
    recommendation: null,
    distinct_from: [],
    linked: null,
    record_id: null,
    record_path: null,
    record_title: null,
    record_text: null,
    choice: null,
    task_id: null,
    staged: null,
    staged_at: null,
    interpretation: null,
    card_id: null,
    why: null,
    notes: [],
    ...fields,
  };
}

/** A review document's line in the inbox, its fields passed through (the stub cuts nothing). */
export function entryOf(proposal: Proposal): InboxEntry {
  const target = proposal.target_id ?? proposal.target_ids[0] ?? "";
  return {
    id: proposal.id ?? "",
    kind: proposal.kind ?? "",
    status: proposal.status ?? "",
    target_id: target,
    target_ids: proposal.target_ids.length > 0 ? [...proposal.target_ids] : target === "" ? [] : [target],
    branch: proposal.branch ?? "",
    created_at: proposal.created_at ?? "",
    rationale: proposal.rationale,
    severity: proposal.severity,
    summary: proposal.summary,
    record_id: proposal.record_id,
    staged_at: proposal.staged_at,
  };
}

/** `spec review PR --json` for a PR the queue lacks (exit 1, the daemon's 404 as data). */
export function noReview(id: string): Proposal {
  return {
    id: null,
    project: null,
    kind: null,
    status: null,
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
    diagnostics: null,
    diff: null,
    preview: null,
    conflict: null,
    revisions: null,
    drift: null,
    decided_by: null,
    decided_at: null,
    decision_note: null,
    applied_commit: null,
    created_at: null,
    updated_at: null,
    target_ids: [],
    severity: null,
    gap_type: null,
    summary: null,
    working_answer: null,
    price_of_other: null,
    evidence: [],
    options: [],
    recommendation: null,
    distinct_from: [],
    linked: null,
    record_id: null,
    record_path: null,
    record_title: null,
    record_text: null,
    choice: null,
    task_id: null,
    staged: null,
    staged_at: null,
    interpretation: null,
    card_id: null,
    why: null,
    notes: [`no proposal \`${id}\` in this project's queue`],
  };
}

/** A whole inbox entry for tests. */
export function anEntry(fields: Partial<Proposal> & { id: string }): InboxEntry {
  return entryOf(aProposal(fields));
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
    // A non-UTF-8 file's whole-file node has none (docs/features/norm-hash.md "Data").
    norm_hash: fields.utf8 === false ? null : "b3:1111111111111111111111111111111111111111111111111111111111111111",
    span_hash: "b3:0000000000000000000000000000000000000000000000000000000000000000",
    text: `## ${fields.id ?? "Node"}\n\nCurrent text.\n`,
    truncated: false,
    omitted: null,
    ancestors: [],
    links: null,
    bindings: null,
    history: null,
    ...fields,
  };
}

/** One history entry for tests: a hand commit unless told, absent keys null (docs/features/node-history.md "Acceptance criteria", NH12). */
export function aHistoryEntry(fields: Partial<HistoryEntry> & Pick<HistoryEntry, "id">): HistoryEntry {
  return {
    source: "commit",
    at: "2026-01-01T10:00:00Z",
    change: "hand",
    created: false,
    rev: 1,
    rev_before: 1,
    by: "Ann Owner <ann@example.org>",
    by_name: "Ann Owner",
    proposal: null,
    proposed_by: null,
    proposer: null,
    record: null,
    title: null,
    rationale: null,
    ...fields,
  };
}

/** A shown node's history for tests: the entries, not truncated, nothing omitted unless told. */
export function aHistory(entries: HistoryEntry[], fields: Partial<ShownHistory> = {}): ShownHistory {
  return { entries, truncated: false, omitted: 0, ...fields };
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
    // The other end's title and kind (docs/features/doc-modal.md "Acceptance criteria", DM-1): none unless told.
    title: null,
    end_kind: null,
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

/** One code binding row for tests: an item judged in sync, exactly anchored; omitted keys as given. */
export function aBindingRow(fields: Partial<BindingRow> & Pick<BindingRow, "node">): BindingRow {
  return {
    relation: "implements",
    file: "src/a.rs",
    kind: "item",
    symbol: "crate::a::run",
    item_kind: "function_item",
    line: 10,
    end_line: 20,
    signature: "pub fn run()",
    layer: "a",
    sync: "ok",
    category: null,
    detail: null,
    anchor: { state: "exact", from: null, candidates: null },
    ...fields,
  };
}

/** A row the bundle placed for tests: the row, then its token estimate. */
export function aBundleRow(fields: Partial<BundleBindingRow> & Pick<BundleBindingRow, "node">): BundleBindingRow {
  const { tokens_est: tokens, ...row } = fields;
  return { ...aBindingRow(row), tokens_est: tokens ?? 12 };
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

/** A row's line above 0, as `spec flow` words it; the policy keeps this file off the mocks, so `src/mocks/flow.test.ts` holds this table to theirs. */
export const GAP_REMEDIES: Readonly<Record<string, string>> = {
  "no-start": "no step here is a start: match the flow's first step in [[flow.start]], or, reading a part, add a follows link into it from outside",
  "two-starts": "more than one start: narrow [[flow.start]] to one",
  unreachable: "no path from a start reaches it: add a follows link on it, naming a step before it",
  "no-way-to-end": "no path from it reaches an end: have a later step follow it, or match it in [[flow.end]]",
  "trap-cycle": "a loop with no way out: have a step outside it follow a member, or match a member in [[flow.end]]",
  "unjoined-parallel": "a parallel fork with one branch, or branches that never meet: add a branch, or a later step following them all",
  "cycle-no-repeat": "a loop with no repeat edge: write the edge closing it as {to: ID, kind: repeat}",
  "definition-unused": "no document uses it: add a uses link to it, or drop it from [[flow.definition]]",
  "definition-once": "only one document uses it: add a uses link from a second document, or fold it into that document",
  "supply-unfed": "these only supply each other: add a supplies link into them from a node outside",
};

/** A measured gap row for tests: above 0 it carries its remedy, at 0 `null`, as the daemon sends it. */
export function aFlowGap(code: string, items: string[][] = [], fields: Partial<FlowGap> = {}): FlowGap {
  // Each member untitled unless told (docs/features/doc-modal.md "Acceptance criteria", DM-3): `titles` has `items`' shape.
  const titles = items.map((item) => item.map(() => null));
  return { code, state: "measured", count: items.length, items, titles, cause: items.length > 0 ? (GAP_REMEDIES[code] ?? null) : null, ...fields };
}

/** The ten gap rows in table order, all measured at 0, `rows` replacing theirs by code. */
export function tenGapRows(rows: FlowGap[] = []): FlowGap[] {
  const codes = ["no-start", "two-starts", "unreachable", "no-way-to-end", "trap-cycle", "unjoined-parallel", "cycle-no-repeat", "definition-unused", "definition-once", "supply-unfed"];
  return codes.map((code) => rows.find((row) => row.code === code) ?? aFlowGap(code));
}

/** One box of a flow read for tests: a live step, not a group. */
export function aFlowNode(fields: Partial<FlowNode> & Pick<FlowNode, "id" | "distance">): FlowNode {
  return {
    kind: "widget",
    title: `Title of ${fields.id ?? "the node"}`,
    path: `docs/spec/${(fields.id ?? "node").toLowerCase()}.md`,
    line: 1,
    group: null,
    stage: null,
    start: false,
    end: false,
    in_flow: true,
    holds: 0,
    inner: [],
    gaps: 0,
    rollup: null,
    counts: null,
    ...fields,
  };
}

/** One edge of a flow read for tests: a plain edge of an invented type, written once. */
export function aFlowEdge(fields: Partial<FlowEdge> & Pick<FlowEdge, "from" | "to">): FlowEdge {
  return {
    type: "alpha_link",
    kind: null,
    when: null,
    count: 1,
    path: `docs/spec/${fields.to.toLowerCase()}.md`,
    line: 3,
    ...fields,
  };
}

/**
 * The served `types` of a test's flow reads: the stub's two invented types and a third, in the order
 * the daemon would serve its own (docs/features/flow-scenario.md D3); never a real link type's name.
 */
export const FLOW_TYPES: readonly string[] = ["alpha_link", "beta_link", "gamma_link"];

/** A whole flow read for tests: every key present, the ten rows at 0, the served types, no cut. */
export function aFlowView(nodes: FlowNode[], edges: FlowEdge[], fields: Partial<FlowView> = {}): FlowView {
  return {
    root: null,
    reason: null,
    depth: 0,
    notes: [],
    left_out: { generated: 0, tier3: 0 },
    truncated: false,
    omitted_nodes: 0,
    omitted_edges: 0,
    not_in_flow: null,
    types: [...FLOW_TYPES],
    gaps: tenGapRows(),
    nodes,
    edges,
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
    tested: fields.tested ?? null,
    spec_snapshot: fields.spec_snapshot ?? null,
    snapshot_diff: fields.snapshot_diff ?? null,
    claim: fields.claim ?? null,
    runs: fields.runs ?? [],
    bundle: fields.bundle ?? null,
    roadmap: fields.roadmap ?? null,
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

/** A check finding for tests: the six keys, no `fix`, `debt` or `introduced` unless given. */
export function aCheckFinding(fields: Partial<CheckFinding> & Pick<CheckFinding, "code">): CheckFinding {
  return {
    severity: "error",
    path: "docs/spec/doc-1.md",
    line: 1,
    subject: "",
    message: `${fields.code} found`,
    ...fields,
  };
}

/**
 * A `spec check --json` report for tests: `clean` under `enforce` with no finding unless given;
 * the counts are the test's own (the UI never derives them), seven of the nine keys by default.
 */
export function aCheckReport(fields: Partial<Omit<CheckReport, "counts">> & { counts?: Partial<CheckCounts> } = {}): CheckReport {
  const { counts, ...rest } = fields;
  return {
    mode: "enforce",
    verdict: "clean",
    findings: [],
    stale: [],
    cannot_check: [],
    ...rest,
    counts: { documents: 7, errors: 0, warnings: 0, debt: 0, expired: 0, stale: 0, worst_w_bytes: 4321, ...counts },
  };
}

/**
 * One unapplied amendment for tests, keys in `spec health --json`'s order: a draft spec with no ID
 * amending R-1, resolved, from its front matter's line 6 (docs/features/health-left.md "Data").
 */
export function anAmendment(fields: Partial<UnappliedAmendment> & Pick<UnappliedAmendment, "path">): UnappliedAmendment {
  const { path, ...rest } = fields;
  return {
    path,
    id: null,
    title: `Amending R-1 from ${path}`,
    class: "spec",
    status: "draft",
    links: [aLink({ type: "amends", written: "R-1", path, line: 6 })],
    ...rest,
  };
}

/** Row 4 or 5 of `spec health --json` for tests, the code layer off (docs/features/health-code.md "Data"). */
export function aCodeRowOff(): HealthAssumes & HealthUnbound {
  return { state: "off", causes: ["specengine.toml has no [code] table: the code layer is off (add [code] with roots)"], counts: null, rows: null };
}

/** Row 6 of `spec health --json` for tests, the code layer off: no counts, rows or tested, no note (docs/features/health-drift.md "Data"). */
export function aDriftOff(): HealthDrift {
  return { state: "off", causes: ["specengine.toml has no [code] table: the code layer is off (add [code] with roots)"], counts: null, rows: null, tested: null, notes: [] };
}

/** Row 4 or 5 of `spec health --json` for tests in a state the daemon does not measure: its causes, no counts, no rows. */
export function aCodeRow(state: string, causes: string[]): HealthAssumes & HealthUnbound {
  return { state, causes, counts: null, rows: null };
}

/** The queue item a row 4 line names, for tests, keys in the JSON's order: an open discrepancy, nothing decided. */
export function anAssumedProposal(fields: Partial<AssumedProposal> = {}): AssumedProposal {
  return { kind: "discrepancy", status: "open", choice: null, record_id: null, record_path: null, reason: null, ...fields };
}

/** The node an `@assumes` names, for tests, keys in the JSON's order: accepted, no successor, no open item. */
export function anAssumedNode(fields: Partial<AssumedNode> & Pick<AssumedNode, "id">): AssumedNode {
  const { id, ...rest } = fields;
  return { id, path: "docs/spec/rules.md", line: 7, status: "accepted", successor: null, proposals: [], ...rest };
}

/** One row 4 line for tests, keys in the JSON's order: an `@assumes` resting on an open discrepancy. */
export function anAssumesRow(fields: Partial<AssumesRow> & Pick<AssumesRow, "id">): AssumesRow {
  const { id, ...rest } = fields;
  return { id, state: "pending", path: "src/lib.rs", line: 3, column: 4, relation: "assumes", proposal: anAssumedProposal(), node: null, ...rest };
}

/** Row 4 measured for tests: `rows` as given, over `markers` judged markers, one count per state as listed. */
export function measuredAssumes(rows: AssumesRow[], markers: number = rows.length): HealthAssumes {
  const of = (state: string) => rows.filter((row) => row.state === state).length;
  return {
    state: "measured",
    causes: [],
    counts: {
      markers,
      rows: rows.length,
      pending: of("pending"),
      as_assumed: of("as_assumed"),
      otherwise: of("otherwise"),
      applied: of("applied"),
      rejected: of("rejected"),
      unknown: of("unknown"),
      archived: of("archived"),
      in_question: of("in_question"),
    },
    rows,
  };
}

/** A marker naming an unbound node without binding it, for tests, keys in the JSON's order: one that cannot be verified. */
export function anUnboundMarker(fields: Partial<UnboundMarker> = {}): UnboundMarker {
  return { path: "src/lib.rs", line: 9, column: 4, relation: "implements", id_state: "resolved", binding: "cannot_verify", reason: "cannot_verify", ...fields };
}

/** One unbound node for tests, keys in the JSON's order: a rule failing rule 1, no mention, no marker. */
export function anUnboundRow(fields: Partial<UnboundRow> & Pick<UnboundRow, "id">): UnboundRow {
  const { id, ...rest } = fields;
  return { id, kind: "rule", title: `Rule ${id}`, path: "docs/spec/rules.md", line: 5, rules: [1], mentions: 0, markers: [], ...rest };
}

/** Row 5 measured for tests: `rows` unbound of `selected` nodes under `rules` rules. */
export function measuredUnbound(rows: UnboundRow[], selected: number = rows.length + 3, rules = 1): HealthUnbound {
  return { state: "measured", causes: [], counts: { rules, selected, bound: selected - rows.length, unbound: rows.length }, rows };
}

/**
 * One row 6 line for tests, `spec verify --json`'s row, its 17 keys in the JSON's order: an
 * `implements` binding of an item verify finds `unlocked` (class `check`), no lock entry, nothing
 * compared, no anchor (docs/features/health-drift.md "Data").
 */
export function aDriftRow(fields: Partial<DriftRow> & Pick<DriftRow, "node">): DriftRow {
  const { node, ...rest } = fields;
  return {
    node,
    relation: "implements",
    file: "src/lib.rs",
    kind: "item",
    symbol: "alpha::lib::run",
    item_kind: "function_item",
    state: "unlocked",
    category: null,
    detail: null,
    class: "check",
    pre_existing: false,
    markers: [{ line: 3, rev: 1 }],
    node_rev: 1,
    lock: null,
    compared: null,
    levels: null,
    anchor: null,
    ...rest,
  };
}

/** One `[[tested]]` entry for tests, keys in the JSON's order: stale, the spec changed since the run vouched for rev 1. */
export function aTestedRow(fields: Partial<TestedRow> & Pick<TestedRow, "node">): TestedRow {
  const { node, ...rest } = fields;
  return { node, state: "stale", category: "spec_changed", rev: 1, node_rev: 2, commit: "0123456789abcdef0123456789abcdef01234567", verified_at: "2026-10-08", ...rest };
}

/**
 * Row 6 measured for tests: `rows` as given, verify's counts by their classes, `ok` rows in sync and
 * `skipped` markers as given; the test evidence `[]` and no note unless given.
 */
export function measuredDrift(rows: DriftRow[], fields: { ok?: number; skipped?: number; tested?: TestedRow[] | null; notes?: string[] } = {}): HealthDrift {
  const { ok = 4, skipped = 0, tested = [], notes = [] } = fields;
  const of = (value: string) => rows.filter((row) => row.class === value).length;
  return {
    state: "measured",
    causes: [],
    counts: { rows: rows.length + ok, ok, fix: of("fix"), check: of("check"), cannot: of("cannot"), pre_existing: 0, skipped },
    rows,
    tested,
    notes,
  };
}

/** Row 6 for tests in a state the daemon does not measure: its causes; no counts, rows or test evidence, no note. */
export function aDriftIn(state: string, causes: string[]): HealthDrift {
  return { state, causes, counts: null, rows: null, tested: null, notes: [] };
}

/** `spec health --json` for tests: the amendments as given over 7 documents (aCheckReport's), no note, rows 4-6 off. */
export function aHealthReport(amendments: UnappliedAmendment[] = [], fields: Partial<Omit<HealthReport, "amendments">> = {}): HealthReport {
  return { documents: 7, amendments, notes: [], code: null, assumes: aCodeRowOff(), unbound: aCodeRowOff(), drift: aDriftOff(), ...fields };
}

/** One item of a round sheet for tests: a question without options, nothing staged; omitted keys as the sheet gives them. */
export function aRoundItem(fields: Partial<RoundItem> & { id: string }): RoundItem {
  return {
    kind: "question",
    severity: "normal",
    gap_type: null,
    summary: `Summary of ${fields.id}`,
    working_answer: null,
    price_of_other: null,
    options: [],
    recommendation: null,
    evidence: [],
    targets: [],
    tasks: [],
    worktree: "/work/alpha",
    branch: "main",
    author: { type: "agent", role: "rust-developer", model: null, run: null },
    staged: null,
    staged_at: null,
    created_at: "2026-10-01T10:00:00Z",
    updated_at: "2026-10-01T10:00:00Z",
    ...fields,
  };
}

/** A round sheet of `items` as given, in that order: `more`, `minutes`, `notes` as passed or none. */
export function aRoundSheet(items: RoundItem[], fields: Partial<Omit<RoundSheet, "items">> = {}): RoundSheet {
  return { project: "alpha", items, more: 0, minutes: items.length === 0 ? 0 : 1, notes: [], ...fields };
}

/** A plan item for tests: open, `draft`, no priority, release, estimate, wait or task unless given. */
export function aRoadmapItem(fields: Partial<RoadmapItem> & Pick<RoadmapItem, "id">): RoadmapItem {
  return {
    position: null,
    title: `Title of ${fields.id}`,
    path: `plan/${fields.id}.md`,
    status: "draft",
    release: null,
    priority: null,
    estimate: null,
    waits_on: [],
    cycle: false,
    tasks: [],
    ...fields,
  };
}

/** The sum of items as the plan adds it: estimated bounds added, the rest counted. */
export function sumOfItems(items: readonly RoadmapItem[]): RoadmapSum {
  return items.reduce<RoadmapSum>(
    (sum, item) =>
      item.estimate === null
        ? { ...sum, unestimated: sum.unestimated + 1 }
        : { ...sum, min: sum.min + item.estimate.min, max: sum.max + item.estimate.max },
    { min: 0, max: 0, unestimated: 0 },
  );
}

/**
 * A group for tests: `release` with a name, else the pool or unplaced (name null); its items as
 * given, a release's numbered from 1 unless a position is given; open, done and sum from them.
 */
export function aRoadmapGroup(group: string, name: string | null, items: RoadmapItem[], fields: Partial<Omit<RoadmapGroup, "items">> = {}): RoadmapGroup {
  const placed = items.map((item, index) => (group === "release" && item.position === null ? { ...item, position: index + 1, release: item.release ?? name } : item));
  return { group, name, current: false, mvp_cut: false, open: placed.length, done: 0, sum: sumOfItems(placed), items: placed, ...fields };
}

/** A whole plan for tests, every key present: counts, current and total from the groups unless given; no cut. */
export function aRoadmap(groups: RoadmapGroup[], fields: Partial<RoadmapDocument> = {}): RoadmapDocument {
  const releases = groups.filter((group) => group.group === "release");
  return {
    release: null,
    pool: false,
    reason: null,
    prefix: "RM",
    dir: "plan",
    unit: "days",
    current: releases.find((group) => group.current)?.name ?? null,
    counts: { open: groups.reduce((count, group) => count + group.open, 0), done: groups.reduce((count, group) => count + group.done, 0) },
    cut: null,
    total: sumOfItems(releases.flatMap((group) => group.items)),
    groups,
    pool_tasks: [],
    notes: [],
    truncated: false,
    omitted: 0,
    ...fields,
  };
}

/** A flow for the Board's tests: invented columns, every ordered pair of the open ones a move, as a reader expands them. */
export function aCardFlow(fields: Partial<CardFlow> & { name?: string } = {}): CardFlow {
  return {
    name: "work",
    // Untitled, a flow's title is its name, as the daemon serves it.
    title: fields.name ?? "work",
    disciplines: [],
    columns: [
      { id: "todo", title: "To do" },
      { id: "doing", title: "Doing" },
      { id: "check", title: "Check" },
      { id: "closed", title: "Closed" },
    ],
    moves: [
      { from: "todo", to: "doing", by: [] },
      { from: "todo", to: "check", by: [] },
      { from: "doing", to: "todo", by: [] },
      { from: "doing", to: "check", by: [] },
      { from: "check", to: "todo", by: [] },
      { from: "check", to: "doing", by: [] },
    ],
    accept: { from: "check", to: "closed" },
    return: null,
    ...fields,
  };
}

/** A row as `spec card list --json` gives it; omitted keys null or empty. */
export function aCardRow(fields: Partial<CardRow> & { id: string }): CardRow {
  return {
    flow: "work",
    column: "todo",
    position: 1,
    title: `Card ${fields.id}`,
    discipline: null,
    targets: [],
    assignee: null,
    reviewer: null,
    mandate: null,
    estimate: null,
    due: null,
    parent: null,
    corrects: null,
    updated_at: "2026-10-30T09:00:00Z",
    revision: "1",
    accepted: false,
    cancelled: false,
    mark: "open",
    on: null,
    discrepancy: null,
    correction: null,
    ...fields,
  };
}

/** `spec card list --json` over rows; the flow is one by default. */
export function aCardList(cards: CardRow[], fields: Partial<CardList> = {}): CardList {
  return { flows: [aCardFlow()], cards, notes: [], truncated: false, omitted: null, ...fields };
}

/** `spec card show C --history --json`; omitted keys null or empty. */
export function aCard(fields: Partial<Card> & { id: string }): Card {
  return {
    flow: "work",
    column: "todo",
    position: 1,
    title: `Card ${fields.id}`,
    goal: null,
    discipline: null,
    targets: [],
    mark: null,
    on: null,
    discrepancy: null,
    correction: null,
    snapshot: null,
    criteria: [],
    assignee: null,
    reviewer: null,
    mandate: null,
    estimate: null,
    due: null,
    parent: null,
    corrects: null,
    evidence: [],
    tasks: [],
    items: [],
    subtask_items: [],
    acceptance: null,
    cancellation: null,
    created_by: "local",
    created_at: "2026-10-29T09:00:00Z",
    updated_at: "2026-10-30T09:00:00Z",
    revision: "1",
    allowed: [],
    history: [],
    context: null,
    notes: [],
    ...fields,
  };
}

/** A write's document for a card. */
export function aCardWrite(id: string, fields: Partial<CardWrite> = {}): CardWrite {
  return { id, flow: "work", column: "todo", position: 1, revision: "2", allowed: [], notes: [], ...fields };
}

/** A card with its mark over a node, as `spec trace --json` gives it. */
export function aTraceCard(fields: Partial<TraceCard> & { id: string }): TraceCard {
  return { flow: "work", column: "todo", title: `Card ${fields.id}`, mark: "open", on: null, discrepancy: null, correction: null, ...fields };
}

/** One node of `spec trace --json`. */
export function aTraceNode(id: string, cards: TraceCard[], fields: Partial<TraceNode> = {}): TraceNode {
  return { id, title: null, path: `docs/spec/${id}.md`, stage: null, distance: null, rollup: "in work", counts: { no_card: 0, in_work: 1, suspect: 0, departed: 0, confirmed: 0 }, cards, uses: [], ...fields };
}

/** `spec trace --json` over nodes. */
export function aTrace(nodes: TraceNode[], fields: Partial<TraceDocument> = {}): TraceDocument {
  return { ref: null, reason: null, notes: [], counts: { no_card: 0, in_work: 0, suspect: 0, departed: 0, confirmed: 0 }, stages: [], nodes, truncated: false, omitted: null, ...fields };
}

/** The implicit workspace's eight sections: `sections.ts`'s nine but `tasks`, in order (docs/features/workspaces.md D3). */
export const IMPLICIT_SECTIONS = ["inbox", "board", "roadmap", "flow", "tree", "graph", "health", "questions"];

/** One workspace; omitted keys as the implicit one's (docs/features/workspaces.md "Data"). */
export function aWorkspace(fields: Partial<Workspace> & Pick<Workspace, "name">): Workspace {
  return { title: fields.name, landing: "overview", sections: [...IMPLICIT_SECTIONS], flows: null, disciplines: [], module: null, ...fields };
}

/** One rooted module: `kinds` null. */
export function aModule(fields: Partial<WorkspaceModule> & { name: string; root: string }): WorkspaceModule {
  return { title: fields.name, kinds: null, ...fields };
}

/** One direction: a module by node kind, `root` null (docs/canon/workspaces.md "The lens"). */
export function aDirection(fields: Partial<WorkspaceModule> & { name: string; kinds: string[] }): WorkspaceModule {
  return { title: fields.name, root: null, ...fields };
}

/** `spec workspace --json` over a project with no table: the implicit document of docs/features/workspaces.md "Data". */
export function anImplicitWorkspaces(notes: string[] = []): WorkspaceDocument {
  return { source: "implicit", notes, workspaces: [aWorkspace({ name: "all", title: "All sections" })], modules: [] };
}

/** A configured workspaces document. */
export function aWorkspaces(workspaces: Workspace[], modules: WorkspaceModule[] = [], fields: Partial<WorkspaceDocument> = {}): WorkspaceDocument {
  return { source: "config", notes: [], workspaces, modules, ...fields };
}
