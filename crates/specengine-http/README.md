---
class: canon
tier: 1
scope: [crates/specengine-http]
owner: owner
reviewed: 2026-10-06
---

# specengine-http -- the read surface over HTTP

A read-only HTTP adapter over the CLI library: the CLI's `--json` documents and a live tail of the queue's `events`, on `127.0.0.1`, for the web UI (`ui/README.md` "Contract seam"). The fourth adapter of 05 s1 (one core under CLI, MCP, HTTP and the CI check). Not ADR-0019's daemon: no sole writer, socket bridge, auto-start, gate or watcher (`spec serve` will launch it). No second consent channel: a decision is confirmed only on a terminal (ADR-0035); no hook consults it (ADR-0006). Task spec: `docs/features/daemon-read.md`.

Dependencies: `specengine-cli` (never the store or `rusqlite`; the reverse edge is forbidden), `axum =0.8.9` (no default features; `http1`, `tokio`), `tokio` (+ `net`, `sync`), `futures-util =0.3.34`, `clap`, `serde`, `serde_json`; a default member; pins and licences: eval `build_graph.rs` (04 s6).

## Binary

`specengine-http --root DIR... [--port N]` (`main.rs`, `start.rs`), in the foreground. `--root` repeats, served in that order; `--port` 7777 by default, `0` a free one. Bound to `127.0.0.1` only: no flag names another address (`--host`, `--bind` -> exit 2). Start: each root canonicalised, its own `specengine.toml` read, a `[project] slug` required, nothing opened in the data directory, then the bind. stdout: `serving <slug> <root>` per root, then `listening http://127.0.0.1:<port>`. Exit 2 before listening, one stderr line `specengine-http: <reason>`: usage, no `--root`; a root missing, without a config, with a broken one or no slug (naming it and the CLI's line); two roots of one slug (naming both); the port taken. Exit 1: the server stopped after listening. SIGINT, SIGTERM end it at any point: every write a read makes is one SQLite transaction.

## Fence

Around the whole router (`app.rs` `fence`), failing -> 403 without `Allow`, nothing read: `Host` exactly `127.0.0.1:<port>` or `localhost:<port>`; `Origin` absent or `http://` + one of them; `Sec-Fetch-Site` absent, `same-origin` or `none`; any of them sent twice -> 403. No authentication, ever (ADR-0034): the fence stops other sites' pages and identifies no one. No `Access-Control-*` header on any response, `OPTIONS` included; `Cache-Control: no-store` on every one.

Methods, past the fence: a read route takes GET only, any other (HEAD too) -> 405 `Allow: GET`, ``method <M> is not served on <path>: only GET is served here``; the decision route POST only, else 405 `Allow: POST`, ``...: this path takes only POST (refused): decisions are made on a terminal``; an unknown path -> 404, whatever the method.

## Endpoints

`:p` a served slug; query names are the MCP arguments (`docs/canon/mcp-read.md` "Tools").

| Method, path | Query | Answer |
|---|---|---|
| GET `/api/projects` | -- | `[{slug, name, root, branch}]`, `--root` order (CLI `project_entry`): `name` or `null`, `root` canonical, `branch` the current one, `null` detached or outside git |
| GET `/api/projects/:p/tree` | `root`, `depth`, `kinds`, `archive` | `spec tree --json`, browser view |
| GET `.../nodes/{*ref}` | `with`, `archive` | `spec show REF --json`, browser view |
| GET `.../search` | `query`, `kinds`, `limit`, `archive` | `spec search --json`, browser view |
| GET `.../bundle` | `node_ids`, `budget` | `spec bundle --json` |
| GET `.../inbox` | -- | `spec inbox --json` |
| GET `.../proposals/:id` | -- | `spec review PR --json` (not `--brief`) |
| GET `.../events` | header `Last-Event-ID` | the live tail (SSE) |
| POST `.../proposals/:id/decision` | -- | always 403 |

**Query**, raw (`args.rs`): split on `&` (empty parts skipped), each at its first `=`, decoded as a form (`+` a space, a literal plus `%2B`), strictly: a bad `%` or non-UTF-8 -> 400 naming the parameter. An array repeats its key (`kinds=a&kinds=b`), integers decimal, booleans `true`|`false`, `with` only `links`. 400 also: an unknown name (listing the endpoint's), a scalar repeated, a bad value, `query` or `node_ids` missing. **Path**: the slug, the proposal ID and the REF (all after `nodes/`) are percent-decoded once, strictly, as UTF-8; the REF goes to `show` as is. The client sends `encodeURIComponent` (`#` -> `%23`, `/` -> `%2F`); `%2523` reaches the CLI as `%23`; an empty REF is an unknown route.

**Browser view** (CLI `View::Browser`; CLI and MCP keep `Capped`): no `OUTPUT_CAP_CHARS` cut (`truncated` false, `omitted` null, the same keys); a search hit's `snippet` is `{segments: [{text, hit}], cut_start, cut_end}`, `null` when empty. The store's one FTS5 `snippet()` marks a hit with U+0002 ... U+0003, a cut with U+0004 (store `SNIPPET_*`): a corpus `**` is text, never a hit. `Capped` renders the structure back (`**` around a hit, U+2026 per cut): CLI and MCP bytes unchanged.

## Answers and statuses

One CLI library call per request (`answer.rs`): the process's `Env`, `Globals {root: Some(<root>)}`, `render_json`, the config read again first (a slug changed since the start -> 503 naming both; `/api/projects` -> 503 if any did). Exit 0 -> 200, the document; exit 1 -> 404, the exit-1 document (`reason` set: data, not an error); exit 2 -> 503, the error body, `message` the `CliError` line(s) verbatim. **Error body** `{"status":<code>,"message":"..."}`, exactly two keys, for 400, 403, 404 (an unknown slug or route, listing the served ones), 405, 500 (a caught panic), 503. Bodies `application/json; charset=utf-8`, compact: the CLI's bytes without the final LF.

**One door.** Until staging ships (`docs/canon/decision-staging.md`), the decision POST reads no body and answers 403 ``decisions are made on a terminal: `spec approve PR-0004` or `spec reject PR-0004 --reason ...` in <root>; nothing changed`` (`...` is U+2026; an `:id` not `PR-` + digits prints `PR-...`). No handler calls `approve`, `reject`, `propose`, `import_state`, `export_*`, `init`, `index` or `check` (`tests/door.rs` scans `src`); the only writes are the reads' index refresh in the data directory (`docs/canon/architecture.md#storage`): nothing under any root changes.

**Worktrees**: `tree`, `nodes`, `search`, `bundle` read the registered root's files on disk (uncommitted edits included, never `HEAD`); `inbox` the root's repository, every worktree; `proposals/:id` previews in the proposal's recorded worktree, naming it and its branch (`docs/canon/proposal-apply.md`).

## Live tail

`GET .../events` (`tail.rs`): the queue `events` of the project's slug only. A stream opens with one poll (a newer build's queue or a changed slug: 503), then `id: <start>` and a blank line, no data: the browser's last event ID set, nothing dispatched, so a stream dropped before its first event resumes there. `<start>` is the highest `seq` now (0 without a database), or `Last-Event-ID` when sent and not above it; one above it (the database wiped or made anew) starts from the highest, so the new database's events come. Then per event `id: <seq>`, `event: <type>`, `data: <payload>` (the stored JSON, one line), a blank line; a `:` comment every 15 s. Without `Last-Event-ID` nothing is replayed; `Last-Event-ID: n` gives exactly `seq > n`, at once; not a non-negative decimal integer, or sent twice -> 400.

The types are a closed set, the queue's (`docs/canon/proposal-queue.md` "States and events"): `proposal.created`, `.approved`, `.applied`, `.rejected`, `.apply_failed`; a row with a NULL type or payload, or CR or LF in its type, is skipped. Polls come at most 250 ms apart, at once after a full page (`EVENTS_PAGE_MAX`, 512 rows read, skipped ones counted). Each reads the config (a changed slug ends the stream) and runs one short read transaction on the stream's own connection (CLI `EventsTail`): kept between polls, opened again when the file is replaced (path, or device and inode), dropped when it is gone; no transaction spans polls, no database is created. A poll that cannot run ends the stream; the browser reconnects with its last ID and meets the error. A client gone ends its stream and the connection.

## Concurrency

Requests of one project run one at a time on the blocking pool (each read refreshes the project's index): a request awaits its project's turn (a `tokio` mutex) in its own task, so one whose client is gone first is dropped unrun; the turn goes with the call to the pool. Projects run in parallel; `/api/projects` and the tail's polls take no turn. With the CLI and the plugin's MCP server it is one more direct writer of the index (WAL, `busy_timeout`; `docs/canon/proposal-queue.md` "Store"); they share a queue only under one `HOME`.

## Known limits

- Repositories of one slug share `<slug>.db` and its events: a tail carries every one's (the UI refetches).
- U+0002 to U+0004 in the corpus can fake a hit or a cut in the snippet structure, and alter the capped CLI and MCP snippet too (a U+0002 ... U+0003 pair renders as `**`, a leading or trailing U+0004 as U+2026).
- One `EventSource` per browser tab over HTTP/1.1: about six tabs exhaust Chrome's per-host connection limit.
- A database replaced under a live stream: its new events with a `seq` at or below the stream's cursor are never sent.
- An uncut document fetched by an agent costs its context, but is no door.

## Tests

`tests/common/mod.rs`: byte copies of the git fixtures `spec-a`, `spec-b`, a scratch `HOME` and a `--port 0` daemon per test (killed and reaped on drop, alarm 300), raw HTTP/1.1, SSE and stdio MCP clients, a stale binary refused; setup git with auto-maintenance off (git 2.54 repacks after a commit). A file per criterion: `fence.rs`, `parity.rs` (byte-equal to `spec ... --json`), `door.rs` (the call scan, with a positive control), `refs.rs`, `worktree.rs`, `projects.rs`, `live.rs`, `daemon_keys.rs` (writes and asserts `fixtures/daemon-keys.json`, the UI's key sets; `SPECENGINE_WRITE_DAEMON_KEYS=1` regenerates); `edges.rs` (a newer queue, a slug change, malformed `Host`, byte determinism); `methods.rs` (405s, `Allow`), `query.rs` (strict decoding), `tail.rs` (clamp, full page, kept or replaced connection, `user_version`), `turn.rs` (dropped queued requests). Run: `cargo nextest run -p specengine-http --test <file>`; graph and licences: eval `build_graph.rs`.
