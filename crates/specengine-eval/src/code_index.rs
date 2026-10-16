//! Measurement `code-index`: the code index of the CLI library
//! (`specengine_cli::refresh_code`, the store's code tables) over a corpus
//! (`docs/canon/symbol-index.md` "AC-10 measurement"; symbol-index RD-12).
//!
//! The `[code]` walk's files, and every `Cargo.toml` on the way from one up
//! to the corpus root, are copied under `--out/code-index/<label>/corpus`;
//! the copy is indexed into `--out/code-index/<label>/index.db`: a full
//! refresh of an empty code index, an unchanged one, and one after a
//! comment line is appended to the largest `.rs` file (by bytes, the first
//! by path among equals; never simply the first file). `hash_ms` is the
//! parse, the item list, the levels and the display signatures of every
//! `.rs` file, no database. The counts come from the index after the
//! refreshes: `items` its symbols; `twins` its in-file twin groups by rule
//! (a discriminator of digits: the ordinal rule, else the `cfg` rule) and
//! its `duplicate` items; `signatures` the items with one and those cut;
//! `roles` each Rust file's role under the layout's target table against
//! `cargo metadata`'s (`$SPECENGINE_CARGO` or `cargo`, as `ast-hash` runs
//! it: `ast_hash::targets`), `differ` `null` when no package answered.
//!
//! `[ids]`, `[paths]` and `[code]` come from `--scheme` (default:
//! `SPECENGINE_SCHEME_A` / `_B` under `--label pilot-a` / `pilot-b`, else
//! `specengine.toml` at the corpus root); under a pilot label
//! `SPECENGINE_CODE_ROOTS_A` / `_B` (comma-separated) give the roots
//! instead. No `[code]` and no roots: refused (exit 2). The corpus is only
//! read; stdout carries counts and times, the refresh reports go to
//! `--out` only.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use specengine_code::qpath::{self, PackageTargets, TargetSource};
use specengine_code::{
    DISCRIMINATOR_SEPARATOR, RustParser, bind_tree, item_levels, item_signatures,
};
use specengine_core::{CodeConfig, IdSchemeToml, Paths};
use specengine_model::IdScheme;
use specengine_store::{CodeReport, IndexWriter, SqliteIndex, StoredSymbol, WorkingTree};

use crate::ast_hash::targets;
use crate::harness::{self, Corpus};

/// Fixture directory (relative to `fixtures/`) used without `--pilot`.
pub const FIXTURE: &str = "code-index";

/// The measurement's name, and its directory under `--out`.
pub const MEASUREMENT: &str = "code-index";

/// Scheme file looked up at the corpus root when `--scheme` is absent.
const DEFAULT_SCHEME: &str = "specengine.toml";

/// The project name of the scratch index.
const PROJECT: &str = "specengine-eval";

/// The line appended to the largest `.rs` file for the one-file refresh: a
/// comment, so the counts read after it are the corpus's.
const APPENDED: &str = "// A line appended by the code-index measurement.\n";

/// The manifest every package directory holds.
const MANIFEST: &str = "Cargo.toml";

/// The end of a digest-cut display signature.
const CUT: char = '\u{2026}';

/// What the run needs, read before anything is written.
pub struct Setup {
    scheme: IdScheme,
    paths: Paths,
    code: CodeConfig,
    /// The end of `--timeout` (the cargo calls end before it).
    run_end: Option<Instant>,
}

/// Reads the scheme file (`[ids]`, `[paths]`, `[code]`) and, under a pilot
/// label, the code roots; any error refuses the run.
pub fn prepare(
    root: &Path,
    scheme: Option<&Path>,
    label: Option<&str>,
    timeout: Duration,
) -> Result<Setup, String> {
    let run_end = Instant::now().checked_add(timeout);
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
    let ids = IdScheme::from_toml(&text).map_err(|error| error.at(&shown))?;
    let paths = Paths::from_toml(&text).map_err(|error| error.at(&shown))?;
    let code = match harness::text_from_environment(harness::PILOT_CODE_ROOTS, label) {
        Some(roots) => {
            let list: Vec<String> = roots
                .split(',')
                .map(|root| format!("{:?}", root.trim()))
                .collect();
            let table = format!("[code]\nroots = [{}]\n", list.join(", "));
            CodeConfig::from_toml(&table)
                .map_err(|error| error.at("the code roots variable"))?
                .ok_or_else(|| "the code roots variable gives no [code] table".to_owned())?
        }
        None => CodeConfig::from_toml(&text)
            .map_err(|error| error.at(&shown))?
            .ok_or_else(|| format!("{shown} has no [code] table and no code roots are set"))?,
    };
    Ok(Setup {
        scheme: ids,
        paths,
        code,
        run_end,
    })
}

/// In-file twins of one rule: the groups and their members.
#[derive(Debug, Default, Serialize)]
pub struct TwinCount {
    pub groups: usize,
    pub items: usize,
}

/// The twins the index holds.
#[derive(Debug, Default, Serialize)]
pub struct Twins {
    /// Named apart by their `cfg` chains.
    pub cfg: TwinCount,
    /// Named apart by their ordinals.
    pub ordinal: TwinCount,
    /// Items another file renders too (`duplicate`).
    pub cross_file: usize,
}

/// The display signatures the index holds.
#[derive(Debug, Default, Serialize)]
pub struct Signatures {
    /// Items with a signature.
    pub items: usize,
    /// Those cut at the cap.
    pub cut: usize,
}

/// Each Rust file's role under the layout against `cargo metadata`.
#[derive(Debug, Default, Serialize)]
pub struct Roles {
    /// Rust files in a package.
    pub files: usize,
    /// Those whose package `cargo metadata` answered for.
    pub metadata: usize,
    /// Of those, the files whose role differs; `null` when none answered.
    pub differ: Option<usize>,
}

/// The `result` object.
#[derive(Debug, Serialize)]
pub struct CodeIndexResult {
    /// `.rs` newlines / 1000.
    pub kloc: f64,
    pub rs_files: usize,
    pub ron_files: usize,
    /// The bytes of the `.rs` and `.ron` files.
    pub bytes: usize,
    /// Stored symbols.
    pub items: usize,
    /// Parse, item list, levels and signatures of every `.rs` file.
    pub hash_ms: u128,
    /// The first refresh of an empty code index.
    pub full_ms: u128,
    /// A refresh with nothing changed.
    pub noop_ms: u128,
    pub noop_parsed: usize,
    /// A refresh after the largest `.rs` file grew one line; `null` without
    /// a `.rs` file.
    pub one_file_ms: Option<u128>,
    pub one_file_parsed: usize,
    /// The database and its WAL after the refreshes.
    pub db_bytes: u64,
    /// One read of every stored file, marker and mention (`spec refs`'s).
    pub refs_ms: u128,
    pub twins: Twins,
    pub signatures: Signatures,
    pub roles: Roles,
}

#[derive(Serialize)]
struct Reports<'a> {
    full: &'a CodeReport,
    noop: &'a CodeReport,
    one_file: Option<&'a CodeReport>,
}

pub fn run(corpus: &Corpus, setup: Setup) -> Result<CodeIndexResult, String> {
    let Setup {
        scheme,
        paths,
        code,
        run_end,
    } = setup;
    let out_dir = corpus.out.join(MEASUREMENT).join(&corpus.label);
    let copy = out_dir.join("corpus");
    let db = out_dir.join("index.db");
    clear_scratch(&copy, &db)?;
    fs::create_dir_all(&copy)
        .map_err(|error| format!("cannot create {}: {error}", copy.display()))?;

    // The copy: every listed file, every manifest on the way up from one.
    let listed = specengine_cli::listed_code_files(&corpus.root, &code);
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for path in &listed {
        let Ok(bytes) = fs::read(corpus.root.join(path)) else {
            continue;
        };
        write_file(&copy, path, &bytes)?;
        files.insert(path.clone(), bytes);
        let mut dir = Path::new(path).parent();
        while let Some(at) = dir {
            let manifest = at.join(MANIFEST);
            let source = corpus.root.join(&manifest);
            let target = copy.join(&manifest);
            if !target.exists()
                && fs::symlink_metadata(&source).is_ok_and(|meta| meta.is_file())
                && let Ok(bytes) = fs::read(&source)
            {
                write_file(&copy, &manifest.to_string_lossy(), &bytes)?;
            }
            dir = at.parent();
        }
    }
    let rust: Vec<(&String, &Vec<u8>)> = files
        .iter()
        .filter(|(path, _)| path.ends_with(".rs"))
        .collect();
    let newlines: usize = rust
        .iter()
        .map(|(_, bytes)| bytes.iter().filter(|&&byte| byte == b'\n').count())
        .sum();
    let rs_files = rust.len();
    let ron_files = files.len() - rs_files;
    let bytes: usize = files.values().map(Vec::len).sum();
    let largest = rust
        .iter()
        .max_by(|a, b| a.1.len().cmp(&b.1.len()).then_with(|| b.0.cmp(a.0)))
        .map(|(path, _)| (*path).clone());

    let hash_ms = hash_ms(&rust)?;

    let mut index = SqliteIndex::open(&db, PROJECT, &copy).map_err(|error| error.to_string())?;
    let tree = WorkingTree::new(&copy, &paths).map_err(|error| error.to_string())?;
    index
        .update(&tree, &scheme)
        .map_err(|error| error.to_string())?;
    let root = index.root().to_path_buf();
    let mut refresh = || {
        let started = Instant::now();
        let refreshed = specengine_cli::refresh_code(&mut index, &root, &code, &scheme, false)
            .map_err(|error| error.message.clone())?;
        Ok::<_, String>((started.elapsed().as_millis(), refreshed.report))
    };
    let (full_ms, full) = refresh()?;
    let (noop_ms, noop) = refresh()?;
    let mut one_file = None;
    let mut one_file_ms = None;
    if let Some(path) = &largest {
        append_line(&copy.join(path))?;
        let (ms, report) = refresh()?;
        one_file_ms = Some(ms);
        one_file = Some(report);
    }

    let started = Instant::now();
    index.code_refs().map_err(|error| error.to_string())?;
    let refs_ms = started.elapsed().as_millis();
    let symbols = index
        .code_symbols()
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    drop(index);
    let db_bytes = [db.clone(), wal_of(&db)]
        .iter()
        .filter_map(|file| fs::metadata(file).ok())
        .map(|meta| meta.len())
        .sum();

    let roles = roles(corpus, &rust, run_end);
    let result = CodeIndexResult {
        kloc: newlines as f64 / 1000.0,
        rs_files,
        ron_files,
        bytes,
        items: symbols.len(),
        hash_ms,
        full_ms,
        noop_ms,
        noop_parsed: noop.parsed,
        one_file_ms,
        one_file_parsed: one_file.as_ref().map_or(0, |report| report.parsed),
        db_bytes,
        refs_ms,
        twins: twins(&symbols),
        signatures: Signatures {
            items: symbols
                .iter()
                .filter(|symbol| symbol.signature.is_some())
                .count(),
            cut: symbols
                .iter()
                .filter(|symbol| {
                    symbol
                        .signature
                        .as_deref()
                        .is_some_and(|text| text.ends_with(CUT))
                })
                .count(),
        },
        roles,
    };

    let reports = Reports {
        full: &full,
        noop: &noop,
        one_file: one_file.as_ref(),
    };
    let detail = out_dir.join("reports.json");
    let json = serde_json::to_string_pretty(&reports).map_err(|error| error.to_string())?;
    fs::write(&detail, json)
        .map_err(|error| format!("cannot write {}: {error}", detail.display()))?;
    eprintln!(
        "code-index: {} .rs, {} .ron, {} items; hash {} ms, full {} ms, no-op {} ms, one file {} ms; detail in {}",
        result.rs_files,
        result.ron_files,
        result.items,
        result.hash_ms,
        result.full_ms,
        result.noop_ms,
        result
            .one_file_ms
            .map_or("-".to_owned(), |ms| ms.to_string()),
        out_dir.display()
    );
    Ok(result)
}

/// Parse, item list, levels and display signatures of every `.rs` file
/// (not UTF-8: skipped), in milliseconds.
fn hash_ms(rust: &[(&String, &Vec<u8>)]) -> Result<u128, String> {
    let mut parser =
        RustParser::new().map_err(|error| format!("the Rust grammar cannot be loaded: {error}"))?;
    let started = Instant::now();
    for (_, bytes) in rust {
        let Ok(text) = std::str::from_utf8(bytes) else {
            continue;
        };
        let Some(tree) = parser.parse(text) else {
            continue;
        };
        let count = bind_tree(&tree, text).analysis.items.len();
        let every: Vec<usize> = (0..count).collect();
        let _ = item_levels(&tree, text, &every);
        let _ = item_signatures(&tree, text, &every);
    }
    Ok(started.elapsed().as_millis())
}

/// The in-file twin groups by rule, and the `duplicate` items.
fn twins(symbols: &[StoredSymbol]) -> Twins {
    let mut groups: BTreeMap<(&str, &str), (bool, usize)> = BTreeMap::new();
    let mut twins = Twins::default();
    for symbol in symbols {
        if symbol.ambiguity.as_deref() == Some(specengine_store::AMBIGUITY_DUPLICATE) {
            twins.cross_file += 1;
        }
        let Some(value) = symbol.discriminator.as_deref() else {
            continue;
        };
        let name = symbol
            .qpath
            .strip_suffix(value)
            .and_then(|rest| rest.strip_suffix(DISCRIMINATOR_SEPARATOR))
            .unwrap_or(&symbol.qpath);
        let ordinal = value.bytes().all(|byte| byte.is_ascii_digit());
        groups
            .entry((symbol.path.as_str(), name))
            .or_insert((ordinal, 0))
            .1 += 1;
    }
    for (ordinal, members) in groups.into_values() {
        let count = if ordinal {
            &mut twins.ordinal
        } else {
            &mut twins.cfg
        };
        count.groups += 1;
        count.items += members;
    }
    twins
}

/// Each Rust file in a package: its role under the layout's table against
/// `cargo metadata`'s, over the corpus itself (read-only).
fn roles(corpus: &Corpus, rust: &[(&String, &Vec<u8>)], run_end: Option<Instant>) -> Roles {
    let root = &corpus.root;
    let is_package = |dir: &Path| {
        fs::symlink_metadata(root.join(dir).join(MANIFEST)).is_ok_and(|meta| meta.is_file())
    };
    let mut packages: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
    for (path, _) in rust {
        let file = Path::new(path.as_str());
        if let Some(dir) = qpath::package_dir(file, is_package) {
            let inner = file.strip_prefix(&dir).unwrap_or(file).to_path_buf();
            packages.entry(dir).or_default().push(inner);
        }
    }
    let metadata = targets::tables(root, &corpus.label, &packages, run_end);
    let mut roles = Roles::default();
    let mut differ = 0;
    for (dir, files) in &packages {
        let layout: PackageTargets = qpath::layout_targets(
            &targets::package_name(root, dir),
            files.iter().map(PathBuf::as_path),
        );
        let answered = metadata
            .get(dir)
            .filter(|table| table.source == TargetSource::Metadata);
        for file in files {
            roles.files += 1;
            let Some(table) = answered else {
                continue;
            };
            roles.metadata += 1;
            if qpath::file_role(file, table) != qpath::file_role(file, &layout) {
                differ += 1;
            }
        }
    }
    roles.differ = (roles.metadata > 0).then_some(differ);
    roles
}

/// `relative` under `root`, its directories created.
fn write_file(root: &Path, relative: &str, bytes: &[u8]) -> Result<(), String> {
    let target = root.join(relative);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    fs::write(&target, bytes).map_err(|error| format!("cannot write {}: {error}", target.display()))
}

/// The WAL file of `db`.
fn wal_of(db: &Path) -> PathBuf {
    let mut name = db.as_os_str().to_owned();
    name.push("-wal");
    PathBuf::from(name)
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
