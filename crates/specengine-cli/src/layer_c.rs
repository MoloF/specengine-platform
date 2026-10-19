//! `spec resolve [--timeout S] [--json]` (task spec `layer-c-resolve`):
//! the one command that spawns the layer C executable, checks its reply
//! and stores it as the worktree's one advisory snapshot; and
//! [`layer_c_state`], the snapshot's freshness. No other command, read,
//! MCP tool or daemon route reaches the spawn.
//!
//! In order (each failure exit 2, stdout empty, nothing stored):
//!
//! 1. Unix only; discovery, the config, `[code]`, the spec index and the
//!    code index refreshed, a walk gap refused (6a's reads). The request's
//!    files: every walked Rust file whose content is UTF-8, in the walk's
//!    byte order; none: refused. Their tree fingerprint computed.
//! 2. `<data dir>/<slug>.ra/lock` taken (`File::try_lock`); `target/`,
//!    `tmp/` beside it; that directory canonicalised.
//! 3. The binary beside `spec`, else on `PATH` (`spec serve`'s lookup).
//! 4. One run on the process-group core ([`crate::layer_c_run`]): the
//!    request, the reply judged line by line, the ending.
//! 5. After `done` and exit 0: the code index refreshed again and the tree
//!    fingerprint recomputed; equal: one transaction replaces the
//!    snapshot; else nothing stored.
//!
//! Every reason after step 1 reads `spec resolve: <reason>; the previous
//! snapshot is kept`. Timings (`ms`, `s`, `GiB`, `peak_rss`) are printed
//! here only, the CLI's one exception to "no timing"; they are never
//! stored. The join with layer A, agreement and collisions are the code
//! crate's (`specengine_code::layer_c`); this module hashes bytes and
//! counts.
//!
//! **Freshness** (`tree_fp`): BLAKE3 over byte-sorted entries: the
//! reply's protocol, `ra_ap` and `host`; each requested file's path and
//! BLAKE3; each code package's directory and manifest hash; the bytes of
//! each file the code crate names ([`tree_files`]) in each directory from
//! a walked file's up to the root that exists (one that cannot be read:
//! its name alone). [`layer_c_state`]: `absent` (no snapshot), `fresh`
//! (equal), `stale` (any change).

// Off Unix only the refusal is built; the rest is the run's.
#![cfg_attr(not(unix), allow(dead_code, unused_imports))]

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use specengine_code::MOD_KIND;
use specengine_code::layer_c::{
    JoinItem, TargetCrate, agrees, collision_groups, join_file, target_crate, tree_files,
};
use specengine_store::{
    AMBIGUITY_PATH_ATTRIBUTE, AMBIGUITY_UNROOTED, CodeTreeFile, CodeTreeRows, LayerCFile,
    LayerCItem, LayerCSnapshot, SnapshotSymbols, SqliteIndex, StoredSymbol, b3_hash,
};

use crate::code_index::{LANG_RUST, package_targets, refresh_code};
use crate::corpus::indexed_in;
use crate::location::{open_index, prepared_data_dir};
use crate::project::{ProjectRoot, discover_with_text};
use crate::refs::code_config;
use crate::{CliError, Env, Globals, Message, escape_controls, store_error};

/// The layer C executable's name (canon `layer-c-protocol`): looked up
/// beside `spec` or on `PATH` by `spec resolve` only.
pub(crate) const LAYER_C_BIN: &str = "specengine-ra";

/// The fewest, most and default seconds of `--timeout`.
pub const RESOLVE_TIMEOUT_MIN: u64 = 1;
pub const RESOLVE_TIMEOUT_MAX: u64 = 3600;
pub const RESOLVE_TIMEOUT_DEFAULT: u64 = 300;

/// The suffix of the slug's layer C directory in the data directory.
const LAYER_DIR_SUFFIX: &str = ".ra";
/// Its lock file, the binary's target directory and its scratch.
const LOCK_FILE: &str = "lock";
const TARGET_DIR: &str = "target";
const TMP_DIR: &str = "tmp";

/// How the reasons after step 1 end.
const KEPT: &str = "the previous snapshot is kept";

/// The fingerprint's entry kinds.
const ENTRY_PROTOCOL: &str = "protocol";
const ENTRY_RA_AP: &str = "ra_ap";
const ENTRY_HOST: &str = "host";
const ENTRY_FILE: &str = "file";
const ENTRY_PACKAGE: &str = "package";
const ENTRY_BYTES: &str = "bytes";
const ENTRY_UNREADABLE: &str = "unreadable";

/// Bytes in a GiB, milliseconds in a second.
const GIB: f64 = 1_073_741_824.0;
const MS_PER_S: f64 = 1000.0;

/// What `spec resolve` is given besides the globals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveRequest {
    /// `--timeout`, seconds ([`RESOLVE_TIMEOUT_MIN`] to
    /// [`RESOLVE_TIMEOUT_MAX`]).
    pub timeout: u64,
    /// This `spec`'s path (`std::env::current_exe()`), if known: the
    /// binary is looked for beside it first.
    pub spec_path: Option<PathBuf>,
    /// `PATH`, searched next.
    pub path: Option<OsString>,
}

/// What `spec resolve` stored and counted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolveOutcome {
    #[serde(skip)]
    pub messages: Vec<Message>,
    pub protocol: u64,
    pub ra_ap: String,
    pub host: String,
    pub workspaces: Vec<ResolvedWorkspace>,
    pub files: u64,
    pub in_crate: u64,
    pub items: u64,
    pub by_state: ResolveByState,
    pub joined: u64,
    pub unmatched: u64,
    pub c_only: u64,
    pub agrees: u64,
    pub disagrees: u64,
    pub collisions: u64,
    /// The binary's wall time.
    pub ms: u64,
    /// The binary's peak resident set, bytes; `None` where unknown.
    pub peak_rss: Option<u64>,
}

/// One workspace of the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedWorkspace {
    /// As the reply gave it (`..` included).
    pub dir: String,
    pub toolchain: Option<String>,
    pub sysroot_packages: u64,
    pub metadata_degraded: bool,
    pub discover_ms: u64,
    pub database_ms: u64,
}

/// The items per state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ResolveByState {
    pub moniker: u64,
    pub local: u64,
    pub none: u64,
    pub unresolved: u64,
    pub not_loaded: u64,
}

/// The layer C snapshot against the tree as it is now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerCState {
    /// No snapshot.
    Absent,
    /// Its fingerprint equals the tree's.
    Fresh,
    /// Anything it covers changed: every row stale, its values kept.
    Stale,
}

impl LayerCState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Fresh => "fresh",
            Self::Stale => "stale",
        }
    }
}

/// Exit 2 after step 1: `spec resolve: <reason>; the previous snapshot is
/// kept`.
fn kept(reason: &str) -> CliError {
    CliError::cannot(format!("spec resolve: {reason}; {KEPT}"))
}

/// `spec resolve` (see the module documentation).
pub fn resolve(
    env: &Env,
    globals: &Globals,
    request: &ResolveRequest,
) -> Result<ResolveOutcome, CliError> {
    if !(RESOLVE_TIMEOUT_MIN..=RESOLVE_TIMEOUT_MAX).contains(&request.timeout) {
        return Err(CliError::spec(format!(
            "--timeout {} is out of range: {RESOLVE_TIMEOUT_MIN} to {RESOLVE_TIMEOUT_MAX} seconds",
            request.timeout
        )));
    }
    #[cfg(unix)]
    {
        resolve_unix(env, globals, request)
    }
    #[cfg(not(unix))]
    {
        let _ = (env, globals);
        Err(CliError::spec("spec resolve runs on Unix only"))
    }
}

/// The code tree as layer C sees it: the code index's rows after a
/// refresh, the requested files among them and the fingerprint's tree
/// entries.
struct Tree {
    rows: CodeTreeRows,
    /// Indices into `rows.files`, in the walk's byte order.
    requested: Vec<usize>,
    /// Byte-sorted; the stamps aside.
    entries: Vec<String>,
}

impl Tree {
    /// The refreshed code index's rows of `index`, the files under `root`.
    fn read(index: &SqliteIndex, root: &Path) -> Result<Self, CliError> {
        let rows = index
            .code_tree_rows()
            .map_err(store_error)?
            .unwrap_or_default();
        let requested: Vec<usize> = rows
            .files
            .iter()
            .enumerate()
            .filter(|(_, file)| is_requested(file))
            .map(|(at, _)| at)
            .collect();
        let entries = tree_entries(root, &rows, &requested);
        Ok(Self {
            rows,
            requested,
            entries,
        })
    }

    fn requested(&self) -> impl Iterator<Item = &CodeTreeFile> {
        self.requested.iter().map(|&at| &self.rows.files[at])
    }
}

/// A walked Rust file whose content is UTF-8 (read, hashed).
fn is_requested(file: &CodeTreeFile) -> bool {
    file.lang == LANG_RUST && file.utf8 && file.read_error.is_none() && file.blake3.is_some()
}

/// One fingerprint entry: a JSON array.
fn entry(value: &impl Serialize) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// The fingerprint's entries but the stamps, byte-sorted.
fn tree_entries(root: &Path, rows: &CodeTreeRows, requested: &[usize]) -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();
    for &at in requested {
        let file = &rows.files[at];
        entries.push(entry(&(ENTRY_FILE, &file.path, &file.blake3)));
    }
    for package in &rows.packages {
        entries.push(entry(&(ENTRY_PACKAGE, &package.dir, &package.manifest_b3)));
    }
    for name in tree_files(rows.files.iter().map(|file| file.path.as_str())) {
        let path = root.join(&name);
        match fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() => match fs::read(&path) {
                Ok(bytes) => entries.push(entry(&(ENTRY_BYTES, &name, b3_hash(&bytes)))),
                Err(_) => entries.push(entry(&(ENTRY_UNREADABLE, &name))),
            },
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => entries.push(entry(&(ENTRY_UNREADABLE, &name))),
        }
    }
    entries.sort();
    entries.dedup();
    entries
}

/// `tree_fp`: `b3:` of the entries and the stamps, byte-sorted, one per
/// line.
fn tree_fp(protocol: u64, ra_ap: &str, host: &str, entries: &[String]) -> String {
    let mut all: Vec<String> = entries.to_vec();
    all.push(entry(&(ENTRY_PROTOCOL, protocol)));
    all.push(entry(&(ENTRY_RA_AP, ra_ap)));
    all.push(entry(&(ENTRY_HOST, host)));
    all.sort();
    b3_hash(all.join("\n").as_bytes())
}

/// The spec index and the code index of `index`'s worktree refreshed as
/// the reads refresh them, the walk's notes in `messages`; `Ok(Err(_))`
/// when the walk has a gap (`spec: the code walk is incomplete: …`).
fn refreshed_tree(
    index: &mut SqliteIndex,
    project: &ProjectRoot,
    code: &specengine_core::CodeConfig,
    messages: &mut Vec<Message>,
) -> Result<Result<Tree, CliError>, CliError> {
    indexed_in(index, project, messages, false)?;
    let refreshed = refresh_code(
        index,
        &project.root,
        code,
        &project.config_label,
        &project.config.scheme,
        false,
    )?;
    if let Err(gap) = refreshed.require_complete() {
        return Ok(Err(gap));
    }
    messages.extend(refreshed.messages.iter().cloned());
    Tree::read(index, &project.root).map(Ok)
}

/// The snapshot's freshness (see the module documentation), the code
/// index refreshed first as the reads refresh it: exit 2 as they do (no
/// `[code]`, a config error, an incomplete walk). Never spawns anything.
pub fn layer_c_state(env: &Env, globals: &Globals) -> Result<LayerCState, CliError> {
    let (project, text) = discover_with_text(env, globals)?;
    project.slug()?;
    let code = code_config(&project, &text)?;
    let mut open = open_index(env, &project)?;
    let mut messages = Vec::new();
    refreshed_tree(&mut open.index, &project, &code, &mut messages)??;
    state_in(&open.index, &project.root)
}

/// [`layer_c_state`] over a code index the caller refreshed.
pub(crate) fn state_in(index: &SqliteIndex, root: &Path) -> Result<LayerCState, CliError> {
    let Some(stamp) = index.layer_c_stamp().map_err(store_error)? else {
        return Ok(LayerCState::Absent);
    };
    let tree = Tree::read(index, root)?;
    let now = tree_fp(
        u64::from(stamp.protocol),
        &stamp.ra_ap,
        &stamp.host,
        &tree.entries,
    );
    Ok(if now == stamp.tree_fp {
        LayerCState::Fresh
    } else {
        LayerCState::Stale
    })
}

#[cfg(unix)]
fn resolve_unix(
    env: &Env,
    globals: &Globals,
    request: &ResolveRequest,
) -> Result<ResolveOutcome, CliError> {
    use crate::layer_c_reply::PROTOCOL;
    use crate::layer_c_run::{self, Run};
    use crate::serve::find_beside_or_on_path;

    // Step 1.
    let (project, text) = discover_with_text(env, globals)?;
    let slug = project.slug()?.to_owned();
    let code = code_config(&project, &text)?;
    let mut messages = Vec::new();
    let mut open = open_index(env, &project)?;
    let before = refreshed_tree(&mut open.index, &project, &code, &mut messages)??;
    if before.requested.is_empty() {
        return Err(kept("no Rust file to resolve"));
    }

    // Step 2.
    let data = prepared_data_dir(env, &project)?;
    let layer_dir = data.join(format!("{slug}{LAYER_DIR_SUFFIX}"));
    for dir in [
        layer_dir.clone(),
        layer_dir.join(TARGET_DIR),
        layer_dir.join(TMP_DIR),
    ] {
        fs::create_dir_all(&dir)
            .map_err(|error| kept(&format!("cannot create {}: {error}", dir.display())))?;
    }
    let lock_path = layer_dir.join(LOCK_FILE);
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| kept(&format!("cannot open {}: {error}", lock_path.display())))?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            return Err(kept(&format!("a layer C run of {slug} is in progress")));
        }
        Err(TryLockError::Error(error)) => {
            return Err(kept(&format!(
                "cannot lock {}: {error}",
                lock_path.display()
            )));
        }
    }
    let dir = fs::canonicalize(&layer_dir)
        .map_err(|error| kept(&format!("cannot resolve {}: {error}", layer_dir.display())))?;
    let target_dir = dir.join(TARGET_DIR);
    let tmp_dir = dir.join(TMP_DIR);

    // Step 3.
    let spec_path = request.spec_path.as_deref();
    let program = find_beside_or_on_path(LAYER_C_BIN, spec_path, request.path.as_deref())
        .ok_or_else(|| kept(&not_found(spec_path)))?;

    // Step 4.
    let requested: Vec<String> = before.requested().map(|file| file.path.clone()).collect();
    let line = request_line(
        &project.root,
        &requested,
        &target_dir,
        &tmp_dir,
        request.timeout,
    )?;
    let reply = layer_c_run::run(Run {
        program: &program,
        dir: &tmp_dir,
        request: line,
        requested,
        seconds: request.timeout,
    })
    .map_err(|reason| kept(&reason))?;

    // Step 6.
    let mut after_notes = Vec::new();
    let unchanged = refreshed_tree(&mut open.index, &project, &code, &mut after_notes)?
        .is_ok_and(|after| after.entries == before.entries);
    if !unchanged {
        return Err(kept("the tree changed during the run"));
    }
    let (ra_ap, host) = reply.hello.clone().unwrap_or_default();
    let snapshot = snapshot_of(&reply, &before, PROTOCOL, &ra_ap, &host);
    open.index
        .replace_layer_c(&snapshot)
        .map_err(|error| kept(&format!("cannot store the snapshot: {error}")))?;
    drop(lock);

    let counts = join_counts(&open.index, &snapshot, &before.rows)?;
    Ok(outcome_of(
        &reply, &snapshot, counts, messages, &ra_ap, &host,
    ))
}

/// The missing binary's reason, its install line.
fn not_found(spec_path: Option<&Path>) -> String {
    let spec = spec_path.map_or_else(|| "spec".to_owned(), |path| path.display().to_string());
    format!(
        "{LAYER_C_BIN} not found beside {spec} or on PATH; install it: cargo install --locked \
         --force --path <clone>/crates/{LAYER_C_BIN} (rustc >= 1.98, rust-src)"
    )
}

/// Protocol v1's request: exactly its six keys, in order.
#[derive(Serialize)]
struct RequestLine<'a> {
    protocol: u64,
    root: &'a str,
    files: &'a [String],
    target_dir: &'a str,
    tmp_dir: &'a str,
    budget_ms: u64,
}

/// The request line, its line end included; a path that is not UTF-8
/// cannot be written in it.
#[cfg(unix)]
fn request_line(
    root: &Path,
    files: &[String],
    target_dir: &Path,
    tmp_dir: &Path,
    seconds: u64,
) -> Result<String, CliError> {
    let line = RequestLine {
        protocol: crate::layer_c_reply::PROTOCOL,
        root: utf8(root)?,
        files,
        target_dir: utf8(target_dir)?,
        tmp_dir: utf8(tmp_dir)?,
        budget_ms: seconds.saturating_mul(1000),
    };
    let mut text = serde_json::to_string(&line)
        .map_err(|error| kept(&format!("cannot write the request: {error}")))?;
    text.push('\n');
    Ok(text)
}

/// `path` as the request writes it: UTF-8 only.
#[cfg(unix)]
fn utf8(path: &Path) -> Result<&str, CliError> {
    path.to_str().ok_or_else(|| {
        kept(&format!(
            "{} is not UTF-8: the request names UTF-8 paths only",
            path.display()
        ))
    })
}

/// The rows a complete reply stores.
#[cfg(unix)]
fn snapshot_of(
    reply: &crate::layer_c_reply::Reply,
    tree: &Tree,
    protocol: u64,
    ra_ap: &str,
    host: &str,
) -> LayerCSnapshot {
    let toolchains: BTreeSet<String> = reply
        .workspaces
        .iter()
        .filter_map(|workspace| workspace.toolchain.clone())
        .collect();
    let hashes: BTreeMap<&str, Option<&String>> = tree
        .requested()
        .map(|file| (file.path.as_str(), file.blake3.as_ref()))
        .collect();
    let mut files: Vec<LayerCFile> = reply
        .files
        .iter()
        .map(|file| LayerCFile {
            path: file.path.clone(),
            blake3: hashes.get(file.path.as_str()).copied().flatten().cloned(),
            in_crate: file.in_crate,
        })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let mut items: Vec<LayerCItem> = reply
        .files
        .iter()
        .flat_map(|file| {
            file.items.iter().map(|item| LayerCItem {
                path: file.path.clone(),
                start: item.start,
                end: item.end,
                kind: item.kind.clone(),
                state: item.state.clone(),
                moniker: item.moniker.clone(),
            })
        })
        .collect();
    items.sort_by(|a, b| (&a.path, a.start).cmp(&(&b.path, b.start)));
    LayerCSnapshot {
        tree_fp: tree_fp(protocol, ra_ap, host, &tree.entries),
        protocol: u32::try_from(protocol).unwrap_or(u32::MAX),
        ra_ap: ra_ap.to_owned(),
        host: host.to_owned(),
        toolchains: toolchains.into_iter().collect(),
        degraded: reply
            .workspaces
            .iter()
            .any(|workspace| workspace.metadata_degraded),
        workspaces: u32::try_from(reply.workspaces.len()).unwrap_or(u32::MAX),
        files,
        items,
    }
}

/// The join's figures over one snapshot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct JoinCounts {
    joined: u64,
    unmatched: u64,
    c_only: u64,
    agrees: u64,
    disagrees: u64,
    collisions: u64,
}

/// Every layer A row of the snapshot's files joined with its rows: the
/// counts `spec resolve` prints (the code crate's join, agreement and
/// collisions over the code index's symbols and package tables).
fn join_counts(
    index: &SqliteIndex,
    snapshot: &LayerCSnapshot,
    rows: &CodeTreeRows,
) -> Result<JoinCounts, CliError> {
    let symbols = index
        .code_snapshot(SnapshotSymbols::All)
        .map_err(store_error)?
        .map(|read| read.symbols)
        .unwrap_or_default();
    let mut by_path: BTreeMap<&str, Vec<&StoredSymbol>> = BTreeMap::new();
    for symbol in &symbols {
        by_path
            .entry(symbol.path.as_str())
            .or_default()
            .push(symbol);
    }
    let mut items: BTreeMap<&str, Vec<&LayerCItem>> = BTreeMap::new();
    for item in &snapshot.items {
        items.entry(item.path.as_str()).or_default().push(item);
    }
    let packages: BTreeMap<&str, _> = rows
        .packages
        .iter()
        .map(|row| (row.dir.as_str(), package_targets(row)))
        .collect();
    let file_packages: BTreeMap<&str, Option<&str>> = rows
        .files
        .iter()
        .map(|file| (file.path.as_str(), file.package.as_deref()))
        .collect();

    let mut counts = JoinCounts::default();
    for file in &snapshot.files {
        let path = file.path.as_str();
        let a: &[&StoredSymbol] = by_path.get(path).map_or(&[], Vec::as_slice);
        let a_by_ord: BTreeMap<u32, &StoredSymbol> =
            a.iter().map(|symbol| (symbol.ord, *symbol)).collect();
        let c: &[&LayerCItem] = items.get(path).map_or(&[], Vec::as_slice);
        let a_items: Vec<JoinItem<'_>> = a
            .iter()
            .map(|symbol| JoinItem {
                start: u64::from(symbol.start_byte),
                end: u64::from(symbol.end_byte),
                kind: &symbol.item_kind,
            })
            .collect();
        let c_items: Vec<JoinItem<'_>> = c
            .iter()
            .map(|item| JoinItem {
                start: item.start,
                end: item.end,
                kind: &item.kind,
            })
            .collect();
        let joins = join_file(&a_items, &c_items);
        let crate_of_file: Option<TargetCrate> =
            file_packages.get(path).copied().flatten().and_then(|dir| {
                let targets = packages.get(dir)?.as_ref()?;
                let relative = Path::new(path).strip_prefix(dir).ok()?;
                target_crate(relative, targets)
            });
        let mut joined_c: BTreeSet<usize> = BTreeSet::new();
        for (at, join) in joins.iter().enumerate() {
            let Some(c_at) = *join else {
                counts.unmatched += 1;
                continue;
            };
            counts.joined += 1;
            joined_c.insert(c_at);
            let Some(moniker) = c[c_at].moniker.as_deref() else {
                continue;
            };
            let symbol = a[at];
            let untargeted = matches!(
                symbol.ambiguity.as_deref(),
                Some(AMBIGUITY_PATH_ATTRIBUTE | AMBIGUITY_UNROOTED)
            );
            let Some(target) = crate_of_file.as_ref().filter(|_| !untargeted) else {
                continue;
            };
            let mut modules = target.modules.clone();
            modules.extend(inline_modules(&a_by_ord, symbol));
            if agrees(moniker, &target.name, &modules) {
                counts.agrees += 1;
            } else {
                counts.disagrees += 1;
            }
        }
        counts.c_only += count(c.len() - joined_c.len());
        let monikers: Vec<Option<&str>> = c.iter().map(|item| item.moniker.as_deref()).collect();
        counts.collisions += count(collision_groups(&monikers).len());
    }
    Ok(counts)
}

/// The names of the inline modules enclosing `symbol` in its file
/// (`by_ord`: the file's symbols by `ord`), outermost first.
fn inline_modules(by_ord: &BTreeMap<u32, &StoredSymbol>, symbol: &StoredSymbol) -> Vec<String> {
    let mut names = Vec::new();
    let mut parent = symbol.parent_ord;
    // A chain longer than the file's items is a cycle: stop.
    for _ in 0..by_ord.len() {
        let Some(ord) = parent else {
            break;
        };
        let Some(enclosing) = by_ord.get(&ord) else {
            break;
        };
        if enclosing.item_kind == MOD_KIND {
            names.push(enclosing.name.clone());
        }
        parent = enclosing.parent_ord;
    }
    names.reverse();
    names
}

/// A length as a count.
fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

/// The outcome of a stored run.
#[cfg(unix)]
fn outcome_of(
    reply: &crate::layer_c_reply::Reply,
    snapshot: &LayerCSnapshot,
    counts: JoinCounts,
    messages: Vec<Message>,
    ra_ap: &str,
    host: &str,
) -> ResolveOutcome {
    use crate::layer_c_reply::{Last, PROTOCOL};

    let tallies = reply.tallies();
    let (ms, peak_rss) = match &reply.last {
        Some(Last::Done(done)) => (done.ms, done.peak_rss),
        _ => (0, None),
    };
    ResolveOutcome {
        messages,
        protocol: PROTOCOL,
        ra_ap: ra_ap.to_owned(),
        host: host.to_owned(),
        workspaces: reply
            .workspaces
            .iter()
            .map(|workspace| ResolvedWorkspace {
                dir: workspace.dir.clone(),
                toolchain: workspace.toolchain.clone(),
                sysroot_packages: workspace.sysroot_packages,
                metadata_degraded: workspace.metadata_degraded,
                discover_ms: workspace.discover_ms,
                database_ms: workspace.database_ms.unwrap_or(0),
            })
            .collect(),
        files: count(snapshot.files.len()),
        in_crate: count(snapshot.files.iter().filter(|file| file.in_crate).count()),
        items: count(snapshot.items.len()),
        by_state: ResolveByState {
            moniker: tallies[0],
            local: tallies[1],
            none: tallies[2],
            unresolved: tallies[3],
            not_loaded: tallies[4],
        },
        joined: counts.joined,
        unmatched: counts.unmatched,
        c_only: counts.c_only,
        agrees: counts.agrees,
        disagrees: counts.disagrees,
        collisions: counts.collisions,
        ms,
        peak_rss,
    }
}

/// One decimal of `value`.
fn tenths(value: f64) -> String {
    format!("{value:.1}")
}

/// `spec resolve`'s stdout: the stamps, a line per workspace, the
/// counts, the join's figures with the timings.
pub(crate) fn render_text(outcome: &ResolveOutcome) -> String {
    let mut out = format!(
        "layer c: {LAYER_C_BIN} protocol {}, ra_ap {}, host {}\n",
        outcome.protocol,
        escape_controls(&outcome.ra_ap),
        escape_controls(&outcome.host)
    );
    for (at, workspace) in outcome.workspaces.iter().enumerate() {
        let toolchain = workspace
            .toolchain
            .as_deref()
            .map_or_else(|| "unknown".to_owned(), escape_controls);
        let degraded = if workspace.metadata_degraded {
            "yes"
        } else {
            "no"
        };
        out.push_str(&format!(
            "workspace {} {}: discover {} ms (toolchain {toolchain}, sysroot {}, degraded \
             {degraded}), database {} ms\n",
            at + 1,
            escape_controls(&workspace.dir),
            workspace.discover_ms,
            workspace.sysroot_packages,
            workspace.database_ms
        ));
    }
    let states = &outcome.by_state;
    out.push_str(&format!(
        "files {} (in crate {}), items {}: moniker {}, local {}, none {}, unresolved {}, \
         not_loaded {}\n",
        outcome.files,
        outcome.in_crate,
        outcome.items,
        states.moniker,
        states.local,
        states.none,
        states.unresolved,
        states.not_loaded
    ));
    #[allow(clippy::cast_precision_loss)]
    let seconds = tenths(outcome.ms as f64 / MS_PER_S);
    #[allow(clippy::cast_precision_loss)]
    let peak = outcome.peak_rss.map_or_else(
        || "unknown".to_owned(),
        |bytes| format!("{} GiB", tenths(bytes as f64 / GIB)),
    );
    out.push_str(&format!(
        "joined {}, unmatched {}, c_only {}; agrees {}, disagrees {}; collisions {}; stored {}, \
         {seconds} s, peak {peak}\n",
        outcome.joined,
        outcome.unmatched,
        outcome.c_only,
        outcome.agrees,
        outcome.disagrees,
        outcome.collisions,
        LayerCState::Fresh.as_str()
    ));
    out
}
