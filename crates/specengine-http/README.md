---
class: canon
tier: 1
scope: [crates/specengine-http]
owner: owner
reviewed: 2026-10-15
---

# specengine-http -- SpecEngine over HTTP

An HTTP adapter over the CLI library, 05 s1's fourth: the CLI's `--json` documents, the queue's live `events` and stages, on `127.0.0.1`, for the web UI (`ui/README.md` "Contract seam"). The daemon, one per data directory, `spec serve` launches (`serve.md`), serving the web UI compiled in (`serve-ui.md`); not ADR-0019's: no sole writer, socket bridge, auto-start, gate or watcher. No second consent channel: a staged decision is confirmed only on a terminal (ADR-0035); no hook consults it (ADR-0039).

Dependencies: `specengine-cli` (never the store or `rusqlite`; no reverse edge), `axum` (`http1`, `tokio` only), `tokio`, `futures-util`, `clap`, `serde`, `serde_json`, `rust-embed` (`debug-embed`); a std-only build script `src/build.rs`; a default member; pins, licences: eval `build_graph.rs`.

## Binary

`specengine-http --root DIR... [--port N]` (`main.rs`, `start.rs` `served`, `daemon.rs`), foreground, by `spec serve` or directly; `--root` repeats, served in order; `--port` 7777, `0` a free one; `127.0.0.1` only (`--host`, `--bind` -> exit 2). The start, its stdout lines and its exit 2 before listening (one stderr line `specengine-http: <reason>`): `serve.md` "Start", "Lines". Exit 1: stopped after listening. SIGINT, SIGTERM end it at any point: every write is one SQLite transaction.

## Fence

Around the whole router (`app.rs` `fence`), failing -> 403 without `Allow`, nothing read: `Host` exactly `127.0.0.1:<port>` or `localhost:<port>`; `Origin` absent or `http://` + one of them; `Sec-Fetch-Site` absent, `same-origin` or `none`; any of them sent twice -> 403. No authentication (ADR-0034). No `Access-Control-*` header on any response, `OPTIONS` included; `Cache-Control: no-store` on every one.

Methods, past the fence: a read route takes GET only, any other (HEAD too) -> 405 `Allow: GET`; the decision route POST and DELETE, else 405 `Allow: POST, DELETE`, `round/answers` POST, else `Allow: POST`, both ending ``confirmed on a terminal``; an unknown path -> 404 listing the routes (the web UI's last), whatever the method.

## Endpoints

`:p` a served slug; query names are the MCP arguments (`docs/canon/mcp-read.md` "Tools"), `graph`'s its JSON echo keys, `tasks`' `--status`.

| Method, path | Query | Answer |
|---|---|---|
| GET `/api/projects` | -- | `[{slug, name, root, branch}]`, `--root` order (CLI `project_entry`): `name` or `null`, `root` canonical, `branch` current, `null` detached or outside git |
| GET `/api/projects/:p/tree` | `root`, `depth`, `kinds`, `archive` | `spec tree --json`, browser view |
| GET `.../nodes/{*ref}` | `with`, `archive` | `spec show REF --json`, browser view |
| GET `.../search` | `query`, `kinds`, `limit`, `archive` | `spec search --json`, browser view |
| GET `.../bundle` | `node_ids`, `budget` | `spec bundle --json` |
| GET `.../graph` | `ref`, `impact`, `types`, `depth`, `archive` | `spec graph REF --json`, browser view |
| GET `.../check` | -- | `spec check --json`, the plain run |
| GET `.../health` | -- | `spec health --json` (`docs/canon/health.md`) |
| GET `.../inbox` | -- | `spec inbox --json` |
| GET `.../proposals/:id` | -- | `spec review PR --json` (not `--brief`) |
| GET `.../tasks` | `status` | `spec task list --json` |
| GET `.../tasks/:id` | -- | `spec task show T --json`, uncut |
| GET `.../round`; POST `.../round/answers` | `limit`; -- | `spec round new --json`, uncut; stage answers (`round-ui.md`) |
| GET `.../events` | header `Last-Event-ID` | the live tail (SSE) |
| POST, DELETE `.../proposals/:id/decision` | -- | stage, unstage |
| GET `/`, `/assets/<file>` | ignored | the web UI (`serve-ui.md`) |

**Query**, raw (`args.rs`): split on `&` (empty parts skipped), each at its first `=`, form-decoded (`+` a space, a literal plus `%2B`), strictly: a bad `%` or non-UTF-8 -> 400 naming the parameter. Arrays repeat the key, in order; integers decimal, booleans `true`|`false`, `with` only `links`, `status` by `TaskStatus::parse` (a refusal lists `TaskStatus::ALL`). 400 also, nothing run: an unknown name (listing the endpoint's; `check`, `health`, `tasks/:id` take none), a scalar repeated, a bad value, `query`, `node_ids` or `ref` missing. **Path**: the slug, the proposal or task ID, the REF (all after `nodes/`) percent-decoded once, strictly, as UTF-8; the client sends `encodeURIComponent` (`#` -> `%23`, `/` -> `%2F`), `%2523` reaches the CLI as `%23`, an empty REF is an unknown route.

**Browser view** (CLI `View::Browser`; CLI and MCP keep `Capped`): no `OUTPUT_CAP_CHARS` cut, `graph` too (`truncated` false, `omitted` null, the same keys); a hit's `snippet` is `{segments: [{text, hit}], cut_start, cut_end}`, `null` when empty (the store's `SNIPPET_*` marks: a corpus `**` is text).

## Answers and statuses

One CLI library call per request (`answer.rs`: the process's `Env`, `Globals {root}`, `render_json`), the config found again first (a changed slug, a config `discover` refuses -> 503 with the reason, `check` too: no cannot-check report; `/api/projects` 503 if any). Exit 0 -> 200, the document; 1 -> 404, the exit-1 document (`reason` set: data, not an error); 2 -> 503, the error body, `message` the `CliError` line(s) verbatim. **The exceptions** (`answered`), never a verdict word: `Outcome::Check` (a report), `Outcome::RoundAnswers` 200 whatever the exit; `Outcome::Stage` by its cause (`docs/canon/decision-staging.md` "Daemon"); a `CliError` 503. **Error body** `{"status":<code>,"message":"..."}`, exactly two keys: 400, 403, 404 (an unknown slug or route, listing the served ones), 405, 413, 415, 500 (a caught panic), 503. Bodies `application/json; charset=utf-8`, the CLI's stdout less the final LF; stderr not carried (a read's notes: JSON `notes`).

**One door.** The write routes only stage, unstage (the CLI's `stage`, `unstage`, `round_stage_answers`), past `decision-staging.md` "Daemon"'s steps. No handler calls `approve`, `reject`, `propose`, `round_answer`, `import_state`, `export_*`, `init`, `index`, any `task_` but `task_list`, `task_show`; `check` only plain (`CheckRequest::default()`); `tests/door.rs` scans `src` for these and `Staged`, `Changed`, `baseline`, `RoundTerminal`, `WalkOutcome`, `AnswerOutcome`. No `src` literal holds a verdict, a `LINK_TYPES` name, a configured kind or a task state as a word, `decision`, `answers` only as whole route-path segments (ADR-0008, `tests/no_domain.rs`). The only writes, all in the data directory: stages in the queue, the reads' index refresh, the start's `daemon.lock`, `daemon.json` (`docs/canon/architecture.md#storage`); nothing under a root changes.

**Worktrees**: `tree`, `nodes`, `search`, `bundle`, `graph`, `check`, `health` read the root's files on disk (uncommitted edits too, never `HEAD`; `check`, `health` untracked and ignored ones, no git or database); `inbox` the root's repository, every worktree; `proposals/:id` previews in the proposal's recorded worktree, naming it and its branch; `tasks`, `tasks/:id` the root's repository and a task's compared place (`tasks.md` "Place"), by git reads only.

## Live tail

`GET .../events` (`tail.rs`): the project slug's queue `events` only. A stream opens with one poll (a newer build's queue or a changed slug: 503), then `id: <start>` and a blank line, so one dropped before its first event resumes there. `<start>`: the highest `seq` now (0 without a database), or `Last-Event-ID` when not above it (above: a new database, the highest). Per event `id: <seq>`, `event: <type>`, `data: <payload>` (the stored JSON, one line), a blank line; a `:` comment every 15 s. No `Last-Event-ID`: no replay; `Last-Event-ID: n`: exactly `seq > n`, at once; not a non-negative decimal integer, or sent twice -> 400.

Types: all stored, the seven `proposal.*` (`proposal-queue.md` "States and events"), the nine `task.*` (`tasks.md` "Store"), `gate.observed` (`gate.md` "Event"); a row with a NULL type or payload, or CR or LF in its type, skipped. Polls at most 250 ms apart, at once after a full page (`EVENTS_PAGE_MAX`, 512 rows read, skipped ones counted), each reads the config (a changed slug ends the stream) and runs one short read transaction on the stream's own connection (CLI `EventsTail`), kept between polls, reopened when the file is replaced, dropped when gone; no database created. A poll that cannot run ends the stream (the browser reconnects with its last ID); a client gone ends it and the connection.

## Concurrency

One request per project at a time, projects in parallel, beside the CLI's and MCP's direct writes: `serve.md` "Concurrency".

## Known limits

- Repositories of one slug share `<slug>.db` and its events: a tail carries every one's (the UI refetches).
- U+0002 to U+0004 in the corpus can fake a snippet's hit or cut, capped too.
- One `EventSource` per tab over HTTP/1.1: about six tabs exhaust Chrome's per-host connections.
- A database replaced under a live stream: its events with `seq` <= the stream's cursor are never sent.
- An uncut graph is big: 701 nodes, 700 edges ~220 KB.
- In the project's turn, other reads wait: `check`, `health` walk the tree (27 ms at 700 sections); a package read runs `git diff --no-index` per changed node (128: 1 to 1.5 s).

## Tests

`tests/common/mod.rs`: git byte copies of `fixtures/spec-a`, `-b`, a scratch `HOME`, a `--port 0` daemon per test (reaped on drop, alarm 300), raw HTTP/1.1, SSE, stdio MCP clients, a stale binary refused (build `--workspace --bins` first). Files: byte parity with `spec ... --json` (`parity.rs`, `graph.rs`, `check.rs`, `health.rs`, `tasks.rs`, state S: `task_state/`), source scans with positive controls (`door.rs`, `no_domain.rs`), `daemon_keys.rs` (asserts `fixtures/daemon-keys.json`, the UI's 46 key sets, a drift item raised on a code-a copy; `SPECENGINE_WRITE_DAEMON_KEYS=1` rewrites it), `drift.rs` (a drift item served, its stage 409), `turn.rs` (queued requests dropped, a read waits for its own project), `methods.rs`, `stage.rs`, `round.rs`, `projects.rs` (route and slug lists), `serve.rs` (`serve.md` "Tests"), `ui.rs` and the `src/ui.rs` units (`serve-ui.md` "Tests").
