---
class: canon
tier: 1
scope: [crates/specengine-eval]
owner: owner
reviewed: 2026-10-06
---

# specengine-eval — the permanent measurement harness

One subcommand per measurement of the engine's claims over a corpus named at run time; re-run on a grammar, Claude Code, Bevy or `ra_ap` upgrade, for 08 §3 AC-1 / AC-10. A default member (`w`: the `specengine-cli` library); comparison-only dependencies behind features: `syn` (`syn =3.0.6` + `extra-traits`, `quote`, `proc-macro2`) and `ra` (`specengine-ra`, `libc`; rustc ≥ 1.98).

## CLI contract

```
specengine-eval <ast-hash|ron|census|import|parse|index|check|layout|w|bevy-detector|ra> [--pilot <dir>] --out <scratch-dir>
    [--label <name>] [--config <toml>] [--scheme <toml>] [--baseline <toml>] [--today <date>] [--dump <app_data.ron>]
    [--cargo-target-dir <dir>] [--proc-macros both|with|without] [--timeout <s>] [--tasks <toml>] [--budget <n>]
```

| Subcommand | Measures | Fixture (no `--pilot`) | Detail under `--out/<m>/<label>/` |
|---|---|---|---|
| `ast-hash` | stability of `specengine-hash/v2` under default rustfmt, contrasting rustfmt (effectively `max_width = 60`) and comment stripping; parse errors, `cannot_verify`, `qpath` ambiguity, units, target sources; `syn` 3 comparison (`null` without the feature) | `fixtures/ast-hash` | hash manifest (`unit`, `targets`) |
| `ron` | `.ron` markers by the own lexer: parse-clean share, rejected categories, comment ranges, anchors (`ambiguous` apart), level lists, empty IDs, colliding groups; keeps the removed grammar path's fields (`false` / `null`, `recommendation` `lexer`) | `fixtures/ron` | `files.json`, `markers.json`, `rejected.json` (≤ 20 samples per category) |
| `census` | `specengine-import` dry-run counts; convention from `--config` | `fixtures/corpus-mini` | `labels`, `records`, `documents`, `rows_without_id`, `broken_links`, `diagnostics` (`.json`) |
| `import` | `specengine-import`'s "before" report (its README); config as `census` | `fixtures/import-one` | ten files (its README) |
| `parse` | `specengine-core` over the census's documents (its `--config` and walk): `files` (walked), `unreadable`, `panics` (caught per file), `not_utf8`, `front_matter.present`, `diagnostics.<code>` (every code, zeros too), `sections.{parsed, census_id_sections, differ}` (`differ`: per-file symmetric difference of heading lines), `heading_attrs_not_section` (anchors), `references.{inline, declared, homoglyph, alias}` (`inline`: ID references only; `homoglyph`: non-alias ones), `tokens_est.{total, max_node}` | `fixtures/corpus-mini` | `files`, `diagnostics`, `sections_differ`, `panics`, `problems` (`.json`) |
| `index` | `specengine-store` over a scratch copy of the files the `[paths]` walk finds: `files`, `nodes`, `links`, `diagnostics`, `unreadable` (left out of the copy + stored with `read_error`), `missing_roots`; `full_ms` (first update), `noop_ms` + `noop_parsed`, `one_file_ms` + `one_file_parsed` (a line appended to the first file), `one_path_ms` (again, by `update_paths`) | `fixtures/spec-b` | `corpus/` (the copy, edited there), `index.db`, `reports.json` |
| `check` | `spec check` (the store's `load_check`, `check_source`), read-only: `files`, `verdicts.{observe, enforce}`, `codes.<code>.{error, warning, debt}`, `expired`, `stale` | `fixtures/spec-a` | `findings.json` (the report: paths, IDs, messages) |
| `layout` | the import model as an after-tree, read back by identity, its check attributed (`docs/canon/import-layout*.md`) | `fixtures/import-layout/one` | `tree/`, `index.db`, seven `.json` |
| `w` | task W, every task document: the pilot's reading protocol on the corpus vs `spec bundle` + follow-ups on the after-tree (`docs/canon/w-measurement.md`) | `fixtures/pilot-w/one` | `tree/`, `home-{1,2}/`, `bundles/`, `tasks.json` |
| `bevy-detector` (alias `bevy`) | the `specengine-code` detector; with `--dump` its match against a Bevy 0.19 `schedule_data` dump (own crates = `[package]`, `[lib]`, `[[bin]]` names), miss categories `generic_instance`, `repeated_site`, `other_schedule`, `macro_rules`, `macro_call`, `closure`, `indirect`, `not_in_source` | `fixtures/bevy-mini` | `registrations`, `plugins`, `plugin_uses`, `uncertain`, `crates`, `dump_match`, `dump_schema` (`.json`) |
| `ra` (feature `ra`) | layer C loads through `specengine-ra`: cold and warm time, peak RSS, moniker share, per load `without` / `with` the proc-macro server | `fixtures/ra-mini` | per mode: `items`, `errors`, `warm` (`.json`), the worker's `tmp/` |

`--label` names the detail directory (default `pilot` with `--pilot`, else `fixtures`); `pilot-a` / `pilot-b` read `SPECENGINE_PILOT_A` / `_B`. `--config` (`census`, `import`, `parse`, `layout`, `w`) defaults to the corpus root's `census.toml`, `--scheme` (`parse`: `[ids]`; `index`: + `[paths]`; `check`, `layout`, `w`: the whole config, validated like `spec check`) to its `specengine.toml`; for `--label pilot-a` / `pilot-b` a set `SPECENGINE_{CENSUS_CONFIG,SCHEME}_A` / `_B` comes first (a refusal names it). `check --baseline` defaults to the corpus root's `.spec-debt.toml` (if any), `--today` to today (UTC). `--timeout` (default 600 s) bounds a measurement (`layout`, `w`: after import and emission); `ra`: each load's budget (the run: loads × budget + 60 s), `--proc-macros` default `both` (`without` first), `--cargo-target-dir` `<out>/ra/<label>/target`. `ast-hash` targets (`cargo metadata`, else Cargo's layout, exit 0; cwd `/`), keys, budgets, rules: `docs/canon/code-identity.md`.

**Exit codes.** 0 = measured (a timed-out value: `"timeout"`); 2 = refused, nothing written: `--out` under the corpus (`layout`, `w`: either nesting), an unreadable pilot, a `--label` not one plain path component, `--config` on `index` / `check`, a missing or invalid census config, scheme, `check` baseline or `--today` (`file:line: message`); `layout`: a symlinked `<out>/layout[/<label>]`, an index output the tree holds; an unreadable or unparsable `--dump` (its line), `ra`: no `Cargo.toml` at the corpus root, `--cargo-target-dir` under the corpus; a `--timeout` the clock cannot represent; 1 = internal failure.

**Output.** stdout: one JSON object `{measurement, label, versions, wall_ms, result}`, counts and times only, never a file name, path, ID or class name (`class-N` / `prefix-N` / `pattern-N` by descending count, `unclassified`; the mapping in `labels.json`); stderr: a human summary; names and per-file detail only under `--out`, never in the repository.

## Rules

- **Read-only corpus.** The guard runs before any write; rustfmt runs on copies (`--emit stdout`); no marker is added to a pilot (ADR-0016); a fixture run leaves `git status -- fixtures/` empty.
- **Scratch copies.** `ra` `with` / `both` runs the corpus's build scripts and proc macros with its permissions: a pilot's `with` load, like the manual instrumented Bevy build for `--dump` (05 §5.1), runs only on a scratch copy outside the repository. The harness never patches, builds or runs a pilot.
- **`ra` loads.** Each load runs in the hidden `ra-worker` subcommand, leading its own process group, under the load's budget; on overrun the whole group (cargo, rustc, build scripts, proc macros) is killed, unreported fields become `"timeout"`; so is a worker outliving its last report by 10 s. `status`: `ok`, `timeout`, `crashed` (`detail.exit`), `load_failed` (`detail.load_error`). `without` skips build scripts too.
- **RSS verdict rule**: `max(group_peak_rss_mb, group_peak_rss_floor_mb)` vs 4096 MiB. The peak: the largest summed RSS of the load's process group, sampled every 100 ms (macOS, Linux; `null` elsewhere); the floor: the max of that peak and the worker's and largest child's `getrusage` peaks, a lower bound if sampling misses a spike.
- `ra` fields: `cold_ms` = metadata + build scripts + database + first pass; `warm_ms` = an in-memory appended function + a full pass; `metadata_degraded` = `--locked` failed, `--no-deps` used; `target_dir_fresh` = no build output in the target directory beforehand.

## Pilot runs and tests

Pilot paths come only from the environment: `SPECENGINE_{PILOT,CENSUS_CONFIG,SCHEME,TASKS}_A` / `_B` (configs outside the repository; `TASKS`: `w`), `SPECENGINE_PILOT_A_DUMP` / `_B_DUMP`; `SPECENGINE_RUSTFMT` overrides the rustfmt binary (canon above). Pilot tests are `#[ignore]`: `cargo nextest run -p specengine-eval --test <file> --run-ignored only pilot`. They fail, never skip, naming an unset or empty variable or a scheme without `[paths]`. Those of `index`, `check`, `parse`, `import`, `layout`, `w` (`tests/pilot/mod.rs`) give the child only `PATH`, the label's variables and an empty scratch `HOME` (kept empty), and prove the run read-only: `git --no-optional-locks status` and a (path, kind, size, mtime) listing of the scheme's roots (`import`: corpus and code roots) equal before and after. `ra` pilot runs: manual, release, under a self-terminating timeout.

Tests: a `<subcommand>_cli.rs` each (`bevy_cli.rs`; `index_cli.rs` pilot runs check 08 AC-10's `full_ms`, `one_file_ms`; `ra_cli.rs` `--features ra`), `import_gaps_cli.rs`, `ast_hash_units.rs` (`fixtures/cargo-units`, cargo and rustfmt stubs), `identity_literals.rs`, `doc_pointers.rs` (headings cited from code exist), `import_genre.rs`, `label_cli.rs`, `links_census.rs` (`broken_links` = `link-dangling`), `build_graph.rs` (nine default members, model / core / store layers, pins, the daemon's licences; feature-only dependencies, never Bevy, off the core graph), `anonymity.rs` (no absolute path or pilot name in `docs/`, `crates/`, `fixtures/`; raw Cyrillic only in `fixtures/{spec-b,token-calibration}/`: Q5, core README).

## Open minors

- `ra`: the sampled group peak can fall below the floor (the verdict takes the max); `crashed` / `load_failed` loads still report a numeric group peak; `group_rss_sample_ms` is set even when no sample was read; the Linux `/proc` reader and its Linux-only tests are unverified; zombies count 0 there.
- The `syn` comparison can overflow the stack on pathological nesting.