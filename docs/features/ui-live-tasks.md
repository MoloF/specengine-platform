---
class: spec
status: shipped
scope: [crates/specengine-http, ui]
ref: ui-live analysis 2026-10-07 ("Tasks later"), ui-live "Out of scope", task-package "Open", ui-tasks AC-08; working answers by the orchestrator (HEAD e55935d); 08 s2 Phase 4 on Phase 2's daemon
shipped: 2026-10-07
adrs: []
---

# UI live tasks: the tasks over HTTP

## Why

On the daemon, the UI's default client, Tasks, Home's task panel and the palette's task group said "Not built yet": `specengine-http` served neither `spec task list` nor `spec task show`, and the UI listened to no `task.*` event, so a claim or an approval made in a terminal never reached the screen. Now both are served read-only over the shipped CLI calls, the UI listens to the nine task events, the Inbox card's "Task" link is back (ui-tasks AC-08) and the UI's task types cite the canon (`docs/canon/tasks.md`, `docs/canon/task-package.md`). No new ADR: reads under the rules `daemon-read` and `ui-live` set (ADR-0033, 0034, 0012, 0006, 0008); owner actions stay commands to copy, agent actions MCP only. How it works now: `crates/specengine-http/README.md`, `ui/README.md`.

## Description and interactions

`GET .../tasks` -> `task_list(.., &TaskListRequest {statuses, git: process_git(env)})`, `GET .../tasks/:id` -> `task_show(.., &TaskShowRequest {id: Some(id), next: false, git: process_git(env)})`: GET only, behind the fence, in the project's turn after `answer.rs` `found`, the generic exit map. No other `task_` call: the daemon never moves a task (`docs/canon/tasks.md` "Commands"; agents over MCP). The tail already forwarded `task.*`. UI: `getTasks`, `getTask` fetch; nine `task.*` listeners; task invalidation; superseded reads aborted; the Inbox "Task" fact; screens unchanged. Roles: `rust-developer` `crates/specengine-http/src`; `ui-developer` `ui/src` and its Vitest; `test-engineer` `crates/specengine-http/tests`, `fixtures/daemon-keys.json`.

## Data

`/tasks`: `status` repeated, in order (= `--status`), each judged by `TaskStatus::parse`, a refusal listing `TaskStatus::ALL` (400) -> 200 `spec task list [--status S]... --json`. `/tasks/:id`: no query name, `:id` decoded as `proposals/:id` -> 200 `spec task show T --json`, uncut (`docs/canon/task-package.md` "Package"); exit 1 -> 404 `{id, reason}` (`docs/canon/tasks.md` "Commands"), as `{"id":"T-0099","reason":"no task T-0099 in this repository"}`; exit 2 -> 503.

### Key sets

`fixtures/daemon-keys.json` + 17 sets from state S's documents, 32 in all: `TaskList` 2, `TaskListEntry` 6, `TaskNotFound` 2, `TaskPackage` 25, `TaskTarget` 4, `TaskCriterion` 2, `TaskAssumption` 2, `TaskProposal` 6, `OwnerNote` 2, `SpecSnapshot` 3, `SnapshotPlace` 4, `SnapshotNode` 3, `SnapshotDiff` 5, `TaskClaim` 4, `TaskRun` 7, `TaskBundle` 3, `Author` 4.

### Client and events

```ts
/** GET /api/projects/:p/tasks (crates/specengine-http/README.md "Endpoints"; = spec task list --json) */
getTasks(project: string, signal?: AbortSignal): Promise<TaskList>;
/** GET /api/projects/:p/tasks/:id (crates/specengine-http/README.md "Endpoints"; = spec task show T --json, uncut; 404 the exit-1 document) */
getTask(project: string, id: string, signal?: AbortSignal): Promise<TaskPackage | TaskNotFound>;
getCheck(project: string): Promise<CheckReport>; // no signal: a walk in flight is never aborted
```

Every read (`getProjects`, `getInbox`, `getProposal`, `getTree`, `getNode`, `search`, `getBundle`, `getGraph`, `getTasks`, `getTask`) takes a trailing `signal?` passed to `fetch`, an aborted one rejecting with the abort error; each `queryFn` passes TanStack's `signal` but `useCheck` (`cancelRefetch: false` kept); the decision POST sends none; the mock ignores it; the UI sends no `status`. `QUEUE_EVENT_TYPES`: the five `proposal.*`, then the nine `task.*` in `docs/canon/tasks.md` "Store" order. Invalidated on `p` (fetched when on screen, else stale): a `task.*` with a string `id` `["tasks", p]` and `["task", p, id]` exact, without one every `["task", p]`; any `proposal.*` or a gap as before + `["tasks", p]`, `["task", p]`. Types (`provisional.ts`): `TaskBundle.bundle_hash: string | null`; each task type cites `tasks.md` "Commands", "Transitions", "Task-bound proposals" or `task-package.md` "Package", "Versioning", "Staleness", "Caps".

## Rules and edge cases

- WHEN `:id` is not a task ID THEN 404, `id` `null`; a look-alike 503 naming its Latin form; `T` in `[ids]`, no worktree or `git`, a newer queue schema -> 503; a corrupt row or another repository's task: 503 on `show`, a 200 with a note on `list` (the CLI's lines).
- A read writes nothing under the root, moves no task, raises no event; it may create, migrate `<slug>.db` and refresh the index. Two reads with no change are byte-identical.
- Every method but GET -> 405 `Allow: GET`; `POST .../tasks/:id/transition` stays an unknown route (`decision-staging`).
- A `task.*` reads only `p`'s task reads, never the inbox, a proposal or the spec; a burst supersedes: `proposal.applied` + two `task.refreshed` on the open task -> one package read answered, two aborted.
- The Inbox card's "Task" fact comes from the review document (`InboxEntry` stays 11 keys): absent until it is read and for the exit-1 document; `null` -> "No task"; else a link to the task.

## Acceptance criteria

Setup as `ui-live`: git copies of `fixtures/spec-a` (A), `-b` (B), a scratch `HOME`, a `--port 0` daemon per test. **State S** (`crates/specengine-http/tests/task_state/mod.rs`, the CLI library, a consent closure): T-0001 `draft`; T-0002 approved, its target then edited; T-0003 planned with a criterion, `changes --note`, re-planned, approved, a question with a working answer on its target, claimed, a run reported; T-0004 cancelled. "The trial": `tasks.rs` `ac01_to_ac05_the_task_routes_are_the_clis_documents_and_change_nothing`. M: the mutation, seen red.

- [x] AC-01 -- list parity: A and B in S: `/tasks`, `?status=ready`, `?status=in_progress&status=draft`, a state twice -> 200, byte-equal to `spec --root R task list [--status S]... --json` less its final LF (the trial; M: `status` ignored).
- [x] AC-02 -- show parity: T-0001 to T-0004 on A and B -> 200, byte-equal to `spec --root R task show T --json` less its LF; two reads byte-identical; T-0002 `stale` true, one `snapshot_diff` entry (the trial; M: `next: true`).
- [x] AC-03 -- refusals, **corrected** (the route list writes `tasks/<id>`, as `proposals/<id>`): `/tasks/T-0099` -> 404 exactly the document of "Data"; `/tasks/foo` 404, `id` `null`; `/tasks/%D0%A2-0001` 503 naming `T-0001`; `/tasks/`, `/tasks/T-0001/transition` 404 listing the routes, `tasks`, `tasks/<id>` among them; a copy of A without `.git`, one whose `[ids]` claims `T`: 503 on both routes; a corrupt row, another repository's task: 503 on show, a note on list (the trial, `ac03_no_worktree_and_t_in_ids_are_a_503_on_both_routes`, `ac03_a_corrupt_row_or_another_repositorys_task_is_a_503_on_show_a_note_on_list`, `methods.rs`, `projects.rs`; M: exit 1 mapped to 200).
- [x] AC-04 -- query: `?status=triage`, `?status=`, `?status=Ready`, `?state=ready`, `?x=1`, `%FF`, `/tasks/T-0001?next=true` -> 400 naming the parameter, a fresh `HOME` left empty (`ac04_a_bad_query_is_a_400_naming_the_parameter_and_reads_nothing`); `no_domain.rs` adds the ten names of `TaskStatus::ALL`, `src` green (`ac04_the_scan_sees_a_task_state_in_a_literal`; M: a handler comparing `"ready"`).
- [x] AC-05 -- read-only: after AC-01 to AC-04 `git status --porcelain --ignored` empty, `HEAD`, the highest event `seq`, `spec export state`'s bytes unchanged (the trial; `door.rs` `ac03_no_endpoint_changes_the_repository_or_the_queue`); POST, PUT, DELETE, HEAD on both -> 405 `Allow: GET` (`methods.rs`); `door.rs` forbids every `task_` name but `task_list`, `task_show`, probes calling `task_claim`, `task_approve` caught (`ac05_the_scan_catches_every_task_call_but_list_and_show`; M: a handler calls `task_claim`).
- [x] AC-06 -- tail, turn: a stream on A receives `task.created`, `.approved`, `.claimed`, `.run_reported` raised through the library, each `id: <seq>`, `event: <type>`, `data` the stored payload, in order (`tail.rs` `ac06_a_stream_receives_the_task_events_raised_through_the_library`); A's `/tasks/T-0003` waits for A's running read, not B's (`turn.rs` `ac06_a_task_read_waits_for_its_own_projects_reads_not_anothers`) (M: the tail keeping only `proposal.`; `show`, `list` each outside the turn).
- [x] AC-07 -- key sets: `daemon_keys.rs` writes the 17 from S's documents, the fixture 32 names (`ac09_the_daemons_key_sets_are_fixtures_daemon_keys_json`); `daemonKeys.test.ts` checks all 32 against `provisional.ts` ("names exactly the thirty-two types"; M: a `TaskRun` key dropped from the fixture, red in Rust and Vitest).
- [x] AC-08 -- client (stubbed `fetch`): `getTasks("harbor-sim")` requests exactly `/api/projects/harbor-sim/tasks`, `getTask("harbor-sim", "T-0107")` `.../tasks/T-0107`; 404 `{id, reason}` resolves as data; a 404 error body, a 503 -> `ClientError` verbatim; the comments of "Data", no `MISSING ENDPOINT` in `client.ts`, no `notServed(` in `http.ts`; each read passes its `AbortSignal` to `fetch`, the check and the decision none, an aborted read rejects with the abort error (`http.test.ts`, `client.test.ts`, `queries.test.tsx` "abort signals"; M: `getTask` still refused; the signal not passed to `fetch`; `getTask`'s `queryFn` without it; the check given one).
- [x] AC-09 -- events: `QUEUE_EVENT_TYPES` equals the fourteen of "Data" in order; on a stubbed `EventSource` each reaches `subscribe`'s handler as `{seq, type, payload}` (`http.test.ts`; M: `task.refreshed` left out).
- [x] AC-10 -- invalidation (`live.test.tsx` "the task reads on the live tail", "a burst of events on the live tail"): alpha's Tasks with T-0109 open, T-0107 cached: `task.claimed {id: "T-0109"}` fetches alpha's list and T-0109 once each, T-0107 stale unfetched, no inbox, proposal or spec read, nothing of beta; each `proposal.*` the list and T-0109; a gap both; a payload without a string `id` every task of alpha; `proposal.applied` + two `task.refreshed`: one package read answered, two aborted; a burst of applies never aborts the check (M: `["task", p, id]` left out; `proposal.*` not reading tasks).
- [x] AC-11 -- screens over `HttpClient` (`liveScreens.test.tsx` "Tasks over the daemon", 6 tests): the served entries in their groups; T-0107's tabs, `bundle_hash` `null`, a `diff` `null` with `cut` false as the quiet line; T-0099 its `reason` + "Back to tasks"; a 503 an alert + Retry; Home's task panel and the palette's tasks from `/tasks`; never "Not built yet" (M: `getTasks` still refused).
- [x] AC-12 -- Inbox link (`InboxView.test.tsx` "the card's Task fact", `tasks.smoke.test.tsx`, `liveScreens.test.tsx`): PR-0041's card, its review read, links `#/harbor-sim/tasks/T-0107`; an unbound proposal "No task"; no fact before the review arrives or for the exit-1 document; ui-tasks AC-08 whole again (M: the fact removed; the href from the summary).
- [x] AC-13 -- types, citations: as "Data", asserted by `provisional.test.ts` "the task types"; `grep -rn 'docs/features/task-package.md' ui/src` empty; `doc_pointers`, `anonymity`, `ui_policy` green (M: a type citing a canon page's "Data" -> Vitest and `doc_pointers` red; `bundle_hash: string` -> `tsc` red).
- [x] AC-14 -- housekeeping: http 60/60, http + CLI + eval 941 passed; `pnpm lint` 0 warnings, `build`, `test` (63 files, 1 412 tests) green; 17 packages, no new crate or dependency; READMEs http 10 188, UI 8 192 B; `CLAUDE.md` untouched; docs gate clean, worst W <= 108 468 B (M: an 18th package).

## Owner's manual check

Open: `specengine-http --root <a scratch copy of this repository>`, `pnpm --dir ui dev`: Tasks matches `spec task list`; a task's six tabs; `spec task approve` in a terminal and an agent's `claim_task` each update the screen within 1 s, no reload; a bound proposal's Inbox card links its task.

## Open

**Working answers** (orchestrator 2026-10-07, open to the owner): WA-1 `status` = `--status`, the UI sends none; WA-2 `/tasks/:id` takes no query (`--next`, the brief: MCP `get_task`); WA-3 the generic exit map; WA-4 every `proposal.*` re-reads the task reads; WA-5 `NOT_SERVED`, `ClientError.notServed`, "Not built yet" kept for the next missing endpoint, `notServed.test.tsx` on a stub client; WA-6 `TaskPackage.bundle`, `.author` nullable in the UI; WA-7 `Author` a key set; WA-8 `process_git(env)`, as `inbox`; WA-9 compacting `task-package.md`'s "Description and interactions", "Data": **deferred**, `crates/*/tests` still cite them (http `daemon_keys.rs`, store `format.rs`, `queue_state.rs`, `queue_intake.rs`, `queue_tasks.rs`, CLI `proposal_create.rs`, MCP `mcp_create.rs`, `common/read.rs`): `test-engineer` re-points them first.

**Assumptions**: A1 a task read needs `git` and a worktree, unlike `check`; A2 **corrected**: a 128-node package, every node edited, reads in 0.99 s warm, 1.49 s cold (release CLI), 1.32-1.36 s through the debug daemon, one `git diff --no-index` per changed node (~7 ms; 0.08 s with none): under 1 s only warm, a known limit; A3 a spec edit on disk raises no event: `stale` shows at the next read.

## Out of scope

Any write over HTTP (a task mover, `POST .../tasks/:id/transition`: `decision-staging`, ADR-0035); `--next`, the brief, bundle text over HTTP; task events in the mock; file-change or drift events; a server cache; generated types; new packages, crates, dependencies.

## Implementation

Canon: http and UI READMEs, `task-package.md` "Versioning"; 07 s3, 08 s2; amendment lines in `ui-tasks` AC-01, AC-08, `ui-home` "Data", `ui-live` AC-11, `decision-staging` "Order". One Rust, two UI iterations; review 1 accepted (m1, n2 UI iteration 2; m2, n1, n3 here).

| Module | What it does |
|---|---|
| http `app.rs` | routes `tasks`, `tasks/{id}` via `read()`; `task_list`, `task_show` in the turn; the 404 route list; the one-door comment |
| http `args.rs` | `Args::statuses`: repeated `status` by `TaskStatus::parse`, the refusal listing `TaskStatus::ALL` |
| UI `http.ts`, `client.ts` | `getTasks`, `getTask`; a trailing `signal?` on every read, to `fetch`; `QUEUE_EVENT_TYPES` 14; `notServed()`, `DAEMON_READ_GAP` gone |
| UI `queries.ts` | `useLiveQueue`'s task invalidation; each `queryFn` passes `signal` but `useCheck` |
| UI `ProposalCard.tsx` | the "Task" fact from the review document |
| UI `provisional.ts`, `actions.ts`, `labels.ts`, `answer.ts`, `SpecChangesPanel.tsx`, mocks, `tokens.css` | types and citations on the canon |

Tests: http `tasks.rs`, `task_state/mod.rs` new; `door`, `no_domain`, `daemon_keys`, `methods`, `projects`, `tail`, `turn` changed; Rust mutations red: `status` ignored, `next: true`, exit 1 -> 200, a `"ready"` literal, `task_claim` called, the tail `proposal.` only, `show`, `list` outside the turn. Trials 73/73 on A and B in state S.

Deviations: no CLI, `answer.rs` or `tail.rs` change; the no-git 503 is the CLI's line verbatim; AC-03 `tasks/<id>`; A2 corrected; WA-9 deferred; `getCheck` takes no signal; the "a task" case of `notServed.test.tsx` shows two notes on its stub client; `MockClient.getBundle`'s comment and informal task-package mentions (`tokens.css`, mocks, test names) re-pointed, the three "task-package G4" ones to `task-package.md` "Package". Owner: the manual check, WA-1 to WA-9.
