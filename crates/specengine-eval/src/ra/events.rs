//! The worker → parent protocol: one JSON object per line on the worker's
//! stdout, one per finished step, so a budget overrun still keeps every step
//! that finished. Aggregates only; names and paths go to the detail files.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Metadata(MetadataStep),
    BuildScripts(BuildScriptsStep),
    Database(DatabaseStep),
    FirstPass(PassStep),
    WarmFile(TimedStep),
    Warm(TimedStep),
    Done(DoneStep),
    /// A load step failed outright; nothing follows.
    LoadFailed(LoadFailedStep),
}

/// `cargo metadata` of the workspace and the sysroot, toolchain queries.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MetadataStep {
    pub ms: u64,
    /// Toolchain version of the corpus (e.g. `1.97.1`): the analysed code's own.
    pub toolchain: Option<String>,
    pub packages: usize,
    /// Metadata with dependencies failed (e.g. a stale lock under `--locked`)
    /// and the load fell back to `--no-deps`: no dependency crates, no build scripts.
    pub metadata_degraded: bool,
    pub sysroot_packages: usize,
    pub sysroot_error: bool,
}

/// `cargo check --compile-time-deps` into the scratch target directory.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildScriptsStep {
    pub ms: u64,
    /// `false`: skipped by the loader (degraded metadata).
    pub ran: bool,
    /// Some build script or proc-macro crate failed to build.
    pub errors: bool,
}

/// Files and crate graph into the database; proc-macro server started.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DatabaseStep {
    pub ms: u64,
    /// `disabled`, `running` or `failed`.
    pub proc_macro_server: String,
    /// Proc-macro crates whose dylib the server loaded / did not load
    /// (disabled, not built, rejected).
    pub proc_macro_crates_loaded: usize,
    pub proc_macro_crates_not_loaded: usize,
    /// Peak RSS of the worker so far (MiB).
    pub peak_rss_mb: Option<f64>,
}

/// One pass over every `.rs` file of the corpus: items and monikers.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PassStep {
    pub ms: u64,
    pub counts: PassCounts,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PassCounts {
    /// `.rs` files under the corpus (same walk as `ast-hash`).
    pub files: usize,
    /// Of those, loaded by rust-analyzer (inside a package root).
    pub files_loaded: usize,
    /// Of those, in some crate's module tree.
    pub files_in_crate: usize,
    /// Files whose scan panicked (rust-analyzer crash), items not counted.
    pub files_panicked: usize,
    pub files_unreadable: usize,
    pub items: usize,
    /// `moniker`, `local`, `none`, `unresolved`, `not_loaded` → items.
    pub by_status: BTreeMap<String, usize>,
    pub by_kind: BTreeMap<String, KindCounts>,
    /// Items whose moniker another item also has (inherent `impl` blocks of
    /// one type share theirs).
    pub duplicate_moniker_items: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct KindCounts {
    pub items: usize,
    pub moniker: usize,
    pub duplicate: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimedStep {
    pub ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DoneStep {
    /// Peak RSS of the worker (the analyser), MiB.
    pub peak_rss_mb: Option<f64>,
    /// Largest peak RSS among the worker's reaped children (cargo, rustc,
    /// the proc-macro server), MiB.
    pub children_peak_rss_mb: Option<f64>,
    /// No file had an item with a moniker: nothing to edit, no warm step.
    pub no_warm_target: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LoadFailedStep {
    /// `path`, `metadata`, `build_scripts`, `database`.
    pub category: String,
}
