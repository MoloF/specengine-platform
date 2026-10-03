//! Measurement `index`: the spec index of `specengine-store` over a corpus
//! (`crates/specengine-eval/README.md`, "CLI contract").
//!
//! The walked files are copied under `--out/index/<label>/corpus`, and the
//! copy is indexed into `--out/index/<label>/index.db`: a full index, an
//! unchanged update, an update after appending a line to the first file,
//! and the same edit again through `update_paths`. The corpus is only read.
//! The ID scheme and the walk come from `--scheme` (`[ids]` and `[paths]`;
//! default: `SPECENGINE_SCHEME_A` / `_B` for `--label pilot-a` / `pilot-b`
//! when set, else `specengine.toml` at the corpus root); a missing or
//! invalid one refuses the run (exit 2, `file:line: message`).
//!
//! stdout carries counts and times only; the update reports (which name
//! roots and directories) go to `--out` only.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;
use specengine_core::{IdSchemeToml, Paths};
use specengine_model::IdScheme;
use specengine_store::{IndexWriter, Source, SpecIndex, SqliteIndex, UpdateReport, WorkingTree};

use crate::harness::{self, Corpus};

/// Fixture directory (relative to `fixtures/`) used without `--pilot`.
pub const FIXTURE: &str = "spec-b";

/// Scheme file looked up at the corpus root when `--scheme` is absent.
pub const DEFAULT_SCHEME: &str = "specengine.toml";

/// The project name of the scratch index.
const PROJECT: &str = "specengine-eval";

/// The line appended to the first file for the one-file updates; no ID.
const APPENDED: &str = "\nA line appended by the index measurement.\n";

/// What the run needs, read before anything is written.
pub struct Setup {
    scheme: IdScheme,
    paths: Paths,
}

/// Reads `[ids]` and `[paths]` of the scheme file; any error refuses the run.
pub fn prepare(root: &Path, scheme: Option<&Path>, label: Option<&str>) -> Result<Setup, String> {
    let path = harness::resolve_file(
        "--scheme",
        scheme,
        harness::PILOT_SCHEME,
        label,
        root,
        DEFAULT_SCHEME,
    )?;
    let shown = path.display().to_string();
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("{shown}: cannot read the scheme: {error}"))?;
    let scheme = IdScheme::from_toml(&text).map_err(|error| error.at(&shown))?;
    let paths = Paths::from_toml(&text).map_err(|error| error.at(&shown))?;
    Ok(Setup { scheme, paths })
}

/// The `result` object.
#[derive(Serialize)]
pub struct IndexResult {
    /// Files in the index after the full index.
    pub files: usize,
    pub nodes: usize,
    pub links: usize,
    pub diagnostics: usize,
    /// Corpus files left out of the copy as unreadable, plus files the full
    /// index stored with a `read_error` (a parser panic).
    pub unreadable: usize,
    /// Configured roots that name no directory and no `.md` file.
    pub missing_roots: usize,
    /// The first `update` of an empty index.
    pub full_ms: u128,
    /// An `update` with nothing changed.
    pub noop_ms: u128,
    pub noop_parsed: usize,
    /// An `update` after appending a line to the first file; `null` without
    /// files.
    pub one_file_ms: Option<u128>,
    pub one_file_parsed: usize,
    /// `update_paths` of that file after appending another line.
    pub one_path_ms: Option<u128>,
}

#[derive(Serialize)]
struct Reports<'a> {
    full: &'a UpdateReport,
    noop: &'a UpdateReport,
    one_file: Option<&'a UpdateReport>,
    one_path: Option<&'a UpdateReport>,
}

pub fn run(corpus: &Corpus, setup: Setup) -> Result<IndexResult, String> {
    let Setup { scheme, paths } = setup;
    let out_dir = corpus.out.join("index").join(&corpus.label);
    let copy = out_dir.join("corpus");
    let db = out_dir.join("index.db");
    clear_scratch(&copy, &db)?;
    fs::create_dir_all(&copy)
        .map_err(|error| format!("cannot create {}: {error}", copy.display()))?;

    let tree = WorkingTree::new(&corpus.root, &paths).map_err(|error| error.to_string())?;
    let listing = tree
        .list()
        .map_err(|error| format!("cannot list the corpus: {error}"))?;
    let mut left_out = 0;
    for path in &listing.paths {
        // An unreadable file is left out of the copy (the index of the copy
        // cannot see it either way) and counted as `unreadable`.
        let Ok(bytes) = tree.read(path) else {
            left_out += 1;
            continue;
        };
        let target = copy.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
        fs::write(&target, bytes)
            .map_err(|error| format!("cannot write {}: {error}", target.display()))?;
    }

    let scratch = WorkingTree::new(&copy, &paths).map_err(|error| error.to_string())?;
    let mut index = SqliteIndex::open(&db, PROJECT, &copy).map_err(|error| error.to_string())?;

    let started = Instant::now();
    let full = index
        .update(&scratch, &scheme)
        .map_err(|error| error.to_string())?;
    let full_ms = started.elapsed().as_millis();

    let started = Instant::now();
    let noop = index
        .update(&scratch, &scheme)
        .map_err(|error| error.to_string())?;
    let noop_ms = started.elapsed().as_millis();

    let mut result = IndexResult {
        files: 0,
        nodes: 0,
        links: 0,
        diagnostics: 0,
        unreadable: left_out + full.unreadable,
        missing_roots: listing.missing_roots.len(),
        full_ms,
        noop_ms,
        noop_parsed: noop.parsed,
        one_file_ms: None,
        one_file_parsed: 0,
        one_path_ms: None,
    };
    let stored = index.files().map_err(|error| error.to_string())?;
    result.files = stored.len();
    for path in &stored {
        if let Some(parsed) = index
            .file(path)
            .map_err(|error| error.to_string())?
            .and_then(|file| file.parsed)
        {
            result.nodes += parsed.nodes.len();
            result.links += parsed.links.len();
            result.diagnostics += parsed.diagnostics.len();
        }
    }

    let mut one_file = None;
    let mut one_path = None;
    if let Some(first) = stored.first() {
        append_line(&copy.join(first))?;
        let started = Instant::now();
        let report = index
            .update(&scratch, &scheme)
            .map_err(|error| error.to_string())?;
        result.one_file_ms = Some(started.elapsed().as_millis());
        result.one_file_parsed = report.parsed;
        one_file = Some(report);

        append_line(&copy.join(first))?;
        let started = Instant::now();
        let report = index
            .update_paths(&scratch, &scheme, &[first.as_str()])
            .map_err(|error| error.to_string())?;
        result.one_path_ms = Some(started.elapsed().as_millis());
        one_path = Some(report);
    }

    let reports = Reports {
        full: &full,
        noop: &noop,
        one_file: one_file.as_ref(),
        one_path: one_path.as_ref(),
    };
    let detail = out_dir.join("reports.json");
    let json = serde_json::to_string_pretty(&reports).map_err(|error| error.to_string())?;
    fs::write(&detail, json)
        .map_err(|error| format!("cannot write {}: {error}", detail.display()))?;
    eprintln!(
        "index: {} files, {} nodes; full {} ms, no-op {} ms, one file {} ms, one path {} ms; detail in {}",
        result.files,
        result.nodes,
        result.full_ms,
        result.noop_ms,
        result
            .one_file_ms
            .map_or("-".to_owned(), |ms| ms.to_string()),
        result
            .one_path_ms
            .map_or("-".to_owned(), |ms| ms.to_string()),
        out_dir.display()
    );
    Ok(result)
}

/// Removes the copy and the DB of an earlier run (both are this
/// measurement's own scratch under `--out`).
fn clear_scratch(copy: &Path, db: &Path) -> Result<(), String> {
    if copy.exists() {
        fs::remove_dir_all(copy)
            .map_err(|error| format!("cannot clear {}: {error}", copy.display()))?;
    }
    for suffix in ["", "-wal", "-shm"] {
        let mut name = db.as_os_str().to_owned();
        name.push(suffix);
        let file = PathBuf::from(name);
        if file.exists() {
            fs::remove_file(&file)
                .map_err(|error| format!("cannot clear {}: {error}", file.display()))?;
        }
    }
    Ok(())
}

fn append_line(path: &Path) -> Result<(), String> {
    fs::OpenOptions::new()
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(APPENDED.as_bytes()))
        .map_err(|error| format!("cannot append to {}: {error}", path.display()))
}
