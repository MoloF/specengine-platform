---
class: canon
tier: 1
scope: [crates/specengine-cli]
owner: owner
reviewed: 2026-11-12
---

# specengine-cli -- the spec binary

The CLI: the agent read loop (search, read by ID) over an index refreshed every call; `check`, `export index` over a fresh parse; the proposal queue; `gate` (`gate.md`: own exits, JSON on 2). Binary `spec`, a default member (ADR-0015). Dependencies: `specengine-{model,core,store,code}`, `toml` (`refs`), `clap`, `serde{,_json}`; never `rusqlite` nor `specengine-{import,mcp,http,eval,ra}` (eval `build_graph.rs`). `main.rs` parses and prints (+ terminal check); commands in the library, reused by `specengine-{mcp,http}`. Tests: `tests/`, a `HOME` each.

## API

`discover(&Env, &Globals) -> ProjectRoot`; `locate` -> `Located` (Discovery; config unread); `data_dir`, `{checked,prepared}_data_dir`, `db_path(&Env, slug)`, `open_index`; `init`, `index`, `search`, `show`, `tree`, `graph`, `impact`, `bundle`, `refs`, `symbols`, `unmapped`, `roadmap`, `check`, `health`, `verify{,_as}`, `lock_diff`, `bump`, `export_index` (`&Env, &Globals, &<Command>Request`) -> an outcome or `CliError` (whole stderr lines); `documents` -> `Vec<DocumentEntry>`, indexed live files by path (MCP `resources/list`); `specengine-http`'s: `{tree,show,search,graph,roadmap,delivery}_with_view`, `roadmap_flags`, `project_entry`, `events_after`, `EventsTail::after` -> `EventsPage`, `EVENTS_PAGE_MAX`, `stage`, `unstage` (`decision-staging.md`), `team_read`, `TeamRow` (`hub-team.md`), `confirm_{question,staged}`, `ConfirmOutcome::outside` (`hub-confirm.md`), `store_hub_floor` (`queue-backup.md` "ID floor"); `sync.rs` `sync_once`, `hub_sync`, `outputs.rs` `render_tree` (a sync's merge render) (`hub-git.md`); `round_{new,answer,stage,stage_answers}`, `round_limit`, `RoundAnswersBody`, `RoundTerminal`; `serve()`, the rest: `serve.md`; `Outcome::{exit, stderr_lines}`, `render_text`, `render_json`; `Exit {Answered, NotFound, CannotRun}`; the store's `MIN_TERM_CHARS`, `SEARCH_LIMIT_*`; `derive_slug`; `utc_now`, `process_git(&Env)`; re-exports core `intake`, `TEXT_MAX_BYTES`; for MCP `propose_drift` (`DriftRequest` -> `DriftOutcome`), `stage_form` (`StageFormOutcome`, `StageForm`), `staged_command`, `escape_controls` (`review-form.md`); `refresh_code` -> `CodeRefresh`, `listed_code_files` (`symbol-index.md`); `known_frameworks`, `role_counts` -> `RoleCounts` (`code-roles.md`); `SymbolsForm`, `SYMBOLS_LIMIT_*`, `symbols_limit_error`, row types, `SymbolRole` (`symbol-reads.md`); `HealthOutcome.{code_rows, drift}` (`health*.md`); `verify_tests(.., consent)` (`VerifyTestsRequest` -> `VerifyTestsOutcome`); `resolve` (`ResolveRequest` -> `ResolveOutcome`), `layer_c_state`, `RESOLVE_TIMEOUT_*`, `SymbolsLayerC` (`layer-c{,-reads}.md`); `reject_follow_up(.., core's FollowUp, consent)`; a hub's `IndexTrust`, `RECONCILE_EVERY`, `trusted`, `walking`, `trusting`, `doubt_in_scope` (`spec-read-freshness.md`). `Env {cwd, home, xdg_data_home}`, `CheckRequest.tree: CheckedTree`, `VerifyRequest {tree: VerifiedTree, task, node, symbol, git}` (`VerifyOutcome.{task, node}`, `VerifyNode`), `BumpRequest {staged, git}` are passed in. `specengine.toml` (`CONFIG_FILE`): core's `ProjectConfig` (`check`, `export index`: the store's loader).

## Commands

`--root DIR`, `--config FILE`, `--json` anywhere.

- `spec init [--slug S] [--pack PATH]` writes only `specengine.toml`, exclusively: `[project]\nslug = "<slug>"\n`, then comments only, or a blank line and a pack's bytes, checked first; JSON `{path, slug, pack}`; the slug, the checks, the presets: `presets.md`.
- `spec index [--full]`: store `update` (`--full`: `rebuild`); `indexed <slug>: walked 13, parsed 13, unchanged 0, removed 0, unreadable 0` (+ `, reparsed all`), `db <path>`; JSON `project`, `db`, `UpdateReport`'s fields. Then the code index: `code <slug>: …` before `db`, JSON `code` (`null` without `[code]`), its roles with `[code] frameworks` (`code-roles.md` "spec index"); a bad `[code]`: the md index printed, then its config line, exit 2; a code-walk gap: a `warning:`, exit 0 (`symbol-index.md` "spec refs and spec index").
- `spec search QUERY… [--kind K]… [--limit N] [--archive]`: FTS5 in the store. Terms under 3 chars dropped (a `note:`); none left -> exit 2 suggesting `spec show`; `--limit` 1..=200, default 20.
- `spec symbols QUERY… [--limit N] | --node REF | --file PATH | --at PATH:LINE` (one form) and `spec unmapped [PATH]`: the code index's lookup, bindings, roles (with `[code] frameworks`), layer C and coverage (`symbol-reads.md`).
- `spec show REF`: an ID, an alias (`aliases:`, `aliases_from`), `slug/ID`, `ID#SECTION` (`@rev` ignored, a `note:`) or a root-relative `.md` path (`spec-cli-read.md` "Resolution").
- In `docs/canon/`: `check`, `export index`: `spec-check-{cli,git}.md`; `health`: `health*.md`; `tree`, `graph`, `show --links`: `spec-cli-graph.md`; `impact` (`code_delta.rs`): `spec-impact.md`; `show --bindings`, bundle and package rows: `code-reads.md`; `--history`: `spec-history.md`; `bundle`: `spec-cli-bundle.md`; `roadmap`: `spec-roadmap.md`; `delivery`: `spec-delivery.md`; `flow`: `spec-flow.md`; `refs`: `code-bindings.md`, `spec-cli-refs.md`, `symbol-index.md`; `propose` (`similar.rs`), `inbox`, `review`, `approve`, `reject`: `proposal-*.md` (`propose drift`, a drift item's approve: `proposal-drift.md`; `reject --follow-up`: `proposal-completion.md` "Reject"); `approve`'s flags: `decision-record.md`; `propose question|discrepancy`, `--brief`: `agent-intake.md`; `export state`, `import-state`: `queue-backup.md`; `task …`, `--task`: `task{s,-package}.md`; `card …`: `card{s,-split,-upstream}.md`; `trace` (`TraceStage`, `TraceBadge`, `group_marks`), `card clear` (`CardCause::Outside`): `spec-trace.md`; `round`: `round{,-ui}.md`; `serve`: `serve.md`; `sparse-set` (`sparse.rs`): `hub-git.md`; `team` (`team.rs`): `hub-team.md`; `workspace`: `workspaces.md`; `lock …`, `tested.rs`: `spec-lock{,-accept,-rebase,-tested,-diff}.md`; `verify …`, anchors, a terminal's `--tests`: `spec-verify{,-output,-scope,-anchors,-tests}.md`; `resolve`: `layer-c.md` (their `group.rs`, the CLI's only `unsafe`); `bump`: `node-revision.md` "Raising".

**Discovery.** Without `--root` and `--config`, walk up from the canonical cwd to the first with `specengine.toml`; none -> exit 2 naming `spec init`. `--root DIR`: no walk. `--config FILE` replaces `<root>/specengine.toml`; without `--root` the root is the cwd. Config errors: `<config as given>:<line>: message`.

## Database

`~/Library/Application Support/specengine/<slug>.db`; elsewhere `$XDG_DATA_HOME/specengine/` when absolute, else `$HOME/.local/share/specengine/`. `HOME` unset, empty or relative -> exit 2. One DB per slug: index rows keyed `(project, root)`, proposals by git common dir (worktrees share it). A data directory inside the canonical root (nearest existing ancestor; a project at `$HOME`: a limit) -> exit 2, nothing created.

## Rules

- Writes: `index`, `resolve`, the reads (`search`, `show`, `tree`, `graph`, `impact`, `bundle`, `refs`, `symbols`, `unmapped`, `verify`, `health`, `roadmap`, `flow`, `trace`, `lock diff`) and the queue (`import-state`, `task`, `card`, `stage`, `gate`) only the data directory, `approve`, `round answer`, a hub's confirm also its target or a new file (`--editorial`: `spec.lock` too; a drift item: `spec.lock` alone) and the index's changed outputs (`outputs.rs`, `proposal-outputs.md`) and commit in the proposal's worktree, `lock accept`, `lock rebase`, `verify --tests` `spec.lock` and a commit, `regen` the file; `init` only its file; `export index` only the index, its shards and its roadmap view; `export state` only its dump, outside the repository; `check`, `bump`, `delivery`, `verify --staged`, `serve` nothing; nothing else under the root (`architecture.md#storage`) but what the project's command `verify --tests` runs writes.
- Freshness: reads run `freshness.rs`' step first, the stat-keyed `update` or a hub's trusted index (`spec-read-freshness.md`), never changing the exit: broken or unreadable files indexed with diagnostics; a missing root -> a `warning:` if `[paths]` is written, else none. `index`, `refs`, `symbols`, `unmapped`, `health`, `resolve` (with `[code]`: `show --bindings`, `bundle`, `task show`, `impact`) then refresh the code index; a code-walk gap: `index` warns, `health`'s rows 4-6 `cannot`, bindings reads note it, the rest refuse.
- Subset: `show`, `--links`, `bundle` read their REFs' rows and files, BLAKE3-checked, or walk (`subset.rs`; tests' walk: `#[doc(hidden)]` `show_walk`, `bundle_walk`): `spec-read-subset.md`.
- Tier 3: `search` drops it in the query, before the limit, unless `--archive`; `show` reaches it, marked ` | archived`.
- Determinism: one DB state (`check`, `delivery --as-of`: one tree, one date), byte-equal stdout; nothing rests on rowid, insertion, time or the absolute root.

## Exit codes and streams

0 answered, zero hits too. 1 `show`, `lock diff` found nothing: dangling, no configured prefix, a `.md` path or every holder unreadable or changed while read; `refs`, `symbols --node`, `impact`: REF, QPATH or REV names nothing; `roadmap --release R`: no such release; `check`, `verify` blocked. 2 could not run: usage, no project or slug, a config error, `HOME`, the data directory, a `StoreError`; `check` cannot-check; an `export index` refusal.

stdout: results only; `--json`: one compact document for exit 0 and 1, none for 2 (`check`, `verify`: its report; `bump`: `{rows, cannot}`; `index`: its report, `code` `null`); every key present, absent `null` (`check`: `Report::to_json` verbatim). stderr: `note:`, `warning:` lines (JSON `notes`: the notes only), then exit 1's `spec: <reason>` or exit 2's error (`export index`: `spec-check-cli.md`). No colour, no timing (but `resolve`'s stdout: `layer-c.md`); paths root-relative but `db`. **One-line rule**: every stderr message and JSON `reason`, `notes` is one line (CR, LF -> space); JSON `ref`, `path`, `holders` stay raw; a clap usage error keeps its `Usage:` block after the `spec:` line.

## Output and the cap

`search`, `show` text and JSON, `OUTPUT_CAP_CHARS` (40 000), tails: `spec-cli-read.md`.

## Open

The reads': `spec-cli-read.md` "Known limits".
