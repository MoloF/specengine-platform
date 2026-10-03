---
class: canon
tier: 1
scope: [crates/specengine-eval]
owner: owner
reviewed: 2026-10-03
---

# specengine-eval — the permanent measurement harness

One subcommand per measurement of the engine's claims over a corpus whose path enters at run time. Re-run on a grammar, Claude Code, Bevy or `ra_ap` upgrade and for AC-1 / AC-10 of 08 §3. A default workspace member; comparison-only dependencies stay behind features: `syn` (`syn =3.0.6` + `extra-traits`, `quote`, `proc-macro2`) and `ra` (`specengine-ra`, `libc`; rustc ≥ 1.98).

## CLI contract

```
specengine-eval <ast-hash|ron|census|parse|index|check|bevy-detector|ra> [--pilot <dir>] --out <scratch-dir>
    [--label <name>] [--config <toml>] [--scheme <toml>] [--baseline <toml>] [--today <date>] [--dump <app_data.ron>]
    [--cargo-target-dir <dir>] [--proc-macros both|with|without] [--timeout <s>]
```

| Subcommand | Measures | Fixture (no `--pilot`) | Detail under `--out/<m>/<label>/` |
|---|---|---|---|
| `ast-hash` | stability of `specengine-hash/v2` under default rustfmt, contrasting rustfmt (effectively `max_width = 60`) and comment stripping; parse errors, `cannot_verify`, `qpath` ambiguity, units, target sources; `syn` 3 comparison (`null` without the feature) | `fixtures/ast-hash` | hash manifest (`unit`, `targets`) |
| `ron` | `.ron` markers by the own lexer: parse-clean share, rejected categories, comment ranges, anchors (`ambiguous` apart), level lists, empty IDs, colliding groups; keeps the removed grammar path's fields (`false` / `null`, `recommendation` `lexer`) | `fixtures/ron` | `files.json`, `markers.json`, `rejected.json` (≤ 20 samples per category) |
| `census` | `specengine-import` dry-run counts; convention from `--config` | `fixtures/corpus-mini` | `labels.json`, `records.json`, `documents.json`, `rows_without_id.json`, `broken_links.json`, `diagnostics.json` |
| `parse` | `specengine-core` over the census's documents (its `--config` and walk): `files` (walked), `unreadable` (of them), `panics` (caught per file), `not_utf8`, `front_matter.present`, `diagnostics.<code>` (every code, zeros too), `sections.{parsed, census_id_sections, differ}` (`differ`: per-file symmetric difference of heading lines), `heading_attrs_not_section` (anchors), `references.{inline, declared, homoglyph, alias}` (`inline`: ID references only; `homoglyph`: non-alias ones), `tokens_est.{total, max_node}` | `fixtures/corpus-mini` | `files.json`, `diagnostics.json`, `sections_differ.json`, `panics.json`, `problems.json` |
| `index` | `specengine-store` over a scratch copy of the files the `[paths]` walk finds: `files`, `nodes`, `links`, `diagnostics`, `unreadable` (left out of the copy + stored with `read_error`), `missing_roots`; `full_ms` (first update), `noop_ms` + `noop_parsed`, `one_file_ms` + `one_file_parsed` (a line appended to the first file), `one_path_ms` (again, by `update_paths`) | `fixtures/spec-b` | `corpus/` (the copy, edited there), `index.db`, `reports.json` |
| `check` | `spec check` (the store's `load_check`, `check_source`), read-only: `files`, `verdicts.{observe, enforce}`, `codes.<code>.{error, warning, debt}`, `expired`, `stale` | `fixtures/spec-a` | `findings.json` (the report: paths, IDs, messages) |
| `bevy-detector` (alias `bevy`) | the `specengine-code` detector; with `--dump` its match against a Bevy 0.19 `schedule_data` dump (own crates = `[package]`, `[lib]`, `[[bin]]` names), miss categories `generic_instance`, `repeated_site`, `other_schedule`, `macro_rules`, `macro_call`, `closure`, `indirect`, `not_in_source` | `fixtures/bevy-mini` | `registrations.json`, `plugins.json`, `plugin_uses.json`, `uncertain.json`, `crates.json`, `dump_match.json`, `dump_schema.json` |
| `ra` (feature `ra`) | layer C loads through `specengine-ra`: cold and warm time, peak RSS, moniker share, per load `without` / `with` the proc-macro server | `fixtures/ra-mini` | per mode: `items.json`, `errors.json`, `warm.json`, the worker's `tmp/` |

`--label` names the detail directory: `pilot-a` / `pilot-b` without `--pilot` read `SPECENGINE_PILOT_A` / `_B`; the default is `pilot` with `--pilot`, `fixtures` without. `--config` (`census`, `parse`) defaults to the corpus root's `census.toml`, `--scheme` (`parse`: `[ids]`; `index`: + `[paths]`; `check`: the whole config, validated as by `spec check`) to its `specengine.toml`; for `--label pilot-a` / `pilot-b` a set `SPECENGINE_{CENSUS_CONFIG,SCHEME}_A` / `_B` comes first, and a refusal names the variable it looked for. `check --baseline` defaults to the corpus root's `.spec-debt.toml` if present, `--today` to the UTC date. `--timeout` (default 600 s) bounds a measurement; for `ra` it is the budget of each load, the run gets loads × budget + 60 s. `ra`: `--proc-macros` default `both` (`without` first), `--cargo-target-dir` default `<out>/ra/<label>/target`. `ast-hash` targets: `${SPECENGINE_CARGO:-cargo} metadata` per package dir, else Cargo's layout, exit 0; cargo and rustfmt run at cwd `/`. Keys, budgets, rules: `docs/canon/code-identity.md`.

**Exit codes.** 0 = measured (a timed-out value is the string `"timeout"`); 2 = refused before anything is written: `--out` under the corpus, an unreadable pilot, a `--label` that is not one plain path component, `--config` on `index` / `check`, a missing or invalid census config, `parse` / `index` / `check` scheme, `check` baseline or `--today` (`file:line: message`: an unknown key, a wrong type, a `..` or absolute root), an unreadable or unparsable `--dump` (with its line), for `ra` no `Cargo.toml` at the corpus root, `--out` or `--cargo-target-dir` under the corpus, a `--timeout` the clock cannot represent; 1 = internal failure.

**Output.** stdout is exactly one JSON object `{measurement, label, versions, wall_ms, result}`, counts and times only — never a file name, path, ID or class name: census classes and prefixes are `class-N` / `prefix-N` (by descending count) and `unclassified`, the mapping only in `labels.json`. stderr carries a human summary. Names and per-file detail go only under `--out`, never into the repository.

## Rules

- **Read-only corpus.** The guard runs before any file is opened for writing; rustfmt runs on copies through `--emit stdout`; no marker is ever added to a pilot (ADR-0016). A fixture run leaves `git status -- fixtures/` empty.
- **Scratch copies for code that runs.** `ra` with the proc-macro server (`with`, `both`) runs the corpus's build scripts and proc macros with its permissions: a pilot's `with` load runs only on a scratch copy outside the repository. The instrumented Bevy build for `--dump` (05 §5.1) is a manual step on a scratch copy; the harness never patches, builds or runs a pilot.
- **`ra` loads.** Each load runs in the hidden `ra-worker` subcommand, leading its own process group under the per-load budget; on overrun the whole group (cargo, rustc, build scripts, proc macros) is killed, unreported fields become `"timeout"`, no process is left; a worker outliving its last report by 10 s is killed. `status`: `ok`, `timeout`, `crashed` (`detail.exit`), `load_failed` (`detail.load_error`). `without` skips build scripts too.
- **RSS verdict rule.** Compare `max(group_peak_rss_mb, group_peak_rss_floor_mb)` with 4096 MiB. `group_peak_rss_mb` is the largest sum of resident sizes over the load's process group, sampled every 100 ms (macOS, Linux; `null` elsewhere); the floor is the largest of that peak and the worker's and largest child's `getrusage` peaks, a sure lower bound where sampling misses a spike.
- Other `ra` fields: `cold_ms` = metadata + build scripts + database + first pass; `warm_ms` = an in-memory appended function + a full pass; `metadata_degraded` = the lock did not resolve under `--locked`, fell back to `--no-deps`; `target_dir_fresh` = the target directory held no build output before the load.

## Pilot runs and tests

Pilot paths enter only through the environment: `SPECENGINE_{PILOT,CENSUS_CONFIG,SCHEME}_A` / `_B` (configs outside the repository), `SPECENGINE_PILOT_A_DUMP` / `_B_DUMP`; `SPECENGINE_RUSTFMT` overrides the rustfmt binary (a relative path: against the harness's cwd). Pilot tests are `#[ignore]`: `cargo nextest run -p specengine-eval --test <file> --run-ignored only pilot`. They fail, never skip: an unset or empty variable, or a scheme without `[paths]`, fails naming the variable. Those of `index`, `check`, `parse` (`tests/pilot/mod.rs`) give the child only `PATH`, the label's variables and an empty scratch `HOME` that must stay empty, and prove the run read-only: `git --no-optional-locks status` and a (path, kind, size, mtime) listing of the scheme's roots, equal before and after. `ra` pilot runs are manual, release build, under a self-terminating timeout.

Tests: `ast_hash_cli.rs`, `ast_hash_units.rs` (`fixtures/cargo-units`, cargo and rustfmt stubs), `identity_literals.rs`, `doc_pointers.rs` (headings cited from code exist), `ron_cli.rs`, `census_cli.rs`, `parse_cli.rs`, `index_cli.rs` (pilot runs check 08 AC-10: `full_ms` ≤ 10 000, `one_file_ms` ≤ 200), `check_cli.rs`, `bevy_cli.rs`, `ra_cli.rs` (`--features ra`), `label_cli.rs`, `links_census.rs` (`broken_links` = `link-dangling`), `build_graph.rs` (the eight default members, model / core / store layering, the pins; feature-only dependencies, never Bevy, outside the core graph), `anonymity.rs` (no absolute path or pilot name in `docs/`, `crates/`, `fixtures/`; raw Cyrillic only in `fixtures/spec-b/`, `fixtures/token-calibration/`: Q5, core README).

## Open minors

- `ra`: the sampled group peak can fall below the floor (the verdict takes the max); `crashed` / `load_failed` loads still report a numeric group peak; `group_rss_sample_ms` is set even when no sample was read; the Linux `/proc` reader and its unit tests are compiled only on Linux and were not verified here, and zombie processes count 0 there.
- The `syn` comparison can overflow the stack on pathological nesting.
- `parse` copies the census walk (→ `specengine-import` in the importer increment).