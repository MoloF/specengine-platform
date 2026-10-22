---
class: canon
tier: 1
scope: [crates/specengine-ra]
owner: owner
reviewed: 2026-10-22
---

# specengine-ra — layer C: rust-analyzer as a library

Layer C of 05 §5.1 (ADR-0020, ADR-0049): loads a Cargo workspace read-only into rust-analyzer's database and answers, for every item of a file, whether it resolves to a definition with a moniker (`MonikerResult::from_def`). Measured on both pilots: affordable (05 §5.1). A library for `specengine-eval --features ra` and the crate's own executable `specengine-ra`, the only process that runs layer C for `spec`.

## Placement and pins

- **Outside `default-members`**: `ra_ap_*` never enters the core build graph. It is reached only through `specengine-eval --features ra` and the `specengine-ra` binary; `specengine-eval/tests/build_graph.rs` fails if it becomes a default member, if a core crate or the CLI pulls `ra_ap_*`, or if the direct dependencies leave the pinned set below.
- Nine direct `ra_ap_*` crates (`load-cargo`, `project_model`, `ide`, `ide_db`, `hir_expand`, `vfs`, `paths`, `syntax`, `proc_macro_api`), all `=0.0.352` (`RA_AP_VERSION`); 32 `ra_ap_*` crates in the graph, all one version.
- Direct exact pins held only to keep that set building: `salsa`, `salsa-macros`, `salsa-macro-rules` `=0.28.2` (0.28.5 changed `HashEqLike`) and `unicode-ident =1.0.24` (1.0.26 is Unicode 18 against `unicode-properties` 0.1.4's Unicode 17). Bump them only together with `ra_ap`.
- The binary's own, all already pinned in `[workspace.dependencies]`: `serde`, `serde_json` (request and reply lines), `libc` (`getrusage`, `getpgrp`, `kill`). Exactly 16 direct normal dependencies, no `ra_ap_hir`; dev: `specengine-code` (the kind map, B4's layer A comparison). Licences: the daemon's set plus `CC0-1.0` (`notify`, `docs/canon/stack.md`).
- `rust-version = "1.98"`: the set needs rustc ≥ 1.98 (the rest of the workspace needs 1.90). No `rust-toolchain.toml`.
- The proc-macro server is the sysroot's: `rustup component add rust-analyzer`.

## The executable

`specengine-ra --protocol 1` (`src/main.rs`): one request line on stdin, JSON lines on stdout, exits 0/1/2/3; request, reply, order, endings, environment scrub, watchdogs and read-only guarantees: `docs/canon/layer-c-protocol.md`. Install: `cargo install --locked --force --path crates/specengine-ra`, beside `spec`, from the same commit.

| Module | What it does |
|---|---|
| `main` | the scrub first (seven overrides removed, both wrappers `""`), argv, request, `TMPDIR` = `tmp_dir`, the two watchdogs (`terminate`: silent with the last line's code once it is written), `hello`, the analysis thread (64 MiB stack), exit codes |
| `request` | `check_argv`, `read_line` (≤ 64 MiB), `parse` (every check, in order), `create_dirs` |
| `reply` | the line types, `t` first; `PROTOCOL` 1; `ByState`; `emit` (one write, flushed); `emit_done`, `emit_error` (the last line, `Ending` 0/1) behind the `LAST_LINE` mark, set before the first byte; `last_line` |
| `run` | nearest manifests, workspaces one at a time (discover, claim members, load, scan, close), files parsed alone, `file` lines in request order, `done`; `read_text` (every claimed requested file read and UTF-8 checked before any scan, else `path`); `guarded` maps a rust-analyzer panic to `metadata` or `database` |
| `sys` | `peak_rss` (`RUSAGE_SELF` + `RUSAGE_CHILDREN`, bytes), `leads_its_group`, `kill_own_group` |

## Loading — three steps a caller can time and bound

1. `Project::discover(LoadOptions)` — `cargo metadata` of the workspace and the sysroot, toolchain queries; `ProjectSummary` (toolchain, packages, sysroot, `metadata_degraded` when metadata with dependencies fails and the load falls back to `--no-deps`).
2. `Project::run_build_scripts` — only with `proc_macros`: `cargo check --compile-time-deps` builds build scripts and proc-macro dylibs into the target directory; `BuildScriptsSummary`.
3. `Project::into_workspace` — files and crate graph into the database, the proc-macro server when asked (`ProcMacroServer`: `disabled`, `running`, `failed`; `ProcMacroCrates` loaded / not loaded) → `Workspace`.

`specengine-eval ra` times them: `cold_ms` = metadata + build scripts + database + first pass; `warm_ms` = an in-memory appended function + a full pass; `metadata_degraded` = `--locked` failed, `--no-deps` used; `target_dir_fresh` = no build output in the target directory beforehand.

Each load runs in the eval's hidden `ra-worker` subcommand, leading its own process group, under the load's budget; on overrun the whole group (cargo, rustc, build scripts, proc macros) is killed and unreported fields become `"timeout"`, as does a worker outliving its last report by 10 s. `status`: `ok`, `timeout`, `crashed` (`detail.exit`), `load_failed` (`detail.load_error`). `without` skips build scripts too.

**RSS verdict rule**: `max(group_peak_rss_mb, group_peak_rss_floor_mb)` vs 4096 MiB. The peak: the largest summed RSS of the load's process group, sampled every 100 ms (macOS, Linux; `null` elsewhere); the floor: the max of that peak and the worker's and largest child's `getrusage` peaks, a lower bound if sampling misses a spike.

`Project::workspace_root` (the root cargo resolved) and `member_manifests` (sorted: the root manifest and the members') let a caller group files by workspace before loading. `Workspace::{file, file_at, scan, append, close}`: look a file up (root-relative, or `file_at` absolute), scan its items, append text in memory (the warm edit), and **`close` shuts the proc-macro server down** — call it before reporting, so no server outlives the load. A failed step is a `LoadError` (`path`, `metadata`, `build_scripts`, `database` via `category()`); its message may name paths, so it belongs in per-run detail only.

`scan` walks the layer A item set (`specengine-code` `ITEM_KINDS` / `MEMBER_KINDS`: 13 `ItemKind`s, not entering function bodies or macro invocations) and gives each item a `MonikerStatus`: `Moniker` (non-local, SCIP-style descriptors), `Local`, `NoMoniker`, `Unresolved` (`cfg`-disabled or outside the module tree), `NotLoaded` (`scan_unloaded`, a file outside every package). `ItemRecord.line` counts from the node's start (attributes included); `start`, `end` are bytes on the raw text (CRLF kept) from the first child that is no `ATTR`, `DOC_COMMENT`, `OUTER_DOC_COMMENT`, `COMMENT` or `WHITESPACE` (the set rust-analyzer's `remove_attrs_and_docs` strips; in 0.0.352 a doc comment is a node of its own, not trivia) to the node's end: layer A's item range.

## Read-only cargo configuration

Every cargo call the loader triggers gets the caller's target directory (`CARGO_TARGET_DIR` and `--target-dir`, absolute, outside the root), `--locked` and `--offline`; an existing `Cargo.lock` is copied to a temporary directory and resolved against the copy, so nothing writes a lock file or the project's `target/`. **Build scripts and proc macros are the project's own code and run with its permissions**: they run only with `proc_macros`, and a real project's `with` load belongs on a scratch copy (a build script was seen writing into its own tree). The loader does not scrub the environment: the caller removes toolchain overrides (`RUSTUP_TOOLCHAIN`, `RUSTC`, `RUSTC_WRAPPER`, `CARGO_TARGET_DIR`, …) — `specengine-eval` does so when it spawns its worker, and points `TMPDIR` under `--out`; the binary does so first in `main` (`docs/canon/layer-c-protocol.md` "Environment").

## Known limits

- Items under attribute proc macros (`#[tokio::main]`-style) lose their moniker when the server runs: the scan does not map an item through its attribute expansion (11b, RM-0022). TE-6 (2026-10-22, `specengine-eval ra`): with the server no item gains one, A loses 1 (0.01 %), B 19 (0.25 %), cold 3.9 and 6.3 times; under 11b's 1 % trigger.
- On one pilot 2 proc-macro crates were not loaded without a reason in the load errors (unexplained).
- Memory is near the threshold: the whole-process-group peak reached 3.76 GiB against 4 GiB on the heavier pilot (08 §5); re-measure with `specengine-eval ra` when a pilot grows or `ra_ap` is bumped.
- Only macOS was verified. The Linux process-group RSS reader lives in `specengine-eval` (`ra/sys.rs`) and is not compiled here.

## Tests

On `fixtures/ra-mini`: its own `[workspace]` (`arena`, path proc-macro crate `arena-derive`) and `Cargo.lock`, a build script, a `cfg`-gated function, a stray file and a file outside every package; the fixture stays byte-identical and gets no `target/`. The library end to end through `specengine-eval/tests/ra_cli.rs` (`--features ra`); the binary through `tests/protocol.rs` (one file: each integration test executable links `ra_ap`), its masked golden `fixtures/layer-c/ra-mini.jsonl` (`docs/canon/layer-c-protocol.md` "Tests"). Build only `-p specengine-ra`, never `--workspace`: a debug binary is ≈ 120 MB, a run on ra-mini 20-50 s in debug.
