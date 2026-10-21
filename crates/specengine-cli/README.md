---
class: canon
tier: 1
scope: [crates/specengine-cli]
owner: owner
reviewed: 2026-10-21
---

# specengine-cli -- the spec binary

The CLI: the agent read loop (search, read by ID) over an index refreshed on every call; `check`, `export index` over a fresh parse; the proposal queue; `gate` (`gate.md`: own exits, JSON on 2). Binary `spec`, a default member (ADR-0015). Dependencies: `specengine-{model,core,store,code}`, `toml` (`refs`), `clap` (derive), `serde`, `serde_json`; never `rusqlite` nor `specengine-{import,mcp,http,eval,ra}` (eval `build_graph.rs`). `main.rs` parses and prints (+ terminal check); commands live in the library, reused by `specengine-{mcp,http}`. Tests: `tests/`, a `HOME` each.

## API

`discover(&Env, &Globals) -> ProjectRoot`; `locate` -> `Located` (Discovery; config unread); `data_dir`, `{checked,prepared}_data_dir`, `db_path(&Env, slug)`, `open_index`; `init`, `index`, `search`, `show`, `tree`, `graph`, `bundle`, `refs`, `symbols`, `unmapped`, `roadmap`, `check`, `verify`, `bump`, `export_index` (`&Env, &Globals, &<Command>Request`) -> an outcome or `CliError` (whole stderr lines); `documents` -> `Vec<DocumentEntry>`, indexed live files by path (MCP `resources/list`); `specengine-http`'s: `{tree,show,search,graph,roadmap}_with_view`, `roadmap_flags`, `project_entry`, `events_after`, `EventsTail::after` -> `EventsPage`, `EVENTS_PAGE_MAX`, `stage`, `unstage` (`decision-staging.md`); `round_{new,answer,stage,stage_answers}`, `round_limit`, `RoundAnswersBody`, `RoundTerminal`; `serve()`, the rest: `serve.md`; `Outcome::{exit, stderr_lines}`, `render_text`, `render_json`; `Exit {Answered, NotFound, CannotRun}`; the store's `MIN_TERM_CHARS`, `SEARCH_LIMIT_*`; `derive_slug`; `utc_now`, `process_git(&Env)`; re-exports core `intake`, `TEXT_MAX_BYTES`; `propose_drift` (`DriftRequest` -> `DriftOutcome`) for MCP; `refresh_code` -> `CodeRefresh`, `listed_code_files` (`symbol-index.md` "Crates and seams"); `known_frameworks`, `role_counts` -> `RoleCounts` (`roles.rs`, `code-roles.md`); `SymbolsForm`, `SYMBOLS_LIMIT_*`, `symbols_limit_error`, row types, `SymbolRole` (`symbol-reads.md` "Crates and seams"); `HealthOutcome.code_rows: CodeRows` (`health-code.md`); `verify_tests(.., consent)` (`VerifyTestsRequest` -> `VerifyTestsOutcome`); `resolve` (`ResolveRequest` -> `ResolveOutcome`), `layer_c_state` -> `LayerCState`, `RESOLVE_TIMEOUT_*` (`layer-c.md`); `reject_follow_up(.., FollowUp, consent)` beside `reject` (`follow_up.rs`; core's `FollowUp` re-exported). `Env {cwd, home, xdg_data_home}`, `CheckRequest.tree: CheckedTree {WorkingTree, Staged(GitEnv), Changed(GitEnv)}`, `VerifyRequest {tree: VerifiedTree {WorkingTree, Changed, Staged}, task, node, symbol, git}` (`VerifyOutcome.{task, node}`, `VerifyNode`), `BumpRequest {staged, git}` are passed in. `specengine.toml` (`CONFIG_FILE`): core's `ProjectConfig` (`check`, `export index`: the store's loader).

## Commands

`--root DIR`, `--config FILE`, `--json` go anywhere.

- `spec init [--slug S]` writes exactly `[project]\nslug = "<slug>"\n` (one there -> exit 2), prints `created <path> with slug <slug>`, JSON `{path, slug}`. Slug: `--slug`, else the directory name, ASCII letters and digits lower-cased, other runs -> `-`, trimmed; core `slug_problem` -> exit 2 naming `--slug`. Never walks; a config in an ancestor -> a `warning:`.
- `spec index [--full]`: store `update` (`--full`: `rebuild`); `indexed <slug>: walked 13, parsed 13, unchanged 0, removed 0, unreadable 0` (+ `, reparsed all`), `db <path>`; JSON `project`, `db`, `UpdateReport`'s fields. Then the code index: `code <slug>: …` before `db`, JSON `code` (`null` without `[code]`), its roles with `[code] frameworks` (`code-roles.md` "spec index"); a bad `[code]`: the md index printed, then its config line, exit 2; a code-walk gap: a `warning:`, exit 0 (`symbol-index.md` "spec refs and spec index").
- `spec search QUERY… [--kind K]… [--limit N] [--archive]`: the store's FTS5 search in its order. Terms under 3 characters dropped (a `note:`); none left -> exit 2 suggesting `spec show`; `--kind` free, repeatable; `--limit` 1..=200, default 20.
- `spec symbols QUERY… [--limit N] | --node REF | --file PATH | --at PATH:LINE` (one form) and `spec unmapped [PATH]`: the code index's lookup, bindings, roles (with `[code] frameworks`) and coverage (`symbol-reads.md`).
- `spec show REF`: an ID, an `aliases:` entry, an `aliases_from` legacy ID, `slug/ID`, `ID#SECTION` (`@rev` ignored with a `note:`) or a root-relative `.md` path.
- In `docs/canon/`: `check`, `export index`: `spec-check-{cli,git}.md`; `health`: `health{,-code}.md`; `tree`, `graph`, `show --links`: `spec-cli-graph.md`; `show --bindings`, bundle and package rows (`bindings.rs`, `bundle_with`, `Beside`, `beside_rows`, `named_set`, `row_line`, `row_binding`): `code-reads.md`; `bundle`: `spec-cli-bundle.md`; `roadmap`: `spec-roadmap.md`; `refs`: `code-bindings.md`, `spec-cli-refs.md`, `symbol-index.md`; `propose`, `inbox`, `review`, `approve`, `reject`: `proposal-*.md` (`propose drift`, a drift item's approve: `proposal-drift.md`; `reject --follow-up`: `proposal-completion.md` "Reject"); `approve`'s flags: `decision-record.md`; `propose question|discrepancy`, `--brief`: `agent-intake.md`; `export state`, `import-state`: `queue-backup.md`; `task …`, `--task`: `task{s,-package}.md`; `round`: `round{,-ui}.md`; `serve`: `serve.md`; `lock …`: `spec-lock{,-accept,-rebase}.md`; `verify`, `--staged`, `--task`, `--node`, anchors: `spec-verify{,-output,-scope,-anchors}.md`; `verify --tests [--task T]`, a terminal's: `spec-verify-tests.md`; `resolve`: `layer-c.md` (their process-group core `group.rs`, the CLI's only `unsafe`); `bump`: `node-revision.md` "Raising".

**Discovery.** Without `--root` and `--config`, walk up from the canonical cwd to the first with `specengine.toml`; none -> exit 2 naming `spec init`. `--root DIR`: no walk. `--config FILE` replaces `<root>/specengine.toml`; without `--root` the root is the cwd. Config errors: `<config as given>:<line>: message`.

## Database

`~/Library/Application Support/specengine/<slug>.db`; elsewhere `$XDG_DATA_HOME/specengine/` when absolute, else `$HOME/.local/share/specengine/`. `HOME` unset, empty or relative -> exit 2 (any host). One DB per slug: index rows keyed `(project, root)`, proposals by git common dir (worktrees share it). A data directory inside the canonical root (nearest existing ancestor; a project at `$HOME`: a limitation) -> exit 2, nothing created.

## Rules

- Writes: `index`, `resolve`, the reads (`search`, `show`, `tree`, `graph`, `bundle`, `refs`, `symbols`, `unmapped`, `verify`, `health`, `roadmap`) and the queue (`import-state`, `task`, `stage`, `gate`, `verify --tests` too) only the data directory, `approve`, `round answer` also its target or a new file (`--editorial`: `spec.lock` too; a drift item: `spec.lock` alone) and commit in the proposal's worktree, `lock accept`, `lock rebase` `spec.lock` and a commit, `regen` the file; `init` only its file; `export index` only the index, its shards and its roadmap view; `export state` only its dump, outside the repository; `check`, `bump`, `verify --staged`, `serve` nothing; nothing else under the root (`architecture.md#storage`) but what the project's command `verify --tests` runs writes.
- Freshness: the reads run `update` first, never changing the exit code: broken or unreadable files indexed with diagnostics; a missing root -> a `warning:` if `[paths]` is written, else silent. `index`, `refs`, `symbols`, `unmapped`, `health`, `resolve` (with `[code]`: `show --bindings`, `bundle`, `task show`) then refresh the code index; a code-walk gap: `index` warns, `health`'s rows 4-5 `cannot`, bindings reads note it, the rest refuse.
- Resolution is the check's: a `*.md` argument (queue targets too) is a path, a walked name exactly (not `is_clean_relative` -> exit 2; none -> exit 1 `is no indexed document`); else `grammar::parse_reference` (none -> exit 1 listing the prefixes; look-alike or mixed-script -> exit 2 naming the Latin fix; `project:` -> exit 2), then `Resolver::resolve_detached` over `SpecIndex::indexed_input`. Per holder, the nodes whose `id` is the ID, else its `aliases_from` target, else (an `aliases:` entry) the document; `#SECTION`: that section. Spans come from a parse of the very bytes printed, read once, never the index (non-UTF-8 -> U+FFFD, `utf8: false`); a holder whose fresh parse lost the ID is skipped. Several: all by `(path, ord)`, one `warning:`.
- Tier 3: `search` drops it in the query, before the limit, unless `--archive`; `show` reaches it, marked ` | archived`.
- Determinism: one DB state (`check`: one tree, one date), byte-identical stdout; nothing depends on rowid, insertion, time or the absolute root.

## Exit codes and streams

0 answered, zero hits too. 1 `show` found nothing: dangling, no configured prefix, a `.md` path unreadable (`cannot be read`), every holder unreadable (`none of its files could be read`) or changed while read; `refs`, `symbols --node`: the REF names nothing; `roadmap --release R`: no such release; `check`, `verify` blocked. 2 could not run: usage, no project or slug, a config error, `HOME`, the data directory, a `StoreError`, each exit 2 named above; `check` cannot-check; an `export index` refusal.

stdout: results only; `--json`: one compact document for exit 0 and 1, none for 2 (`check`, `verify`: its report; `bump`: `{rows, cannot}`; `index`: its report, `code` `null`); every key present, absent = `null` (`check`: `Report::to_json` verbatim). stderr: `note:`, `warning:` lines (JSON `notes`: the notes only), then exit 1's `spec: <reason>` or exit 2's error (`export index`: `spec-check-cli.md`). No colour, no timing (but `resolve`'s stdout: `layer-c.md`); paths root-relative but `db`. **One-line rule**: every stderr message and JSON `reason`, `notes` is one line (CR, LF -> space); JSON `ref`, `path`, `holders` stay raw; a clap usage error keeps its `Usage:` block after the `spec:` line.

## Output and the cap

`search` and `show` text and JSON, `OUTPUT_CAP_CHARS` (40 000 characters) and the tails: `docs/canon/spec-cli-read.md`.

## Open

The reads' known limits: `docs/canon/spec-cli-read.md` "Known limits".
