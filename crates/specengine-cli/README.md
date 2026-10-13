---
class: canon
tier: 1
scope: [crates/specengine-cli]
owner: owner
reviewed: 2026-10-13
---

# specengine-cli -- the spec binary

The CLI: the agent read loop (search, read by ID) over an index refreshed on every call; `check`, `export index` over a fresh parse; the proposal queue; `gate` (`gate.md`: own exits, JSON on 2). Binary `spec`, a default member (ADR-0015). Dependencies: `specengine-{model,core,store,code}`, `toml` (`refs`), `clap` (derive), `serde`, `serde_json`; never `rusqlite` nor `specengine-{import,mcp,http,eval,ra}` (eval `build_graph.rs`). `main.rs` parses and prints (+ terminal check); commands live in the library, reused by `specengine-{mcp,http}`. Tests: `tests/`, a `HOME` each.

## API

`discover(&Env, &Globals) -> ProjectRoot`; `locate` -> `Located` (Discovery; config unread); `data_dir`, `{checked,prepared}_data_dir`, `db_path(&Env, slug)`, `open_index`; `init`, `index`, `search`, `show`, `tree`, `graph`, `bundle`, `refs`, `check`, `verify`, `export_index` (`&Env, &Globals, &<Command>Request`) -> an outcome or `CliError` (whole stderr lines); `documents` -> `Vec<DocumentEntry>`, indexed live files by path (MCP `resources/list`); `specengine-http`'s: `{tree,show,search,graph}_with_view`, `project_entry`, `events_after`, `EventsTail::after` -> `EventsPage`, `EVENTS_PAGE_MAX`, `stage`, `unstage` (`decision-staging.md`); `round_{new,answer,stage,stage_answers}`, `round_limit`, `RoundAnswersBody`, `RoundTerminal`; `serve()`, the rest: `serve.md`; `Outcome::{exit, stderr_lines}`, `render_text`, `render_json`; `Exit {Answered, NotFound, CannotRun}`; the store's `MIN_TERM_CHARS`, `SEARCH_LIMIT_{MIN,MAX,DEFAULT}`; `derive_slug`; `utc_now`, `process_git(&Env)`; re-exports core `intake`, `TEXT_MAX_BYTES`. `Env {cwd, home, xdg_data_home}`, `CheckRequest.tree: CheckedTree {WorkingTree, Staged(GitEnv), Changed(GitEnv)}` are passed in. `specengine.toml` (`CONFIG_FILE`): core's `ProjectConfig` (`check`, `export index`: the store's loader).

## Commands

`--root DIR`, `--config FILE`, `--json` go anywhere.

- `spec init [--slug S]` writes exactly `[project]\nslug = "<slug>"\n` (one there -> exit 2), prints `created <path> with slug <slug>`, JSON `{path, slug}`. Slug: `--slug`, else the directory name, ASCII letters and digits lower-cased, other runs -> `-`, trimmed; core `slug_problem` -> exit 2 naming `--slug`. Never walks; a config in an ancestor -> a `warning:`.
- `spec index [--full]`: store `update` (`--full`: `rebuild`); `indexed <slug>: walked 13, parsed 13, unchanged 0, removed 0, unreadable 0` (+ `, reparsed all`), `db <path>`; JSON `project`, `db`, `UpdateReport`'s fields.
- `spec search QUERY… [--kind K]… [--limit N] [--archive]`: the store's FTS5 search in its order. Terms under 3 characters dropped (a `note:`); none left -> exit 2 suggesting `spec show`; `--kind` free, repeatable; `--limit` 1..=200, default 20.
- `spec show REF`: `REF` is an ID, an `aliases:` entry, an `aliases_from` legacy ID, `slug/ID`, `ID#SECTION` (`@rev` ignored with a `note:`) or a root-relative `.md` path.
- In `docs/canon/`: `check`, `export index`: `spec-check-{cli,git}.md`; `health`: `health.md`; `tree`, `graph`, `show --links`: `spec-cli-graph.md`; `bundle`: `spec-cli-bundle.md`; `refs`: `code-bindings.md`; `propose`, `inbox`, `review`, `approve`, `reject`: `proposal-*.md`; `approve`'s flags: `decision-record.md`; `propose question|discrepancy`, `--brief`: `agent-intake.md`; `export state`, `import-state`: `queue-backup.md`; `task …`, `--task`: `task{s,-package}.md`; `round`: `round{,-ui}.md`; `serve`: `serve.md`; `lock …`: `spec-lock{,-accept}.md`; `verify`: `spec-verify{,-output}.md`.

**Discovery.** Without `--root` and `--config`, walk up from the canonical cwd to the first with `specengine.toml`; none -> exit 2 naming `spec init`. `--root DIR`: no walk. `--config FILE` replaces `<root>/specengine.toml`; without `--root` the root is the cwd. Config errors: `<config as given>:<line>: message`.

## Database

`~/Library/Application Support/specengine/<slug>.db`; elsewhere `$XDG_DATA_HOME/specengine/` when absolute, else `$HOME/.local/share/specengine/`. `HOME` unset, empty or relative -> exit 2 (any host). One DB per slug: index rows keyed `(project, root)`, proposals by git common dir (worktrees share it). A data directory inside the canonical root (nearest existing ancestor; a project at `$HOME`: a limitation) -> exit 2, nothing created.

## Rules

- Writes: `index`, the reads (`search`, `show`, `tree`, `graph`, `bundle`, `refs`, `verify`) and the queue (`import-state`, `task`, `stage`, `gate` too) only the data directory, `approve`, `round answer` also its target (or a new file) and commit in the proposal's worktree, `lock accept` `spec.lock` and a commit, `regen` the file; `init` only its file; `export index` only the index and its shards; `export state` only its dump, outside the repository; `check`, `health`, `serve` nothing; nothing else under the root (`architecture.md#storage`).
- Freshness: the reads run `update` first, never changing the exit code: broken or unreadable files indexed with diagnostics; a missing root -> a `warning:` if `[paths]` is written, else silent.
- Resolution is the check's: a `*.md` argument (queue targets too) is a path, a walked name exactly (not `is_clean_relative` -> exit 2; none -> exit 1 `is no indexed document`); else `grammar::parse_reference` (none -> exit 1 listing the prefixes; look-alike or mixed-script -> exit 2 naming the Latin fix; `project:` -> exit 2), then `Resolver::resolve_detached` over `SpecIndex::indexed_input`. Per holder, the nodes whose `id` is the ID, else its `aliases_from` target, else (an `aliases:` entry) the document; `#SECTION`: that section. Spans come from a parse of the very bytes printed, read once, never the index (non-UTF-8 -> U+FFFD, `utf8: false`); a holder whose fresh parse lost the ID is skipped. Several: all by `(path, ord)`, one `warning:`.
- Tier 3 (`check::is_tier3_file`): `search` drops it in the query, before the limit, unless `--archive`; `show` reaches it, marked ` | archived`.
- Determinism: one DB state (`check`: one tree, one date), byte-identical stdout; nothing depends on rowid, insertion, time or the absolute root.

## Exit codes and streams

0 answered, zero hits too. 1 `show` found nothing: dangling, no configured prefix, a `.md` path unreadable (`cannot be read`), every holder unreadable (`none of its files could be read`) or changed while read; `check`, `verify` blocked. 2 could not run: usage, no project or slug, a config error, `HOME`, the data directory, a `StoreError`, each exit 2 named above; `check` cannot-check; an `export index` refusal.

stdout: results only; `--json`: one compact document for exit 0 and 1, none for 2 (`check`, `verify`: its report); every key present, absent = `null` (`check`: `Report::to_json` verbatim). stderr: `note:`, `warning:` lines (JSON `notes`: the notes only), then exit 1's `spec: <reason>` or exit 2's error (`export index`: `spec-check-cli.md`). No colour, no timing; paths root-relative but `db`. **One-line rule**: every stderr message and JSON `reason`, `notes` is one line (CR, LF -> space); JSON `ref`, `path`, `holders` stay raw; a clap usage error keeps its `Usage:` block after the `spec:` line.

## Output and the cap

- `search`: per hit `<id or path> | <kind or -> | <title or -> | <path>:<line>` (+ ` | archived`), then the snippet on one line, four spaces in; then `hits 2 (limit 20); archived matches left out: 1 (--archive)` or `…; archive included`. JSON keys `archive, hits, kinds, limit, notes, query, tier3_left_out, truncated`; a hit `{id, kind, title, path, line, ord, archived, snippet}`, the snippet's line breaks raw.
- `show`: per node the search line's first four fields + ` | <tokens_est> tokens` (+ ` | status <s>` for a document, ` | rev <n>`, ` | archived`, ` | not UTF-8`, ` | norm <norm_hash>`) + ` | span <span_hash>`, then its bytes (a section's span, a document's file; `\n` added if missing), an empty line between nodes. JSON `{ref, reason, notes, nodes}`, a node `{id, kind, title, path, line, end_line, status, rev, tokens_est, archived, utf8, sections, norm_hash, span_hash, text, truncated, omitted, links}`, `span_hash` `b3:` of the whole span's bytes read (`propose --base`), `norm_hash` null or `node-revision.md`'s; `omitted` `{lines: [a, b], sections, sections_more, holders, holders_more}` when cut.

`OUTPUT_CAP_CHARS` = 40 000 characters before the tail line (`View::Capped`; the daemon's `Browser` is uncut, `graph` too):

- `show`: on the text (JSON: the sum of `text`), cut at the last line end within it; a longer first line (a header included) at the cap. Tail `[truncated: <path> lines <a>-<b> not shown; sections not shown: <IDs or none>; holders not shown: <path:line, … or none>]`, a section not shown when its heading line is cut. Each list: the first `SHOW_TAIL_NAMES` (20; sections by source, holders by print order), then `, <k> more`. JSON drops the nodes after the cut one; the cut node's `sections` holds only the IDs whose heading line ends in its `text`, `omitted` the names by the same rule at the JSON's cut (`--links`: the text's), `*_more` the k (0 if none).
- `search`: on the text, cut at a hit boundary, the same hits in text and JSON; a `note:`, `truncated: true`, tail `[truncated: <k> of <n> hits not shown; lower --limit or narrow the query]`; `hits <n>` counts every store hit. The first hit is always printed: alone over the cap, its snippet is kept first, then name (ID, else path), kind and title share the rest, shortest first; cut to nothing -> `-` in text, `null` in JSON.

## Open

- Note and tail say "title and snippet cut" for a cut name or kind too; a cut first hit's JSON `id` is marked by `truncated: true` alone.
- `one_line` flattens only CR, LF: VT, FF, NEL, U+2028/2029, ESC in quoted input reach stderr and `reason`.
- A caught parser panic prints Rust's panic message (fix: a quiet panic hook).
- `show`'s `cannot be read` exits are untested (a concrete `WorkingTree`; fix: `&dyn Source`).
- The reads decode every index row per call (`mcp-read.md` "Latency").
- bm25 statistics span the DB: ranks shift across worktrees; moved or deleted roots' rows stay (no prune).
