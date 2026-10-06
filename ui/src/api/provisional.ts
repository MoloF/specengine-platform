// PROVISIONAL — hand-written until `spec serve` generates types from Rust; replace, do not extend; generated types win.
//
// Each shape copies documented JSON: keys as the JSON, absent = null. `kind` and `contour` are
// project vocabulary (ADR-0031), plain strings. A closed table lists its literals and still accepts
// any other string, which the UI shows raw in a neutral badge.

/** A value outside a closed table, kept verbatim. Source: `docs/features/ui-shell.md` "Data". */
export type Unlisted = string & Record<never, never>;

/**
 * A project the daemon serves, in `--root` order: `name` its `[project] name`, `root` canonical,
 * `branch` the root's current one (null detached or outside git). Source: `docs/features/daemon-read.md` "Data".
 */
export interface Project {
  slug: string;
  name: string | null;
  root: string;
  branch: string | null;
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

/** A check finding's two severities. Source: `docs/canon/spec-check.md` "Findings, debt, verdict". */
export type KnownFindingSeverity = "error" | "warning";

/** A check finding's severity as sent; other strings kept. Source: `docs/canon/spec-check.md` "Findings, debt, verdict". */
export type FindingSeverity = KnownFindingSeverity | Unlisted;

/**
 * The six keys every check finding has: a proposal's diagnostics carry these, `spec check --json`
 * adds three optional ones (CheckFinding). Source: `docs/canon/spec-check.md` "Findings, debt, verdict".
 */
export interface Finding {
  code: string;
  severity: FindingSeverity;
  path: string;
  line: number;
  subject: string;
  message: string;
}

// `spec check --json` (docs/features/ui-health.md "Data"): a key marked `?` is omitted when absent,
// never null (`docs/canon/spec-check-cli.md` "spec check", W-1); `code`, the budget slot in a
// `subject` and the mode are the core's strings, shown raw.

/**
 * The four verdicts of the check. The third is a check outcome, never a hold on work: shown as
 * "Fails the check". It is spelled on this one line alone in app code (ui-health AC-11), so the
 * type and the tuple the Health screen labels through share it.
 * Source: `docs/canon/spec-check.md` "Findings, debt, verdict".
 */
export type KnownCheckVerdict = (typeof KNOWN_CHECK_VERDICTS)[number]; export const KNOWN_CHECK_VERDICTS = ["clean", "observed", "blocked", "cannot-check"] as const;

/** A verdict as sent; other strings kept, shown raw. Source: `docs/canon/spec-check.md` "Findings, debt, verdict". */
export type CheckVerdict = KnownCheckVerdict | Unlisted;

/** The check's mode, `[check] mode`. Source: `docs/canon/spec-check.md` "Configuration". */
export type KnownCheckMode = "observe" | "enforce-introduced" | "enforce";

/** A mode as sent; other strings kept, shown raw. Source: `docs/canon/spec-check.md` "Configuration". */
export type CheckMode = KnownCheckMode | Unlisted;

/**
 * The summary counts: `errors` and `warnings` exclude live debt; `introduced` and `new_debt` only
 * with a git base; `worst_w_bytes` 0 on `cannot-check`. Source: `docs/canon/spec-check.md` "Findings, debt, verdict".
 */
export interface CheckCounts {
  documents: number;
  errors: number;
  warnings: number;
  debt: number;
  expired: number;
  stale: number;
  introduced?: number;
  new_debt?: number;
  worst_w_bytes: number;
}

/**
 * A finding as `spec check --json` prints it: the six keys, then `fix` (data, never applied here),
 * `debt` (the matched baseline entry; `expires` the last day it holds) and `introduced` (only with
 * a base). Source: `docs/canon/spec-check.md` "Findings, debt, verdict".
 */
export type CheckFinding = Finding & {
  fix?: { span: { start: number; end: number }; text: string };
  debt?: { reason: string; expires: string; expired: boolean };
  introduced?: boolean;
};

/**
 * A `.spec-debt.toml` entry; `line` is its line in that file (core `check/baseline.rs` `DebtEntry`),
 * never a line of `path`. Source: `docs/canon/spec-check.md` "Findings, debt, verdict".
 */
export interface DebtEntry {
  code: string;
  path: string;
  subject: string;
  reason: string;
  expires: string;
  line: number;
}

/** A baseline entry its base lacks or holds with an earlier `expires`: only with a base. Source: `docs/canon/spec-check.md` "Findings, debt, verdict". */
export type NewDebtEntry = DebtEntry & { head_expires?: string };

/** Why the check cannot vouch for the corpus; `path` `""` for none. Source: `docs/canon/spec-check.md` "Findings, debt, verdict". */
export interface CheckCause {
  path: string;
  message: string;
}

/**
 * `spec check --json`, the daemon's `check`: findings sorted by (path, line, code, subject,
 * message), `stale` the baseline entries that matched nothing, `cannot_check` non-empty exactly on
 * `cannot-check`. The report carries no time. Source: `docs/canon/spec-check.md` "Findings, debt, verdict".
 */
export interface CheckReport {
  mode: CheckMode;
  verdict: CheckVerdict;
  counts: CheckCounts;
  findings: CheckFinding[];
  stale: DebtEntry[];
  new_debt?: NewDebtEntry[];
  cannot_check: CheckCause[];
}

/** The owner's choice a decision record holds (JSON with one key). Source: `docs/features/decision-apply.md` "Data". */
export type Choice = { option: number } | { working_answer: true } | { answer: string };

/**
 * The review document of one proposal, as `spec review PR --json` (the daemon's `proposals/:id`):
 * every key present, absent `null`, lists `[]`. The exit-1 document (no such proposal here) has
 * every scalar `null` and its reason as the last of `notes`. A question or a discrepancy fills the
 * eleven keys after `updated_at` instead of an update's texts, `diff` and `preview`; a decided
 * one its record's five after `linked`.
 * Sources: `docs/canon/proposal-queue.md` "Commands"; `docs/canon/agent-intake.md` "Review document";
 * `docs/features/decision-apply.md` "Data"; `docs/features/daemon-read.md` "Data".
 */
export interface Proposal {
  id: string | null;
  project: string | null;
  kind: string | null;
  status: ProposalStatus | null;
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
  diagnostics: Finding[] | null;
  /** Pre-computed unified hunks; the UI never computes a diff. */
  diff: string | null;
  preview: Preview | null;
  conflict: string | null;
  decided_by: string | null;
  decided_at: string | null;
  decision_note: string | null;
  applied_commit: string | null;
  /** UTC, as stored, e.g. 2026-10-05T21:14:03Z. */
  created_at: string | null;
  updated_at: string | null;
  /** The canonical targets; an update's `[target_id]`. */
  target_ids: string[];
  severity: Severity | null;
  gap_type: GapType | null;
  /** A question's text, a discrepancy's summary. */
  summary: string | null;
  working_answer: string | null;
  /** What the other answer to a question would cost. */
  price_of_other: string | null;
  evidence: Evidence[];
  options: ProposalOption[];
  /** Index into `options`. */
  recommendation: number | null;
  /** The hits the author named as distinct when raising it. */
  distinct_from: string[];
  /** A discrepancy's proposed patch as its own update, and back. */
  linked: string | null;
  record_id: string | null;
  record_path: string | null;
  record_title: string | null;
  /** The record's bytes. */
  record_text: string | null;
  choice: Choice | null;
  /** Why a preview is unavailable, what a reader should know, a refusal's reason last. */
  notes: string[];
}

/**
 * One line of the owner's queue, `spec inbox --json`: `rationale` an update's first line (at most
 * 80 characters), `severity` and `summary` (its first line) a question's or a discrepancy's.
 * Sources: `docs/canon/proposal-queue.md` "Commands"; `docs/features/daemon-read.md` "Data".
 */
export interface InboxEntry {
  id: string;
  kind: string;
  status: ProposalStatus;
  /** The first canonical target. */
  target_id: string;
  /** Every stored canonical target; an update's `[target_id]`. */
  target_ids: string[];
  branch: string;
  /** UTC, as stored. */
  created_at: string;
  rationale: string | null;
  severity: Severity | null;
  summary: string | null;
  /** A decided question's or discrepancy's record. */
  record_id: string | null;
}

/** The owner's queue, as `spec inbox --json`: the current repository's open and approved proposals. Source: `docs/canon/proposal-queue.md` "Commands". */
export interface Inbox {
  proposals: InboxEntry[];
  notes: string[];
}

/**
 * One queue event of a project's live tail: the SSE `id`, `event` and `data` (the stored payload,
 * JSON with `id`). Sources: `docs/features/daemon-read.md` "Data"; `docs/canon/proposal-queue.md` "States and events".
 */
export interface QueueEvent {
  seq: number;
  type: string;
  payload: unknown;
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

/** One link of a shown node; `type` is corpus vocabulary, a plain string (ADR-0031). Source: `docs/canon/spec-cli-graph.md` "spec show --links". */
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

/** A link type and the way the walk follows it; `type` a plain string (ADR-0031). Source: `docs/canon/spec-cli-graph.md` "spec graph". */
export interface FollowedType {
  type: string;
  direction: Direction;
}

/** A node the walk reached, at its distance from the REF's holders. Source: `docs/canon/spec-cli-graph.md` "spec graph". */
export interface GraphNode {
  id: string | null;
  kind: string | null;
  title: string | null;
  path: string;
  line: number;
  distance: number;
  archived: boolean;
}

/** An edge met from a reached node, in link direction; an unresolved end is null, `written` as written. Source: `docs/canon/spec-cli-graph.md` "spec graph". */
export interface GraphEdge {
  src: string | null;
  type: string;
  dst: string | null;
  written: string;
  path: string;
  line: number;
  state: LinkState;
  reason: string | null;
}

/** `spec graph --json`: what the walk reached, or why REF names nothing (exit 1). Source: `docs/canon/spec-cli-graph.md` "spec graph". */
export interface GraphView {
  ref: string;
  reason: string | null;
  impact: boolean;
  types: FollowedType[];
  depth: number | null;
  archive: boolean;
  notes: string[];
  left_out: LeftOut;
  truncated: boolean;
  nodes: GraphNode[];
  edges: GraphEdge[];
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

/**
 * The daemon's error body, exactly these two keys (`message` the CLI's line(s) verbatim): 403 a
 * decision (made on a terminal), 503 a read that cannot run; 409 = decided elsewhere (the mock).
 * Sources: `docs/features/daemon-read.md` "Data"; `docs/features/ui-shell.md` "Data".
 */
export interface ApiError {
  status: number;
  message: string;
}

// Tasks (docs/features/ui-tasks.md "Data"): the shapes of the draft task package; re-pointed to
// its canon when task-package ships. `kind`, `role` and `profile` are plain strings (ADR-0027,
// ADR-0031): shown verbatim, never compared.

/**
 * The ten task states, as the keys of a record: two of them are also words of the mocks' spec
 * statuses, which app code never quotes. Source: `docs/specs/specengine-platform/05-architecture.md` "3.3. Index schema (SQLite)".
 */
interface TaskStates {
  draft: true;
  analysis: true;
  review: true;
  changes_requested: true;
  ready: true;
  in_progress: true;
  in_review: true;
  done: true;
  accepted: true;
  cancelled: true;
}

/** The ten task states. Source: `docs/specs/specengine-platform/05-architecture.md` "3.3. Index schema (SQLite)". */
export type KnownTaskStatus = keyof TaskStates;

/** A task state as sent; other strings kept. Source: `docs/features/task-package.md` "Data". */
export type TaskStatus = KnownTaskStatus | Unlisted;

/** A run's outcome (Caps: no other value, never a hold). Source: `docs/features/task-package.md` "Data". */
export type KnownRunOutcome = "completed" | "partial" | "failed" | "abandoned";

/** A run's outcome as sent; other strings kept. Source: `docs/features/task-package.md` "Data". */
export type RunOutcome = KnownRunOutcome | Unlisted;

/** One row of `spec task list --json`: `stale` as the package's, per read. Source: `docs/features/task-package.md` "Description and interactions". */
export interface TaskListEntry {
  id: string;
  status: TaskStatus;
  title: string | null;
  /** Canonical IDs or paths. */
  targets: string[];
  stale: boolean | null;
  /** UTC, as stored. */
  updated_at: string;
}

/** `spec task list --json`: the repository's tasks by number; a skipped row or a gone place in `notes`. Source: `docs/features/task-package.md` "Description and interactions". */
export interface TaskList {
  tasks: TaskListEntry[];
  notes: string[];
}

/** `spec task show T --json` for an unknown T (exit 1, the daemon's 404). Source: `docs/features/task-package.md` "Description and interactions". */
export interface TaskNotFound {
  id: string | null;
  reason: string;
}

/** A target resolved in the compared place; a gone node keeps its stored `id`, `path`, the rest null. Source: `docs/features/task-package.md` "Data". */
export interface TaskTarget {
  id: string | null;
  path: string | null;
  kind: string | null;
  title: string | null;
}

/** A criterion: a reference and its text there (null: gone), or free text. Source: `docs/features/task-package.md` "Data". */
export interface TaskCriterion {
  ref: string | null;
  text: string | null;
}

/** A working assumption: an open question's working answer, a discrepancy's recommended option. Source: `docs/features/task-package.md` "Data". */
export interface TaskAssumption {
  proposal: string;
  text: string;
}

/** An open or approved proposal on the task's nodes or bound to it. Source: `docs/features/task-package.md` "Data". */
export interface TaskProposal {
  id: string;
  kind: string;
  status: ProposalStatus;
  target_ids: string[];
  task_id: string | null;
  summary: string;
}

/** The owner's note of a `changes --note`, oldest first. Source: `docs/features/task-package.md` "Data". */
export interface OwnerNote {
  at: string;
  note: string;
}

/** Where the approval froze the spec (ADR-0032). Source: `docs/features/task-package.md` "Data". */
export interface SnapshotPlace {
  worktree: string;
  root_rel: string;
  branch: string;
  commit: string;
}

/** A node as frozen at approval. Source: `docs/features/task-package.md` "Data". */
export interface SnapshotNode {
  id: string;
  path: string;
  span_hash: string;
}

/** The spec as the owner approved it. Source: `docs/features/task-package.md` "Data". */
export interface SpecSnapshot {
  at: string;
  place: SnapshotPlace;
  nodes: SnapshotNode[];
}

/** A node changed since approval: unified hunks from the snapshot text, cut at 8 192 B. Source: `docs/features/task-package.md` "Data". */
export interface SnapshotDiff {
  id: string;
  path: string;
  /** The snapshot's (the old side). */
  span_hash: string;
  diff: string;
  cut: boolean;
}

/** The one claim of a task. Source: `docs/features/task-package.md` "Data". */
export interface TaskClaim {
  at: string;
  role: string;
  worktree: string;
  branch: string;
}

/** A run of the task; an open run has `ended_at` null. Source: `docs/features/task-package.md` "Data". */
export interface TaskRun {
  run: number;
  role: string;
  started_at: string;
  ended_at: string | null;
  outcome: RunOutcome | null;
  summary: string | null;
  changed_files: string[];
}

/** The bundle an agent gets for the targets. Source: `docs/features/task-package.md` "Data". */
export interface TaskBundle {
  node_ids: string[];
  budget: number;
  bundle_hash: string;
}

/**
 * `spec task show T --json`, uncut: every key present, absent null, lists []. `stale`, `snapshot_diff`,
 * `open_proposals` and `assumptions` are computed by the core per read. Source: `docs/features/task-package.md` "Data".
 */
export interface TaskPackage {
  schema_version: number;
  id: string;
  project: string;
  status: TaskStatus;
  title: string | null;
  goal: string | null;
  profile: string | null;
  stale: boolean | null;
  targets: TaskTarget[];
  criteria: TaskCriterion[];
  affected_nodes: string[];
  plan: string | null;
  assumptions: TaskAssumption[];
  open_proposals: TaskProposal[];
  owner_notes: OwnerNote[];
  /** Empty until Phase 3. */
  bindings: unknown[];
  spec_snapshot: SpecSnapshot | null;
  snapshot_diff: SnapshotDiff[] | null;
  claim: TaskClaim | null;
  runs: TaskRun[];
  bundle: TaskBundle | null;
  author: Author | null;
  created_at: string;
  updated_at: string;
  notes: string[];
}
