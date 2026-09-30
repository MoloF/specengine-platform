---
class: canon
tier: 1
scope: [crates/specengine-cli]
owner: owner
reviewed: 2026-09-30
---

# specengine-cli — the spec binary

The Phase 1 CLI: the agent read loop (pass 1, `docs/features/spec-cli.md`), search, then read by ID, over an index refreshed on every call (07 §1.2); `check`, `export index` over a fresh parse (pass 2a.1). Binary `spec`, a default member, `cargo install --path crates/specengine-cli` (ADR-0015). Normal dependencies: `specengine-{model,core,store}`, `clap` (derive), `serde`, `serde_json`, all workspace entries; never `rusqlite` (the store owns the DB) nor `specengine-{code,import,mcp,eval,ra}` (eval `build_graph.rs`). `main.rs` parses and prints; commands live in the library, which MCP stdio and the Phase 2 daemon bridge reuse.

## API

`discover(&Env, &Globals) -> ProjectRoot`; `data_dir`, `db_path(&Env, slug)`, `open_index`; `init`, `index`, `search`, `show`, `check`, `export_index` (`&Env, &Globals, &<Command>Request`) → an outcome or `CliError` (whole stderr lines); `Outcome::{exit, stderr_lines}`, `render_text`, `render_json`; `Exit {Answered = 0, NotFound = 1, CannotRun = 2}`; `OUTPUT_CAP_CHARS`; `derive_slug`. `Env {cwd, home, xdg_data_home}`, `CheckRequest.staged: Option<GitEnv>` (`main`: the process's) are passed in. `specengine.toml` (`CONFIG_FILE`, the store's): core's `ProjectConfig` (`check`, `export index`: all of it, by the store's loader).

## Commands

`--root DIR`, `--config FILE`, `--json` go anywhere.

- `spec init [--slug S]` writes exactly `[project]\nslug = "<slug>"\n` (`create_new`: a file there → exit 2; a partial one is removed), prints `created <path> with slug <slug>`, JSON `{path, slug}`. Slug: `--slug` validated, else the directory name with ASCII letters and digits lower-cased, every other run (non-UTF-8 bytes included) → `-`, trimmed (`My Project_2` → `my-project-2`); not `grammar::is_slug` or over 64 bytes → exit 2 naming `--slug`. Never walks; a config in an ancestor → a `warning:`.
- `spec index [--full]`: store `update` (`--full`: `rebuild`); `indexed <slug>: walked 13, parsed 13, unchanged 0, removed 0, unreadable 0` (+ `, reparsed all`), then `db <path>`; JSON `project`, `db` + the `UpdateReport` fields.
- `spec search QUERY… [--kind K]… [--limit N] [--archive]`: the store's FTS5 search in its order. Terms under 3 characters dropped with a `note:`; none left → exit 2 suggesting `spec show`; `--kind` free, repeatable; `--limit` 1..=200, default 20.
- `spec check [--staged] [--baseline F] [--debt]`, `spec export index [--stdout]`: `docs/canon/spec-check-cli.md`.
- `spec show REF`: `REF` is an ID, an `aliases:` entry, an `aliases_from` legacy ID, `slug/ID`, `ID#SECTION` (`@rev` ignored with a `note:`) or a root-relative `.md` path.

**Discovery.** Without `--root` and `--config`, walk up from the canonical current directory to the first holding a `specengine.toml` file; none → exit 2 naming `spec init`. `--root DIR`: no walk. `--config FILE` replaces `<root>/specengine.toml`; without `--root` the root is the current directory (read-only pilots). Config errors: `<config as given>:<line>: message`.

```toml
[project]               # closed; name, language optional
slug = "lantern-keep"   # index, search, show need it
name = "Lantern Keep"
language = "en"
```

## Database

Owner's answer Q1 (2026-09-30): `~/Library/Application Support/specengine/<slug>.db`; outside macOS (assumed: ADR-0003 names only macOS) `$XDG_DATA_HOME/specengine/` when absolute, else `$HOME/.local/share/specengine/`. `HOME` unset, empty or relative → exit 2 on every host. One DB per project, each worktree's rows keyed `(project, root)` by the store; no refusal by root in Phase 1; a repository-identity check (by git common dir, so task worktrees pass) comes with the Phase 2 queue. A data directory inside the canonical root (nearest existing ancestor; a project at `$HOME`: a limitation) → exit 2, nothing created. The DB is derived: `spec index` rebuilds it from git; only Phase 2's open proposals and tasks would be lost, `spec export` protects them (05 §8).

## Rules

- Writes: `index`, `search`, `show` only the data directory; `init` only its file; `export index` only `[paths] index`; `check` nothing; nothing else under the root (`docs/canon/architecture.md#storage`).
- Freshness: `search`, `show` run `update` first; a missing root → a `warning:` if `[paths]` is written, else silent; never exit 2.
- Indexing is never fatal: broken or unreadable files are indexed with diagnostics and change no exit code.
- Resolution is the check's: a `*.md` argument is a path (not `is_clean_relative` → exit 2); else `grammar::parse_reference` (none → exit 1 listing the prefixes; look-alike or mixed-script → exit 2 naming the Latin fix; `project:` → exit 2), then `Resolver::resolve_detached` over `SpecIndex::indexed_input`. Per holder, the nodes whose `id` is the ID, else its `aliases_from` target, else (an `aliases:` entry) the document; `#SECTION`: that section. Spans come from a parse of the very bytes printed, read once, never the index (non-UTF-8 → U+FFFD, `utf8: false`); a holder whose fresh parse lost the ID is skipped. Several: all by `(path, ord)`, one `warning:`.
- Tier 3 (`check::is_tier3_file`): `search` leaves it out in the query, before the limit, unless `--archive`; `show` reaches it, marked ` | archived`.
- Determinism: one DB state (`check`: one tree and one date), byte-identical stdout; nothing depends on rowid, insertion, time or the absolute root.

## Exit codes and streams

0 answered, zero hits included. 1 `show` found nothing: dangling, no configured prefix, a `.md` path not indexed or unreadable (`cannot be read`), every holder unreadable (`none of its files could be read`) or changed while read; `check` blocked. 2 could not run: usage, no project or slug, a config error, `HOME`, the data directory, a `StoreError`, each exit 2 named above; `check` cannot-check; an `export index` refusal.

stdout: results only; `--json`: one compact document for exit 0 and 1, none for 2 (`check`: its report); every key present, absent = `null` (`check`: `Report::to_json` verbatim). stderr: `note:`, `warning:` lines (JSON `notes`: the notes only), then exit 1's `spec: <reason>` or exit 2's error (`export index`'s config error: a `<config>:<line>: message` per cause). No colour, no timing; paths root-relative but `db`. **One-line rule**: every stderr message and JSON `reason`, `notes` is one line (CR, LF → space); JSON `ref`, `path`, `holders` stay raw; a clap usage error keeps its `Usage:` block after the `spec:` line.

## Output and the cap

- `search`: per hit `<id or path> | <kind or -> | <title or -> | <path>:<line>` (+ ` | archived`), then the snippet on one line, indented four spaces; then `hits 2 (limit 20); archived matches left out: 1 (--archive)` or `…; archive included`. JSON keys `archive, hits, kinds, limit, notes, query, tier3_left_out, truncated`; a hit `{id, kind, title, path, line, ord, archived, snippet}`, the snippet's line breaks raw.
- `show`: per node the search line's first four fields + ` | <tokens_est> tokens` (+ ` | status <s>` for a document, ` | rev <n>`, ` | archived`, ` | not UTF-8`), then its bytes (a section's span, a document's file; `\n` added if missing), an empty line between nodes. JSON `{ref, reason, notes, nodes}`, a node `{id, kind, title, path, line, end_line, status, rev, tokens_est, archived, utf8, sections, text, truncated, omitted}`; `omitted` `{lines: [a, b], sections, holders}` when cut.

`OUTPUT_CAP_CHARS` = 40 000 characters (07 §1.1) before the tail line:

- `show`: on the text (JSON: the sum of `text`), cut at the last line end within it; a longer first line (a header included) at the cap. Tail `[truncated: <path> lines <a>-<b> not shown; sections not shown: <IDs or none>; holders not shown: <path:line, … or none>]`, a section not shown when its heading line is cut; JSON drops the nodes after the cut one.
- `search`: on the text, cut at a hit boundary, the same hits in text and JSON; a `note:`, `truncated: true`, tail `[truncated: <k> of <n> hits not shown; lower --limit or narrow the query]`; `hits <n>` counts every store hit. The first hit is always printed: alone over the cap, its snippet is kept first, then name (ID, else path), kind and title share the rest, shortest first; cut to nothing → `-` in text, `null` in JSON. **Known limit:** a cut first hit's JSON `id` is a prefix, marked only by `truncated: true`.

## Open

- The note and tail say "title and snippet cut" when the name or kind was; a cut JSON `id` (never cut it, or mark it).
- `one_line` flattens only CR, LF: VT, FF, NEL, U+2028/2029, ESC in quoted input reach stderr and `reason`.
- A caught parser panic prints Rust's panic message (fix: a quiet panic hook).
- `show` reads over a concrete `WorkingTree`, so exit 1's `cannot be read`, `none of its files could be read` are untested (fix: `&dyn Source`).
- `show` decodes every index row per call: a lighter resolver input before MCP and the daemon.
- bm25 statistics span the DB: ranks shift across worktrees; moved or deleted roots' rows stay (no prune).

## Next passes

MCP stdio follows 3–4.

- `spec-cli-introduced` (2b shipped): `docs/canon/spec-check-cli.md`.
- 3 `spec-cli-graph`: `tree`, `graph`, `show --links`; default link types.
- 4 `spec-cli-bundle`: `bundle`, `bundle_hash`; Q7 token calibration, `rusqlite_migration`.

Tests: `tests/`, over copies of `fixtures/spec-a`, `-b` in temp dirs, each run with its own `HOME`.
