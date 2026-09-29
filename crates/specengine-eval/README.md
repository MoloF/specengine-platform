---
class: canon
tier: 1
scope: [crates/specengine-eval]
owner: owner
reviewed: 2026-09-29
---

# specengine-eval — the permanent measurement harness

One subcommand per measurement of the engine's claims over a corpus whose path enters at run time. Re-run on a grammar, Claude Code, Bevy or `ra_ap` upgrade and for AC-1 / AC-10 of 08 §3. A default workspace member; comparison-only dependencies stay behind features: `syn` (`syn =3.0.6` + `extra-traits`, `quote`, `proc-macro2`) and `ra` (`specengine-ra`, `libc`; rustc ≥ 1.98). Default features are empty.

## CLI contract

```
specengine-eval <ast-hash|ron|census|parse|bevy-detector|ra> [--pilot <dir>] --out <scratch-dir>
    [--label <name>] [--config <toml>] [--scheme <toml>] [--dump <app_data.ron>]
    [--cargo-target-dir <dir>] [--proc-macros both|with|without] [--timeout <s>]
```

| Subcommand | Measures | Fixture (no `--pilot`) | Detail under `--out/<m>/<label>/` |
|---|---|---|---|
| `ast-hash` | stability of `specengine-hash/v2` under default rustfmt, contrasting rustfmt (effectively `max_width = 60`: stable rustfmt rejects `trailing_comma`) and comment stripping; parse errors, `cannot_verify`, `qpath` ambiguity; `syn` 3 comparison (`null` without the feature) | `fixtures/ast-hash` | hash manifest |
| `ron` | `.ron` markers by the own lexer: parse-clean share, rejected categories, comment ranges, anchors; keeps the removed grammar path's fields (`grammar` `{built: false, loads: false, …}`, per-approach `null`, `recommendation` `lexer`) | `fixtures/ron` | `files.json`, `markers.json`, `rejected.json` (≤ 20 samples per category) |
| `census` | `specengine-import` dry-run counts; convention from `--config`, default `census.toml` at the corpus root | `fixtures/corpus-mini` | `labels.json`, `records.json`, `documents.json`, `rows_without_id.json`, `broken_links.json`, `diagnostics.json` |
| `parse` | `specengine-core` over the census's documents (its `--config` and walk): `files`, `panics` (caught per file), `not_utf8`, `front_matter.{present, diagnostics.<code>}`, `sections.{parsed, census_id_sections, differ}` (`differ`: per-file symmetric difference of heading lines), `heading_attrs_not_section` (anchors), `references.{inline, declared, homoglyph, alias}`, `tokens_est.{total, max_node}` | `fixtures/corpus-mini` | `files.json`, `diagnostics.json`, `sections_differ.json`, `panics.json`, `problems.json` |
| `bevy-detector` (alias `bevy`) | the `specengine-code` detector; with `--dump` its match against a Bevy 0.19 `schedule_data` dump (own crates = `[package]`, `[lib]`, `[[bin]]` names), miss categories `generic_instance`, `repeated_site`, `other_schedule`, `macro_rules`, `macro_call`, `closure`, `indirect`, `not_in_source` | `fixtures/bevy-mini` | `registrations.json`, `plugins.json`, `plugin_uses.json`, `uncertain.json`, `crates.json`, `dump_match.json`, `dump_schema.json` |
| `ra` (feature `ra`) | layer C loads through `specengine-ra`: cold and warm time, peak RSS, moniker share, per load `without` / `with` the proc-macro server | `fixtures/ra-mini` | per mode: `items.json`, `errors.json`, `warm.json`, the worker's `tmp/` |

`--label` names the detail directory: `pilot-a` / `pilot-b` without `--pilot` read `SPECENGINE_PILOT_A` / `_B`; the default is `pilot` with `--pilot`, `fixtures` without. `parse --scheme` (`[ids]`) defaults to `SPECENGINE_SCHEME_A` / `_B` for `--label pilot-a` / `pilot-b`, else the corpus root's `specengine.toml`. `--timeout` (default 600 s) bounds a measurement; for `ra` it is the budget of each load, the run gets loads × budget + 60 s. `ra`: `--proc-macros` default `both` (`without` first), `--cargo-target-dir` default `<out>/ra/<label>/target`.

**Exit codes.** 0 = measured (a timed-out value is the string `"timeout"`); 2 = refused before anything is written: `--out` under the corpus, an unreadable pilot, a `--label` that is not one plain path component (empty, a separator, `.`, `..`, absolute), a missing or invalid census config or `parse` scheme (`file:line: message`), an unreadable or unparsable `--dump` (with its line), for `ra` no `Cargo.toml` at the corpus root, `--out` or `--cargo-target-dir` under the corpus, a `--timeout` the clock cannot represent; 1 = internal failure.

**Output.** stdout is exactly one JSON object `{measurement, label, versions, wall_ms, result}`, counts and times only — never a file name, path, ID or class name: census classes and prefixes are `class-N` / `prefix-N` (by descending count) and `unclassified`, the mapping only in `labels.json`. stderr carries a human summary. Names and per-file detail go only under `--out`, never into the repository.

## Rules

- **Read-only corpus.** The guard runs before any file is opened for writing; rustfmt runs on copies through `--emit stdout`; no marker is ever added to a pilot (ADR-0016). A fixture run leaves `git status -- fixtures/` empty.
- **Scratch copies for code that runs.** `ra` with the proc-macro server (`with`, `both`) runs the corpus's build scripts and proc macros with its permissions, and a pilot build script was seen writing into its own tree: a pilot's `with` load runs only on a scratch copy outside the repository. The instrumented Bevy build for `--dump` (6-line patch, own target directory, headless, `HOME` at a scratch directory; 05 §5.1) is a manual step on a scratch copy; the harness never patches, builds or runs a pilot.
- **`ra` loads.** Each load runs in the hidden `ra-worker` subcommand, leading its own process group under the per-load budget; on overrun the whole group (cargo, rustc, build scripts, proc-macro server) is killed, unreported fields become `"timeout"`, no process is left; a worker outliving its last report by 10 s is killed. `status`: `ok`, `timeout`, `crashed` (`detail.exit`), `load_failed` (`detail.load_error`). `without` skips build scripts too.
- **RSS verdict rule.** Compare `max(group_peak_rss_mb, group_peak_rss_floor_mb)` with 4096 MiB. `group_peak_rss_mb` is the largest sum of resident sizes over the load's process group, sampled every 100 ms (`proc_pidinfo` on macOS, `/proc/<pid>/stat` on Linux, `null` elsewhere); the floor is the largest of that peak and the worker's and largest child's `getrusage` peaks, a sure lower bound where sampling misses a spike.
- Other `ra` fields: `cold_ms` = metadata + build scripts + database + first pass; `warm_ms` = an in-memory appended function + a full pass; `metadata_degraded` = the lock did not resolve under `--locked`, fell back to `--no-deps`; `target_dir_fresh` = the target directory held no build output before the load.

## Pilot runs and tests

Pilot paths enter only through the environment: `SPECENGINE_{PILOT,CENSUS_CONFIG,SCHEME}_A` / `_B` (configs live outside the repository), `SPECENGINE_PILOT_A_DUMP` / `_B_DUMP`; `SPECENGINE_RUSTFMT` overrides the rustfmt binary. Pilot tests are `#[ignore]`: `cargo nextest run -p specengine-eval --test <file> --run-ignored only pilot`. `ra` pilot runs are manual, release build, under a self-terminating timeout.

Tests: `ast_hash_cli.rs`, `ron_cli.rs`, `census_cli.rs`, `parse_cli.rs`, `bevy_cli.rs`, `ra_cli.rs` (`--features ra`), `label_cli.rs`, `build_graph.rs` (the seven default members, model / core layering, the pins; `ra_ap_*`, `libc` only with `--features ra`; no `syn` 3 or Bevy in the core graph), `anonymity.rs` (no absolute path or pilot name in `docs/`, `crates/`, `fixtures/`; raw Cyrillic only in `fixtures/spec-b/`, `fixtures/token-calibration/`: Q5, core README).

## Open minors

- `ra`: the sampled group peak can fall below the floor (the verdict takes the max); `crashed` / `load_failed` loads still report a numeric group peak; `group_rss_sample_ms` is set even when no sample was read; the Linux `/proc` reader and its unit tests are compiled only on Linux and were not verified here, and zombie processes count 0 there.
- The `syn` comparison can overflow the stack on pathological nesting.
- `parse`: `line_of` is O(sections × bytes); `front_matter.diagnostics` also counts body diagnostics, `files` unreadable files; the census walk is copied here (→ `specengine-import` in the importer increment).