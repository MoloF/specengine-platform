---
class: spec
status: shipped
scope: [ui]
ref: ui-tasks analysis 2026-10-06, recommendations accepted by the orchestrator; 08 section 2 Phase 4; iterations 1-2, review accepted
shipped: 2026-10-06
adrs: []
---

# UI tasks: the queue on mocks

## Why

The owner sees which tasks wait for him, what an approved task froze and what changed since, without a terminal read per task. Slice 4 renders `spec task list` and `spec task show --json` of the draft `docs/features/task-package.md` over the mock (ADR-0033), read-only: an owner action is a command for a terminal, which asks `[y/N]` (`architecture.md#control`, ADR-0012). States, staleness and proposal matching stay in core.

Working answers, 2026-10-06 (orchestrator; no new ADR): cite the draft, re-point at its shipping; commands only until daemon-read's Q4 ADR; a list, not a board; task-package's G1-G4. Assumed: commands run in the root's worktree; the Clipboard API works on `localhost`. What became true: `ui/README.md` "Contract seam" (owner actions as commands) and "Screen rules" (`role`, `profile` never quoted); the client contract below.

## Data

**Client** (`ui/src/api/client.ts`):

```ts
/** MISSING ENDPOINT GET /api/projects/:p/tasks (07 section 3 lists it; = spec task list --json; rust-developer, daemon-read "Out of scope") */
getTasks(project: string): Promise<TaskList>;
/** MISSING ENDPOINT GET /api/projects/:p/tasks/:id (07 section 3 lacks it; = spec task show T --json, uncut) */
getTask(project: string, id: string): Promise<TaskPackage | TaskNotFound>;
```

`getTask("harbor-sim", "T-0107")` -> `GET /api/projects/harbor-sim/tasks/T-0107`; 404: the exit-1 document, as data; other non-2xx: `ClientError` verbatim. No write member (`POST .../tasks/:id/transition` waits for Q4). Keys `["tasks", p]`, `["task", p, id]`, both in `READS_AFTER_DECISION`; `getTask` without `keepPreviousData`.

**Provisional types** (`ui/src/api/provisional.ts`), citing `docs/features/task-package.md` "Description and interactions" (`TaskList`, `TaskListEntry`, `TaskNotFound`) or "Data" (the rest); `KnownTaskStatus` `docs/specs/specengine-platform/05-architecture.md` "3.3. Index schema (SQLite)"; `kind`, `role`, `profile` plain strings (ADR-0027, ADR-0031):

```ts
TaskList { tasks: TaskListEntry[]; notes: string[] }
TaskListEntry { id; status: TaskStatus; title?; targets: string[]; stale?: boolean; updated_at }
TaskNotFound { id?; reason }
TaskPackage { schema_version: number; id; project; status: TaskStatus; title?; goal?; profile?; stale?: boolean;
  targets: TaskTarget[]; criteria: TaskCriterion[]; affected_nodes: string[]; plan?; assumptions: TaskAssumption[];
  open_proposals: TaskProposal[]; owner_notes: OwnerNote[]; bindings: unknown[]; spec_snapshot?: SpecSnapshot;
  snapshot_diff?: SnapshotDiff[]; claim?: TaskClaim; runs: TaskRun[]; bundle?: TaskBundle; author?: Author;
  created_at; updated_at; notes: string[] }  // 25 keys
TaskTarget { id?; path?; kind?; title? }  TaskCriterion { ref?; text? }  TaskAssumption { proposal; text }
OwnerNote { at; note }  TaskProposal { id; kind; status: ProposalStatus; target_ids: string[]; task_id?; summary }
SpecSnapshot { at; place: SnapshotPlace; nodes: SnapshotNode[] }  SnapshotPlace { worktree; root_rel; branch; commit }
SnapshotNode { id; path; span_hash }  SnapshotDiff { id; path; span_hash; diff; cut: boolean }
TaskClaim { at; role; worktree; branch }  TaskBundle { node_ids: string[]; budget: number; bundle_hash }
TaskRun { run: number; role; started_at; ended_at?; outcome?: RunOutcome; summary?; changed_files: string[] }
TaskStatus = KnownTaskStatus | Unlisted  // the ten of 05 section 3.3
RunOutcome = KnownRunOutcome | Unlisted  // completed | partial | failed | abandoned
// `?`: `| null`, never omitted; unmarked: string; per type a `satisfies Record<keyof T, true>` record
```

**Mock** (`ui/src/mocks/tasks.ts`): one builder, fixed UTC, no stack word; `getTask` derives `open_proposals`, `assumptions` from the mock's proposals per read as task-package "Data" (`open`/`approved`, bound or on the snapshot), so an Inbox decision drops the proposal from both. `harbor-sim`: T-0104 `done` (a closed `completed` run); T-0105 `cancelled`; T-0107 `review`, a plan, PR-0041, PR-0042 bound (PR-0046 deferred: not listed), one proposal on its nodes; T-0108 `ready`, `stale` true, a removed target `RULE-NIGHT-LIGHTS` (G4); T-0109 `in_progress`, run 1 open, `stale` true, one cut diff; T-0110 `changes_requested`, an owner note; T-0111 `ready`, `stale` null + a note (the snapshot's place gone); T-0112 `ready`, `stale` false, PR-0044 bound; T-0113 `draft`, no snapshot; the list note `T-0106: ...; skipped` (corrupt-row form). `ledger-api`: T-0031 `in_progress`, T-0033 `review`, a `profile`, roles of another vocabulary. `large`: T-0200 ... T-0519 added (the seven reachable states in turn, each `in_progress` one claimed with run 1 open), T-0200 at every cap of task-package "Data". Diff heads `--- snapshot <path>`, `+++ current <path>`.

## Rules and edge cases

- WHEN a state, outcome or proposal status is outside its table THEN a neutral badge with the raw text, sorted last.
- Nothing computed, nothing held (ADR-0012): groups from `status` and `stale`; no diff, staleness or match derived; proposals split only by `task_id`; no control changes a task.
- `role`, `profile`, `kind` never quoted or compared outside `src/mocks/` and tests (ADR-0027, ADR-0031).
- A command: fixed words and a validated ID (`^T-[0-9]{4,}$`). A route T failing the pattern is still read; it gets "No command for this ID", no Copy.
- Approval place (ADR-0032): no worktree named until `Project.root`. `ui/README.md` "Screen rules" hold.

## Acceptance criteria

Verifiers: Vitest, `ui_policy.rs` (`test-engineer`), the gates; "builder": a test-built answer. M: the mutation turning it red; all 35 applied and red in iteration 1 (AC-02's dropped and extra keys in `tsc`, `pnpm build`).

- [x] AC-01 - `client.ts?raw` holds both `MISSING ENDPOINT` comments; on a counting stub `#/harbor-sim/tasks` makes one `getTasks`, no `getTask`; `.../tasks/T-0107` one of each; no client member matches `/transition|approve|cancel|claim/i` (M: a comment removed; `getTask` per row; `transitionTask`). Amended by `ui-live-tasks`: both are fetched, their comments cite the http README, no `MISSING ENDPOINT`; each takes its query's `AbortSignal`.
- [x] AC-02 - the task types cite existing headings (`doc_pointers`); key records equal the cited lists: `TaskPackage` 25, `TaskListEntry` 6, `TaskRun` 7, `OwnerNote` 2, `TaskNotFound` 2 (M: a key dropped; an extra `TaskRun` key; "Data" renamed).
- [x] AC-03 - ten states, four outcomes: distinct labels, an icon each; builder status `triage`: a neutral badge, raw, after every known group (M: a badge without text; unknown first).
- [x] AC-04 - `harbor-sim`: "Waiting for you" exactly T-0107, T-0108; then T-0113; T-0110; T-0111, T-0112; T-0109; T-0104, T-0105 only with "Closed" (M: `draft` waiting; `stale` ignored; closed by default).
- [x] AC-05 - chips and typing make no call; counts over the whole answer; a decomposed query finds a precomposed title (builder); "Clear filters" restores the defaults (M: a call per chip; no NFC).
- [x] AC-06 - T-0108, T-0112, T-0111 (+ its note), T-0113: the four staleness displays; Spec changes: exactly `snapshot_diff`'s entries in order, T-0109's with the cut notice (M: `null` as unchanged; a diff from `spec_snapshot`).
- [x] AC-07 - goal, plan, criterion text, note, summary: `textContent` equals the JSON string (builder plan with blank leading lines, trailing spaces); every `a[href]` from `sectionHash` as Detail; none for a gone target (M: `plan.trim()`; an `href` from a title). Since `ui-markdown` (2026-10-06) these render as markdown, the words kept (`TasksView.test.tsx`).
- [x] AC-08 - T-0107: "Raised by this task" exactly its `task_id` T-0107 entries, the rest "On its nodes"; PR-0041's Inbox card links `#/harbor-sim/tasks/T-0107`; PR-0041 rejected, then T-0107: a fresh `getTask`, PR-0041 gone from Proposals and assumptions (M: split by `kind`; `"task"` not invalidated). Since `daemon-read` the Inbox card had no task link (the review document had no `task_id`); it returned with `ui-live-tasks` (2026-10-07), from the review document's `task_id`.
- [x] AC-09 - per state exactly its table row + `spec task show <T>`; Copy writes the exact text (stubbed clipboard), a polite "Copied"; a rejecting clipboard: the message; builder `T-1; rm -rf ~`: no command, no Copy (M: `changes` for `ready`; a note in the command; the ID unchecked).
- [x] AC-10 - Package: `JSON.parse` of its text deep-equals the answer, `null` and `[]` kept; Copy copies it; builder `schema_version` 2: a notice naming 2, Package only (M: nulls dropped; no guard).
- [x] AC-11 - `slow`: skeletons, `aria-busy="true"`, both regions; T-0112 right after T-0107 never shows T-0107's `h1`; `empty`: "No tasks yet" + the command; `error`: verbatim, Retry one more `getTasks`; `T-0999`: `reason`, "Back to tasks"; a throwing detail leaves the list (M: one boundary; inert Retry; `keepPreviousData` on `getTask`).
- [x] AC-12 - one row `tabindex="0"`; each key of Keyboard acts; Enter one history entry; Esc focuses the row; nothing with Ctrl, Meta, Alt or from the field; no `document` or `window` keydown listener; one `h1`; focus never on `body` (M: a `document` listener; focus on `body`).
- [x] AC-13 - title, plan, note, role `<img src=x onerror=alert(1)>` as text, no `img`; `policy.test.ts`: no `role`, `profile`, `kind` value quoted or compared outside `src/mocks/` and tests (M: a branch on `profile`; the plan through an HTML sink).
- [x] AC-14 - two mock builds deep-equal, no clock read; every Inbox `task_id` resolves; `harbor-sim` has the seven reachable states; `large`: 329 entries from one call, all listed with "Closed"; no P2-3 word (07 section 1.2) in `src/mocks/tasks.ts` (M: a dangling `task_id`; `Date.now()` in the builder).
- [x] AC-15 - 15 packages; `pnpm lint` (0 warnings), `build`, `test` green, silent; `ui_policy`, `anonymity`, `doc_pointers` green (33/33); `ui/README.md` <= 8 192 B (8 183 B, net -4); the Tasks `about` without "accepted" or "board"; docs gate clean, worst W <= this draft's 108 506 B (M: a 16th package; the old `about`).

## Owner's manual check

`pnpm --dir ui dev`: `#/harbor-sim/tasks/T-0107`, all six tabs; copy a command, paste it in a terminal, answer N; T-0109 (cut diff), T-0111 (Unknown + its note), T-0108 ("Removed since approval"); `?scenario=large#/harbor-sim/tasks/T-0200`, its Package tab, an `in_progress` task's Runs; `?scenario=empty`, `error`, `slow`; keyboard alone (arrows, `j`/`k`, Home/End, Enter, Esc back to the row, `?`); "Back to tasks" by plain click and by Cmd-click; VoiceOver on rows, chips, Copy; 200 %: panes stacked, opening a task from the list scrolls its heading into view below the header, focus kept.

## Open

- **Endpoints** (`rust-developer`): `GET /api/projects/:p/tasks` (07 section 3), `.../tasks/:id` (not there), uncut, 404 with `TaskNotFound`; `task.*` events with `ui-live`. **Q4** (daemon-read): browser owner actions take an ADR. **`Project.root`** (daemon-read) brings "Run them in `<root>`.".
- **At task-package shipping**: citations re-pointed, key records re-checked against the shipped JSON and `fixtures/daemon-keys.json`, `Proposal.task_id` restored (daemon-read drops it; the Inbox card's "Task" link needs it).
- **`.claude/agents/ui-developer.md`** (owner's text; ADR-0033 "Cost"): lines 18-23 prepend "Until `src/api/generated/` exists the contract is `src/api/provisional.ts`: documented shapes citing their source (`ui/README.md` "Contract seam"), draft specs included."; line 38 "both the light and the dark theme" -> "the dark theme"; lines 41-43 "Then, from the root, regenerate ... from Rust." -> "`pnpm test`, then the root checks of `ui/README.md` "Gates"." and "`spec serve` + `pnpm --dir ui dev`" -> "`pnpm --dir ui dev` on the mock"; lines 47-48 as `ui-shell` "Open".

## Out of scope

Any write; live updates (`ui-live`); the brief as text; a Bundle tab; a column board; a nav count; creating or planning tasks; applied proposals; bindings, `@assumes` (Phase 3); priority (Phase 6); generated types; the endpoints.

## Implementation

**Route** `#/<p>/tasks[/<T>]`, the list beside the task; nothing auto-selected. At <= 48em the panes stack and opening a task from the list (Enter or click) scrolls its heading into view (`scroll-margin-top: 7rem`), focus kept. **List** (listbox, one roving row; Up/Down, `j`/`k`, Home/End, Enter, `?`): a count line, then "Waiting for you" (`review`; `ready` with `stale` true), Draft, Changes requested, Ready, In progress, Closed, reserved, "Other states" (raw state per row); chips per state present (counts), "Spec changed", a text field, no call. Nothing left: "No task matches the filters." + "Clear filters"; when the defaults hide every task because all are closed: "Every task is closed: press Closed to list them." **Detail**: `h1` "T-0107: <title>" (colon and space visually hidden; "Untitled"); the staleness badge in its four displays; a "Waiting for you" note; the owner commands (`actions.ts`, the state table; Copy, polite "Copied"); six tabs with counts, the selected one kept across tasks and alone rendered, lists over 10 items collapsed. Spec changes: a `DiffView` per `snapshot_diff` entry; a node whose hunks all end `+0,0` (`removesAll`) is text tagged "Removed since approval" in the frozen list and its diff head. `schema_version` not 1: only `spec task show <T>`. Unknown T: `reason`, "Back to tasks", focusing the list only on a plain same-tab click (`movesHere`). Esc returns to the row. Inbox cards link "Task" to `#/<p>/tasks/<task_id>`.

| Module | What it does |
|---|---|
| `ui/src/tasks/` | `TasksView` (route, a boundary per pane), `TaskList`, `TaskDetail` with `OverviewPanel`, `SpecChangesPanel`, `TaskProposalsPanel`, `RunsPanel`, `PackagePanel`, `OwnerCommands`; `actions.ts`, `labels.ts`, `groups.ts`, `filter.ts`, `answer.ts`, `parts.tsx` |
| `ui/src/ui/` | `CopyButton.tsx` (Clipboard API); `DiffView.tsx`, `diff.ts` (`removesAll`) moved from `src/inbox/` |
| `ui/src/api/`, `mocks/` | `getTasks`, `getTask`, the types, keys, `useTask` without placeholder; `tasks.ts` builder, `MockClient` |
| `ui/src/app/`, `inbox/`, `styles/` | route, `about`, keys, `RegionBoundary` heading; `ProposalCard` Task link; `run-*`, `stale-*` roles |

Tests: 826 Vitest in 39 files (new `tasks/TasksView`, `actions`, `groups`, `filter`, `labels`, `mocks/tasks`, `tasks.smoke`; extended client, provisional, queries, App, `policy`, tokens); lint 0, build, 15 packages. Iteration 2 fixed the review's nits: the Back focus, the `h1`, removed nodes, the stacked scroll, `large`'s claims and runs, the all-closed message.

Deviations, accepted: the empty detail a `section`; `KnownTaskStatus` the keys of an unexported record; the fallback keeps an `h1`; labels `review` "Plan review", `in_review` "Work in review", `partial` "Partly done"; a non-question summary is the rationale's first line; `policy.test.ts` detects `.role`/`.profile`/`.kind` comparisons; the all-closed message.
