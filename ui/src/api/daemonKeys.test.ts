import { describe, expect, it } from "vitest";
import source from "./provisional.ts?raw";
import type {
  AssumedNode,
  AssumedProposal,
  AssumesCounts,
  AssumesRow,
  Author,
  BundleView,
  CheckCause,
  CheckCounts,
  CheckFinding,
  CheckReport,
  DebtEntry,
  DriftEvidence,
  FollowedType,
  GraphEdge,
  GraphNode,
  GraphView,
  HealthAssumes,
  HealthCode,
  HealthReport,
  HealthUnbound,
  InboxEntry,
  Interpretation,
  NodeView,
  OwnerNote,
  Project,
  Proposal,
  QueueEvent,
  RevisionChange,
  RoundAnswerEntry,
  RoundAnswersOutcome,
  RoundItem,
  RoundSheet,
  RoundTarget,
  RoundTask,
  SearchHit,
  ShownLink,
  SnapshotDiff,
  SnapshotNode,
  SnapshotPlace,
  SpecSnapshot,
  StagedApprove,
  StagedReject,
  TaskAssumption,
  TaskBundle,
  TaskClaim,
  TaskCriterion,
  TaskList,
  TaskListEntry,
  TaskNotFound,
  TaskPackage,
  TaskProposal,
  TaskRun,
  TaskTarget,
  UnappliedAmendment,
  UnboundCounts,
  UnboundMarker,
  UnboundRow,
} from "./types";

// AC-09 of docs/features/daemon-read.md and of docs/features/ui-live.md, AC-07 of
// docs/features/ui-live-tasks.md, the review document's `task_id` (`docs/canon/tasks.md`
// "Task-bound proposals"), AC-14 of docs/features/decision-staging.md (the review document's
// `staged`, `staged_at`, an inbox entry's `staged_at`, the stage's two shapes): the provisional
// types of the daemon's documents. Each record names exactly its type's keys (`satisfies` fails
// `pnpm build` otherwise) and equals the list the cited headings write; the keys the daemon serves
// (the record less the base-only keys of a plain check, NEVER_SERVED) equal
// `fixtures/daemon-keys.json`, which a Rust test regenerates from the daemon on fixture A:
// daemon-read's six names, ui-live's nine, ui-live-tasks' seventeen, decision-staging's two, and
// ui-round's six (T-04 of docs/features/ui-round.md: the sheet, an item, a target, a task, the
// answers document, an entry) and kind-interpretation's one (AC-U1, AC-T4 of
// docs/features/kind-interpretation.md: the review document's `interpretation`, the
// `Interpretation` set), and health-left's three (AC-U2, AC-T2 of docs/features/health-left.md: the
// health document, an amendment, and the link `spec show --links` already sent), and
// rev-discipline's one (UI-4, TE-3 of docs/features/rev-discipline.md: `RevisionRow`, a `revisions`
// entry, the provisional `RevisionChange`), with the review document's `revisions` after `conflict`
// (47 keys) and the stage's `editorial` after `canon` (7 keys), and drift-queue's one (UI-1, TE-11 of
// docs/features/drift-queue.md: `DriftEvidence`, a drift item's evidence, the forty-sixth set), with
// the review document's `drift` after `revisions` (48 keys), both written into the file by its TE-11,
// and health-code's ten (UI-1, AC-9 of docs/features/health-code.md: the health document's `code`,
// `assumes`, `unbound` after `notes`, rows 4-5 and their parts, the fifty-sixth and last
// `UnboundMarker`). A key the types carry ahead of the file is listed in AHEAD_OF_FIXTURE, empty
// today; a listed key the file already names fails. While that file is absent the comparison is
// skipped, saying so; once it exists it names all fifty-six, in the slices' order.

const KEYS = {
  // docs/features/daemon-read.md "Data": `[{slug, name, root, branch}]`.
  Project: { slug: true, name: true, root: true, branch: true } satisfies Record<keyof Project, true>,
  // Same heading, "Inbox entry" (D1, with decision-apply).
  InboxEntry: {
    id: true,
    kind: true,
    status: true,
    target_id: true,
    target_ids: true,
    branch: true,
    created_at: true,
    rationale: true,
    severity: true,
    summary: true,
    record_id: true,
    staged_at: true,
  } satisfies Record<keyof InboxEntry, true>,
  // docs/canon/proposal-queue.md "Commands" to `updated_at`, docs/canon/agent-intake.md "Review
  // document" the eleven after it, docs/features/decision-apply.md "Data" the five after `linked`,
  // docs/canon/tasks.md "Task-bound proposals" (Review document) `task_id` after `choice`,
  // docs/canon/decision-staging.md "Documents" `staged`, `staged_at` between `task_id` and `notes`,
  // docs/features/kind-interpretation.md "Data" `interpretation` after `staged_at`,
  // docs/features/rev-discipline.md "Review document, stage, queue" `revisions` after `conflict`,
  // docs/features/drift-queue.md "Review, inbox, round, stage" `drift` after `revisions`.
  Proposal: {
    id: true,
    project: true,
    kind: true,
    status: true,
    target_id: true,
    target_path: true,
    worktree: true,
    branch: true,
    base_commit: true,
    base_hash: true,
    base_text: true,
    new_text: true,
    patch_hash: true,
    rationale: true,
    author: true,
    diagnostics: true,
    diff: true,
    preview: true,
    conflict: true,
    revisions: true,
    drift: true,
    decided_by: true,
    decided_at: true,
    decision_note: true,
    applied_commit: true,
    created_at: true,
    updated_at: true,
    target_ids: true,
    severity: true,
    gap_type: true,
    summary: true,
    working_answer: true,
    price_of_other: true,
    evidence: true,
    options: true,
    recommendation: true,
    distinct_from: true,
    linked: true,
    record_id: true,
    record_path: true,
    record_title: true,
    record_text: true,
    choice: true,
    task_id: true,
    staged: true,
    staged_at: true,
    interpretation: true,
    notes: true,
  } satisfies Record<keyof Proposal, true>,
  // docs/features/kind-interpretation.md "Data": the row's JSON, keys in its order.
  Interpretation: { decision: true, how: true, why: true } satisfies Record<keyof Interpretation, true>,
  // docs/canon/decision-staging.md "The stage": its two shapes, keys in its order; the approve's
  // `editorial` after `canon`, docs/features/rev-discipline.md "Review document, stage, queue".
  StagedApprove: {
    decision: true,
    option: true,
    answer: true,
    canon: true,
    editorial: true,
    note: true,
    span_hash: true,
  } satisfies Record<keyof StagedApprove, true>,
  StagedReject: { decision: true, reason: true } satisfies Record<keyof StagedReject, true>,
  // crates/specengine-cli/README.md "Output and the cap": `spec show --json`.
  NodeView: { ref: true, reason: true, notes: true, nodes: true } satisfies Record<keyof NodeView, true>,
  // Same heading: a hit.
  SearchHit: {
    id: true,
    kind: true,
    title: true,
    path: true,
    line: true,
    ord: true,
    archived: true,
    snippet: true,
  } satisfies Record<keyof SearchHit, true>,
  // docs/canon/spec-cli-bundle.md "Output".
  BundleView: {
    refs: true,
    reason: true,
    notes: true,
    task: true,
    budget: true,
    tokens: true,
    chars: true,
    bytes: true,
    bundle_hash: true,
    body: true,
    layers: true,
    tail: true,
    more: true,
  } satisfies Record<keyof BundleView, true>,
  // docs/canon/spec-cli-graph.md "spec graph": the JSON, a node, an edge, `types`' entries.
  GraphView: {
    ref: true,
    reason: true,
    impact: true,
    types: true,
    depth: true,
    archive: true,
    notes: true,
    left_out: true,
    truncated: true,
    nodes: true,
    edges: true,
  } satisfies Record<keyof GraphView, true>,
  GraphNode: { id: true, kind: true, title: true, path: true, line: true, distance: true, archived: true } satisfies Record<keyof GraphNode, true>,
  GraphEdge: {
    src: true,
    type: true,
    dst: true,
    written: true,
    path: true,
    line: true,
    state: true,
    reason: true,
  } satisfies Record<keyof GraphEdge, true>,
  FollowedType: { type: true, direction: true } satisfies Record<keyof FollowedType, true>,
  // docs/canon/spec-check.md "Findings, debt, verdict": the JSON, its counts, a finding.
  CheckReport: {
    mode: true,
    verdict: true,
    counts: true,
    findings: true,
    stale: true,
    new_debt: true,
    cannot_check: true,
  } satisfies Record<keyof CheckReport, true>,
  CheckCounts: {
    documents: true,
    errors: true,
    warnings: true,
    debt: true,
    expired: true,
    stale: true,
    introduced: true,
    new_debt: true,
    worst_w_bytes: true,
  } satisfies Record<keyof CheckCounts, true>,
  CheckFinding: {
    code: true,
    severity: true,
    path: true,
    line: true,
    subject: true,
    message: true,
    fix: true,
    debt: true,
    introduced: true,
  } satisfies Record<keyof CheckFinding, true>,
  // docs/features/ui-live.md "Data": a `stale` entry, a `cannot_check` cause.
  DebtEntry: { code: true, path: true, subject: true, reason: true, expires: true, line: true } satisfies Record<keyof DebtEntry, true>,
  CheckCause: { path: true, message: true } satisfies Record<keyof CheckCause, true>,
  // docs/canon/tasks.md "Commands": `spec task list --json`, a row, `spec task show`'s exit-1 document.
  TaskList: { tasks: true, notes: true } satisfies Record<keyof TaskList, true>,
  TaskListEntry: { id: true, status: true, title: true, targets: true, stale: true, updated_at: true } satisfies Record<keyof TaskListEntry, true>,
  TaskNotFound: { id: true, reason: true } satisfies Record<keyof TaskNotFound, true>,
  // docs/canon/task-package.md "Package": the package and each of its parts, `author` among them.
  TaskPackage: {
    schema_version: true,
    id: true,
    project: true,
    status: true,
    title: true,
    goal: true,
    profile: true,
    stale: true,
    targets: true,
    criteria: true,
    affected_nodes: true,
    plan: true,
    assumptions: true,
    open_proposals: true,
    owner_notes: true,
    bindings: true,
    spec_snapshot: true,
    snapshot_diff: true,
    claim: true,
    runs: true,
    bundle: true,
    author: true,
    created_at: true,
    updated_at: true,
    notes: true,
  } satisfies Record<keyof TaskPackage, true>,
  TaskTarget: { id: true, path: true, kind: true, title: true } satisfies Record<keyof TaskTarget, true>,
  TaskCriterion: { ref: true, text: true } satisfies Record<keyof TaskCriterion, true>,
  TaskAssumption: { proposal: true, text: true } satisfies Record<keyof TaskAssumption, true>,
  TaskProposal: { id: true, kind: true, status: true, target_ids: true, task_id: true, summary: true } satisfies Record<keyof TaskProposal, true>,
  OwnerNote: { at: true, note: true } satisfies Record<keyof OwnerNote, true>,
  SpecSnapshot: { at: true, place: true, nodes: true } satisfies Record<keyof SpecSnapshot, true>,
  SnapshotPlace: { worktree: true, root_rel: true, branch: true, commit: true } satisfies Record<keyof SnapshotPlace, true>,
  SnapshotNode: { id: true, path: true, span_hash: true } satisfies Record<keyof SnapshotNode, true>,
  SnapshotDiff: { id: true, path: true, span_hash: true, diff: true, cut: true } satisfies Record<keyof SnapshotDiff, true>,
  TaskClaim: { at: true, role: true, worktree: true, branch: true } satisfies Record<keyof TaskClaim, true>,
  TaskRun: {
    run: true,
    role: true,
    started_at: true,
    ended_at: true,
    outcome: true,
    summary: true,
    changed_files: true,
  } satisfies Record<keyof TaskRun, true>,
  TaskBundle: { node_ids: true, budget: true, bundle_hash: true } satisfies Record<keyof TaskBundle, true>,
  Author: { type: true, role: true, model: true, run: true } satisfies Record<keyof Author, true>,
  // docs/canon/round.md "JSON": the sheet, an item's 19 keys, a target, a task.
  RoundSheet: { project: true, items: true, more: true, minutes: true, notes: true } satisfies Record<keyof RoundSheet, true>,
  RoundItem: {
    id: true,
    kind: true,
    severity: true,
    gap_type: true,
    summary: true,
    working_answer: true,
    price_of_other: true,
    options: true,
    recommendation: true,
    evidence: true,
    targets: true,
    tasks: true,
    worktree: true,
    branch: true,
    author: true,
    staged: true,
    staged_at: true,
    created_at: true,
    updated_at: true,
  } satisfies Record<keyof RoundItem, true>,
  RoundTarget: { id: true, path: true, title: true } satisfies Record<keyof RoundTarget, true>,
  RoundTask: { id: true, status: true, title: true } satisfies Record<keyof RoundTask, true>,
  // docs/features/ui-round.md "Data": the answers document and an entry of it.
  RoundAnswersOutcome: { project: true, answers: true, notes: true } satisfies Record<keyof RoundAnswersOutcome, true>,
  RoundAnswerEntry: {
    id: true,
    outcome: true,
    stage: true,
    staged_at: true,
    replaced: true,
    reason: true,
  } satisfies Record<keyof RoundAnswerEntry, true>,
  // docs/features/health-left.md "Data": the document, an amendment; a link as
  // docs/canon/spec-cli-graph.md "spec show --links" lists one. docs/features/health-code.md "Data":
  // `code`, `assumes`, `unbound` after `notes`.
  HealthReport: {
    documents: true,
    amendments: true,
    notes: true,
    code: true,
    assumes: true,
    unbound: true,
  } satisfies Record<keyof HealthReport, true>,
  UnappliedAmendment: { path: true, id: true, title: true, class: true, status: true, links: true } satisfies Record<keyof UnappliedAmendment, true>,
  ShownLink: {
    type: true,
    origin: true,
    at: true,
    name: true,
    written: true,
    path: true,
    line: true,
    state: true,
    reason: true,
  } satisfies Record<keyof ShownLink, true>,
  // docs/features/rev-discipline.md "Review document, stage, queue": a `revisions` entry, keys in its
  // order. The file names the set `RevisionRow`; the provisional type is `RevisionChange`.
  RevisionRow: { id: true, rev: true, to: true, lock_entries: true, refused: true } satisfies Record<keyof RevisionChange, true>,
  // docs/features/drift-queue.md "Review, inbox, round, stage": the review's `drift`, `{"hash","rows","more"}`.
  DriftEvidence: { hash: true, rows: true, more: true } satisfies Record<keyof DriftEvidence, true>,
  // docs/features/health-code.md "Data": the code count, rows 4-5, and their parts, keys in its order.
  HealthCode: { files: true, rs: true, ron: true, skipped: true } satisfies Record<keyof HealthCode, true>,
  HealthAssumes: { state: true, causes: true, counts: true, rows: true } satisfies Record<keyof HealthAssumes, true>,
  HealthUnbound: { state: true, causes: true, counts: true, rows: true } satisfies Record<keyof HealthUnbound, true>,
  AssumesCounts: {
    markers: true,
    rows: true,
    pending: true,
    as_assumed: true,
    otherwise: true,
    applied: true,
    rejected: true,
    unknown: true,
    archived: true,
    in_question: true,
  } satisfies Record<keyof AssumesCounts, true>,
  AssumesRow: {
    id: true,
    state: true,
    path: true,
    line: true,
    column: true,
    relation: true,
    proposal: true,
    node: true,
  } satisfies Record<keyof AssumesRow, true>,
  AssumedProposal: {
    kind: true,
    status: true,
    choice: true,
    record_id: true,
    record_path: true,
    reason: true,
  } satisfies Record<keyof AssumedProposal, true>,
  AssumedNode: { id: true, path: true, line: true, status: true, successor: true, proposals: true } satisfies Record<keyof AssumedNode, true>,
  UnboundCounts: { rules: true, selected: true, bound: true, unbound: true } satisfies Record<keyof UnboundCounts, true>,
  UnboundRow: {
    id: true,
    kind: true,
    title: true,
    path: true,
    line: true,
    rules: true,
    mentions: true,
    markers: true,
  } satisfies Record<keyof UnboundRow, true>,
  UnboundMarker: {
    path: true,
    line: true,
    column: true,
    relation: true,
    id_state: true,
    binding: true,
    reason: true,
  } satisfies Record<keyof UnboundMarker, true>,
};

type Named = keyof typeof KEYS;

/** The key lists as the cited headings write them, copied verbatim (the review document's joined). */
const CITED: Record<Named, string> = {
  Project: "slug, name, root, branch",
  InboxEntry: "id, kind, status, target_id, target_ids, branch, created_at, rationale, severity, summary, record_id, staged_at",
  Proposal:
    "id, project, kind, status, target_id, target_path, worktree, branch, base_commit, base_hash, base_text, new_text, patch_hash, rationale, author, diagnostics, diff, preview, conflict, " +
    "revisions, " +
    "drift, " +
    "decided_by, decided_at, decision_note, applied_commit, created_at, updated_at, " +
    "target_ids, severity, gap_type, summary, working_answer, price_of_other, evidence, options, recommendation, distinct_from, linked, " +
    "record_id, record_path, record_title, record_text, choice, " +
    "task_id, " +
    "staged, staged_at, " +
    "interpretation, " +
    "notes",
  Interpretation: "decision, how, why",
  StagedApprove: "decision, option, answer, canon, editorial, note, span_hash",
  StagedReject: "decision, reason",
  NodeView: "ref, reason, notes, nodes",
  SearchHit: "id, kind, title, path, line, ord, archived, snippet",
  BundleView: "refs, reason, notes, task, budget, tokens, chars, bytes, bundle_hash, body, layers, tail, more",
  GraphView: "ref, reason, impact, types, depth, archive, notes, left_out, truncated, nodes, edges",
  GraphNode: "id, kind, title, path, line, distance, archived",
  GraphEdge: "src, type, dst, written, path, line, state, reason",
  FollowedType: "type, direction",
  CheckReport: "mode, verdict, counts, findings, stale, new_debt?, cannot_check",
  CheckCounts: "documents, errors, warnings, debt, expired, stale, introduced?, new_debt?, worst_w_bytes",
  CheckFinding: "code, severity, path, line, subject, message, fix?, debt?, introduced?",
  DebtEntry: "code, path, subject, reason, expires, line",
  CheckCause: "path, message",
  TaskList: "tasks, notes",
  TaskListEntry: "id, status, title, targets, stale, updated_at",
  TaskNotFound: "id, reason",
  TaskPackage:
    "schema_version, id, project, status, title, goal, profile, stale, targets, criteria, affected_nodes, plan, assumptions, open_proposals, owner_notes, bindings, spec_snapshot, snapshot_diff, claim, runs, bundle, author, created_at, updated_at, notes",
  TaskTarget: "id, path, kind, title",
  TaskCriterion: "ref, text",
  TaskAssumption: "proposal, text",
  TaskProposal: "id, kind, status, target_ids, task_id, summary",
  OwnerNote: "at, note",
  SpecSnapshot: "at, place, nodes",
  SnapshotPlace: "worktree, root_rel, branch, commit",
  SnapshotNode: "id, path, span_hash",
  SnapshotDiff: "id, path, span_hash, diff, cut",
  TaskClaim: "at, role, worktree, branch",
  TaskRun: "run, role, started_at, ended_at, outcome, summary, changed_files",
  TaskBundle: "node_ids, budget, bundle_hash",
  Author: "type, role, model, run",
  RoundSheet: "project, items, more, minutes, notes",
  RoundItem:
    "id, kind, severity, gap_type, summary, working_answer, price_of_other, options, recommendation, evidence, targets, tasks, worktree, branch, author, staged, staged_at, created_at, updated_at",
  RoundTarget: "id, path, title",
  RoundTask: "id, status, title",
  RoundAnswersOutcome: "project, answers, notes",
  RoundAnswerEntry: "id, outcome, stage, staged_at, replaced, reason",
  HealthReport: "documents, amendments, notes, code, assumes, unbound",
  UnappliedAmendment: "path, id, title, class, status, links",
  ShownLink: "type, origin, at, name, written, path, line, state, reason",
  RevisionRow: "id, rev, to, lock_entries, refused",
  DriftEvidence: "hash, rows, more",
  HealthCode: "files, rs, ron, skipped",
  HealthAssumes: "state, causes, counts, rows",
  HealthUnbound: "state, causes, counts, rows",
  AssumesCounts: "markers, rows, pending, as_assumed, otherwise, applied, rejected, unknown, archived, in_question",
  AssumesRow: "id, state, path, line, column, relation, proposal, node",
  AssumedProposal: "kind, status, choice, record_id, record_path, reason",
  AssumedNode: "id, path, line, status, successor, proposals",
  UnboundCounts: "rules, selected, bound, unbound",
  UnboundRow: "id, kind, title, path, line, rules, mentions, markers",
  UnboundMarker: "path, line, column, relation, id_state, binding, reason",
};

const NAMES = Object.keys(KEYS) as Named[];

/** daemon-read's six (docs/features/daemon-read.md AC-09). */
const DAEMON_READ: readonly Named[] = ["Project", "InboxEntry", "Proposal", "NodeView", "SearchHit", "BundleView"];

/** ui-live's nine (docs/features/ui-live.md AC-09). */
const UI_LIVE: readonly Named[] = ["GraphView", "GraphNode", "GraphEdge", "FollowedType", "CheckReport", "CheckCounts", "CheckFinding", "DebtEntry", "CheckCause"];

/** ui-live-tasks' seventeen (docs/features/ui-live-tasks.md "Key sets", AC-07). */
const UI_LIVE_TASKS: readonly Named[] = [
  "TaskList",
  "TaskListEntry",
  "TaskNotFound",
  "TaskPackage",
  "TaskTarget",
  "TaskCriterion",
  "TaskAssumption",
  "TaskProposal",
  "OwnerNote",
  "SpecSnapshot",
  "SnapshotPlace",
  "SnapshotNode",
  "SnapshotDiff",
  "TaskClaim",
  "TaskRun",
  "TaskBundle",
  "Author",
];

/** decision-staging's two: the stage's shapes (docs/features/decision-staging.md "Data", "Documents"). */
const DECISION_STAGING: readonly Named[] = ["StagedApprove", "StagedReject"];

/** ui-round's six: the sheet, an item, a target, a task, the answers document, an entry (docs/features/ui-round.md T-04). */
const UI_ROUND: readonly Named[] = ["RoundSheet", "RoundItem", "RoundTarget", "RoundTask", "RoundAnswersOutcome", "RoundAnswerEntry"];

/** kind-interpretation's one: an interpretation's decision and texts (docs/features/kind-interpretation.md "Data", AC-T4). */
const KIND_INTERPRETATION: readonly Named[] = ["Interpretation"];

/** health-left's three: the health document, an amendment, a shown link (docs/features/health-left.md "Data", AC-T2). */
const HEALTH_LEFT: readonly Named[] = ["HealthReport", "UnappliedAmendment", "ShownLink"];

/** The thirty-four types the fixture named before ui-round. */
const BEFORE_ROUND: readonly Named[] = [...DAEMON_READ, ...UI_LIVE, ...UI_LIVE_TASKS, ...DECISION_STAGING];

/** The forty-one types the fixture named before health-left. */
const BEFORE_HEALTH_LEFT: readonly Named[] = [...BEFORE_ROUND, ...UI_ROUND, ...KIND_INTERPRETATION];

/** rev-discipline's one: a `revisions` entry (docs/features/rev-discipline.md "Review document, stage, queue", TE-3). */
const REV_DISCIPLINE: readonly Named[] = ["RevisionRow"];

/** The forty-four types the fixture named before rev-discipline. */
const BEFORE_REV_DISCIPLINE: readonly Named[] = [...BEFORE_HEALTH_LEFT, ...HEALTH_LEFT];

/** The forty-five types the fixture named before drift-queue. */
const BEFORE_DRIFT_QUEUE: readonly Named[] = [...BEFORE_REV_DISCIPLINE, ...REV_DISCIPLINE];

/** drift-queue's one: a drift item's evidence (docs/features/drift-queue.md "Review, inbox, round, stage", TE-11). */
const DRIFT_QUEUE: readonly Named[] = ["DriftEvidence"];

/** The forty-six types the fixture named before health-code. */
const BEFORE_HEALTH_CODE: readonly Named[] = [...BEFORE_DRIFT_QUEUE, ...DRIFT_QUEUE];

/**
 * health-code's ten: the code count, rows 4-5, and their parts, in "The document"'s listing order
 * (docs/features/health-code.md "Data", AC-9).
 */
const HEALTH_CODE: readonly Named[] = [
  "HealthCode",
  "HealthAssumes",
  "HealthUnbound",
  "AssumesCounts",
  "AssumesRow",
  "AssumedProposal",
  "AssumedNode",
  "UnboundCounts",
  "UnboundRow",
  "UnboundMarker",
];

/** Every type the fixture names, in the slices' order. */
const ALL: readonly Named[] = [...BEFORE_HEALTH_CODE, ...HEALTH_CODE];

/**
 * The keys only a check against a git base sends: the daemon runs the plain check, so it never
 * serves them (`docs/canon/spec-check.md` "Findings, debt, verdict"; docs/features/ui-live.md "Data").
 */
const NEVER_SERVED: readonly (readonly [Named, string])[] = [
  ["CheckReport", "new_debt"],
  ["CheckCounts", "introduced"],
  ["CheckCounts", "new_debt"],
  ["CheckFinding", "introduced"],
];

/** The keys the daemon sends for a type: its record's, less the never-served ones. */
function served(name: Named): string[] {
  return Object.keys(KEYS[name]).filter((key) => !NEVER_SERVED.some(([type, never]) => type === name && never === key));
}

/**
 * The keys the types carry ahead of `fixtures/daemon-keys.json`: a slice's keys while its daemon
 * side is built alongside and the Rust test has not yet regenerated the file. While the file lacks
 * a key, the comparison leaves it out of the type's keys, and a set all of whose keys it lacks out
 * of the file's types. A listed key the file already names fails ("lists no key ahead of the file
 * that the file names"): the list is emptied in the change that regenerates the file. Empty since
 * drift-queue's TE-11 wrote its review `drift` and `DriftEvidence` set
 * (`docs/features/drift-queue.md` "Review, inbox, round, stage").
 */
const AHEAD_OF_FIXTURE: readonly (readonly [Named, string])[] = [];

function isAhead(name: Named, key: string): boolean {
  return AHEAD_OF_FIXTURE.some(([type, ahead]) => type === name && ahead === key);
}

/** An interface's keys in the order `provisional.ts` declares them (its own lines, two-space indented). */
function declaredKeys(type: string): string[] {
  const start = source.indexOf(`export interface ${type} {`);
  const end = source.indexOf("\n}\n", start);
  if (start < 0 || end < 0) {
    return [];
  }
  return Array.from(source.slice(start, end).matchAll(/^ {2}(\w+)\??:/gm), (match) => match[1] ?? "");
}

describe("the daemon's document types (AC-09)", () => {
  it.each(NAMES)("%s has exactly the cited keys, in order", (name) => {
    // An optional key is cited with its `?` (W-1: omitted when unset, never null).
    expect(Object.keys(KEYS[name]).join(", ")).toBe(CITED[name].replaceAll("?", ""));
  });

  it("counts 4 project keys, 12 inbox-entry keys, 48 review keys (AC-14 of decision-staging, AC-U1 of kind-interpretation, UI-4 of rev-discipline, UI-1 of drift-queue)", () => {
    expect([KEYS.Project, KEYS.InboxEntry, KEYS.Proposal].map((keys) => Object.keys(keys).length)).toEqual([4, 12, 48]);
  });

  it("gives the review document its drift right after revisions, before decided_by, and the DriftEvidence set its 3 keys (UI-1 of drift-queue)", () => {
    const review = Object.keys(KEYS.Proposal);
    expect(review.slice(review.indexOf("revisions"), review.indexOf("revisions") + 3)).toEqual(["revisions", "drift", "decided_by"]);
    expect(DRIFT_QUEUE.map((name) => served(name))).toEqual([["hash", "rows", "more"]]);
    expect(Object.keys(KEYS.InboxEntry)).not.toContain("drift");
    expect(BEFORE_HEALTH_CODE.at(-1)).toBe("DriftEvidence");
    expect(ALL.at(-1)).toBe("UnboundMarker");
  });

  it("declares in provisional.ts the review document's keys and the DriftEvidence set's, in the records' order: drift after revisions (UI-1 of drift-queue)", () => {
    expect(declaredKeys("Proposal")).toEqual(Object.keys(KEYS.Proposal));
    expect(declaredKeys("DriftEvidence")).toEqual(Object.keys(KEYS.DriftEvidence));
    expect(source).toMatch(/^  drift: DriftEvidence \| null;$/m);
    // Untyped rows until Phase 4's drift screen, a count past them.
    expect(source).toMatch(/^export interface DriftEvidence \{\n  hash: string;\n  rows: unknown\[\];\n  more: number;\n\}$/m);
  });

  it('DriftEvidence cites `docs/features/drift-queue.md` "Review, inbox, round, stage"; the review document cites it too', () => {
    for (const type of ["DriftEvidence", "Proposal"]) {
      const at = source.indexOf(`export interface ${type} `);
      expect([type, at > 0]).toEqual([type, true]);
      expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/drift-queue.md` "Review, inbox, round, stage"');
    }
  });

  it("lists ahead of the fixture only keys their types serve, none since drift-queue's TE-11 wrote its keys into the file", () => {
    for (const [name, key] of AHEAD_OF_FIXTURE) {
      expect([name, key, served(name).includes(key)]).toEqual([name, key, true]);
    }
    expect(AHEAD_OF_FIXTURE).toEqual([]);
  });

  it("gives the review document its revisions right after conflict, and the approve stage its editorial right after canon (UI-4 of rev-discipline)", () => {
    const review = Object.keys(KEYS.Proposal);
    expect(review.slice(review.indexOf("preview"), review.indexOf("preview") + 3)).toEqual(["preview", "conflict", "revisions"]);
    expect(Object.keys(KEYS.StagedApprove)).toEqual(["decision", "option", "answer", "canon", "editorial", "note", "span_hash"]);
    expect(Object.keys(KEYS.InboxEntry)).not.toContain("revisions");
    expect(Object.keys(KEYS.StagedReject)).not.toContain("editorial");
  });

  it("counts rev-discipline's one: a revisions entry's 5 keys, in the spec's order (UI-4, TE-3 of rev-discipline)", () => {
    expect(REV_DISCIPLINE.map((name) => served(name))).toEqual([["id", "rev", "to", "lock_entries", "refused"]]);
    expect(BEFORE_DRIFT_QUEUE.at(-1)).toBe("RevisionRow");
  });

  it('RevisionRow is the provisional RevisionChange, which cites `docs/features/rev-discipline.md` "Review document, stage, queue"', () => {
    const at = source.indexOf("export interface RevisionChange ");
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/rev-discipline.md` "Review document, stage, queue"');
  });

  it("gives the review document an interpretation after staged_at, before notes, and its three keys (AC-U1 of kind-interpretation)", () => {
    expect(Object.keys(KEYS.Proposal).slice(-3)).toEqual(["staged_at", "interpretation", "notes"]);
    expect(KIND_INTERPRETATION.map((name) => served(name))).toEqual([["decision", "how", "why"]]);
    expect(Object.keys(KEYS.InboxEntry)).not.toContain("interpretation");
  });

  it("counts the stage's two shapes: 7 keys staged as an approve, 2 as a reject (AC-14 of decision-staging, UI-4 of rev-discipline)", () => {
    expect(DECISION_STAGING.map((name) => served(name).length)).toEqual([7, 2]);
  });

  it("counts the daemon's keys of ui-live's nine: 11, 7, 8, 2; 6, 7, 8, 6, 2 (AC-09 of ui-live)", () => {
    expect(Object.fromEntries(UI_LIVE.map((name) => [name, served(name).length]))).toEqual({
      GraphView: 11,
      GraphNode: 7,
      GraphEdge: 8,
      FollowedType: 2,
      CheckReport: 6,
      CheckCounts: 7,
      CheckFinding: 8,
      DebtEntry: 6,
      CheckCause: 2,
    });
    expect([...ALL].sort()).toEqual([...NAMES].sort());
  });

  it("counts the daemon's keys of ui-live-tasks' seventeen: 2, 6, 2, 25, 4, 2, 2, 6, 2, 3, 4, 3, 5, 4, 7, 3, 4 (AC-07 of ui-live-tasks)", () => {
    expect(UI_LIVE_TASKS.map((name) => served(name).length)).toEqual([2, 6, 2, 25, 4, 2, 2, 6, 2, 3, 4, 3, 5, 4, 7, 3, 4]);
    expect(UI_LIVE_TASKS).toHaveLength(17);
    expect(BEFORE_ROUND).toHaveLength(34);
    expect(BEFORE_HEALTH_LEFT).toHaveLength(41);
    expect(BEFORE_REV_DISCIPLINE).toHaveLength(44);
    expect(BEFORE_DRIFT_QUEUE).toHaveLength(45);
    expect(BEFORE_HEALTH_CODE).toHaveLength(46);
    expect(ALL).toHaveLength(56);
    expect(new Set(ALL).size).toBe(56);
  });

  it("counts health-left's three: 3 document keys (6 since health-code's three follow them), 6 amendment keys, 9 link keys (AC-U2 of health-left)", () => {
    expect(HEALTH_LEFT.map((name) => served(name).length)).toEqual([6, 6, 9]);
    expect(served("HealthReport").slice(0, 3)).toEqual(["documents", "amendments", "notes"]);
    expect(served("UnappliedAmendment")).toEqual(["path", "id", "title", "class", "status", "links"]);
  });

  it.each(["HealthReport", "UnappliedAmendment"])('%s cites `docs/features/health-left.md` "Data" (AC-U2 of health-left)', (type) => {
    const at = source.indexOf(`export interface ${type} `);
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/health-left.md` "Data"');
  });

  it("gives the health document code, assumes, unbound after notes, and counts health-code's ten: 4, 4, 4, 10, 8, 6, 6, 4, 8, 7 (UI-1 of health-code)", () => {
    expect(served("HealthReport")).toEqual(["documents", "amendments", "notes", "code", "assumes", "unbound"]);
    expect(HEALTH_CODE.map((name) => served(name).length)).toEqual([4, 4, 4, 10, 8, 6, 6, 4, 8, 7]);
    expect(served("HealthAssumes")).toEqual(served("HealthUnbound"));
  });

  it.each<Named>(["HealthReport", ...HEALTH_CODE])('%s cites `docs/features/health-code.md` "Data" and declares its keys in the record order (UI-1 of health-code)', (type) => {
    const at = source.indexOf(`export interface ${type} `);
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/health-code.md` "Data"');
    expect(declaredKeys(type)).toEqual(Object.keys(KEYS[type]));
  });

  it('ShownLink cites `docs/canon/spec-cli-graph.md` "spec show --links"', () => {
    const at = source.indexOf("export interface ShownLink ");
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/canon/spec-cli-graph.md` "spec show --links"');
  });

  it("counts ui-round's six: 5 sheet keys, 19 item keys, 3, 3; 3 answers-document keys, 6 entry keys (T-04 of ui-round)", () => {
    expect(UI_ROUND.map((name) => served(name).length)).toEqual([5, 19, 3, 3, 3, 6]);
    expect(Object.keys(KEYS.RoundItem).slice(-4)).toEqual(["staged", "staged_at", "created_at", "updated_at"]);
  });

  it.each(["RoundSheet", "RoundItem", "RoundTarget", "RoundTask"])('%s cites `docs/canon/round.md` "JSON"', (type) => {
    const at = source.indexOf(`export interface ${type} `);
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/canon/round.md` "JSON"');
  });

  it.each(["RoundAnswersOutcome", "RoundAnswerEntry", "RoundAnswerBody", "RoundAnswersBody"])('%s cites `docs/features/ui-round.md` "Data"', (type) => {
    const at = source.indexOf(`export interface ${type} `);
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/ui-round.md` "Data"');
  });

  it("lists each base-only key once, a key of its type the citation marks optional", () => {
    expect(NEVER_SERVED.map(([name, key]) => `${name}.${key}`)).toEqual([
      "CheckReport.new_debt",
      "CheckCounts.introduced",
      "CheckCounts.new_debt",
      "CheckFinding.introduced",
    ]);
    for (const [name, key] of NEVER_SERVED) {
      expect([name, key, CITED[name].split(", ").includes(`${key}?`)]).toEqual([name, key, true]);
    }
  });

  it("names no task in an inbox entry; the review document's task_id sits after choice, then the stage's two, the interpretation, notes", () => {
    expect(Object.keys(KEYS.InboxEntry)).not.toContain("task_id");
    expect(Object.keys(KEYS.Proposal).slice(-6)).toEqual(["choice", "task_id", "staged", "staged_at", "interpretation", "notes"]);
  });

  it("gives an inbox entry the stage's time alone, after record_id: the stage itself is the review document's", () => {
    expect(Object.keys(KEYS.InboxEntry).slice(-2)).toEqual(["record_id", "staged_at"]);
    expect(Object.keys(KEYS.InboxEntry)).not.toContain("staged");
  });

  it("types a queue event as the stream sends it: {seq, type, payload}", () => {
    const event = { seq: 12, type: "proposal.created", payload: { id: "PR-0005" } } satisfies QueueEvent;
    expect(Object.keys(event)).toEqual(["seq", "type", "payload"]);
  });

  it.each(["Proposal", "Interpretation"])('%s cites `docs/features/kind-interpretation.md` "Data"', (type) => {
    const at = source.indexOf(`export interface ${type} `);
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/kind-interpretation.md` "Data"');
  });

  it.each(["Project", "InboxEntry", "Proposal", "QueueEvent"])("%s cites docs/features/daemon-read.md \"Data\"", (type) => {
    const at = source.indexOf(`export interface ${type} `);
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/daemon-read.md` "Data"');
  });

  it.each([
    ...["GraphView", "GraphNode", "GraphEdge", "FollowedType"].map((type) => [type, '`docs/canon/spec-cli-graph.md` "spec graph"'] as const),
    ...["CheckReport", "CheckCounts", "CheckFinding", "DebtEntry", "CheckCause"].map(
      (type) => [type, '`docs/canon/spec-check.md` "Findings, debt, verdict"'] as const,
    ),
  ])("%s cites %s", (type, citation) => {
    const at = Math.max(source.indexOf(`export interface ${type} `), source.indexOf(`export type ${type} =`));
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain(citation);
  });
});

/** `fixtures/daemon-keys.json` as parsed JSON, when a Rust test has written it (absent: undefined). */
const written = Object.values(import.meta.glob<unknown>("../../../fixtures/daemon-keys.json", { import: "default", eager: true }))[0];

const ABSENT =
  "fixtures/daemon-keys.json is absent: the daemon's key-set test (crates/specengine-http/tests) writes it; " +
  "the comparison with the daemon is skipped until then";

/** The file's lists by type name; a malformed file fails here, naming what is wrong. */
function keySets(parsed: unknown): Record<string, string[]> {
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new Error("fixtures/daemon-keys.json is not a JSON object of type name to key list");
  }
  const sets: Record<string, string[]> = {};
  for (const [name, keys] of Object.entries(parsed)) {
    if (!Array.isArray(keys) || !keys.every((key): key is string => typeof key === "string")) {
      throw new Error(`fixtures/daemon-keys.json: ${name} is not a list of key names`);
    }
    sets[name] = keys;
  }
  return sets;
}

describe.skipIf(written === undefined)(written === undefined ? ABSENT : "the key sets equal fixtures/daemon-keys.json (AC-09)", () => {
  const sets = written === undefined ? {} : keySets(written);

  /** The type's keys the file is compared with: the served ones, less a key ahead of the file it does not name yet. */
  function compared(name: Named): string[] {
    const file = sets[name] ?? [];
    return served(name).filter((key) => file.includes(key) || !isAhead(name, key));
  }

  /** The types the file is compared with: all, less a set ahead of the file (every key ahead) it does not name yet. */
  const expected = ALL.filter((name) => Object.hasOwn(sets, name) || !served(name).every((key) => isAhead(name, key)));

  it("names exactly the fifty-six types: daemon-read's six, ui-live's nine, ui-live-tasks' seventeen, decision-staging's two, ui-round's six, kind-interpretation's one, health-left's three, rev-discipline's one, drift-queue's one, health-code's ten", () => {
    expect(Object.keys(sets).sort()).toEqual([...expected].sort());
    // None is ahead of the file today: it names all fifty-six, in the slices' order.
    expect(expected).toEqual(ALL);
    expect(Object.keys(sets)).toEqual([...ALL]);
    expect(Object.keys(sets)).toHaveLength(56);
  });

  it("lists no key ahead of the file that the file names: the list is emptied when the file catches up", () => {
    expect(AHEAD_OF_FIXTURE.filter(([name, key]) => (sets[name] ?? []).includes(key)).map(([name, key]) => `${name}.${key}`)).toEqual([]);
  });

  it.each(NAMES)("%s: the daemon's keys are the type's", (name) => {
    expect([...(sets[name] ?? [])].sort()).toEqual(compared(name).sort());
  });

  it("names rev-discipline's keys where the types put them: revisions after conflict, editorial after canon, a revisions entry's five in order, the forty-fifth set, before drift-queue's (UI-4, TE-3)", () => {
    const review = sets.Proposal ?? [];
    expect(review.slice(review.indexOf("conflict") - 1, review.indexOf("conflict") + 2)).toEqual(["preview", "conflict", "revisions"]);
    expect(sets.StagedApprove).toEqual(served("StagedApprove"));
    expect(sets.RevisionRow).toEqual(served("RevisionRow"));
    expect(Object.keys(sets).indexOf("RevisionRow")).toBe(BEFORE_REV_DISCIPLINE.length);
    expect(Object.keys(sets).indexOf("RevisionRow")).toBe(44);
    expect(Object.keys(sets).slice(0, BEFORE_HEALTH_CODE.length).at(-2)).toBe("RevisionRow");
  });

  it("names drift-queue's keys where the types put them: the review's 48, drift after revisions, before decided_by; DriftEvidence's three in order, the forty-sixth set, before health-code's (UI-1, TE-11)", () => {
    const review = sets.Proposal ?? [];
    expect(review).toEqual(served("Proposal"));
    expect(review).toHaveLength(48);
    expect(review.slice(review.indexOf("revisions"), review.indexOf("revisions") + 3)).toEqual(["revisions", "drift", "decided_by"]);
    expect(sets.DriftEvidence).toEqual(["hash", "rows", "more"]);
    expect(Object.keys(sets).indexOf("DriftEvidence")).toBe(BEFORE_DRIFT_QUEUE.length);
    expect(Object.keys(sets).indexOf("DriftEvidence")).toBe(45);
    expect(Object.keys(sets).at(-1)).toBe("UnboundMarker");
    expect(sets.InboxEntry).not.toContain("drift");
  });

  it("names health-left's keys in the document's order: the health document's three, then health-code's three, an amendment's six (AC-T2; UI-1 of health-code)", () => {
    expect(sets.HealthReport).toEqual(["documents", "amendments", "notes", "code", "assumes", "unbound"]);
    expect(sets.UnappliedAmendment).toEqual(["path", "id", "title", "class", "status", "links"]);
  });

  it("names health-code's ten after DriftEvidence, in the types' order and with their keys (UI-1, AC-9 of health-code)", () => {
    expect(Object.keys(sets).slice(BEFORE_HEALTH_CODE.length)).toEqual([...HEALTH_CODE]);
    expect(HEALTH_CODE.map((name) => sets[name])).toEqual(HEALTH_CODE.map((name) => served(name)));
  });

  it("names kind-interpretation's keys: the review document's interpretation after staged_at, the Interpretation set (AC-T4)", () => {
    expect(sets.Proposal?.slice(-3)).toEqual(["staged_at", "interpretation", "notes"]);
    expect(sets.Interpretation).toEqual(["decision", "how", "why"]);
  });
});
