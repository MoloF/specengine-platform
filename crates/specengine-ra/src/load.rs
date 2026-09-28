//! Read-only loading of a Cargo workspace into rust-analyzer's database, in
//! three steps a caller can time: metadata, build scripts, database.

use std::fmt;
use std::path::{Path, PathBuf};

use ra_ap_hir_expand::proc_macro::ProcMacros;
use ra_ap_ide::{AnalysisHost, FileId};
use ra_ap_ide_db::{ChangeWithProcMacros, FxHashMap};
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, load_workspace};
use ra_ap_paths::{AbsPathBuf, Utf8PathBuf};
use ra_ap_proc_macro_api::ProcMacroClient;
use ra_ap_project_model::{
    CargoConfig, ProjectManifest, ProjectWorkspace, ProjectWorkspaceKind, RustLibSource,
    TargetDirectoryConfig,
};
use ra_ap_vfs::{FileExcluded, Vfs, VfsPath};

use crate::scan::{self, FileScan};

/// Given to every `cargo metadata` and `cargo check` the loader runs: never
/// write or update a lock file, never reach the network.
const CARGO_READ_ONLY_ARGS: [&str; 2] = ["--locked", "--offline"];

/// What to load and where cargo may write.
#[derive(Clone, Debug)]
pub struct LoadOptions {
    /// Workspace root: the directory holding `Cargo.toml`; absolute UTF-8.
    pub root: PathBuf,
    /// Target directory of every cargo invocation; absolute UTF-8, outside `root`.
    pub target_dir: PathBuf,
    /// With proc macros: build scripts run (`cargo check --compile-time-deps`
    /// into `target_dir`) and the sysroot's proc-macro server expands macros.
    /// Without: neither, so no cargo build runs at all.
    pub proc_macros: bool,
}

/// A load step that failed outright. The message may name paths: it belongs
/// in per-run detail, never in aggregates; [`LoadError::category`] does.
#[derive(Debug)]
pub enum LoadError {
    /// A path is not absolute UTF-8 (rust-analyzer's paths must be).
    Path(String),
    /// The manifest, `cargo metadata` or the toolchain queries failed.
    Metadata(String),
    /// Build scripts could not be started (their own failures are
    /// [`BuildScriptsSummary::error`], not this).
    BuildScripts(String),
    /// Files and crate graph could not be loaded into the database.
    Database(String),
}

impl LoadError {
    /// Stable category name for aggregates.
    pub fn category(&self) -> &'static str {
        match self {
            LoadError::Path(_) => "path",
            LoadError::Metadata(_) => "metadata",
            LoadError::BuildScripts(_) => "build_scripts",
            LoadError::Database(_) => "database",
        }
    }

    fn message(&self) -> &str {
        match self {
            LoadError::Path(message)
            | LoadError::Metadata(message)
            | LoadError::BuildScripts(message)
            | LoadError::Database(message) => message,
        }
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.category(), self.message())
    }
}

impl std::error::Error for LoadError {}

/// Step 1 done: the workspace as `cargo metadata` describes it.
pub struct Project {
    workspace: ProjectWorkspace,
    config: CargoConfig,
    root: PathBuf,
    proc_macros: bool,
}

/// What step 1 found. Error texts may name paths (detail only).
#[derive(Clone, Debug)]
pub struct ProjectSummary {
    /// Toolchain version cargo reported for the workspace, e.g. `1.97.1`.
    pub toolchain: Option<String>,
    /// Packages in the crate graph source (workspace, dependencies, sysroot).
    pub packages: usize,
    /// `cargo metadata` with dependencies failed and the loader fell back to
    /// `--no-deps`: the workspace has no dependency crates and build scripts
    /// do not run. A stale lock file under `--locked` lands here.
    pub metadata_error: Option<String>,
    /// Sysroot packages loaded (0: no standard library in the graph).
    pub sysroot_packages: usize,
    pub sysroot_error: Option<String>,
}

/// What step 2 found.
#[derive(Clone, Debug)]
pub struct BuildScriptsSummary {
    /// `false` when the loader skips them (degraded metadata).
    pub ran: bool,
    /// Build scripts or proc-macro crates that failed to build (text may name paths).
    pub error: Option<String>,
}

/// Proc-macro crates of the loaded crate graph (see [`Workspace::proc_macro_crates`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProcMacroCrates {
    pub loaded: usize,
    pub not_loaded: usize,
}

/// State of the proc-macro server after step 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcMacroServer {
    /// Proc macros were not requested.
    Disabled,
    Running,
    /// Requested, but the sysroot has no server or it did not start.
    Failed,
}

impl ProcMacroServer {
    pub fn as_str(self) -> &'static str {
        match self {
            ProcMacroServer::Disabled => "disabled",
            ProcMacroServer::Running => "running",
            ProcMacroServer::Failed => "failed",
        }
    }
}

impl Project {
    /// Step 1: `cargo locate-project`, `cargo metadata` of the workspace and of
    /// the sysroot, toolchain and target queries — all read-only (module doc
    /// of the crate).
    pub fn discover(options: &LoadOptions) -> Result<Project, LoadError> {
        let manifest = abs_utf8(&options.root.join("Cargo.toml"))?;
        let target_dir = utf8_absolute(&options.target_dir)?;
        let manifest = ProjectManifest::from_manifest_file(manifest)
            .map_err(|error| LoadError::Metadata(format!("{error:#}")))?;
        let config = read_only_config(target_dir);
        let workspace = ProjectWorkspace::load(manifest, &config, &|_| {})
            .map_err(|error| LoadError::Metadata(format!("{error:#}")))?;
        Ok(Project {
            workspace,
            config,
            root: options.root.clone(),
            proc_macros: options.proc_macros,
        })
    }

    pub fn summary(&self) -> ProjectSummary {
        let metadata_error = match &self.workspace.kind {
            ProjectWorkspaceKind::Cargo {
                error: Some(error), ..
            } => Some(format!("{error:#}")),
            _ => None,
        };
        ProjectSummary {
            toolchain: self.workspace.toolchain.as_ref().map(ToString::to_string),
            packages: self.workspace.n_packages(),
            metadata_error,
            sysroot_packages: self.workspace.sysroot.num_packages(),
            sysroot_error: self.workspace.sysroot.error().map(str::to_owned),
        }
    }

    /// Step 2, only meaningful with proc macros: `cargo check --workspace
    /// --compile-time-deps` into the target directory builds build scripts and
    /// proc-macro dylibs; their outputs (`OUT_DIR`, cfgs, dylib paths) enter the
    /// crate graph. Failures of individual scripts are reported, not fatal.
    pub fn run_build_scripts(&mut self) -> Result<BuildScriptsSummary, LoadError> {
        let ran = matches!(
            &self.workspace.kind,
            ProjectWorkspaceKind::Cargo { error: None, .. }
        );
        let scripts = self
            .workspace
            .run_build_scripts(&self.config, &|_| {})
            .map_err(|error| LoadError::BuildScripts(format!("{error:#}")))?;
        let error = scripts.error().map(str::to_owned);
        self.workspace.set_build_scripts(scripts);
        Ok(BuildScriptsSummary { ran, error })
    }

    /// Step 3: files and crate graph into a fresh database; with proc macros,
    /// the sysroot's `rust-analyzer-proc-macro-srv` is spawned and every
    /// proc-macro dylib built in step 2 is loaded into it. No cache priming:
    /// queries stay lazy.
    pub fn into_workspace(self) -> Result<Workspace, LoadError> {
        let load_config = LoadCargoConfig {
            load_out_dirs_from_check: self.proc_macros,
            with_proc_macro_server: if self.proc_macros {
                ProcMacroServerChoice::Sysroot
            } else {
                ProcMacroServerChoice::None
            },
            prefill_caches: false,
            num_worker_threads: 1,
            proc_macro_processes: 1,
        };
        let (db, vfs, server) =
            load_workspace(self.workspace, &self.config.extra_env, &load_config)
                .map_err(|error| LoadError::Database(format!("{error:#}")))?;
        let proc_macro_server = match (self.proc_macros, server.is_some()) {
            (false, _) => ProcMacroServer::Disabled,
            (true, true) => ProcMacroServer::Running,
            (true, false) => ProcMacroServer::Failed,
        };
        Ok(Workspace {
            host: AnalysisHost::with_database(db),
            vfs,
            root: self.root,
            server,
            proc_macro_server,
        })
    }
}

/// A loaded workspace: the database, its files and the proc-macro server.
pub struct Workspace {
    host: AnalysisHost,
    vfs: Vfs,
    root: PathBuf,
    /// The client of the proc-macro server. Every proc macro loaded into the
    /// database holds a handle to the same server process too, so the process
    /// is killed and reaped only when this and the database are both dropped
    /// ([`Workspace::close`]).
    server: Option<ProcMacroClient>,
    proc_macro_server: ProcMacroServer,
}

/// A file of the loaded workspace (see [`Workspace::file`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoadedFile(FileId);

impl Workspace {
    pub fn proc_macro_server(&self) -> ProcMacroServer {
        self.proc_macro_server
    }

    /// Proc-macro crates of the crate graph whose dylib the server loaded,
    /// and those it did not (disabled, not built, or rejected).
    pub fn proc_macro_crates(&self) -> ProcMacroCrates {
        let db = self.host.raw_database();
        let mut crates = ProcMacroCrates::default();
        let Some(proc_macros) = ProcMacros::try_get(db) else {
            return crates;
        };
        for crate_macros in proc_macros.by_crate(db).values() {
            if crate_macros.get_error().is_none() {
                crates.loaded += 1;
            } else {
                crates.not_loaded += 1;
            }
        }
        crates
    }

    /// The loaded file at `relative` (to the workspace root), or `None` when
    /// rust-analyzer did not load it (outside every package root).
    pub fn file(&self, relative: &Path) -> Option<LoadedFile> {
        let absolute = abs_utf8(&self.root.join(relative)).ok()?;
        match self.vfs.file_id(&VfsPath::from(absolute)) {
            Some((file_id, FileExcluded::No)) => Some(LoadedFile(file_id)),
            _ => None,
        }
    }

    /// Every item of `file` with its moniker status (see [`FileScan`]).
    pub fn scan(&self, file: LoadedFile) -> FileScan {
        scan::scan_loaded(self.host.raw_database(), file.0)
    }

    /// An in-memory edit: `suffix` appended to the file's current text. The
    /// file on disk is never touched.
    pub fn append(&mut self, file: LoadedFile, suffix: &str) -> Result<(), String> {
        let text = self
            .host
            .analysis()
            .file_text(file.0)
            .map_err(|_| "the file text query was cancelled".to_owned())?;
        let mut change = ChangeWithProcMacros::default();
        change.change_file(file.0, Some(format!("{text}{suffix}")));
        self.host.apply_change(change);
        Ok(())
    }

    /// Drops the database, its files and the proc-macro client. The proc-macro
    /// server process lives as long as any handle to it — the client and every
    /// proc macro the database loaded — so it is killed and reaped only here,
    /// once both are gone; on return it is no longer running.
    pub fn close(self) {
        let Workspace {
            host, vfs, server, ..
        } = self;
        drop(host);
        drop(server);
        drop(vfs);
    }
}

/// The cargo configuration of every load: read-only flags, the caller's
/// target directory for every invocation (the environment variable reaches
/// `cargo rustc --print` and the sysroot's `cargo metadata` too, so not even
/// `.rustc_info.json` lands in the project), rust-analyzer's defaults otherwise.
fn read_only_config(target_dir: Utf8PathBuf) -> CargoConfig {
    let mut extra_env = FxHashMap::default();
    extra_env.insert("CARGO_TARGET_DIR".to_owned(), Some(target_dir.to_string()));
    extra_env.insert("CARGO_NET_OFFLINE".to_owned(), Some("true".to_owned()));
    let read_only: Vec<String> = CARGO_READ_ONLY_ARGS
        .iter()
        .map(|arg| (*arg).to_owned())
        .collect();
    CargoConfig {
        all_targets: true,
        sysroot: Some(RustLibSource::Discover),
        extra_args: read_only.clone(),
        metadata_extra_args: read_only,
        extra_env,
        target_dir_config: TargetDirectoryConfig::Directory(target_dir),
        // Only for toolchains before 1.89 (no `--compile-time-deps`): it would
        // make the calling binary a rustc wrapper, which this one is not.
        wrap_rustc_in_build_scripts: false,
        set_test: true,
        ..CargoConfig::default()
    }
}

fn abs_utf8(path: &Path) -> Result<AbsPathBuf, LoadError> {
    AbsPathBuf::try_from(utf8_absolute(path)?)
        .map_err(|path| LoadError::Path(format!("not absolute: {path}")))
}

fn utf8_absolute(path: &Path) -> Result<Utf8PathBuf, LoadError> {
    let utf8 = Utf8PathBuf::from_path_buf(path.to_path_buf())
        .map_err(|path| LoadError::Path(format!("not UTF-8: {}", path.display())))?;
    if utf8.is_absolute() {
        Ok(utf8)
    } else {
        Err(LoadError::Path(format!("not absolute: {utf8}")))
    }
}
