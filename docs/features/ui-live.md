---
class: spec
status: shipped
scope: [crates/specengine-http, crates/specengine-cli, ui]
ref: ui-live analysis 2026-10-07 (HEAD 930cdea), every recommendation and Q1-Q6 working answer accepted; 08 s2 Phase 4 on Phase 2's daemon
shipped: 2026-10-07
adrs: []
---

# UI live: graph and check over HTTP

## Why

On the daemon, the UI's default client, Graph and Health said "Not built yet": `specengine-http` served neither `spec graph` nor `spec check`. Now both are served over the shipped CLI calls, `graph` uncut, `check` the plain run with every verdict a 200 document; the client fetches them, the live tail re-reads them. Tasks wait for `ui-live-tasks`. No new ADR (ADR-0033, 0012, 0034, 0006, 0008): the amended rules (one door, the exit map) were `daemon-read` working answers. How it works now: `crates/specengine-http/README.md`, CLI README "Output and the cap", `ui/README.md`.

**Working answers** (orchestrator 2026-10-07, open to the owner's review): Q1 tasks routes, `task.*` listeners, the Inbox "Task" link: `ui-live-tasks`; Q2 `check` re-read on `proposal.applied` or a gap only while Health is on screen; Q3 `check` takes the project's turn; Q4 `graph` uncut, the UI applies its drawing limits; Q5 order `proposal-kinds`, this, `task-package`, `ui-live-tasks`, `decision-staging`; Q6 only a config `discover` refuses (TOML syntax, the project tables) answers 503, a bad `[check]` table is a 200 cannot-check report. A1-A5: the REF a query value; debt by the UTC date per call; a file edit raises no event; `depth=-1` a 503; a walk < 1 s (4-52 ms).

## Description and interactions

`GET /api/projects/:p/graph` -> `graph_with_view(.., View::Browser)`, `GET .../check` -> `check(.., &CheckRequest::default())`: GET only, behind the fence, one call each in the project's turn after `answer.rs` `found`. CLI `graph` = `graph_with_view(.., View::Capped)`, bytes unchanged.

## Data

Graph query names = `spec graph`'s JSON echo keys: `ref` (required), `impact`, `types` (repeated, in order, never validated), `depth`, `archive`; the generic exit map. Check: no query name; the tree on disk, untracked and ignored files, the date per call, no database or git. **The check exception** (`answer.rs` `answered`), keyed on `Outcome::Check`, never on a verdict word: every report 200; a `CliError` or a changed slug 503. `CheckReport` (`docs/canon/spec-check.md` "Findings, debt, verdict"), plain: no `new_debt`, `introduced`. `fixtures/daemon-keys.json`: 15 sets (+9: a `stale` entry, a `cannot_check` cause among them), base-only keys listed once as never served. UI: a 200 report and a graph 404 are data; the tasks stay `notServed`.

### The one door, amended

No handler calls `approve`, `reject`, `propose*`, `import_state`, `export_*`, `init`, `index`; `check` only plain. `tests/door.rs` forbids `Staged`, `Changed`, `baseline` instead of `check`.

## Rules and edge cases

- WHEN `proposal.applied` or a gap reaches `p` THEN `p`'s graph on screen and, Health on screen, its check are read again, once; other events neither; a check in flight is kept, not run twice.
- Both read the root's files on disk, never `HEAD`, and write nothing under it; a file edit is seen at the next read (A3).

## Acceptance criteria

Setup as `daemon-read`: git copies of `fixtures/spec-a` (A), `-b` (B), a scratch `HOME`, a `--port 0` daemon per test; Rust tests in `crates/specengine-http/tests/`. M: the mutation, seen red.

- [x] AC-01 -- graph parity: on A, 3 REFs (`%23`, `%2F` among them) x {none, `impact=true`, two `types`, `depth=1`, `archive=true`}, `types` order, an unknown type: 200, byte-equal to `spec --root A graph REF <flags> --json` less its final LF (`graph.rs` `ac01_every_graph_is_the_clis_document_byte_for_byte`; M: `impact` ignored).
- [x] AC-02 -- uncut: A + 700 sections linked `depends_on`: the CLI's JSON cut (`truncated` true, `edges` `[]`), the daemon's `truncated` false, counts = the CLI text's `nodes 701, edges 700` (`ac02_the_daemon_sends_whole_the_graph_the_cli_cuts`; M: `Capped` in the handler).
- [x] AC-03 -- graph query: unknown REFs, `ref=` -> 404 = the CLI's exit-1 document; 16 malformed queries -> exact 400s, `HOME` left empty; `depth=-1`, a look-alike, `shared:DEC-0023` -> 503, the CLI's line (`ac03_the_graph_query_404_400_503`; M: `ref` optional).
- [x] AC-04 -- check verdicts: copies of A clean, observed, blocked, cannot-check: each 200, byte-equal to `spec --root R check --json` (exits 0, 0, 1, 2) (`check.rs` `ac04_every_verdict_is_a_200_document_byte_equal_to_the_cli`; M: the generic exit map).
- [x] AC-05 -- reach: 8 query names -> 400; no `git` on `PATH`: `/check` the CLI's bytes; `HOME` left empty; `git status --porcelain --ignored` empty, `HEAD` unchanged (`ac05_check_takes_no_query_needs_no_git_and_writes_nothing`, `check_and_graph_read_the_working_tree_on_disk_not_head`, `a_config_that_no_longer_parses_is_a_503_on_check_and_graph`; M: `staged` wired).
- [x] AC-06 -- turn: queued `/check`s whose clients left run no walk (CPU ratio 0.28; 1.01 mutated); A's check waits for A's read, not B's (`turn.rs` `ac06_a_queued_check_dropped_by_its_client_never_walks`, `ac06_a_check_waits_for_its_own_projects_reads_not_anothers`; M: `check` outside the turn).
- [x] AC-07 -- door: probes calling `CheckedTree::Changed`, naming `baseline` caught, the writers still; `src` passes; no endpoint changes the repository or queue (`door.rs` `ac07_the_scan_catches_a_checks_git_mode_or_baseline_and_every_writer`, `ac03_*`; M: a handler calls `index`).
- [x] AC-08 -- no domain, **corrected**: no string literal in `crates/specengine-http/src` holds, as a case-insensitive word, a verdict (`clean`, `observed`, `blocked`, `cannot-check`), a `LINK_TYPES` name, or a kind of the union of every `kind = "..."` in the root and `fixtures/` `specengine.toml`s (8, 15 kinds); exempt only `decision` as a segment of a route-path literal (`app.rs` `"/api/projects/{p}/proposals/{id}/decision"`, `strip_suffix("/decision")`) (`no_domain.rs` `ac08_*`, 2 tests; M: a branch on `"blocked"`).
- [x] AC-09 -- key sets: `daemon_keys.rs` writes the nine, the fixture 15 names; `daemonKeys.test.ts` checks all 15 against `provisional.ts`, each required when the fixture exists (`ac09_the_daemons_key_sets_are_fixtures_daemon_keys_json`, "names exactly the fifteen types"; M: a fixture key dropped, red in Vitest and Rust). **Corrected**: a `GraphEdge` key dropped from `provisional.ts` is caught by `tsc` (`pnpm build`), not Vitest.
- [x] AC-10 -- client (stubbed `fetch`): `getGraph("harbor-sim", {ref: "MEC-TIDES#RULE-TIDE-WINDOW", impact: true, types: ["depends_on", "constrains"], depth: 3})` requests exactly `/api/projects/harbor-sim/graph?ref=MEC-TIDES%23RULE-TIDE-WINDOW&impact=true&types=depends_on&types=constrains&depth=3`; each verdict's 200 data; 503, 400 -> `ClientError` verbatim (`http.test.ts` "each method hits its URL", "resolves the check's 200 `%s` report as data, never a refusal"; M: `types` dropped).
- [x] AC-11 -- `getTasks`, `getTask` -> 501 `notServed`, no `fetch`, naming `docs/features/ui-live.md "Out of scope"` (`http.test.ts`, `client.test.ts`, `notServed.test.tsx`; M: `getTasks` fetching). Superseded by `ui-live-tasks` AC-08: both fetched.
- [x] AC-12 -- live: `proposal.applied` on alpha reads its graph and, Health on screen, its check once, nothing of beta; other events neither; a gap both; off screen the check stale; one `/check` during a walk; a UI decision no check (`live.test.tsx` "the graph and the check on the live tail", 8 tests; M: `graph` left out of `READS_OF_THE_SPEC`).
- [x] AC-13 -- screens over `HttpClient`: Health "Fails the check" + finding, "Could not check" + causes, a 503 alert + Retry; Graph a 404's `reason`, "2 nodes, 1 edge", a 503 alert; never "Not built yet" (`liveScreens.test.tsx`, 6 tests; M: `getCheck` still `notServed`).
- [x] AC-14 -- housekeeping: http, CLI, eval 896 passed; `mcp_read`, `mcp_size` 12/12; `build_graph.rs` pins unchanged; `pnpm lint` 0, `build`, `test` green; 17 packages; READMEs http 10 183, CLI 10 215, UI 8 188 B; `CLAUDE.md` untouched; docs gate clean, worst W <= 108 283 B (M: an 18th package).
- [ ] AC-15 -- owner's manual check: `specengine-http --root <this repository>`, `pnpm --dir ui dev`: Health's verdict and counts match `spec check`; a Graph matches `spec graph`; a terminal approval that applies refreshes both within 1 s, no reload. Open.

## Out of scope

`ui-live-tasks`, after `task-package`: `GET .../tasks`, `.../tasks/:id` (404 `TaskNotFound`), nine `task.*` listeners, `["tasks"|"task", p]` invalidation, the Inbox "Task" link, `ui/src` citations moved off `task-package.md`. Staging, file-change, drift events; `health` (Phase 3), `symbols`, Round; git modes or a baseline over HTTP; a server cache or watcher; a graph MCP tool; new packages, crates, dependencies.

## Implementation

Canon: http, CLI, UI READMEs, `spec-cli-graph.md`; 07 s3, 08 s2; `decision-staging.md`; amendment lines in `ui-graph`, `ui-health`, `daemon-read`. One Rust, two UI iterations; review 1 accepted (m1, m2, n4 fixed here; m3, n3 UI iteration 2; n1 to `ui-live-tasks`).

| Module | What it does |
|---|---|
| CLI `graph.rs`, `lib.rs`, `cap.rs` | `graph_with_view`; the cut moved unchanged into `capped` |
| http `app.rs`, `answer.rs` | the two routes; `answered(&Outcome)`: the exit map + the check exception |
| UI `http.ts`, `client.ts`, `queries.ts` | `getGraph`, `getCheck`; comments; `READS_OF_THE_SPEC` + `graph`, `check` |

Tests: http `graph.rs`, `check.rs`, `no_domain.rs`, UI `liveScreens.test.tsx` new; `turn`, `door`, `methods`, `projects`, `daemon_keys` changed. Trials: the UI's example URL byte-equal; at 700 sections the daemon 701 nodes, 700 edges, about 220 KB (the CLI's JSON 462 nodes, 0 edges); `/check` 4 ms on A, 27 ms at 700 sections.

Deviations: the fence's Host refusal says "serves its own origin only" (was "answers", a `LINK_TYPES` name); the 404 route list names `graph`, `check`; Graph's first read sends `depth=2`; `client.ts` tasks comments still cite `daemon-read` (n1); Q6's gap. UI iteration 2: all 15 sets required when the fixture exists; `check` invalidated with `cancelRefetch: false` (a walk in flight kept). Owner: AC-15, Q1-Q6.
