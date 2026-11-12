---
class: canon
tier: 1
scope: [crates/specengine-http]
owner: owner
reviewed: 2026-11-13
---

# specengine-http -- SpecEngine over HTTP

An HTTP adapter over the CLI library: the CLI's `--json` documents, the queue's live `events` and stages, on `127.0.0.1`, for the web UI (`ui/README.md` "Contract seam"). The daemon, one per data directory, `spec serve` launches (`serve.md`), serving the web UI compiled in (`serve-ui.md`); not ADR-0019's: no sole writer, bridge, auto-start, gate, watcher. A staged decision is confirmed on a terminal (ADR-0035) or, on a hub, `hub-confirm.md`; no hook consults it (ADR-0039).

Dependencies: `specengine-cli` (never the store or `rusqlite`; no reverse edge), `specengine-mcp` and `rmcp`, named in `src/mcp.rs` alone, `axum`, `rust-embed`, others `Cargo.toml`'s; a std-only build script `src/build.rs`; a default member; features: `stack.md`; pins, licences: eval `build_graph.rs`.

## Binary

`specengine-http --root DIR... [--port N] [--public-origin URL [--push ...]]` (`main.rs`, `start.rs` `served`, `daemon.rs`), foreground; `--root` repeats, served in order; `--port` 7777, `0` a free one; `127.0.0.1` only (`--host`, `--bind` -> exit 2). The start, its stdout lines and its exit 2 before listening (one stderr line `specengine-http: <reason>`), a hub's floor: `serve.md` "Start", "Lines"; `--push`, `sync.rs`: `hub-git.md`. Exit 1: stopped after listening.

## Fence

Around the whole router (`app.rs` `fence`), failing -> 403 without `Allow`, nothing read: `Host` exactly `127.0.0.1:<port>` or `localhost:<port>`; `Origin` absent or `http://` + one of them; `Sec-Fetch-Site` absent, `same-origin` or `none`, but for the landing (`serve-ui.md` "Rules"); any sent twice -> 403. No authentication (ADR-0034). No `Access-Control-*` header, `OPTIONS` included; `Cache-Control: no-store` always. A hub's, `--public-origin`: `hub.md`.

Methods, past the fence: a read route takes GET only, any other (HEAD too) -> 405 `Allow: GET`; the decision route POST and DELETE, else 405 `Allow: POST, DELETE`, `round/answers` POST, else `Allow: POST`, both ending ``confirmed on a terminal``; the card routes `board-ui.md`'s; an unknown path -> 404 listing the routes, whatever the method.

## Endpoints

`:p` a served slug; query names are the MCP arguments (`mcp-read.md` "Tools"), `graph`'s its JSON echo keys, `tasks`' `--status`.

| Method, path | Query | Answer |
|---|---|---|
| GET `/api/projects` | -- | `[{slug, name, root, branch}]`, `--root` order (CLI `project_entry`): `name` or `null`, `root` canonical, `branch` current or `null` (detached, no git) |
| GET `/api/identity`, `.../workspaces`; a hub's `.../team`, GET, POST `.../proposals/:id/confirm`; a `--push` hub's `.../sync` | -- | `hub{,-department,-team,-git}.md`, `workspaces.md` |
| GET `/api/projects/:p/tree` | `root`, `depth`, `kinds`, `archive` | `spec tree --json` |
| GET `.../nodes/{*ref}` | `with`, `archive` | `spec show REF --json` |
| GET `.../search` | `query`, `kinds`, `limit`, `archive` | `spec search --json` |
| GET `.../bundle` | `node_ids`, `budget` | `spec bundle --json` |
| GET `.../graph` | `ref`, `impact`, `types`, `depth`, `archive` | `spec graph REF --json` |
| GET `.../flow` | `spec-flow.md`'s | `spec flow --json` |
| GET `.../check`, `.../health` | -- | the plain `spec check --json`; `spec health --json` (`health*.md`) |
| GET `.../roadmap` | `release`, `pool` | `spec roadmap --json` |
| GET `.../delivery` | `as_of` | `spec delivery --json`, `spec-delivery.md` |
| GET `.../catalog` | `kinds`, `archive` | `spec catalog --json`, `App.catalogs` cached per `HEAD`, `spec-catalog.md` |
| GET `.../lock-diff` | `ref` | `spec lock diff REF --json` |
| GET `.../inbox`, `.../proposals/:id` | -- | `spec inbox --json`; `spec review PR --json` (not `--brief`) |
| GET `.../tasks`, `.../tasks/:id` | `status`; -- | `spec task list --json`; `spec task show T --json`, uncut |
| GET `.../round`; POST `.../round/answers` | `limit`; -- | `spec round new --json`, uncut; stage answers (`round-ui.md`) |
| GET `.../events` | header `Last-Event-ID` | the live tail (SSE) |
| POST, DELETE `.../proposals/:id/decision` | -- | stage, unstage |
| GET `.../cards`, `.../cards/:id`, `.../cards/counts`, `.../trace`; POST `.../cards`, `.../cards/split`, `:id`'s seven verbs, `.../trace/clear` | `board-ui.md`, `card-counts.md` | Board, Overview |
| POST, GET, DELETE `/mcp/<slug>`, a hub's alone (`mcp.rs`) | refused | MCP (`hub-mcp.md`) |
| GET `/`, `/assets/<file>` | ignored | the web UI (`serve-ui.md`) |

**Query**, raw (`args.rs`): split on `&` (empty parts skipped), each at its first `=`, form-decoded (`+` a space), strictly: a bad `%` or non-UTF-8 -> 400 naming the parameter. Arrays repeat the key, in order; integers decimal, booleans `true`|`false`, `with` `links`, `bindings`, `history`, `status` by `TaskStatus::parse`. 400 also, nothing run: an unknown name (listing the endpoint's; `check`, `health`, `tasks/:id` take none), a scalar repeated, a bad value, `release` with `pool=true`, `query`, `node_ids` or `ref` missing. **Path**: the slug, the proposal or task ID, the REF (all after `nodes/`) percent-decoded once, strictly, as UTF-8; the client sends `encodeURIComponent`; an empty REF is an unknown route.

**Browser view** (CLI `View::Browser`; CLI and MCP keep `Capped`) of `tree`, `nodes`, `search`, `graph`, `flow`, `roadmap`, `delivery`, `catalog`, `trace`: no `OUTPUT_CAP_CHARS` cut (`truncated` false, `omitted` null, `roadmap`'s 0, the same keys); a hit's `snippet` is `{segments: [{text, hit}], cut_start, cut_end}`, `null` when empty.

## Answers and statuses

One CLI library call per request (`answer.rs`: the process's `Env`, `Globals {root}`, `render_json`), the config found again first (a changed slug, a config `discover` refuses -> 503 with the reason, `check` too; `/api/projects` 503 if any). Exit 0 -> 200, the document; 1 -> 404, the exit-1 document; 2 -> 503, the error body, `message` the `CliError` line(s) verbatim. **The exceptions** (`answered`), never a verdict word: `Outcome::Check` (a report), `Outcome::RoundAnswers` 200 whatever the exit; `Outcome::{Stage,Confirm}`, `Outcome::Card{,Group}` by their cause (`caused`; `decision-staging.md` "Daemon"); a `CliError` 503. **Error body** `{"status":<code>,"message":"..."}`, exactly two keys: 400, 403, 404 (an unknown slug or route, listing the served ones), 405, 413, 415, 500 (a caught panic), 503. Bodies `application/json; charset=utf-8`, the CLI's stdout less the final LF; stderr not carried (a read's notes: JSON `notes`).

**One door.** The write routes stage, unstage (the CLI's `stage`, `unstage`, `round_stage_answers`), past `decision-staging.md` "Daemon"'s steps, or write a card (`board-ui.md`). No handler calls `approve`, `reject`, `propose`, `round_answer`, `import_state`, `export_*`, `init`, `index`, any `task_` but `task_list`, `task_show`, any `card_` outside `CARD_CALLS` (13), `confirm_*` outside `src/confirm.rs`; `check` only plain (`CheckRequest::default()`); `tests/door.rs` scans `src` for these and six more. A departing accept's proposal stands inside `card_accept` (ADR-0057), a card's question or decision inside `card_raise` (ADR-0077); a hub's `/mcp` raises proposals and writes cards through `specengine-mcp`'s hub server (`hub-mcp.md`). No `src` literal holds a verdict, a `LINK_TYPES` name, a configured kind, a workspace or module name or a task state as a word, `decision`, `answers` only as whole route-path segments (ADR-0008, `tests/no_domain.rs`). The only writes, but a hub's confirm, in the data directory: stages and cards in the queue, the reads' index refresh, the start's `daemon.lock`, `daemon.json` (`architecture.md#storage`).

**Worktrees**: the corpus reads read the root's files on disk (uncommitted edits too, never `HEAD` but `catalog`'s last change; `check`, `health` untracked and ignored ones, `check` no git); `inbox` the root's repository, every worktree; `proposals/:id` previews in the proposal's recorded worktree, naming it and its branch; `tasks`, `tasks/:id` the root's repository and a task's compared place (`tasks.md` "Place"), by git reads only.

## Live tail

`GET .../events` (`tail.rs`): the project slug's queue `events` only. A stream opens with one poll (a newer build's queue or a changed slug: 503), then `id: <start>` and a blank line, so one dropped before its first event resumes there. `<start>`: the highest `seq` now (0 without a database), or `Last-Event-ID` when not above it (above: a new database, the highest). Per event `id: <seq>`, `event: <type>`, `data: <payload>` (the stored JSON, one line), blank; a `:` comment every 15 s. No `Last-Event-ID`: no replay; `Last-Event-ID: n`: exactly `seq > n`, at once; not a non-negative decimal integer, or sent twice -> 400. On a hub it ends after 300 s; types and polls: `serve.md` "Live tail".

## Concurrency

One request per project at a time, projects in parallel: `serve.md` "Concurrency". On a hub each call runs in its project's `IndexTrust` scope (`Project.trust`): read routes trust (`run_read`, `run_as`), the rest walk; a panic or exit 2 doubts (`spec-read-freshness.md`).

## Known limits

- Repositories of one slug share `<slug>.db` and its events: a tail carries every one's.
- U+0002 to U+0004 in the corpus can fake a snippet's hit or cut.
- A database replaced under a live stream: its events up to the stream's cursor are never sent.

## Tests

`tests/common/mod.rs`: git byte copies of `fixtures/spec-a`, `-b`, a scratch `HOME`, a `--port 0` daemon per test, raw HTTP/1.1, SSE, stdio MCP clients, a stale binary refused (build `--workspace --bins`; a `#[cfg(test)]` edit relinks no bin). Files: byte parity with `spec ... --json` (`{parity,graph,flow,check,health,tasks,roadmap,delivery,catalog,workspaces}.rs`, state S: `task_state/`), source scans with positive controls (`door.rs`, `no_domain.rs`), `daemon_keys.rs` (`fixtures/daemon-keys.json`, the UI's key sets; `SPECENGINE_WRITE_DAEMON_KEYS=1` rewrites it), a file per surface (a hub's `hub{,_team,_confirm,_ids,_git,_mcp,_trust}.rs`, `common/mcp_http.rs`; its deployment's files, `deploy{,_admin,_ops}.rs`, `hub-deploy.md`), `serve.rs` (`serve.md`), `ui.rs` and the `src/ui.rs` units (`serve-ui.md`).
