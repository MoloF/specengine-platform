---
class: spec
status: shipped
shipped: 2026-11-14
scope: [crates/specengine-http, scripts/deploy]
ref: RM-0118; the owner's report of 2026-11-14 (the hosted hub hung about 14:01-14:12 UTC, with no evidence of which request) and the coordinator's diagnosis; patch release 0.46.1
adrs: []
---

# slow-requests: the daemon names its slow requests and long turns (RM-0118)

## Why

On 2026-11-14 the hosted hub hung for minutes, at about 14:01-14:12 UTC, and the owner could not use it. The container did not restart and there was no OOM. Nothing in the daemon's log or Traefik's said which request was slow: the daemon logged no requests and Traefik kept no access log. Every CLI equivalent measured afterwards was fast (review 40-60 ms, inbox 108 ms over 154 open items, check 1.1 s, map 0.5 s), so the cause is probably a wait: one project runs one call at a time, its turn, and one long holder stalls every page of it. Next time the log must say who held the turn, who waited, how long, and whether a read walked.

## Description and interactions

`specengine-http --slow-ms N` (2 000 by default, 0 every request and hold). A request past it writes one stderr line when it completes or its client leaves (`status=gone`); a turn held past it writes `held` while still held, the evidence a true hang leaves, and `released` when it ends, for every holder: a request, a hub's `/mcp` call or resource read, a `--push` sync's merge. The live tail and `/mcp`'s GET stream write none, and the tail's polls take no turn. No login, body, header, query, raw path, cookie or MCP argument; no change of behaviour. Traefik's access log is on: JSON, 2 s and up, fields from an allowlist. Canon: `docs/canon/serve-slow.md`; the operator's lines: `scripts/deploy/README.md` "Going live".

## Data

```text
specengine-http: slow request at=<T> method=<M> route=<R> project=<P> id=<I> tool=<N> status=<S> ms=<n> wait_ms=<n|-> run_ms=<n|-> index=<walked|trusted|-> class=<page|mcp> line_ms=<n|-> ahead=<n|->
specengine-http: slow turn at=<T> project=<P> by=<R|sync> id=<I> tool=<N> state=held|released held_ms=<n> class=<page|mcp|sync>
```

The keys, their values and hygiene, the timing of `held` and the fourteen Traefik flags: `docs/canon/serve-slow.md` "Lines", "Traefik's access log". `class`, `line_ms` and `ahead` are `docs/features/hub-turn-fairness.md`'s, appended in the same release.

## Rules and edge cases

Exactly one request line per request at or past the threshold, none under it or for a stream; one `held` once a hold reaches it, one `released` when it ends; no line holds a login, body, header, query or raw path, a value outside its charset `?`; a line changes no answer and delays none past its write; lines interleave only line by line.

## Acceptance criteria

The daemon criteria are integration tests in `crates/specengine-http/tests/slow.rs`, spawning the binary and reading its stderr through `tests/common/slow.rs`'s strict grammar; a forced slow holder is a `tree` call on a generated corpus of 4 000 node files. Evidence: the test-engineer's final report (`cargo nextest run -p specengine-http -p specengine-mcp`: 970 passed; clippy default and probes clean; 12 of 12 named mutations red); the Rust review, ACCEPT after iteration 2.

- [x] SR1 — **The line** (`sr1_a_request_writes_one_line_in_the_grammar`). At `--slow-ms 0`, `GET …/proposals/PR-0001` writes one request line: `method=GET`, `route=/api/projects/{p}/proposals/{id}`, `project=<p>`, `id=PR-0001`, `tool=-`, `status=200`, integer times with `wait_ms + run_ms <= ms`, `index=-` locally; one `held` then one `released`, `by=` that template. Mutations: the raw path in `route`; `wait_ms` folded into `run_ms`.
- [x] SR2 — **The threshold** (`sr2_…`). With no `--slow-ms`, ten fast reads write no ` slow ` line. Mutation: the threshold ignored.
- [x] SR3 — **Privacy** (`sr3_…`). A hub at `--slow-ms 0`, `Remote-User: mira`: a search `q=needle7`, a card write with `"as":"mira"`, an `/mcp/{p}` `search` of `needle8`; no line holds `mira`, `needle7`, `needle8`, `q=`, `Remote-User`, `Cookie` or a body byte; the call's line has `tool=search`. Mutation: the login on the line.
- [x] SR4 — **Wait and run** (`sr4_…`). At `--slow-ms 200`, a slow `tree` holds the turn and an inbox read queues: the `tree`'s `held` line is on stderr while it is unanswered; the inbox's `wait_ms` is at least half the `tree`'s `run_ms`, its `run_ms` under a fifth of its `wait_ms`. Mutations: `wait_ms` clocked from the turn; `held` written only at release.
- [x] SR5 — **Gone** (`sr5_…`). Behind the same `tree`, a client closing 300 ms after sending writes `status=gone`, `run_ms=-`, `wait_ms` at least 200. Mutation: no line on drop.
- [x] SR6 — **Index on a hub** (`sr6_…`). A hub's first `GET …/tree` reads `index=walked`, the second `index=trusted`. Mutation: `index` from the route class.
- [x] SR7 — **The sync** (`sr7_…`). A `--push` hub with the fake remote writes `by=sync` `held` and `released` per served project of the repository, in `--root` order. Mutation: the sync's hold unwatched.
- [x] SR8 — **Streams** (`sr8_…`). An open `GET …/events`, read for one keep-alive and closed, writes no line with `route=/api/projects/{p}/events`. Mutation: streams logged.
- [x] SR9 — **Hygiene** (`sr9_…`, `data_no_value_is_percent_decoded`). `PR-0001%E2%80%AE` writes `id=?`, a 65-letter project `project=?`; no byte above 0x7E. Mutation: the raw segment printed.
- [x] SR10 — **The flag** (`sr10_…`). `--slow-ms -1`, `x` and `3600001` each exit 2 with one stderr line before listening; `3600000` starts; `spec serve`'s launch unchanged (`tests/serve.rs`).
- [x] SR11 — **The deploy** (`tests/deploy.rs` `sr11_traefiks_access_log_keeps_slow_requests_and_an_allowlist`). The `traefik` service's `command:` holds the fourteen flags exactly, no client or path field kept, headers dropped. Mutation: `RequestPath=keep` added.
- [x] SR12 — **Gates.** The full gate on the final tree, 2026-11-14: `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo nextest run --workspace --retries 0 --no-fail-fast` 6 110 passed, 100 skipped; `doc_pointers` and `spec check` (562 documents, 0 errors) pass.

## Out of scope

Request metrics, a status route, sampling, request IDs, log rotation; the login on the line and a per-person view; the CLI and the stdio MCP server; finding the 2026-11-14 hang's cause.

## Implementation

| Module | What it does |
|---|---|
| http `src/slow.rs` (new) | `SLOW_MS`, `SLOW_MS_MAX`, the threshold, `Exchange` (a request's record), `watch` (the outermost layer), `routed`, hygiene, the request line |
| http `src/turn.rs` (new) | `take`, `take_blocking`, `Holder`, `Held`, `Seats`, the watcher and the turn lines |
| http `src/main.rs` | `--slow-ms` (clap's range), `usage_error` joining clap's message to its first blank line |
| http `src/app.rs` | `.noted(<template>)` on every route |
| http `src/answer.rs`, `mcp.rs`, `sync.rs`, `start.rs` | each holder of a turn through `turn.rs`; the MCP call's tool from the body's one parse; `Project.seats` |
| mcp `src/hub.rs`, `lib.rs` | `call_extensions`, the call's request extensions |
| deploy `docker-compose.yml` | Traefik's fourteen access-log flags |
| tests | http `tests/slow.rs`, `tests/common/slow.rs`, `tests/deploy.rs` SR11 |

**Departures**, accepted: route templates written again per route (`.noted`), since axum's `matched-path` feature is off in the pinned build (turning it on is the owner's option, `serve-slow.md` "Known limits"); a streamed answer (an MCP call's SSE) writes its line at its body's end, a HEAD's, 1xx, 204 or 304 at its return; the watcher is a std thread, outside the runtime; a turn line's `project` is the served slug; held and released texts are built only when written; an unknown MCP tool name prints `tool=?`; clap's refusals are one line, joined to the first blank line (`serve.md` "Lines").

**Divergence.** "`--slow-ms` lowered in the compose" is not reachable: the entrypoint rebuilds its arguments and reads no variable for it (`serve-slow.md` "Known limits"); a hub runs at 2 000 ms.

**Release**: the patch 0.46.1, `docs/features/board-subtasks.md` "Implementation"; a hub gets the lines only from the `v0.46.1` image.
