//! Layer C of SpecEngine (05 §5.1, ADR-0020): rust-analyzer as a library.
//!
//! One pinned release of the `ra_ap_*` crates ([`RA_AP_VERSION`]) loads a Cargo
//! workspace read-only and answers, for every source item of a file, whether
//! rust-analyzer resolves it to a definition with a moniker
//! (`MonikerResult::from_def`).
//!
//! Loading runs in three explicit steps so a caller can time and bound each:
//! [`Project::discover`] (`cargo metadata` of the workspace and the sysroot,
//! toolchain queries), [`Project::run_build_scripts`] (only with proc macros:
//! `cargo check --compile-time-deps`, which builds build scripts and
//! proc-macro dylibs) and [`Project::into_workspace`] (files and crate graph
//! into the database, the proc-macro server when asked).
//!
//! **Read-only.** Every cargo invocation the loader triggers gets the caller's
//! target directory (`CARGO_TARGET_DIR` and `--target-dir`), `--locked` and
//! `--offline`, so `cargo metadata` never writes a lock file and nothing lands
//! in the project's own `target/`; the loader copies an existing `Cargo.lock`
//! to a temporary directory and resolves against the copy. Build scripts and
//! proc macros are the project's own code and run with the project's
//! permissions: layer C runs them only when proc macros are requested.
//!
//! This crate is not a default workspace member, so `ra_ap_*` never enters the
//! core build graph (AC-01 of `docs/features/phase-0-spikes.md`).

mod load;
mod scan;

pub use load::{
    BuildScriptsSummary, LoadError, LoadOptions, LoadedFile, ProcMacroCrates, ProcMacroServer,
    Project, ProjectSummary, Workspace,
};
pub use scan::{FileScan, ItemKind, ItemRecord, MonikerStatus, scan_unloaded};

/// The one `ra_ap_*` release this crate is built against (pinned in the root manifest).
pub const RA_AP_VERSION: &str = "0.0.352";
