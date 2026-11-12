import { describe, expect, it } from "vitest";
import source from "./provisional.ts?raw";
import type {
  AssumedNode,
  AssumedProposal,
  AssumesCounts,
  AssumesRow,
  Author,
  BindingRow,
  BundleView,
  Card,
  CardAcceptance,
  CardCancellation,
  CardColumn,
  CardColumnCount,
  CardCounts,
  CardEvent,
  CardClearGroupWrite,
  CardEvidence,
  CardFlow,
  CardFlowCount,
  CardFrozen,
  CardItem,
  CardList,
  CardMove,
  CardNotFound,
  CardRow,
  CardSplit,
  CardSplitChild,
  CardSplitParent,
  CardSubtaskItem,
  CardTask,
  CardUpstream,
  CardUpstreamChange,
  CardWrite,
  CatalogChange,
  CatalogDocument,
  CatalogRow,
  CheckCause,
  CheckCounts,
  CheckFinding,
  CheckReport,
  ConfirmRead,
  DebtEntry,
  DeliveryCancelled,
  DeliveryCounts,
  DeliveryDepartment,
  DeliveryDocument,
  DeliveryItem,
  DeliveryLate,
  DeliveryMore,
  DeliveryReestimate,
  DeliveryRelease,
  DeliveryReturn,
  DriftCounts,
  DriftEvidence,
  DriftLevels,
  DriftLock,
  DriftMarker,
  DriftRow,
  FlowEdge,
  FlowGap,
  FlowInner,
  FlowNode,
  FlowView,
  FollowedType,
  GraphEdge,
  GraphNode,
  GraphView,
  HealthAssumes,
  HealthCode,
  HealthDrift,
  HealthReport,
  HealthUnbound,
  HistoryEntry,
  HubSync,
  Identity,
  InboxEntry,
  IntakeDocument,
  IntakeMatch,
  Interpretation,
  LeadDays,
  LockDiffCodeSide,
  LockDiffNode,
  LockDiffRow,
  LockDiffSpecSide,
  LockDiffView,
  NodeView,
  OwnerNote,
  Project,
  Proposal,
  QueueEvent,
  RevisionChange,
  RoadmapCounts,
  RoadmapCut,
  RoadmapDocument,
  RoadmapEstimate,
  RoadmapGroup,
  RoadmapItem,
  RoadmapItemTask,
  RoadmapPoolTask,
  RoadmapSum,
  RoundAnswerEntry,
  RoundAnswersOutcome,
  RoundItem,
  RoundSheet,
  RoundTarget,
  RoundTask,
  SearchHit,
  ShownAncestor,
  ShownBindings,
  ShownHistory,
  ShownLink,
  ShownNode,
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
  TaskRoadmap,
  TaskRun,
  TaskTarget,
  Team,
  TestedRow,
  TraceCard,
  TraceCounts,
  TraceDocument,
  TraceNode,
  TraceStage,
  TraceUse,
  UnappliedAmendment,
  UnboundCounts,
  UnboundMarker,
  UnboundRow,
  Workspace,
  WorkspaceDocument,
  WorkspaceModule,
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
// `UnboundMarker`); roadmap-surfaces adds a key, no set (UI-1 of docs/features/roadmap-surfaces.md:
// the package's `roadmap` after `bundle`, 26 keys); roadmap-screen's ten (UI-1, TE-1 of
// docs/features/roadmap-screen.md: the plan document, its counts, cut, sums, groups, items,
// estimates, item tasks, pool tasks, and the package's `TaskRoadmap`, the sixty-sixth);
// bundle-bindings' one (UI-1, RD-8 of docs/features/bundle-bindings.md: `BindingRow`, a package's
// `bindings` row, the sixty-seventh; the package keeps its 26 keys); tested-reads' one (UI, TE-5 of
// docs/features/tested-reads.md: `TestedRow`, a package's `tested` entry, the sixty-eighth and last,
// with the package's `tested` after `bindings`, 27 keys); health-drift's six (UI-1 of
// docs/features/health-drift.md: the health document's `drift` after `unbound`, row 6, verify's
// counts, its row, the row's marker, lock and levels, the seventy-fourth `DriftLevels`);
// ui-node-bindings' two (UI-8, TE-1 of docs/features/ui-node-bindings.md: a shown node's 19 keys,
// `bindings` after `links`, and its `ShownBindings`, the seventy-sixth); and lock-diff's five
// (UI-1, TE-2 of docs/features/lock-diff.md: the lock diff document, a holder, a kept row, the
// spec side and the code side, the eighty-first `LockDiffCodeSide`); and flow-read's five
// (docs/features/flow-read.md "Output": the flow document, a gap row, a node, an edge and a node's
// `inner` entry, the eighty-sixth `FlowInner`); and board-ui's twelve (docs/features/board-ui.md
// "Types": the list, a flow, a column, a move, a row, a card, an evidence entry, an acceptance, a
// cancellation, an event, the write document, no such card, the ninety-eighth `CardNotFound`); and
// card-trace's seven (docs/features/card-trace.md "Data": the trace document, its counts, a node, a
// card with its mark, a use, a frozen entry, the group clear's answer, the hundred-and-fifth
// `CardClearGroupWrite`), with an acceptance's `verdict`, `departs`, `discrepancy`; and trace-views'
// one (docs/features/trace-views.md "Routes and types": a lane, the hundred-and-sixth and last
// `TraceStage`), with `FlowNode` at 15 keys, `CardRow` at 21, `TraceDocument` at 8, `TraceNode` at 9; and
// hub-identity's one (docs/features/hub-identity.md "Data": the answer of `GET /api/identity`, the
// hundred-and-seventh `Identity`); and hub-team's one (docs/features/hub-team.md "Data": the answer of
// `GET .../team`, the hundred-and-eighth `Team`); and hub-confirm's one (docs/features/hub-confirm.md
// "Data": the answer of `GET .../confirm`, the hundred-and-ninth `ConfirmRead`), with an
// author's optional `login`, in no set; and hub-git's one (docs/features/hub-git.md "Data": the answer
// of `GET .../sync`, set 110 `HubSync`); and workspaces' three (docs/features/workspaces.md "Data": the
// answer of `GET .../workspaces`, a workspace, a module, sets 111 to 113, `WorkspaceModule`); and
// card-tasks' one (docs/features/card-tasks.md "Documents": a card's linked task, set 114 `CardTask`),
// with a card's `tasks` after `evidence` (31 keys); and flow-scenario's one (RM-0094: a shown
// node's ancestor, set 115 `ShownAncestor`), with a shown node's
// `ancestors` after `omitted` (20 keys), the flow document's `not_in_flow` and `types` after
// `omitted_edges` (13) and a flow node's `in_flow` after `end` (16); and card-questions' three
// (docs/features/card-questions.md "Acceptance criteria", K1, L3: a card's item, the intake
// document, a match, sets 116 to 118, the last), with the review document's `card_id` and `why`
// after `interpretation` (50 keys) and a card's `items` after `tasks` (32); and department-split's three
// (docs/features/department-split.md "Acceptance criteria", S3, U2: the split document, its parent and a
// subtask, sets 119 to 121); and task-mandate's one (docs/features/task-mandate.md
// "Acceptance criteria", M6, M7: a subtask's item, set 122 `CardSubtaskItem`, the last), with a row's `mandate`
// after `reviewer` (22 keys), a card's `mandate` after `reviewer` and `subtask_items` after `items`
// (34) and a split's subtask's `mandate` last (9); and card-context's one key, no set
// (docs/features/card-context.md "Acceptance criteria", CC10: a card's `context` after `history`, 35
// keys, written by its Rust test); and node-history's two (docs/features/node-history.md
// "Acceptance criteria", NH12: a shown node's history and its entry, sets 123 and 124, the last),
// with a shown node's `history` after `bindings` (21 keys); and upstream-signal's two
// (docs/features/upstream-signal.md "Acceptance criteria", US-12: what changed above a subtask and
// one change, sets 125 and 126, the last), with a row's `upstream` after `correction` (23 keys) and
// a card's (36); and intake-similar's one key, no set
// (docs/features/intake-similar.md "Acceptance criteria", IS10: the review document's `similar` after
// `why`, 51 keys, always null on the route, written by its Rust test); and delivery-analytics' eleven
// (docs/features/delivery-analytics.md "Acceptance criteria", DA-11: the delivery document, a release
// row, a department row, the total's counts, a lead time, a late task, a return, a re-estimate, a
// cancelled subtask, an item and the cut's counts, sets 127 to 137), written by its Rust test; and
// direction-catalog's three
// (docs/features/direction-catalog.md "Acceptance criteria", DC-9: the catalog document, a row and
// its last change, sets 138 to 140), written by its Rust test; and department-overview's three
// (docs/features/department-overview.md "Acceptance criteria", DO10: the counts document, a column's
// counts and a flow's, sets 141 to 143, the last), written by its Rust test; and plain-questions'
// nine keys, no set (docs/features/plain-questions.md "Acceptance criteria", PQ10: `gist` and
// `matters` after `summary` on the review document, 53 keys, a round item, 21, a card's item, 18,
// and a subtask's item, 19, and `gist` alone on an inbox entry, 13), written by its Rust test. A
// key the types carry ahead of the file
// is listed in AHEAD_OF_FIXTURE until the change that writes it; a listed key the file already
// names fails. While that file is absent the comparison is skipped, saying so; once it exists it
// names all one hundred and forty-three written so far, in the slices' order.

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
    gist: true,
    record_id: true,
    staged_at: true,
  } satisfies Record<keyof InboxEntry, true>,
  // docs/canon/proposal-queue.md "Commands" to `updated_at`,
  // docs/canon/agent-intake.md "Review document" the eleven after it,
  // docs/features/decision-apply.md "Data" the five after `linked`,
  // docs/canon/tasks.md "Task-bound proposals" (Review document) `task_id` after `choice`,
  // docs/canon/decision-staging.md "Documents" `staged`, `staged_at` between `task_id` and `notes`,
  // docs/features/kind-interpretation.md "Data" `interpretation` after `staged_at`,
  // docs/features/rev-discipline.md "Review document, stage, queue" `revisions` after `conflict`,
  // docs/features/drift-queue.md "Review, inbox, round, stage" `drift` after `revisions`,
  // docs/features/card-questions.md "Acceptance criteria" (L1) `card_id`, `why` after `interpretation`,
  // docs/features/intake-similar.md "Acceptance criteria" (IS10) `similar` after `why`,
  // docs/features/plain-questions.md "Acceptance criteria" (PQ10) `gist`, `matters` after `summary`.
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
    gist: true,
    matters: true,
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
    card_id: true,
    why: true,
    similar: true,
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
  // docs/canon/task-package.md "Package": the package and each of its parts, `author` among them;
  // docs/features/roadmap-surfaces.md "Data" `roadmap` after `bundle` (its `TaskRoadmap` set: 3a);
  // docs/canon/spec-lock-tested.md "Reads" `tested` after `bindings` (its `TestedRow` set: 8b-2).
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
    tested: true,
    spec_snapshot: true,
    snapshot_diff: true,
    claim: true,
    runs: true,
    bundle: true,
    roadmap: true,
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
  // docs/features/hub-confirm.md "Data" `login`: a departing accept's on a hub alone, skipped when none.
  Author: { type: true, role: true, model: true, run: true, login: true } satisfies Record<keyof Author, true>,
  // docs/canon/round.md "JSON": the sheet, an item's 21 keys (`gist`, `matters` after `summary`:
  // docs/features/plain-questions.md "Acceptance criteria", PQ10), a target, a task.
  RoundSheet: { project: true, items: true, more: true, minutes: true, notes: true } satisfies Record<keyof RoundSheet, true>,
  RoundItem: {
    id: true,
    kind: true,
    severity: true,
    gap_type: true,
    summary: true,
    gist: true,
    matters: true,
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
  // `code`, `assumes`, `unbound` after `notes`; docs/features/health-drift.md "Data": `drift` after them.
  HealthReport: {
    documents: true,
    amendments: true,
    notes: true,
    code: true,
    assumes: true,
    unbound: true,
    drift: true,
  } satisfies Record<keyof HealthReport, true>,
  UnappliedAmendment: { path: true, id: true, title: true, class: true, status: true, links: true } satisfies Record<keyof UnappliedAmendment, true>,
  ShownLink: {
    type: true,
    origin: true,
    at: true,
    name: true,
    title: true,
    end_kind: true,
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
  // docs/canon/spec-roadmap.md "Command": the plan document and its parts, keys in its order;
  // docs/canon/task-package.md "Package": the package's `roadmap` (roadmap-screen's ten).
  RoadmapDocument: {
    release: true,
    pool: true,
    reason: true,
    prefix: true,
    dir: true,
    unit: true,
    current: true,
    counts: true,
    cut: true,
    total: true,
    groups: true,
    pool_tasks: true,
    notes: true,
    truncated: true,
    omitted: true,
  } satisfies Record<keyof RoadmapDocument, true>,
  RoadmapCounts: { open: true, done: true } satisfies Record<keyof RoadmapCounts, true>,
  RoadmapCut: { release: true, min: true, max: true, unestimated: true } satisfies Record<keyof RoadmapCut, true>,
  RoadmapSum: { min: true, max: true, unestimated: true } satisfies Record<keyof RoadmapSum, true>,
  RoadmapGroup: {
    group: true,
    name: true,
    current: true,
    mvp_cut: true,
    open: true,
    done: true,
    sum: true,
    items: true,
  } satisfies Record<keyof RoadmapGroup, true>,
  RoadmapItem: {
    position: true,
    id: true,
    title: true,
    path: true,
    status: true,
    release: true,
    priority: true,
    estimate: true,
    waits_on: true,
    cycle: true,
    tasks: true,
  } satisfies Record<keyof RoadmapItem, true>,
  RoadmapEstimate: { min: true, max: true } satisfies Record<keyof RoadmapEstimate, true>,
  RoadmapItemTask: { id: true, status: true } satisfies Record<keyof RoadmapItemTask, true>,
  RoadmapPoolTask: { id: true, status: true, title: true, targets: true } satisfies Record<keyof RoadmapPoolTask, true>,
  TaskRoadmap: {
    item: true,
    group: true,
    release: true,
    position: true,
    of: true,
    priority: true,
    waits_on: true,
  } satisfies Record<keyof TaskRoadmap, true>,
  // docs/canon/code-reads.md "The binding row": a package's `bindings` row, its 14 keys in order
  // (docs/features/bundle-bindings.md "UI and daemon keys").
  BindingRow: {
    node: true,
    relation: true,
    file: true,
    kind: true,
    symbol: true,
    item_kind: true,
    line: true,
    end_line: true,
    signature: true,
    layer: true,
    sync: true,
    category: true,
    detail: true,
    anchor: true,
  } satisfies Record<keyof BindingRow, true>,
  // docs/canon/spec-lock-tested.md "Reads": a package's `tested` entry, plain verify's 7 keys in order
  // (docs/features/tested-reads.md, UI and TE-5).
  TestedRow: {
    node: true,
    state: true,
    category: true,
    rev: true,
    node_rev: true,
    commit: true,
    verified_at: true,
  } satisfies Record<keyof TestedRow, true>,
  // docs/features/health-drift.md "Data": row 6, then verify's counts, its row, the row's marker,
  // lock and levels, keys in docs/canon/spec-verify-output.md "JSON"'s order.
  HealthDrift: { state: true, causes: true, counts: true, rows: true, tested: true, notes: true } satisfies Record<keyof HealthDrift, true>,
  DriftCounts: {
    rows: true,
    ok: true,
    fix: true,
    check: true,
    cannot: true,
    pre_existing: true,
    skipped: true,
  } satisfies Record<keyof DriftCounts, true>,
  DriftRow: {
    node: true,
    relation: true,
    file: true,
    kind: true,
    symbol: true,
    item_kind: true,
    state: true,
    category: true,
    detail: true,
    class: true,
    pre_existing: true,
    markers: true,
    node_rev: true,
    lock: true,
    compared: true,
    levels: true,
    anchor: true,
  } satisfies Record<keyof DriftRow, true>,
  DriftMarker: { line: true, rev: true } satisfies Record<keyof DriftMarker, true>,
  DriftLock: { rev: true, commit: true, verified_at: true } satisfies Record<keyof DriftLock, true>,
  DriftLevels: { lock: true, markers: true, changed: true } satisfies Record<keyof DriftLevels, true>,
  // crates/specengine-cli/README.md "Output and the cap": `spec show --json`'s node; its 19th key
  // `bindings` after `links` and that key's three: docs/canon/code-reads.md "spec show --bindings"
  // (docs/features/ui-node-bindings.md, UI-8 and TE-1); `ancestors` after `omitted`, 20 keys (RM-0094
  // flow-scenario); `history` after `bindings`, 21 (docs/features/node-history.md "Acceptance criteria", NH12).
  ShownNode: {
    id: true,
    kind: true,
    title: true,
    path: true,
    line: true,
    end_line: true,
    status: true,
    rev: true,
    tokens_est: true,
    archived: true,
    utf8: true,
    sections: true,
    norm_hash: true,
    span_hash: true,
    text: true,
    truncated: true,
    omitted: true,
    ancestors: true,
    links: true,
    bindings: true,
    history: true,
  } satisfies Record<keyof ShownNode, true>,
  ShownBindings: { rows: true, omitted: true, tested: true } satisfies Record<keyof ShownBindings, true>,
  // docs/features/lock-diff.md "Daemon keys": the five sets after `ShownBindings`, keys in "Data"'s
  // order; a kept row's `row` is DriftRow's 17, pinned already.
  LockDiffView: { ref: true, reason: true, notes: true, nodes: true } satisfies Record<keyof LockDiffView, true>,
  LockDiffNode: { id: true, path: true, line: true, rows: true } satisfies Record<keyof LockDiffNode, true>,
  LockDiffRow: { row: true, spec: true, code: true } satisfies Record<keyof LockDiffRow, true>,
  LockDiffSpecSide: { commit: true, path: true, diff: true } satisfies Record<keyof LockDiffSpecSide, true>,
  LockDiffCodeSide: { blob: true, file: true, diff: true } satisfies Record<keyof LockDiffCodeSide, true>,
  // docs/features/flow-read.md "Output": the five sets after `LockDiffCodeSide`, keys in its order;
  // `not_in_flow`, `types` and a node's `in_flow`: docs/canon/spec-flow.md "Output" (RM-0094 flow-scenario).
  FlowView: {
    root: true,
    reason: true,
    depth: true,
    notes: true,
    left_out: true,
    truncated: true,
    omitted_nodes: true,
    omitted_edges: true,
    not_in_flow: true,
    types: true,
    gaps: true,
    nodes: true,
    edges: true,
  } satisfies Record<keyof FlowView, true>,
  FlowGap: { code: true, state: true, count: true, items: true, titles: true, cause: true } satisfies Record<keyof FlowGap, true>,
  FlowNode: {
    id: true,
    kind: true,
    title: true,
    path: true,
    line: true,
    distance: true,
    group: true,
    stage: true,
    start: true,
    end: true,
    in_flow: true,
    holds: true,
    inner: true,
    gaps: true,
    rollup: true,
    counts: true,
  } satisfies Record<keyof FlowNode, true>,
  FlowEdge: {
    from: true,
    to: true,
    type: true,
    kind: true,
    when: true,
    count: true,
    path: true,
    line: true,
  } satisfies Record<keyof FlowEdge, true>,
  FlowInner: { type: true, count: true } satisfies Record<keyof FlowInner, true>,
  // docs/canon/cards.md "Documents and events", "Commands" (the write document, no such card), "Store" (an evidence entry, the two closures).
  CardList: { flows: true, cards: true, notes: true, truncated: true, omitted: true } satisfies Record<keyof CardList, true>,
  // docs/canon/card-flows.md "The rule": a flow's `return` after `accept` (board-handoff, RM-0085).
  CardFlow: { name: true, title: true, disciplines: true, columns: true, moves: true, accept: true, return: true } satisfies Record<keyof CardFlow, true>,
  CardColumn: { id: true, title: true } satisfies Record<keyof CardColumn, true>,
  CardMove: { from: true, to: true, by: true } satisfies Record<keyof CardMove, true>,
  CardRow: {
    id: true,
    flow: true,
    column: true,
    position: true,
    title: true,
    discipline: true,
    targets: true,
    assignee: true,
    reviewer: true,
    // docs/features/task-mandate.md "Acceptance criteria" (M1): the mandate after the reviewer.
    mandate: true,
    estimate: true,
    due: true,
    parent: true,
    corrects: true,
    updated_at: true,
    revision: true,
    accepted: true,
    cancelled: true,
    mark: true,
    on: true,
    discrepancy: true,
    correction: true,
    // docs/features/upstream-signal.md "Acceptance criteria" (US-12): the upstream flag after `correction`, ahead of the file.
    upstream: true,
  } satisfies Record<keyof CardRow, true>,
  Card: {
    id: true,
    flow: true,
    column: true,
    position: true,
    title: true,
    goal: true,
    discipline: true,
    targets: true,
    mark: true,
    on: true,
    discrepancy: true,
    correction: true,
    // docs/features/upstream-signal.md "Acceptance criteria" (US-12): what changed above a subtask, after `correction`, ahead of the file.
    upstream: true,
    snapshot: true,
    criteria: true,
    assignee: true,
    reviewer: true,
    // docs/features/task-mandate.md "Acceptance criteria" (M1, M6): the mandate after the reviewer, the subtasks' items after the card's.
    mandate: true,
    estimate: true,
    due: true,
    parent: true,
    corrects: true,
    evidence: true,
    // docs/features/card-tasks.md "Documents": the linked tasks after `evidence` (card-tasks, RM-0092).
    tasks: true,
    items: true,
    subtask_items: true,
    acceptance: true,
    cancellation: true,
    created_by: true,
    created_at: true,
    updated_at: true,
    revision: true,
    allowed: true,
    history: true,
    // docs/features/card-context.md "Acceptance criteria" (CC10): always null from the daemon, after `history`.
    context: true,
    notes: true,
  } satisfies Record<keyof Card, true>,
  CardEvidence: { kind: true, value: true, by: true, at: true } satisfies Record<keyof CardEvidence, true>,
  CardAcceptance: { by: true, at: true, note: true, verdict: true, departs: true, discrepancy: true } satisfies Record<keyof CardAcceptance, true>,
  CardCancellation: { by: true, at: true, reason: true } satisfies Record<keyof CardCancellation, true>,
  CardEvent: { seq: true, type: true, at: true, payload: true } satisfies Record<keyof CardEvent, true>,
  CardWrite: { id: true, flow: true, column: true, position: true, revision: true, allowed: true, notes: true } satisfies Record<keyof CardWrite, true>,
  CardNotFound: { id: true, reason: true } satisfies Record<keyof CardNotFound, true>,
  // docs/features/card-trace.md "Data".
  TraceDocument: { ref: true, reason: true, notes: true, counts: true, stages: true, nodes: true, truncated: true, omitted: true } satisfies Record<keyof TraceDocument, true>,
  TraceCounts: { no_card: true, in_work: true, suspect: true, departed: true, confirmed: true } satisfies Record<keyof TraceCounts, true>,
  TraceNode: { id: true, title: true, path: true, stage: true, distance: true, rollup: true, counts: true, cards: true, uses: true } satisfies Record<keyof TraceNode, true>,
  TraceCard: { id: true, flow: true, column: true, title: true, mark: true, on: true, discrepancy: true, correction: true } satisfies Record<keyof TraceCard, true>,
  TraceUse: { id: true, rollup: true, cards: true } satisfies Record<keyof TraceUse, true>,
  // docs/features/trace-views.md "Routes and types": one lane.
  TraceStage: { stage: true, distance: true, counts: true } satisfies Record<keyof TraceStage, true>,
  CardFrozen: { id: true, rev: true, norm_hash: true, within_hash: true, also: true } satisfies Record<keyof CardFrozen, true>,
  CardClearGroupWrite: { group: true, cleared: true, notes: true } satisfies Record<keyof CardClearGroupWrite, true>,
  // docs/canon/hub.md "Routes": the identity read's answer.
  Identity: { login: true } satisfies Record<keyof Identity, true>,
  // docs/features/hub-team.md "Data": the team read's answer, the asker's own row, no role.
  Team: { login: true, name: true, allowed: true } satisfies Record<keyof Team, true>,
  // docs/features/hub-confirm.md "Data": the confirm read's answer.
  ConfirmRead: { id: true, question: true, updated_at: true } satisfies Record<keyof ConfirmRead, true>,
  // docs/features/hub-git.md "Data": the sync read's answer, all five keys always present.
  HubSync: { ahead: true, behind: true, state: true, error: true, at: true } satisfies Record<keyof HubSync, true>,
  // docs/features/workspaces.md "Data": the workspaces read's answer, a workspace, a module; every key present.
  WorkspaceDocument: { source: true, notes: true, workspaces: true, modules: true } satisfies Record<keyof WorkspaceDocument, true>,
  Workspace: { name: true, title: true, landing: true, sections: true, flows: true, disciplines: true, module: true } satisfies Record<keyof Workspace, true>,
  WorkspaceModule: { name: true, title: true, root: true, kinds: true } satisfies Record<keyof WorkspaceModule, true>,
  // docs/features/card-tasks.md "Documents": one linked task, read when the card is; every key present.
  CardTask: { id: true, status: true, title: true, stale: true } satisfies Record<keyof CardTask, true>,
  // crates/specengine-cli/README.md "Output and the cap": one entry of a shown node's `ancestors` (RM-0094); every key present.
  ShownAncestor: { id: true, kind: true, title: true, path: true } satisfies Record<keyof ShownAncestor, true>,
  // docs/features/card-questions.md "Acceptance criteria" (K1, L3): a card's item (set 116), the intake document (117) and a match (118); every key present.
  // A card's item's `gist` and `matters` after `summary` (docs/features/plain-questions.md "Acceptance criteria", PQ10).
  CardItem: {
    id: true,
    kind: true,
    status: true,
    summary: true,
    gist: true,
    matters: true,
    working_answer: true,
    why: true,
    author: true,
    created_at: true,
    updated_at: true,
    staged_at: true,
    decided_by: true,
    decided_at: true,
    decision_note: true,
    record_id: true,
    record_path: true,
    record_title: true,
  } satisfies Record<keyof CardItem, true>,
  IntakeDocument: { id: true, created: true, hits: true, related: true, linked: true, diagnostics: true, notes: true } satisfies Record<keyof IntakeDocument, true>,
  IntakeMatch: { id: true, source: true, status: true, path: true, answer: true, record: true } satisfies Record<keyof IntakeMatch, true>,
  // docs/features/department-split.md "Acceptance criteria" (S3, U2): the split document (set 119), its parent (120) and a subtask (121); every key present.
  CardSplit: { dry_run: true, parent: true, children: true, reviewer: true, notes: true } satisfies Record<keyof CardSplit, true>,
  CardSplitParent: { id: true, created: true, title: true, discipline: true, flow: true, flow_title: true, column: true, targets: true } satisfies Record<keyof CardSplitParent, true>,
  CardSplitChild: { id: true, discipline: true, flow: true, flow_title: true, column: true, targets: true, from: true, suggested: true, mandate: true } satisfies Record<keyof CardSplitChild, true>,
  // docs/features/task-mandate.md "Acceptance criteria" (M6, M7): a subtask's item (set 122), its card first, then a card's item's 18; every key present.
  CardSubtaskItem: {
    card_id: true,
    id: true,
    kind: true,
    status: true,
    summary: true,
    gist: true,
    matters: true,
    working_answer: true,
    why: true,
    author: true,
    created_at: true,
    updated_at: true,
    staged_at: true,
    decided_by: true,
    decided_at: true,
    decision_note: true,
    record_id: true,
    record_path: true,
    record_title: true,
  } satisfies Record<keyof CardSubtaskItem, true>,
  // docs/features/node-history.md "Acceptance criteria" (NH12): a shown node's history (set 123) and
  // its entry's 15 keys in the table's order (set 124); every key present.
  ShownHistory: { entries: true, truncated: true, omitted: true } satisfies Record<keyof ShownHistory, true>,
  HistoryEntry: {
    source: true,
    at: true,
    id: true,
    change: true,
    created: true,
    rev: true,
    rev_before: true,
    by: true,
    by_name: true,
    proposal: true,
    proposed_by: true,
    proposer: true,
    record: true,
    title: true,
    rationale: true,
  } satisfies Record<keyof HistoryEntry, true>,
  // docs/features/upstream-signal.md "Acceptance criteria" (US-12): what changed above a subtask
  // (set 125) and one change's nine keys in "JSON"'s order (set 126); every key present.
  CardUpstream: {
    parent: true,
    mark: true,
    on: true,
    correction: true,
    changes: true,
    more: true,
  } satisfies Record<keyof CardUpstream, true>,
  CardUpstreamChange: {
    seq: true,
    at: true,
    source: true,
    by: true,
    fields: true,
    item: true,
    kind: true,
    record: true,
    on: true,
  } satisfies Record<keyof CardUpstreamChange, true>,
  // docs/features/delivery-analytics.md "Acceptance criteria" (DA-11): the eleven sets in "JSON"'s
  // order, sets 127 to 137, the fixture's names; `Delivery` is the document's type, `DeliveryTotal`
  // the counts' (a release and a department row carry them after their own two keys).
  Delivery: { as_of: true, notes: true, releases: true, departments: true, total: true, late: true, truncated: true, omitted: true } satisfies Record<keyof DeliveryDocument, true>,
  DeliveryRelease: {
    group: true,
    release: true,
    tasks: true,
    delivered: true,
    delivered_late: true,
    open: true,
    overdue: true,
    cancelled: true,
    lead_days: true,
  } satisfies Record<keyof DeliveryRelease, true>,
  DeliveryDepartment: {
    name: true,
    title: true,
    tasks: true,
    delivered: true,
    delivered_late: true,
    open: true,
    overdue: true,
    cancelled: true,
    lead_days: true,
  } satisfies Record<keyof DeliveryDepartment, true>,
  DeliveryTotal: { tasks: true, delivered: true, delivered_late: true, open: true, overdue: true, cancelled: true, lead_days: true } satisfies Record<keyof DeliveryCounts, true>,
  LeadDays: { median: true, max: true } satisfies Record<keyof LeadDays, true>,
  DeliveryLate: {
    id: true,
    title: true,
    flow: true,
    column: true,
    parent: true,
    group: true,
    release: true,
    departments: true,
    created_at: true,
    taken_at: true,
    accepted_at: true,
    due: true,
    days_late: true,
    lead_days: true,
    estimate: true,
    returns: true,
    reestimates: true,
    cancelled_subtasks: true,
    items: true,
    more: true,
  } satisfies Record<keyof DeliveryLate, true>,
  DeliveryReturn: { at: true, from: true, note: true } satisfies Record<keyof DeliveryReturn, true>,
  DeliveryReestimate: { at: true, old: true, new: true, reason: true } satisfies Record<keyof DeliveryReestimate, true>,
  DeliveryCancelled: { id: true, title: true, at: true, reason: true } satisfies Record<keyof DeliveryCancelled, true>,
  DeliveryItem: { id: true, kind: true, status: true } satisfies Record<keyof DeliveryItem, true>,
  DeliveryMore: { returns: true, reestimates: true, cancelled_subtasks: true, items: true } satisfies Record<keyof DeliveryMore, true>,
  // docs/features/direction-catalog.md "Acceptance criteria" (DC-9): the three sets in "JSON"'s
  // order, sets 138 to 140, the fixture's names; `Catalog` is the document's type.
  Catalog: { kinds: true, archive: true, notes: true, rows: true, truncated: true, omitted: true } satisfies Record<keyof CatalogDocument, true>,
  CatalogRow: {
    id: true,
    kind: true,
    title: true,
    path: true,
    line: true,
    rev: true,
    status: true,
    archived: true,
    state: true,
    open_items: true,
    tasks: true,
    tasks_open: true,
    last_change: true,
  } satisfies Record<keyof CatalogRow, true>,
  CatalogChange: { at: true, by: true, by_name: true, commit: true, proposal: true, of_file: true } satisfies Record<keyof CatalogChange, true>,
  // docs/features/department-overview.md "Acceptance criteria" (DO10): the three sets in "JSON"'s
  // order, sets 141 to 143, the fixture's names; the document's `flows` reuses CardFlow.
  CardCounts: { flows: true, columns: true, by_flow: true, agent_tasks: true, notes: true, truncated: true, omitted: true } satisfies Record<keyof CardCounts, true>,
  CardColumnCount: { flow: true, column: true, open: true, accepted: true, cancelled: true, open_subtasks: true } satisfies Record<keyof CardColumnCount, true>,
  CardFlowCount: { flow: true, open_items: true, oldest_open_item: true, ready_parents: true, review_agent_tasks: true } satisfies Record<keyof CardFlowCount, true>,
};

type Named = keyof typeof KEYS;

/** The key lists as the cited headings write them, copied verbatim (the review document's joined). */
const CITED: Record<Named, string> = {
  Project: "slug, name, root, branch",
  InboxEntry: "id, kind, status, target_id, target_ids, branch, created_at, rationale, severity, summary, gist, record_id, staged_at",
  Proposal:
    "id, project, kind, status, target_id, target_path, worktree, branch, base_commit, base_hash, base_text, new_text, patch_hash, rationale, author, diagnostics, diff, preview, conflict, " +
    "revisions, " +
    "drift, " +
    "decided_by, decided_at, decision_note, applied_commit, created_at, updated_at, " +
    "target_ids, severity, gap_type, summary, gist, matters, working_answer, price_of_other, evidence, options, recommendation, distinct_from, linked, " +
    "record_id, record_path, record_title, record_text, choice, " +
    "task_id, " +
    "staged, staged_at, " +
    "interpretation, " +
    "card_id, why, " +
    "similar, " +
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
    "schema_version, id, project, status, title, goal, profile, stale, targets, criteria, affected_nodes, plan, assumptions, open_proposals, owner_notes, bindings, tested, spec_snapshot, snapshot_diff, claim, runs, bundle, roadmap, author, created_at, updated_at, notes",
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
  Author: "type, role, model, run, login?",
  RoundSheet: "project, items, more, minutes, notes",
  RoundItem:
    "id, kind, severity, gap_type, summary, gist, matters, working_answer, price_of_other, options, recommendation, evidence, targets, tasks, worktree, branch, author, staged, staged_at, created_at, updated_at",
  RoundTarget: "id, path, title",
  RoundTask: "id, status, title",
  RoundAnswersOutcome: "project, answers, notes",
  RoundAnswerEntry: "id, outcome, stage, staged_at, replaced, reason",
  HealthReport: "documents, amendments, notes, code, assumes, unbound, drift",
  UnappliedAmendment: "path, id, title, class, status, links",
  ShownLink: "type, origin, at, name, title, end_kind, written, path, line, state, reason",
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
  RoadmapDocument: "release, pool, reason, prefix, dir, unit, current, counts, cut, total, groups, pool_tasks, notes, truncated, omitted",
  RoadmapCounts: "open, done",
  RoadmapCut: "release, min, max, unestimated",
  RoadmapSum: "min, max, unestimated",
  RoadmapGroup: "group, name, current, mvp_cut, open, done, sum, items",
  RoadmapItem: "position, id, title, path, status, release, priority, estimate, waits_on, cycle, tasks",
  RoadmapEstimate: "min, max",
  RoadmapItemTask: "id, status",
  RoadmapPoolTask: "id, status, title, targets",
  TaskRoadmap: "item, group, release, position, of, priority, waits_on",
  BindingRow: "node, relation, file, kind, symbol, item_kind, line, end_line, signature, layer, sync, category, detail, anchor",
  TestedRow: "node, state, category, rev, node_rev, commit, verified_at",
  HealthDrift: "state, causes, counts, rows, tested, notes",
  DriftCounts: "rows, ok, fix, check, cannot, pre_existing, skipped",
  DriftRow: "node, relation, file, kind, symbol, item_kind, state, category, detail, class, pre_existing, markers, node_rev, lock, compared, levels, anchor",
  DriftMarker: "line, rev",
  DriftLock: "rev, commit, verified_at",
  DriftLevels: "lock, markers, changed",
  ShownNode: "id, kind, title, path, line, end_line, status, rev, tokens_est, archived, utf8, sections, norm_hash, span_hash, text, truncated, omitted, ancestors, links, bindings, history",
  ShownBindings: "rows, omitted, tested",
  LockDiffView: "ref, reason, notes, nodes",
  LockDiffNode: "id, path, line, rows",
  LockDiffRow: "row, spec, code",
  LockDiffSpecSide: "commit, path, diff",
  LockDiffCodeSide: "blob, file, diff",
  FlowView: "root, reason, depth, notes, left_out, truncated, omitted_nodes, omitted_edges, not_in_flow, types, gaps, nodes, edges",
  FlowGap: "code, state, count, items, titles, cause",
  FlowNode: "id, kind, title, path, line, distance, group, stage, start, end, in_flow, holds, inner, gaps, rollup, counts",
  FlowEdge: "from, to, type, kind, when, count, path, line",
  FlowInner: "type, count",
  CardList: "flows, cards, notes, truncated, omitted",
  CardFlow: "name, title, disciplines, columns, moves, accept, return",
  CardColumn: "id, title",
  CardMove: "from, to, by",
  CardRow: "id, flow, column, position, title, discipline, targets, assignee, reviewer, mandate, estimate, due, parent, corrects, updated_at, revision, accepted, cancelled, mark, on, discrepancy, correction, upstream",
  Card: "id, flow, column, position, title, goal, discipline, targets, mark, on, discrepancy, correction, upstream, snapshot, criteria, assignee, reviewer, mandate, estimate, due, parent, corrects, evidence, tasks, items, subtask_items, acceptance, cancellation, created_by, created_at, updated_at, revision, allowed, history, context, notes",
  CardEvidence: "kind, value, by, at",
  CardAcceptance: "by, at, note, verdict, departs, discrepancy",
  CardCancellation: "by, at, reason",
  CardEvent: "seq, type, at, payload",
  CardWrite: "id, flow, column, position, revision, allowed, notes",
  CardNotFound: "id, reason",
  TraceDocument: "ref, reason, notes, counts, stages, nodes, truncated, omitted",
  TraceCounts: "no_card, in_work, suspect, departed, confirmed",
  TraceNode: "id, title, path, stage, distance, rollup, counts, cards, uses",
  TraceCard: "id, flow, column, title, mark, on, discrepancy, correction",
  TraceUse: "id, rollup, cards",
  TraceStage: "stage, distance, counts",
  CardFrozen: "id, rev, norm_hash, within_hash, also",
  CardClearGroupWrite: "group, cleared, notes",
  Identity: "login",
  Team: "login, name, allowed",
  ConfirmRead: "id, question, updated_at",
  HubSync: "ahead, behind, state, error, at",
  WorkspaceDocument: "source, notes, workspaces, modules",
  Workspace: "name, title, landing, sections, flows, disciplines, module",
  WorkspaceModule: "name, title, root, kinds",
  CardTask: "id, status, title, stale",
  ShownAncestor: "id, kind, title, path",
  CardItem: "id, kind, status, summary, gist, matters, working_answer, why, author, created_at, updated_at, staged_at, decided_by, decided_at, decision_note, record_id, record_path, record_title",
  IntakeDocument: "id, created, hits, related, linked, diagnostics, notes",
  IntakeMatch: "id, source, status, path, answer, record",
  CardSplit: "dry_run, parent, children, reviewer, notes",
  CardSplitParent: "id, created, title, discipline, flow, flow_title, column, targets",
  CardSplitChild: "id, discipline, flow, flow_title, column, targets, from, suggested, mandate",
  CardSubtaskItem: "card_id, id, kind, status, summary, gist, matters, working_answer, why, author, created_at, updated_at, staged_at, decided_by, decided_at, decision_note, record_id, record_path, record_title",
  ShownHistory: "entries, truncated, omitted",
  HistoryEntry: "source, at, id, change, created, rev, rev_before, by, by_name, proposal, proposed_by, proposer, record, title, rationale",
  CardUpstream: "parent, mark, on, correction, changes, more",
  CardUpstreamChange: "seq, at, source, by, fields, item, kind, record, on",
  Delivery: "as_of, notes, releases, departments, total, late, truncated, omitted",
  DeliveryRelease: "group, release, tasks, delivered, delivered_late, open, overdue, cancelled, lead_days",
  DeliveryDepartment: "name, title, tasks, delivered, delivered_late, open, overdue, cancelled, lead_days",
  DeliveryTotal: "tasks, delivered, delivered_late, open, overdue, cancelled, lead_days",
  LeadDays: "median, max",
  DeliveryLate:
    "id, title, flow, column, parent, group, release, departments, created_at, taken_at, accepted_at, due, days_late, lead_days, estimate, returns, reestimates, cancelled_subtasks, items, more",
  DeliveryReturn: "at, from, note",
  DeliveryReestimate: "at, old, new, reason",
  DeliveryCancelled: "id, title, at, reason",
  DeliveryItem: "id, kind, status",
  DeliveryMore: "returns, reestimates, cancelled_subtasks, items",
  Catalog: "kinds, archive, notes, rows, truncated, omitted",
  CatalogRow: "id, kind, title, path, line, rev, status, archived, state, open_items, tasks, tasks_open, last_change",
  CatalogChange: "at, by, by_name, commit, proposal, of_file",
  CardCounts: "flows, columns, by_flow, agent_tasks, notes, truncated, omitted",
  CardColumnCount: "flow, column, open, accepted, cancelled, open_subtasks",
  CardFlowCount: "flow, open_items, oldest_open_item, ready_parents, review_agent_tasks",
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

/** decision-staging's two: the stage's shapes (docs/canon/decision-staging.md "The stage", "Documents"). */
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

/** The fifty-six types the fixture named before roadmap-screen. */
const BEFORE_ROADMAP_SCREEN: readonly Named[] = [...BEFORE_HEALTH_CODE, ...HEALTH_CODE];

/**
 * roadmap-screen's ten: the plan document and its parts in `docs/canon/spec-roadmap.md` "Command"'s
 * order, then the package's `TaskRoadmap` (docs/features/roadmap-screen.md UI-1, TE-1).
 */
const ROADMAP_SCREEN: readonly Named[] = [
  "RoadmapDocument",
  "RoadmapCounts",
  "RoadmapCut",
  "RoadmapSum",
  "RoadmapGroup",
  "RoadmapItem",
  "RoadmapEstimate",
  "RoadmapItemTask",
  "RoadmapPoolTask",
  "TaskRoadmap",
];

/** The sixty-six types the fixture named before bundle-bindings. */
const BEFORE_BUNDLE_BINDINGS: readonly Named[] = [...BEFORE_ROADMAP_SCREEN, ...ROADMAP_SCREEN];

/** bundle-bindings' one: a package's `bindings` row (docs/features/bundle-bindings.md "UI and daemon keys", UI-1). */
const BUNDLE_BINDINGS: readonly Named[] = ["BindingRow"];

/** The sixty-seven types the fixture named before tested-reads. */
const BEFORE_TESTED_READS: readonly Named[] = [...BEFORE_BUNDLE_BINDINGS, ...BUNDLE_BINDINGS];

/** tested-reads' one: a package's `tested` entry (docs/canon/spec-lock-tested.md "Reads"; docs/features/tested-reads.md TE-5). */
const TESTED_READS: readonly Named[] = ["TestedRow"];

/** The sixty-eight types the fixture named before health-drift. */
const BEFORE_HEALTH_DRIFT: readonly Named[] = [...BEFORE_TESTED_READS, ...TESTED_READS];

/**
 * health-drift's six: row 6, verify's counts, its row, the row's marker, lock and levels, in
 * "Data"'s order (docs/features/health-drift.md UI-1); the row's anchor and its places are the
 * binding row's types, its `tested` entries TestedRow.
 */
const HEALTH_DRIFT: readonly Named[] = ["HealthDrift", "DriftCounts", "DriftRow", "DriftMarker", "DriftLock", "DriftLevels"];

/** The seventy-four types the fixture named before ui-node-bindings. */
const BEFORE_UI_NODE_BINDINGS: readonly Named[] = [...BEFORE_HEALTH_DRIFT, ...HEALTH_DRIFT];

/** ui-node-bindings' two: a shown node and its bindings (docs/features/ui-node-bindings.md UI-8, TE-1). */
const UI_NODE_BINDINGS: readonly Named[] = ["ShownNode", "ShownBindings"];

/** The seventy-six types the fixture named before lock-diff. */
const BEFORE_LOCK_DIFF: readonly Named[] = [...BEFORE_UI_NODE_BINDINGS, ...UI_NODE_BINDINGS];

/**
 * lock-diff's five: the document, a holder, a kept row, the spec side, the code side, in "Daemon
 * keys"' order (docs/features/lock-diff.md UI-1, TE-2).
 */
const LOCK_DIFF: readonly Named[] = ["LockDiffView", "LockDiffNode", "LockDiffRow", "LockDiffSpecSide", "LockDiffCodeSide"];

/** The eighty-one types the fixture named before flow-read. */
const BEFORE_FLOW_READ: readonly Named[] = [...BEFORE_LOCK_DIFF, ...LOCK_DIFF];

/** flow-read's five: the document, a gap row, a node, an edge, a node's `inner` entry (docs/features/flow-read.md "Output"). */
const FLOW_READ: readonly Named[] = ["FlowView", "FlowGap", "FlowNode", "FlowEdge", "FlowInner"];

/** The eighty-six types the fixture named before board-ui. */
const BEFORE_BOARD_UI: readonly Named[] = [...BEFORE_FLOW_READ, ...FLOW_READ];

/** board-ui's twelve, in the list's order, then the card's, the write document, no such card (docs/features/board-ui.md "Types"). */
const BOARD_UI: readonly Named[] = ["CardList", "CardFlow", "CardColumn", "CardMove", "CardRow", "Card", "CardEvidence", "CardAcceptance", "CardCancellation", "CardEvent", "CardWrite", "CardNotFound"];

/** The ninety-eight types the fixture named before card-trace. */
const BEFORE_CARD_TRACE: readonly Named[] = [...BEFORE_BOARD_UI, ...BOARD_UI];

/** card-trace's seven, in the file's order (docs/features/card-trace.md "Data"). */
const CARD_TRACE: readonly Named[] = ["TraceDocument", "TraceCounts", "TraceNode", "TraceCard", "TraceUse", "CardFrozen", "CardClearGroupWrite"];

/** The hundred and five types the fixture named before trace-views. */
const BEFORE_TRACE_VIEWS: readonly Named[] = [...BEFORE_CARD_TRACE, ...CARD_TRACE];

/** trace-views' one: a lane of `spec trace` (docs/features/trace-views.md "Routes and types"). */
const TRACE_VIEWS: readonly Named[] = ["TraceStage"];

/** The hundred and six types the fixture named before hub-identity. */
const BEFORE_HUB_IDENTITY: readonly Named[] = [...BEFORE_TRACE_VIEWS, ...TRACE_VIEWS];

/** hub-identity's one: the identity read's answer (docs/features/hub-identity.md "Data"). */
const HUB_IDENTITY: readonly Named[] = ["Identity"];

/** The hundred and seven types the fixture named before hub-team. */
const BEFORE_HUB_TEAM: readonly Named[] = [...BEFORE_HUB_IDENTITY, ...HUB_IDENTITY];

/** hub-team's one: the team read's answer (docs/features/hub-team.md "Data"). */
const HUB_TEAM: readonly Named[] = ["Team"];

/** The hundred and eight types the fixture named before hub-confirm. */
const BEFORE_HUB_CONFIRM: readonly Named[] = [...BEFORE_HUB_TEAM, ...HUB_TEAM];

/** hub-confirm's one: the confirm read's answer (docs/features/hub-confirm.md "Data"). */
const HUB_CONFIRM: readonly Named[] = ["ConfirmRead"];

/** The hundred and nine types the fixture named before hub-git. */
const BEFORE_HUB_GIT: readonly Named[] = [...BEFORE_HUB_CONFIRM, ...HUB_CONFIRM];

/** hub-git's one: the sync read's answer, set 110 (docs/features/hub-git.md "Data"). */
const HUB_GIT: readonly Named[] = ["HubSync"];

/** The hundred and ten types the fixture named before workspaces. */
const BEFORE_WORKSPACES: readonly Named[] = [...BEFORE_HUB_GIT, ...HUB_GIT];

/** workspaces' three: the workspaces read's answer, a workspace, a module, sets 111 to 113 (docs/features/workspaces.md "Data"). */
const WORKSPACES: readonly Named[] = ["WorkspaceDocument", "Workspace", "WorkspaceModule"];

/** The hundred and thirteen types the fixture named before card-tasks. */
const BEFORE_CARD_TASKS: readonly Named[] = [...BEFORE_WORKSPACES, ...WORKSPACES];

/** card-tasks' one: a card's linked task, set 114 (docs/features/card-tasks.md "Documents"). */
const CARD_TASKS: readonly Named[] = ["CardTask"];

/** The hundred and fourteen types the fixture named before flow-scenario. */
const BEFORE_FLOW_SCENARIO: readonly Named[] = [...BEFORE_CARD_TASKS, ...CARD_TASKS];

/** flow-scenario's one: a shown node's ancestor, set 115 (RM-0094; fixtures/daemon-keys.json). */
const FLOW_SCENARIO: readonly Named[] = ["ShownAncestor"];

/** The hundred and fifteen types the fixture named before card-questions. */
const BEFORE_CARD_QUESTIONS: readonly Named[] = [...BEFORE_FLOW_SCENARIO, ...FLOW_SCENARIO];

/** card-questions' three: a card's item, the intake document, a match, sets 116 to 118 (docs/features/card-questions.md "Acceptance criteria", K1, L3). */
const CARD_QUESTIONS: readonly Named[] = ["CardItem", "IntakeDocument", "IntakeMatch"];

/** The hundred and eighteen types the fixture named before department-split. */
const BEFORE_DEPARTMENT_SPLIT: readonly Named[] = [...BEFORE_CARD_QUESTIONS, ...CARD_QUESTIONS];

/** department-split's three: the split document, its parent, a subtask, sets 119 to 121 (docs/features/department-split.md "Acceptance criteria", S3, U2). */
const DEPARTMENT_SPLIT: readonly Named[] = ["CardSplit", "CardSplitParent", "CardSplitChild"];

/** The hundred and twenty-one types the fixture named before task-mandate. */
const BEFORE_TASK_MANDATE: readonly Named[] = [...BEFORE_DEPARTMENT_SPLIT, ...DEPARTMENT_SPLIT];

/** task-mandate's one: a subtask's item, set 122 (docs/features/task-mandate.md "Acceptance criteria", M6, M7). */
const TASK_MANDATE: readonly Named[] = ["CardSubtaskItem"];

/** The hundred and twenty-two types the fixture named before node-history. */
const BEFORE_NODE_HISTORY: readonly Named[] = [...BEFORE_TASK_MANDATE, ...TASK_MANDATE];

/** node-history's two: a shown node's history and its entry, sets 123 and 124 (docs/features/node-history.md "Acceptance criteria", NH12). */
const NODE_HISTORY: readonly Named[] = ["ShownHistory", "HistoryEntry"];

/** The hundred and twenty-four types the fixture named before upstream-signal. */
const BEFORE_UPSTREAM_SIGNAL: readonly Named[] = [...BEFORE_NODE_HISTORY, ...NODE_HISTORY];

/** upstream-signal's two: what changed above a subtask and one change, sets 125 and 126 (docs/features/upstream-signal.md "Acceptance criteria", US-12). */
const UPSTREAM_SIGNAL: readonly Named[] = ["CardUpstream", "CardUpstreamChange"];

/** The hundred and twenty-six types the fixture named before delivery-analytics. */
const BEFORE_DELIVERY_ANALYTICS: readonly Named[] = [...BEFORE_UPSTREAM_SIGNAL, ...UPSTREAM_SIGNAL];

/** delivery-analytics' eleven, sets 127 to 137 in "JSON"'s order (docs/features/delivery-analytics.md "Acceptance criteria", DA-11). */
const DELIVERY_ANALYTICS: readonly Named[] = [
  "Delivery",
  "DeliveryRelease",
  "DeliveryDepartment",
  "DeliveryTotal",
  "LeadDays",
  "DeliveryLate",
  "DeliveryReturn",
  "DeliveryReestimate",
  "DeliveryCancelled",
  "DeliveryItem",
  "DeliveryMore",
];

/** The hundred and thirty-seven types the fixture names before direction-catalog. */
const BEFORE_DIRECTION_CATALOG: readonly Named[] = [...BEFORE_DELIVERY_ANALYTICS, ...DELIVERY_ANALYTICS];

/** direction-catalog's three, sets 138 to 140 in "JSON"'s order (docs/features/direction-catalog.md "Acceptance criteria", DC-9). */
const DIRECTION_CATALOG: readonly Named[] = ["Catalog", "CatalogRow", "CatalogChange"];

/** The hundred and forty types the fixture names before department-overview. */
const BEFORE_DEPARTMENT_OVERVIEW: readonly Named[] = [...BEFORE_DIRECTION_CATALOG, ...DIRECTION_CATALOG];

/** department-overview's three, sets 141 to 143 in "JSON"'s order (docs/features/department-overview.md "Acceptance criteria", DO10). */
const DEPARTMENT_OVERVIEW: readonly Named[] = ["CardCounts", "CardColumnCount", "CardFlowCount"];

/**
 * plain-questions' keys, on five sets the fixture already names, as pairs (docs/features/plain-questions.md
 * "Acceptance criteria", PQ10): a question's lead after `summary` on the review document, the inbox entry
 * (`gist` alone), the round item and a card's item; and a subtask's item, which flattens a card's.
 */
const PLAIN_QUESTIONS: readonly (readonly [Named, string])[] = [
  ["Proposal", "gist"],
  ["Proposal", "matters"],
  ["InboxEntry", "gist"],
  ["RoundItem", "gist"],
  ["RoundItem", "matters"],
  ["CardItem", "gist"],
  ["CardItem", "matters"],
  ["CardSubtaskItem", "gist"],
  ["CardSubtaskItem", "matters"],
];

/** The types each set names in `provisional.ts`, where the fixture's name is not the type's own. */
const DECLARED_AS: Partial<Record<Named, string>> = { Delivery: "DeliveryDocument", DeliveryTotal: "DeliveryCounts", Catalog: "CatalogDocument" };

/** Every type the fixture names, in the slices' order. */
const ALL: readonly Named[] = [...BEFORE_DEPARTMENT_OVERVIEW, ...DEPARTMENT_OVERVIEW];

/**
 * The keys only a check against a git base sends: the daemon runs the plain check, so it never
 * serves them (`docs/canon/spec-check.md` "Findings, debt, verdict"; docs/features/ui-live.md "Data");
 * and an author's `login`, skipped when none, so the file keeps `Author` at four keys
 * (docs/features/hub-confirm.md "Data").
 */
const NEVER_SERVED: readonly (readonly [Named, string])[] = [
  ["CheckReport", "new_debt"],
  ["CheckCounts", "introduced"],
  ["CheckCounts", "new_debt"],
  ["CheckFinding", "introduced"],
  ["Author", "login"],
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
 * workspaces' test wrote its three sets, the hundred-and-eleventh to the hundred-and-thirteenth
 * (docs/features/workspaces.md "Data"), since board-handoff's wrote a flow's `return`, since
 * card-tasks' wrote a card's `tasks` and set 114 `CardTask` (docs/features/card-tasks.md
 * "Documents"), since flow-scenario's (RM-0094) wrote its four keys and set 115 `ShownAncestor`,
 * since doc-modal's wrote a link's `title` and `end_kind` and a gap row's `titles`
 * (docs/features/doc-modal.md "Acceptance criteria", DM-2), and since departments' wrote a
 * flow's `title`, a workspace's `disciplines` and a module's `kinds`
 * (docs/features/departments.md "Acceptance criteria", DP-4), and again since card-questions'
 * wrote the review document's `card_id` and `why`, a card's `items` and sets 116 to 118
 * (docs/features/card-questions.md "Acceptance criteria", K1, L1, L3), and since department-split's
 * wrote sets 119 to 121 (docs/features/department-split.md "Acceptance criteria", S3, U2), and
 * since task-mandate's wrote a row's and a card's `mandate`, a card's `subtask_items`, a split's
 * subtask's `mandate` and set 122 (docs/features/task-mandate.md "Acceptance criteria", M7), and
 * since card-context's wrote a card's `context`
 * (docs/features/card-context.md "Acceptance criteria", CC10), and since node-history's wrote a
 * shown node's `history` and sets 123 and 124 (docs/features/node-history.md "Acceptance criteria",
 * NH12), and since upstream-signal's wrote a row's and a card's `upstream` and sets 125 and 126
 * (docs/features/upstream-signal.md "Acceptance criteria", US-12). intake-similar's review
 * document's `similar` until its Rust test wrote it
 * (docs/features/intake-similar.md "Acceptance criteria", IS10), then empty again; and
 * delivery-analytics' eleven sets, every key of each, until its Rust test wrote them
 * (docs/features/delivery-analytics.md "Acceptance criteria", DA-11), then empty again; and
 * direction-catalog's three sets, every key of each, until its Rust test wrote them
 * (docs/features/direction-catalog.md "Acceptance criteria", DC-9), then empty again; and
 * department-overview's three sets, every key of each, until its Rust test wrote them
 * (docs/features/department-overview.md "Acceptance criteria", DO10), then empty again. Now
 * plain-questions' nine keys on five sets the file names, as pairs, until its Rust test wrote them,
 * then empty again (docs/features/plain-questions.md "Acceptance criteria", PQ10).
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

  it("counts 4 project keys, 13 inbox-entry keys, 53 review keys (AC-14 of decision-staging, AC-U1 of kind-interpretation, UI-4 of rev-discipline, UI-1 of drift-queue, L1 of card-questions, IS10 of intake-similar, PQ10 of plain-questions)", () => {
    // 13 and 53 since plain-questions' `gist`, and its `matters` on the review (docs/features/plain-questions.md "Acceptance criteria", PQ10).
    expect([KEYS.Project, KEYS.InboxEntry, KEYS.Proposal].map((keys) => Object.keys(keys).length)).toEqual([4, 13, 53]);
  });

  it("gives the review document its drift right after revisions, before decided_by, and the DriftEvidence set its 3 keys (UI-1 of drift-queue)", () => {
    const review = Object.keys(KEYS.Proposal);
    expect(review.slice(review.indexOf("revisions"), review.indexOf("revisions") + 3)).toEqual(["revisions", "drift", "decided_by"]);
    expect(DRIFT_QUEUE.map((name) => served(name))).toEqual([["hash", "rows", "more"]]);
    expect(Object.keys(KEYS.InboxEntry)).not.toContain("drift");
    expect(BEFORE_HEALTH_CODE.at(-1)).toBe("DriftEvidence");
    expect(BEFORE_ROADMAP_SCREEN.at(-1)).toBe("UnboundMarker");
    expect(BEFORE_BUNDLE_BINDINGS.at(-1)).toBe("TaskRoadmap");
    expect(BEFORE_TESTED_READS.at(-1)).toBe("BindingRow");
    expect(BEFORE_HEALTH_DRIFT.at(-1)).toBe("TestedRow");
    expect(BEFORE_UI_NODE_BINDINGS.at(-1)).toBe("DriftLevels");
    expect(BEFORE_LOCK_DIFF.at(-1)).toBe("ShownBindings");
    expect(BEFORE_FLOW_READ.at(-1)).toBe("LockDiffCodeSide");
    expect(BEFORE_BOARD_UI.at(-1)).toBe("FlowInner");
    expect(BEFORE_CARD_TRACE.at(-1)).toBe("CardNotFound");
    expect(BEFORE_TRACE_VIEWS.at(-1)).toBe("CardClearGroupWrite");
    expect(BEFORE_HUB_IDENTITY.at(-1)).toBe("TraceStage");
    expect(BEFORE_HUB_TEAM.at(-1)).toBe("Identity");
    expect(BEFORE_HUB_CONFIRM.at(-1)).toBe("Team");
    expect(BEFORE_HUB_GIT.at(-1)).toBe("ConfirmRead");
    expect(BEFORE_WORKSPACES.at(-1)).toBe("HubSync");
    expect(BEFORE_CARD_TASKS.at(-1)).toBe("WorkspaceModule");
    expect(BEFORE_FLOW_SCENARIO.at(-1)).toBe("CardTask");
    expect(BEFORE_CARD_QUESTIONS.at(-1)).toBe("ShownAncestor");
    expect(BEFORE_DEPARTMENT_SPLIT.at(-1)).toBe("IntakeMatch");
    expect(BEFORE_TASK_MANDATE.at(-1)).toBe("CardSplitChild");
    expect(BEFORE_NODE_HISTORY.at(-1)).toBe("CardSubtaskItem");
    expect(BEFORE_UPSTREAM_SIGNAL.at(-1)).toBe("HistoryEntry");
    expect(BEFORE_DELIVERY_ANALYTICS.at(-1)).toBe("CardUpstreamChange");
    expect(BEFORE_DIRECTION_CATALOG.at(-1)).toBe("DeliveryMore");
    expect(BEFORE_DEPARTMENT_OVERVIEW.at(-1)).toBe("CatalogChange");
    expect(ALL.at(-1)).toBe("CardFlowCount");
  });

  it("declares in provisional.ts the review document's keys and the DriftEvidence set's, in the records' order: drift after revisions (UI-1 of drift-queue)", () => {
    expect(declaredKeys("Proposal")).toEqual(Object.keys(KEYS.Proposal));
    expect(declaredKeys("DriftEvidence")).toEqual(Object.keys(KEYS.DriftEvidence));
    expect(source).toMatch(/^  drift: DriftEvidence \| null;$/m);
    // Its rows typed as stored (StoredDriftRow, checked when drawn), a count past them (UI-1 of ui-drift-evidence).
    expect(source).toMatch(/^export interface DriftEvidence \{\n  hash: string;\n  rows: StoredDriftRow\[\];\n  more: number;\n\}$/m);
  });

  it("types a drift item's stored row as verify's 17 keys in DriftRow's order, levels and anchor alone optional (older items lack them); no key set of the daemon's: it passes rows through as stored (UI-1 of ui-drift-evidence)", () => {
    expect(declaredKeys("StoredDriftRow")).toEqual(served("DriftRow"));
    const start = source.indexOf("export interface StoredDriftRow {");
    const body = source.slice(start, source.indexOf("\n}\n", start));
    const optional = Array.from(body.matchAll(/^ {2}(\w+)\?:/gm), (match) => match[1]);
    expect(optional).toEqual(["levels", "anchor"]);
    expect(body).toMatch(/^ {2}levels\?: DriftLevels \| null;\n {2}anchor\?: BindingAnchor \| null;$/m);
    // Every other key the type shares with DriftRow, declared alike.
    const lineOf = (type: string, key: string) => {
      const at = source.indexOf(`export interface ${type} {`);
      return source.slice(at, source.indexOf("\n}\n", at)).split("\n").find((line) => line.startsWith(`  ${key}:`) || line.startsWith(`  ${key}?:`));
    };
    for (const key of served("DriftRow").filter((name) => name !== "levels" && name !== "anchor")) {
      expect([key, lineOf("StoredDriftRow", key)]).toEqual([key, lineOf("DriftRow", key)]);
    }
    const at = source.indexOf("export interface StoredDriftRow ");
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/canon/proposal-drift-item.md` "The item"');
    expect(Object.keys(KEYS)).not.toContain("StoredDriftRow");
    expect(ALL).toHaveLength(143);
  });

  it('DriftEvidence cites `docs/features/drift-queue.md` "Review, inbox, round, stage"; the review document cites it too', () => {
    for (const type of ["DriftEvidence", "Proposal"]) {
      const at = source.indexOf(`export interface ${type} `);
      expect([type, at > 0]).toEqual([type, true]);
      expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/drift-queue.md` "Review, inbox, round, stage"');
    }
  });

  it("lists ahead of the fixture only keys their types serve, none since card-questions' test wrote its three sets and three keys into the file, nor since intake-similar's Rust test wrote `similar`, nor since delivery-analytics' Rust test wrote its eleven sets, nor since direction-catalog's Rust test wrote its three, nor since department-overview's Rust test wrote its three, nor since plain-questions' Rust test wrote its nine (UI-1 of lock-diff, U1 of hub-git, docs/canon/card-flows.md \"The rule\", docs/features/card-tasks.md \"Documents\", RM-0094, DM-2 of doc-modal, DP-4 of departments, K1, L1, L3 of card-questions, IS10 of intake-similar, DA-11 of delivery-analytics, DC-9 of direction-catalog, DO10 of department-overview, PQ10 of plain-questions)", () => {
    for (const [name, key] of AHEAD_OF_FIXTURE) {
      expect([name, key, served(name).includes(key)]).toEqual([name, key, true]);
    }
    expect(LOCK_DIFF.flatMap((name) => served(name))).toHaveLength(17);
    // Empty again since node-history's Rust test wrote a shown node's `history` (docs/features/node-history.md "Acceptance criteria", NH12);
    // upstream-signal's two until its Rust test wrote them, then empty again (docs/features/upstream-signal.md "Acceptance criteria", US-12);
    // intake-similar's review document's `similar` until its Rust test wrote it, then empty again (docs/features/intake-similar.md "Acceptance criteria", IS10);
    // delivery-analytics' eleven sets, every key, until its Rust test wrote them, then empty again (docs/features/delivery-analytics.md "Acceptance criteria", DA-11);
    // direction-catalog's three sets, every key, until its Rust test wrote them, then empty again (docs/features/direction-catalog.md "Acceptance criteria", DC-9);
    // department-overview's three sets, every key, until its Rust test wrote them, then empty again (docs/features/department-overview.md "Acceptance criteria", DO10).
    // plain-questions' nine keys on five sets, as pairs, until its Rust test wrote them, then empty again (docs/features/plain-questions.md "Acceptance criteria", PQ10).
    expect(AHEAD_OF_FIXTURE.filter(([name]) => DELIVERY_ANALYTICS.includes(name))).toHaveLength(0);
    expect(AHEAD_OF_FIXTURE.filter(([name]) => DIRECTION_CATALOG.includes(name))).toHaveLength(0);
    expect(AHEAD_OF_FIXTURE.filter(([name]) => DEPARTMENT_OVERVIEW.includes(name))).toHaveLength(0);
    expect(AHEAD_OF_FIXTURE).toEqual([]);
    // A flow's display title right after its name (docs/features/departments.md "Acceptance criteria", DP-1).
    expect(served("CardFlow")).toEqual(["name", "title", "disciplines", "columns", "moves", "accept", "return"]);
  });

  it('types the workspaces read\'s three as the spec writes them: 4, 7 and 4 keys, every one present, `flows`, `module`, a module\'s `root` and `kinds` nullable, a source kept whatever its word, each citing `docs/features/workspaces.md` "Data" (DP-3, DP-4 of departments: `disciplines` after `flows`, `kinds` after `root`)', () => {
    expect(WORKSPACES.map((name) => served(name))).toEqual([
      ["source", "notes", "workspaces", "modules"],
      ["name", "title", "landing", "sections", "flows", "disciplines", "module"],
      ["name", "title", "root", "kinds"],
    ]);
    for (const name of WORKSPACES) {
      expect([name, declaredKeys(name)]).toEqual([name, served(name)]);
      const at = source.indexOf(`export interface ${name} `);
      expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/workspaces.md` "Data"');
    }
    expect(source).toMatch(/^export interface WorkspaceDocument \{\n {2}source: WorkspaceSource;\n {2}notes: string\[\];\n {2}workspaces: Workspace\[\];\n {2}modules: WorkspaceModule\[\];\n\}$/m);
    expect(source).toMatch(/^export interface Workspace \{\n {2}name: string;\n {2}title: string;\n {2}landing: string;\n {2}sections: string\[\];\n {2}flows: string\[\] \| null;\n {2}disciplines: string\[\];\n {2}module: string \| null;\n\}$/m);
    expect(source).toMatch(/^export interface WorkspaceModule \{\n {2}name: string;\n {2}title: string;\n {2}root: string \| null;\n {2}kinds: string\[\] \| null;\n\}$/m);
    expect(source).toMatch(/^export interface CardFlow \{\n {2}name: string;\n {2}title: string;\n {2}disciplines: string\[\];\n/m);
    expect(source).toMatch(/^export type KnownWorkspaceSource = "config" \| "implicit";$/m);
    expect(source).toMatch(/^export type WorkspaceSource = KnownWorkspaceSource \| Unlisted;$/m);
    expect(ALL.slice(BEFORE_WORKSPACES.length, BEFORE_CARD_TASKS.length)).toEqual([...WORKSPACES]);
  });

  it('types a card\'s linked tasks as docs/features/card-tasks.md "Documents" writes them: `tasks` after `evidence` (34 keys since card-questions\' `items` and task-mandate\'s `subtask_items` after it), a CardTask\'s four, its state a task state or null, title and stale nullable, citing `docs/canon/cards.md` "Documents and events" (CT-5)', () => {
    const card = declaredKeys("Card");
    expect(card.slice(card.indexOf("evidence"), card.indexOf("evidence") + 5)).toEqual(["evidence", "tasks", "items", "subtask_items", "acceptance"]);
    expect(card).toEqual(served("Card"));
    // 35 since card-context's `context` (docs/features/card-context.md "Acceptance criteria", CC10), 36 since upstream-signal's `upstream` (US-12).
    expect(card).toHaveLength(36);
    expect(served("CardTask")).toEqual(["id", "status", "title", "stale"]);
    expect(declaredKeys("CardTask")).toEqual(served("CardTask"));
    expect(source).toMatch(/^export interface CardTask \{\n {2}id: string;\n {2}status: TaskStatus \| null;\n {2}title: string \| null;\n {2}stale: boolean \| null;\n\}$/m);
    expect(source).toMatch(/^ {2}evidence: CardEvidence\[\];\n {2}tasks: CardTask\[\];\n {2}items: CardItem\[\];\n(?: {2}\/\*\*[^\n]*\*\/\n)? {2}subtask_items: CardSubtaskItem\[\];\n {2}acceptance: CardAcceptance \| null;$/m);
    const at = source.indexOf("export interface CardTask ");
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/canon/cards.md` "Documents and events"');
    expect(ALL.slice(BEFORE_CARD_TASKS.length, BEFORE_FLOW_SCENARIO.length)).toEqual([...CARD_TASKS]);
  });

  it('types flow-scenario\'s keys (RM-0094) as the daemon serves them: a shown node\'s `ancestors` after `omitted` (20 keys), a ShownAncestor\'s four, `id`, `kind` and `title` nullable; the flow document\'s `not_in_flow` and `types` after `omitted_edges` (13), a flow node\'s `in_flow` after `end` (16) (FS-11)', () => {
    const node = declaredKeys("ShownNode");
    expect(node.slice(node.indexOf("omitted"), node.indexOf("omitted") + 3)).toEqual(["omitted", "ancestors", "links"]);
    expect(node).toEqual(served("ShownNode"));
    // 21 since node-history's `history` (docs/features/node-history.md "Acceptance criteria", NH12).
    expect(node).toHaveLength(21);
    expect(served("ShownAncestor")).toEqual(["id", "kind", "title", "path"]);
    expect(declaredKeys("ShownAncestor")).toEqual(served("ShownAncestor"));
    expect(source).toMatch(/^export interface ShownAncestor \{\n {2}id: string \| null;\n {2}kind: string \| null;\n {2}title: string \| null;\n {2}path: string;\n\}$/m);
    expect(source).toMatch(/^ {2}ancestors: ShownAncestor\[\];\n {2}links: ShownLinks \| null;$/m);
    const view = declaredKeys("FlowView");
    expect(view.slice(view.indexOf("omitted_edges"), view.indexOf("omitted_edges") + 4)).toEqual(["omitted_edges", "not_in_flow", "types", "gaps"]);
    expect([view.length, declaredKeys("FlowNode").length]).toEqual([13, 16]);
    expect(source).toMatch(/^ {2}not_in_flow: number \| null;\n {2}types: string\[\];$/m);
    expect(source).toMatch(/^ {2}end: boolean;\n {2}in_flow: boolean;\n {2}holds: number;$/m);
    const at = source.indexOf("export interface ShownAncestor ");
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`crates/specengine-cli/README.md` "Output and the cap"');
    expect(ALL.slice(BEFORE_FLOW_SCENARIO.length, BEFORE_CARD_QUESTIONS.length)).toEqual([...FLOW_SCENARIO]);
  });

  it('types card-questions\' three as the spec writes them: a card\'s item 16 keys, its kind a plain string, its status a proposal\'s, its author an author; the intake document 7, a match 6; the review document\'s `card_id` and `why` after `interpretation`, a card\'s `items` after `tasks`, each citing `docs/features/card-questions.md` "Acceptance criteria" (K1, L1, L3 of card-questions)', () => {
    // A card's item's 18 since plain-questions' `gist` and `matters` (docs/features/plain-questions.md "Acceptance criteria", PQ10).
    expect(CARD_QUESTIONS.map((name) => served(name).length)).toEqual([18, 7, 6]);
    for (const name of CARD_QUESTIONS) {
      expect([name, declaredKeys(name)]).toEqual([name, served(name)]);
      const at = source.indexOf(`export interface ${name} `);
      expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/card-questions.md` "Acceptance criteria"');
    }
    expect(source).toMatch(/^export interface CardItem \{\n {2}id: string;\n {2}kind: string;\n {2}status: ProposalStatus;\n/m);
    expect(source).toMatch(/^ {2}why: string \| null;\n {2}author: Author;\n {2}created_at: string;$/m);
    expect(source).toMatch(/^export interface IntakeDocument \{\n {2}id: string \| null;\n {2}created: boolean;\n {2}hits: IntakeMatch\[\];\n {2}related: IntakeMatch\[\];\n/m);
    const review = declaredKeys("Proposal");
    // `similar` after `why` since intake-similar (docs/features/intake-similar.md "Acceptance criteria", IS10).
    expect(review.slice(-5)).toEqual(["interpretation", "card_id", "why", "similar", "notes"]);
    expect(source).toMatch(/^ {2}card_id: string \| null;\n(?: {2}\/\*\*[^\n]*\*\/\n)? {2}why: string \| null;$/m);
    expect(ALL.slice(BEFORE_CARD_QUESTIONS.length, BEFORE_DEPARTMENT_SPLIT.length)).toEqual([...CARD_QUESTIONS]);
    // The body of the card's raise and the stage's `as`: requests, no set.
    expect(declaredKeys("CardRaiseBody")).toEqual(["kind", "node_ids", "text", "working_answer", "price_of_other", "why", "severity", "distinct_from", "as"]);
    expect(source).toMatch(/^export type StageBody = StageChoice & \{ updated_at: string; as\?: string \| null \};$/m);
  });

  it('types department-split\'s three as the spec writes them: the split document 5 keys, its parent 8, a subtask 9 (task-mandate\'s `mandate` last), every one present; an `id` null in a plan, `parent` and `reviewer` nullable, `from` a served word kept whatever it is, each citing `docs/features/department-split.md` "Acceptance criteria" (S3, U2 of department-split)', () => {
    expect(DEPARTMENT_SPLIT.map((name) => served(name).length)).toEqual([5, 8, 9]);
    for (const name of DEPARTMENT_SPLIT) {
      expect([name, declaredKeys(name)]).toEqual([name, served(name)]);
      const at = source.indexOf(`export interface ${name} `);
      expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/department-split.md` "Acceptance criteria"');
    }
    expect(source).toMatch(/^export interface CardSplit \{\n {2}dry_run: boolean;\n {2}parent: CardSplitParent \| null;\n {2}children: CardSplitChild\[\];\n {2}reviewer: string \| null;\n {2}notes: string\[\];\n\}$/m);
    expect(source).toMatch(/^export interface CardSplitParent \{\n {2}id: string \| null;\n {2}created: boolean;\n {2}title: string;\n {2}discipline: string \| null;\n/m);
    expect(source).toMatch(/^ {2}from: SplitTargetsFrom;\n {2}suggested: boolean;\n(?: {2}\/\*\*[^\n]*\*\/\n)? {2}mandate: Mandate \| null;\n\}$/m);
    expect(source).toMatch(/^export type KnownSplitTargetsFrom = "kinds" \| "parent";$/m);
    expect(source).toMatch(/^export type SplitTargetsFrom = KnownSplitTargetsFrom \| Unlisted;$/m);
    expect(ALL.slice(BEFORE_DEPARTMENT_SPLIT.length, BEFORE_TASK_MANDATE.length)).toEqual([...DEPARTMENT_SPLIT]);
    // The body of the split's route: a request, no set (docs/features/department-split.md "Acceptance criteria", S7), its `mandate` after `child` (task-mandate, M7).
    expect(declaredKeys("CardSplitBody")).toEqual(["target", "title", "goal", "discipline", "parent", "child", "mandate", "reviewer", "dry_run", "as"]);
  });

  it('types card-context\'s key as the spec writes it: a card\'s `context` always null, after `history`, 35 keys, the type citing `docs/features/card-context.md` "Acceptance criteria" (CC10 of card-context)', () => {
    const card = declaredKeys("Card");
    expect(card.slice(card.indexOf("history"))).toEqual(["history", "context", "notes"]);
    expect(source).toMatch(/^ {2}history: CardEvent\[\] \| null;\n {2}context: null;\n {2}notes: string\[\];\n\}$/m);
    const at = source.indexOf("export interface Card {");
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/card-context.md` "Acceptance criteria", CC10');
    // 36 since upstream-signal's `upstream` (docs/features/upstream-signal.md "Acceptance criteria", US-12).
    expect(served("Card")).toHaveLength(36);
  });

  it('types intake-similar\'s key as the spec writes it: the review document\'s `similar` always null, after `why`, 51 keys then, 53 since plain-questions\' two, the key citing `docs/features/intake-similar.md` "Acceptance criteria" (IS10 of intake-similar)', () => {
    const review = declaredKeys("Proposal");
    expect(review.slice(review.indexOf("why"))).toEqual(["why", "similar", "notes"]);
    expect(source).toMatch(/^ {2}why: string \| null;\n {2}\/\*\*[^\n]*\*\/\n {2}similar: null;\n {2}\/\*\*[^\n]*\*\/\n {2}notes: string\[\];\n\}$/m);
    const at = source.indexOf("  similar: null;\n");
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/intake-similar.md` "Acceptance criteria", IS10');
    // 53 since plain-questions' `gist` and `matters` (docs/features/plain-questions.md "Acceptance criteria", PQ10).
    expect(served("Proposal")).toHaveLength(53);
    expect(Object.keys(KEYS.InboxEntry)).not.toContain("similar");
  });

  it('types task-mandate\'s keys as the spec writes them: a mandate `binding`, `open` or null, a row\'s after `reviewer` (22 keys), a card\'s after `reviewer` with `subtask_items` after `items` (34), a split\'s subtask\'s last (9); set 122 `CardSubtaskItem`, `card_id` then a card\'s item\'s 16 in their order; each citing `docs/features/task-mandate.md` "Acceptance criteria" (M1, M6, M7 of task-mandate)', () => {
    expect(source).toMatch(/^export type Mandate = "binding" \| "open";$/m);
    // A card's 35 since card-context's `context` (docs/features/card-context.md "Acceptance criteria", CC10); a row's 23 and a card's 36 since upstream-signal's `upstream` (US-12).
    // A subtask's item's 19 since plain-questions' two, a card's item's (docs/features/plain-questions.md "Acceptance criteria", PQ10).
    expect([served("CardRow").length, served("Card").length, served("CardSplitChild").length, served("CardSubtaskItem").length]).toEqual([23, 36, 9, 19]);
    const row = declaredKeys("CardRow");
    expect(row.slice(row.indexOf("reviewer"), row.indexOf("reviewer") + 3)).toEqual(["reviewer", "mandate", "estimate"]);
    const card = declaredKeys("Card");
    expect(card.slice(card.indexOf("reviewer"), card.indexOf("reviewer") + 3)).toEqual(["reviewer", "mandate", "estimate"]);
    expect(card.slice(card.indexOf("items"), card.indexOf("items") + 3)).toEqual(["items", "subtask_items", "acceptance"]);
    expect(declaredKeys("CardSplitChild").at(-1)).toBe("mandate");
    expect(served("CardSubtaskItem")).toEqual(["card_id", ...served("CardItem")]);
    expect(declaredKeys("CardSubtaskItem")).toEqual(served("CardSubtaskItem"));
    expect(source).toMatch(/^export interface CardSubtaskItem \{\n {2}card_id: string;\n {2}id: string;\n {2}kind: string;\n {2}status: ProposalStatus;\n/m);
    for (const type of ["Mandate", "CardSubtaskItem"]) {
      const at = Math.max(source.indexOf(`export interface ${type} `), source.indexOf(`export type ${type} =`));
      expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/task-mandate.md` "Acceptance criteria"');
    }
    expect(ALL.slice(BEFORE_TASK_MANDATE.length, BEFORE_NODE_HISTORY.length)).toEqual([...TASK_MANDATE]);
    // The bodies of the three writes gain one key each: requests, no set (M7).
    expect(declaredKeys("CardCreateBody").slice(-3)).toEqual(["reviewer", "mandate", "as"]);
    expect(declaredKeys("CardEditBody").slice(-4)).toEqual(["mandate", "reason", "revision", "as"]);
    expect(source).toMatch(/^ {2}mandate\?: Mandate;$/m);
    expect(source).toMatch(/^ {2}mandate\?: string\[\] \| null;$/m);
  });

  it('types upstream-signal\'s keys as the spec writes them: a row\'s `upstream` a flag or null after `correction` (23 keys), a card\'s an object or null after `correction` (36); CardUpstream\'s six and CardUpstreamChange\'s nine in the spec\'s order, a mark `suspect` or `correcting` and a source `edit` or `item`, each kept whatever its word; each citing `docs/features/upstream-signal.md` "Acceptance criteria" (US-12 of upstream-signal)', () => {
    const row = declaredKeys("CardRow");
    expect(row.slice(row.indexOf("correction"))).toEqual(["correction", "upstream"]);
    const card = declaredKeys("Card");
    expect(card.slice(card.indexOf("correction"), card.indexOf("correction") + 3)).toEqual(["correction", "upstream", "snapshot"]);
    expect([row.length, card.length]).toEqual([23, 36]);
    expect(source).toMatch(/^ {2}correction: string \| null;\n(?: {2}\/\*\*[^\n]*\*\/\n)? {2}upstream: boolean \| null;\n\}$/m);
    expect(source).toMatch(/^ {2}correction: string \| null;\n(?: {2}\/\*\*[^\n]*\*\/\n)? {2}upstream: CardUpstream \| null;\n {2}snapshot: CardFrozen\[\] \| null;$/m);
    expect(declaredKeys("CardUpstream")).toEqual(["parent", "mark", "on", "correction", "changes", "more"]);
    expect(declaredKeys("CardUpstreamChange")).toEqual(["seq", "at", "source", "by", "fields", "item", "kind", "record", "on"]);
    expect(source).toMatch(/^export interface CardUpstream \{\n {2}parent: string;\n {2}mark: KnownUpstreamMark \| Unlisted \| null;\n {2}on: string \| null;\n {2}correction: string \| null;\n {2}changes: CardUpstreamChange\[\];\n {2}more: number;\n\}$/m);
    expect(source).toMatch(
      /^export interface CardUpstreamChange \{\n {2}seq: number;\n {2}at: string;\n {2}source: KnownUpstreamSource \| Unlisted;\n {2}by: string \| null;\n {2}fields: string\[\];\n {2}item: string \| null;\n {2}kind: string \| null;\n {2}record: string \| null;\n {2}on: string \| null;\n\}$/m,
    );
    expect(source).toMatch(/^export type KnownUpstreamMark = "suspect" \| "correcting";$/m);
    expect(source).toMatch(/^export type KnownUpstreamSource = "edit" \| "item";$/m);
    for (const type of ["Card", "CardUpstream", "CardUpstreamChange", "KnownUpstreamMark", "KnownUpstreamSource"]) {
      const at = Math.max(source.indexOf(`export interface ${type} `), source.indexOf(`export type ${type} =`));
      expect([type, source.slice(source.lastIndexOf("/**", at), at).includes('`docs/features/upstream-signal.md` "Acceptance criteria", US-12')]).toEqual([type, true]);
    }
    // Ahead of the file until its Rust test wrote them, with the two sets CardUpstream and CardUpstreamChange: none of them now.
    expect(AHEAD_OF_FIXTURE.filter(([name, key]) => key === "upstream" || UPSTREAM_SIGNAL.includes(name)).map(([name, key]) => `${name}.${key}`)).toEqual([]);
  });

  it('types delivery-analytics\' eleven as the spec writes them: 8, 9, 9, 7, 2, 20, 3, 4, 4, 3, 4 keys in "JSON"\'s order, every one present, a group kept whatever its word, each citing `docs/features/delivery-analytics.md` "Acceptance criteria", DA-11, none ahead of the file since its Rust test wrote them (DA-11 of delivery-analytics)', () => {
    expect(ALL.slice(BEFORE_DELIVERY_ANALYTICS.length, BEFORE_DIRECTION_CATALOG.length)).toEqual([...DELIVERY_ANALYTICS]);
    expect(DELIVERY_ANALYTICS.map((name) => served(name).length)).toEqual([8, 9, 9, 7, 2, 20, 3, 4, 4, 3, 4]);
    for (const name of DELIVERY_ANALYTICS) {
      const type = DECLARED_AS[name] ?? name;
      expect([name, declaredKeys(type)]).toEqual([name, served(name)]);
      const at = source.indexOf(`export interface ${type} {`);
      expect([type, source.slice(source.lastIndexOf("/**", at), at).includes('`docs/features/delivery-analytics.md` "Acceptance criteria", DA-11')]).toEqual([type, true]);
      // Ahead of the file until its Rust test wrote the set: no key of it now.
      expect([name, served(name).filter((key) => isAhead(name, key))]).toEqual([name, []]);
    }
    // A release row and a department row are their own two keys, then the total's seven.
    expect(served("DeliveryRelease").slice(2)).toEqual(served("DeliveryTotal"));
    expect(served("DeliveryDepartment").slice(2)).toEqual(served("DeliveryTotal"));
    expect(source).toMatch(/^export type KnownDeliveryGroup = "release" \| "pool" \| "unplaced" \| "none";$/m);
    expect(source).toMatch(/^export type DeliveryGroup = KnownDeliveryGroup \| Unlisted;$/m);
    expect(source).toMatch(/^ {2}group: DeliveryGroup;\n {2}release: string \| null;\n {2}tasks: number;$/m);
    expect(source).toMatch(/^ {2}lead_days: LeadDays \| null;$/m);
    expect(source).toMatch(/^ {2}new: string \| null;\n {2}reason: string;$/m);
    // No key names a person, nor totals an estimate (DA-6).
    const keys = DELIVERY_ANALYTICS.flatMap((name) => served(name));
    expect(keys.filter((key) => ["as", "by", "assignee", "reviewer", "created_by", "decided_by"].includes(key))).toEqual([]);
    expect(keys.filter((key) => /^estimates?_|_estimates?$/.test(key))).toEqual([]);
  });

  it('types direction-catalog\'s three as the spec writes them: 6, 13, 6 keys in "JSON"\'s order, every one present, a row eight of the tree\'s keys then its five, a state kept whatever its word, each citing `docs/features/direction-catalog.md` "Acceptance criteria", DC-9, none ahead of the file since its Rust test wrote them (DC-9 of direction-catalog)', () => {
    expect(ALL.slice(BEFORE_DIRECTION_CATALOG.length, BEFORE_DEPARTMENT_OVERVIEW.length)).toEqual([...DIRECTION_CATALOG]);
    expect(DIRECTION_CATALOG.map((name) => served(name).length)).toEqual([6, 13, 6]);
    for (const name of DIRECTION_CATALOG) {
      const type = DECLARED_AS[name] ?? name;
      expect([name, declaredKeys(type)]).toEqual([name, served(name)]);
      const at = source.indexOf(`export interface ${type} {`);
      expect([type, source.slice(source.lastIndexOf("/**", at), at).includes('`docs/features/direction-catalog.md` "Acceptance criteria", DC-9')]).toEqual([type, true]);
      // Ahead of the file until its Rust test wrote the set: no key of it now.
      expect([name, served(name).filter((key) => isAhead(name, key))]).toEqual([name, []]);
    }
    // A row keeps eight of the tree's keys, in "JSON"'s order (`rev` before `status`), then its own five.
    expect(served("CatalogRow").slice(0, 8)).toEqual(["id", "kind", "title", "path", "line", "rev", "status", "archived"]);
    expect(served("CatalogRow").slice(0, 8).every((key) => declaredKeys("TreeNode").includes(key))).toBe(true);
    expect(source).toMatch(/^export type KnownRollup = "no card" \| "departed" \| "suspect" \| "in work" \| "confirmed";$/m);
    expect(source).toMatch(/^ {2}archived: boolean;\n {2}state: KnownRollup \| Unlisted;\n {2}open_items: number;$/m);
    expect(source).toMatch(/^ {2}tasks_open: number;\n {2}last_change: CatalogChange \| null;\n\}$/m);
    expect(source).toMatch(/^ {2}by_name: string;\n {2}commit: string;\n {2}proposal: string \| null;\n {2}of_file: boolean;\n\}$/m);
    const at = source.indexOf("export type KnownRollup ");
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/direction-catalog.md` "Acceptance criteria", DC-9');
  });

  it('types department-overview\'s three as the spec writes them: 7, 6, 5 keys in "JSON"\'s order, every one present, the document\'s `flows` the card list\'s, a word for agent tasks kept whatever it is, the agent tasks in review a list or null, no age anywhere, each citing `docs/features/department-overview.md` "Acceptance criteria", DO10, none ahead of the file since its Rust test wrote them (DO5, DO10 of department-overview)', () => {
    expect(ALL.slice(BEFORE_DEPARTMENT_OVERVIEW.length)).toEqual([...DEPARTMENT_OVERVIEW]);
    expect(DEPARTMENT_OVERVIEW.map((name) => served(name).length)).toEqual([7, 6, 5]);
    for (const name of DEPARTMENT_OVERVIEW) {
      expect([name, declaredKeys(name)]).toEqual([name, served(name)]);
      const at = source.indexOf(`export interface ${name} {`);
      expect([name, source.slice(source.lastIndexOf("/**", at), at).includes('`docs/features/department-overview.md` "Acceptance criteria", DO10')]).toEqual([name, true]);
      // Ahead of the file until its Rust test wrote the set: no key of it now.
      expect([name, served(name).filter((key) => isAhead(name, key))]).toEqual([name, []]);
    }
    // The document's `flows` is the card list's own type; its tail as every cut document's.
    expect(source).toMatch(/^export interface CardCounts \{\n {2}flows: CardFlow\[\];\n {2}columns: CardColumnCount\[\];\n {2}by_flow: CardFlowCount\[\];\n {2}agent_tasks: KnownAgentTasks \| Unlisted;\n {2}notes: string\[\];\n {2}truncated: boolean;\n {2}omitted: number \| null;\n\}$/m);
    expect(source).toMatch(/^export type KnownAgentTasks = "counted" \| "local_only";$/m);
    expect(source).toMatch(/^ {2}oldest_open_item: string \| null;\n {2}ready_parents: number;\n {2}review_agent_tasks: string\[\] \| null;\n\}$/m);
    const at = source.indexOf("export type KnownAgentTasks ");
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/department-overview.md` "Acceptance criteria", DO10');
    // No clock: the sets hold stored times and counts, never an age (DO5; M: an `oldest_open_item_days` key).
    const keys = DEPARTMENT_OVERVIEW.flatMap((name) => served(name));
    expect(keys.filter((key) => /(^|_)(age|days?)(_|$)/.test(key))).toEqual([]);
  });

  it('types the identity read\'s answer as the canon writes it: one key, a login or null, citing `docs/canon/hub.md` "Routes" (AC-12 of hub-identity)', () => {
    expect(served("Identity")).toEqual(["login"]);
    expect(declaredKeys("Identity")).toEqual(["login"]);
    expect(source).toMatch(/^export interface Identity \{\n {2}login: string \| null;\n\}$/m);
    const at = source.indexOf("export interface Identity ");
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/canon/hub.md` "Routes"');
  });

  it('types the team read\'s answer as the spec writes it: three keys, no role among them, an act kept whatever its word, citing `docs/features/hub-team.md` "Data" (A9 of hub-team)', () => {
    expect(served("Team")).toEqual(["login", "name", "allowed"]);
    expect(declaredKeys("Team")).toEqual(served("Team"));
    expect(source).toMatch(/^export interface Team \{\n {2}login: string;\n {2}name: string \| null;\n {2}allowed: TeamAct\[\];\n\}$/m);
    expect(source).toMatch(/^export type TeamAct = KnownTeamAct \| Unlisted;$/m);
    const at = source.indexOf("export interface Team ");
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/hub-team.md` "Data"');
  });

  it('types the confirm read\'s answer as the spec writes it: three keys, citing `docs/features/hub-confirm.md` "Data"; the POST\'s body the read less its `id`, closed, no choice in it (B10 of hub-confirm)', () => {
    expect(served("ConfirmRead")).toEqual(["id", "question", "updated_at"]);
    expect(declaredKeys("ConfirmRead")).toEqual(served("ConfirmRead"));
    expect(source).toMatch(/^export interface ConfirmRead \{\n {2}id: string;\n {2}question: string;\n {2}updated_at: string;\n\}$/m);
    expect(declaredKeys("ConfirmBody")).toEqual(["question", "updated_at"]);
    expect(source).toMatch(/^export interface ConfirmBody \{\n {2}question: string;\n {2}updated_at: string;\n\}$/m);
    for (const type of ["ConfirmRead", "ConfirmBody"]) {
      const at = source.indexOf(`export interface ${type} `);
      expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/hub-confirm.md` "Data"');
    }
  });

  it('types the sync read\'s answer as the spec writes it: five keys, always present, the counts and the times nullable, a state kept whatever its word, citing `docs/features/hub-git.md` "Data" (U1 of hub-git)', () => {
    expect(served("HubSync")).toEqual(["ahead", "behind", "state", "error", "at"]);
    expect(declaredKeys("HubSync")).toEqual(served("HubSync"));
    expect(source).toMatch(/^export interface HubSync \{\n {2}ahead: number \| null;\n {2}behind: number \| null;\n {2}state: HubSyncState;\n {2}error: string \| null;\n {2}at: string \| null;\n\}$/m);
    expect(source).toMatch(/^export type KnownHubSyncState = "none" \| "ok" \| "retrying" \| "held" \| "stopped";$/m);
    expect(source).toMatch(/^export type HubSyncState = KnownHubSyncState \| Unlisted;$/m);
    const at = source.indexOf("export interface HubSync ");
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/hub-git.md` "Data"');
  });

  it('gives an author an optional `login`, last, never in the file\'s four keys: skipped when none (docs/features/hub-confirm.md "Data")', () => {
    expect(declaredKeys("Author")).toEqual(["type", "role", "model", "run", "login"]);
    expect(source).toMatch(/^ {2}run: string \| null;\n {2}login\?: string;\n\}$/m);
    expect(served("Author")).toEqual(["type", "role", "model", "run"]);
    const at = source.indexOf("export interface Author ");
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/hub-confirm.md` "Data"');
  });

  it("gives the group clear's body an optional count, last: a key of a request, no set (docs/features/hub-team.md \"Data\")", () => {
    expect(declaredKeys("CardClearGroupBody")).toEqual(["group", "reason", "as", "count"]);
    expect(source).toMatch(/^ {2}as: string;\n {2}count\?: number;\n\}$/m);
  });

  it("counts lock-diff's five: 4, 4, 3, 3, 3, after ui-node-bindings' two, the seventy-seventh to eighty-first sets, declared so (UI-1 of lock-diff)", () => {
    expect(LOCK_DIFF.map((name) => served(name).length)).toEqual([4, 4, 3, 3, 3]);
    expect(ALL.slice(BEFORE_LOCK_DIFF.length, BEFORE_FLOW_READ.length)).toEqual([...LOCK_DIFF]);
    expect(ALL.indexOf("LockDiffView")).toBe(76);
    for (const name of LOCK_DIFF) {
      expect([name, declaredKeys(name)]).toEqual([name, Object.keys(KEYS[name])]);
    }
    // The document is `spec show`'s envelope: the node read's four keys, its nodes the holders.
    expect(served("LockDiffView")).toEqual(served("NodeView"));
    // The two sides differ in their first two keys only: five sets, not four.
    expect(served("LockDiffSpecSide").slice(2)).toEqual(served("LockDiffCodeSide").slice(2));
    expect(served("LockDiffSpecSide").slice(0, 2)).not.toEqual(served("LockDiffCodeSide").slice(0, 2));
  });

  it("types the five as the spec's \"UI provisional types\": rows null when not judged, an id null for an ID-less document, each side and its diff nullable, a kept row's row verify's DriftRow (UI-1 of lock-diff)", () => {
    expect(source).toMatch(/^export interface LockDiffView \{\n {2}ref: string;\n {2}reason: string \| null;\n {2}notes: string\[\];\n {2}nodes: LockDiffNode\[\];\n\}$/m);
    expect(source).toMatch(/^export interface LockDiffNode \{\n {2}id: string \| null;\n {2}path: string;\n {2}line: number;\n {2}rows: LockDiffRow\[\] \| null;\n\}$/m);
    expect(source).toMatch(/^export interface LockDiffRow \{\n {2}row: DriftRow;\n {2}spec: LockDiffSpecSide \| null;\n {2}code: LockDiffCodeSide \| null;\n\}$/m);
    expect(source).toMatch(/^export interface LockDiffSpecSide \{\n {2}commit: string;\n {2}path: string;\n {2}diff: string \| null;\n\}$/m);
    expect(source).toMatch(/^export interface LockDiffCodeSide \{\n {2}blob: string;\n {2}file: string;\n {2}diff: string \| null;\n\}$/m);
  });

  it.each<Named>(LOCK_DIFF)('%s cites `docs/features/lock-diff.md` "Data" on one line (UI-1 of lock-diff)', (type) => {
    const at = source.indexOf(`export interface ${type} `);
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at).split("\n").some((line) => line.includes('`docs/features/lock-diff.md` "Data"'))).toBe(true);
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

  it("gives the review document an interpretation after staged_at, before card-questions' two, intake-similar's one and notes, and its three keys (AC-U1 of kind-interpretation)", () => {
    expect(Object.keys(KEYS.Proposal).slice(-6)).toEqual(["staged_at", "interpretation", "card_id", "why", "similar", "notes"]);
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

  it("counts the daemon's keys of ui-live-tasks' seventeen: 2, 6, 2, 27, 4, 2, 2, 6, 2, 3, 4, 3, 5, 4, 7, 3, 4 (AC-07 of ui-live-tasks, UI-1 of roadmap-surfaces, UI of tested-reads)", () => {
    expect(UI_LIVE_TASKS.map((name) => served(name).length)).toEqual([2, 6, 2, 27, 4, 2, 2, 6, 2, 3, 4, 3, 5, 4, 7, 3, 4]);
    expect(UI_LIVE_TASKS).toHaveLength(17);
    expect(BEFORE_ROUND).toHaveLength(34);
    expect(BEFORE_HEALTH_LEFT).toHaveLength(41);
    expect(BEFORE_REV_DISCIPLINE).toHaveLength(44);
    expect(BEFORE_DRIFT_QUEUE).toHaveLength(45);
    expect(BEFORE_HEALTH_CODE).toHaveLength(46);
    expect(BEFORE_ROADMAP_SCREEN).toHaveLength(56);
    expect(BEFORE_BUNDLE_BINDINGS).toHaveLength(66);
    expect(BEFORE_TESTED_READS).toHaveLength(67);
    expect(BEFORE_HEALTH_DRIFT).toHaveLength(68);
    expect(BEFORE_UI_NODE_BINDINGS).toHaveLength(74);
    expect(BEFORE_LOCK_DIFF).toHaveLength(76);
    expect(BEFORE_FLOW_READ).toHaveLength(81);
    expect(BEFORE_BOARD_UI).toHaveLength(86);
    expect(BEFORE_CARD_TRACE).toHaveLength(98);
    expect(BEFORE_TRACE_VIEWS).toHaveLength(105);
    expect(BEFORE_HUB_IDENTITY).toHaveLength(106);
    expect(BEFORE_HUB_TEAM).toHaveLength(107);
    expect(BEFORE_HUB_CONFIRM).toHaveLength(108);
    expect(BEFORE_HUB_GIT).toHaveLength(109);
    expect(BEFORE_WORKSPACES).toHaveLength(110);
    expect(BEFORE_CARD_TASKS).toHaveLength(113);
    expect(BEFORE_FLOW_SCENARIO).toHaveLength(114);
    expect(BEFORE_CARD_QUESTIONS).toHaveLength(115);
    expect(BEFORE_DEPARTMENT_SPLIT).toHaveLength(118);
    expect(BEFORE_TASK_MANDATE).toHaveLength(121);
    expect(BEFORE_NODE_HISTORY).toHaveLength(122);
    expect(BEFORE_UPSTREAM_SIGNAL).toHaveLength(124);
    expect(BEFORE_DELIVERY_ANALYTICS).toHaveLength(126);
    expect(BEFORE_DIRECTION_CATALOG).toHaveLength(137);
    expect(BEFORE_DEPARTMENT_OVERVIEW).toHaveLength(140);
    expect(ALL).toHaveLength(143);
    expect(new Set(ALL).size).toBe(143);
  });

  it("gives the package its roadmap right after bundle, before author, declared in that order: a key, no set (UI-1 of roadmap-surfaces)", () => {
    const pkg = Object.keys(KEYS.TaskPackage);
    expect(pkg.slice(pkg.indexOf("bundle"), pkg.indexOf("bundle") + 3)).toEqual(["bundle", "roadmap", "author"]);
    expect(declaredKeys("TaskPackage")).toEqual(pkg);
    expect(source).toMatch(/^  roadmap: TaskRoadmap \| null;$/m);
    // The TaskRoadmap set came with 3a's roadmap document (docs/features/roadmap-screen.md UI-1): the ten names.
    expect(NAMES.filter((name) => name.includes("Roadmap"))).toEqual([...ROADMAP_SCREEN]);
  });

  it("counts bundle-bindings' one: a package row's 14 keys in the canon's order, declared so, the sixty-seventh set; the package's bindings stays its 16th key (UI-1 of bundle-bindings)", () => {
    expect(BUNDLE_BINDINGS.map((name) => served(name).length)).toEqual([14]);
    expect(BEFORE_TESTED_READS.slice(BEFORE_BUNDLE_BINDINGS.length)).toEqual([...BUNDLE_BINDINGS]);
    expect(declaredKeys("BindingRow")).toEqual(Object.keys(KEYS.BindingRow));
    expect(served("TaskPackage").indexOf("bindings")).toBe(15);
    expect(source).toMatch(/^ {2}bindings: BindingRow\[\];$/m);
  });

  it("counts tested-reads' one: an entry's 7 keys in the canon's order, declared so, the sixty-eighth set; the package's tested its 17th of 27 keys, right after bindings, declared so (UI of tested-reads)", () => {
    expect(TESTED_READS.map((name) => served(name))).toEqual([["node", "state", "category", "rev", "node_rev", "commit", "verified_at"]]);
    expect(ALL.slice(BEFORE_TESTED_READS.length, BEFORE_HEALTH_DRIFT.length)).toEqual([...TESTED_READS]);
    expect(declaredKeys("TestedRow")).toEqual(Object.keys(KEYS.TestedRow));
    const pkg = served("TaskPackage");
    expect([pkg.indexOf("tested"), pkg.length]).toEqual([16, 27]);
    expect(pkg.slice(pkg.indexOf("bindings"), pkg.indexOf("bindings") + 3)).toEqual(["bindings", "tested", "spec_snapshot"]);
    expect(declaredKeys("TaskPackage")).toEqual(pkg);
    expect(source).toMatch(/^ {2}tested: TestedRow\[\] \| null;$/m);
  });

  it('TestedRow cites `docs/canon/spec-lock-tested.md` "Reads" (UI of tested-reads)', () => {
    const at = source.indexOf("export interface TestedRow ");
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/canon/spec-lock-tested.md` "Reads"');
  });

  it("counts ui-node-bindings' two: a shown node's 20 keys (flow-scenario's `ancestors` after `omitted`), bindings its last after links, and ShownBindings' 3, declared so, the seventy-fifth and seventy-sixth sets (UI-8 of ui-node-bindings)", () => {
    // A shown node's 21 since node-history's `history` (docs/features/node-history.md "Acceptance criteria", NH12).
    expect(UI_NODE_BINDINGS.map((name) => served(name).length)).toEqual([21, 3]);
    expect(ALL.slice(BEFORE_UI_NODE_BINDINGS.length, BEFORE_LOCK_DIFF.length)).toEqual([...UI_NODE_BINDINGS]);
    expect(declaredKeys("ShownNode")).toEqual(Object.keys(KEYS.ShownNode));
    expect(declaredKeys("ShownBindings")).toEqual(Object.keys(KEYS.ShownBindings));
    // `history` last, after `bindings` (docs/features/node-history.md "Acceptance criteria", NH12).
    expect(served("ShownNode").slice(-3)).toEqual(["links", "bindings", "history"]);
    expect(source).toMatch(/^ {2}links: ShownLinks \| null;\n(?: {2}\/\*\*[^]*?\*\/\n)? {2}bindings: ShownBindings \| null;\n(?: {2}\/\*\*[^]*?\*\/\n)? {2}history: ShownHistory \| null;\n\}$/m);
    expect(source).toMatch(/^export interface ShownBindings \{\n {2}rows: BindingRow\[\];\n {2}omitted: number;\n {2}tested: TestedRow\[\] \| null;\n\}$/m);
  });

  it('ShownBindings cites `docs/canon/code-reads.md` "spec show --bindings"; ShownNode `crates/specengine-cli/README.md` "Output and the cap" (UI-8 of ui-node-bindings)', () => {
    for (const [type, citation] of [
      ["ShownBindings", '`docs/canon/code-reads.md` "spec show --bindings"'],
      ["ShownNode", '`crates/specengine-cli/README.md` "Output and the cap"'],
    ] as const) {
      const at = source.indexOf(`export interface ${type} `);
      expect([type, at > 0]).toEqual([type, true]);
      expect(source.slice(source.lastIndexOf("/**", at), at)).toContain(citation);
    }
  });

  it('BindingRow cites `docs/canon/code-reads.md` "The binding row" (UI-1 of bundle-bindings)', () => {
    const at = source.indexOf("export interface BindingRow ");
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/canon/code-reads.md` "The binding row"');
  });

  it("counts roadmap-screen's ten: 15, 2, 4, 3, 8, 11, 2, 2, 4, 7, after health-code's (UI-1 of roadmap-screen)", () => {
    expect(ROADMAP_SCREEN.map((name) => served(name).length)).toEqual([15, 2, 4, 3, 8, 11, 2, 2, 4, 7]);
    expect(BEFORE_BUNDLE_BINDINGS.slice(BEFORE_ROADMAP_SCREEN.length)).toEqual([...ROADMAP_SCREEN]);
    expect(served("RoadmapItem").slice(-2)).toEqual(["cycle", "tasks"]);
    expect(served("RoadmapCut").slice(1)).toEqual(served("RoadmapSum"));
  });

  it.each<Named>(ROADMAP_SCREEN)("%s declares its keys in provisional.ts in the record's order (UI-1 of roadmap-screen)", (type) => {
    expect(declaredKeys(type)).toEqual(Object.keys(KEYS[type]));
  });

  it.each<Named>(ROADMAP_SCREEN.filter((name) => name !== "TaskRoadmap"))('%s cites `docs/canon/spec-roadmap.md` "Command"; TaskRoadmap keeps the package\'s citation (UI-1 of roadmap-screen)', (type) => {
    const at = source.indexOf(`export interface ${type} `);
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/canon/spec-roadmap.md` "Command"');
    const own = source.indexOf("export interface TaskRoadmap ");
    expect(source.slice(source.lastIndexOf("/**", own), own)).toContain('`docs/canon/task-package.md` "Package"');
  });

  it("counts health-left's three: 3 document keys (7 since health-code's three and health-drift's one follow them), 6 amendment keys, 9 link keys (11 since doc-modal's two, DM-2) (AC-U2 of health-left)", () => {
    expect(HEALTH_LEFT.map((name) => served(name).length)).toEqual([7, 6, 11]);
    // The other end's title and kind right after `name` (docs/features/doc-modal.md "Acceptance criteria", DM-1).
    expect(served("ShownLink")).toEqual(["type", "origin", "at", "name", "title", "end_kind", "written", "path", "line", "state", "reason"]);
    expect(served("HealthReport").slice(0, 3)).toEqual(["documents", "amendments", "notes"]);
    expect(served("UnappliedAmendment")).toEqual(["path", "id", "title", "class", "status", "links"]);
  });

  it.each(["HealthReport", "UnappliedAmendment"])('%s cites `docs/features/health-left.md` "Data" (AC-U2 of health-left)', (type) => {
    const at = source.indexOf(`export interface ${type} `);
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/health-left.md` "Data"');
  });

  it("gives the health document code, assumes, unbound after notes, and counts health-code's ten: 4, 4, 4, 10, 8, 6, 6, 4, 8, 7 (UI-1 of health-code)", () => {
    expect(served("HealthReport").slice(0, 6)).toEqual(["documents", "amendments", "notes", "code", "assumes", "unbound"]);
    expect(HEALTH_CODE.map((name) => served(name).length)).toEqual([4, 4, 4, 10, 8, 6, 6, 4, 8, 7]);
    expect(served("HealthAssumes")).toEqual(served("HealthUnbound"));
  });

  it.each<Named>(["HealthReport", ...HEALTH_CODE])('%s cites `docs/features/health-code.md` "Data" and declares its keys in the record order (UI-1 of health-code)', (type) => {
    const at = source.indexOf(`export interface ${type} `);
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/health-code.md` "Data"');
    expect(declaredKeys(type)).toEqual(Object.keys(KEYS[type]));
  });

  it("gives the health document drift after unbound, its seventh and last key, and counts health-drift's six: 6, 7, 17, 2, 3, 3, before ui-node-bindings' two (UI-1 of health-drift)", () => {
    expect(served("HealthReport")).toEqual(["documents", "amendments", "notes", "code", "assumes", "unbound", "drift"]);
    expect(HEALTH_DRIFT.map((name) => served(name).length)).toEqual([6, 7, 17, 2, 3, 3]);
    expect(ALL.slice(BEFORE_HEALTH_DRIFT.length, BEFORE_UI_NODE_BINDINGS.length)).toEqual([...HEALTH_DRIFT]);
    // Row 6 is a code row (state, causes, counts, rows) with verify's tested and its own notes.
    expect(served("HealthDrift")).toEqual([...served("HealthAssumes"), "tested", "notes"]);
    expect(source).toMatch(/^ {2}unbound: HealthUnbound;\n {2}drift: HealthDrift;\n\}$/m);
  });

  it("types row 6's row as verify's 17 keys: the binding row's key, then state, category, detail; its anchor the binding row's, its tested TestedRow (UI-1 of health-drift)", () => {
    expect(served("DriftRow").slice(0, 6)).toEqual(served("BindingRow").slice(0, 6));
    expect(served("DriftRow").slice(6, 9)).toEqual(["state", "category", "detail"]);
    expect(served("DriftRow").at(-1)).toBe("anchor");
    expect(source).toMatch(/^ {2}anchor: BindingAnchor \| null;\n\}$/m);
    expect(source).toMatch(/^ {2}tested: TestedRow\[\] \| null;\n {2}notes: string\[\];\n\}$/m);
    expect(served("DriftLock")).toEqual(["rev", "commit", "verified_at"]);
  });

  it.each<Named>(["HealthReport", ...HEALTH_DRIFT])('%s cites `docs/features/health-drift.md` "Data" on one line and declares its keys in the record order (UI-1 of health-drift)', (type) => {
    const at = source.indexOf(`export interface ${type} `);
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at).split("\n").some((line) => line.includes('`docs/features/health-drift.md` "Data"'))).toBe(true);
    expect(declaredKeys(type)).toEqual(Object.keys(KEYS[type]));
  });

  it('ShownLink cites `docs/canon/spec-cli-graph.md` "spec show --links"', () => {
    const at = source.indexOf("export interface ShownLink ");
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/canon/spec-cli-graph.md` "spec show --links"');
  });

  it('types plain-questions\' keys as the spec writes them: `gist` and `matters` after `summary`, each `string | null`, on the review document, the round item, a card\'s item and a subtask\'s; the inbox entry\'s `gist` alone; each type citing `docs/features/plain-questions.md` "Acceptance criteria" (PQ10)', () => {
    for (const type of ["Proposal", "RoundItem", "CardItem", "CardSubtaskItem"] as const) {
      const keys = declaredKeys(type);
      expect([type, keys.slice(keys.indexOf("summary"), keys.indexOf("summary") + 3)]).toEqual([type, ["summary", "gist", "matters"]]);
      const at = source.indexOf(`export interface ${type} {`);
      const body = source.slice(at, source.indexOf("\n}\n", at));
      expect([type, /^ {2}gist: string \| null;$/m.test(body), /^ {2}matters: string \| null;$/m.test(body)]).toEqual([type, true, true]);
      expect([type, source.slice(source.lastIndexOf("/**", at), at).includes('`docs/features/plain-questions.md` "Acceptance criteria"')]).toEqual([type, true]);
    }
    const entry = declaredKeys("InboxEntry");
    expect(entry.slice(entry.indexOf("summary"), entry.indexOf("summary") + 3)).toEqual(["summary", "gist", "record_id"]);
    expect(entry).not.toContain("matters");
    expect(PLAIN_QUESTIONS.map(([name, key]) => `${name}.${key}`)).toEqual([
      "Proposal.gist",
      "Proposal.matters",
      "InboxEntry.gist",
      "RoundItem.gist",
      "RoundItem.matters",
      "CardItem.gist",
      "CardItem.matters",
      "CardSubtaskItem.gist",
      "CardSubtaskItem.matters",
    ]);
    // No set is added: the five are the file's already.
    expect(ALL).toEqual(expect.arrayContaining(["Proposal", "InboxEntry", "RoundItem", "CardItem", "CardSubtaskItem"]));
  });

  it("counts ui-round's six: 5 sheet keys, 21 item keys, 3, 3; 3 answers-document keys, 6 entry keys (T-04 of ui-round, PQ10 of plain-questions)", () => {
    // An item's 21 since plain-questions' `gist` and `matters` (docs/features/plain-questions.md "Acceptance criteria", PQ10).
    expect(UI_ROUND.map((name) => served(name).length)).toEqual([5, 21, 3, 3, 3, 6]);
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
      "Author.login",
    ]);
    for (const [name, key] of NEVER_SERVED) {
      expect([name, key, CITED[name].split(", ").includes(`${key}?`)]).toEqual([name, key, true]);
    }
  });

  it("names no task in an inbox entry; the review document's task_id sits after choice, then the stage's two, the interpretation, the card and why, similar, notes", () => {
    expect(Object.keys(KEYS.InboxEntry)).not.toContain("task_id");
    expect(Object.keys(KEYS.InboxEntry)).not.toContain("card_id");
    expect(Object.keys(KEYS.InboxEntry)).not.toContain("similar");
    expect(Object.keys(KEYS.Proposal).slice(-9)).toEqual(["choice", "task_id", "staged", "staged_at", "interpretation", "card_id", "why", "similar", "notes"]);
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

  /** A type's keys as `provisional.ts` declares them, less a key ahead of the file it does not name yet. */
  function declaredNow(name: Named): string[] {
    const file = sets[name] ?? [];
    return declaredKeys(name).filter((key) => file.includes(key) || !isAhead(name, key));
  }

  /** The types the file is compared with: all, less a set ahead of the file (every key ahead) it does not name yet. */
  const expected = ALL.filter((name) => Object.hasOwn(sets, name) || !served(name).every((key) => isAhead(name, key)));

  it("names exactly the one hundred and forty-three types: daemon-read's six, ui-live's nine, ui-live-tasks' seventeen, decision-staging's two, ui-round's six, kind-interpretation's one, health-left's three, rev-discipline's one, drift-queue's one, health-code's ten, roadmap-screen's ten, bundle-bindings' one, tested-reads' one, health-drift's six, ui-node-bindings' two, lock-diff's five, flow-read's five, board-ui's twelve, card-trace's seven, trace-views' one, hub-identity's one, hub-team's one, hub-confirm's one, hub-git's one, workspaces' three, card-tasks' one, flow-scenario's one, card-questions' three, department-split's three, task-mandate's one, node-history's two, upstream-signal's two, delivery-analytics' eleven, direction-catalog's three, department-overview's three", () => {
    expect(Object.keys(sets).sort()).toEqual([...expected].sort());
    // delivery-analytics' eleven since its Rust test wrote them (DA-11); direction-catalog's three
    // since its own did (DC-9); department-overview's three since its own did (DO10): it names
    // all one hundred and forty-three, in the slices' order.
    expect(expected).toEqual(ALL);
    expect(Object.keys(sets)).toEqual([...ALL]);
    expect(Object.keys(sets)).toHaveLength(143);
  });

  it("names department-split's three after IntakeMatch, sets 119 to 121 with their keys in the types' order: 5, 8, 9 (S3, U2 of department-split; a subtask's `mandate` last, M2 of task-mandate)", () => {
    expect(Object.keys(sets).slice(BEFORE_DEPARTMENT_SPLIT.length, BEFORE_TASK_MANDATE.length)).toEqual([...DEPARTMENT_SPLIT]);
    expect(Object.keys(sets).indexOf("CardSplit")).toBe(118);
    expect(Object.keys(sets).at(BEFORE_TASK_MANDATE.length - 1)).toBe("CardSplitChild");
    expect(DEPARTMENT_SPLIT.map((name) => sets[name])).toEqual(DEPARTMENT_SPLIT.map((name) => served(name)));
    expect(DEPARTMENT_SPLIT.map((name) => sets[name]?.length)).toEqual([5, 8, 9]);
    expect(DEPARTMENT_SPLIT.map((name) => sets[name])).toEqual(DEPARTMENT_SPLIT.map((name) => declaredKeys(name)));
  });

  it("names task-mandate's one last, after CardSplitChild, set 122 `CardSubtaskItem`: `card_id`, then a card's item's 18; a row's `mandate` after `reviewer` (22), a card's after `reviewer` and its `subtask_items` after `items` (34) (M6, M7 of task-mandate)", () => {
    expect(Object.keys(sets).slice(BEFORE_TASK_MANDATE.length, BEFORE_NODE_HISTORY.length)).toEqual([...TASK_MANDATE]);
    expect(Object.keys(sets).indexOf("CardSubtaskItem")).toBe(121);
    expect(Object.keys(sets).at(BEFORE_NODE_HISTORY.length - 1)).toBe("CardSubtaskItem");
    expect(sets.CardSubtaskItem).toEqual(["card_id", ...(sets.CardItem ?? [])]);
    expect(sets.CardSubtaskItem).toEqual(declaredNow("CardSubtaskItem"));
    for (const [name, key, after] of [
      ["CardRow", "mandate", "reviewer"],
      ["Card", "mandate", "reviewer"],
      ["Card", "subtask_items", "items"],
    ] as const) {
      const file = sets[name] ?? [];
      expect([name, key, file[file.indexOf(after) + 1]]).toEqual([name, key, key]);
    }
    // 35 since card-context's `context` (docs/features/card-context.md "Acceptance criteria", CC10); 23
    // and 36 since upstream-signal's `upstream` (docs/features/upstream-signal.md "Acceptance criteria", US-12).
    expect([sets.CardRow?.length, sets.Card?.length, sets.CardSplitChild?.at(-1)]).toEqual([23, 36, "mandate"]);
    expect([sets.CardRow, sets.Card]).toEqual([declaredNow("CardRow"), declaredNow("Card")]);
  });

  it("names card-questions' three after ShownAncestor, before department-split's three, sets 116 to 118 with their keys in the types' order: 18, 7, 6 (K1, L3 of card-questions; PQ10 of plain-questions)", () => {
    expect(Object.keys(sets).slice(BEFORE_CARD_QUESTIONS.length, BEFORE_DEPARTMENT_SPLIT.length)).toEqual([...CARD_QUESTIONS]);
    expect(Object.keys(sets).indexOf("CardItem")).toBe(115);
    expect(Object.keys(sets).at(BEFORE_DEPARTMENT_SPLIT.length - 1)).toBe("IntakeMatch");
    expect(CARD_QUESTIONS.map((name) => sets[name])).toEqual(CARD_QUESTIONS.map((name) => compared(name)));
    // 18 since plain-questions' Rust test wrote its `gist` and `matters` (docs/features/plain-questions.md "Acceptance criteria", PQ10).
    expect(CARD_QUESTIONS.map((name) => sets[name]?.length)).toEqual([18, 7, 6]);
    expect(CARD_QUESTIONS.map((name) => sets[name])).toEqual(CARD_QUESTIONS.map((name) => declaredNow(name)));
  });

  it('names flow-scenario\'s one after CardTask, before card-questions\' three, set 115 `ShownAncestor` with its four keys in the types\' order; a shown node\'s `ancestors` after `omitted` (20), the flow document\'s 13 keys and a flow node\'s 16 (RM-0094, FS-11)', () => {
    expect(Object.keys(sets).slice(BEFORE_FLOW_SCENARIO.length, BEFORE_CARD_QUESTIONS.length)).toEqual([...FLOW_SCENARIO]);
    expect(Object.keys(sets).at(BEFORE_CARD_QUESTIONS.length - 1)).toBe("ShownAncestor");
    expect(Object.keys(sets).indexOf("ShownAncestor")).toBe(114);
    expect(sets.ShownAncestor).toEqual(["id", "kind", "title", "path"]);
    expect(sets.ShownAncestor).toEqual(declaredKeys("ShownAncestor"));
    const node = sets.ShownNode ?? [];
    expect(node.slice(node.indexOf("omitted"), node.indexOf("omitted") + 3)).toEqual(["omitted", "ancestors", "links"]);
    expect(node).toEqual(declaredNow("ShownNode"));
    expect([sets.FlowView?.length, sets.FlowNode?.length]).toEqual([13, 16]);
    expect(sets.FlowView).toEqual(declaredKeys("FlowView"));
    expect(sets.FlowNode).toEqual(declaredKeys("FlowNode"));
  });

  it('names card-tasks\' one after WorkspaceModule, before flow-scenario\'s one, set 114 `CardTask` with its four keys in the types\' order; a card\'s `tasks` right after `evidence`, 34 keys since card-questions\' `items` and task-mandate\'s two (docs/features/card-tasks.md "Documents")', () => {
    expect(Object.keys(sets).slice(BEFORE_CARD_TASKS.length, BEFORE_FLOW_SCENARIO.length)).toEqual([...CARD_TASKS]);
    expect(Object.keys(sets).at(BEFORE_FLOW_SCENARIO.length - 1)).toBe("CardTask");
    expect(Object.keys(sets).indexOf("CardTask")).toBe(113);
    expect(sets.CardTask).toEqual(["id", "status", "title", "stale"]);
    expect(sets.CardTask).toEqual(declaredKeys("CardTask"));
    // A card's `items` right after `tasks` since card-questions (docs/features/card-questions.md "Acceptance criteria", K1).
    const card = sets.Card ?? [];
    expect(card.slice(card.indexOf("evidence"), card.indexOf("evidence") + 5)).toEqual(["evidence", "tasks", "items", "subtask_items", "acceptance"]);
    expect(card).toEqual(declaredNow("Card"));
    // 35 since card-context's `context` (docs/features/card-context.md "Acceptance criteria", CC10); 36
    // since upstream-signal's `upstream` (docs/features/upstream-signal.md "Acceptance criteria", US-12).
    expect(card).toHaveLength(36);
  });

  it('names workspaces\' three after HubSync, before card-tasks\' one, sets 111 to 113, each with its keys in the types\' order: 4, 7, 4 (docs/features/workspaces.md "Data"; DP-4 of departments)', () => {
    expect(Object.keys(sets).slice(BEFORE_WORKSPACES.length, BEFORE_CARD_TASKS.length)).toEqual([...WORKSPACES]);
    expect(Object.keys(sets).at(BEFORE_CARD_TASKS.length - 1)).toBe("WorkspaceModule");
    expect(Object.keys(sets).indexOf("WorkspaceDocument")).toBe(110);
    expect(WORKSPACES.map((name) => sets[name])).toEqual(WORKSPACES.map((name) => served(name)));
    expect(WORKSPACES.map((name) => sets[name]?.length)).toEqual([4, 7, 4]);
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
    // 50 since card-questions' `card_id` and `why` (L1 of card-questions); 51 once the file names
    // intake-similar's `similar` (docs/features/intake-similar.md "Acceptance criteria", IS10).
    const review = sets.Proposal ?? [];
    expect(review).toEqual(compared("Proposal"));
    // 53 since plain-questions' Rust test wrote its `gist` and `matters` (docs/features/plain-questions.md "Acceptance criteria", PQ10).
    expect(review).toHaveLength(53);
    expect(review.slice(review.indexOf("revisions"), review.indexOf("revisions") + 3)).toEqual(["revisions", "drift", "decided_by"]);
    expect(sets.DriftEvidence).toEqual(["hash", "rows", "more"]);
    expect(Object.keys(sets).indexOf("DriftEvidence")).toBe(BEFORE_DRIFT_QUEUE.length);
    expect(Object.keys(sets).indexOf("DriftEvidence")).toBe(45);
    expect(Object.keys(sets).at(BEFORE_ROADMAP_SCREEN.length - 1)).toBe("UnboundMarker");
    expect(Object.keys(sets).at(BEFORE_BUNDLE_BINDINGS.length - 1)).toBe("TaskRoadmap");
    expect(sets.InboxEntry).not.toContain("drift");
  });

  it("names the package's 27 keys in the types' order: roadmap after bundle, before author; tested after bindings (UI-1 of roadmap-surfaces, UI of tested-reads)", () => {
    expect(sets.TaskPackage).toEqual(served("TaskPackage"));
    expect(sets.TaskPackage).toHaveLength(27);
  });

  it("names health-left's keys in the document's order: the health document's three, then health-code's three, then health-drift's drift, an amendment's six (AC-T2; UI-1 of health-code, of health-drift)", () => {
    expect(sets.HealthReport).toEqual(["documents", "amendments", "notes", "code", "assumes", "unbound", "drift"]);
    expect(sets.UnappliedAmendment).toEqual(["path", "id", "title", "class", "status", "links"]);
  });

  it("names health-code's ten after DriftEvidence, in the types' order and with their keys (UI-1, AC-9 of health-code)", () => {
    expect(Object.keys(sets).slice(BEFORE_HEALTH_CODE.length, BEFORE_ROADMAP_SCREEN.length)).toEqual([...HEALTH_CODE]);
    expect(HEALTH_CODE.map((name) => sets[name])).toEqual(HEALTH_CODE.map((name) => served(name)));
  });

  it("names roadmap-screen's ten after UnboundMarker, before bundle-bindings' one, in the types' order and with their keys (UI-1, TE-1 of roadmap-screen)", () => {
    expect(Object.keys(sets).slice(BEFORE_ROADMAP_SCREEN.length, BEFORE_BUNDLE_BINDINGS.length)).toEqual([...ROADMAP_SCREEN]);
    expect(ROADMAP_SCREEN.map((name) => sets[name])).toEqual(ROADMAP_SCREEN.map((name) => served(name)));
    expect(ROADMAP_SCREEN.map((name) => sets[name]?.length)).toEqual([15, 2, 4, 3, 8, 11, 2, 2, 4, 7]);
  });

  it("names bundle-bindings' BindingRow after TaskRoadmap, the sixty-seventh, with a package row's 14 keys in the types' order (UI-1, RD-8 of bundle-bindings)", () => {
    expect(Object.keys(sets).at(BEFORE_TESTED_READS.length - 1)).toBe("BindingRow");
    expect(sets.BindingRow).toEqual(served("BindingRow"));
  });

  it("names tested-reads' TestedRow after BindingRow, the sixty-eighth, with an entry's 7 keys in the types' order, and the package's tested right after bindings, its 17th (UI, TE-5 of tested-reads)", () => {
    expect(Object.keys(sets).slice(BEFORE_TESTED_READS.length)).toEqual(expected.slice(BEFORE_TESTED_READS.length));
    expect(Object.keys(sets).at(BEFORE_HEALTH_DRIFT.length - 1)).toBe("TestedRow");
    expect(sets.TestedRow ?? []).toEqual(compared("TestedRow"));
    expect(sets.TestedRow).toEqual(["node", "state", "category", "rev", "node_rev", "commit", "verified_at"]);
    const pkg = sets.TaskPackage ?? [];
    expect(pkg.slice(pkg.indexOf("bindings"), pkg.indexOf("bindings") + 2)).toEqual(["bindings", "tested"]);
    expect(pkg.indexOf("tested")).toBe(16);
  });

  it("names health-drift's six after TestedRow, before ui-node-bindings' two, in the types' order and with their keys: 6, 7, 17, 2, 3, 3; the health document's drift its 7th, after unbound (UI-1, TE-5 of health-drift)", () => {
    expect(Object.keys(sets).slice(BEFORE_HEALTH_DRIFT.length, BEFORE_UI_NODE_BINDINGS.length)).toEqual([...HEALTH_DRIFT]);
    expect(Object.keys(sets).at(BEFORE_UI_NODE_BINDINGS.length - 1)).toBe("DriftLevels");
    expect(HEALTH_DRIFT.map((name) => sets[name])).toEqual(HEALTH_DRIFT.map((name) => served(name)));
    expect(HEALTH_DRIFT.map((name) => sets[name]?.length)).toEqual([6, 7, 17, 2, 3, 3]);
    const health = sets.HealthReport ?? [];
    expect([health.indexOf("drift"), health.length]).toEqual([6, 7]);
  });

  it("names ui-node-bindings' two after DriftLevels, before lock-diff's five: a shown node's 20 keys, bindings after links, and ShownBindings' 3, in the types' order (UI-8, TE-1 of ui-node-bindings, flow-scenario's `ancestors`)", () => {
    expect(Object.keys(sets).slice(BEFORE_UI_NODE_BINDINGS.length, BEFORE_LOCK_DIFF.length)).toEqual([...UI_NODE_BINDINGS]);
    expect(Object.keys(sets).indexOf("ShownNode")).toBe(74);
    expect(Object.keys(sets).at(BEFORE_LOCK_DIFF.length - 1)).toBe("ShownBindings");
    expect(UI_NODE_BINDINGS.map((name) => sets[name])).toEqual(UI_NODE_BINDINGS.map((name) => compared(name)));
    expect(sets.ShownBindings).toEqual(["rows", "omitted", "tested"]);
    const node = sets.ShownNode ?? [];
    // docs/features/node-history.md "Acceptance criteria", NH12: 21 keys, `history` after `bindings`.
    expect([node.indexOf("bindings"), node.length]).toEqual([19, 21]);
    expect(node.slice(-2)).toEqual(["bindings", "history"]);
  });

  it("names node-history's two after CardSubtaskItem, sets 123 and 124: a shown node's history's 3 keys and its entry's 15, in the types' order (NH12 of node-history)", () => {
    expect(Object.keys(sets).slice(BEFORE_NODE_HISTORY.length, BEFORE_UPSTREAM_SIGNAL.length)).toEqual([...NODE_HISTORY]);
    expect(Object.keys(sets).indexOf("ShownHistory")).toBe(122);
    expect(Object.keys(sets).at(BEFORE_UPSTREAM_SIGNAL.length - 1)).toBe("HistoryEntry");
    expect(NODE_HISTORY.map((name) => sets[name])).toEqual(NODE_HISTORY.map((name) => served(name)));
    expect(NODE_HISTORY.map((name) => sets[name])).toEqual(NODE_HISTORY.map((name) => declaredKeys(name)));
    expect(NODE_HISTORY.map((name) => sets[name]?.length)).toEqual([3, 15]);
  });

  it("names upstream-signal's two after HistoryEntry, sets 125 and 126: what changed above a subtask's 6 keys and a change's 9, in the types' order; a row's and a card's `upstream` right after `correction`, 23 and 36 keys (US-12 of upstream-signal)", () => {
    expect(Object.keys(sets).slice(BEFORE_UPSTREAM_SIGNAL.length, BEFORE_DELIVERY_ANALYTICS.length)).toEqual([...UPSTREAM_SIGNAL]);
    expect(Object.keys(sets).indexOf("CardUpstream")).toBe(124);
    expect(Object.keys(sets).at(BEFORE_DELIVERY_ANALYTICS.length - 1)).toBe("CardUpstreamChange");
    expect(UPSTREAM_SIGNAL.map((name) => sets[name])).toEqual(UPSTREAM_SIGNAL.map((name) => served(name)));
    expect(UPSTREAM_SIGNAL.map((name) => sets[name])).toEqual(UPSTREAM_SIGNAL.map((name) => declaredKeys(name)));
    expect(UPSTREAM_SIGNAL.map((name) => sets[name]?.length)).toEqual([6, 9]);
    for (const name of ["CardRow", "Card"] as const) {
      const file = sets[name] ?? [];
      expect([name, file[file.indexOf("correction") + 1]]).toEqual([name, "upstream"]);
      expect([name, file]).toEqual([name, declaredKeys(name)]);
    }
  });

  it("names delivery-analytics' eleven after CardUpstreamChange, before direction-catalog's three, sets 127 to 137: 8, 9, 9, 7, 2, 20, 3, 4, 4, 3, 4 keys in the types' order, `Delivery` typed by `DeliveryDocument` and `DeliveryTotal` by `DeliveryCounts` (DA-11 of delivery-analytics)", () => {
    expect(Object.keys(sets).slice(BEFORE_DELIVERY_ANALYTICS.length, BEFORE_DIRECTION_CATALOG.length)).toEqual([...DELIVERY_ANALYTICS]);
    expect(Object.keys(sets).indexOf("Delivery")).toBe(126);
    expect(Object.keys(sets).at(BEFORE_DELIVERY_ANALYTICS.length - 1)).toBe("CardUpstreamChange");
    expect(Object.keys(sets).at(BEFORE_DIRECTION_CATALOG.length - 1)).toBe("DeliveryMore");
    expect(DELIVERY_ANALYTICS.map((name) => sets[name])).toEqual(DELIVERY_ANALYTICS.map((name) => served(name)));
    expect(DELIVERY_ANALYTICS.map((name) => sets[name])).toEqual(DELIVERY_ANALYTICS.map((name) => declaredKeys(DECLARED_AS[name] ?? name)));
    expect(DELIVERY_ANALYTICS.map((name) => sets[name]?.length)).toEqual([8, 9, 9, 7, 2, 20, 3, 4, 4, 3, 4]);
  });

  it("names direction-catalog's three after DeliveryMore, sets 138 to 140: 6, 13, 6 keys in the types' order, `Catalog` typed by `CatalogDocument` (DC-9 of direction-catalog)", () => {
    expect(Object.keys(sets).slice(BEFORE_DIRECTION_CATALOG.length, BEFORE_DEPARTMENT_OVERVIEW.length)).toEqual([...DIRECTION_CATALOG]);
    expect(Object.keys(sets).indexOf("Catalog")).toBe(137);
    expect(Object.keys(sets).at(BEFORE_DIRECTION_CATALOG.length - 1)).toBe("DeliveryMore");
    expect(Object.keys(sets).at(BEFORE_DEPARTMENT_OVERVIEW.length - 1)).toBe("CatalogChange");
    expect(DIRECTION_CATALOG.map((name) => sets[name])).toEqual(DIRECTION_CATALOG.map((name) => served(name)));
    expect(DIRECTION_CATALOG.map((name) => sets[name])).toEqual(DIRECTION_CATALOG.map((name) => declaredKeys(DECLARED_AS[name] ?? name)));
    expect(DIRECTION_CATALOG.map((name) => sets[name]?.length)).toEqual([6, 13, 6]);
  });

  it("names department-overview's three last, after CatalogChange, sets 141 to 143: 7, 6, 5 keys in the types' order (DO10 of department-overview)", () => {
    expect(Object.keys(sets).slice(BEFORE_DEPARTMENT_OVERVIEW.length)).toEqual([...DEPARTMENT_OVERVIEW]);
    expect(Object.keys(sets).indexOf("CardCounts")).toBe(140);
    expect(Object.keys(sets).at(BEFORE_DEPARTMENT_OVERVIEW.length - 1)).toBe("CatalogChange");
    expect(Object.keys(sets).at(-1)).toBe("CardFlowCount");
    expect(DEPARTMENT_OVERVIEW.map((name) => sets[name])).toEqual(DEPARTMENT_OVERVIEW.map((name) => served(name)));
    expect(DEPARTMENT_OVERVIEW.map((name) => sets[name])).toEqual(DEPARTMENT_OVERVIEW.map((name) => declaredKeys(name)));
    expect(DEPARTMENT_OVERVIEW.map((name) => sets[name]?.length)).toEqual([7, 6, 5]);
  });

  it("names lock-diff's five after ShownBindings, before flow-read's five: 4, 4, 3, 3, 3 keys in the types' order, the two sides' first two keys apart (UI-1, TE-2 of lock-diff)", () => {
    expect(Object.keys(sets).slice(BEFORE_LOCK_DIFF.length, BEFORE_FLOW_READ.length)).toEqual([...LOCK_DIFF]);
    expect(Object.keys(sets).indexOf("LockDiffView")).toBe(76);
    expect(Object.keys(sets).at(BEFORE_FLOW_READ.length - 1)).toBe("LockDiffCodeSide");
    expect(LOCK_DIFF.map((name) => sets[name])).toEqual(LOCK_DIFF.map((name) => served(name)));
    expect(LOCK_DIFF.map((name) => sets[name]?.length)).toEqual([4, 4, 3, 3, 3]);
    expect(sets.LockDiffSpecSide).toEqual(["commit", "path", "diff"]);
    expect(sets.LockDiffCodeSide).toEqual(["blob", "file", "diff"]);
  });

  it('names hub-git\'s HubSync last, after ConfirmRead, the hundred-and-tenth, with its five keys in the types\' order (docs/features/hub-git.md "Data")', () => {
    expect(Object.keys(sets).slice(BEFORE_HUB_GIT.length, BEFORE_WORKSPACES.length)).toEqual([...HUB_GIT]);
    expect(Object.keys(sets).at(BEFORE_WORKSPACES.length - 1)).toBe("HubSync");
    expect(Object.keys(sets).indexOf("HubSync")).toBe(109);
    expect(sets.HubSync).toEqual(["ahead", "behind", "state", "error", "at"]);
    expect(sets.HubSync).toEqual(served("HubSync"));
  });

  it('names hub-confirm\'s ConfirmRead after Team, before hub-git\'s HubSync, the hundred-and-ninth, and keeps an author at four keys (docs/features/hub-confirm.md "Data")', () => {
    expect(Object.keys(sets).slice(BEFORE_HUB_CONFIRM.length, BEFORE_HUB_GIT.length)).toEqual([...HUB_CONFIRM]);
    expect(Object.keys(sets).at(BEFORE_HUB_GIT.length - 1)).toBe("ConfirmRead");
    expect(sets.ConfirmRead).toEqual(["id", "question", "updated_at"]);
    expect(sets.Author).toEqual(["type", "role", "model", "run"]);
  });

  it('names hub-team\'s Team after Identity, before hub-confirm\'s ConfirmRead: the team read\'s three keys, the hundred-and-eighth (docs/features/hub-team.md "Data")', () => {
    expect(Object.keys(sets).slice(BEFORE_HUB_TEAM.length, BEFORE_HUB_CONFIRM.length)).toEqual([...HUB_TEAM]);
    expect(Object.keys(sets).at(BEFORE_HUB_CONFIRM.length - 1)).toBe("Team");
    expect(sets.Team).toEqual(["login", "name", "allowed"]);
  });

  it('names hub-identity\'s Identity after TraceStage, before hub-team\'s Team: the identity read\'s one key, the hundred-and-seventh (docs/features/hub-identity.md "Data")', () => {
    expect(Object.keys(sets).slice(BEFORE_HUB_IDENTITY.length, BEFORE_HUB_TEAM.length)).toEqual([...HUB_IDENTITY]);
    expect(Object.keys(sets).at(BEFORE_HUB_TEAM.length - 1)).toBe("Identity");
    expect(sets.Identity).toEqual(["login"]);
  });

  it("names trace-views' TraceStage after card-trace's seven, before hub-identity's Identity: a lane's three keys, the hundred-and-sixth; a document's stages after counts, a node's stage and distance after path (docs/features/trace-views.md \"Routes and types\")", () => {
    expect(Object.keys(sets).slice(BEFORE_TRACE_VIEWS.length, BEFORE_HUB_IDENTITY.length)).toEqual([...TRACE_VIEWS]);
    expect(Object.keys(sets).at(BEFORE_HUB_IDENTITY.length - 1)).toBe("TraceStage");
    expect(sets.TraceStage).toEqual(["stage", "distance", "counts"]);
    expect(declaredKeys("TraceStage")).toEqual(["stage", "distance", "counts"]);
    expect(sets.TraceDocument?.slice(3, 5)).toEqual(["counts", "stages"]);
    expect(sets.TraceNode?.slice(2, 5)).toEqual(["path", "stage", "distance"]);
  });

  it("names card-trace's seven after CardNotFound, before TraceStage: 8, 5, 9, 8, 3, 5, 3 keys in the types' order, the hundred-and-fifth CardClearGroupWrite (docs/features/card-trace.md \"Data\")", () => {
    expect(Object.keys(sets).slice(BEFORE_CARD_TRACE.length, BEFORE_TRACE_VIEWS.length)).toEqual([...CARD_TRACE]);
    expect(Object.keys(sets).indexOf("TraceDocument")).toBe(98);
    expect(Object.keys(sets).at(BEFORE_TRACE_VIEWS.length - 1)).toBe("CardClearGroupWrite");
    expect(CARD_TRACE.map((name) => sets[name])).toEqual(CARD_TRACE.map((name) => served(name)));
    expect(CARD_TRACE.map((name) => sets[name]?.length)).toEqual([8, 5, 9, 8, 3, 5, 3]);
    expect(CARD_TRACE.map((name) => declaredKeys(name))).toEqual(CARD_TRACE.map((name) => served(name)));
  });

  it("names board-ui's twelve after FlowInner, before card-trace's seven: 5, 7, 2, 3, 22, 34, 4, 6, 3, 4, 7, 2 keys in the types' order, the ninety-eighth CardNotFound; an acceptance's three new keys after note (AC-09 of docs/features/board-ui.md, docs/features/card-trace.md \"Data\", docs/features/card-tasks.md \"Documents\")", () => {
    expect(Object.keys(sets).slice(BEFORE_BOARD_UI.length, BEFORE_CARD_TRACE.length)).toEqual([...BOARD_UI]);
    expect(Object.keys(sets).indexOf("CardList")).toBe(86);
    expect(Object.keys(sets).at(BEFORE_CARD_TRACE.length - 1)).toBe("CardNotFound");
    expect(BOARD_UI.map((name) => sets[name])).toEqual(BOARD_UI.map((name) => compared(name)));
    // A flow's `return` after `accept` (board-handoff, docs/canon/card-flows.md "The rule"); a
    // card's `tasks` after `evidence` (card-tasks, docs/features/card-tasks.md "Documents"); a
    // flow's `title` after `name` (docs/features/departments.md "Acceptance criteria", DP-4); a
    // card's `items` after `tasks` (K1 of card-questions); a row's and a card's `mandate`, a card's
    // `subtask_items` (M1, M6 of task-mandate).
    // A card's 35 since card-context's `context` (docs/features/card-context.md "Acceptance criteria", CC10);
    // a row's 23 and a card's 36 since upstream-signal's `upstream` (docs/features/upstream-signal.md
    // "Acceptance criteria", US-12).
    expect(BOARD_UI.map((name) => sets[name]?.length)).toEqual([5, 7, 2, 3, 23, 36, 4, 6, 3, 4, 7, 2]);
    expect(sets.CardFlow?.at(-1)).toBe("return");
    expect(BOARD_UI.map((name) => declaredKeys(name))).toEqual(BOARD_UI.map((name) => served(name)));
  });

  it("names flow-read's five after LockDiffCodeSide, before board-ui's twelve: 13, 6, 16, 8, 2 keys in the types' order (flow-scenario's three, doc-modal's gap `titles`), the eighty-sixth FlowInner (docs/features/flow-read.md, flow-canvas.md, flow-scenario.md, doc-modal.md)", () => {
    expect(Object.keys(sets).slice(BEFORE_FLOW_READ.length, BEFORE_BOARD_UI.length)).toEqual([...FLOW_READ]);
    expect(Object.keys(sets).indexOf("FlowView")).toBe(81);
    expect(Object.keys(sets).at(BEFORE_BOARD_UI.length - 1)).toBe("FlowInner");
    expect(FLOW_READ.map((name) => sets[name])).toEqual(FLOW_READ.map((name) => served(name)));
    expect(FLOW_READ.map((name) => sets[name]?.length)).toEqual([13, 6, 16, 8, 2]);
    expect(sets.FlowGap).toEqual(["code", "state", "count", "items", "titles", "cause"]);
    expect(FLOW_READ.map((name) => declaredKeys(name))).toEqual(FLOW_READ.map((name) => served(name)));
  });

  it("names kind-interpretation's keys: the review document's interpretation after staged_at, the Interpretation set (AC-T4)", () => {
    // Then intake-similar's `similar`, once the file names it, before notes (docs/features/intake-similar.md "Acceptance criteria", IS10).
    const review = sets.Proposal ?? [];
    expect(review.slice(review.indexOf("staged_at"), review.indexOf("staged_at") + 4)).toEqual(["staged_at", "interpretation", "card_id", "why"]);
    expect(review.slice(review.indexOf("why"))).toEqual(isAhead("Proposal", "similar") ? ["why", "notes"] : ["why", "similar", "notes"]);
    expect(sets.Interpretation).toEqual(["decision", "how", "why"]);
  });
});
