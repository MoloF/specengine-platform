---
class: spec
status: shipped
scope: [crates/specengine-http, crates/specengine-cli, crates/specengine-store, ui]
ref: daemon-read analysis 2026-10-06, every recommendation accepted; 08 s2 Phase 2, the daemon
shipped: 2026-10-06
---

# Daemon read: the HTTP read surface

## Why

The UI (ADR-0033) read a mock: the owner saw no real tree or queue in a browser, and an agent's question reached him only on a terminal. `specengine-http`, a read-only HTTP adapter over the CLI library, serves the CLI's documents and a live tail of the queue's `events`: the fourth adapter of 05 s1. It is not ADR-0019's daemon (sole writer, socket bridges, auto-start, the gate) and must not become a second consent channel: decisions stay on the terminal (ADR-0004, ADR-0012). After `decision-apply` (queue schema 3); every binary from one commit.

Working answers (orchestrator, 2026-10-06; no new ADR): Q1 no token until the first write endpoint, the fence instead (ADR-0017); Q2 a separate binary, the CLI keeps no async runtime (`CLI_FORBIDDEN`), `spec serve` later launches it; Q3 repeated `--root`; Q4 no decision from the UI ("Open"); Q5 provisional types corrected, a key-set test; Q6 events per project. **D1** inbox entries gain `target_ids`, CLI and daemon alike (closed `ui-tree-node` "Open"); **D2** the snippet structure, snake_case. Dependencies approved by the owner 2026-10-06.

## Description and interactions

Moved to the canon at shipping: `crates/specengine-http/README.md` "Binary" (start, exit codes), "Answers and statuses" (one CLI call per request, the worktrees), "Concurrency" (the per-project turn). CLI text and `--json` unchanged but D1; MCP, hook, CI, `plugin/` unchanged.

## Data

Server side moved to the canon: `crates/specengine-http/README.md` "Endpoints" (query, path, browser view, snippet), "Answers and statuses" (status map, error body, the refused decision), "Live tail" (SSE, the opening frame, the closed event set); inbox `target_ids`: `docs/canon/proposal-queue.md` "Commands"; the CLI's `View`, `*_with_view`, `project_entry`, `events_after`, `EventsTail` and the store's `Snippet`, `events_after`: their READMEs' "API". Dependencies (owner, 2026-10-06; a root `Cargo.toml` comment): `axum =0.8.9` (`http1`, `tokio`), `futures-util =0.3.34`, tokio `net` and `sync` (04 s6); not taken: axum `query`, `json`, `tower-http`, `rust-embed`, `notify`, a TS type generator.

**UI** (`ui-developer`; `ui/README.md` "Contract seam"). `src/api/http.ts` `HttpClient` on `fetch` and `EventSource`, no package, `dataSource: "daemon"`; a path segment (project, REF, proposal ID) `encodeURIComponent`d once, an array repeats its key. `getProposal(p, id)` the review document, `subscribe(p, onEvent, onGap?) -> unsubscribe` (mock: a no-op); `decideProposal` POSTs, its 403 a `ClientError`. A GET 404 not carrying the error body resolves as the exit-1 document; other non-2xx -> `ClientError {status, message}` verbatim (an empty body says so; a 502 adds "the dev server's proxy reached no daemon; is specengine-http running?"); no response -> status 0. A read the daemon does not serve ("Out of scope": `getGraph`, `getTasks`, `getTask`) rejects unsent with `ClientError {status: 501, notServed: true}` (`isNotServed`): its screen says "Not built yet: the daemon has no endpoint for this read" and the message, no Retry.

**Live** (`queries.ts`): one `EventSource` per shown, listed project, the five types of the closed set; a `proposal.*` event invalidates `["inbox", p]` and `["proposal", p, <payload id>]`, `proposal.applied` also `["tree"|"node"|"search"|"bundle", p]`, that project only. The browser's own reconnect sends `Last-Event-ID`; once it gives up (CLOSED) the client reopens after 1, 2, 4 ... 30 s and calls `onGap`: the inbox, `["proposal", p]` and those four read again.

**Bootstrap, proxy**: `src/main.tsx` the daemon by default, no "Mock data"; any `?scenario=` the mock, flagged. `vite.config.ts`: `/api` -> `http://127.0.0.1:7777`, `changeOrigin`; `Origin` removed only when exactly `http://127.0.0.1:<port>` or `http://localhost:<port>` of the connection's own port (dev, preview), else forwarded for the fence to refuse; `server.cors`, `preview.cors` false; the plugin `specengine-api-this-machine-only` answers a non-loopback peer on `/api` with a 403 before the proxy.

**Types** (`provisional.ts`; key sets = `fixtures/daemon-keys.json`): `Project` + `root`, `branch`, `name` nullable; `InboxEntry` the 11 keys; `Proposal` the review document (42 keys, scalars nullable, no `task_id`); `Choice`; `QueueEvent {seq, type, payload}`. The Inbox lists entries (targets on the line); a card reads `getProposal` (loading, error + Retry, exit-1 notes) and shows the other answer's price, the decision record, the linked proposal, `distinct_from`; no Task fact; the decision buttons wait for the review document; a 403 shows verbatim with its terminal command; after a decision the cache takes the returned document.

## Rules and edge cases

Moved to `crates/specengine-http/README.md` "Answers and statuses" (one door: no handler decides, stores, exports, imports, initialises, indexes or checks), "Known limits"; the reads' only writes: `docs/canon/architecture.md#storage`. No hook consults the daemon (ADR-0006); no kind literal or pilot name in `crates/specengine-http/src`.

## Acceptance criteria

Setup: git copies of `fixtures/spec-a` (A) and `fixtures/spec-b` (B), a scratch `HOME`; tests (`crates/specengine-http/tests/`) start the binary on `--port 0`, read the port from stdout, kill it after. M: the mutation turning it red. Evidence: iteration 1 (test-engineer), each M red; iteration 2: "Implementation".

- [x] AC-01 -- fence: the socket's local address `127.0.0.1`; `--host`/`--bind` -> exit 2; `Host: evil.example:<port>`, `Origin: http://evil.example`, `Sec-Fetch-Site: cross-site`|`same-site` -> 403, no database in a fresh `HOME` after; no response (`OPTIONS` too) carries `Access-Control-*` (M: bind `0.0.0.0`; Host check off; permissive CORS). `fence.rs`.
- [x] AC-02 -- parity: `--root A --root B`; `tree`, `nodes/<every tree ID>`, `inbox`, `proposals/<id>`, `bundle?node_ids=...` byte-equal to `spec --root R <cmd> --json` stdout without its final LF, same `HOME`; `search` equal but `snippet`, which rendered equals the CLI's; a document over 40 000 characters whole, `truncated` false, while `spec show` cuts; a question on two targets: `target_ids` lists both in CLI and daemon; `mcp_read`, `mcp_size` green (M: pretty JSON; `notes` dropped; the cap kept; MCP given the browser view; `target_ids` only in the daemon). `parity.rs`; MCP 21/21.
- [x] AC-03 -- one door: a temp repo with an open `update`: every endpoint, the POST too, leaves `git status --porcelain` empty, `HEAD`, the proposal's state, `events` unchanged; the POST -> 403 naming `spec approve PR-0001`; no call of `approve`, `reject`, `propose`, `import_state`, `export_`, `init`, `index` in `crates/specengine-http/src` (M: the POST approves with an always-yes consent). `door.rs`, the call scan with a positive control.
- [x] AC-04 -- REF, snippets: `nodes/docs%2Fspec%2Fmovement%2Fstamina.md`, a `<slug>%2F<ID>`, `nodes/MEC-STAMINA%23RULE-STAM-REGEN` answer as `spec show` of the decoded REF; `MEC-STAMINA%2523X` reaches the CLI as `MEC-STAMINA%23X`; an unknown ID -> 404, byte-equal to the CLI's exit-1 document; a section with `**bold**` and the term -> exactly one hit segment, `**bold**` plain (M: decoding twice; 404 without the document; `**` markers reused). `refs.rs`.
- [x] AC-05 -- worktree: root on X with an uncommitted node edit, a proposal raised in a second worktree on Y: `inbox` lists it; `proposals/:id` has Y's `base_text`, its `preview`, that `worktree`, `branch` Y; `nodes` the root's edited text; `/api/projects` the root and X (M: preview against the root; `HEAD` read). `worktree.rs`.
- [x] AC-06 -- projects: `--root A --root B` both, in order; a root without config, a broken one, two of one slug -> exit 2 before listening, naming them, the port free; an unknown slug -> 404 error body (M: a duplicate slug deduplicated). `projects.rs`.
- [x] AC-07 -- live: a subscriber on A; a separate process (`spec propose question`, then `specengine-mcp` `ask_question`) stores an item -> `proposal.created` with its `seq` within 1 s of its exit; `Last-Event-ID: n` -> exactly `seq > n`; B's events never on A's stream; meanwhile `PRAGMA wal_checkpoint(TRUNCATE)` on another connection -> busy 0 (M: a 2 s poll; resume ignored; slug filter off; one read transaction across polls). `live.rs`: the opening frame, `n` in {0, first, a foreign row, last}, keep-alive 13-17 s, a bad ID 400.
- [x] AC-08 -- graph, licences: `build_graph.rs` pins the crate's `[dependencies]`; `CLI_FORBIDDEN` + `specengine-http`, `axum`, `hyper`; every package of its normal graph licensed within {MIT, Apache-2.0, BSD-3-Clause, Unlicense, BSL-1.0, Zlib, Unicode-3.0, ISC} (`OR`: one; `AND`: all) (M: the CLI declares axum; BSD-3-Clause dropped, matchit fails). An SPDX evaluator; pins updated in iteration 2.
- [x] AC-09 -- UI (Vitest, stubbed `fetch`, `EventSource`): each method hits its URL, REF encoded; a 404 document resolves as data; other non-2xx -> `ClientError` verbatim; no response -> 0; not served -> 501 + `notServed`, nothing requested; daemon by default, no "Mock data"; `?scenario=empty` -> mock, flag shown; a stubbed `proposal.created` refetches that project's inbox and that proposal only; key sets of `Project`, `InboxEntry`, `Proposal`, `NodeView`, `SearchHit`, `BundleView` = `fixtures/daemon-keys.json`, which a Rust test regenerates from the daemon on A and asserts unchanged (M: a generic error; the flag under the daemon; no invalidation; a review key missing). `pnpm test` 1049 passed, `daemonKeys` 20/20, `daemon_keys.rs`.
- [x] AC-10 -- housekeeping: no server outlives a test; `pnpm lint` (0 warnings), `build`, `test`, `ui_policy`, `anonymity`, `doc_pointers` green; docs gate clean, worst W <= min(109 484, at start); a new canon <= 12 288 B, the new README <= 10 189 B; `CLAUDE.md` not grown (M: one byte added to it). README 9 883 B, `CLAUDE.md` 5 438 B (5 444 before), W: "Implementation".
- [ ] AC-11 -- owner's manual check: `specengine-http --root <a project>`, `pnpm --dir ui dev`: the switcher lists it, the Inbox its queue; an `ask_question` from Claude Code there appears within 1 s, no reload; a decision shows the terminal command. Open.

## Out of scope

Decisions from the UI; 07 s3's token; `spec serve`, the `spec mcp` bridge, the sole writer, a socket, auto-start; a persistent registry, `--config`; serving `ui/dist` (`rust-embed`; CI builds no UI); file-change, drift events; `graph`, `tasks`, `tasks/:id`, `symbols`, `health`, `/mcp`; the hook gate; generated types.

## Open

- **Q4, owner**: how a decision may come from the UI -- the terminal confirming staged decisions, or WebAuthn/Touch ID: an ADR before any write endpoint, the token with it; consent stays terminal-only, never `--yes`. The decision dialog collects a note before its 403; a copy-the-command flow (as Tasks) may fit better.
- AC-11, the owner's manual check.

## Implementation

Canon: `crates/specengine-http/README.md` (new), `ui/README.md`, `architecture.md#ui`, `proposal-queue.md`, the CLI, store, eval READMEs, 04 s6, 07 s2-3, 08 s1-2, `CLAUDE.md`. Rust and UI two iterations each; review 1 accepted (5 minor, 10 nits: fixed in iteration 2 or recorded).

| Module | What it does |
|---|---|
| `crates/specengine-http` (new): `main`, `start`, `app`, `answer`, `args`, `tail` | start, fence, routes, the turn, the strict query, the live tail |
| CLI `cap`, `tree`, `show`, `search`; `inbox`, `project`, `events` (new) | `View`, `*_with_view`; `target_ids`; `ProjectEntry`; `events_after`, `EventsTail` |
| store `lib`, `read`, `queue` | `Snippet`, `SNIPPET_*`, `snippet_parts`; `EventsAfter`, `TailEvent` |
| root `Cargo.toml`, `Cargo.lock` | the member, the pins; 17 new packages, all within AC-08's licences |
| UI `api/http.ts` (new), `client`, `provisional`, `queries`, `main.tsx`, `vite.config.ts`; `inbox/*`, `ui/states.tsx`, `mocks/*` | the client, seam, types, live tail, bootstrap, proxy; review cards, the not-served state, CLI-like mocks |

Tests: `crates/specengine-http/tests/` (13 files + `common`), eval `build_graph.rs` (pins; `default_members_are_exactly_the_core_packages`), `import_support`, CLI `proposal_create.rs`, `fixtures/daemon-keys.json`; UI `http`, `live`, `proxy`, `daemonKeys`, `notServed`, `main`. Iteration 1: http 28/28, eval 68/68, MCP 21/21, store 150/150, workspace 1782/1782; 22 Rust mutations red; UI 1049 passed, iteration 2's 27 UI mutations red. Iteration 2: `methods.rs`, `query.rs`, `tail.rs`, `turn.rs` added, http 40/40, eval 68/68, UI 1049 passed; N1-N11 (per-route 405 `Allow`, the fence around the routes, 405 texts, clamp, strict query, full page, kept connection, replaced database, `user_version`, dropped queued requests, the routes fallback) and the re-run M1b, M4a, M7a-d red. git 2.54 (Xcode 27) repacks in the background after a commit: the harness turns auto-maintenance off for its setup git; the CLI and MCP test repositories get the same fix separately. Gate: worst W 108 139 B (108 324 at the start).

Deviations, now canon unless marked. Iteration 1: (1) `View` an argument, not a request field; (2) the store renders the marked snippet (`snippet` beside `snippet_parts`), CLI and MCP bytes unchanged; (3) U+0002-U+0004 alter the capped snippet too (limit); (4) a browser snippet `null` when empty; (6) each call re-runs `discover`, the slug check (`/api/projects` 503 if any changed); (7) `/api/projects` and polls take no turn; (8) a panic -> 500, a failing poll ends the stream, malformed event rows skipped, an empty REF 404, `branch` null without git; (9) SSE `no-store`; (5), (10) superseded. Iteration 2: (1) the clamp, an opening poll per stream (503 at the open); (2) an own strict query parser, axum `query` dropped; (3) another method on an unknown path -> 404; (4) `EventsTail` beside the one-shot `events_after`; (5) the fence around the whole router (an outer router's fallback service, no direct `tower`), 403 without `Allow`, 405 per route; (6) tokio `sync` declared; (7) a database replaced under a live stream (limit). Added: the opening `id:` frame; the closed event set (`queue.rs` `log()` its only writer). UI choices: `onGap` a third argument; the five types hard-coded; the not-served marker; the empty-body and 502 messages; port 7777 fixed, the proxy typed by a cast; a REF `.` or `..` cannot be a path segment.

Known limits (the README's) and, UI only, **the first connect**: if the browser's very first connect fails before any `id:` arrives, its reconnect sends no `Last-Event-ID` and no `onGap` fires; an event in between shows at the next read.
