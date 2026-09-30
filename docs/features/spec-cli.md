---
class: spec
status: shipped
scope: [crates/specengine-cli, crates/specengine-core, crates/specengine-store]
ref: 08 §2 Phase 1 CLI, pass 1 of 5; 07 §1-2; owner's answer Q1 (2026-09-30)
shipped: 2026-09-30
adrs: []
---

# The spec CLI, pass 1: the agent read loop

## Why

A task's context cost must not grow with the project: an agent finds and reads by ID instead of grepping and opening whole files (07 §1.2: "search instead of list-all", "read-more by id"). Parser, index and check were only libraries (store README Q6).

The `spec` binary fixes the contracts MCP stdio, the Phase 2 daemon bridge (05 §1 principle 6, ADR-0019) and the hook inherit: where the project and its database are, what a command may write, what an answer looks like (bounded, deterministic, with an exit code); plus `search` → `show` over an always-fresh index, logic below `main`. No ADR: the DB's place is the owner's answer Q1 inside ADR-0003. What shipped is canon in `crates/specengine-cli/README.md` (commands, config, database, exit codes, streams, output, the cap, open items, passes 2a–4); the core and store additions in their READMEs, `docs/canon/spec-check-links.md` "Resolution", `docs/canon/spec-check-graph.md` "Tier 3".

## Acceptance criteria

In `crates/specengine-cli/tests/` unless named, over copies of `fixtures/spec-a`, `-b` under `std::env::temp_dir()` (never `CARGO_TARGET_TMPDIR`); tests spawn only `env!("CARGO_BIN_EXE_spec")`, each with its own `HOME`, no `XDG_DATA_HOME`; each named mutation turns its test red.

- [x] AC-01 `crates/specengine-eval/tests/build_graph.rs`: nine default members incl. `specengine-cli`; its linked graph (`-e normal,no-proc-macro`) has `specengine-{model,core,store}`, lacks `specengine-{code,import,mcp,eval,ra}`, `tokio`, `rmcp`, `syn`, `ra_ap_*` (`syn` enters only through the `clap_derive`, `serde_derive` proc-macros, as for model, core, store); `rusqlite` only via the store; direct dependencies all workspace entries. Mutations: `rusqlite.workspace = true` in the CLI; the CLI out of `default-members`.
- [x] AC-02 `discovery.rs`: `spec show MEC-STAMINA` from the root, from `docs/spec/movement`, with `--root` from elsewhere → identical stdout; no config up the tree → exit 2, empty stdout, stderr names `spec init`. Mutation: current directory only.
- [x] AC-03 `--config`: the copy's config deleted, `--root <copy> --config <outside>` serves `index`, `search`, `show`; the copy unchanged. Mutation: `--config` ignored.
- [x] AC-04 `config.rs`: exit 2, one stderr line at the right line, no data directory: an unknown `[project]` key; slug `"Lantern"`, `"1x"`, `"a/b"`, `"../x"`, `""`, `3`, 65 characters; no slug for `index`, `search`, `show`; a bad `[ids]` prefix; an absolute `[paths]` root; an unknown table. Mutation: any string is a slug.
- [x] AC-05 `location.rs`: `index` creates the host rule's `<slug>.db` (+ at most `-wal`, `-shm`) and nothing else under `HOME`; two slugs → two files; a second copy of one slug shares the DB, `show` works from both; `HOME` unset or inside the copy → exit 2, nothing created. Mutation: the DB under the root.
- [x] AC-06 `init.rs`: in empty `My Project_2` writes exactly `[project]\nslug = "my-project-2"\n`, then `index` exits 0; rerun or a pre-existing file → exit 2, bytes unchanged; `123` or a Cyrillic (escaped) name → exit 2 naming `--slug`; `--slug x-1` kept; `--slug Bad` → exit 2, nothing written; a failed write leaves no partial file. Mutation: a non-exclusive create.
- [x] AC-07 `index.rs`: spec-a `walked 13, parsed 13`; rerun `parsed 0, unchanged 13`; a line appended → `parsed 1`; `--full` → `reparsed_all`; `--json` = `project`, `db` + `UpdateReport` keys; an unclosed front-matter, a non-UTF-8 and an unreadable file → exit 0, counted. Mutations: `--full` calling `update`; `?` on a broken file.
- [x] AC-08 `freshness.rs`: after `index`, a word appended to `stamina.md` is found by `search` with `MEC-STAMINA` among hits; `R-12.md` deleted → `show R-12` exits 1. Mutation: no update before reads.
- [x] AC-09 `show.rs`: `RULE-STAM-REGEN` → a header with `docs/spec/movement/stamina.md:21`, then lines 21–23 byte for byte; `MEC-STAMINA` → the whole file; `docs/spec/game.md` → that file; `--json` keys exactly as the CLI README. Mutation: print the stored `own_text`.
- [x] AC-10 `resolve.rs`: `TERM-tired` → `TERM-exhausted`; `QST-031` → `Q-031`; `stamina-tuning/AC-07`, `MEC-STAMINA#RULE-STAM-REGEN` → sections; `R-12@3` → `R-12` + a `note:`; `{#AC-07}` added to a second feature document → bare `AC-07` prints both, exit 0, one warning; `R-99`, `FOO-1` → exit 1; `\u{0410}-101` → exit 2 naming `A-101`; `other:R-12` → exit 2; spec-b `\u{0422}\u{0420}\u{0411}-001` → `REQ-001`; every form's holders equal `resolve_detached` over the walk-fed `check_input`. Mutation: `lookup_id` only.
- [x] AC-11 `archive.rs`: a word only in `DEC-0007` (superseded) → zero hits, exit 0, the summary names 1 excluded match; `--archive` finds it marked; `show DEC-0007` prints it ` | archived`; three archived nodes outranking three live ones → `--limit 3` gives the live three. Mutation: filter after the limit.
- [x] AC-12 `search.rs`: the store's order, a line + snippet per hit; `--kind rule` only rules; only short terms → exit 2 naming `spec show`; no hits → exit 0; spec-b finds a capitalised Cyrillic word (escaped) by its lower case; two runs identical. Mutation: sort by path only.
- [x] AC-13 `bounds.rs`: a ~100 000-character document → stdout before the tail ≤ 40 000 characters ending at a line end, one tail line with path, omitted lines, sections not shown, holders after the cut; `--json` `truncated: true`, `text` ≤ 40 000; a first line or a header longer than the cap is cut at it, never dropped. `search` over the cap: cut at a hit boundary, ≤ 40 000 characters before the tail, the same hits in `--json` with `truncated: true`; a first hit alone over the cap is printed cut (a huge kind keeps the title, a huge ID is cut to fit). `--limit 0`, `201` → exit 2. Mutation: no cap.
- [x] AC-14 `exit.rs`: every exit case of the CLI README across the four commands, text and `--json`; `--json` stdout one document for exit 0 and 1, empty for 2; errors only on stderr, each line `spec:`, `warning:`, `note:` or `<config>:<line>:` (a usage error: its first line, clap's `Usage:` block after); no ANSI escape or timing. `names.rs`, the one-line rule: a line break in a file name, a cited path, an exit-2 message or an exit-1 reference leaves every stderr line and JSON `reason`, `notes` on one line, JSON `ref` raw. `spec show <unreadable indexed path>` exits 1, reason `cannot be read` (code only, untested: CLI README "Open"). Mutation: an unknown ID exiting 0.
- [x] AC-15 `read_only.rs`: spec-a and spec-b copies byte- and path-identical after `index`, `index --full`, `search`, every AC-10 form; new files only in the data directory. Mutation: a lock file under the root.
- [x] AC-16 `determinism.rs`: two copies written in opposite orders, separate `HOME`s → identical `show` and `search` stdout. Mutation: holders by insertion order.
- [x] AC-17 `genre.rs` (ADR-0008): AC-06…AC-15 on both fixtures; `crates/specengine-cli/src` holds none of `docs/`, `ADR`, `CLAUDE.md`, `index.md`, `RULE-`, `MEC-`, `REQ-`, `mechanic`. Mutation: a `"docs/"` default.
- [x] AC-18 `crates/specengine-eval/tests/anonymity.rs` green with the crate; no committed output holds a DB path. Mutation: a temp-directory path in an expected output.
- [x] AC-19 Store tests (`tier3.rs`, `format.rs`, `genre.rs`): `tier3`, `line`, `archive` before `LIMIT`, `tier3_left_out`, `indexed_input` = walk-fed `check_input` in paths, sizes, parses, read errors; `INDEX_FORMAT` 6 with its history line; incremental = rebuild. Core tests (`project_config.rs`, `detached.rs`): loader, `is_tier3_file`, `resolve_detached`. Mutations: a column without a history line; a bare feature-scoped ID left dangling by `resolve_detached`.
- [x] AC-20 `cargo xtask docs index --write && cargo xtask docs check` green; `cargo xtask docs budget` worst W ≤ 117 756 B at shipping; `cargo nextest run --workspace` once; clippy and fmt clean.

## Implementation

Four iterations, review accepted in each; the full workspace run: 787 passed, 0 failed, 15 skipped. (1) The crate (binary `spec`, logic in the library), core's `ProjectConfig`, public `is_tier3`, `is_live`, `is_tier3_file`, `Resolver::resolve_detached`, store `files.tier3` + `nodes.line` (`INDEX_FORMAT` 6), `SearchQuery.archive`, `SearchHit.{line, tier3}`, `SearchResults.tier3_left_out`, `SpecIndex::indexed_input`, the four commands. (2) A header longer than the cap cut, not dropped; `show`'s alias-checked fallback; the search cap; a distinct "none of its files could be read"; `kind` on one line; `init` removes a partial file; the canonical dump derives its columns from the schema. (3) Search JSON `truncated`; the first hit always shown; `pragma_table_xinfo(?1, 'main')` and `main.` qualification; paths on one line. (4, owner-approved) One-line stderr at the choke points; the dump's doc rewrapped; `null` for a field cut to nothing; name, kind, title cut shortest first.

| Module | What it does |
|---|---|
| cli `main.rs` | clap (no colour), one command, stdout and stderr, the exit code |
| cli `lib.rs` | `Exit`, `CliError`, `Env`, `Globals`, `Message`, `Outcome`, renderers, `one_line` |
| cli `project.rs`, `location.rs` | discovery, `--root`, `--config`; the data directory, the `HOME` rule, `open_index` |
| cli `init.rs`, `refresh.rs` | `init`, slug derivation; `index` and the freshness `update` |
| cli `search.rs`, `show.rs`, `cap.rs` | search and its cap; resolution, holder reads, node pick; the `show` cap |
| core `project_toml.rs` | `ProjectConfig`, `slug_problem`, `MAX_SLUG_BYTES` |
| core `check/{render,mod,resolve}.rs` | the public Tier 3 predicates; `resolve_detached` |
| store `schema.rs`, `rows.rs`, `write.rs` | `files.tier3`, `nodes.line`, `INDEX_FORMAT` 6 |
| store `read.rs`, `index.rs`, `lib.rs`, `dump.rs` | `archive` before `LIMIT`, `tier3_left_out`, `indexed_input`; schema-derived dump |
| fixtures `spec-{a,b}/specengine.toml` | `[project] slug` |

Accepted deviations: init output `created <path> with slug <slug>`, JSON `{path, slug}`; the cap on `search` as well as `show`; JSON snippets keep raw line breaks; exit 1's reason also on stderr; warnings stderr only, JSON `notes` holds notes; no stderr note when all terms are short; an extra public `ProjectConfig.project_line`; errors name the config as given; `HOME` required on every host. Known limit: a cut first hit's JSON `id` is a prefix of the ID, marked only by `truncated: true`. Open items: CLI README "Open". A stale `specengine-core` build artifact once failed a multi-package run spuriously (`cargo clean -p specengine-core`).
