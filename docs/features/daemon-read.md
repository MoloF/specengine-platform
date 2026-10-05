---
class: spec
status: draft
scope: [crates/specengine-http, crates/specengine-cli, ui]
ref: daemon-read analysis 2026-10-06, every recommendation accepted; 08 §2 Phase 2, the daemon
---

# Daemon read: the HTTP read surface

## Why

The UI (ADR-0033) reads a mock: the owner sees no real tree or queue in a browser, and an agent's question reaches him only on a terminal. `specengine-http`, a read-only HTTP adapter over the CLI library, serves the CLI's documents and a live tail of the queue's `events`: the fourth adapter of 05 §1. It is not ADR-0019's daemon (sole writer, socket bridges, auto-start, the gate) and must not become a second consent channel: decisions stay on the terminal (ADR-0004, ADR-0012). After `decision-apply` (queue schema 3); every binary from one commit.

Working answers (orchestrator, 2026-10-06; no new ADR): Q1 no token until the first write endpoint, the fence instead (ADR-0017); Q2 a separate binary, the CLI keeps no async runtime (`CLI_FORBIDDEN`), `spec serve` later launches it; Q3 repeated `--root`; Q4 no decision from the UI ("Open"); Q5 provisional types corrected, a key-set test; Q6 events per project. **D1** inbox entries gain `target_ids`, CLI and daemon alike: closes `ui-tree-node` "Open" (a second target matches its node). **D2** snippet names as `ui-tree-node` proposed (snake_case like every document); this spec owns them. **Dependencies approved by the owner 2026-10-06** (Data).

## Description and interactions

`specengine-http --root DIR… [--port N]`, foreground, bound to `127.0.0.1` only (no flag names another address), port 7777 by default, `0` ephemeral. Start: each root canonicalised, its own `specengine.toml` read (a slug required), nothing opened in the data directory, then the bind; stdout `serving <slug> <root>` per root, then `listening http://127.0.0.1:<port>`. Exit 2 before listening, stderr `specengine-http: <reason>`: no `--root`; a root missing, without or with an invalid config or no slug (naming it and the CLI's line); two roots of one slug (naming both); the port taken. SIGINT, SIGTERM end the process (every write is one SQLite transaction).

A request runs one CLI library call (blocking pool, the process's `Env`, `Globals {root: Some(<root>)}`, `render_json`), serialized per project (each read refreshes the index, R3); never the store or `rusqlite` (A3). The config is re-read per call; a slug changed since start → 503 naming both. Daemon, CLI and the plugin's MCP server share a queue only under one `HOME`.

**Worktree**: `tree`, `nodes`, `search`, `bundle` read the registered root's files on disk (uncommitted edits included, never `HEAD`); `inbox` the root's repository, every worktree; `proposals/:id` previews in the proposal's recorded worktree, naming it and its branch (`docs/canon/proposal-apply.md`). Unchanged: CLI text and `--json` (but D1), MCP, hook, CI, `plugin/`.

## Data

**Endpoints** (`:p` a registered slug; query names = the MCP arguments, `docs/canon/mcp-read.md` "Tools"):

| Method, path | Query | Body ≙ |
|---|---|---|
| GET `/api/projects` | — | `[{slug, name, root, branch}]` |
| GET `/api/projects/:p/tree` | `root`, `depth`, `kinds`, `archive` | `spec tree --json`, browser view |
| GET `…/:p/nodes/{*ref}` | `with`, `archive` | `spec show REF --json`, browser view |
| GET `…/:p/search` | `query`, `kinds`, `limit`, `archive` | `spec search --json`, browser view |
| GET `…/:p/bundle` | `node_ids`, `budget` | `spec bundle --json` |
| GET `…/:p/inbox` | — | `spec inbox --json` |
| GET `…/:p/proposals/:id` | — | `spec review PR --json` (not `--brief`) |
| GET `…/:p/events` | header `Last-Event-ID` | SSE |
| POST `…/:p/proposals/:id/decision` | — | always 403 |

**Projects**: `name` `[project] name` else `null`; `root` canonical; `branch` the root's current branch, `null` detached or outside git; `--root` order.

**Query**: pairs (axum `Query<Vec<(String, String)>>`), an array repeats its key (`kinds=a&kinds=b`), integers decimal, booleans `true`|`false`, `with` only `links`. 400: an unknown name (listing the endpoint's), a scalar repeated, a bad value, `query` or `node_ids` missing.

**REF**: everything after `nodes/`, percent-decoded once as UTF-8, given to `show` as is; the client's `encodeURIComponent` (`#` → `%23`, `/` → `%2F`). A bad `%` or non-UTF-8 → 400; `%2523` reaches the CLI as `%23`.

**Status**: exit 0 → 200, the document; exit 1 → 404, the exit-1 document (`reason` set; data to the client), the error body if none is printed; exit 2 → 503, the error body, `message` the `CliError` line(s) verbatim. **Error body** `{"status":<code>,"message":"…"}`, exactly two keys, also for 400, 403, 404 (unknown project or route), 405. Bodies `application/json; charset=utf-8`, compact, the CLI's bytes without the final LF; `Cache-Control: no-store`.

**Browser view**: `View {Capped, Browser}` on the CLI library's tree, show and search requests, default `Capped` (CLI, MCP). `Browser`: no `OUTPUT_CAP_CHARS` cut (`truncated` false, `omitted` null, same keys); a hit's `snippet` the structure below.

**Snippet** (D2), or `null`:

```json
{"segments":[{"text":"regenerates ","hit":false},{"text":"stamina","hit":true},{"text":" while **idle**","hit":false}],"cut_start":true,"cut_end":false}
```

The store's one FTS5 `snippet()` takes sentinels U+0002 (hit start), U+0003 (hit end), U+0004 (cut) for today's `'**', '**', '…'` (`read.rs`): a corpus `**` is text, never a hit. `Capped` renders it back (`…` per flag, `**` around a hit): CLI and MCP bytes unchanged.

**Inbox entry** (D1): `target_ids` after `target_id` (stored canonical targets; an update's `[target_id]`); with `decision-apply`: `{id, kind, status, target_id, target_ids, branch, created_at, rationale, severity, summary, record_id}`. Text unchanged.

**SSE**: per event `id: <seq>`, `event: <type>`, `data: <payload>` (stored JSON, one line), a blank line; a `:` comment every 15 s (axum `KeepAlive`). Only queue `events` of the project's slug. No `Last-Event-ID` → after the current highest `seq` (no replay); `Last-Event-ID: n` → exactly `seq > n`; not a non-negative integer → 400. A stream polls at most 250 ms apart, each poll one short read transaction, none held across polls: CLI library `events_after(&Env, &Globals, after: Option<i64>) -> EventsPage {events: [{seq, type, payload}], last_seq}` over store `events_after(seq, 512)`; no database → none, nothing created.

**Fence**, before routing (failing → 403, nothing read): `Host` exactly `127.0.0.1:<port>` or `localhost:<port>`; `Origin` absent or `http://` + one of them; `Sec-Fetch-Site` absent, `same-origin` or `none`. No `Access-Control-*` header on any response; another method → 405.

**Decision POST**: body unread; 403 ``decisions are made on a terminal: `spec approve PR-0004` or `spec reject PR-0004 --reason …` in <root>; nothing changed`` (an `:id` not `PR-` + digits: `PR-…`).

**Dependencies**, approved by the owner 2026-10-06 (a root `Cargo.toml` comment as 2026-09-29's):

| Crate | Pin, features | Licence | Brings |
|---|---|---|---|
| `axum` | =0.8.9, no default features; `http1`, `tokio`, `query` | MIT | hyper 1, hyper-util, tower 0.5 (+ layer, service), axum-core, http, http-body(-util), mime, httparse, httpdate; serde_urlencoded (MIT OR Apache-2.0), sync_wrapper (Apache-2.0), matchit (MIT AND BSD-3-Clause) |
| `tokio` / `futures-util` | =1.53.1 (workspace) / =0.3.34 | MIT / MIT OR Apache-2.0 | both in the lock; axum's `tokio` turns on tokio `net` (mio in the lock, socket2 likely new) |

Reused: workspace `clap`, `serde`, `serde_json`; `specengine-cli`. A default member; graph fixed by `Cargo.lock` (`cargo tree -p specengine-http -e normal`). Not taken: axum `json`, `tower-http`, `rust-embed`, `notify`, a TS type generator; axum 0.9 changes nothing.

**UI** (`ui-developer`): `src/api/http.ts` `HttpClient` on `fetch` + `EventSource`, no package, `dataSource: "daemon"`; new `getProposal(p, id)`, `subscribe(p, onEvent) → unsubscribe` (mock: a no-op); `getTree`, `getNode`, `search`, `getBundle` as `ui-tree-node` "Data"; `decideProposal`'s 403 a `ClientError`. A 404 not carrying the error body → the document; other non-2xx → `ClientError {status, message}` verbatim; no response → status 0. `src/main.tsx`: the daemon by default, `?scenario=` the mock with "Mock data". `vite.config.ts`: `server.proxy` `/api` → `http://127.0.0.1:7777`, `changeOrigin: true`, the proxied `Origin` removed (`proxyReq.removeHeader("origin")`) so a decision reaches its 403. A `proposal.*` event invalidates `["inbox", p]`, `["proposal", p, <payload id>]`. Provisional types: `Project` + `root`, `branch`, `name` nullable; `InboxEntry` as above (no `task_id`); `Proposal` the review document (`price_of_other`, `distinct_from`, `linked`, record keys; no `task_id`); the Inbox lists entries, a card reads `getProposal`.

## Rules and edge cases

- **One door** (R2): no handler calls `approve`, `reject`, `propose`, `import_state`, `export_*`, `init`, `index`, `check`; the only writes are the reads' index refresh in the data directory (`docs/canon/architecture.md#storage`). WHEN any request is answered THEN nothing under any root changes.
- No hook consults it (ADR-0006, R8); one more direct SQLite writer (A2). Busy past `busy_timeout` → 503; a build older than the queue's schema → 503 per call (R4).
- No kind literal or pilot name in `crates/specengine-http/src` (R6). An uncut document fetched by an agent costs its context; not a door (R7).
- **Known limits**: repositories of one slug share `<slug>.db` and its events (refetches, R9); a corpus U+0002–U+0004 may fake a hit or cut in the browser view; an index walk takes 3.6 s, a bundle ~2 s debug.

## Acceptance criteria

Setup: git copies of `fixtures/spec-a` (A) and `fixtures/spec-b` (B), a scratch `HOME`; tests (`crates/specengine-http/tests/`) start the binary on `--port 0`, read the port from stdout, kill it after. M: the mutation turning it red.

- [ ] AC-01 — fence: the socket's local address `127.0.0.1`; `--host`/`--bind` → exit 2; `Host: evil.example:<port>`, `Origin: http://evil.example`, `Sec-Fetch-Site: cross-site`|`same-site` → 403, no database in a fresh `HOME` after; no response (`OPTIONS` too) carries `Access-Control-*` (M: bind `0.0.0.0`; Host check off; permissive CORS).
- [ ] AC-02 — parity: `--root A --root B`; `tree`, `nodes/<every tree ID>`, `inbox`, `proposals/<id>`, `bundle?node_ids=…` byte-equal to `spec --root R <cmd> --json` stdout without its final LF, same `HOME`; `search` equal but `snippet`, which rendered (`…` per flag, hits in `**`) equals the CLI's; a document over 40 000 characters whole, `truncated` false, while `spec show` cuts; a question on two targets: `target_ids` lists both in CLI and daemon; `mcp_read`, `mcp_size` green (M: pretty JSON; `notes` dropped; the cap kept; MCP given the browser view; `target_ids` only in the daemon).
- [ ] AC-03 — one door: a temp repo with an open `update`: every endpoint, the POST too, leaves `git status --porcelain` empty, `HEAD`, the proposal's state, `events` unchanged; the POST → 403 naming `spec approve PR-0001`; no call of `approve`, `reject`, `propose`, `import_state`, `export_`, `init`, `index` in `crates/specengine-http/src` (M: the POST approves with an always-yes consent).
- [ ] AC-04 — REF, snippets: `nodes/docs%2Fspec%2Fmovement%2Fstamina.md`, a `<slug>%2F<ID>`, `nodes/MEC-STAMINA%23RULE-STAM-REGEN` answer as `spec show` of the decoded REF; `MEC-STAMINA%2523X` reaches the CLI as `MEC-STAMINA%23X`; an unknown ID → 404, byte-equal to the CLI's exit-1 document; a section with `**bold**` and the term → exactly one hit segment, `**bold**` plain (M: decoding twice; 404 without the document; `**` markers reused).
- [ ] AC-05 — worktree: root on X with an uncommitted node edit, a proposal raised in a second worktree on Y: `inbox` lists it; `proposals/:id` has Y's `base_text`, its `preview`, that `worktree`, `branch` Y; `nodes` the root's edited text; `/api/projects` the root and X (M: preview against the root; `HEAD` read).
- [ ] AC-06 — projects: `--root A --root B` both, in order; a root without config, a broken one, two of one slug → exit 2 before listening, naming them, the port free; an unknown slug → 404 error body (M: a duplicate slug deduplicated).
- [ ] AC-07 — live: a subscriber on A; a separate process (`spec propose question`, then `specengine-mcp` `ask_question`) stores an item → `proposal.created` with its `seq` within 1 s of its exit; `Last-Event-ID: n` → exactly `seq > n`; B's events never on A's stream; meanwhile `PRAGMA wal_checkpoint(TRUNCATE)` on another connection → busy 0 (M: a 2 s poll; resume ignored; slug filter off; one read transaction across polls).
- [ ] AC-08 — graph, licences: `build_graph.rs` pins the crate's `[dependencies]`; `CLI_FORBIDDEN` + `specengine-http`, `axum`, `hyper`; every package of its normal graph licensed within {MIT, Apache-2.0, BSD-3-Clause, Unlicense, BSL-1.0, Zlib, Unicode-3.0, ISC} (`OR`: one; `AND`: all) (M: the CLI declares axum; BSD-3-Clause dropped, matchit fails).
- [ ] AC-09 — UI (Vitest, stubbed `fetch`, `EventSource`): each method hits its URL, REF encoded; a 404 document resolves as data; other non-2xx → `ClientError` verbatim; no response → 0; daemon by default, no "Mock data"; `?scenario=empty` → mock, flag shown; a stubbed `proposal.created` refetches that project's inbox and that proposal only; key sets of `Project`, `InboxEntry`, `Proposal`, `NodeView`, `SearchHit`, `BundleView` = `fixtures/daemon-keys.json`, which a Rust test regenerates from the daemon on A and asserts unchanged (M: a generic error; the flag under the daemon; no invalidation; a review key missing).
- [ ] AC-10 — housekeeping: no server outlives a test; `pnpm lint` (0 warnings), `build`, `test`, `ui_policy`, `anonymity`, `doc_pointers` green; docs gate clean, worst W ≤ min(109 484, at start); a new canon ≤ 12 288 B, the new README ≤ 10 189 B (largest Tier 1 today); `CLAUDE.md` not grown (M: one byte added to it).
- [ ] AC-11 — owner's manual check: `specengine-http --root <a project>`, `pnpm --dir ui dev`: the switcher lists it, the Inbox its queue; an `ask_question` from Claude Code there appears within 1 s, no reload; a decision shows the terminal command.

## Out of scope

Decisions from the UI; 07 §3's token; `spec serve`, the `spec mcp` bridge, the sole writer, a socket, auto-start; a persistent registry, `--config`; serving `ui/dist` (`rust-embed`; CI builds no UI); file-change, drift events; `graph`, `tasks`, `symbols`, `health`, `/mcp`; the hook gate; generated types.

## Open

- **Q4, owner (morning)**: how a decision may come from the UI — the terminal confirming staged decisions, or WebAuthn/Touch ID: an ADR before any write endpoint, the token with it; consent stays terminal-only, never `--yes`.

## Implementation

Not built. With the code: the root `Cargo.toml` comment; 04 §6 `axum` → `=0.8.9` and features (04 net ≤ 0). At shipping (R1, R10): `crates/specengine-http/README.md`; `architecture.md#ui` ("Until `spec serve` exists…"); `ui/README.md` "Contract seam"; `proposal-queue.md` "Commands" (`target_ids`), "Store" ("until the daemon is the sole writer", `specengine-http`); CLI README "Output and the cap" (browser view); 07 §2–3; `CLAUDE.md` "Layout"; `CLAUDE.md`, 04, 05 net ≤ 0; `ui-tree-node` "Open" closed.
