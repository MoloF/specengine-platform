// PROVISIONAL — hand-written until `spec serve` generates types from Rust; replace, do not extend; generated types win.
//
// Each shape copies documented JSON: keys as the JSON, absent = null. `kind` and `contour` are
// project vocabulary (ADR-0031), plain strings. A closed table lists its literals and still accepts
// any other string, which the UI shows raw in a neutral badge.

/** A value outside a closed table, kept verbatim. Source: `docs/features/ui-shell.md` "Data". */
export type Unlisted = string & Record<never, never>;

/** A project the daemon serves (MISSING ENDPOINT GET /api/projects). Source: `docs/features/ui-shell.md` "Data". */
export interface Project {
  slug: string;
  name: string;
}

/** Queue order, never a hold on work (ADR-0012). Source: `docs/specs/specengine-platform/05-architecture.md` "3.3. Index schema (SQLite)". */
export type KnownSeverity = "high" | "normal" | "low";

/** Severity as sent; other strings kept. Source: `docs/features/ui-shell.md` "Data". */
export type Severity = KnownSeverity | Unlisted;

/** The seven proposal states. Source: `docs/specs/specengine-platform/05-architecture.md` "3.3. Index schema (SQLite)". */
export type KnownProposalStatus =
  | "open"
  | "changes_requested"
  | "approved"
  | "applied"
  | "rejected"
  | "deferred"
  | "superseded";

/** A proposal state as sent; other strings kept. Source: `docs/features/ui-shell.md` "Data". */
export type ProposalStatus = KnownProposalStatus | Unlisted;

/** Gap type of a discrepancy. Source: `docs/specs/specengine-platform/05-architecture.md` "7. Proposals, owner queue and gates". */
export type KnownGapType = "missing" | "partial" | "contradicts" | "unrequested";

/** A gap type as sent; other strings kept. Source: `docs/features/ui-shell.md` "Data". */
export type GapType = KnownGapType | Unlisted;

/** Read-only apply preview of an open proposal. Source: `docs/canon/proposal-queue.md` "Commands". */
export type KnownPreview = "applies" | "rebases" | "conflicts" | "unavailable";

/** A preview as sent; other strings kept. Source: `docs/features/ui-shell.md` "Data". */
export type Preview = KnownPreview | Unlisted;

/** Who wrote a proposal. Source: `docs/canon/proposal-queue.md` "Store". */
export interface Author {
  type: "human" | "agent" | Unlisted;
  role: string | null;
  model: string | null;
  run: string | null;
}

/** What the code says against what the spec says. Source: `docs/specs/specengine-platform/06-workflows.md` "3.2. What the agent sends". */
export interface Evidence {
  file: string;
  qpath: string | null;
  lines: string | null;
  observed: string;
  documented: string;
}

/** One way to settle a discrepancy, with its price. Source: `docs/specs/specengine-platform/06-workflows.md` "3.2. What the agent sends". */
export interface ProposalOption {
  label: string;
  effect: string;
  price: string;
}

/** A check finding's severity. Source: `docs/canon/spec-check.md` "Findings, debt, verdict". */
export type FindingSeverity = "error" | "warning" | Unlisted;

/** A finding the proposal's patch introduces. Source: `docs/canon/spec-check.md` "Findings, debt, verdict". */
export interface Finding {
  code: string;
  severity: FindingSeverity;
  path: string;
  line: number;
  subject: string;
  message: string;
}

/**
 * A queue card: the `review` JSON plus the queue fields and the agent's summary.
 * Sources: `docs/canon/proposal-queue.md` "Commands"; `docs/specs/specengine-platform/05-architecture.md` "3.3. Index schema (SQLite)";
 * `docs/specs/specengine-platform/07-interfaces.md` "1.2. Tools (`core` set)".
 */
export interface Proposal {
  id: string;
  project: string;
  kind: string;
  status: ProposalStatus;
  target_id: string | null;
  target_path: string | null;
  worktree: string | null;
  branch: string | null;
  base_commit: string | null;
  base_hash: string | null;
  base_text: string | null;
  new_text: string | null;
  patch_hash: string | null;
  rationale: string | null;
  author: Author | null;
  diagnostics: Finding[];
  /** Pre-computed unified hunks; the UI never computes a diff. */
  diff: string | null;
  preview: Preview | null;
  conflict: string | null;
  decided_by: string | null;
  decided_at: string | null;
  decision_note: string | null;
  applied_commit: string | null;
  /** UTC, as stored, e.g. 2026-10-05T21:14:03Z. */
  created_at: string;
  updated_at: string;
  notes: string[];
  severity: Severity | null;
  gap_type: GapType | null;
  task_id: string | null;
  target_ids: string[];
  evidence: Evidence[];
  options: ProposalOption[];
  /** Index into `options`. */
  recommendation: number | null;
  working_answer: string | null;
  summary: string | null;
}

/** The owner's queue, as `spec inbox --json`. Source: `docs/canon/proposal-queue.md` "Commands". */
export interface Inbox {
  proposals: Proposal[];
  notes: string[];
}

/** Where `show` cut a node at the output cap. Source: `crates/specengine-cli/README.md` "Output and the cap". */
export interface Omitted {
  lines: [number, number];
  sections: string[];
  sections_more: number;
  holders: string[];
  holders_more: number;
}

/** Where a link is written. Source: `docs/canon/spec-cli-graph.md` "spec show --links". */
export type LinkOrigin = "frontmatter" | "inline" | Unlisted;

/** What a link resolved to. Source: `docs/canon/spec-cli-graph.md` "spec show --links". */
export type LinkState = "resolved" | "dangling" | "skipped" | "unchecked" | Unlisted;

/** Links the live rule left out. Source: `docs/canon/spec-cli-graph.md` "spec show --links". */
export interface LeftOut {
  generated: number;
  tier3: number;
}

/** One link of a shown node; `type` becomes a closed table with the graph slice. Source: `docs/canon/spec-cli-graph.md` "spec show --links". */
export interface ShownLink {
  type: string;
  origin: LinkOrigin;
  at: string | null;
  name: string | null;
  written: string;
  path: string;
  line: number;
  state: LinkState;
  reason: string | null;
}

/** A shown node's links (`null` without `--links`). Source: `docs/canon/spec-cli-graph.md` "spec show --links". */
export interface ShownLinks {
  outgoing: ShownLink[];
  incoming: ShownLink[];
  left_out: LeftOut;
  omitted: number;
}

/** One node as `spec show --json` prints it. Source: `crates/specengine-cli/README.md` "Output and the cap". */
export interface ShownNode {
  id: string | null;
  kind: string | null;
  title: string | null;
  path: string;
  line: number;
  end_line: number;
  status: string | null;
  rev: number | null;
  tokens_est: number;
  archived: boolean;
  utf8: boolean;
  sections: string[];
  /** `b3:` of the whole span's bytes read: a proposal's base (`propose --base`). */
  span_hash: string;
  text: string;
  truncated: boolean;
  omitted: Omitted | null;
  links: ShownLinks | null;
}

/** `spec show --json`: the nodes, or the reason there are none. Source: `crates/specengine-cli/README.md` "Output and the cap". */
export interface NodeView {
  ref: string;
  reason: string | null;
  notes: string[];
  nodes: ShownNode[];
}

/** Why a tree row is a root although its document declares a parent. Source: `docs/canon/spec-cli-graph.md` "spec tree". */
export type TreeMark = "dangling-parent" | "parent-cycle" | Unlisted;

/** One line of `spec tree --json`, flat pre-order. Source: `docs/canon/spec-cli-graph.md` "spec tree". */
export interface TreeNode {
  id: string | null;
  kind: string | null;
  title: string | null;
  path: string;
  line: number;
  /** Counted from the roots (0); a row's parent is the nearest earlier row one level up. */
  depth: number;
  /** The name of the node this row is listed under; null for a root. */
  parent: string | null;
  mark: TreeMark | null;
  status: string | null;
  rev: number | null;
  tokens_est: number;
  archived: boolean;
}

/** `spec tree --json`: the containment tree, or the reason ROOT names nothing. Source: `docs/canon/spec-cli-graph.md` "spec tree". */
export interface TreeView {
  /** ROOT as given; null for the whole tree. */
  ref: string | null;
  reason: string | null;
  notes: string[];
  depth: number | null;
  kinds: string[];
  archive: boolean;
  left_out: LeftOut;
  truncated: boolean;
  nodes: TreeNode[];
}

/** One run of a search snippet: a hit or the text around it. Source: `docs/features/ui-tree-node.md` "Data". */
export interface SnippetSegment {
  text: string;
  hit: boolean;
}

/**
 * A search snippet as structure, in place of the store's `**` markers
 * (`crates/specengine-store/README.md` "Writes, reads, search"); `cut_start`, `cut_end`: text
 * goes on before or after it. Source: `docs/features/ui-tree-node.md` "Data".
 */
export interface Snippet {
  segments: SnippetSegment[];
  cut_start: boolean;
  cut_end: boolean;
}

/** One search hit. Source: `crates/specengine-cli/README.md` "Output and the cap". */
export interface SearchHit {
  id: string | null;
  kind: string | null;
  title: string | null;
  path: string;
  line: number;
  ord: number;
  archived: boolean;
  snippet: Snippet | null;
}

/** `spec search --json`, as the browser view gets it: uncut. Source: `crates/specengine-cli/README.md` "Output and the cap". */
export interface SearchResults {
  archive: boolean;
  hits: SearchHit[];
  kinds: string[];
  limit: number;
  notes: string[];
  query: string;
  /** Archived matches left out of `hits` (without `archive`). */
  tier3_left_out: number;
  truncated: boolean;
}

/** The form a bundle item takes. Source: `docs/canon/spec-cli-bundle.md` "Output". */
export type BundleForm = "text" | "outline" | "header" | "summary" | Unlisted;

/** The way a link is followed from the target. Source: `docs/canon/spec-cli-bundle.md` "Output". */
export type Direction = "in" | "out" | Unlisted;

/** A link type an item came in by. Source: `docs/canon/spec-cli-bundle.md` "Output". */
export interface BundleVia {
  type: string;
  direction: Direction;
}

/** An open question's working answer. Source: `docs/canon/spec-cli-bundle.md` "Output". */
export interface WorkingAnswer {
  name: string | null;
  written: string;
  path: string;
  line: number;
  state: LinkState;
}

/** One item of a bundle layer, without its text. Source: `docs/canon/spec-cli-bundle.md` "Output". */
export interface BundleItem {
  name: string;
  kind: string | null;
  title: string | null;
  path: string;
  line: number;
  form: BundleForm;
  status: string | null;
  via: BundleVia[] | null;
  working_answer: WorkingAnswer | null;
  tokens_est: number;
  archived: boolean;
}

/** The nine layers in print order, each `[]` when empty. Sources: `docs/canon/spec-cli-bundle.md` "Layers", "Output". */
export interface BundleLayers {
  targets: BundleItem[];
  open_questions: BundleItem[];
  ancestors: BundleItem[];
  criteria: BundleItem[];
  bindings: BundleItem[];
  decisions: BundleItem[];
  neighbours: BundleItem[];
  terms: BundleItem[];
  tests: BundleItem[];
}

/** A node the budget left out, named for a follow-up read. Source: `docs/canon/spec-cli-bundle.md` "Output". */
export interface TailEntry {
  name: string;
  title: string | null;
  path: string;
  line: number;
  tokens_est: number;
  /** The layer key it stands in. */
  layer: keyof BundleLayers | Unlisted;
}

/** `spec bundle --json`; exit 1 sets `refs`, `reason`, `notes` and leaves the rest null. Source: `docs/canon/spec-cli-bundle.md` "Output". */
export interface BundleView {
  refs: string[];
  reason: string | null;
  notes: string[];
  /** Phase 2's task bundles: always null. */
  task: null;
  budget: number | null;
  tokens: number | null;
  chars: number | null;
  bytes: number | null;
  bundle_hash: string | null;
  body: string | null;
  layers: BundleLayers | null;
  tail: TailEntry[] | null;
  more: number | null;
}

/**
 * The owner's decision on one proposal; `option` indexes `options`.
 * Sources: `docs/specs/specengine-platform/06-workflows.md` "3.4. What happens after the decision";
 * `docs/features/ui-shell.md` "Data".
 */
export type Decision =
  | { decision: "accept"; option: number | null; note: string | null }
  | { decision: "reject"; reason: string }
  | { decision: "needs_clarification"; note: string }
  | { decision: "defer"; note: string | null };

/** The commit `apply_proposal` made. Source: `docs/canon/proposal-apply.md` "Apply steps". */
export interface Commit {
  sha: string;
  subject: string;
}

/** The proposal after the decision and, for an applied one, its commit. Source: `docs/features/ui-shell.md` "Data". */
export interface DecisionResult {
  proposal: Proposal;
  commit: Commit | null;
}

/** The daemon's refusal; 409 = decided elsewhere. Source: `docs/features/ui-shell.md` "Data". */
export interface ApiError {
  status: number;
  message: string;
}
