---
class: canon
tier: 1
scope: [crates/specengine-ra]
owner: owner
reviewed: 2026-09-29
---

# specengine-ra — layer C: rust-analyzer as a library

Layer C of 05 §5.1 (ADR-0020): loads a Cargo workspace read-only into rust-analyzer's database and answers, for every item of a file, whether it resolves to a definition with a moniker (`MonikerResult::from_def`). Measured on both pilots: affordable (05 §5.1); used in Phase 3 for identity and Bevy registrations by resolved types.

## Placement and pins

- **Outside `default-members`**: `ra_ap_*` never enters the core build graph. It is reached only through `specengine-eval --features ra` today; `specengine-eval/tests/build_graph.rs` fails if it becomes a default member or if a core crate pulls `ra_ap_*`.
- Nine direct `ra_ap_*` crates (`load-cargo`, `project_model`, `ide`, `ide_db`, `hir_expand`, `vfs`, `paths`, `syntax`, `proc_macro_api`), all `=0.0.352` (`RA_AP_VERSION`); 32 `ra_ap_*` crates in the graph, all one version.
- Direct exact pins held only to keep that set building: `salsa`, `salsa-macros`, `salsa-macro-rules` `=0.28.2` (0.28.5 changed `HashEqLike`) and `unicode-ident =1.0.24` (1.0.26 is Unicode 18 against `unicode-properties` 0.1.4's Unicode 17). Bump them only together with `ra_ap`.
- `rust-version = "1.98"`: the set needs rustc ≥ 1.98 (the rest of the workspace needs 1.90). No `rust-toolchain.toml`.
- The proc-macro server is the sysroot's: `rustup component add rust-analyzer`.

## Loading — three steps a caller can time and bound

1. `Project::discover(LoadOptions)` — `cargo metadata` of the workspace and the sysroot, toolchain queries; `ProjectSummary` (toolchain, packages, sysroot, `metadata_degraded` when metadata with dependencies fails and the load falls back to `--no-deps`).
2. `Project::run_build_scripts` — only with `proc_macros`: `cargo check --compile-time-deps` builds build scripts and proc-macro dylibs into the target directory; `BuildScriptsSummary`.
3. `Project::into_workspace` — files and crate graph into the database, the proc-macro server when asked (`ProcMacroServer`: `disabled`, `running`, `failed`; `ProcMacroCrates` loaded / not loaded) → `Workspace`.

`Workspace::{file, scan, append, close}`: look a file up, scan its items, append text in memory (the warm edit), and **`close` shuts the proc-macro server down** — call it before reporting, so no server outlives the load. A failed step is a `LoadError` (`path`, `metadata`, `build_scripts`, `database` via `category()`); its message may name paths, so it belongs in per-run detail only.

`scan` walks the layer A item set (`specengine-code` `ITEM_KINDS` / `MEMBER_KINDS`: 13 `ItemKind`s, not entering function bodies or macro invocations) and gives each item a `MonikerStatus`: `Moniker` (non-local, SCIP-style descriptors), `Local`, `NoMoniker`, `Unresolved` (`cfg`-disabled or outside the module tree), `NotLoaded` (`scan_unloaded`, a file outside every package).

## Read-only cargo configuration

Every cargo call the loader triggers gets the caller's target directory (`CARGO_TARGET_DIR` and `--target-dir`, absolute, outside the root), `--locked` and `--offline`; an existing `Cargo.lock` is copied to a temporary directory and resolved against the copy, so nothing writes a lock file or the project's `target/`. **Build scripts and proc macros are the project's own code and run with its permissions**: they run only with `proc_macros`, and a real project's `with` load belongs on a scratch copy (a build script was seen writing into its own tree). The loader does not scrub the environment: the caller removes toolchain overrides (`RUSTUP_TOOLCHAIN`, `RUSTC`, `RUSTC_WRAPPER`, `CARGO_TARGET_DIR`, …) — `specengine-eval` does so when it spawns its worker, and points `TMPDIR` under `--out`.

## Known limits

- Items under attribute proc macros (`#[tokio::main]`-style) lose their moniker when the server runs: the scan does not map an item through its attribute expansion (Phase 3 item).
- On one pilot 2 proc-macro crates were not loaded without a reason in the load errors (unexplained).
- Memory is near the threshold: the whole-process-group peak reached 3.76 GiB against 4 GiB on the heavier pilot (08 §5); re-measure with `specengine-eval ra` when a pilot grows or `ra_ap` is bumped.
- Only macOS was verified. The Linux process-group RSS reader lives in `specengine-eval` (`ra/sys.rs`) and is not compiled here.

## Tests

End to end through `specengine-eval/tests/ra_cli.rs` (`--features ra`) on `fixtures/ra-mini`: its own `[workspace]` (`arena`, path proc-macro crate `arena-derive`) and `Cargo.lock`, a build script, a `cfg`-gated function, a stray file and a file outside every package; the fixture stays byte-identical and gets no `target/`.
