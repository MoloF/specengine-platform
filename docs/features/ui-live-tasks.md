---
class: spec
status: draft
scope: [crates/specengine-http, ui]
ref: ui-live analysis 2026-10-07 ("Tasks later"), ui-live "Out of scope", task-package "Open", ui-tasks AC-08; working answers by the orchestrator (HEAD e55935d); 08 s2 Phase 4 on Phase 2's daemon
adrs: []
---

# UI live tasks: the tasks over HTTP

## Why

On the daemon, the UI's default client, Tasks, Home's task panel and the palette's task group say "Not built yet": `specengine-http` serves neither `spec task list` nor `spec task show`, and the UI listens to no `task.*` event, so a claim or an approval made in a terminal never reaches the screen. Task-package shipped both commands (`docs/canon/tasks.md`, `docs/canon/task-package.md`); this slice serves them read-only, listens to the nine task events, brings back the Inbox card's "Task" link (ui-tasks AC-08) and re-points the UI's task types from the task-package spec to the canon.

No new ADR: the routes are reads under the rules `daemon-read` and `ui-live` set (ADR-0033, 0034, 0012, 0006, 0008); owner actions stay commands to copy, agent actions MCP only.

## Description and interactions

**Daemon** (`rust-developer`, `crates/specengine-http/src` only; no CLI change):

- `GET /api/projects/:p/tasks` -> `task_list(env, globals, &TaskListRequest {statuses, git: process_git(env)})`, `Outcome::TaskList`.
- `GET /api/projects/:p/tasks/:id` -> `task_show(env, globals, &TaskShowRequest {id: Some(id), next: false, git: process_git(env)})`, `Outcome::TaskShow`.
- One call each in the project's turn after `answer.rs` `found`, the generic exit map (no exception: a refusal is exit 1, a 404 document); body the CLI's stdout less its final LF (notes: the JSON's `notes`).
- Never called: `task_new`, `task_approve`, `task_changes`, `task_cancel`, `task_plan`, `task_claim`, `task_report`, `task_complete`. Owner actions stay terminal commands (`docs/canon/tasks.md` "Commands", `architecture.md#control`); an agent moves a task over MCP only (`docs/canon/task-package.md` "MCP"); the daemon never claims, reports or approves.
- The live tail is unchanged: it forwards every stored type (`EventLine`), `task.*` since task-package; its README line is stale.

**UI** (`ui-developer`, `ui/src`): `HttpClient.getTasks`, `getTask` fetch; `QUEUE_EVENT_TYPES` gains the nine `task.*`; `useLiveQueue` invalidates the task reads; the Inbox card's "Task" fact returns; types and citations fixed. Tasks, Home (`ui-home`) and the palette then read the daemon with no screen change.

**Tests** (`test-engineer`): `crates/specengine-http/tests` (`tasks.rs` new; `door`, `no_domain`, `daemon_keys`, `methods`, `projects`, `tail`, `turn`), `fixtures/daemon-keys.json`, Vitest beside the changed files.

## Data

### Routes

| Method, path | Query | Answer |
|---|---|---|
| GET `.../tasks` | `status`, repeated, in order (= `--status`) | `spec task list [--status S]... --json`: 200 `{tasks, notes}` |
| GET `.../tasks/:id` | -- | `spec task show T --json`: 200 the package; exit 1 -> 404 `{id, reason}`; exit 2 -> 503 |

Documents: the list and `{id, reason}` `docs/canon/tasks.md` "Commands"; the package `docs/canon/task-package.md` "Package" (25 keys, uncut, at most `PACKAGE_BUDGET` 460 000 characters: "Caps"). `:id` percent-decoded once, strictly, as `proposals/:id`. A `status` value is judged by the model's `TaskStatus::parse` and a refusal lists `TaskStatus::ALL`, never a literal: `` `status=triage`: not a task state: draft, analysis, ..., cancelled `` (400). `/tasks/:id` takes no query name (`` `next` is no query name of tasks/:id: it takes no query name ``).

```
GET /api/projects/harbor-sim/tasks?status=ready&status=in_progress
200 {"tasks":[{"id":"T-0108","status":"ready","title":"Night lights","targets":["MEC-TIDES"],"stale":true,"updated_at":"2026-10-07T09:00:00Z"}],"notes":[]}
GET /api/projects/harbor-sim/tasks/T-0099
404 {"id":"T-0099","reason":"no task T-0099 in this repository"}
```

### Key sets

`fixtures/daemon-keys.json` + 17 sets from served documents, 32 in all: `TaskList` 2, `TaskListEntry` 6, `TaskNotFound` 2, `TaskPackage` 25, `TaskTarget` 4, `TaskCriterion` 2, `TaskAssumption` 2, `TaskProposal` 6, `OwnerNote` 2, `SpecSnapshot` 3, `SnapshotPlace` 4, `SnapshotNode` 3, `SnapshotDiff` 5, `TaskClaim` 4, `TaskRun` 7, `TaskBundle` 3, `Author` 4 (the package's `author`). `one_key_set` needs an instance of each: state S below holds every shape.

### Client and events

```ts
/** GET /api/projects/:p/tasks (crates/specengine-http/README.md "Endpoints"; = spec task list --json) */
getTasks(project: string): Promise<TaskList>;
/** GET /api/projects/:p/tasks/:id (crates/specengine-http/README.md "Endpoints"; = spec task show T --json, uncut; 404 the exit-1 document) */
getTask(project: string, id: string): Promise<TaskPackage | TaskNotFound>;
```

The UI sends no `status` (ui-tasks filters the whole answer). `QUEUE_EVENT_TYPES`: the five `proposal.*`, then `task.created`, `task.planned`, `task.approved`, `task.changes_requested`, `task.claimed`, `task.run_reported`, `task.completed`, `task.cancelled`, `task.refreshed` (`docs/canon/tasks.md` "Store"; payload `{id}`, + `run`; `.refreshed` `{id, proposal, node}`).

| Event on `p` | Invalidated (fetched when on screen, else stale) |
|---|---|
| `task.*`, `id` a string | `["tasks", p]` exact, `["task", p, id]` exact |
| `task.*`, no string `id` | `["tasks", p]`, `["task", p]` |
| any `proposal.*` | as today + `["tasks", p]`, `["task", p]` |
| a gap (stream reopened) | as today + `["tasks", p]`, `["task", p]` |

Types (`provisional.ts`): `TaskBundle.bundle_hash: string | null`; `TaskProposal.summary` documented as the canon: "a question's or discrepancy's summary, else the rationale's first line, `null` for an empty rationale"; `SnapshotDiff.diff` both `null`s: past 262 144 B in all `cut` true, git cannot make it `cut` false with a note.

### Citations

| `ui/src` cites `docs/features/task-package.md` | Re-pointed to |
|---|---|
| "Description and interactions": `TaskList`, `TaskListEntry`, `TaskNotFound` | `docs/canon/tasks.md` "Commands" |
| "Data": states, owner rows (`actions.ts`, `labels.ts`, tests) | `docs/canon/tasks.md` "Transitions" |
| "Data": review document `task_id` (`provisional.ts`, `mocks/build.ts`, `daemonKeys.test.ts`) | `docs/canon/tasks.md` "Task-bound proposals" |
| "Data": the package and parts, `answer.ts` | `docs/canon/task-package.md` "Package"; `schema_version` "Versioning" |
| "Data": `SnapshotDiff`, `SpecChangesPanel.tsx`; `RunOutcome` | `docs/canon/task-package.md` "Staleness", "Caps" |

`provisional.test.ts` `DRAFT`/`LISTED` follow the table; `client.ts`'s tasks comments drop `daemon-read`; `http.ts`'s `DAEMON_READ_GAP` goes with its helper.

## Rules and edge cases

- WHEN `:id` is not a task ID THEN 404 with the CLI's `id` `null` and reason; a look-alike (`T` in Cyrillic) 503 naming its Latin form; `T` in `[ids]`, a root in no git worktree, no `git`, a newer queue schema, a corrupt row on `show`, another repository's task (same slug) -> 503, the CLI's line.
- WHEN a corrupt row or another repository's task meets `list` THEN 200, a note (the CLI's).
- A read writes nothing under the root and moves no task: no event row, the queue's dump unchanged; like `inbox` it may create and migrate `<slug>.db` and refresh the index (the package's `bundle_hash`) in the data directory (`architecture.md#storage`).
- Every method but GET on both routes -> 405 `Allow: GET`; `POST .../tasks/:id/transition` (07 s3, for `decision-staging`) stays an unknown route, 404.
- One state, one answer: two reads with no change are byte-identical (no read time in the package).
- WHEN a `task.*` or `proposal.*` event reaches `p` THEN only `p`'s task reads, per the table; a `task.*` never reads the inbox, a proposal or a spec read.
- The Inbox card's "Task" fact comes from the review document (`InboxEntry` stays 11 keys): absent until it is read; `null` -> "No task"; else a link `sectionHash(p, "tasks", task_id)`.

## Acceptance criteria

Setup as `ui-live`: git copies of `fixtures/spec-a` (A), `-b` (B), a scratch `HOME`, a `--port 0` daemon per test. **State S**, made through the CLI library (owner commands with a consent yes, as CLI `tests/task_common`): T-0001 `draft`; T-0002 approved, its target then edited on disk (`stale` true, one diff); T-0003 planned with a criterion, `changes --note`, re-planned, approved, an open question with a working answer on its target, claimed, a run reported; T-0004 cancelled. M: the mutation, seen red.

- [ ] AC-01 -- list parity: A and B in S: `/tasks`, `?status=ready`, `?status=in_progress&status=draft` -> 200, byte-equal to `spec --root R task list [--status S]... --json` less its final LF (`tasks.rs`; M: `status` ignored).
- [ ] AC-02 -- show parity: T-0001 to T-0004 on A and B -> 200, byte-equal to `spec --root R task show T --json` less its LF; two reads byte-identical; T-0002 `stale` true, one `snapshot_diff` entry (M: the request built with `next: true`).
- [ ] AC-03 -- refusals: `/tasks/T-0099` -> 404 exactly the document of "Data"; `/tasks/foo` 404, `id` `null`; `/tasks/%D0%A2-0001` 503 naming `T-0001`; `/tasks/`, `/tasks/T-0001/transition` 404 listing the routes (`tasks`, `tasks/:id` among them); a copy of A without `.git` and one whose `[ids]` claims `T`: 503 on both routes (M: exit 1 mapped to 200).
- [ ] AC-04 -- query: `?status=triage`, `?status=`, `?state=ready`, `/tasks/T-0001?next=true`, `?x=1` -> 400 naming the parameter, a fresh `HOME` left empty; `no_domain.rs` adds the ten names of `TaskStatus::ALL` to its words, `src` green (M: a handler comparing `"ready"`).
- [ ] AC-05 -- read-only: after AC-01 to AC-04 `git status --porcelain --ignored` empty, `HEAD` unchanged, the highest event `seq` and `spec state export`'s bytes as before; POST, PUT, DELETE, HEAD on both -> 405 `Allow: GET`; `door.rs` forbids every `task_` name but `task_list`, `task_show`: probes calling `task_claim`, `task_approve` caught, `src` passes (M: a handler calls `task_claim`).
- [ ] AC-06 -- tail, turn: a stream on A receives `task.created`, `task.approved`, `task.claimed`, `task.run_reported` raised through the library, each `id: <seq>`, `event: <type>`, `data` the stored payload, in order (`tail.rs`); A's `/tasks/T-0003` waits for A's running read, not B's (`turn.rs`) (M: the tail keeping only `proposal.`; the handler outside `run`).
- [ ] AC-07 -- key sets: `daemon_keys.rs` writes the 17 from S's documents, the fixture 32 names; `daemonKeys.test.ts` checks all 32 against `provisional.ts` ("names exactly the thirty-two types") (M: a `TaskRun` key dropped from the fixture, red in Rust and Vitest).
- [ ] AC-08 -- client (stubbed `fetch`): `getTasks("harbor-sim")` requests exactly `/api/projects/harbor-sim/tasks`, `getTask("harbor-sim", "T-0107")` `/api/projects/harbor-sim/tasks/T-0107`; 404 `{id, reason}` resolves as data; a 404 error body, a 503 -> `ClientError` verbatim; the two comments of "Data", no `MISSING ENDPOINT` in `client.ts`, no `notServed(` in `http.ts` (`http.test.ts`, `client.test.ts`; M: `getTask` still refused).
- [ ] AC-09 -- events: `QUEUE_EVENT_TYPES` equals the fourteen of "Data" in order; on a stubbed `EventSource` each reaches `subscribe`'s handler as `{seq, type, payload}` (M: `task.refreshed` left out).
- [ ] AC-10 -- invalidation (`live.test.tsx`): alpha's Tasks with T-0109 open, T-0107 cached: `task.claimed {id: "T-0109"}` fetches alpha's list and T-0109 once each, T-0107 stale unfetched, no inbox, proposal or spec read, nothing of beta; `proposal.created` fetches the list and T-0109; a gap both; a payload without a string `id` every task of alpha (M: `["task", p, id]` left out; `proposal.*` not reading tasks).
- [ ] AC-11 -- screens over `HttpClient` (stubbed daemon): `#/harbor-sim/tasks` lists the served entries in their groups; T-0107's tabs from the served package, `bundle_hash` `null` and a `diff` `null` with `cut` false rendered (the quiet line); T-0099 its `reason` + "Back to tasks"; a 503 an alert + Retry; Home's task panel and the palette's tasks from `/tasks`; never "Not built yet" (M: `getTasks` still refused).
- [ ] AC-12 -- Inbox link, over the mock and the stubbed daemon: PR-0041's card, its review read, shows "Task" linking `#/harbor-sim/tasks/T-0107`; an unbound proposal "No task"; no Task fact before the review arrives; ui-tasks AC-08 whole again (M: the fact removed; the href from the summary).
- [ ] AC-13 -- types, citations: the types of "Data"; each task type cites as the table, asserted by `provisional.test.ts`; `grep -rn 'docs/features/task-package.md' ui/src` empty; `doc_pointers`, `anonymity`, `ui_policy` green (M: a type citing "Data" of a canon page -> `doc_pointers` red; `bundle_hash: string` -> `tsc` red on AC-11's null).
- [ ] AC-14 -- housekeeping: http and eval green; `pnpm lint` 0 warnings, `build`, `test` green; 17 packages, no new crate or dependency (`build_graph.rs` pins unchanged); READMEs http <= 10 240 B, UI <= 8 192 B; `CLAUDE.md` not grown; docs gate clean, worst W <= 108 468 B (M: an 18th package).

## Owner's manual check

`specengine-http --root <a scratch copy of this repository>`, `pnpm --dir ui dev`: Tasks matches `spec task list`; a task's six tabs; `spec task approve` in a terminal and an agent's `claim_task` each update the screen within 1 s, no reload; a bound proposal's Inbox card links its task.

## Open

**Working answers** (orchestrator 2026-10-07; the owner accepts or overrides):

- WA-1 `/tasks` takes `status` (repeated) = `--status`, the command's only option; the UI sends none.
- WA-2 `/tasks/:id` takes no query: no `--next` (an agent's `get_task {next}` is MCP), no brief text.
- WA-3 the generic exit map, no exception; a not-an-ID `:id` the CLI's 404, a look-alike its 503.
- WA-4 every `proposal.*` re-reads the task reads (created, rejected change `open_proposals`, `assumptions`; applied `stale`), not only `applied`.
- WA-5 `NOT_SERVED`, `ClientError`'s `notServed` and the screen's note stay for the next missing endpoint (health, symbols, Round); `http.ts` loses only its helper and `DAEMON_READ_GAP`; `notServed.test.tsx` uses a stub client.
- WA-6 `TaskPackage.bundle`, `.author` stay nullable in the UI: wider than Rust, harmless until generated types.
- WA-7 `Author` joins the key sets (the package's; the review's has the same four keys).
- WA-8 git as `inbox`: `process_git(env)`, the daemon's environment.
- WA-9 at shipping, `task-package.md` drops its kept "Description and interactions" and "Data" blocks; `test-engineer` first re-points the `crates/*/tests` comments citing them (`daemon_keys.rs`, store `format.rs`, `queue_state.rs`, `queue_intake.rs`, CLI `proposal_create.rs`, MCP `mcp_create.rs`) to the canon.

**Assumptions**: A1 unlike `check`, a task read needs `git` and the root in a worktree (else 503); A2 a 128-node package assembles in < 1 s here, measured at implementation; A3 a spec edit on disk raises no event: `stale` shows at the next read (ui-live A3).

## Out of scope

Any write: a task mover over HTTP, `POST .../tasks/:id/transition` (`decision-staging`, ADR-0035), staging; `--next`, the brief, bundle text over HTTP; task events in the mock; file-change or drift events; a server cache; generated types; new packages, crates, dependencies.

## Implementation

Not built. Doc lines at shipping (`spec-writer`): http README "Endpoints" (+2 rows), "Answers and statuses" (the door: `task_list`, `task_show` only), "Worktrees" (the compared place, git reads), "Live tail" (+ the nine `task.*`), "Known limits" (a package read's time in the turn), "Tests" (`tasks.rs`), net <= 57 B; `ui/README.md` State and "Contract seam" (no `HttpClient` member refused), net <= 11 B; 07 s3 line 124 (`tasks`, `tasks/:id` built); 08 s2 lines 32, 51; `docs/canon/task-package.md` "Versioning" last sentence; `task-package.md` compacted (WA-9); amendment lines in `ui-tasks` AC-01, AC-08 and `ui-home` line 20; `decision-staging.md` "Order".
