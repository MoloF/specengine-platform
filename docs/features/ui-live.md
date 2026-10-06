---
class: spec
status: draft
scope: [crates/specengine-http, crates/specengine-cli, ui]
ref: ui-live analysis 2026-10-07 (HEAD 930cdea), every recommendation and Q1-Q6 working answer accepted; 08 s2 Phase 4 on Phase 2's daemon
adrs: []
---

# UI live: graph and check over HTTP

## Why

On the daemon, the UI's default client, Graph and Health say "Not built yet": `HttpClient.getGraph`, `getCheck` reject unsent (`notServed`); `specengine-http` serves neither `spec graph` nor `spec check`. This slice serves both over the shipped CLI calls, `graph` uncut, `check` the plain run with every verdict a 200 document; the client switches to them, the live tail re-reads them. A thin follow-up to `daemon-read`, Phase 4 on Phase 2's daemon (08 s2); tasks wait for `ui-live-tasks` (no `spec task list|show` yet).

Rests on ADR-0033 and `docs/canon/architecture.md#ui`, ADR-0012 (`blocked` never a hold), ADR-0034 (the fence), ADR-0006 (no hook or CI reads it), ADR-0008 (no verdict, link type or kind in the daemon), `#storage`. No new ADR: the amended rules (one door, the exit map) were `daemon-read` working answers.

**Working answers** (orchestrator 2026-10-07, the owner's standing instruction; open to his review): **Q1** tasks routes, `task.*` listeners, the Inbox "Task" link: `ui-live-tasks`, right after `task-package`; **Q2** `check` re-read on `proposal.applied` or a gap only while Health is on screen; **Q3** `check` takes the project's turn; **Q4** `graph` uncut on the server, the UI applies its drawing limits (`ui-graph` AC-09, AC-10); **Q5** order: `proposal-kinds` (committed first), this, `task-package`, `ui-live-tasks`, `decision-staging`; **Q6** a discovery refusal is a 503, as everywhere. **Assumptions**: A1 the REF a query value, not a path; A2 debt judged by the daemon's UTC date per call; A3 a direct file edit raises no event: seen at the next read ("Check again", other options, a remount: no `staleTime`, `provider.tsx`); A4 `depth=-1` passes `args.rs` `integer`: CLI exit 2, 503; A5 a walk here < 1 s (`docs/canon/mcp-read.md` "Latency").

Roles: `rust-developer` `crates/specengine-http/src`, CLI `graph.rs`, `lib.rs`; `ui-developer` `ui/src`; `test-engineer` tests, `fixtures/daemon-keys.json`. Unchanged: CLI text and `--json`, MCP, store, hook, CI, `plugin/`; no new crate, dependency or UI package.

## Description and interactions

**Server**: two read routes beside `daemon-read`'s (`crates/specengine-http/README.md`), behind the fence, GET only (else 405), `no-store`, one CLI library call each in the project's turn, after `answer.rs` `found` (a changed slug, a config `discover` refuses: 503):

- `GET /api/projects/:p/graph` -> `graph_with_view(env, globals, &request, View::Browser)`; it refreshes the index like `tree`;
- `GET /api/projects/:p/check` -> `check(env, globals, &CheckRequest::default())`; no database, data directory or git, yet in the turn (Q3): one walk in flight per project, a queued request whose client left dropped unrun (`tests/turn.rs`).

**CLI**: `graph_with_view(&Env, &Globals, &GraphRequest, View) -> Result<GraphOutcome, CliError>`; `graph` = it with `View::Capped`, bytes unchanged. `Browser`: `shown_nodes`, `shown_edges` the full counts, `truncated` false; `Capped` cuts as now (`docs/canon/spec-cli-graph.md` "Exit, cap, determinism"; a node cut drops every edge, `graph.rs`). No `check_with_view`: `check`'s JSON is never cut (W-1, `docs/canon/spec-check-cli.md` "spec check").

**UI**: `getGraph`, `getCheck` fetch; Graph and Health keep their states (loading, error + Retry, the exit-1 reason); the tail re-reads both.

## Data

### Graph: `GET /api/projects/:p/graph`

Query, strict (README "Endpoints", **Query**), names = the JSON echo keys and `ui-graph`'s options: `ref` (required, once, any text; `%23` for `#`) -> `reference`; `impact` (`true`|`false`, once; absent false); `types` (repeated, any text, in order); `depth` (decimal integer, once); `archive` (`true`|`false`, once; absent false). 400, error body, nothing run: `ref` missing; a scalar repeated; `impact=yes`, `depth=x`, a bad `%`; an unknown name (`x=1`: ``... it takes ref, impact, types, depth, archive``). A link type is never validated (ADR-0008): an unknown one follows nothing, as on the CLI. Statuses, the generic map (`answer.rs` `run`): exit 0 -> 200, the document; 1 -> 404, the exit-1 document (`reason` set: an unknown REF, `ref=` empty); 2 -> 503, the error body (`depth` < 0, a look-alike ID, a `project:` REF, `HOME`).

Example (`ui-graph` "Data"): `GET /api/projects/harbor-sim/graph?ref=MEC-TIDES%23RULE-TIDE-WINDOW&impact=true&types=depends_on&types=constrains&depth=3` -> 200, `spec --root R graph 'MEC-TIDES#RULE-TIDE-WINDOW' --impact --type depends_on --type constrains --depth 3 --json` stdout without its final LF, every node and edge.

`GraphView` (`docs/canon/spec-cli-graph.md` "spec graph"), 11 keys, absent = `null`: `{ref, reason, impact, types: [{type, direction}], depth, archive, notes, left_out: {generated, tier3}, truncated, nodes: [{id, kind, title, path, line, distance, archived}], edges: [{src, type, dst, written, path, line, state, reason}]}`. Browser view: `truncated` false, the CLI's keys and order; uncut, the bytes equal.

### Check: `GET /api/projects/:p/check`

No query: any name -> 400 (``... it takes no query name``); nothing maps to `--staged`, `--changed` (git), `--baseline` (a client path), `--debt` (text only). `CheckRequest::default()`: the working tree on disk, untracked and ignored files included, `specengine.toml`, `.spec-debt.toml` from the root, `today_utc()` per call.

**The check exception** (`answer.rs`), keyed on the outcome being `Outcome::Check`, never on a verdict word: `Ok` -> 200 whatever `exit()` (clean, observed 0; blocked 1; cannot-check 2, causes in `cannot_check`); `Err(CliError)` (discovery, Q6) or a changed slug -> 503. The generic map would make `blocked` a 404 and `cannot-check` a 503 losing its causes (`CheckOutcome::exit`). Body = `spec --root R check --json` stdout less its final LF, same `HOME`; stderr `note:` lines not carried. No cache: the answer depends on disk and the date; no watcher.

`CheckReport` (`docs/canon/spec-check.md` "Output"), plain run, 6 keys: `{mode, verdict, counts: {documents, errors, warnings, debt, expired, stale, worst_w_bytes}, findings: [{code, severity, path, line, subject, message, fix?, debt?}], stale: [{code, path, subject, reason, expires, line}], cannot_check: [{path, message}]}`; `?` omitted when unset, never `null` (W-1); never `new_debt`, `introduced`. 200:

```json
{"mode":"enforce","verdict":"blocked","counts":{"documents":9,"errors":1,"warnings":0,"debt":0,"expired":0,"stale":0,"worst_w_bytes":24576},
"findings":[{"code":"key-missing","severity":"error","path":"docs/spec/movement/sprint.md","line":1,"subject":"status","message":"..."}],"stale":[],"cannot_check":[]}
```

### The one door, amended

`daemon-read`'s rule (no handler "indexes or checks") becomes: no handler calls `approve`, `approve_with`, `reject`, `propose*`, `import_state`, `export_*`, `init`, `index`; `check` only as the plain run. `tests/door.rs` `forbidden`: `check` leaves; `Staged`, `Changed`, `baseline` join (a git mode or client path named in `src`), with a positive control, a probe calling `CheckedTree::Changed`. The only writes stay the reads' index refresh (`#storage`).

### UI (`ui/README.md` "Contract seam")

- `client.ts`: the `MISSING ENDPOINT` comments of `getGraph`, `getCheck` become `/** GET /api/projects/:p/graph (crates/specengine-http/README.md "Endpoints"; the browser view, uncut) */` and `/** GET /api/projects/:p/check (crates/specengine-http/README.md "Endpoints"; every verdict a 200 document) */`, asserted by `client.test.ts`; `getTasks`, `getTask` keep theirs.
- `http.ts`: `getGraph(p, options)` -> `GET /api/projects/<p>/graph?` + `queryOf` of `ref`, `impact`, `types`, `depth`, `archive` (absent omitted, `types` repeated, `impact`, `archive` only `true`: `ui-graph` "Data"); `getCheck(p)` -> `GET /api/projects/<p>/check`. A 200 is data, a `blocked` report too; a graph 404 document data (the GET-404 branch); other non-2xx `ClientError` verbatim. `getTasks`, `getTask` still `notServed`, `DAEMON_READ_GAP` -> `docs/features/ui-live.md "Out of scope"`.
- `queries.ts`: `READS_OF_THE_SPEC` = `["tree", "node", "search", "bundle", "graph", "check"]`; only reads on screen refetch, the rest marked stale (Q2). Query keys, `useCheck`'s options (no focus, reconnect, interval refetch), `READS_AFTER_DECISION` (a decision reads no `check`), event types unchanged.
- `provisional.ts` unchanged. `fixtures/daemon-keys.json` gains nine sets, written by `tests/daemon_keys.rs` from the daemon: `GraphView` 11, `GraphNode` 7, `GraphEdge` 8, `FollowedType` 2 (a graph on A with edges); `CheckReport` 6, `CheckCounts` 7, `CheckFinding` 8, `DebtEntry` 6 (`stale`), `CheckCause` 2 (checks on copies of A with a `fix` finding (a look-alike ID), one in debt, a stale entry, a cannot-check). `CheckFinding` is the union of its instances in core's field order (`one_key_set` needs equal sets, panics on none); `expires` dates far from today. `daemonKeys.test.ts` compares all 15; the base-only keys (`CheckReport.new_debt`, `CheckCounts.introduced`, `.new_debt`, `CheckFinding.introduced`) listed once as never served.

## Rules and edge cases

- WHEN the config no longer parses THEN both routes 503 with `discover`'s line, where `spec check` prints a cannot-check report (a parity gap, Q6).
- WHEN a graph passes `OUTPUT_CAP_CHARS` THEN the daemon sends it whole, possibly megabytes (Q4).
- WHEN a `check` runs THEN the project's other reads wait for it and it for them (Q3); another project's do not.
- WHEN a file changes on disk without a queue event THEN nothing is pushed; the next read shows it (A3).
- WHEN `proposal.applied` or a gap reaches `p` THEN `p`'s graph and check on screen read again, once each; other `proposal.*` events read neither.
- Both read the registered root's files on disk, uncommitted edits included, never `HEAD` or the git index; neither writes under a root.

## Acceptance criteria

Setup as `daemon-read`: git copies of `fixtures/spec-a` (A), `fixtures/spec-b` (B), a scratch `HOME`, a `--port 0` daemon per test (http `tests/common/mod.rs`). M: the mutation turning it red.

- [ ] AC-01 -- graph parity: on A, `MEC-SPRINT`, `MEC-STAMINA#RULE-STAM-REGEN` (`%23`), `docs/spec/movement/sprint.md` (`%2F`) x {none, `impact=true`, `types=depends_on&types=constrains`, `depth=1`, `archive=true`}: 200, byte-equal to `spec --root A graph REF <flags> --json` stdout less its final LF, same `HOME` (M: `impact` ignored).
- [ ] AC-02 -- uncut: A plus a generated document linking enough sections that the CLI cuts (its JSON `truncated` true, `edges` `[]`): the daemon's `truncated` false, its node and edge counts = the CLI text's `nodes <n>, edges <e>` (M: `View::Capped` in the handler; `graph` given `Browser`).
- [ ] AC-03 -- graph query: an unknown REF -> 404, byte-equal to the CLI's exit-1 document; `ref` missing, `ref` twice, `impact=yes`, `depth=x`, `x=1` -> 400 naming the parameter, no data directory in a fresh `HOME`; `depth=-1` -> 503 (M: `ref` optional).
- [ ] AC-04 -- check verdicts: copies of A clean, observed (`[check] mode = "observe"` + an error), blocked (enforce + an error), cannot-check (an invalid `.spec-debt.toml`): each 200, byte-equal to `spec --root R check --json` stdout less its final LF (M: the generic exit map).
- [ ] AC-05 -- reach: `?staged=true`, `?baseline=x`, `?debt=true` -> 400; the daemon started with no `git` on `PATH`: `/check` 200; a fresh `HOME`: no data directory after; `git status --porcelain` empty, `HEAD` unchanged (M: `staged` wired to `CheckedTree::Staged`).
- [ ] AC-06 -- turn: as `turn.rs`, `/check` requests queued behind a held turn, clients gone, run no walk; a `/check` on A waits for A's running read, not B's (M: `check` outside the turn).
- [ ] AC-07 -- door: `door.rs` catches a probe calling `CheckedTree::Changed` and one naming `baseline`; `approve`, `reject`, `propose`, `import_state`, `export_*`, `init`, `index` still caught; `src` passes (M: a handler calls `index`).
- [ ] AC-08 -- no domain: no string literal in `crates/specengine-http/src` holds, as a word, a verdict (`clean`, `observed`, `blocked`, `cannot-check`), a `LINK_TYPES` name or a kind (M: a branch on `"blocked"`).
- [ ] AC-09 -- key sets: `daemon_keys.rs` writes the nine; the fixture holds 15 names; `daemonKeys.test.ts` checks all 15 against `provisional.ts` (M: a `GraphEdge` key dropped from `provisional.ts`).
- [ ] AC-10 -- client (Vitest, stubbed `fetch`): `getGraph("harbor-sim", {ref: "MEC-TIDES#RULE-TIDE-WINDOW", impact: true, types: ["depends_on", "constrains"], depth: 3})` requests exactly the example URL; `getCheck("harbor-sim")` `/api/projects/harbor-sim/check`; a 200 `blocked` report resolves as data; a 503 -> `ClientError` verbatim (M: `types` dropped).
- [ ] AC-11 -- `getTasks`, `getTask` -> `ClientError {status: 501, notServed: true}`, no `fetch`, naming `docs/features/ui-live.md "Out of scope"` (M: `getTasks` fetching).
- [ ] AC-12 -- live (stubbed `EventSource`): `proposal.applied` on `p` refetches the graph on screen once, with Health on screen the check once; `proposal.created` neither; a gap both; a UI decision reads no `check`; another project's event nothing (M: `graph` left out of `READS_OF_THE_SPEC`).
- [ ] AC-13 -- screens over `HttpClient` (stubbed `fetch`): Health on a 200 `blocked` report shows "Fails the check" and its findings, on `cannot-check` "Could not check" and the causes verbatim; Graph on a 404 document its `reason`; no "Not built yet" (M: `getCheck` still `notServed`).
- [ ] AC-14 -- housekeeping: http tests, `mcp_read`, `mcp_size`, eval `build_graph.rs` (pins unchanged), `ui_policy`, `anonymity`, `doc_pointers` green; `pnpm lint` (0 warnings), `build`, `test`; the UI's 17 packages (M: an 18th); http, CLI READMEs <= 10 240 B, `ui/README.md` <= 8 192 B, `CLAUDE.md` not grown; docs gate clean, worst W <= the start's.
- [ ] AC-15 -- owner's manual check: `specengine-http --root <this repository>`, `pnpm --dir ui dev`: Health's verdict and counts match `spec check`; a Graph matches `spec graph`; a terminal approval that applies refreshes both screens within 1 s, no reload. Open.

## Out of scope

`ui-live-tasks`, after `task-package`: `GET .../tasks`, `.../tasks/:id` (404 `TaskNotFound`), nine `task.*` listeners (the tail forwards any stored type), `["tasks"|"task", p]` invalidation, the Inbox "Task" link, `ui/src` citations moved off `task-package.md`. Staging events; file-change, drift events; `health` (Phase 3), `symbols`, Round; git modes or a baseline over HTTP; a server cache or watcher; a graph MCP tool; new packages, crates, dependencies.

## Open

- The owner's review of Q1-Q6 (working answers); AC-15.

## Implementation

Not built. **At shipping** (`spec-writer`), in place, caps never raised (overflow moves down a tier):

- `crates/specengine-http/README.md` (10 221 B): "Endpoints" two rows; "Browser view"; "Answers and statuses" the check exception; "One door" (`check` only plain; `Staged`, `Changed`, `baseline` forbidden); "Worktrees"; "Concurrency"; "Known limits" (`discover` refusing: 503, the CLI a report; stderr `note:` lost; an uncut graph's size; a walk delays the project's reads); "Tests".
- CLI README (10 238 B, net <= +2): "API" `graph_with_view`, "Output and the cap" `View`. `ui/README.md` (8 184 of 8 192 B): the state line. 07 s3: `graph`, `check` built. 08 s2: Phase 2's list, Phase 4's "next".
- Drafts: `proposal-kinds.md` "Why" **Order** if still a draft; `decision-staging.md` "Data" one-door list (`check` plain allowed). Done 2026-10-07: `decision-staging.md` **Order**, `task-package.md` order pointer and `ui-live-tasks`.
- Shipped specs, one "Amendment" line each: `ui-graph.md` AC-01, `ui-health.md` AC-01 (the comment, the 501), `daemon-read.md` AC-03 (`check` called, plain), AC-09 (15 key sets).
- `CLAUDE.md` not grown; this spec compacted to "Why", criteria, a summary <= 3 KB, keeping "Out of scope" (cited by `http.ts`).
