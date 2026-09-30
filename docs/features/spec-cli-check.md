---
class: spec
status: shipped
scope: [crates/specengine-cli, crates/specengine-core, crates/specengine-store, crates/specengine-eval]
ref: 08 §2 Phase 1, CLI pass 2a.1 = spec check increment 3 part 1; 07 §2; owner's answers Q3, Q4 (2026-09-30)
shipped: 2026-09-30
adrs: []
---

# The spec CLI, pass 2a.1: check and export index

## Why

The convention binds every project (ADR-0022), yet its only gate was this repository's `xtask`: hardcoded here, unusable in the pilots, blind to what is committed, without a debt ratchet (08 §4.2 item 3). The engine was a finished library, but no `spec` command ran it or wrote the index §11.5 compares, so 2b could not retire `xtask` (`docs/canon/architecture.md#checks-migration`). Agents get one bounded command — a line per blocking finding, a summary, an exit code — and index regeneration without a project script. No ADR: the gate is ADR-0022, the writer is the owner's Q3 within ADR-0003 (`#apply` amended); 2b needs an ADR amending ADR-0023. What shipped is canon in `docs/canon/spec-check-cli.md` (both commands, Q3, Q4, working answers, 2a.2 and 2b fixed, open items) and the CLI, store, core, eval READMEs.

## Acceptance criteria

In `crates/specengine-cli/tests/` unless named; copies of `fixtures/spec-a`, `-b` under `std::env::temp_dir()`, own `HOME` unless said; configs written into the copies, never `fixtures/`; only `CARGO_BIN_EXE_spec` spawned; far expiry dates (`2000-01-01`, `2999-12-31`). "The library": `check_worktree` on the same copy, `today_utc()`. Each mutation turns its test red.

- [x] AC-01 `crates/specengine-eval/tests/build_graph.rs`: the CLI's normal dependencies are pass 1's; no `git2`, `gix`. Mutation: `git2` added.
- [x] AC-02 `check.rs`, both fixtures: stdout = the library's `lines(false)`, exit = its `exit_code()`. A baseline covering every blocking finding → exit 0; then one front-matter reference to an undefined ID → exit 1, exactly one `error  ` line (`ref-dangling`), `— blocked`; `mode = "observe"` → exit 0, `— observed`; `--debt` → `lines(true)`. Mutation: exit 0 whatever the verdict.
- [x] AC-03 baseline: the copy's `.spec-debt.toml` applies; `--baseline <outside, without the entry>` replaces it; `--baseline <missing>` → exit 2, `cannot  <as typed>: …`, `— cannot-check`; `expires = "2000-01-01"` → exit 1, `1 expired`; `"2999-12-31"` → debt. Mutation: `--baseline` ignored when the root has one.
- [x] AC-04 JSON: exactly `to_json()` + `\n` for clean, blocked, observed, cannot-check; `--json --debt` the same bytes; no config up the tree → exit 2, empty stdout, one stderr line naming `spec init`. Mutation: no JSON for cannot-check.
- [x] AC-05 config: an unknown top-level table; an unknown `[project]` key (at `spec index`'s line); `tier0_bytes = 0`; `mode = "strict"` → exit 2, one cause `specengine.toml:<line>: …` each (`--config` as typed when passed); no `[project]` → checks normally. Mutation: `ProjectConfig` validation skipped.
- [x] AC-06 `read_only.rs`: `HOME` unset → `check` (each verdict), `export index`, `--stdout` exit as their verdicts say; `HOME` set → nothing created under it; after `check` the copy is byte- and path-identical, after `export index` only `[paths] index` differs. Mutation: `check` refreshes the index.
- [x] AC-07 `determinism.rs`: two copies at different absolute paths, files written in opposite orders → byte-identical stdout, stderr, text and JSON, blocked and cannot-check (bad default baseline); no absolute root printed. Mutation: the default baseline named by `root.join(…)`.
- [x] AC-08 a file `a\nb.md` with a blocking error → stdout has blocking findings + 1 lines; JSON `path` keeps the LF. Mutation: no `one_line`.
- [x] AC-09 `export.rs`, both fixtures, `command = "gen-index"`, `index = true`: the file = the library's `render_index` (header naming `gen-index`, `spec check`), then `spec check` has no `index-drift`, `index-missing`; a rerun: `unchanged`, mtime kept; `--stdout`: the same bytes, nothing written; JSON `{path, bytes, written}`; `gate = "gen-check"` reaches the header. Mutations: the header names the binary; equal bytes rewritten.
- [x] AC-10 `parity.rs`: a scratch copy of this repository's walked documents, the config from `check_parity.rs`'s code (shared, not copied), `docs/index.md` deleted → `spec export index` recreates it byte-identical to the committed file; `spec check` → exit 0, `clean`. Mutation: the final `\n` dropped.
- [x] AC-11 refusals, exit 2, empty stdout, index unchanged or absent: no `[[generators]]`; no `index = true`; a mode-000 document, directory; a missing written root; the index or its directory a symlink (target unchanged); a missing parent (not created); a config error. A non-UTF-8 name → written, one `warning:`. Mutation: write despite a read error.
- [x] AC-12 `exit.rs`, `names.rs`: every row of the exit and stream table, text and `--json`; bare `spec export`, `--stdout --json` → exit 2; stderr lines start only with `spec:`, `warning:`, `note:`, `<config>:<line>:` (+ `Usage:`), no ANSI. Mutation: the report on stderr.
- [x] AC-13 `genre.rs`: AC-02…AC-09 pass on both fixtures; no genre literal in `crates/specengine-cli/src`; no `[paths] index` → AC-11's refusal. Mutation: a `"docs/index.md"` fallback.
- [x] AC-14 dogfood: `spec check --root <this repository> --config <parity config in a temp dir>` → exit 0, `clean`, stdout = the library's; `git status --porcelain --untracked-files=all` unchanged (git only observes). Mutation: `--config` ignored under `--root`.
- [x] AC-15 `crates/specengine-store/tests/check_loader.rs`: config bytes named `cfg.toml`, an unknown `[project]` key and `mode = "strict"` → two causes `cfg.toml:<line>` only; `check_worktree` = the loader's report on spec-a; a broken default baseline → cause `.spec-debt.toml`; `specengine-eval check` with `[bogus]` → exit 2 at its line. Mutation: eval keeps its own parsing.
- [x] AC-16 docs: `cargo xtask docs check` green; worst W ≤ 117 739 B; `docs/canon/spec-check.md` net growth ≤ 0; CLI README ≤ 10 240 B; `#apply` carries Q3; Q3, Q4 in canon, dated; 07 §2 names `spec export index [--stdout]`; `cargo nextest run --workspace` once; clippy, fmt clean.

## Implementation

Three iterations, review accepted in the second and third; the full workspace run: 820 passed, 0 failed, 15 skipped. (1) `spec check` (fresh parse, no DB or `HOME`, verdict exit codes, the report on cannot-check); the store's loader seam, `check_worktree` now its wrapper, two absolute-path leaks fixed; `spec export index` (writer, refusals, `--stdout`); eval `check` validating the whole config. (2) Review major: `--json` with `export index --stdout` refused in every argument order with one message; an unreadable root is the cause `.`, not a phantom baseline; the overwrite compares device and inode, refusing a replaced file; core's `walk_gap`, the §11.5 predicate shared with the writer. (3) A pre-existing walker defect gave a false `clean`: a directory on the way to a default root that cannot be listed was filed as a missing root; now `unreadable_dirs` → cannot check, exit 2; `spec index` warns.

| Module | What it does |
|---|---|
| cli `check.rs` | `check`, `CheckRequest`, `CheckOutcome`: loader, `check_tree`, text and JSON |
| cli `export.rs` | `export_index`, `ExportIndexRequest`, `ExportOutcome`: registration, `walk_gap` refusal, warnings, the guarded write |
| cli `project.rs`, `main.rs`, `lib.rs` | `locate` (discovery, config unread); the subcommands; the `--stdout --json` usage error; `CliError::lines` |
| core `check/generated.rs` | `WalkGap`, `walk_gap` |
| store `check.rs`, `lib.rs` | `NamedBytes`, `CheckSetup`, `default_baseline`, `load_config`, `load_check`, `check_source`, `check_tree`, `check_worktree` |
| store `source.rs` | an unlistable directory on the way to a root → `unreadable_dirs` (`""` = the root) |
| eval `check.rs` | `load_check` + `check_source` |

Accepted deviations: `Box<Report>` errors (clippy); `check_worktree` names the config by its file name, a passed baseline as passed, the default `.spec-debt.toml`, the root `.`; the mode on a config error is `enforce` (read only when all of `CheckConfig` is valid); `--stdout --json` a usage error; `--json --debt` adds one `note:`; the default baseline rule "an entry exists" (a directory or dangling symlink of that name refuses the run); export config errors one line per cause; bare `spec export` prints clap's full help (its help block counts as the usage block); the unreachable "`[paths] index` is not set" guard kept. Known limits (causes sorted as strings, `ProjectConfig`'s first error only, untested paths): `docs/canon/spec-check-cli.md` "Open".
