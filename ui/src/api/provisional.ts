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
 * An approve staged on an open proposal: the flags of `spec approve`, absent `null`; `option`
 * indexes `options`; `editorial` next, `true` for `--editorial` (the revisions kept, the vouching
 * `spec.lock` entries refreshed), else `null`, never `false` (7 keys); `span_hash` the
 * target's span hash when staged (an update's, a section-form create's), `null` for a deciding
 * kind or a file-form create.
 * Sources: `docs/canon/decision-staging.md` "The stage"; `docs/features/rev-discipline.md` "Review document, stage, queue".
 */
export interface StagedApprove {
  decision: "approve";
  option: number | null;
  answer: string | null;
  canon: string | null;
  editorial: true | null;
  note: string | null;
  span_hash: string | null;
}

/** A reject staged on an open proposal, with its reason. Source: `docs/canon/decision-staging.md` "The stage". */
export interface StagedReject {
  decision: "reject";
  reason: string;
}

/**
 * One staged choice of an open proposal, replaceable, kept only in the queue: an attribute, not a
 * status; confirmed only by `spec approve|reject PR` on a terminal.
 * Source: `docs/canon/decision-staging.md` "The stage".
 */
export type Stage = StagedApprove | StagedReject;

/**
 * What the UI stages, the decision POST's body less `updated_at`: the stage's fields without
 * `span_hash`, which the daemon reads itself. Source: `docs/canon/decision-staging.md` "Daemon".
 */
export type StageChoice = Omit<StagedApprove, "span_hash"> | StagedReject;

/**
 * The review document of one proposal, as `spec review PR --json` (the daemon's `proposals/:id`):
 * every key present, absent `null`, lists `[]`. The exit-1 document (no such proposal here) has
 * every scalar `null` and its reason as the last of `notes`. `revisions`, after `conflict`, the
 * nodes whose meaning the change alters and what approval does to their revisions. A question or a
 * discrepancy fills the eleven keys after `updated_at` instead of an update's texts, `diff` and
 * `preview`; a decided one its record's five after `linked`; `task_id`, after `choice`, the task it
 * was raised for; `staged`, `staged_at` after it, the owner's staged choice; `interpretation` after
 * them, an interpretation's decision and texts; `drift`, after `revisions`, a drift item's evidence
 * (48 keys).
 * Sources: `docs/canon/proposal-queue.md` "Commands"; `docs/canon/agent-intake.md` "Review document";
 * `docs/features/decision-apply.md` "Data"; `docs/features/daemon-read.md` "Data";
 * `docs/canon/tasks.md` "Task-bound proposals"; `docs/canon/decision-staging.md` "Documents";
 * `docs/features/kind-interpretation.md` "Data"; `docs/features/rev-discipline.md` "Review document, stage, queue";
 * `docs/features/drift-queue.md` "Review, inbox, round, stage".
 */
export interface Proposal {
  id: string | null;
  project: string | null;
  /**
   * The queue's kind, plain text (ADR-0031), `drift` among them
   * (`docs/features/drift-queue.md` "The item"): what the card offers follows the keys, never the kind.
   */
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
  /**
   * Each node whose `norm_hash` the change alters, in the daemon's order, a raise refused at
   * apply's step 6 among them (its `to` null, its `refused` the reason); `[]` when none does; null
   * when the kind writes no existing node or the preview conflicts or is unavailable. Shown as
   * sent: the UI never works out a revision.
   */
  revisions: RevisionChange[] | null;
  /**
   * A drift item's evidence as stored when it was raised; null on every other kind. A drift item is
   * decided on a terminal only, never staged: its `preview`, `diff`, `conflict`, `base_text`,
   * `new_text` and `revisions` are null.
   */
  drift: DriftEvidence | null;
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
  /** The task it was raised for (`--task`), never changed; null unbound. */
  task_id: string | null;
  /** The owner's staged choice, confirmed only on a terminal; null when none is staged. */
  staged: Stage | null;
  /** When it was staged, UTC as stored (`YYYY-MM-DDTHH:MM:SSZ`); null when none is staged. */
  staged_at: string | null;
  /** The accepted decision a change carries and how it was read; null on any proposal that carries none. */
  interpretation: Interpretation | null;
  /** Why a preview is unavailable, what a reader should know (a stale stage too), a refusal's reason last. */
  notes: string[];
}

/**
 * One node of the review document's `revisions`, keys in this order: `id` its canonical ID, `rev`
 * its revision in the file now, `to` the revision a meaning approve writes (the daemon's number,
 * never `rev + 1` worked out here), null when that raise is refused; `lock_entries` the
 * `spec.lock` entries vouching for the old text that an editorial approve refreshes (0 without a
 * lock; null when the lock is unreadable, a note saying why); `refused` null, or the daemon's
 * reason a meaning approve cannot raise the node (apply's step 6: a closed class without `rev`,
 * the highest revision, a revision that cannot be written; each names `--editorial`), `to` then
 * null. Sources: `docs/features/rev-discipline.md` "Review document, stage, queue";
 * `docs/canon/node-revision.md` "Raising".
 */
export interface RevisionChange {
  id: string;
  rev: number;
  to: number | null;
  lock_entries: number | null;
  refused: string | null;
}

/**
 * A drift item's evidence, the review document's `drift`, keys in this order: `hash` its
 * `drift_hash` (`b3:` and 64 hex digits; an item raised again with another hash has new evidence),
 * `rows` the first 64 of its node's non-`ok` rows of `spec verify --json` at the commit judged, by
 * key, `more` how many rows are past them. A row stays untyped until Phase 4's drift screen: shown
 * as sent. Sources: `docs/features/drift-queue.md` "Review, inbox, round, stage";
 * `docs/features/drift-queue.md` "The item".
 */
export interface DriftEvidence {
  hash: string;
  rows: unknown[];
  more: number;
}

/**
 * What a proposal carrying an accepted decision into a rule holds, keys in this order: `decision`
 * the decision document's `id:`, `how` the decision was applied and `why` interpretation was
 * needed, the agent's texts raw (never blank, at most 2 048 bytes each).
 * Source: `docs/features/kind-interpretation.md` "Data".
 */
export interface Interpretation {
  decision: string;
  how: string;
  why: string;
}

/**
 * One line of the owner's queue, `spec inbox --json`: `rationale` an update's first line (at most
 * 80 characters), `severity` and `summary` (its first line) a question's or a discrepancy's, a
 * drift item's its rows' states; `staged_at` after `record_id` (12 keys). Sources:
 * `docs/canon/proposal-queue.md` "Commands"; `docs/features/daemon-read.md` "Data";
 * `docs/canon/decision-staging.md` "Documents"; `docs/features/drift-queue.md` "Review, inbox, round, stage".
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
  /** When the owner's choice was staged, UTC as stored; null when none is staged. */
  staged_at: string | null;
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

// The question round (docs/features/ui-round.md "Data"): the sheet is `spec round new --json`
// (`docs/canon/round.md` "JSON"), raw strings as stored, absent null, lists []; the answers are the
// decision route's body per item, and the daemon's document of what each became.

/**
 * A round item's target, resolved now: `id` null for a path target; `title` null when gone or
 * untitled. Sources: `docs/canon/round.md` "JSON"; `docs/features/ui-round.md` "Data".
 */
export interface RoundTarget {
  id: string | null;
  path: string | null;
  title: string | null;
}

/** A task waiting on a round item: neither done nor cancelled, its package would list the item. Source: `docs/canon/round.md` "JSON". */
export interface RoundTask {
  id: string;
  status: TaskStatus;
  title: string | null;
}

/**
 * One item of the round sheet, its 19 keys in this order: an open question or discrepancy of the
 * current repository, `staged` and `staged_at` its staged choice if any. `kind` is plain text,
 * shown raw; the controls follow its options and its working answer, never the kind.
 * Sources: `docs/canon/round.md` "JSON"; `docs/features/ui-round.md` "Data".
 */
export interface RoundItem {
  id: string;
  kind: string;
  severity: Severity | null;
  gap_type: GapType | null;
  summary: string;
  working_answer: string | null;
  price_of_other: string | null;
  options: ProposalOption[];
  /** Index into `options`; marked on screen, never picked for the owner. */
  recommendation: number | null;
  evidence: Evidence[];
  targets: RoundTarget[];
  /** Every waiting task, by ID. */
  tasks: RoundTask[];
  worktree: string;
  branch: string;
  author: Author;
  staged: Stage | null;
  staged_at: string | null;
  created_at: string;
  /** The version an answer is staged against (`updated_at` of the decision route's body). */
  updated_at: string;
}

/**
 * `spec round new [--limit N] --json`, the daemon's `round`: the items in round order, at most the
 * limit; `more` the items past it; `minutes` of reading of the items listed; `notes` what is not on
 * the sheet. Sources: `docs/canon/round.md` "JSON"; `docs/features/ui-round.md` "Data".
 */
export interface RoundSheet {
  project: string;
  items: RoundItem[];
  more: number;
  minutes: number;
  notes: string[];
}

/**
 * The decision route's body: the stage less `span_hash`, then `updated_at` as read
 * (`StageBody`). Sources: `docs/canon/decision-staging.md` "Daemon"; `docs/features/ui-round.md` "Data".
 */
export type StageBody = StageChoice & { updated_at: string };

/** One answer of the round POST: the item and its stage body. Source: `docs/features/ui-round.md` "Data". */
export interface RoundAnswerBody {
  id: string;
  stage: StageBody;
}

/** The round POST's body: the project it is for and the answers, in sheet order. Source: `docs/features/ui-round.md` "Data". */
export interface RoundAnswersBody {
  project: string;
  answers: RoundAnswerBody[];
}

/** What an answer became. Source: `docs/features/ui-round.md` "Data". */
export type KnownRoundOutcome = "staged" | "refused";

/** An answer's outcome as sent; other strings kept, shown raw. Source: `docs/features/ui-round.md` "Data". */
export type RoundOutcome = KnownRoundOutcome | Unlisted;

/**
 * One answer's outcome, in request order, `id` as sent: `stage` as stored and `staged_at` the
 * request's one clock reading when staged; `replaced` the `staged_at` of the choice it replaced;
 * `reason` the refusal, less its `; nothing staged`. Source: `docs/features/ui-round.md` "Data".
 */
export interface RoundAnswerEntry {
  id: string;
  outcome: RoundOutcome;
  stage: Stage | null;
  staged_at: string | null;
  replaced: string | null;
  reason: string | null;
}

/**
 * The round POST's 200 document, whatever the outcomes; `notes` a corrupt row as `spec inbox`
 * words it. Source: `docs/features/ui-round.md` "Data".
 */
export interface RoundAnswersOutcome {
  project: string;
  answers: RoundAnswerEntry[];
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
  /**
   * `b3:` of the node's meaning (`specengine-norm/v1`: its own text less headings and nested ID
   * sections; a document's front-matter links too), never a proposal's base; `null` for a document
   * whose front matter or references failed to parse, and for a non-UTF-8 file's whole-file node.
   * Source: `docs/features/norm-hash.md` "Data".
   */
  norm_hash: string | null;
  /** `b3:` of the whole span's bytes read: a proposal's base (`propose --base`). */
  span_hash: string;
  text: string;
  truncated: boolean;
  omitted: Omitted | null;
  links: ShownLinks | null;
  /**
   * The node's code bindings, after `links`: null without `with=bindings` (the UI never asks yet),
   * without `[code]` or under a cause that lists no row. Source: `docs/canon/code-reads.md` "spec show --bindings".
   */
  bindings: ShownBindings | null;
}

// Code in reads (docs/canon/code-reads.md): a node's code bindings as rows whose `sync` is the
// state `spec verify --node` gives them; every word (relation, kind, item kind, layer, state,
// category, detail) travels as data, shown raw or labelled, never a hold on work (ADR-0012).

/** verify's nine state words, in its table's order. Source: `docs/canon/spec-verify.md` "States". */
export type KnownSyncState =
  | "cannot_verify"
  | "predated"
  | "bad_marker"
  | "broken_binding"
  | "unlocked"
  | "spec_ahead"
  | "code_ahead"
  | "conflict"
  | "ok";

/** A row's sync: one of verify's states, or any other word, shown raw. Source: `docs/canon/spec-verify.md` "States". */
export type SyncState = KnownSyncState | Unlisted;

/** Where verify's anchor puts a binding. Source: `docs/canon/spec-verify-output.md` "JSON". */
export type AnchorState = "exact" | "moved" | "ambiguous" | "lost" | Unlisted;

/** A binding's place as an anchor names it: its key less the row's node and relation. Source: `docs/canon/code-reads.md` "The binding row". */
export interface BindingPlace {
  file: string;
  kind: string;
  symbol: string;
  item_kind: string | null;
}

/**
 * verify's anchor of a row: `from` a moved row's old place, `candidates` an ambiguous row's, in
 * key order, else null. Sources: `docs/canon/code-reads.md` "The binding row"; `docs/canon/spec-verify-output.md` "JSON".
 */
export interface BindingAnchor {
  state: AnchorState;
  from: BindingPlace | null;
  candidates: BindingPlace[] | null;
}

/**
 * One code binding of a node, its 14 keys in this order, all present, absent null: verify's row key
 * (`node` the canonical cited ID, unresolved as written, a bare marker `""`; a keyless row's `kind`,
 * `symbol`, `item_kind` null), the stored symbol's lines, signature and layer (a RON value: `line`
 * its first marker's), then verify's state, category, detail and anchor, each null when the row was
 * not judged (never `ok`). Source: `docs/canon/code-reads.md` "The binding row".
 */
export interface BindingRow {
  node: string;
  relation: string;
  file: string;
  kind: string | null;
  symbol: string | null;
  item_kind: string | null;
  line: number | null;
  end_line: number | null;
  signature: string | null;
  layer: string | null;
  sync: SyncState | null;
  category: string | null;
  detail: string | null;
  anchor: BindingAnchor | null;
}

/**
 * A `[[tested]]` block's state now, judged on each read by plain verify's one function, never stored,
 * never a verdict, exit or hold on work (ADR-0044, ADR-0012): `verified` the owner's passing run
 * still vouches; `stale` the spec or the bound code changed since; `unverifiable` the revision, the
 * hash or a binding not judgeable now; `node_gone` no node holds the ID (never in a read, whose set
 * holds present nodes only). Any other word, shown raw.
 * Sources: `docs/canon/spec-lock-tested.md` "The state"; `docs/canon/spec-lock-tested.md` "Reads".
 */
export type TestedState = "verified" | "stale" | "unverifiable" | "node_gone" | Unlisted;

/**
 * One `[[tested]]` block of a read's set and its state now, plain verify's entry, its 7 keys in this
 * order, all present: `node` the canonical ID; `category` a stale block's `spec_changed` or
 * `code_changed`, an unverifiable one's word, else null; `rev`, `commit`, `verified_at` the block's
 * (the revision vouched for, the commit the tests passed at, the run's UTC date); `node_rev` the
 * node's revision now, null when gone or unreadable. Evidence to read, never a hold on work.
 * Source: `docs/canon/spec-lock-tested.md` "Reads".
 */
export interface TestedRow {
  node: string;
  state: TestedState;
  category: string | null;
  rev: number;
  node_rev: number | null;
  commit: string;
  verified_at: string;
}

/**
 * A shown node's rows, how many the cap left out, and the `[[tested]]` entries of the node and its
 * sections by node bytes: `tested` null when not judged (rows listed under a cause, or the blocks
 * not judgeable, a note says why), [] with no block in its set, and [] too on a node the CLI's cap
 * cut before its closing line (the daemon's view is uncut). Sources: `docs/canon/code-reads.md` "spec show --bindings";
 * `docs/canon/spec-lock-tested.md` "Reads".
 */
export interface ShownBindings {
  rows: BindingRow[];
  omitted: number;
  tested: TestedRow[] | null;
}

/** `spec show --json`: the nodes, or the reason there are none. Source: `crates/specengine-cli/README.md` "Output and the cap". */
export interface NodeView {
  ref: string;
  reason: string | null;
  notes: string[];
  nodes: ShownNode[];
}

// `spec health --json` (docs/features/health-left.md "Data"): the live documents that declare an
// amends link, by path; `class` and `status` raw (ADR-0031), never compared; absent null, empty [].

/**
 * One live document declaring an amends link, not yet absorbed into the node it amends: `path`
 * root-relative; `id` the document node's ID, `title` its `title:` else first H1, `class` and
 * `status` raw, each null when absent; `links` its amends out links exactly as `spec show --links`
 * lists them, every state, never empty. Source: `docs/features/health-left.md` "Data".
 */
export interface UnappliedAmendment {
  path: string;
  id: string | null;
  title: string | null;
  class: string | null;
  status: string | null;
  links: ShownLink[];
}

/**
 * `spec health --json`, the daemon's `health`: `documents` the walked tree's documents (`spec check`'s
 * `counts.documents` on the same tree), the unapplied amendments by path, then the notes (a
 * front-matter that could not be judged), each without `note: `; after them rows 4-5: `code` the
 * stored code files' count, null unless the rows read the code index, `assumes` row 4, `unbound`
 * row 5, then row 6: `drift`, every key present. Sources: `docs/features/health-left.md` "Data";
 * `docs/features/health-code.md` "Data";
 * `docs/features/health-drift.md` "Data".
 */
export interface HealthReport {
  documents: number;
  amendments: UnappliedAmendment[];
  notes: string[];
  code: HealthCode | null;
  assumes: HealthAssumes;
  unbound: HealthUnbound;
  drift: HealthDrift;
}

// `spec health --json`'s rows 4-5 (docs/features/health-code.md "Data"): read from the code index
// the call refreshes; a row `measured`, `off`, `not_configured` or `cannot`, its counts and rows
// null unless measured. Words raw: a row's state, a marker's relation, `id_state` and `binding` as
// `spec refs` gives them.

/** A code row's state as sent; other strings kept (9b and later add words). Source: `docs/features/health-code.md` "Data". */
export type HealthRowState = "measured" | "off" | "not_configured" | "cannot" | Unlisted;

/**
 * `spec refs`' count of the stored code files: all, `.rs`, `.ron`, and the files the refresh
 * skipped (not UTF-8). Source: `docs/features/health-code.md` "Data".
 */
export interface HealthCode {
  files: number;
  rs: number;
  ron: number;
  skipped: number;
}

/**
 * Row 4: the markers naming a queue item, and the `@assumes` markers on an archived node or one in
 * question; `causes` the lines behind any state but `measured`, else `[]`; `counts`, `rows` null
 * unless `measured`. Source: `docs/features/health-code.md` "Data".
 */
export interface HealthAssumes {
  state: HealthRowState;
  causes: string[];
  counts: AssumesCounts | null;
  rows: AssumesRow[] | null;
}

/**
 * Row 5: the nodes `[[health.bound]]` selects that no marker binds; `causes` the lines behind any
 * state but `measured`, else `[]`; `counts`, `rows` null unless `measured`.
 * Source: `docs/features/health-code.md` "Data".
 */
export interface HealthUnbound {
  state: HealthRowState;
  causes: string[];
  counts: UnboundCounts | null;
  rows: UnboundRow[] | null;
}

/** Row 4's state of a marker, as sent; other strings kept. Source: `docs/features/health-code.md` "Data". */
export type AssumedState =
  | "pending"
  | "as_assumed"
  | "otherwise"
  | "applied"
  | "rejected"
  | "unknown"
  | "archived"
  | "in_question"
  | Unlisted;

/** Row 4's counts: the markers judged, the rows listed, one count per state. Source: `docs/features/health-code.md` "Data". */
export interface AssumesCounts {
  markers: number;
  rows: number;
  pending: number;
  as_assumed: number;
  otherwise: number;
  applied: number;
  rejected: number;
  unknown: number;
  archived: number;
  in_question: number;
}

/**
 * One row 4 line: a marker (and holder), `id` as written, its place, its relation raw; `proposal`
 * the queue item it names (null for `unknown` and a node's row), `node` the archived or questioned
 * node an `@assumes` names (null for a queue item's row). Source: `docs/features/health-code.md` "Data".
 */
export interface AssumesRow {
  id: string;
  state: AssumedState;
  path: string;
  line: number;
  column: number;
  relation: string;
  proposal: AssumedProposal | null;
  node: AssumedNode | null;
}

/**
 * The queue item a row 4 marker names: `kind` plain text (ADR-0031), `status` the queue's, `choice`
 * the stored one-key JSON, the record it applied to, `reason` a rejection's; null when absent.
 * Source: `docs/features/health-code.md` "Data".
 */
export interface AssumedProposal {
  kind: string;
  status: ProposalStatus;
  choice: Choice | null;
  record_id: string | null;
  record_path: string | null;
  reason: string | null;
}

/**
 * The node an `@assumes` marker names: canonical ID and place, its document's raw `status:`,
 * `successor` the X of `superseded-by X`, the `open`/`approved` items holding it, by ID.
 * Source: `docs/features/health-code.md` "Data".
 */
export interface AssumedNode {
  id: string;
  path: string;
  line: number;
  status: string | null;
  successor: string | null;
  proposals: string[];
}

/** Row 5's counts: the rules, the nodes they select, those bound and unbound. Source: `docs/features/health-code.md` "Data". */
export interface UnboundCounts {
  rules: number;
  selected: number;
  bound: number;
  unbound: number;
}

/**
 * One selected node no marker binds: `kind`, `title` raw; `rules` the 1-based indices of the rules
 * it fails; `mentions` those naming it; `markers` those naming it without binding.
 * Source: `docs/features/health-code.md` "Data".
 */
export interface UnboundRow {
  id: string;
  kind: string | null;
  title: string | null;
  path: string;
  line: number;
  rules: number[];
  mentions: number;
  markers: UnboundMarker[];
}

/** Why a marker naming a node binds it not, the first that applies; other strings kept. Source: `docs/features/health-code.md` "Data". */
export type UnboundReason = "homoglyph" | "unanchored" | "cannot_verify" | "relation" | Unlisted;

/**
 * A marker naming an unbound node without binding it: its place, relation, `id_state` and
 * `binding` as `spec refs` gives them, and why. Source: `docs/features/health-code.md` "Data".
 */
export interface UnboundMarker {
  path: string;
  line: number;
  column: number;
  relation: string;
  id_state: string;
  binding: string;
  reason: UnboundReason;
}

// `spec health --json`'s row 6 (docs/features/health-drift.md "Data"): plain `spec verify`'s
// judgement over the code index the call refreshes, read-only, never its mode or verdict. Its rows
// are verify's rows but `ok`, verbatim; its counts verify's; its `tested` verify's entries but
// `verified`. Words raw: a state through verify's states, a class, relation, kind, category,
// detail, `compared` and a level as sent. Nothing drawn until 9b-2.

/**
 * Row 6, drift: `causes` the cause lines behind any state but `measured`, each without `cannot: `
 * (`off`: rows 4-5's sentence), else `[]`; `counts`, `rows` and `tested` null unless `measured`, never
 * 0, never partial; `tested` [] with no block or every block `verified`, null when the `[[tested]]`
 * blocks cannot be judged; `notes` the row's own notes (today only `tested: <cause>`), without `note: `.
 * Source: `docs/features/health-drift.md` "Data".
 */
export interface HealthDrift {
  state: HealthRowState;
  causes: string[];
  counts: DriftCounts | null;
  rows: DriftRow[] | null;
  tested: TestedRow[] | null;
  notes: string[];
}

/**
 * Row 6's counts, `spec verify --json`'s seven verbatim: the bindings judged, `ok`, one per class,
 * `pre_existing` (0: plain verify has no base) and the markers skipped.
 * Sources: `docs/features/health-drift.md` "Data"; `docs/canon/spec-verify-output.md` "JSON".
 */
export interface DriftCounts {
  rows: number;
  ok: number;
  fix: number;
  check: number;
  cannot: number;
  pre_existing: number;
  skipped: number;
}

/**
 * One row 6 line, `spec verify --json`'s row verbatim, its 17 keys in this order, all present: the
 * lock key (`node` the canonical cited ID, unresolved as written, a bare marker `""`; a keyless row's
 * `kind`, `symbol`, `item_kind` null), verify's state, category and detail, `class` (`fix`, `check`,
 * `cannot`; null only for `ok`, which row 6 never lists), `pre_existing`, the markers (`[]` for a
 * lock-only row), the node's revision (null unknown or gone), the lock entry (null without one),
 * `compared` (`blob`, `ast` or null), the levels and the anchor, each null when verify gives none.
 * Sources: `docs/features/health-drift.md` "Data"; `docs/canon/spec-verify-output.md` "JSON".
 */
export interface DriftRow {
  node: string;
  relation: string;
  file: string;
  kind: string | null;
  symbol: string | null;
  item_kind: string | null;
  state: SyncState;
  category: string | null;
  detail: string | null;
  class: string | null;
  pre_existing: boolean;
  markers: DriftMarker[];
  node_rev: number | null;
  lock: DriftLock | null;
  compared: string | null;
  levels: DriftLevels | null;
  anchor: BindingAnchor | null;
}

/**
 * A row 6 line's marker: its line and `@N`, null when unversioned or malformed.
 * Sources: `docs/features/health-drift.md` "Data"; `docs/canon/spec-verify-output.md` "JSON".
 */
export interface DriftMarker {
  line: number;
  rev: number | null;
}

/**
 * A row 6 line's `spec.lock` entry: the revision vouched for, the commit, the UTC date.
 * Sources: `docs/features/health-drift.md` "Data"; `docs/canon/spec-verify-output.md` "JSON".
 */
export interface DriftLock {
  rev: number;
  commit: string;
  verified_at: string;
}

/**
 * A row 6 line's levels, each in canonical order: the lock entry's, its markers' united, and those
 * whose digest changed (null unless `compared` is `ast`). Words as sent.
 * Sources: `docs/features/health-drift.md` "Data"; `docs/canon/spec-verify-output.md` "JSON".
 */
export interface DriftLevels {
  lock: string[];
  markers: string[];
  changed: string[] | null;
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

/**
 * A code binding row the bundle placed in layer 5 or 9: the row, then its `tokens_est` (15 keys).
 * Source: `docs/features/bundle-bindings.md` "Layers 5 and 9".
 */
export type BundleBindingRow = BindingRow & { tokens_est: number };

/**
 * The nine layers in print order, each `[]` when empty; `bindings` (5) and `tests` (9) hold code
 * binding rows, not nodes, both empty without `[code]`. Sources: `docs/canon/spec-cli-bundle.md` "Layers", "Output";
 * `docs/features/bundle-bindings.md` "Layers 5 and 9".
 */
export interface BundleLayers {
  targets: BundleItem[];
  open_questions: BundleItem[];
  ancestors: BundleItem[];
  criteria: BundleItem[];
  bindings: BundleBindingRow[];
  decisions: BundleItem[];
  neighbours: BundleItem[];
  terms: BundleItem[];
  tests: BundleBindingRow[];
}

/**
 * Something the budget left out, named for a follow-up read: a node; or, in layer `bindings` or
 * `tests`, a code row (`name` its symbol, else its file; `title` `<relation> <node>`; `path` the
 * file; `line` its line, else 1), never a tree node. Sources: `docs/canon/spec-cli-bundle.md` "Output";
 * `docs/features/bundle-bindings.md` "Layers 5 and 9".
 */
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
 * The daemon's error body, exactly these two keys (`message` the CLI's line(s) verbatim): 400 a
 * bad stage or its flags' usage, 403 a request past the fence, 503 a read or a stage that cannot
 * run. Sources: `docs/features/daemon-read.md` "Data"; `docs/features/ui-shell.md` "Data";
 * `docs/canon/decision-staging.md` "Daemon".
 */
export interface ApiError {
  status: number;
  message: string;
}

// Tasks (docs/features/ui-tasks.md "Data"): the list and the exit-1 document as `docs/canon/tasks.md`
// writes them, the package and its parts as `docs/canon/task-package.md` does. `kind`, `role` and
// `profile` are plain strings (ADR-0027, ADR-0031): shown verbatim, never compared.

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

/** A task state as sent; other strings kept. Source: `docs/canon/tasks.md` "Transitions". */
export type TaskStatus = KnownTaskStatus | Unlisted;

/** A run's outcome (no other value, never a hold on work). Source: `docs/canon/task-package.md` "Caps". */
export type KnownRunOutcome = "completed" | "partial" | "failed" | "abandoned";

/** A run's outcome as sent; other strings kept. Source: `docs/canon/task-package.md` "Caps". */
export type RunOutcome = KnownRunOutcome | Unlisted;

/** One row of `spec task list --json`: `stale` as the package's, per read. Source: `docs/canon/tasks.md` "Commands". */
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

/** `spec task list --json`: the repository's tasks by number; a skipped row or a gone place in `notes`. Source: `docs/canon/tasks.md` "Commands". */
export interface TaskList {
  tasks: TaskListEntry[];
  notes: string[];
}

/** `spec task show T --json` for an unknown T (exit 1, the daemon's 404; `id` null for no task ID). Source: `docs/canon/tasks.md` "Commands". */
export interface TaskNotFound {
  id: string | null;
  reason: string;
}

/** A target resolved in the compared place; a gone node keeps its stored `id`, `path`, the rest null. Source: `docs/canon/task-package.md` "Package". */
export interface TaskTarget {
  id: string | null;
  path: string | null;
  kind: string | null;
  title: string | null;
}

/** A criterion: a reference and its text there (null: gone), or free text. Source: `docs/canon/task-package.md` "Package". */
export interface TaskCriterion {
  ref: string | null;
  text: string | null;
}

/** A working assumption: an open question's working answer, a discrepancy's recommended option. Source: `docs/canon/task-package.md` "Package". */
export interface TaskAssumption {
  proposal: string;
  text: string;
}

/** An open or approved proposal on the task's nodes or bound to it. Source: `docs/canon/task-package.md` "Package". */
export interface TaskProposal {
  id: string;
  kind: string;
  status: ProposalStatus;
  target_ids: string[];
  task_id: string | null;
  /** A question's or discrepancy's summary, else the rationale's first line, null for an empty rationale. */
  summary: string | null;
}

/** The owner's note of a `changes --note`, oldest first. Source: `docs/canon/task-package.md` "Package". */
export interface OwnerNote {
  at: string;
  note: string;
}

/** Where the approval froze the spec (ADR-0032). Source: `docs/canon/task-package.md` "Package". */
export interface SnapshotPlace {
  worktree: string;
  root_rel: string;
  branch: string;
  commit: string;
}

/** A node as frozen at approval. Source: `docs/canon/task-package.md` "Package". */
export interface SnapshotNode {
  id: string;
  path: string;
  span_hash: string;
}

/** The spec as the owner approved it. Source: `docs/canon/task-package.md` "Package". */
export interface SpecSnapshot {
  at: string;
  place: SnapshotPlace;
  nodes: SnapshotNode[];
}

/**
 * A node changed since approval, the snapshot's path and hash: `git diff --no-index` hunks from the
 * frozen text, cut at 8 192 B at a line end (`cut`). `diff` null has two meanings: `cut` true, left
 * out past 262 144 B in all (or the package's budget), one note says how many; `cut` false, a diff
 * git cannot make, a note says why. Sources: `docs/canon/task-package.md` "Staleness", "Caps".
 */
export interface SnapshotDiff {
  id: string;
  path: string;
  /** The snapshot's (the old side). */
  span_hash: string;
  diff: string | null;
  cut: boolean;
}

/** The one claim of a task. Source: `docs/canon/task-package.md` "Package". */
export interface TaskClaim {
  at: string;
  role: string;
  worktree: string;
  branch: string;
}

/** A run of the task; an open run has `ended_at` null. Source: `docs/canon/task-package.md` "Package". */
export interface TaskRun {
  run: number;
  role: string;
  started_at: string;
  ended_at: string | null;
  outcome: RunOutcome | null;
  summary: string | null;
  changed_files: string[];
}

/**
 * The bundle an agent gets for the targets, by reference: `bundle_hash` null, with a note
 * `bundle: <why>`, when it cannot be made. Source: `docs/canon/task-package.md` "Package".
 */
export interface TaskBundle {
  node_ids: string[];
  budget: number;
  bundle_hash: string | null;
}

/** A plan group: each release, then the pool (no `release`), then unplaced; other strings kept. Source: `docs/canon/spec-roadmap.md` "Command". */
export type RoadmapGroupKind = "release" | "pool" | "unplaced" | Unlisted;

/**
 * The task's place on the plan, read at the reading root as `bundle_hash` is: the open item it
 * executes (`docs/canon/spec-roadmap.md` "Position"), its group; `release` the item's as written
 * (an unplaced item keeps the name), null when absent or mistyped; `position` 1-based in the
 * release, null outside one (a pool item: `release` and `position` null); `of` the group's open
 * items; `priority` the item's, project vocabulary; `waits_on` its open predecessors. Advisory: an
 * unfinished predecessor never holds the task (ADR-0012). The package's `roadmap` is null with no
 * `[roadmap]`, a bad table (a note `roadmap: <why>`), or no open item executed.
 * Source: `docs/canon/task-package.md` "Package".
 */
export interface TaskRoadmap {
  item: string;
  group: RoadmapGroupKind;
  release: string | null;
  position: number | null;
  of: number;
  priority: string | null;
  waits_on: string[];
}

/**
 * `spec task show T --json`, uncut: every key present, absent null, lists []. `stale`, `snapshot_diff`,
 * `open_proposals` and `assumptions` are computed by the core per read; `schema_version` as
 * "Versioning" says. Sources: `docs/canon/task-package.md` "Package", "Versioning".
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
  /**
   * The code rows of the targets, the affected nodes and their sections, in row order, at most 64
   * (a note says how many more); `[]` without `[code]` (`docs/features/bundle-bindings.md` "Task package").
   */
  bindings: BindingRow[];
  /**
   * The `[[tested]]` entries of `spec verify --task T`'s set (targets, criteria refs, affected nodes,
   * their sections), by node bytes, never cut: null when not judged (no `[code]`, a cause, the blocks
   * not judgeable), [] with no block. Source: `docs/canon/spec-lock-tested.md` "Reads".
   */
  tested: TestedRow[] | null;
  spec_snapshot: SpecSnapshot | null;
  snapshot_diff: SnapshotDiff[] | null;
  claim: TaskClaim | null;
  runs: TaskRun[];
  bundle: TaskBundle | null;
  roadmap: TaskRoadmap | null;
  author: Author | null;
  created_at: string;
  updated_at: string;
  notes: string[];
}

// The plan (docs/features/roadmap-screen.md): `spec roadmap --json` as the daemon's browser view
// sends it, uncut (`omitted` 0), every key present, in `docs/canon/spec-roadmap.md` "Command"'s
// order. Release and priority names, the unit and an item's `status:` are project vocabulary
// (ADR-0008), plain strings shown as written and never compared; a task's state is the queue's.

/**
 * `spec roadmap --json`: the whole plan's line (`counts`, `current`, `cut`, `total`) and its groups;
 * `reason` set (the daemon's 404, the exit-1 document) when `release` names no release, `groups`
 * then empty. `notes` are the plan's; the join's go to stderr only, never here.
 * Source: `docs/canon/spec-roadmap.md` "Command".
 */
export interface RoadmapDocument {
  release: string | null;
  pool: boolean;
  reason: string | null;
  prefix: string;
  dir: string;
  unit: string | null;
  current: string | null;
  counts: RoadmapCounts;
  cut: RoadmapCut | null;
  total: RoadmapSum;
  groups: RoadmapGroup[];
  pool_tasks: RoadmapPoolTask[];
  notes: string[];
  truncated: boolean;
  omitted: number;
}

/** The whole plan's open and done items. Source: `docs/canon/spec-roadmap.md` "Command". */
export interface RoadmapCounts {
  open: number;
  done: number;
}

/** The MVP cut's sum: the releases from the first through `mvp_cut`. Source: `docs/canon/spec-roadmap.md` "Command". */
export interface RoadmapCut {
  release: string;
  min: number;
  max: number;
  unestimated: number;
}

/** A sum over open items: the estimated bounds added, the others counted. Source: `docs/canon/spec-roadmap.md` "Command". */
export interface RoadmapSum {
  min: number;
  max: number;
  unestimated: number;
}

/**
 * A group of the plan, in served order: each release, then the pool, then unplaced; `name` null
 * but for a release; its open items listed, its done ones counted. Source: `docs/canon/spec-roadmap.md` "Command".
 */
export interface RoadmapGroup {
  group: RoadmapGroupKind;
  name: string | null;
  current: boolean;
  mvp_cut: boolean;
  open: number;
  done: number;
  sum: RoadmapSum;
  items: RoadmapItem[];
}

/**
 * An open item: `position` 1-based in a release, null elsewhere; `status`, `release` (null when
 * absent or mistyped) and `priority` as written; `waits_on` its open predecessors, never a hold on
 * it (ADR-0012); `tasks` the open tasks executing it. Source: `docs/canon/spec-roadmap.md` "Command".
 */
export interface RoadmapItem {
  position: number | null;
  id: string;
  title: string | null;
  path: string;
  status: string | null;
  release: string | null;
  priority: string | null;
  estimate: RoadmapEstimate | null;
  waits_on: string[];
  cycle: boolean;
  tasks: RoadmapItemTask[];
}

/** An item's estimate in the plan's unit; absent or unreadable: the item's `estimate` null. Source: `docs/canon/spec-roadmap.md` "Command". */
export interface RoadmapEstimate {
  min: number;
  max: number;
}

/** An open task executing the item, by number, its state verbatim. Source: `docs/canon/spec-roadmap.md` "Command". */
export interface RoadmapItemTask {
  id: string;
  status: TaskStatus;
}

/** An open task executing no open item: a pool task, its stored targets. Source: `docs/canon/spec-roadmap.md` "Command". */
export interface RoadmapPoolTask {
  id: string;
  status: TaskStatus;
  title: string | null;
  targets: string[];
}
