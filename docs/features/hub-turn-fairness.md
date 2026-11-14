---
class: spec
status: shipped
shipped: 2026-11-14
scope: [crates/specengine-http, crates/specengine-mcp, ui]
ref: RM-0119; the analyst's report of 2026-11-14 on the hosted hub's starvation (measured live, PR-100166..100173), the owner's word that day ("do all the fixes and release a new version"), the orchestrator's working answers; patch release 0.46.1
adrs: []
---

# hub-turn-fairness: a person's page is served before an agent's queued calls (RM-0119)

## Why

On the hosted hub a person on the page starved while his agent worked the same project over `/mcp`. Every call waited in the project's one FIFO turn, and an agent's write costs 2-5 s of a core inside it (measured live on 2026-11-14, PR-100166 to PR-100173; a dry-run update 1.8 s), so a burst of parallel tool calls put N holders ahead of the page. The page also re-queued itself: every `proposal.*` event invalidated the Inbox read with TanStack's default `cancelRefetch: true`, aborting the read already waiting, and its replacement joined the back of the line. Memory was measured too, 422 MB before 8 writes, 498 MB after, 485 MB ten minutes later; this slice does not address it.

## Description and interactions

The orchestrator's working answers, 2026-11-14, the owner's review open: W1 the page goes before an agent's queued calls, by one FIFO line per login in front of the turn for `/mcp` calls; W2 with one agent bursting 8 parallel writes, every page `/api/` request answers within 10 s; W3 nothing is refused; W4 two agents under one login share a line. No ADR: ADR-0065's "each call in the project's turn" and ADR-0069's trusted reads stand; the order calls reach the turn changes, not what runs in it.

### The Rust half: a line per login

An MCP call that takes the turn first waits in its login's FIFO line for the project, keeping its place until it ends; page requests, the sync's merge and the messages that take no turn wait in no line. A login thus has one call at most holding or waiting for the turn; order within a login is kept, throughput unchanged; both waits race the request's cancellation. `HubSeam::turn(&self, login, command)`; the lines live in `Project.lines`, an entry only while a call holds or waits in it. The slow lines (`docs/features/slow-requests.md`) gain `class=<page|mcp|sync>`, and on a request line `line_ms` and `ahead`. The `probes` build's `probe_hold {ms, tag}` holds the turn and answers its run's `seq`. Canon: `docs/canon/hub-mcp.md` "Turn", "Known limits"; `serve.md` "Concurrency"; `serve-slow.md`.

### The UI half: a live event never aborts a read in flight

Every stream-driven re-read (the `card.*`, `proposal.*`, `task.*` and apply handlers and their gap re-reads) goes through `ui/src/api/coalesce.ts` `readAgainKept`: no read in flight, one at once; one in flight, kept in its place, and exactly one more once it settles, however many events land. A key change and a person's Retry, Read again or Check again keep TanStack's semantics. The health read keeps its own equal debt and the check its walk with no second one (`docs/canon/health.md` "UI"). Canon: `docs/canon/ui-client.md` "Live re-reads", `round-ui.md`, `spec-roadmap-screen.md`.

## Rules and edge cases

A turn-taking MCP call waits for the turn only once it heads its login's line; a page request or the sync waits for the turn alone; no call is refused or reordered within its login, each runs at most once; a stream event never aborts a read in flight and reads once more after it; the stdio MCP server and the local daemon, with no logins, are unchanged.

## Acceptance criteria

The Rust criteria are hub tests in `crates/specengine-http/tests/hub_fairness.rs` over a scratch fixture copy, in the `probes` build, `#[ignore]`d: `cargo nextest run -p specengine-http --features specengine-mcp/probes --test hub_fairness --run-ignored ignored-only`; the agent and the page share `Remote-User: mira`; order is judged from `seq` counters and stderr order, never clocks. Evidence: the test-engineer's final report (the probes run 8 passed with `hub_trust`; 970 http and mcp tests passed; clippy default and probes clean; 11 named mutations applied); the Rust and UI reviews, ACCEPT after iteration 2; the whole UI suite, lint, typecheck and the build green.

- [x] TF1 — **The page goes second** (`tf1_the_page_goes_second_behind_an_agents_burst`). `mira` sends 8 parallel `probe_hold` calls, the first held 3 s; once it holds, the page's `GET …/inbox` answers before the second probe and within 3 holds. Mutations: the page given a line keyed by its login; no line at all.
- [x] TF2 — **Another login goes second** (`tf2_…`). `mira`'s 8 probes, then `ann`'s b1: `seq` a1=1, b1=2, a2 to a8 = 3 to 9. Mutations: one line for every login; a line released before the run, so two of a login's calls reach the turn.
- [x] TF3 — **All once, in order, none refused** (`tf3_…`). TF2's 9 calls each answer 200 with a result, the counter ending at 9. Mutation: a busy refusal past N waiters (a line released before the run is caught by TF2).
- [x] TF4 — **Cancelled in its line** (`tf4_…`). Of `mira`'s three probes the second's client closes while it waits in the line: the third answers `seq` 2, the counter ends at 2; an `ask_question` cancelled in its line stores no proposal. No named mutation: leaving the line's wait outside the cancellation race is equivalent here, the turn's wait staying inside `turned`'s biased race.
- [x] TF5 — **The sync goes second** (`tf5_the_sync_goes_second`). One repository holds two served projects, a `--push` hub with the fake remote at `--slow-ms 0`. `mira` sends 4 probes of 1 s on the first, `a1` held 8 s; during it her page confirms a proposal on the second project, which signals the sync. On the first project the sync's `by=sync` `held` line comes after `a1`'s and before `a2`'s. (A confirm on the first project takes its turn behind `a1`, so `a2`, freed from the line, reaches the turn before the sync: the single-project wording could not hold.) Mutation: the sync given a line.
- [x] TF6 — **Order within a login** (`tf6_…`). One stateful session's 5 probes sent back to back answer `seq` in send order. Mutation: the line as `try_lock` with jitter, random by nature; red on the final run.
- [x] TF7 — **The lines' map** (`src/lines_tests.rs`: `tf7_a_settled_burst_leaves_no_line`, `tf7_a_call_cancelled_alone_in_line_leaves_no_line`). A settled burst and a call cancelled alone leave no entry. Mutation: entries never removed.
- [x] TF8 — **Slow lines** (`tf8_the_slow_lines_name_class_line_and_ahead`). During TF1 at `--slow-ms 0` the GET's line ends `class=page line_ms=- ahead=1`; a probe's has `class=mcp`, an integer `line_ms`, `tool=probe_hold`; TF5's sync turn lines `class=sync`. Mutation: `line_ms` folded into `wait_ms`.
- [x] TF9 — **Default build unchanged** (`tf9_the_default_build_has_no_probe_hold`, default build). The hub's 18 tools; `probe_hold` answers `tool not found`; `tests/turn.rs` and the http suite pass. Mutation: `probe_hold` outside `cfg(feature = "probes")`.
- [x] TF10 — **UI: kept in flight** (`src/api/live.test.tsx`). A held inbox read and a `proposal.created`: its `AbortSignal` is not aborted, one request; its answer starts exactly one more, whose data is shown. Mutations: `cancelRefetch: true` restored; no trailing read.
- [x] TF11 — **UI: coalesced** (`live.test.tsx`). Three events during one held read: one trailing read, two requests; an event with none in flight, one read at once. Mutation: a trailing read per event.
- [x] TF12 — **UI: every stream re-read** (`live.test.tsx`, `catalog.live.test.tsx`, `delivery.live.test.tsx`, `roadmap.live.test.tsx`, `round/QuestionsView.test.tsx`): the card list on `card.*`, the Questions sheet on `task.*` and the others keep their read; the tests that pinned the abort are rewritten; a key change still aborts (`queries.test.tsx`). Mutation: `readAgainKept` bypassed for one handler.
- [x] TF13 — **Gates.** The full gate on the final tree, 2026-11-14: fmt and clippy (`-D warnings`, default and http+mcp with `specengine-mcp/probes`) clean; `cargo nextest run --workspace --retries 0 --no-fail-fast` 6 110 passed, 100 skipped; the probes run (`hub_fairness`, `hub_trust`) 8 passed; UI lint, `tsc`, vitest (231 files, 6 212 tests) and the build pass; `doc_pointers`, `export index` and `spec check` (562 documents, 0 errors) pass.
- [ ] TF14 — **[owner, live hub, after 0.46.1]** With the Inbox open, the owner's agent raises 8 proposals in parallel. The daemon's lines (`docs/canon/serve-slow.md` "Reading them") show each page request with `class=page`, `ahead` at most 1 and `ms` under 10 000; the Inbox shows the new items as they land. Traefik's access log only confirms: a page `/api/` line with a `Duration` of 10 s or more and a daemon request line at the same moment is a failure; a ~300 s line with no daemon line is the tail's stream.

## Out of scope

The next slices, plan text: cutting a write's cost (the full check twice per proposal, `crates/specengine-core/src/patch.rs`; the tree re-parsed, `crates/specengine-store/src/update.rs`; the trailer lookup, `crates/specengine-cli/src/review.rs`); concurrent trusted reads, with an ADR; measuring the memory growth. A busy refusal, a per-login quota, a line per agent (W4).

## Implementation

| Module | What it does |
|---|---|
| mcp `src/hub.rs`, `lib.rs` | `HubSeam::turn(login, Command)`, `call_extensions`, `HUB_PROBES` with `probe_hold` |
| mcp `src/probes.rs` | `probe_hold`: the turn held `ms` on the blocking pool, `{tag, seq}` |
| http `src/turn.rs` (new) | `Lines`, `Place` (the line per login), `Seats` (`ahead`), `Class` |
| http `src/mcp.rs`, `start.rs` | the line, then the turn, in one cancellation race; `Project.lines`, `Project.seats` |
| http `src/slow.rs` | `class`, `line_ms`, `ahead` on the lines |
| UI `src/api/coalesce.ts` (new), `queries.ts` | `readAgainKept`; every stream handler through it, health's own debt, the check kept |
| tests | http `tests/hub_fairness.rs` (new), `src/lines_tests.rs` (new), `tests/common/slow.rs`; mcp `mcp_hub.rs`, `plugin_*.rs`; UI the five live tests, `QuestionsView.test.tsx` |

**Departures**, accepted: the lines are keyed in `turn.rs` `Lines`, `Project.seats` counting the turn's holders and waiters for `ahead`; the turn lines' `project` is the served slug; the health read and the check are the two exceptions to `readAgainKept`, each keeping its `health.md` rule; TF5 confirms on a second project and TF3, TF4, TF6 name their mutations as above.

**Release**: the patch 0.46.1, `docs/features/board-subtasks.md` "Implementation"; no plugin text, MCP schema, queue schema or dump format change.
