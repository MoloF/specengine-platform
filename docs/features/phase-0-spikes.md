---
class: spec
status: in-progress
scope: [spikes]
ref: owner request "Phase 0: spikes" 2026-09-28
adrs: [ADR-0008, ADR-0012, ADR-0016, ADR-0020, ADR-0021, ADR-0022, ADR-0023, ADR-0024]
---

# Phase 0 spikes: turn the engine's unverified claims into numbers on the two pilot corpora

## Why

Spec 05 §5.1–5.2 and 04 §3–4 rest on research claims nobody here has executed: AST-hash traps 2–5 (05 §5.2), `bevy_dev_tools::schedule_data` absent from release notes (05 §5.1), the rust-analyzer cost estimate (04 §1.7), the flagged `tree-sitter-ron` 0.2.0 (04 §6, 08 §5), and the Claude Code behaviours the approval flow depends on — elicitation form, `_meta["anthropic/requiresUserInteraction"]`, output limits, 2-minute backgrounding, both protocol eras (04 §4). If any is wrong, Phases 1–3 build on it: a hash that flickers under `cargo fmt` teaches the owner to ignore drift, a form that never renders removes the single control point (ADR-0012), a layer needing 4 GB cannot run on the laptop.

A measurement campaign delivered as production code: six probe groups (08 §2 Phase 0 item 4) whose outputs — a number or a yes/no per pilot — confirm ADR-0020, ADR-0021 and the stack rows of 05 §9 / 04 §6, or trigger superseding decisions before Phase 1 builds on them. Done: every "Results" table filled for pilot A and pilot B; every claim in "Verdicts" confirmed, refuted or not measured; each refuted claim names its follow-up.

## Description and interactions

The pilots are two local Rust workspaces (Bevy 0.19, edition 2024) without a rustfmt config, a schedule dump or SpecEngine markers. They are named only "pilot A" and "pilot B"; their paths enter at run time via `SPECENGINE_PILOT_A` / `SPECENGINE_PILOT_B` or `--pilot`, are read-only, and the repository records aggregates only. Each measurement prints the fields of its "Results" table.

| Group (loop order) | How it runs | Feeds |
|---|---|---|
| 1 `ast-hash` | `specengine-eval ast-hash`: the 05 §5.2 walker of `specengine-code`; perturbations (a) default rustfmt, (b) contrasting rustfmt (`max_width`, `trailing_comma` flipped), (c) comment stripping incl. `///`; `syn` 3.0.6 + `extra-traits` behind feature `syn`; by-products: parse-error count, `qpath` ambiguity share | 05 §5.2, ADR-0021, baseline for AC-2 / AC-14 of 08 §3 |
| 2 `ron` | `specengine-eval ron`: `.ron` marker extraction of `specengine-code` by its own RON lexer; marker → field path on fixtures (the `tree-sitter-ron` 0.2.0 comparison path was measured in iteration 1 and removed in iteration 3 after the verdict) | 05 §9 "AST" row, 04 §6, 08 §5 "Outdated RON grammar" |
| 3 `import-census` | `specengine-eval census`: the dry-run counter of `specengine-import`; the corpus convention comes from `--config <toml>` (pilot copies stay outside the repository); fixtures use an invented one | 08 §4, AC-6 / AC-11 of 08 §3, Phase 1 migration order |
| 4 `mcp-stdio` | scripted: a raw JSON-RPC client in the test spawns `specengine-mcp` (`rmcp =3.5.0`; the server picks the era from the client's first message — `ClientLifecycleMode::Auto` exists only client-side, "Data") over stdio; interactive: the owner runs the checklist below (`claude -p` cannot answer a form; a headless denial is a different observation) | 07 §1.1–1.2, 04 §3–4, 08 §5 "The MCP protocol changes again" |
| 5 `bevy-schedule` | `specengine-eval bevy-detector` runs the syntactic detector of `specengine-code` (`add_systems`, `add_observer`, `impl Plugin for`, `fn(&mut App)`, macro bodies) against the dump of a scratch copy of one pilot outside the repository: a patch adds `SerializeSchedulesPlugin` (features `debug` + `schedule_data`), own target dir, headless run under a self-terminating timeout; the owner runs it only if a window is unavoidable; second pilot only if the match rate stays open | 05 §5.1 layers A/B/C, 08 §5 "Bevy 0.20", ADR-0020 |
| 6 `rust-analyzer-library` | `specengine-eval ra` (feature `ra` → `specengine-ra`, `ra_ap_*` `=0.0.352`), release profile, `MonikerResult::from_def`; proc-macro server: `rustup component add rust-analyzer` — installed by the owner | 05 §5.1 layer C, ADR-0020, AC-10 of 08 §3 |

Cheap, laptop-light groups first; cold builds last. One implement/verify loop per group, three iterations each (ADR-0023); the owner runs the interactive checks, the spec-writer transcribes.

**Placement** (owner decision: production code in the crates of 08 §1, reviewed against every product rule, never deleted).

| Crate | Phase 0 increment |
|---|---|
| `specengine-code` | normalized AST hash walker (05 §5.2 recipe, traps 1–5, `cannot_verify`); marker extraction for `.rs` and `.ron` (`.ron`: own lexer, decided by measurement); syntactic Bevy registration detector; trap tests over `fixtures/` |
| `specengine-mcp` | stdio server skeleton on `rmcp =3.5.0`: both protocol eras, `review_proposal` as the consent tool (`_meta["anthropic/requiresUserInteraction"]`, elicitation form in both eras, no queue behind it yet); probe tools behind feature `probes` |
| `specengine-import` | census / dry-run counter over an existing corpus; convention from a config file, nothing hard-coded (ADR-0008) |
| `specengine-ra` | layer C: `ra_ap_*` `=0.0.352` loading and monikers; outside `default-members`, so `ra_ap_*` never enters the core graph |
| `specengine-eval` | permanent measurement harness: one subcommand per measurement over pilot corpora, paths at run time, read-only, JSON aggregates; comparison-only dependencies (`syn` 3) only here behind a feature; re-run on grammar / Claude Code / Bevy upgrades and for AC-1 / AC-10 of 08 §3 |

`fixtures/bevy-mini` is its own small Cargo project, workspace-excluded, so Bevy never enters the workspace build. 08 §2 says the spikes come "before the first line of code": the measurements are the first production increments of the real crates; only heavy or comparison-only tooling stays out of the product graph (`syn` 3 behind a feature of `specengine-eval`, `ra_ap_*` in `specengine-ra` outside default members, Bevy in the excluded fixture).

**Rules that bind this code.** Pilots are read-only and get no markers (ADR-0016); the harness refuses to write under a pilot path; stdout is JSON aggregates, stderr a human summary; no path, ID or name of a pilot enters the repository; English only (ADR-0024); versions pinned as in 04 §6; long runs under a self-terminating timeout (CLAUDE.md "Owner's machine").

## Data

**Workspace.** Root `Cargo.toml`: `members = ["xtask", "crates/*"]`, `default-members = ["xtask", "crates/specengine-code", "crates/specengine-mcp", "crates/specengine-import", "crates/specengine-eval"]`, `exclude = ["fixtures/bevy-mini", "fixtures/ra-mini"]`. Features: `specengine-eval` — `default = []`, `syn = ["dep:syn"]`, `ra = ["dep:specengine-ra", "dep:libc"]` (32 `ra_ap_*` crates at `0.0.352`; needs rustc ≥ 1.98); `specengine-mcp` — `probes` (adds `probe_output`, `probe_sleep`). `tree-sitter` 0.27, `tree-sitter-rust` 0.24.2, `rmcp =3.5.0` (features `server`, `macros`, `transport-io`, `request-state`; no HTTP stack in the graph), `blake3` are normal dependencies.

**CLI contract.** `specengine-eval <ast-hash|ron|census|bevy-detector|ra> [--pilot <dir>] --out <scratch-dir> [--label <name>] [--config <toml>] [--dump <app_data.ron>] [--cargo-target-dir <dir>] [--proc-macros both|with|without] [--timeout <s>]` (`--dump` only for `bevy-detector`, alias `bevy`; `--cargo-target-dir` and `--proc-macros` only for `ra`). Without `--pilot` the measurement runs on its fixture. `--label` names `--out/<measurement>/<label>/` and must be one plain path component; `pilot-a` / `pilot-b` without `--pilot` read `SPECENGINE_PILOT_A` / `_B`; default `pilot` with `--pilot`, `fixtures` without. `--timeout` (default 600 s) bounds the measurement; for `ra` it is the budget of each load, and the run as a whole gets loads × budget + 60 s. `ra`: `--proc-macros` default `both` (`without` first, then `with`); `--cargo-target-dir` default `<out>/ra/<label>/target`. Exit 0 = measured; 2 = refused (`--out` under `--pilot`, a pilot path unreadable, a `--label` that is not one plain path component — empty, with a separator, `.`, `..`, absolute — or — `census` — a missing or invalid config, or — `bevy-detector` — an unreadable or unparsable `--dump`, named with its line, or — `ra` — no `Cargo.toml` at the corpus root, `--out` or `--cargo-target-dir` under the corpus (a fixture included), a `--timeout` the system clock cannot represent), nothing written; 1 = internal failure. A timed-out measurement is the JSON string `"timeout"` in its field, exit 0. `--out` receives per-file detail (hash manifest, per-ID lists) — never the repository.

**JSON envelope** (stdout, one object; `result` fields per measurement = the rows of its "Results" table):

```json
{"measurement": "ast-hash", "label": "fixtures", "versions": {"tree-sitter": "0.27.0", "tree-sitter-rust": "0.24.2", "abi": 15},
 "wall_ms": 412, "result": {
   "files": 6, "items": 19, "items_error": 3, "items_error_pct": 15.8, "files_with_errors": 2,
   "stable_pct": {"fmt_default": 100.0, "fmt_contrast": 100.0, "comments": 100.0},
   "files_changed": {"fmt_default": 2, "fmt_contrast": 5},
   "cannot_verify": 3, "cannot_verify_distinct": true, "error_categories": ["macro_punct", "where_multiline"],
   "qpath_ambiguous_pct": 5.3, "path_attrs": 1,
   "syn": {"files_failed": 2, "naive_stable_pct": 61.0, "normalized_stable_pct": 100.0, "time_ratio": 0.7}}}
```

`syn` is `null` without the feature; `ron` keeps its comparison shape after the grammar's removal: `grammar` `{built: false, loads: false, version/abi/second_runtime: null, note}` (`note` explains the removal), every per-approach field `{lexer, grammar: null}`, `agree_pct` / `files_agreeing_pct` `null`, `recommendation` always `"lexer"`; `ra`: below; `census` nests `with_front_matter: {total, per_class}` and `id_rows: {total, per_prefix}` (anonymised keys, below) plus `detail`; `bevy-detector` (shape: `fixtures/bevy-mini/expected.json`) prints `files`, `files_with_errors`, `detected {systems, observers, plugins}`, `detail` (call sites, forms, `plugins_by {impl_plugin, fn_app, in_macros}`, `plugin_uses`, adapters, `uncertain` per category) and `dump` — `null` without `--dump`, else `schema {expected, unknown_fields, missing_fields}`, `schedules`, `systems_total`, `apply_deferred`, `systems` (own crates), `observers` / `plugins` `null` (not in the schema), `matched`, `match_pct`, `name_found_pct`, `name_found_incl_macros_pct`, `misses`, `miss_categories`, `detected_unmatched`, `match_basis`. stdout holds counts only; names go to `--out/bevy/<label>/` (`registrations.json`, `plugins.json`, `plugin_uses.json`, `uncertain.json`, `crates.json`, `dump_match.json`, `dump_schema.json`). Dump fields outside the Bevy 0.19 schema are counted, not fatal. Pilot tests read the dumps from `SPECENGINE_PILOT_A_DUMP` / `_B_DUMP`.

**`ra` result** (group 6; one object per load, `null` for a load not requested; MiB and ms; numbers illustrative — the fixture's asserted values are in "Fixtures"):

```json
{"measurement": "ra", "label": "fixtures", "versions": {"tree-sitter": "0.27.0", "tree-sitter-rust": "0.24.2", "abi": 15}, "wall_ms": 21870,
 "result": {"ra_ap": "0.0.352", "profile": "release", "timeout_s": 600, "without": null,
   "with": {"status": "ok", "cold_ms": 9120, "peak_rss_mb": 412.3, "group_peak_rss_mb": 655.8, "group_peak_rss_floor_mb": 655.8,
     "warm_ms": 41, "moniker_pct": 89.7, "items": 29, "items_with_moniker": 26, "panics": 0,
     "detail": {"metadata": {"ms": 610, "toolchain": "1.98.1", "packages": 2, "metadata_degraded": false, "sysroot_packages": 21, "sysroot_error": false},
       "build_scripts": {"ms": 7300, "ran": true, "errors": false},
       "database": {"ms": 980, "proc_macro_server": "running", "proc_macro_crates_loaded": 1, "proc_macro_crates_not_loaded": 0, "peak_rss_mb": 390.1},
       "first_pass": {"ms": 230, "counts": {"files": 6, "files_loaded": 5, "files_in_crate": 4, "files_panicked": 0, "files_unreadable": 0, "items": 29,
         "by_status": {"moniker": 26, "not_loaded": 1, "unresolved": 2},
         "by_kind": {"function": {"items": 16, "moniker": 13, "duplicate": 0}, "struct": {"items": 13, "moniker": 13, "duplicate": 0}}, "duplicate_moniker_items": 0}},
       "warm_file_ms": 12, "children_peak_rss_mb": 243.5, "group_rss_sample_ms": 100, "target_dir_fresh": true,
       "no_warm_target": false, "load_error": null, "exit": null}}}}
```

- `ra_ap` = `RA_AP_VERSION`; `profile` `release` or `debug` (debug times are not the measurement); `timeout_s` = the per-load budget.
- `status`: `ok`; `timeout` (budget exceeded, the load's process group killed); `crashed` (the worker died on its own; `detail.exit` = `code N` / `signal N`); `load_failed` (a load step failed, category in `detail.load_error`). A numeric field the load did not reach holds that status string instead; `warm_ms` is `"no_warm_target"` when no file has an item with a moniker, `peak_rss_mb` `"unknown"` when a finished load could not read it.
- `cold_ms` = metadata + build scripts (`with` only) + database + first pass. `warm_ms` = an in-memory edit of one file (a function appended) + a full pass; `detail.warm_file_ms` = the pass over the edited file only.
- `peak_rss_mb`: the analysing (worker) process. `detail.children_peak_rss_mb`: the largest waited-for child. `group_peak_rss_mb`, **the verdict's measure**: the largest sum of the resident sizes of every live process of the load's process group (worker, cargo, rustc, build scripts, proc-macro server), sampled every `detail.group_rss_sample_ms` (100) from the worker's start to the load's end; shared pages count once per process; `"timeout"` on timeout; `null` without a group reading (only macOS and Linux have one) or without a sample — never 0. `group_peak_rss_floor_mb` = the largest of the sampled group peak (on timeout, until the kill), `peak_rss_mb` and `children_peak_rss_mb`: a guaranteed lower bound, `null` when none is known.
- `moniker_pct` = `items_with_moniker` / `items`, a non-local moniker from `MonikerResult::from_def`; item statuses `moniker`, `local`, `none`, `unresolved`, `not_loaded` (a file outside every crate); `panics` = files whose scan panicked.
- `detail.metadata.metadata_degraded`: metadata with dependencies failed (e.g. a stale lock under `--locked`) and the load fell back to `--no-deps` — no dependency crates, no build scripts. `build_scripts` is `null` in `without`, `ran: false` when skipped. `database.proc_macro_server` is `disabled`, `running` or `failed`. `target_dir_fresh`: the target directory held no build output before the load (absent, empty, or only cargo's bookkeeping files); `false` = `cold_ms` understates a cold build, `null` = unreadable.
- stdout carries counts and times only. Per load, `--out/ra/<label>/<mode>/` receives `items.json` (path, line, kind, name, status, moniker per item), `errors.json` (load-step error texts), `warm.json` (the edited file), and the worker's `tmp/`.

**Census config** (schema owned by `specengine-import` `config.rs`; `deny_unknown_fields` in every table; errors read `file:line: message`; without `--config` the harness reads `census.toml` at the corpus root; fixture copy `fixtures/corpus-mini/census.toml`, pilot copies outside the repository). Every key, with its default; `[corpus]` and `ids.regex` are required:

```toml
[corpus]
roots = ["."]              # corpus-relative; `..` or an absolute path is an error
extensions = ["md"]        # document extensions, without the dot
exclude = []               # globs over corpus-relative `/` paths: `*`, `**`, `?`
[front_matter]
class_key = "kind"         # no default; absent → every document `unclassified`
[ids]
regex = '^[A-Z]{2}-[0-9]{3}$'  # searched after look-alike normalization, first match wins;
                               # prefix = named group `prefix`, else the leading ASCII letters
[tables]
id_column = 0              # 0-based column holding the ID
id_header = '^ID$'         # no default; regex over the ID-column header cell picks record tables;
                           # absent → a table is a record table if any of its rows has an ID
headerless = false         # opt-in: `|` blocks without a GFM header row (any-row rule)
[sections]
id_attr = true             # `{#ID}` on headings
[links]
wiki = false               # opt-in: also check `[[target]]` links
wiki_root = "."            # only with wiki = true (else an error); where wiki targets resolve
```

- **ID classes** of the verbatim match: `latin` (only ASCII letters; digits and punctuation are neutral), `mixed-script` (ASCII letters plus at least one foreign letter or digit — AC-11 of 08 §3; look-alikes from a table of fullwidth ASCII and Cyrillic and Greek letters identical to a Latin one are normalized before matching), `non-latin` (foreign letters or digits, no ASCII letter: a legacy ID, an alias under ADR-0009). `id_rows.total` counts all three; `mixed_script_ids` and `non_latin_ids` count the last two; `per_prefix` uses the normalized prefix.
- **Records hashed** (BLAKE3; `records_hashed` = ID'd rows + `{#ID}` sections): a row = its line as written, without the terminator; a section = its heading up to the next heading of the same or higher level, trailing whitespace trimmed.
- **Links**: a Markdown destination (inline or reference definition) is resolved after CommonMark backslash unescaping, then percent-decoding — relative to the linking document, `/`-leading from the corpus root; scheme URLs and `#anchor`-only links are not checked; the reported target stays as written. A wiki target resolves when a file under `wiki_root` has it (as written or with a document extension appended, case ignored) as its relative path or a `/`-bounded tail of it, or when it exists relative to the linking document.
- **Output**: stdout is anonymous — classes and prefixes are `class-N` / `prefix-N`, numbered by descending count, plus `unclassified`; the mapping lives only in `--out/census/<label>/labels.json`, beside `records.json` (hash manifest), `documents.json`, `rows_without_id.json`, `broken_links.json`, `diagnostics.json`. `detail`: `files_skipped`, `front_matter_unclosed`, `roots_missing`, `bytes`, `tables`, `headerless_blocks`, `headerless_id_rows`, `record_tables`, `links_checked`, `other_anchors`, `duplicate_ids`, `diagnostics`, `census_ms`.

**MCP server** (`specengine-mcp`, group 4; rmcp 3.5.0 facts that correct the initial plan). Binary `specengine-mcp [--lifecycle auto|legacy]`, stdio only: stdout carries JSON-RPC and nothing else. rmcp has no server-side lifecycle mode (`ClientLifecycleMode::Auto` is client-side): the server detects the era from the client's first message — `initialize` → legacy session; a request with complete 2026-07-28 `_meta` → stateless. The only server knob is `supported_protocol_versions`: `auto` (default) = known versions up to 2026-07-28; `legacy` = up to the latest version with `initialize` (2025-11-25), a stateless request then gets -32022 `UNSUPPORTED_PROTOCOL_VERSION`. A stateless request carries `io.modelcontextprotocol/protocolVersion` and `io.modelcontextprotocol/clientCapabilities` in `_meta` (either missing → -32602; `…/clientInfo` optional); `server/discover` is answered, `tools/list` carries `ttlMs: 0`, `cacheScope: "public"`. `instructions`: 1 339 bytes ASCII (plus a probe paragraph under `probes`), compile-time assert ≤ 2 048 (04 §4 truncation).

- `review_proposal {proposal_id}` — `proposal_id` matches `[A-Za-z][A-Za-z0-9._-]{0,63}`. Tool `_meta {"anthropic/requiresUserInteraction": true}`, `outputSchema` = the `structuredContent` below, annotations `readOnlyHint: false`, `destructiveHint: false`, `idempotentHint: false`, `openWorldHint: false`. Form (`requestedSchema`): `decision` enum `approve|reject`, required; `comment` string, optional. Legacy → server request `elicitation/create`, the call waits. Stateless → round 1 returns `resultType: "input_required"`, the form under `inputRequests.owner_review` and a `requestState` (`rs1.` prefix, HMAC-sealed with a per-process random key, associated data = tool name + proposal ID); round 2 repeats the call with that `requestState` and `inputResponses.owner_review`. Result: prose text + `structuredContent`. Demo only: no queue behind it, nothing recorded or written.
- Behind feature `probes` (never default; both `readOnlyHint: true`): `probe_output {tokens}` → `tokens` × 4 bytes of filler (max 200 000 tokens); `probe_sleep {seconds}` (max 900), ends early on the client's cancellation.
- `fixtures/mcp/mcp.json` (the owner checklist's launcher): `{"mcpServers": {"specengine": {"type": "stdio", "command": "cargo", "args": ["run", "-q", "-p", "specengine-mcp", "--features", "probes"]}}}`.

```json
{"resultType": "input_required", "requestState": "rs1.…",
 "inputRequests": {"owner_review": {"method": "elicitation/create", "params": {"message": "SpecEngine asks you to review proposal P-0001. …",
   "requestedSchema": {"type": "object", "properties": {"decision": {"type": "string", "enum": ["approve", "reject"]}, "comment": {"type": "string"}}, "required": ["decision"]}}}}}
{"resultType": "complete", "isError": false, "content": [{"type": "text", "text": "review_proposal P-0001: approved by the owner.\n…"}],
 "structuredContent": {"proposal_id": "P-0001", "action": "accept", "decision": "approve", "comment": "ok", "era": "stateless", "protocol_version": "2026-07-28"}}
```

`action` is `accept|decline|cancel`; `decision` and `comment` are `null` unless accepted (a blank comment → `null`); `era` is `legacy|stateless`.

**Fixtures** (each with `expected.json` beside it, except `ra-mini`, whose values `ra_cli.rs` asserts; tests in the owning crate's `tests/`).

| Path | Content | Expected |
|---|---|---|
| `fixtures/ast-hash/` | trailing-comma variants, `///` and block comments, one item with a parse error, two different broken items, a `#[path]` module | `stable_pct` all 100.0, `files_changed.*` > 0, `cannot_verify` 3, `cannot_verify_distinct` true, `path_attrs` 1 |
| `fixtures/ron/` | `config.ron`: `// @implements X@1` inside a nested struct and inside a list, one before a closer; `extensions.ron`: `#![enable(..)]`, raw string `r#"…"#` | `root.player.speed`, `root.waves[2]`, `unanchored`; parse-clean 2 / 2 |
| `fixtures/bevy-mini/` | standalone Cargo project, workspace-excluded, parsed only: `macro_rules!`-registered system `tick`, `fn setup_plugin(&mut App)`, observer `on_spawn`, `impl Plugin for MiniPlugin`; `app_data.ron` — a hand-written Bevy 0.19 `schedule_data` dump of it | `detected` `{systems: 1, observers: 1, plugins: 2}`, `plugin_uses` 3, names as listed; with `--dump`: 1 own system, 0 matched, 1 `macro_rules` miss, schema drift 0 / 0 |
| `fixtures/corpus-mini/` | invented convention (prefixes `ZR`, `ZN`): 3 front-matter docs (`kind: rule` ×2, `note`), one table with 5 rows — 3 clean IDs, one empty, one mixed-script — one `{#ZR-004}` section, one link to a missing file | `documents` 3, `with_front_matter` `{3, {class-1: 2, class-2: 1}}`, `id_rows` `{4, {prefix-1: 3, prefix-2: 1}}`, `rows_without_id` 1, `id_sections` 1, `mixed_script_ids` 1, `non_latin_ids` 0, `broken_links` 1, `records_hashed` 5 |
| `fixtures/ra-mini/` | own `[workspace]` (`arena`, `arena-derive`) and `Cargo.lock`, so the root `exclude` keeps it out of the workspace build; build script `arena/build.rs`; path proc-macro crate `arena-derive` whose derive `arena` uses; a `#[cfg]`-gated function; `arena/src/stray.rs` outside the module tree; `tools/probe.rs` outside every package | both loads `ok`: `items` 29, `items_with_moniker` 26 (89.7 %), 2 `unresolved`, `files_loaded` 5, `files_in_crate` 4, `tools/probe.rs` `not_loaded`; `with`: server `running`, ≥ 1 proc-macro crate loaded, build scripts ran without errors; `without`: server `disabled`, 0 loaded, `build_scripts` `null`; `metadata_degraded` false, `target_dir_fresh` true; the fixture stays byte-identical, no `target/` |

## Rules and edge cases

- WHEN `--out` resolves under `--pilot` the harness SHALL exit 2 before opening any file for writing.
- WHEN `--label` is not exactly one plain path component (empty, a separator, `.`, `..`, absolute) every measurement SHALL exit 2 before writing, so `--out/<measurement>/<label>/` cannot escape `--out` or reach a pilot.
- WHEN an item has `has_error()` the walker SHALL report `cannot_verify` and SHALL NOT emit a hash (trap 1 of 05 §5.2: comments filtered by `kind()`, never `is_extra()`).
- WHEN an error-free item's `use` runs nest past `MAX_USE_RUN_NESTING = 64` runs the walker SHALL report `cannot_verify` with category `nesting_too_deep` (the second cause besides parse errors) and SHALL NOT emit a hash; `hash::normalize` returns `false` for such an incomplete stream.
- WHEN formatting a pilot the harness SHALL use `rustfmt --edition 2024 --emit stdout` or a scratch copy, never in place; a pilot may already be rustfmt-clean, hence two configs and `files_changed > 0`.
- WHEN a pinned set does not resolve or build (`ra_ap_*` skew, `rmcp` 3.5.0 brand new) the run SHALL record "does not build": a valid result, nothing blocks (ADR-0012).
- WHEN a measurement exceeds its timeout the value SHALL be `"timeout"`; timeouts use the self-terminating form only.
- WHEN the Bevy instrumented build runs it SHALL use a scratch copy with its own target directory outside the repository (the "one build directory" rule concerns this repository); a failed build records its category and whether `bevy_mod_debugdump` was used.
- Interactive checks record the installed Claude Code version and are re-run on upgrade.
- Time-box one week; an overrun records "not measured" and its 08 §5 risk row stays.
- Nothing corpus-specific enters `specengine-import` (ADR-0008); the fixture corpus goes through the same code path as the pilots.

**MCP server edges** (group 4; the rmcp 3.5.0 behaviours are recorded, not overridden).

- WHEN the first message is neither `initialize`, `ping` nor a request with complete 2026-07-28 `_meta` the session SHALL end: rmcp answers it with an error, the process prints one stderr line and exits 1. Empty stdin (closed before any message) → exit 0, nothing printed.
- rmcp skips non-JSON lines silently (no -32700); after stdin EOF it lets in-flight handlers run for up to 5 s, then drops them unanswered.
- WHEN `proposal_id` breaks the ID rule `review_proposal` SHALL return a tool error (`isError: true`) before any form; a non-Latin or mixed-script ID is refused naming the code point (e.g. `U+0420`, ADR-0009) — no autofix yet (AC-11 of 08 §3 is Phase 1+).
- WHEN the client declares no form elicitation `review_proposal` SHALL return a tool error saying the owner cannot be asked; nothing is recorded.
- WHEN a stateless round 2 carries a tampered `requestState`, one sealed for another proposal ID, or no `inputResponses.owner_review` the server SHALL answer -32602.
- WHEN the client cancels a call (`notifications/cancelled`) the server SHALL send no response to it; a legacy form still open is not withdrawn (open minor, "Implementation").

**RON markers** (group 2; `specengine-code`, own lexer).

- Marker grammar (`markers.rs`, shared by `.rs` and `.ron` comments): `@implements|@verifies|@configures|@assumes ID[@rev] [note]`, several per comment; an ID outside the Latin script clears `id_latin` and is counted, never fatal (ADR-0009).
- Path: `root` = the file's value; struct field `.name`; list element `[i]`; tuple element `.i` (tuple structs such as `Some(x)` count as tuples); map entry `{key}` (key text verbatim, whitespace collapsed). Each segment is capped at `MAX_SEGMENT_BYTES = 128` source bytes, cut at a character boundary, plus `…`. The 05 §5.3 example `data/movement.ron#stamina.regen_per_second` lacks the `root.` prefix — reconciled on shipping.
- Adjacency (owner decisions 2026-09-28, table below; the full rule list with examples is the module doc of `ron/structure.rs`). A *value* is an entry, a list or tuple element, or the root value. Only positions count, not the comment kind — except that a `//` comment never leads. A comment's markers bind by the first rule that applies:
  1. **Leading.** A block comment followed, on the line of its `*/`, by the start of a value (only whitespace and comments between) binds to that value, ahead of rules 2–3: `pos: (/* @A */ 10, /* @B */ 20)` → `root.pos.0`, `root.pos.1`; `a: 1, /* m */ b: 2` → `root.b`; `speed: /* m */ 4.5` → `root.speed`, also with `speed:` alone on the line above. A value starts at a field name or map key (binds to the entry), an element, the root value, or an entry's value after `:`; only identifiers, openers, strings, chars and numbers start one — a closer, `,`, `:` or an extension attribute never does.
  2. **Trailing a value.** A comment that starts on the line of a value's last token binds to that value; the `,` may sit on either side of the comment; of several values closing on one line, the one whose last token comes right before the comment: `speed: 1, // m` → `root.speed`; `waves: [1, 2, 3], // m` → `root.waves`; `[1, 2, 3 /* m */]` → `…[2]`; `Config(..) // m` → `root`. The last entry before a closer is no exception.
  3. **Trailing an opener.** A comment right after `(`, `[` or `{` on its line binds to the container's value: `player: Player( // m` → `root.player`, `Config( // m` → `root`.
  4. **Own line.** A comment with no token before it on its line binds to the entry, element or root value beginning at the next token; extension attributes before the root value are transparent (`#![enable(..)] // m` → `root`).
  5. **Otherwise `unanchored`:** own-line before a closer or `,`; after a map key; after a `:` or inside a value already begun (`speed:`, then `// m`, then `4.5` on separate lines; a block comment whose value starts on a later line than its `*/`); after a `,` on a line where no value ended; between a type name and its `(` (`Player /* m */ (`); after tokens error recovery consumed; after trailing content.
- A multi-line block comment trails by the line of its `/*` and leads by the line of its `*/`; LF and CRLF behave alike; an unclosed container at end of input has no end, so nothing trails it. In a broken file, when error recovery consumes the token a block comment leads, the comment falls back to the value it trails (`a: 1 /* m */ 2` → `root.a`).
- On shipping, 05 §5.3 takes this rule and the `root.` prefix, and records that `.rs` markers follow the same principles when Rust marker binding lands (Phase 3).
- WHEN more than `MAX_DEPTH = 512` containers are open the analyser SHALL record `nesting_too_deep`, skip the container through its closer and report the markers inside it as `cannot_verify`; a comment trailing the skipped container's closer binds to its value (rule 2).
- The walk SHALL stay linear in tokens: a key's end is searched at most 128 bytes ahead, a path is rendered only while a marker is pending, line numbers come from one newline index per file.

**Bevy detector** (group 5; `specengine-code` `bevy`, full list in its module docs; comparison in `specengine-eval` `bevy/compare.rs`).

- Read: systems of `.add_systems(Schedule, …)` and of the one-argument `Schedule::add_systems(…)` (schedule unknown) — tuples nest, `COMBINATORS` are peeled, `ADAPTERS` (`pipe`, `map`, `with_input`, `with_input_from`) recorded; a leaf is a path (`tick`, `a::b`, `f::<T>`), a closure (named by its innermost enclosing `fn`) or a factory call (named by its callee); observers of `.add_observer(…)` read like a leaf; plugins = every `impl Plugin for X` and every function whose only parameter is `&mut App`, qualified or generic, `-> ()` allowed (not `&App`, `App`, `&mut SubApp`, `&mut World`, a `self` method, a second parameter or `-> &mut App`); plugin uses of `.add_plugins(…)`.
- The same calls inside `macro_rules!` transcribers and macro invocation arguments are read from tokens: origin `macro_rules` / `macro_call`, names only, never resolved and never matched against a dump.
- WHEN a construct cannot be read the detector SHALL record `uncertain` with a category (`macro_in_arguments`, `metavariable`, `unknown_method`, `expression`, `arguments`, `nesting_too_deep`, `parse_error`), never a guessed name; tuple nesting past `MAX_NESTING = 256` → `nesting_too_deep`; a parse error stays in its own call of a builder chain; texts are capped at `MAX_TEXT_BYTES = 128` plus `…` at a character boundary; cost linear, no recursion over input.
- Comparison with `--dump`: the corpus's own crates are its `[package]`, `[lib]`, `[[bin]]` names (`-` as `_`); a dumped system is keyed by schedule (`Debug` text) and terminal name (generic arguments dropped; `f::{{closure}}` → `f`; `Pipe(a, b)` → `a`), a detected ordinary-code registration by its label with path qualifiers dropped and its name. One-to-one: same schedule and name, then same name where the detected schedule is not a literal; every other dumped system is one miss, first category that applies: `generic_instance` / `repeated_site`, `other_schedule`, `macro_rules` / `macro_call`, `closure`, `indirect`, `not_in_source`. Test, example and bench candidates are taken after the app's own code.
- **Token-reader limits, deliberate and pinned by tests** (macro bodies and macro arguments only; the syntax-tree reader of ordinary code is exact): a comma inside a type position or closure parameters splits an element — `|q: Query<A, B>| …` as a whole system argument gives `arguments` uncertain instead of a closure registration, and with ≥ 3 generic items a middle path fragment can be registered as a name (open minor); `a::<` unclosed at an element's end is read as the path `a` (non-compiling input only); an element with tokens after its path / postfix chain → `expression`; `sys.$m(c)` and `systems::$name` → `expression` (open minor: should be `metavariable`); `::<<` opens a turbofish.

**rust-analyzer loads** (group 6; `specengine-ra` loads, `specengine-eval` `ra` supervises).

- `without` = no proc-macro server **and** no build scripts; `with` = build scripts and proc-macro dylibs built (`cargo check --compile-time-deps`), then the server. Both are the corpus's own code with its permissions, so **a pilot's `with` load runs only on a scratch copy**: a pilot build script writes into its own source tree when a file is missing (observed). The harness refuses `--out` / `--cargo-target-dir` under the corpus, but cannot stop a build script writing into its tree.
- Every cargo call gets the target directory (`CARGO_TARGET_DIR` and `--target-dir`), `--locked` and `--offline`; an existing `Cargo.lock` is copied to a temporary directory and resolved against the copy; `TMPDIR` points under `--out`; toolchain overrides (`RUSTUP_TOOLCHAIN`, `RUSTC`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`, `CARGO_TARGET_DIR`, `CARGO_BUILD_TARGET_DIR`, …) are removed from the worker's environment. A lock that does not resolve under `--locked` → `metadata_degraded`, a valid result.
- Each load runs in its own worker process (hidden `ra-worker` subcommand) leading its own process group, under the per-load budget. WHEN the budget runs out the harness SHALL kill the whole group (worker, cargo, rustc, build scripts, proc-macro server), report `status: "timeout"` and `"timeout"` in every field not yet reported, and leave no process behind. A worker outliving its last report by 10 s is killed; its reported values stand (`detail.exit`). The `with` load shuts the proc-macro server down (`Workspace::close`) before its report.
- **RSS verdict rule**: compare `max(group_peak_rss_mb, group_peak_rss_floor_mb)` with 4096 MiB — 100 ms sampling can miss the worker's short spike, the floor is a guaranteed lower bound. The group is read with `proc_pidinfo` on macOS and `/proc/<pid>/stat` on Linux; elsewhere `null`.

**Verdict thresholds.** tree-sitter recipe confirmed = 100 % stability on error-free items under all three perturbations and no shared `cannot_verify` hash; `syn` wins only with 100 % normalized stability and zero whole-file failures on both pilots while the tree-sitter recipe misses its threshold — the time ratio does not decide (05 §5.2); RON `grammar` = builds against 0.27, comment byte ranges present, nested marker resolves, parse-clean ≥ 95 % on both pilots, else `lexer`; layer C not needed for registrations = detector ≥ 95 % of dumped own-crate systems, counted per system (not per registration site), with every miss in a named category; rust-analyzer affordable = cold ≤ 3 min and whole-group peak ≤ 4 GiB (4096 MiB) on the larger pilot (both were measured), with the proc-macro server running — the group being the analysing process, cargo and build scripts, and the proc-macro server; measure `max(group_peak_rss_mb, group_peak_rss_floor_mb)` ("rust-analyzer loads"); both-era elicitation confirmed = scripted (c) passes and the form rows of the checklist are yes.

**Questions, answered by the owner** (2026-09-28; rows marked 2026-09-29 answered then):

| Question | Answer |
|---|---|
| Throwaway spike crate? | Rejected: production increments in the real crates plus the permanent `specengine-eval` harness |
| Loops | one per group, three iterations each |
| Interactive Claude Code checks | the owner, from the checklist |
| Instrumented Bevy build | one pilot, scratch copy outside the repository |
| RON grammar vs lexer | stack row in 05 §9 / 04 §6; ADR only if `.ron` markers are dropped |
| Thresholds | 95 % / (cold ≤ 3 min, peak RSS ≤ 4 GiB of the whole process group — the 2026-09-29 row below) |
| Hash manifest | counts in the spec; manifest to a scratch path |
| rust-analyzer component | installed |
| Trailing same-line RON marker (`speed: 1, // @configures X`): own value or next? | its own (rule 2): the common RON style; the literal next-entry reading of 05 §5.3 silently binds the wrong field. `,` on either side of the comment; of several values closing on one line, the one right before the comment; `Config(..) // m` → `root`. This and the four rows below are spec-level rules, no ADR; `.ron` markers stay |
| Same-line comment right after an opener (`player: Player( // m`)? | the container's value, `root.player` (rule 3) — accepted as implemented |
| Block comment before a value on its `*/` line (`pos: (/* @A */ 10, /* @B */ 20)`)? | binds to that value, ahead of rules 2–3 (rule 1): `root.pos.0`, `root.pos.1`; `a: 1, /* m */ b: 2` → `root.b`; `speed: /* m */ 4.5` → `root.speed`; `//` never leads, extension attributes never start a value |
| Broken file: recovery consumes the token a block comment leads (`a: 1 /* m */ 2`)? | falls back to the value it trails, `root.a` |
| Own-line comment after `:` (`speed:`, then `/* m */ 4.5`)? | a block comment binds to the entry when the value starts on the line of its `*/` (`root.speed`); its `//` twin and a block comment whose value starts on a later line stay `unanchored` |
| Layer-C threshold (95 %): per dumped system or per registration site? | per system (pilot B: 93.4 % per system, 100 % per site) |
| New pinned dependencies of groups 1, 3, 4, 6 (2026-09-29)? | approved, all exact pins — list below; the earlier open item is resolved |
| Toolchain for `ra_ap_*` 0.0.352 (2026-09-29)? | it requires rustc ≥ 1.98; the owner updated stable to 1.98.1, no `rust-toolchain.toml`. The ≥ 1.98 floor of `specengine-ra` enters 04 §6 on shipping |
| rust-analyzer "≤ 4 GB" (2026-09-29): which unit, which processes? | ≤ 4 GiB (4096 MiB), peak of the whole process group (analysing process + cargo / build scripts + proc-macro server), with the proc-macro server running — see "Verdict thresholds" |

**Dependencies, approved by the owner** (2026-09-29; all exact pins; none is in 04 §6 yet, all enter it on shipping): group 1: `serde =1.0.229` (derive), `serde_json =1.0.151` (the eval JSON envelope) and `quote =1.0.47`, `proc-macro2 =1.0.107` (only behind `specengine-eval`'s `syn` feature); group 3: `toml =1.1.4` (pulls `toml_parser`, `toml_datetime`, `serde_spanned`, `winnow`) and `regex =1.13.1` (already in the graph via tree-sitter); group 4: `tokio =1.53.1` (features `rt`, `macros`, `io-std`, `time`) and `getrandom =0.4.3` (the `requestState` key), both already in the graph through `rmcp`, which also brings `hmac`, `sha2`, `base64`, `zeroize`, `uuid`, `chrono` (via rmcp's `schemars` feature), `futures`, `tokio-util`, `tracing`, `thiserror`, `indexmap`; group 6: the nine direct `ra_ap_*` `=0.0.352` crates (`load-cargo`, `project_model`, `ide`, `ide_db`, `hir_expand`, `vfs`, `paths`, `syntax`, `proc_macro_api`) and the resulting lock additions; `salsa`, `salsa-macros`, `salsa-macro-rules` `=0.28.2` and `unicode-ident =1.0.24`, held as direct exact dependencies of `specengine-ra` because newer versions break the `ra_ap` 0.0.352 build (salsa 0.28.5 changed `HashEqLike`; unicode-ident 1.0.26 is Unicode 18 against `unicode-properties` 0.1.4's Unicode 17); `libc =0.2.189`, replacing hand-written FFI in the `ra` measurement.

## Acceptance criteria

- [x] AC-01 Build graph: `cargo metadata` default members == `xtask`, `specengine-code`, `specengine-mcp`, `specengine-import`, `specengine-eval`; `cargo tree -e normal,no-proc-macro -p specengine-code -p specengine-mcp -p specengine-import` (default features) lists no `ra_ap_*`, `syn` 3 or `bevy*`; `cargo xtask docs check` compiles only `xtask`; `cargo build -p specengine-eval --features ra` pulls `ra_ap_*` `=0.0.352` via `specengine-ra`. Regression: a test in `specengine-eval` asserts the first two facts from `cargo metadata`; adding `ra_ap_ide` to `specengine-code` or `specengine-ra` to `default-members` turns it red. *Groups 1 and 6: `build_graph.rs` asserts every fact — default members; core, default-`specengine-eval` and default-members graphs free of `ra_ap_*`; `--features ra` pulls 32 `ra_ap_*` crates, all `0.0.352`, only through `specengine-ra`; the salsa / unicode-ident pins; `libc` only with `ra`; `ra-mini` outside the workspace. Mutations, each red: `specengine-ra` in `default-members`; `ra_ap_ide` in `specengine-code`.*
- [x] AC-02 Read-only guard: every measurement exits 2 and writes nothing when `--out` lies under `--pilot`; after a full run over `fixtures/`, `git status --porcelain -- fixtures/` is empty. Mutation: remove the guard → the exit-code test goes red.
- [x] AC-03 Hash stability: on `fixtures/ast-hash` `specengine-eval ast-hash` reports 100 % stable hashes for error-free items under default fmt, contrasting fmt and comment stripping, and `files_changed > 0`. Mutations: drop the anonymous-`,` skip → contrasting-fmt stability < 100 % (red); replace the `kind()` comment filter with `is_extra()` → the two broken items share a hash and `cannot_verify` drops to 0 (red).
- [x] AC-04 `cannot_verify`: items with `has_error()` are reported as `cannot_verify` and never hashed; two different broken items never share a hash (AC-14 of 08 §3). Same mutation as AC-03.
- [x] AC-05 `syn` comparison: the fixture corpus through `syn` 3 prints whole-file failures (n), naive stability (%), normalized stability (%), time ratio; the spec records the verdict on ADR-0021.
- [x] AC-06 Pilot runs: `specengine-eval ast-hash --pilot $SPECENGINE_PILOT_A` and `_B` finish under the timeout and print every JSON field; both Results columns filled with numbers; items ≥ 1 000 per pilot (sanity floor).
- [x] AC-07 RON: on fixtures a `// @implements X@1` comment inside a nested struct/list resolves to a field path string and a comment adjacent to no field resolves to `unanchored`; every case of the `ADJACENCY` table (53 cases, each under LF and CRLF) yields exactly the anchors of the adjacency rules 1–5; on the pilots the parse-clean share and rejected-construct categories are printed; the spec records `grammar` or `lexer`. Mutations, each red: drop the own-line check → an own-line comment before a closer is no longer `unanchored`; drop the trailing claim after `,` → `speed: 1, // m` is no longer `root.speed`; drop the opener claim → `player: Player( // m` is no longer `root.player`; drop the leading branch → `pos: (/* @A */ 10, /* @B */ 20)` no longer gives `root.pos.0`, `root.pos.1`; drop the fallback → `a: 1 /* m */ 2` is no longer `root.a`. *Group 2: `lexer`; fixture paths in `ron_markers.rs` / `ron_cli.rs`, adjacency in `ron_markers.rs` (`ADJACENCY`), pilot categories in `ron_cli.rs` (`pilot_a|b_…`).*
- [x] AC-08 MCP scripted: an integration test spawns `specengine-mcp --features probes` over stdio and passes (a) legacy `initialize` + `tools/list` + `tools/call`, (b) a stateless 2026-07-28 request, (c) elicitation round trips in both eras, (d) `_meta["anthropic/requiresUserInteraction"]` present on `review_proposal` in `tools/list`, (e) a ≥ 26 k-token output without server error. Mutation: force the legacy-only lifecycle mode → (b) fails (red). *Group 4: `mcp_stdio.rs`, tests named `a_…` – `e_…` after the checks; the mutation is the `--lifecycle legacy` flag, and `b_legacy_lifecycle_refuses_stateless_requests` shows (b) refused with -32022; cancellation is covered by a discriminating test (a short `probe_sleep` stays unanswered after it would have ended).*
- [x] AC-09 MCP manual: the spec contains the checklist and its table; every row holds yes/no or the observed text plus the Claude Code version; "not measured" rows only with a matching line in 08 §5. *Run 2026-09-28 on Claude Code 2.1.283 (installed version): every row observed except (8) HTTP 405, "not measured" with 08 §5 "The MCP protocol changes again".*
- [x] AC-10 Bevy: for at least one pilot the dump has N > 0 systems and the detector count, match % and miss categories are printed; on `fixtures/bevy-mini` the detector finds exactly the four hand-listed registrations. Mutation: remove the `fn(&mut App)` rule → the fixture count drops (red). A failed instrumented build records the failure category and the fallback used. *Group 5: both pilots dumped and compared; the fixture run equals `expected.json` (`bevy_cli.rs`); the mutation `if false && is_app_function(…)` turns 5 tests red (fixture `plugins` 2 → 1); both instrumented builds succeeded, so the failure-record path was not exercised and the `bevy_mod_debugdump` fallback was not needed. Tests: `specengine-code/tests/bevy_detector.rs` (35), `specengine-eval/tests/bevy_cli.rs` (19 + 2 ignored pilot tests).*
- [x] AC-11 rust-analyzer: `specengine-eval ra` prints cold time, peak RSS, warm time and moniker share for at least one pilot, with and without the proc-macro server, under a self-terminating timeout; an overrun records `timeout` as the value. *Group 6: both pilots, both loads ("Results"). `ra_cli.rs` (12 tests): refusals write nothing (no `Cargo.toml`; `--out` / `--cargo-target-dir` under the corpus in every spelling; unrepresentable `--timeout`); the fixture run matches "Fixtures", is read-only and repeatable with anonymous stdout; a 1-s budget gives `"timeout"` fields and leaves no process; an overrun kills a running build script and the proc-macro server; the server is reaped before the `with` report; target freshness is decided by the directory's contents. `label_cli.rs`: every subcommand refuses a non-component `--label`.*
- [x] AC-12 Census: on `fixtures/corpus-mini` the counts equal `expected.json` exactly. Mutation: remove the mixed-script check → `mixed_script_ids` 0 (red). On the pilots only counts appear in the spec. *Group 3: `census_cli.rs` (fixture == `expected.json`, fixture untouched, determinism, pilot runs), `specengine-import/tests/census.rs` (config errors, scripts, tables, links, sections, cost bounds).*
- [x] AC-13 Anonymity and language: `cargo xtask docs check` is green; a grep of `docs/`, `crates/`, `fixtures/` for absolute paths and pilot names returns nothing (`anonymity.rs`); census stdout carries only `class-N` / `prefix-N` / `unclassified` keys and no value of `labels.json`, which lives only under `--out` (`census_cli.rs`); all text is English (ADR-0024). *Groups 1–3. The literal check "grep for the ID prefixes named in the runtime config" does not discriminate — the pilot prefixes are 1–4-letter generic tokens, one of them used by this repository itself — and is replaced by the stdout whitelist test.*
- [x] AC-14 Verdict closure: each claim in "Verdicts" ends as "confirmed", "refuted → ADR/spec section" or "not measured → 08 §5 row"; no claim is left without one of the three. *All 12 rows closed: 9 confirmed (the hash-walk row after amendment, its refuted v1 formula → 05 §5.2), 3 refuted → a spec section (`qpath` → 05 §5.1, RON grammar → 05 §9 / 04 §6, output limits → 04 §4 / 07 §1.1); none "not measured" (checklist item (8) stays with 08 §5).*

## Results

Filled after the runs; per-file detail stays in the scratch directory.

**`ast-hash`**

| Field | fixtures | pilot A | pilot B |
|---|---|---|---|
| tree-sitter build and run (y/n, ABI) | yes, ABI 15 | yes, ABI 15 | yes, ABI 15 |
| files / items / items with errors (n, %) / files with errors | 8 / 69 / 3 (4.3 %) / 2 | 195 / 11 852 / 0 (0 %) / 0 | 190 / 7 599 / 0 (0 %) / 0 |
| stable %: default fmt / contrasting fmt / comment stripping | 100.0 / 100.0 / 100.0 | 100.0 / 100.0 / 100.0 | 100.0 / 100.0 / 100.0 |
| files changed: default fmt / contrasting fmt | ≥ 3 / ≥ 2 | 136 / 192 | 0 / 187 |
| `cannot_verify` (n), distinct hashes (y/n) | 3, yes | 0, yes | 0, yes |
| error categories | macro_punct, other | — | — |
| `qpath` ambiguous (%), `#[path]` (n) | — (path_attribute 2), 1 | 32.2 (duplicate 3 714, path_attribute 98), 12 | 16.5 (duplicate 1 257), 0 |
| `syn`: files failed / naive % / normalized % / time ratio | 1 / — / — / — | 0 / 52.9 / 94.5 / ≈ 0.53–0.56 | 0 / 66.5 / 94.9 / ≈ 0.53–0.56 |
| wall time (s) | — | ≈ 13 (release) | ≈ 11 (release) |

Recipe `specengine-hash/v2`, tree-sitter 0.27.0 / tree-sitter-rust 0.24.2 / ABI 15, rustfmt 1.9.0-stable. Stable rustfmt rejects the unstable `trailing_comma` option, so the contrast config is effectively `max_width = 60`; it still rewrites 192 / 187 files. Pilot B is already rustfmt-clean under the default config (0 files changed). `syn` normalized stability under `fmt_contrast` alone: 84.3 / 84.6 %. **v1 baseline** (iteration 1, the 05 §5.2 formula as written): A 99.2 / 84.9 / 100.0, B 100.0 / 85.4 / 100.0 — rustfmt adds or removes `{ }` around single-expression closure bodies and match-arm values, adds `;` after diverging tails (edition 2024), moves braces inside expression-macro arguments and reorders `use`; v2 normalises exactly these four. `—` = not recorded in the spec (per-file detail stays in the scratch manifest).

**`ron`**

| Field | fixtures | pilot A | pilot B |
|---|---|---|---|
| grammar builds against 0.27 (y/n, ABI) | no as a crate API; loads only via its raw C symbol (ABI 14) with a second runtime (iteration 1) | same | same |
| RON files / with comments | 2 / 2 | 748 / 746 (1 050 960 B, 6 400 comments) | 226 / 175 (886 282 B, 2 108 comments) |
| parse-clean, lexer (%) | 100 | 97.5 (729) | 100 (226) |
| parse-clean, grammar (%), iteration 1 | 50 | 97.5 (same 19 files) | 100 |
| rejected categories, lexer (files) | — | `empty_file` 2, `expected_separator` 15, `expected_value` 1, `stray_token` 2, `unbalanced_delimiter` 4 | — |
| comment byte ranges (y/n) | yes | yes (grammar agreed 100 %) | yes (grammar agreed 100 %) |
| nested marker → field path (y/n) | yes: `root.player.speed`, `root.waves[2]`, `unanchored` before a closer | – (no markers, ADR-0016) | – |
| time, lexer | — | ≈ 20 ms | ≈ 20 ms |
| recommendation | lexer | lexer | lexer |

Pilot A's rejected files are genuinely malformed (a file may carry several categories). The grammar, `tree-sitter-ron` 0.2.0, has no crate API for tree-sitter 0.27; it was loaded through its raw C symbol with the tree-sitter 0.20 C runtime linked beside 0.27 — duplicate `_ts_*` symbols, undefined behaviour: in that build Rust parsing broke (`ast-hash` exit 1 "tree-sitter cancelled a parse", 38 / 68 `specengine-code` tests red). Its numbers come from that unsound binary and are indicative only: it rejects `#![enable(..)]` extension attributes and raw strings `r#"…"#` (hence fixture 50 %) and ran 5–6× slower than the lexer. The path was removed in iteration 3.

**`mcp-stdio` scripted** (checks (a)–(e) of AC-08; 22 tests against the real binary)

| Check | Result |
|---|---|
| `rmcp =3.5.0` builds | yes |
| (a) legacy `initialize` handshake + `tools/list` + `tools/call` | yes |
| (b) stateless 2026-07-28 request with `_meta` | yes: `server/discover` answered; `tools/list` carries `ttlMs` 0, `cacheScope` public |
| (c) legacy `elicitation/create` round trip | yes: accept / decline / cancel |
| (c) stateless `input_required` + `requestState` replay | yes; a tampered or cross-proposal state → -32602 |
| (d) `requiresUserInteraction` on `review_proposal` in `tools/list` | yes, both eras |
| (e) ≥ 26 k-token output without server error | yes: 104 000 bytes, both eras |
| cancellation → no response | yes (discriminating test) |

**`bevy-schedule`**

| Field | fixtures | pilot A | pilot B |
|---|---|---|---|
| instrumented build ok (y/n), failure category | – | yes, none | yes, none |
| patch size (lines), cold build | – | 6 in 2 files; 3 min 11 s, own target dir 4.3 GB (deleted) | 6 in 2 files (same shape); not recorded |
| dump obtained (y/n), fallback used (y/n) | – | yes (488 KB; headless run 2 s, exit 0), no | yes, no |
| dump fields unknown / missing vs the 0.19 schema | 0 / 0 (hand-written dump) | 0 / 0 | 0 / 0 |
| dumped: schedules / systems / sync points / own-crate systems | – | 28 / 622 / 46 / 230 | 16 / 510 / 37 / 181 |
| detected: systems / observers / plugins (`fn_app` + `impl`) / plugin uses | 1 / 1 / 2 (1 + 1) / 3 — the four hand-listed registrations | 389 / 6 / 131 (89 + 42) / 1 268 | 212 / 0 / 121 (69 + 52) / 507 |
| uncertain constructs / registrations inside macros | 0 / 1 (`tick`) | 0 / 0 | 0 / 0 |
| match (%), miss categories | – (by design 0 / 1: `macro_rules`) | 230 / 230 = 100.0 %, none | 169 / 181 = 93.4 %: 12 misses, all `generic_instance` (13 instances of one generic system, one registration site) → 100 % per registration site |
| dumped name found in sources (%) | – | 100.0 | 100.0 |
| detected, not in the dump: total (test / example targets, app code) | – | 159 (154, 5) | 43 (39, 4) |

Patch (both pilots, applied to a scratch copy — pilots are read-only): features `debug` + `schedule_data`, winit disabled, `ScheduleRunnerPlugin::run_once()`, `SerializeSchedulesPlugin`; both are single-binary apps. Assets can be left out of the copy (the dump is written before the first frame); `HOME` points at a scratch directory so the game cannot touch the owner's settings. The 0.19 dump holds schedules and their systems (`apply_deferred` = sync points), **no observers and no plugins** — so those two are compared nowhere; the comparison covers own-crate systems only (basis: "Bevy detector"). The fixture's hand-written dump exercises the comparison path: its one system is registered only inside `macro_rules!`, which is never matched.

**`rust-analyzer-library`** (values: without / with the proc-macro server)

| Field | pilot A | pilot B |
|---|---|---|
| `ra_ap_*` set builds (y/n); cold release build of the `--features ra` binary; target; binary | yes (rustc ≥ 1.98; the salsa 0.28.2 and unicode-ident 1.0.24 pins); ≈ 50–79 s; ≈ 0.42–0.44 GiB; ≈ 27 MB | same build |
| cold load (s) | 8.2 / 32.9 (build scripts 21.0) | 10.0 / 64.7 (build scripts 50.4) |
| peak RSS, analysing process (MiB) | 2 487.7 / 3 176.5 | 3 254.1 / 3 748.1 |
| peak RSS, largest child (MiB) | 141.2 / 609.1 | 187.6 / 1 119.6 |
| **whole-group peak `group_peak_rss_mb` (MiB)** | 2 459.2 / 3 246.7 | 3 228.8 / **3 849.3** (3.76 GiB) |
| floor `group_peak_rss_floor_mb` (MiB) | 2 487.7 / 3 246.7 | 3 254.1 / 3 849.3 |
| warm: full pass / edited file (ms), without · with | 233 / 72 · 256 / 61 | 168 / 31 · 173 / 33 |
| items → with a moniker (%) | 11 852 → 99.9 / 99.9 | 7 599 → 100.0 / 99.7 |
| proc-macro crates loaded / not loaded, without · with | 0 / 32 · 32 / 0 | 0 / 49 · 47 / 2 |
| crashes / timeouts / panics | 0 / 0 / 0 in both loads | 0 / 0 / 0 in both loads |
| `metadata_degraded`; `target_dir_fresh`; profile | false; true; release | false; true; release |

Final run: release harness, stable 1.98.1, `ra --proc-macros both --timeout 900` on scratch copies of the pilots, fresh target directories, group sampled every 100 ms; an external 1-s sampler agreed (B `with`: 3 853.7 MiB). Findings: (1) with the server 20 items lose their moniker — functions under attribute proc macros (`#[tokio::main]`, `#[tauri::command]` / `#[specta::specta]`-style): the scan does not map an item through its attribute expansion (open item for Phase 3). (2) Pilot B's 2 proc-macro crates not loaded are unexplained: `errors.json` gives no reason. (3) A pilot build script writes into its own source tree when a file is missing, hence scratch copies ("rust-analyzer loads"). (4) Only macOS was verified: the Linux `/proc` group reader and its unit tests were not compiled here (no Linux target).

**`import-census`**

| Field | fixtures | pilot A | pilot B |
|---|---|---|---|
| documents / with front-matter (per class) | 3 / 3 (class-1 2, class-2 1) | 369 / 369 (5 classes: 142, 98, 71, 32, 26) | 152 / 66 (no class key: all `unclassified`) |
| ID'd rows (per prefix) / rows without ID | 4 (prefix-1 3, prefix-2 1) / 1 | 2 593 (5 prefixes: 1 265, 1 129, 174, 20, 5) / 9 | 343 (3 prefixes: 287, 54, 2) / 185 |
| `{#ID}` sections | 1 | 0 | 0 |
| mixed-script IDs / non-Latin IDs | 1 / 0 | 0 / 20 | 0 / 287 |
| links to missing files / checked | 1 / — | 58 / 10 382 | 143 / 2 875 |
| records hashed | 5 | 2 593 | 343 |
| tables / record tables / headerless blocks (ID rows) | — | 1 145 / 123 / 31 (415) | 850 / 56 (by ID-column header regex) / 10 (20) |
| duplicate IDs | — | 1 407 | 110 |
| wall time (s) | — | ≈ 0.45 (release) | ≈ 0.16 |

One config per pilot, outside the repository. `—` = not recorded in the spec (detail stays under `--out`). What the broken links, non-Latin IDs, duplicates and rows without ID mean: "Verdicts".

### Owner checklist — interactive MCP checks

Run by the owner on 2026-09-28 with Claude Code 2.1.283 (installed version); a probe "token" is 4 bytes of filler ("Data"). Every row holds a result (AC-09); only (8) is "not measured", with its 08 §5 line. From the repository root, build first — `cargo build -p specengine-mcp --features probes` — so the first launch does not time out; record `claude --version`.

1. `claude --mcp-config fixtures/mcp/mcp.json` → `/mcp` shows `specengine` with 3 tools.
2. `review_proposal P-0001` three times: approve with a comment; decline; dismiss with Esc. Is the form rendered? What came back to the agent?
3. Restart with `MCP_PROTOCOL_NEGOTIATION=auto` and call again: the `era` field of the result shows which era Claude Code used.
4. With permission prompts bypassed, call `review_proposal`: is there still a prompt?
5. `probe_output` with 9 000, 12 000 and 26 000 tokens; then again with `MAX_MCP_OUTPUT_TOKENS=50000`.
6. `probe_sleep 150`: backgrounded after 2 min?
7. `review_proposal` with the form left open for 150 s before answering: does it stay in the foreground?
8. Streamable HTTP without GET → 405: not measured — the build is stdio-only.

| Check (step) | Result | Claude Code version |
|---|---|---|
| (1) `specengine` with 3 tools under the default (legacy) handshake | yes: launched via `fixtures/mcp/mcp.json`, `review_proposal` and `probe_output` callable; `/mcp` tool count not reported | 2.1.283 (installed version) |
| (2) elicitation form rendered | yes, in both eras | 2.1.283 (installed version) |
| (2) approve + comment returns to the tool | yes, split: `accept` + approve, no comment → `decision: approve`; a reject with a comment also came back to the tool | 2.1.283 (installed version) |
| (2) decline returns to the tool | yes: `decision: null`, `comment: null` | 2.1.283 (installed version) |
| (2) Esc (cancel) returns to the tool | yes, stateless era: form dismissed with Esc → `action: cancel`, `decision: null`, `comment: null`; the agent did not re-open the form on its own. A repeated call for the same ID after an earlier approve came back as a fresh request: the server keeps no state between calls (2026-07-28 is stateless, the tool writes nothing) | 2.1.283 (installed version) |
| (3) tools visible under `MCP_PROTOCOL_NEGOTIATION=auto`; `era` / `protocol_version` returned | yes: default launch → `era: legacy`, `protocol_version: 2025-11-25`; with `auto` → `era: stateless` (2026-07-28) | 2.1.283 (installed version) |
| (4) `requiresUserInteraction` prompt with permissions bypassed | yes: in bypass-permissions mode the elicitation/consent form still appeared — bypass mode cannot skip the owner's approval | 2.1.283 (installed version) |
| (5) 9 k-token output: observed behaviour | inline, complete (36 000 bytes); no size warning reached the model (a terminal UI warning not reported) | 2.1.283 (installed version) |
| (5) 12 k-token output: observed behaviour | inline, complete (48 000 bytes); no size warning reached the model (terminal UI not reported) | 2.1.283 (installed version) |
| (5) 26 k-token output: observed behaviour | not inserted: the full output (104 000 bytes, complete) saved to a file in the session's tool-results directory; the model got the error "result (104,000 characters across 1,625 lines) exceeds maximum allowed tokens" plus the file path | 2.1.283 (installed version) |
| (5) `MAX_MCP_OUTPUT_TOKENS=50000` raises the cap | no: 9 k and 12 k inline, 26 k rejected with the same error and saved to a file; the error counts characters → the cap appears character-based, between 48 000 and 104 000 characters, independent of the variable; per-tool `_meta["anthropic/maxResultSizeChars"]` not tested | 2.1.283 (installed version) |
| (6) 150 s call goes to background | yes: `probe_sleep 150` moved to the background after 120 s, ran to completion there, and its result arrived intact as a notification; the agent did not call the tool again | 2.1.283 (installed version) |
| (7) call with an open form exempt from backgrounding | yes: form held open 11 min — no error, no timeout, the call stayed pending until answered | 2.1.283 (installed version) |
| (8) Streamable HTTP without GET → 405 | not measured (stdio-only build) → 08 §5 "The MCP protocol changes again" stays | — |

## Verdicts

| Claim (source) | Verdict | Refuted → what changes | Not measured → 08 §5 row |
|---|---|---|---|
| Normalized tree-sitter walk is 100 % stable, traps 1–5 hold (05 §5.2, ADR-0021) | **confirmed after amendment**: recipe v2 is 100.0 / 100.0 / 100.0 on both pilots, `cannot_verify` hashes distinct; the v1 formula as written in 05 §5.2 is refuted (A 99.2 / 84.9, B 100.0 / 85.4) | 05 §5.2 takes the four v2 normalisations on shipping; `syn` did not win, so ADR-0021 stands | — |
| `syn` 3 is not a better digest (ADR-0021, 04 §6) | **confirmed** — `syn` 3 refuted as a replacement: normalized 94.5 / 94.9 % (< 100 %) on the pilots with 0 / 0 whole-file failures; the time ratio ≈ 0.53–0.56 does not decide | — (ADR-0021 stands) | — |
| Parse errors on real code are rare, ≈ 1 % of items (05 §5.2 research estimate) | **confirmed**: 0 items with errors on both pilots (11 852 + 7 599); only the fixture's three deliberately broken items reach `cannot_verify` | — | — |
| `qpath` heuristic (layer A) names items unambiguously (08 §2 Phase 0 "Module resolver") | **refuted for crate-root targets**: ambiguous 32.2 / 16.5 %, almost entirely duplicates — `src/bin`, `examples` and `tests` targets share an empty module path; `#[path]` 12 / 0 | 05 §5.1 "Module resolver" gains a target discriminator in `qpath` on shipping (a Phase 1 item); not a blocker — link identity comes from the marker (ADR-0020) | — |
| `tree-sitter-ron` 0.2.0 serves markers and field paths (05 §9 "AST" row, 04 §6) | **refuted → own RON lexer**: no crate API for tree-sitter 0.27, loads only with a second C runtime (undefined behaviour, broke Rust parsing); rejects `#![enable(..)]` and raw strings; 5–6× slower. The lexer: parse-clean 97.5 / 100 %, comment byte ranges, nested paths on fixtures | 05 §9 "AST" row and 04 §6 `tree-sitter-ron` become "own RON lexer" on shipping; no ADR — `.ron` markers stay (owner's answer) | — (08 §5 "Outdated RON grammar" updated on shipping) |
| Layer C is needed for registrations (05 §5.1, ADR-0020) | **confirmed** (owner's decision 2026-09-28: the threshold counts per system): the syntactic detector alone reaches 100.0 % on pilot A but 93.4 % on pilot B, below 95 %; every miss is `generic_instance` (instances of one generic system from a single site), which layer A cannot enumerate — a layer that sees instances (B: the `schedule_data` dump, or C: rust-analyzer) stays needed for them. Per registration site both pilots reach 100 % (information only); dumped names found in sources 100.0 % on both | — (ADR-0020 stands; no ADR) | — |
| `schedule_data` dump obtainable with a small patch (05 §5.1 layer B, 04 §6) | **confirmed** on both pilots: a 6-line patch in 2 files, single-binary apps, headless, no `bevy_mod_debugdump` fallback; 0 dump fields outside the 0.19 schema. Conditions: a scratch copy (pilots are read-only), assets may be left out (dump written before the first frame), `HOME` at a scratch directory | — (layer B stays `schedule_data`) | — (08 §5 "Bevy 0.20 changes the schedule API" stays: the dump schema is unstable across minors, re-run on upgrade) |
| rust-analyzer as a library is affordable on the laptop (04 §1.7, ADR-0020) | **confirmed** on both pilots with the proc-macro server: cold ≤ 3 min (worst 64.7 s, pilot B) and whole-group peak ≤ 4 GiB (worst 3 849.3 MiB, pilot B — ≈ 6 % headroom); 0 crashes, timeouts, panics; monikers on 99.7–100.0 % of items. Conditions: rustc ≥ 1.98, the salsa / unicode-ident pins, `with` loads on scratch copies | — (ADR-0020 stands; no ADR). Open for Phase 3: items under attribute proc macros lose their moniker | — (on shipping a new 08 §5 risk: "rust-analyzer memory near the 4 GiB threshold (≈ 6 % headroom on the heavier pilot); re-measure when a pilot grows or `ra_ap` is bumped") |
| Both eras, elicitation and `requiresUserInteraction` work in Claude Code (04 §4, 07 §1.1–1.2) | **confirmed** by script (AC-08) and interactively (AC-09, Claude Code 2.1.283): legacy handshake by default (`2025-11-25`), `MCP_PROTOCOL_NEGOTIATION=auto` negotiates 2026-07-28, the form renders and answers return in both eras (Esc → `cancel` observed in the stateless era); with permission prompts bypassed the consent form still appears (step 4), so bypass mode cannot skip the owner's approval — as 04 §4 says | — (the 07 §1.2 `owner` tool set and the ADR-0012 control point stand) | — |
| Output limits: warning from 10 k tokens, hard 25 k, raised by `MAX_MCP_OUTPUT_TOKENS` (04 §4) | **refuted / qualified** (2.1.283): 9 k / 12 k probes inline with no warning reaching the model; 26 k (104 000 characters) rejected by default — consistent with 25 k — but `MAX_MCP_OUTPUT_TOKENS=50000` did not raise it and the error counts characters: the effective cap appears character-based (48 000 < cap < 104 000 characters) and independent of the variable | 04 §4 output-limit line and 07 §1.1 response cap on shipping (MCP findings below) | — |
| An MCP call over 2 minutes goes to the background, except one blocked on an open elicitation form (04 §4) | **confirmed** (2.1.283): a plain 150 s call is backgrounded after 120 s, runs to completion and returns its result intact as a notification (step 6); a call held by an open form is exempt, observed up to 11 min with no error or timeout (step 7) | — (long-call consequence for 07 §1.1 in "MCP findings" below) | — |
| A corpus convention fits a config file, no corpus code in the core (08 §4, ADR-0008) | **confirmed** on both pilots: a config alone describes each corpus, nothing corpus-specific in code; it took three capabilities beyond the initial sample — headerless `\|` blocks, an ID-column header regex for record tables, wiki links | — (ADR-0008 stands; findings (a)–(e) below go to 08 §4 on shipping) | — |

**Census findings for 08 §4** (carried on shipping; each is a per-corpus importer setting or rule, not core code):

- (a) 57 of pilot A's 58 "broken" links point to existing files, written relative to the docs root rather than to the linking file → a per-corpus link base setting.
- (b) Pilot B's broken wiki links target 8 names that exist nowhere: genuine corpus debt for the baseline (08 §4.2 item 3).
- (c) Both pilots keep legacy non-Latin prefixes (20 / 287 IDs; pilot B's whole decision register) → ADR-0009 aliases are needed at import.
- (d) IDs recur across index and reference tables (duplicates 1 407 / 110) → a per-corpus rule telling a record's definition from a reference to it.
- (e) Locally numbered tables (a U+2116 numero-sign column) account for most of pilot B's 185 rows without an ID.

**MCP findings for 04 §4 and 07 §1.1–1.2** (carried on shipping):

- 04 §4 "(in rmcp: `ClientLifecycleMode::Auto`)" is client-side only; a server detects the era from the first message and serves `supported_protocol_versions` ("Data").
- Output cap (AC-09): 04 §4's "hard 25 k, raised by `MAX_MCP_OUTPUT_TOKENS`" gives way to the observed character-based cap. 07 §1.1: every tool response stays well under ~48 000 characters (the largest size measured safe), paginates beyond that and never relies on `MAX_MCP_OUTPUT_TOKENS`; its "≤ 10k tokens" cap is restated in characters. The per-tool `_meta["anthropic/maxResultSizeChars"]` lever is tested later.
- Long operations (AC-09 step 6), 07 §1.1: a tool running past 2 minutes may rely on backgrounding plus the result notification; no polling tool and no "call again" protocol are needed. A long-blocking "wait for approval" stays excluded (04 §4).
- Approval state (AC-09 step 2), 07 §1.2: the MCP server remembers nothing between calls, so an approval decision is stored by the proposal queue when given, never assumed remembered by the server; a `cancel` records nothing and leaves the proposal pending (the agent does not re-open the form on its own).
- Consent-tool requirements for 07 §1.2 (the Phase 0 demo has none of them): bind the sealed state's associated data to the proposal revision / patch hash; a single-use nonce persisted in the queue; a TTL; a shared `requestState` key once a multi-process HTTP server exists; on cancellation, `notifications/cancelled` for the outstanding `elicitation/create`.

## Out of scope

08 §2 items 2–3 (reading tracey/limpet/cgr sources, running tracey, "Hold W"); the rest of the product (`spec` CLI, daemon, SQLite, a writing importer, four-level fingerprints — only the `body` hash is measured; MCP tools beyond `review_proposal`); adding markers to pilots; the full importer "before/after/hashes" report; the Bevy 0.20 observer gate; HTTP beyond one yes/no; benchmarks (wall times are side information). On shipping the truths move to 05 §5.1, §5.2, §9; 04 §6; 07 §1.1–1.2; 08 §4, §5, the crate READMEs and any triggered ADR; the spec compacts to ≤ 3 KB.

## Implementation

**Group 1 `ast-hash`** — iteration 3, review accepted, tests green; group 2 added the use-run nesting cap and the explicit item stack.

| Module | What it does |
|---|---|
| `crates/specengine-code/src/grammar.rs` | pinned grammar: `TREE_SITTER_VERSION` / `TREE_SITTER_RUST_VERSION` constants checked by a test against the built artefact, `RustParser`, `grammar_info()` (ABI) |
| `…/hash.rs` | recipe `specengine-hash/v2`: recipe header, iterative walk, comments by `kind()`, anonymous `,` skipped, u32-LE length prefixes, attached attributes hashed before the item, `has_error` → `cannot_verify` never hashed (`HashState`, `Digest`, `ErrorCategory`); v2 normalisations: (a) `;` after an absent or diverging tail is transparent, (b) label-free single-expression blocks in closure bodies and match-arm values are unwrapped, (c) `{…}` token trees after `\|`, `\|\|`, `=>` inside `(`/`[` expression macros are transparent, (d) runs of `use` members = attached attributes ‖ `use`, sorted by walk; use-run member walk bounded at `MAX_USE_RUN_NESTING = 64` → `cannot_verify` `nesting_too_deep`; `normalize` returns `bool` (`false` = incomplete stream) |
| `…/comments.rs` | comment byte ranges and stripping (perturbation (c), `///` included) |
| `…/items.rs` | item and member enumeration (`ITEM_KINDS`, `MEMBER_KINDS`), `mod` declarations, `#[path]` count, per-file analysis; collection on an explicit stack (no recursion) |
| `…/qpath.rs` | qualified path per item, `FileRole` (`src/bin`, `examples`, `tests`, …), `Ambiguity` (`duplicate`, `path_attribute`), `#[path]` resolution |
| `crates/specengine-eval/src/main.rs`, `harness.rs` | CLI contract of "Data"; corpus resolution (`--pilot` or fixture), canonical read-only guard (exit 2), self-terminating timeout, percent helpers |
| `…/ast_hash/{mod,fmt,syn_cmp}.rs` | the measurement: three perturbations, stability and `files_changed`, JSON envelope; `rustfmt --emit stdout` with the default and contrast configs (`SPECENGINE_RUSTFMT` override); `syn` 3 comparison behind feature `syn` |
| tests | `specengine-code/tests/{hash_traps,hash_v2,hash_depth}.rs`; `specengine-eval/tests/{ast_hash_cli,build_graph,anonymity}.rs`; fixture `fixtures/ast-hash` (8 files, `expected.json`) |

**Deviations and accepted limitations (group 1).**

- tree-sitter-rust exposes attributes as siblings of the item, so the recipe hashes the attached attribute run before the item rather than inside it.
- Stable rustfmt 1.9.0 rejects `trailing_comma` (unstable), so perturbation (b) is effectively `max_width = 60`; it still rewrites 192 / 187 pilot files, which is what the perturbation needs.
- Rule (c) accepts a collision class: token-level DSL macros whose `| {` / `=> {` braces are significant hash the same with and without them (rustfmt never formats such macros); the remedy is a per-project opt-out list of macro names in `specengine.toml`.
- `mod x;` and `extern crate` runs reordered by rustfmt are not sorted (not observed on the pilots; v3 candidate).
- A nested brace-delimited macro inside an expression macro inherits rule (c) (minor, open).
- New pinned dependencies not yet in 04 §6: `serde =1.0.229`, `serde_json =1.0.151` (eval JSON), `quote =1.0.47`, `proc-macro2 =1.0.107` (feature `syn` only) — approved by the owner 2026-09-29 ("Rules and edge cases"), enter 04 §6 on shipping.

**Group 2 `ron`** — iteration 3, review accepted, tests green (115 / 115). Verdict `lexer`; the `tree-sitter-ron` comparison path (iteration 1) was removed in iteration 3. The owner's adjacency rules (after group 3) are implemented and reviewed.

| Module | What it does |
|---|---|
| `crates/specengine-code/src/markers.rs` | marker grammar shared by `.rs` and `.ron` comments: `Relation` (`implements`, `verifies`, `configures`, `assumes`), `Marker` (ID, optional rev, note, `id_latin`), `markers_in` |
| `…/ron/mod.rs` | `analyze` → `RonAnalysis`: markers with `Anchor` (path or `unanchored` / `cannot_verify`), `Rejected` categories, comment byte ranges; path rendering, `MAX_SEGMENT_BYTES` + `TRUNCATION_MARK`, one newline index per file |
| `…/ron/lexer.rs` | `lex` → tokens (comments, `#![enable(..)]`, raw strings) and `LexError` categories; linear on unterminated input |
| `…/ron/structure.rs` | the walk: struct / list / tuple / map adjacency; same-line trailing, opener and leading block-comment binding (pending markers with a trailing fallback, each resolved once); `MAX_DEPTH = 512` → `nesting_too_deep`, container skipped through its closer, error recovery |
| `crates/specengine-eval/src/ron.rs` | the measurement: `RonResult` envelope with the grammar-side shape kept (`GrammarStatus`, `ByApproach`), per-file detail to `--out` (`files.json`, `markers.json`, `rejected.json` with ≤ 20 samples per category) |
| tests | `specengine-code/tests/{ron_markers,ron_depth,ron_cost}.rs` (paths, adjacency, depth cap, cost bounds); `ron_markers.rs` `ADJACENCY` table (53 cases × LF / CRLF) with named mutations: drop the trailing claim after `,`, drop the own-line check, drop the opener claim, drop the leading branch, drop the fallback; `specengine-eval/tests/ron_cli.rs` (fixture == `expected.json`, guard, determinism, pilot runs via `SPECENGINE_PILOT_A`/`_B`); fixture `fixtures/ron` (`config.ron`, `extensions.ron`, `expected.json`) |

**Deviations, accepted limitations and open minors (group 2).**

- Two keys sharing their first 128 bytes render the same path; identity comes from the marker (ADR-0020). Phase 1 must flag duplicate RON paths as ambiguous, never merge them.
- Markers pending when error recovery consumes the next token are `unanchored` (or their trailing fallback, "RON markers"), not `cannot_verify`.
- Crafted inputs still cost more than linear in three places: an unterminated raw string with thousands of `#`, tens of thousands of markers on one comment line, paths up to ≈ 512 × 136 bytes per marker at maximal nesting.
- `#[must_use]` missing on `hash::normalize`; the `syn` comparison (eval only, feature `syn`) can overflow the stack on pathological nesting.
- `fixtures/ron/extensions.ron` still says in its header comment that "the grammar reports `extension_attribute` and `raw_string`" — stale since the removal (test-engineer).

**Group 3 `import-census`** — iteration 3, review accepted, tests green (165 / 165). Verdict: ADR-0008 confirmed.

| Module | What it does |
|---|---|
| `crates/specengine-import/src/config.rs` | `CensusConfig::load`: the "Data" schema, `deny_unknown_fields`, `ConfigError` as `file:line`, roots inside the corpus, exclude globs → regex, `IdPattern::find` (normalized search, first match, `prefix` group or leading ASCII letters) |
| `…/script.rs` | `IdScript` (`latin`, `mixed-script`, `non-latin`), `Normalized` (look-alike → ASCII, one char for one, ranges map back), `ascii_look_alike` (fullwidth ASCII, Cyrillic, Greek; escapes only in source) |
| `…/frontmatter.rs` | `read`: `---` block (CRLF, BOM), class value by `class_key`, unclosed block → diagnostic, body still read |
| `…/markdown.rs` | `scan`: one line scanner, linear per line — fences, HTML comments, code spans (`CodeSpans`), ATX headings with `{#ID}`, GFM tables, opt-in headerless blocks, inline links and reference definitions (`InlineIndex`), opt-in wiki links |
| `…/census.rs` | `run` → `Census`: corpus walk (roots, extensions, excludes, symlinks skipped, non-UTF-8 → diagnostic), record tables, `Record` (`RecordKind` row / section) with BLAKE3, link resolution, duplicates, `DocumentSummary`, `Diagnostic` |
| `crates/specengine-eval/src/census.rs` | the measurement: `load_config` (`--config` or `DEFAULT_CONFIG = "census.toml"`), `CensusResult` envelope with anonymised `class-N` / `prefix-N` keys and `detail`, detail files under `--out/census/<label>/` |
| `…/harness.rs` | `prepare_with`: the measurement's setup (the config read) runs after the read-only guard and before `--out` is created; its error is exit 2, nothing written |
| `xtask/src/docs/mod.rs` | `SKIP_DIRS` gains `fixtures` (test corpora with foreign conventions are not documents) |
| tests | `specengine-import/tests/census.rs` (config errors, scripts by escapes, front-matter, tables, links and escapes, sections, walk, cost bounds); `specengine-eval/tests/census_cli.rs` (fixture == `expected.json`, fixture untouched, determinism, guard, invalid / missing config, anonymous stdout, pilot runs via `SPECENGINE_PILOT_A`/`_B`); fixture `fixtures/corpus-mini` (`census.toml`, `design/`, `expected.json`) |

**Deviations (group 3).** stdout is anonymised including classes (not only prefixes; the mapping only in `labels.json` under `--out`); config keys beyond the initial sample (`corpus.extensions`, `corpus.exclude`, `tables.id_header`, `tables.headerless`, `[links]`) and the extra field `non_latin_ids`; the front-matter reader is not shared with the std-only `xtask`.

**Open minors (group 3).**

- Exclude globs do not prune directories: an excluded subtree is still walked.
- Non-Latin IDs whose letters have no Latin look-alike fall into rows without ID.
- An unclosed fence or HTML comment and skipped symlinks drop records without a diagnostic.
- `strip_comments` ignores backslash escapes; `<…>` destinations and `<!-->` comments deviate from CommonMark; entity references in destinations are not decoded.
- `SKIP_DIRS` in `xtask` matches `fixtures` at any depth, not only at the root.

**Group 4 `mcp-stdio`** — iteration 1, review accepted, tests green (22 scripted tests against the real binary; workspace 169 / 169). Scripted checks (AC-08) and the owner checklist (AC-09) done.

| Module | What it does |
|---|---|
| `crates/specengine-mcp/src/lib.rs` | crate doc (eras, tools, no-write rule); re-exports `Lifecycle`, `SpecEngineServer`, `serve_stdio`, `INSTRUCTIONS`, `ReviewOutcome`, `FormAction`, `Decision`, `Era` |
| `…/server.rs` | `SpecEngineServer` (`ServerHandler`): tool router, `get_info` (server info, `INSTRUCTIONS` + `PROBE_INSTRUCTIONS`, compile-time `TEXT_LIMIT = 2048` asserts), `Lifecycle` → `supported_protocol_versions`; `serve_stdio` (clean exit on a closed stdin, `ServeError`) |
| `…/review.rs` | `review_proposal`: `check_proposal_id`, form capability check, `ask_legacy` (`elicitation/create` via `send_request`, raced with cancellation), `ask_stateless` / `resume` (`input_required` under `INPUT_KEY = "owner_review"`, `RequestStateCodec` sealing with `associated_data`), `finish` → `ReviewOutcome` + prose |
| `…/probes.rs` | feature `probes`: `probe_output` (`BYTES_PER_TOKEN = 4`, `MAX_TOKENS = 200 000`), `probe_sleep` (`MAX_SECONDS = 900`, cancellable) |
| `…/main.rs` | CLI `--lifecycle auto\|legacy` (clap), current-thread tokio runtime, exit 0 / 1, one stderr line on failure |
| tests | `specengine-mcp/tests/common/mod.rs` (spawned-binary JSON-RPC client, `stateless_meta`, timeouts); `mcp_stdio.rs` (checks (a)–(e), ID refusals, cancellation, empty stdin, bad first message, working directory untouched, `mcp.json` fixture shape); `mcp_default.rs` (default build lists only `review_proposal`); fixture `fixtures/mcp/mcp.json` |

rmcp features: `server`, `macros`, `transport-io`, `request-state`. The `elicitation` feature is skipped because it pulls `url` / ICU; the wire message is sent with `send_request`. `chrono` arrives through rmcp's `schemars` feature.

**Deviations (group 4).** `requestState` has no TTL and is not single-use (a replay returns only the answer the client supplies again; a restart invalidates every state); `comment` is optional; the `--lifecycle` flag exists to make the AC-08 mutation executable; `readOnlyHint: false` on `review_proposal`; non-Latin IDs are refused without an autofix; the legacy form has no server-side timeout.

**Open minors (group 4).**

- Cancelling a legacy call does not cancel its outstanding form (fix: `send_cancellable_request` + `handle.cancel`).
- `check_proposal_id` echoes an unbounded ID into its error; the owner's `comment` is echoed unbounded twice (prose and `structuredContent`).
- Tool descriptions are not asserted ≤ 2 048 bytes (only `instructions` is).
- rmcp's stdio codec has no maximum line length — revisit with the daemon bridge.

**Group 5 `bevy-schedule`** — iteration 3, review accepted (no blocker / major), tests green (workspace 223 passed; the 8 ignored pilot tests 8 / 8 green; clippy, fmt, docs check clean). Verdicts: layer C (or B) needed for generic instances, `schedule_data` dump confirmed.

| Module | What it does |
|---|---|
| `crates/specengine-code/src/bevy/mod.rs` | the detector walk (`BevyAnalysis`: registrations with `Target`, `Origin`, `Form`, adapters; `Uncertain` with `UncertainCategory`; `COMBINATORS`, `ADAPTERS`, `MAX_NESTING`, `MAX_TEXT_BYTES`); plugin rules `impl Plugin for` and `fn(&mut App)` (`is_app_function`, `PluginKind`); plugin uses; `compact_span` / `compact_node`: whitespace-free text capped at `MAX_TEXT_BYTES` |
| `…/bevy/expr.rs` | registration arguments read from the syntax tree (ordinary code): tuples on an explicit stack, combinators, adapters, path / closure / factory leaves; `own_error` confines a parse error to its own call of a builder chain |
| `…/bevy/tokens.rs` | the same shapes read from macro token trees (`macro_rules!` transcribers, macro invocation arguments): flattened once, delimiter pairs and the one-pass `angle_close` table (`angle_closes`) let every walk jump over a group; names only |
| `crates/specengine-eval/src/bevy/mod.rs` | the measurement `bevy-detector` (alias `bevy`): corpus walk, own crates from `Cargo.toml` names, `--dump` read before anything is written, counts on stdout, detail files under `--out/bevy/<label>/` |
| `…/bevy/dump.rs` | typed reader of `app_data.ron` (Bevy 0.19 `schedule_data`) over the RON lexer of group 2: generic value tree on an explicit stack (`MAX_DEPTH`), per-value lines for errors, fields outside the schema counted |
| `…/bevy/compare.rs` | dump ↔ detector matching (`DumpSummary`): keys, one-to-one passes, miss categories, test / example / bench candidates after app code, deterministic order |
| tests | `specengine-code/tests/bevy_detector.rs` (35: forms, combinators, adapters, uncertain categories, plugin rules, macro bodies and arguments, pinned token-reader limits, nesting cap on a 2 MB stack, linear cost, determinism); `specengine-eval/tests/bevy_cli.rs` (19 + 2 ignored pilot tests via `SPECENGINE_PILOT_A`/`_B` and `…_DUMP`: fixture == `expected.json` with and without `--dump`, alias, determinism, guard incl. symlink, invalid / missing dumps at their line, schema drift, every miss category, counts-only stdout); fixture `fixtures/bevy-mini` (workspace-excluded; `src/main.rs`, `app_data.ron`, `expected.json`) |

**Deviations (group 5).** Both pilots were instrumented (the plan allowed the second only if the match rate stayed open); the dump schema has no observers or plugins, so the Results rows compare systems only; registrations read from macro tokens are reported but never matched against the dump; the harness neither patches, builds nor runs a pilot — the instrumented scratch-copy build ("Results") is a manual step outside the repository, and `--dump` reads its output.

**Open minors (group 5).**

- Token reader: with ≥ 3 generic items a middle path fragment of a comma-split element can be registered as a name; `sys.$m(c)` / `systems::$name` are `expression`, should be `metavariable` ("Bevy detector").
- `plugin_uses` names `P::<T>::default()` as `">"` in the token reader, and has no leftover rule in tokens.
- The `piped` list of the token reader differs from the code reader's.
- Output is sorted by line only: calls of one chain on the same line come out reversed, while the module doc claims source order.

**Group 6 `rust-analyzer-library`** — iteration 3, review accepted, tests green on stable 1.98.1 (workspace 232 passed / 8 skipped; `--features ra` 75 passed / 8 skipped; clippy on both, fmt, docs check clean). Iteration 2's major — the harness did not report the owner's measure, the whole-group peak — was fixed in iteration 3 (`group_peak_rss_mb`). Verdict: ADR-0020 confirmed. The `ron/mod.rs` `Anchor` rustdoc and module doc were aligned with adjacency rules 1–5 in the same pass.

| Module | What it does |
|---|---|
| `crates/specengine-ra/src/lib.rs` | layer C crate outside `default-members`: crate doc (three load steps, read-only rule), `RA_AP_VERSION = "0.0.352"` |
| `…/load.rs` | `Project::discover` (`cargo metadata` of workspace and sysroot, toolchain) → `run_build_scripts` (`with` only) → `into_workspace` (database, proc-macro server); read-only `CargoConfig` (target directory, `--locked --offline`, lock copied and resolved against the copy); `LoadError::category`; `Workspace::{file, scan, append, close}` |
| `…/scan.rs` | layer A item walk of a file (13 `ItemKind`s), `to_def` → `MonikerResult::from_def` → `MonikerStatus`; `scan_unloaded` for files outside every crate |
| `crates/specengine-eval/src/ra/mod.rs` | the measurement `ra`: `prepare` refusals; supervisor — one worker per load in its own process group, deadline and group kill, 10 s grace after the last report, `GroupSampler` (100 ms), event folding into `ModeResult` (`Field` = value or status, floor), `outer_budget` |
| `…/ra/worker.rs` | hidden `ra-worker`: one load through `specengine-ra`, step events on stdout, `items.json` / `errors.json` / `warm.json` |
| `…/ra/events.rs` | worker → supervisor events (`MetadataStep`, `BuildScriptsStep`, `DatabaseStep`, `PassStep` / `PassCounts`, `DoneStep`, `LoadFailedStep`) |
| `…/ra/sys.rs` | `libc`: `getrusage` (own and children peak RSS), `killpg` / `getpgrp`, `GroupRss` (macOS `proc_pidinfo`, Linux `/proc`), `mib` |
| `…/harness.rs` | `--label` must be one plain path component (`is_single_component`), exit 2 before writing, every subcommand |
| tests | `specengine-eval/tests/ra_cli.rs` (12: refusals, fixture run, read-only, determinism and anonymous stdout, timeout and process reaping, target freshness); `label_cli.rs` (2); `build_graph.rs` (group 6 additions: `ra` graph, pins, `libc`, `ra-mini` outside the workspace); fixture `fixtures/ra-mini/` |

**Deviations (group 6).** The RSS verdict counts the whole process group, not one process (owner's answer 2026-09-29), and uses `max(group_peak_rss_mb, group_peak_rss_floor_mb)`; `without` also skips build scripts; a pilot's `with` load needs a scratch copy — the harness cannot keep a corpus build script from writing into its own tree.

**Open minors (group 6).**

- The sampled group peak can fall below the floor (the verdict uses the max).
- `crashed` / `load_failed` loads still report a numeric group peak.
- The Linux `/proc` parser is compiled only on Linux (not verified here); zombie processes count 0 on Linux.
- `group_rss_sample_ms` is set even when no sample was read.
- Items under attribute proc macros lose their moniker with the server (Results finding (1); Phase 3).
