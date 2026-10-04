//! Measurement `layout` (`docs/features/import-layout.md`): the "before"
//! model of `specengine-import` projected into the target layout and read
//! back through `specengine-core`, every definition compared by identity
//! (path, line, ID), never by hash.
//!
//! One process: the refusals (the census config of `--config` with its
//! `[layout]`, the before scheme of `--scheme`, `--today`, the start rules
//! of `[layout]`, an index output path a tree document takes) and the pure
//! emitter run before anything is written; then the tree is written under
//! `<out>/layout/<label>/tree/` (emptied first) with the emitted
//! `specengine.toml`, the `index = true` entry rendered into it, every
//! record re-read from the written files (the verifier), the before check
//! run on the corpus, the tree checked under `observe`, each finding
//! attributed (source, layout, emitter), the source-caused errors written
//! as the tree's `.spec-debt.toml`, the tree checked again with it under
//! `enforce`, and the tree indexed into `<out>/layout/<label>/index.db`.
//!
//! stdout carries counts only; paths, IDs and texts go only to the detail
//! files beside the tree. Nothing is written under the corpus, `HOME` or
//! the repository.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::ops::RangeInclusive;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use serde::Serialize;
use specengine_core::check::{
    DebtEntry, DocClass, Finding, Mode, Verdict, is_calendar_date, render_index_set,
};
use specengine_core::{DOCUMENT_EXTENSION, is_clean_relative};
use specengine_import::CensusConfig;
use specengine_import::config::CoreKey;
use specengine_import::import::{
    self, Form, HeaderForm, HyphenlessRole, Import, ImportRecord, KeyEntry, Role, document_text,
};
use specengine_import::layout::{
    self, CoreType, Emitted, FileKind, Layout, LayoutDiagnostic, Place, Reason, Scheme, SourceBody,
    SourceDocument, carried_fields, core_type, emit_scheme, field_keys, parses_as, s_heading,
    source_body, toml_text,
};
use specengine_import::walk;
use specengine_model::{
    AnchorOrigin, DiagnosticCode, FmValue, IdScope, LinkOrigin, LinkTarget, MENTIONS, Node,
    ParsedFile, Severity, Span,
};
use specengine_store::{
    BASELINE_FILE, CONFIG_FILE, CheckSetup, IndexWriter, NamedBytes, Source, SpecIndex,
    SqliteIndex, WorkingTree, b3_hash, check_input, check_source, load_check, today_utc,
};

use crate::census::{self, write_json};
use crate::check::refusal;
use crate::harness::{self, Corpus};

/// Fixture directory (relative to `fixtures/`) used without `--pilot`.
pub const FIXTURE: &str = "import-layout/one";

/// The measurement's directory under `--out`.
pub(crate) const MEASUREMENT: &str = "layout";
/// The after-tree's directory beside the detail files.
const TREE: &str = "tree";
/// The index of the tree.
const DB_FILE: &str = "index.db";
/// The project name of the scratch index.
const PROJECT: &str = "specengine-eval";
/// The `reason` of every emitted baseline entry.
const DEBT_REASON: &str = "import-layout source debt";

/// Check codes the attribution and the counts name (`docs/canon/spec-check.md`).
const FILE_NAME: &str = "file-name";
const ID_SCOPE: &str = "id-scope";
const INDEX_CODES: &str = "index-";
const ID_WIDTH: &str = "id-width";
const HOMOGLYPH: &str = "homoglyph";
const ID_TAKEN: &str = "id-taken";
const DUPLICATE_ID: &str = "duplicate-id";
const KEY_MISSING: &str = "key-missing";
const CLASS_MISSING: &str = "class-missing";
const CLASS_UNKNOWN: &str = "class-unknown";
const REF_DANGLING: &str = "ref-dangling";
const MENTION_DANGLING: &str = "mention-dangling";
const UNKNOWN_KEY: &str = "unknown-key";
const LINK_DANGLING: &str = "link-dangling";
const LINK_ANCHOR: &str = "link-anchor";
const FRONTMATTER_CODES: &str = "frontmatter-";
const FRONTMATTER_TYPE: &str = "frontmatter-type";
const CANON_MISSING: &str = "canon-missing";
const CANON_FORM: &str = "canon-form";
const CANON_FILE: &str = "canon-file";
const CANON_ANCHOR: &str = "canon-anchor";
const SHIPPED_MISSING: &str = "shipped-missing";
const UNPARSED_REFERENCE: &str = "unparsed-reference";
const KEY_EXTRA: &str = "key-extra";
const BUDGET: &str = "budget";
/// Core's `budget` slots other than a class's.
const INDEX_SLOT: &str = "index";
const TIER0_SLOT: &str = "tier0";
const TIER1_SLOT: &str = "tier1";
/// The suffix naming a reference-typed key re-read untyped
/// ([`AfterDoc::references_as_written`]).
const AS_WRITTEN: &str = "-as-written";

/// What the run needs, read and emitted before anything is written.
pub struct Setup {
    pub(crate) config: CensusConfig,
    pub(crate) scheme: Scheme,
    /// The before scheme loaded by the store, without a baseline.
    pub(crate) before: CheckSetup,
    pub(crate) today: String,
    pub(crate) import: Import,
    pub(crate) sources: Vec<SourceDocument>,
    pub(crate) layout: Layout,
    /// `[paths] index` and the shards of the `index = true` entry.
    pub(crate) index_outputs: Vec<String>,
    pub(crate) emit_ms: u128,
}

/// Reads the census config (`--config`, as `import`), the before scheme
/// (`--scheme`, as `check`) and the date, applies the start rules of
/// `[layout]`, then imports and emits in memory; an index output path a
/// tree document takes refuses the run too. Any error is a refusal (exit
/// 2, nothing written).
pub fn prepare(
    root: &Path,
    config: Option<&Path>,
    scheme: Option<&Path>,
    today: Option<&str>,
    label: Option<&str>,
) -> Result<Setup, String> {
    let config_path = harness::resolve_file(
        "--config",
        config,
        harness::PILOT_CENSUS_CONFIG,
        label,
        root,
        census::DEFAULT_CONFIG,
    )?;
    let config = CensusConfig::load(&config_path).map_err(|error| error.to_string())?;
    let scheme_path = harness::resolve_file(
        "--scheme",
        scheme,
        harness::PILOT_SCHEME,
        label,
        root,
        CONFIG_FILE,
    )?;
    let scheme_name = scheme_path.display().to_string();
    let before = load_check(&NamedBytes::read(scheme_name.clone(), &scheme_path), None)
        .map_err(|report| refusal(&report))?;
    let scheme_text = fs::read_to_string(&scheme_path)
        .map_err(|error| format!("{scheme_name}: cannot read the scheme: {error}"))?;
    let scheme = Scheme::parse(&scheme_text).map_err(|error| format!("{scheme_name}: {error}"))?;
    let today = match today {
        Some(today) if is_calendar_date(today) => today.to_owned(),
        Some(today) => return Err(format!("--today {today:?} is not a YYYY-MM-DD date")),
        None => today_utc(),
    };
    config
        .layout
        .check_start(&config_path, &scheme, &today)
        .map_err(|error| error.to_string())?;

    let started = Instant::now();
    let walked = walk::documents(root, &config)?;
    let found = import::run_walked(root, &config, walked.clone());
    let mut sources = layout::read_sources(root, &walked.documents);
    // A header the before check cannot parse is carried verbatim; one it
    // reads as no mapping takes no core key either
    // (`docs/features/import-layout.md` AC-05).
    for source in &mut sources {
        let parsed =
            specengine_core::parse(&source.path, source.text.as_bytes(), &before.project.scheme);
        source.header_unparsed = front_matter_failed(&parsed);
        source.header_not_mapping = parsed
            .diagnostics
            .iter()
            .any(|diagnostic| matches!(diagnostic.code, DiagnosticCode::FrontmatterNotMapping));
    }
    let emitted = layout::emit(&config, &found, &sources, &scheme);
    let emit_ms = started.elapsed().as_millis();

    let index_outputs = index_outputs(&before);
    if let Some(taken) = index_outputs.iter().find(|output| emitted.holds(output)) {
        let at = match index_line(&scheme_text, &before, taken) {
            Some(line) => format!("{scheme_name}:{line}"),
            None => scheme_name,
        };
        return Err(format!(
            "{at}: the index output `{taken}` is a document of the after-tree; nothing written"
        ));
    }
    Ok(Setup {
        config,
        scheme,
        before,
        today,
        import: found,
        sources,
        layout: emitted,
        index_outputs,
        emit_ms,
    })
}

/// Refuses a run that could write into the corpus or follow a link out of
/// `--out` (`docs/features/import-layout.md` AC-08): the canonical corpus
/// root under `--out` or `--out` under it, a symlinked `<out>/<measurement>`
/// or `<out>/<measurement>/<label>` (`layout`'s or `w`'s own directory).
/// Runs before anything is created.
pub fn check_out(root: &Path, out: &Path, measurement: &str, label: &str) -> Result<(), String> {
    let out = harness::absolutize(out);
    if root.starts_with(&out) || out.starts_with(root) {
        return Err(format!(
            "--out {} and the corpus {} are nested; nothing written",
            out.display(),
            root.display()
        ));
    }
    let measurement = out.join(measurement);
    for dir in [measurement.join(label), measurement] {
        if is_symlink(&dir) {
            return Err(format!(
                "{} is a symbolic link; nothing written",
                dir.display()
            ));
        }
    }
    Ok(())
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

/// The scheme line naming an index output: `[paths] index`'s for its path,
/// else the first line quoting the path (a generator's `writes`).
fn index_line(text: &str, setup: &CheckSetup, taken: &str) -> Option<usize> {
    #[derive(serde::Deserialize)]
    struct Scheme {
        paths: Option<PathsIndex>,
    }
    #[derive(serde::Deserialize)]
    struct PathsIndex {
        index: Option<toml::Spanned<String>>,
    }
    let line_of = |offset: usize| {
        text.get(..offset)
            .map(|head| head.matches('\n').count() + 1)
    };
    if setup.project.paths.index.as_deref() == Some(taken)
        && let Ok(Scheme {
            paths: Some(PathsIndex { index: Some(index) }),
        }) = toml::from_str::<Scheme>(text)
    {
        return line_of(index.span().start);
    }
    let quoted = [format!("\"{taken}\""), format!("'{taken}'")];
    text.lines()
        .position(|line| {
            !line.trim_start().starts_with('#') && quoted.iter().any(|q| line.contains(q.as_str()))
        })
        .map(|at| at + 1)
}

/// `[paths] index` and the shard paths of the `index = true` entry; none
/// without both.
fn index_outputs(setup: &CheckSetup) -> Vec<String> {
    match (setup.config.index_generator(), &setup.project.paths.index) {
        (Some(generator), Some(index)) => std::iter::once(index.clone())
            .chain(generator.shards.iter().map(|shard| shard.path.clone()))
            .collect(),
        _ => Vec::new(),
    }
}

/// The `result` object: exactly the whitelist of
/// `docs/canon/import-layout-verifier.md` "Output".
#[derive(Debug, Serialize)]
pub struct LayoutResult {
    pub before: Before,
    pub tree: Tree,
    pub hashes: Hashes,
    pub titles: Pair,
    pub fields: Pair,
    pub task_box: TaskBox,
    pub prose: Prose,
    pub extents: Extents,
    pub header: Header,
    pub reasons: BTreeMap<Reason, usize>,
    pub check: CheckCounts,
    pub dangling: Dangling,
    pub index: IndexCounts,
    pub code: CodeCounts,
    pub detail: Detail,
}

#[derive(Debug, Serialize)]
pub struct Before {
    pub documents: usize,
    pub definitions: usize,
    pub duplicates: usize,
    pub hyphenless: Hyphenless,
}

#[derive(Debug, Serialize)]
pub struct Hyphenless {
    pub definitions: usize,
    pub mentions: usize,
}

#[derive(Debug, Serialize)]
pub struct Tree {
    pub documents: usize,
    pub record_files: usize,
    pub feature_documents: usize,
    pub moved: usize,
    pub reshaped: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct Hashes {
    pub matched: usize,
    pub mismatched: usize,
    pub missing: usize,
    pub extra: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct Pair {
    pub matched: usize,
    pub mismatched: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct TaskBox {
    pub carried: usize,
    pub dropped: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct Prose {
    pub documents: usize,
    pub mismatched: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct Extents {
    pub total: usize,
    pub residue: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct Header {
    pub documents: usize,
    pub unparseable: usize,
    pub keys_dropped: usize,
    pub conflicts: usize,
    pub last_row_comment: usize,
}

#[derive(Debug, Serialize)]
pub struct CheckCounts {
    pub observe: Verdict,
    pub enforce: Verdict,
    pub stale: usize,
    pub emitter_findings: usize,
    /// Code → findings of the before check and of the tree by cause; codes
    /// with a finding only.
    pub findings: BTreeMap<String, CodeCauses>,
    pub baseline: BaselineCounts,
}

#[derive(Debug, Default, Serialize)]
pub struct CodeCauses {
    pub before: usize,
    pub source: usize,
    pub layout: usize,
    pub emitter: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct BaselineCounts {
    pub entries: usize,
    pub per_code: BTreeMap<String, usize>,
}

#[derive(Debug, Default, Serialize)]
pub struct Dangling {
    pub mentions: BeforeAfter,
    pub links: BeforeAfter,
}

#[derive(Debug, Default, Serialize)]
pub struct BeforeAfter {
    pub before: usize,
    pub after: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct IndexCounts {
    pub files: usize,
    pub nodes: usize,
    pub links: usize,
    pub full_ms: u128,
}

#[derive(Debug, Default, Serialize)]
pub struct CodeCounts {
    pub moved_cited: usize,
    pub citations_to_moved: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct Detail {
    pub diagnostics: usize,
    pub layout_ms: u128,
}

/// What [`write_tree`] wrote: the documents and index outputs, the notes
/// on paths it skipped, and the emitted config as the store loads it.
pub(crate) struct WrittenTree {
    /// Tree files written: documents and index outputs.
    pub(crate) written: usize,
    pub(crate) notes: Vec<Note>,
    /// The emitted `specengine.toml`, loaded as `spec check` loads it.
    pub(crate) emitted: CheckSetup,
}

/// The tree on disk, before the verifier reads it.
struct Staged {
    out_dir: PathBuf,
    tree: PathBuf,
    /// Tree documents written.
    written: usize,
    notes: Vec<Note>,
    stage_ms: u128,
}

/// A verifier diagnostic (never fatal).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Note {
    pub(crate) path: String,
    pub(crate) line: Option<usize>,
    pub(crate) message: String,
}

pub fn run(corpus: &Corpus, setup: Setup) -> Result<LayoutResult, String> {
    let staged = stage(corpus, &setup)?;
    finish(corpus, setup, staged)
}

/// [`run`] with `tamper` between writing the tree and reading it back: the
/// tamper tests of `docs/features/import-layout.md` change the written
/// files there (the tree directory, the emitter's output).
#[cfg(test)]
#[allow(
    dead_code,
    reason = "the hook of the tamper tests, which the test-engineer writes"
)]
pub(crate) fn run_tampered(
    corpus: &Corpus,
    setup: Setup,
    tamper: impl FnOnce(&Path, &Layout),
) -> Result<LayoutResult, String> {
    let staged = stage(corpus, &setup)?;
    tamper(&staged.tree, &setup.layout);
    finish(corpus, setup, staged)
}

/// Writes the after-tree into the empty directory `tree` as `layout`
/// writes it before its verifier reads it: the tree's documents, the
/// emitted `specengine.toml` (its roots covering the index outputs), then
/// the index outputs the emitted config renders over the written tree. No
/// baseline. `w` reads the tree it leaves (`docs/canon/w-measurement.md`;
/// `docs/features/pilot-w.md` AC-04).
pub(crate) fn write_tree(tree: &Path, setup: &Setup) -> Result<WrittenTree, String> {
    let mut notes = Vec::new();
    let mut written = write_documents(tree, setup, &mut notes)?;
    let emitted = load_emitted(tree)?;
    written += write_index_outputs(tree, &emitted, &mut notes)?;
    Ok(WrittenTree {
        written,
        notes,
        emitted,
    })
}

/// Empties `<out>/layout/<label>/`, writes the tree's documents and the
/// emitted `specengine.toml` (its roots covering the index outputs).
fn stage(corpus: &Corpus, setup: &Setup) -> Result<Staged, String> {
    let started = Instant::now();
    let out_dir = corpus.out.join(MEASUREMENT).join(&corpus.label);
    clear(&out_dir).map_err(|error| format!("cannot clear {}: {error}", out_dir.display()))?;
    let tree = out_dir.join(TREE);
    fs::create_dir_all(&tree)
        .map_err(|error| format!("cannot create {}: {error}", tree.display()))?;
    let mut notes = Vec::new();
    let written = write_documents(&tree, setup, &mut notes)?;
    Ok(Staged {
        out_dir,
        tree,
        written,
        notes,
        stage_ms: started.elapsed().as_millis(),
    })
}

/// The tree's documents and the emitted `specengine.toml`; the documents
/// written.
fn write_documents(tree: &Path, setup: &Setup, notes: &mut Vec<Note>) -> Result<usize, String> {
    let mut written = 0;
    for file in &setup.layout.files {
        if write_file(tree, &file.path, file.content.as_bytes(), notes)? {
            written += 1;
        }
    }
    let scheme_toml = if setup.index_outputs.is_empty() {
        setup.layout.scheme_toml.clone()
    } else {
        let paths = setup
            .layout
            .files
            .iter()
            .map(|file| file.path.as_str())
            .chain(setup.index_outputs.iter().map(String::as_str));
        emit_scheme(&setup.scheme, &setup.config, paths).0
    };
    let scheme_file = tree.join(CONFIG_FILE);
    fs::write(&scheme_file, scheme_toml)
        .map_err(|error| format!("cannot write {}: {error}", scheme_file.display()))?;
    Ok(written)
}

/// The emitted config at the tree's root, loaded by the store as `spec
/// check` loads it, without a baseline.
fn load_emitted(tree: &Path) -> Result<CheckSetup, String> {
    let scheme_file = tree.join(CONFIG_FILE);
    load_check(&NamedBytes::read(CONFIG_FILE, &scheme_file), None).map_err(|report| {
        format!(
            "the emitted {CONFIG_FILE} does not load: {}",
            refusal(&report)
        )
    })
}

/// The index outputs of the emitted config's `index = true` generator
/// over the tree as written; none without the generator and `[paths]
/// index`. The outputs written.
fn write_index_outputs(
    tree: &Path,
    emitted: &CheckSetup,
    notes: &mut Vec<Note>,
) -> Result<usize, String> {
    let mut written = 0;
    if let (Some(generator), Some(index_path)) = (
        emitted.config.index_generator(),
        emitted.project.paths.index.clone(),
    ) {
        let tree_source =
            WorkingTree::new(tree, &emitted.project.paths).map_err(|error| error.to_string())?;
        let input = check_input(&tree_source, &emitted.project.scheme);
        for output in render_index_set(&input, &index_path, generator) {
            if write_file(tree, &output.path, output.bytes.as_bytes(), notes)? {
                written += 1;
            }
        }
    }
    Ok(written)
}

/// Removes an earlier run's directory (this measurement's own scratch);
/// a symbolic link there or at its parent (`<out>/layout`, `<out>/w`) is
/// refused, never followed nor removed: nothing outside the directory is
/// deleted (AC-08).
pub(crate) fn clear(dir: &Path) -> io::Result<()> {
    let parent_link = dir.parent().is_some_and(is_symlink);
    match fs::symlink_metadata(dir) {
        _ if parent_link => Err(io::Error::other("a symbolic link is in its path")),
        Ok(meta) if meta.file_type().is_symlink() => Err(io::Error::other("it is a symbolic link")),
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(dir),
        Ok(_) => fs::remove_file(dir),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Writes one tree file; a path that is not clean and relative is noted
/// and skipped (`false`).
fn write_file(
    tree: &Path,
    path: &str,
    bytes: &[u8],
    notes: &mut Vec<Note>,
) -> Result<bool, String> {
    if !is_clean_relative(path) {
        notes.push(Note {
            path: path.to_owned(),
            line: None,
            message: "not a clean relative path: not written".to_owned(),
        });
        return Ok(false);
    }
    let target = tree.join(path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    fs::write(&target, bytes)
        .map_err(|error| format!("cannot write {}: {error}", target.display()))?;
    Ok(true)
}

/// Everything after the tree is on disk.
fn finish(corpus: &Corpus, setup: Setup, staged: Staged) -> Result<LayoutResult, String> {
    let Staged {
        out_dir,
        tree,
        mut written,
        mut notes,
        stage_ms,
    } = staged;
    let today = setup.today.as_str();

    // The before check: the corpus, the before scheme, no baseline.
    let corpus_tree = WorkingTree::new(&corpus.root, &setup.before.project.paths)
        .map_err(|error| error.to_string())?;
    let before_report = check_source(&corpus_tree, &setup.before, today);

    // The emitted config, loaded by the store as `spec check` loads it.
    let scheme_file = tree.join(CONFIG_FILE);
    let emitted = load_emitted(&tree)?;
    let tree_source =
        WorkingTree::new(&tree, &emitted.project.paths).map_err(|error| error.to_string())?;
    written += write_index_outputs(&tree, &emitted, &mut notes)?;

    // The verifier: every tree file re-read and parsed by core.
    let docs = read_tree(&tree, &tree_source, &setup, &emitted, &mut notes)?;
    let verified = verify(&setup, &docs)?;
    let left_tree = links_leaving(&setup, &docs, &emitted);

    // The tree check, observed, then attributed.
    let observe_report = check_source(&tree_source, &emitted, today);
    let attribution = Attribution::new(&setup, &before_report.findings, &verified, &emitted, &docs);
    let attributed: Vec<(Option<&str>, Cause)> = observe_report
        .findings
        .iter()
        .map(|finding| attribution.cause(finding))
        .collect();

    // The baseline of the source-caused errors, then the enforced check.
    let mut debt: BTreeSet<(String, String, String)> = BTreeSet::new();
    for (finding, (_, cause)) in observe_report.findings.iter().zip(&attributed) {
        if *cause == Cause::Source
            && finding.severity == Severity::Error
            && !finding.path.is_empty()
        {
            debt.insert((
                finding.code.clone(),
                finding.path.clone(),
                finding.subject.clone(),
            ));
        }
    }
    let baseline_file = tree.join(BASELINE_FILE);
    fs::write(
        &baseline_file,
        baseline_text(&debt, &setup.config.layout.debt_expires)?,
    )
    .map_err(|error| format!("cannot write {}: {error}", baseline_file.display()))?;
    let enforced = load_check(
        &NamedBytes::read(CONFIG_FILE, &scheme_file),
        Some(&NamedBytes::read(BASELINE_FILE, &baseline_file)),
    )
    .map_err(|report| {
        format!(
            "the emitted {BASELINE_FILE} does not load: {}",
            refusal(&report)
        )
    })?;
    let enforce_report = check_source(&tree_source, &enforced, today);

    // The index of the tree.
    let index = index_tree(&out_dir.join(DB_FILE), &tree, &tree_source, &emitted)?;

    // Counts.
    let mut findings: BTreeMap<String, CodeCauses> = BTreeMap::new();
    for finding in &before_report.findings {
        findings.entry(finding.code.clone()).or_default().before += 1;
    }
    let mut emitter_findings = 0;
    for (finding, (_, cause)) in observe_report.findings.iter().zip(&attributed) {
        let counts = findings.entry(finding.code.clone()).or_default();
        match cause {
            Cause::Source => counts.source += 1,
            Cause::Layout => counts.layout += 1,
            Cause::Emitter => {
                counts.emitter += 1;
                emitter_findings += 1;
            }
        }
    }
    let mut per_code: BTreeMap<String, usize> = BTreeMap::new();
    for (code, _, _) in &debt {
        *per_code.entry(code.clone()).or_default() += 1;
    }
    let count = |report: &[Finding], codes: &[&str]| {
        report
            .iter()
            .filter(|finding| codes.contains(&finding.code.as_str()))
            .count()
    };
    let dangling = Dangling {
        mentions: BeforeAfter {
            before: count(&before_report.findings, &[MENTION_DANGLING]),
            after: count(&observe_report.findings, &[MENTION_DANGLING]),
        },
        links: BeforeAfter {
            before: count(&before_report.findings, &[LINK_DANGLING, LINK_ANCHOR]),
            after: count(&observe_report.findings, &[LINK_DANGLING, LINK_ANCHOR]) + left_tree.len(),
        },
    };
    let moved_sources: BTreeSet<&str> = setup
        .layout
        .emission
        .documents
        .iter()
        .filter(|document| document.moved)
        .map(|document| document.source.as_str())
        .collect();
    let citations_to_moved: Vec<&str> = setup
        .import
        .code
        .citations
        .iter()
        .filter(|citation| moved_sources.contains(citation.document.as_str()))
        .map(|citation| citation.document.as_str())
        .collect();
    let moved_cited = citations_to_moved.iter().collect::<BTreeSet<_>>().len();

    let mut reasons: BTreeMap<Reason, usize> =
        Reason::ALL.iter().map(|reason| (*reason, 0)).collect();
    for outcome in &verified.records {
        if let Some(reason) = outcome.reason {
            *reasons.entry(reason).or_default() += 1;
        }
    }

    // Detail.
    write_json(&out_dir.join("emission.json"), &setup.layout.emission)?;
    write_json(&out_dir.join("records.json"), &verified.records)?;
    write_json(&out_dir.join("prose.json"), &verified.prose_detail)?;
    write_json(&out_dir.join("extents.json"), &verified.extent_detail)?;
    write_json(
        &out_dir.join("headers.json"),
        &HeadersDetail {
            outcomes: &setup.layout.headers,
            dropped: &verified.keys_dropped,
            unparseable: &verified.unparseable,
            field_values: &verified.field_values,
        },
    )?;
    let tree_findings: Vec<AttributedFinding<'_>> = observe_report
        .findings
        .iter()
        .zip(&attributed)
        .map(|(finding, (source, cause))| AttributedFinding {
            code: &finding.code,
            severity: finding.severity,
            path: &finding.path,
            line: finding.line,
            subject: &finding.subject,
            message: &finding.message,
            source: *source,
            cause: *cause,
        })
        .collect();
    write_json(
        &out_dir.join("findings.json"),
        &FindingsDetail {
            tree: tree_findings,
            stale: &enforce_report.stale,
            left_tree: &left_tree,
        },
    )?;
    write_json(
        &out_dir.join("diagnostics.json"),
        &DiagnosticsDetail {
            layout: &setup.layout.diagnostics,
            verifier: &notes,
        },
    )?;

    let layout_counts = &setup.layout;
    let result = LayoutResult {
        before: Before {
            documents: setup.import.documents,
            definitions: setup.import.role(Role::Definition),
            duplicates: setup.import.duplicates.len(),
            hyphenless: Hyphenless {
                definitions: setup.import.hyphenless(HyphenlessRole::Definition),
                mentions: setup.import.hyphenless(HyphenlessRole::Mention),
            },
        },
        tree: Tree {
            documents: written,
            record_files: layout_counts.record_files(),
            feature_documents: layout_counts.feature_documents(),
            moved: layout_counts.moved(),
            reshaped: layout_counts.placed(Place::Section),
        },
        hashes: verified.hashes,
        titles: verified.titles,
        fields: verified.fields,
        task_box: verified.task_box,
        prose: verified.prose,
        extents: verified.extents,
        header: Header {
            conflicts: layout_counts.conflicts(),
            last_row_comment: layout_counts
                .headers
                .iter()
                .filter(|header| header.last_row_comment)
                .count(),
            ..verified.header
        },
        reasons,
        check: CheckCounts {
            observe: observe_report.verdict_in(Mode::Observe),
            enforce: enforce_report.verdict_in(Mode::Enforce),
            stale: enforce_report.counts.stale,
            emitter_findings,
            findings,
            baseline: BaselineCounts {
                entries: debt.len(),
                per_code,
            },
        },
        dangling,
        index,
        code: CodeCounts {
            moved_cited,
            citations_to_moved: citations_to_moved.len(),
        },
        detail: Detail {
            diagnostics: layout_counts.diagnostics.len() + notes.len(),
            layout_ms: setup.emit_ms + stage_ms,
        },
    };
    summarize(&result, &corpus.label, &out_dir);
    Ok(result)
}

/// The `.spec-debt.toml` of the tree: one `[[debt]]` per (code, path,
/// subject), sorted.
fn baseline_text(
    debt: &BTreeSet<(String, String, String)>,
    expires: &str,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct DebtFile {
        debt: Vec<DebtOut>,
    }
    #[derive(Serialize)]
    struct DebtOut {
        code: String,
        path: String,
        subject: String,
        reason: String,
        expires: String,
    }
    let file = DebtFile {
        debt: debt
            .iter()
            .map(|(code, path, subject)| DebtOut {
                code: code.clone(),
                path: path.clone(),
                subject: subject.clone(),
                reason: DEBT_REASON.to_owned(),
                expires: expires.to_owned(),
            })
            .collect(),
    };
    match toml::Value::try_from(&file) {
        Ok(toml::Value::Table(table)) => Ok(toml_text(&table)),
        Ok(_) => Err("the baseline is not a table".to_owned()),
        Err(error) => Err(format!("cannot render the baseline: {error}")),
    }
}

/// Indexes the tree into a fresh DB: the full update, then the stored counts.
fn index_tree(
    db: &Path,
    tree: &Path,
    source: &WorkingTree,
    setup: &CheckSetup,
) -> Result<IndexCounts, String> {
    let mut index = SqliteIndex::open(db, PROJECT, tree).map_err(|error| error.to_string())?;
    let started = Instant::now();
    index
        .update(source, &setup.project.scheme)
        .map_err(|error| error.to_string())?;
    let full_ms = started.elapsed().as_millis();
    let stored = index.files().map_err(|error| error.to_string())?;
    let mut counts = IndexCounts {
        files: stored.len(),
        full_ms,
        ..IndexCounts::default()
    };
    for path in &stored {
        if let Some(parsed) = index
            .file(path)
            .map_err(|error| error.to_string())?
            .and_then(|file| file.parsed)
        {
            counts.nodes += parsed.nodes.len();
            counts.links += parsed.links.len();
        }
    }
    Ok(counts)
}

fn summarize(result: &LayoutResult, label: &str, out_dir: &Path) {
    eprintln!(
        "layout [{label}]: {} definitions; tree {} documents ({} record files, {} moved, {} reshaped); hashes {} matched, {} mismatched, {} missing, {} extra",
        result.before.definitions,
        result.tree.documents,
        result.tree.record_files,
        result.tree.moved,
        result.tree.reshaped,
        result.hashes.matched,
        result.hashes.mismatched,
        result.hashes.missing,
        result.hashes.extra
    );
    eprintln!(
        "  prose {} of {} mismatched, extent residue {}; check observe {}, enforce {}, stale {}, emitter findings {}, baseline {}",
        result.prose.mismatched,
        result.prose.documents,
        result.extents.residue,
        result.check.observe.as_str(),
        result.check.enforce.as_str(),
        result.check.stale,
        result.check.emitter_findings,
        result.check.baseline.entries
    );
    eprintln!("  detail: {}", out_dir.display());
}

// ---------------------------------------------------------------------------
// The tree read back.

/// A tree file as core reads it back from disk.
struct AfterDoc {
    bytes: Vec<u8>,
    parsed: ParsedFile,
    /// Byte offset of each line's start.
    starts: Vec<usize>,
    /// The document node's front-matter values by key: typed keys (as
    /// core's `fields` name them) and untyped ones; `None` for a value that
    /// is no scalar.
    front: BTreeMap<String, Option<String>>,
    /// The front-matter block's top-level entries as written.
    entries: Vec<HeaderEntry>,
    /// Each reference-typed entry the block holds once, as core's YAML
    /// reader decodes it read under an untyped key: its scalar or items as
    /// written (AC-05; see [`Self::read_back`]).
    written: BTreeMap<String, FmValue>,
}

impl AfterDoc {
    fn new(path: &str, bytes: Vec<u8>, setup: &CheckSetup) -> Self {
        let parsed = specengine_core::parse(path, &bytes, &setup.project.scheme);
        let starts = std::iter::once(0)
            .chain(
                bytes
                    .iter()
                    .enumerate()
                    .filter(|(_, byte)| **byte == b'\n')
                    .map(|(at, _)| at + 1),
            )
            .collect();
        let front = parsed.document().map(front_values).unwrap_or_default();
        let mut doc = Self {
            bytes,
            parsed,
            starts,
            front,
            entries: Vec::new(),
            written: BTreeMap::new(),
        };
        if let Some(block) = doc.parsed.front_matter {
            let open = doc.line_of(block.start);
            let close = doc.line_of(block.end.saturating_sub(1));
            doc.entries =
                header_entries((open + 1..close).map(|number| (number, doc.line(number))));
            doc.written = doc.references_as_written(path, open..=close, setup);
        }
        doc
    }

    /// The reference-typed entries the block holds once, each as core's
    /// YAML reader decodes it under an untyped name
    /// (`docs/features/import-layout.md` AC-05): the block's lines `block`
    /// re-read with those keys renamed (a name of its own the block lacks),
    /// so that every item reads back as written — escaped, folded or not
    /// a reference — where core's typed reading keeps no span of it.
    fn references_as_written(
        &self,
        path: &str,
        block: RangeInclusive<usize>,
        setup: &CheckSetup,
    ) -> BTreeMap<String, FmValue> {
        let names: BTreeSet<&str> = self
            .entries
            .iter()
            .map(|entry| entry.key.as_str())
            .collect();
        let renamed: BTreeMap<usize, (String, String)> = self
            .entries
            .iter()
            .filter(|entry| {
                matches!(
                    core_type(&entry.key),
                    Some(CoreType::Reference | CoreType::ReferenceList)
                ) && self.entry(&entry.key).is_some()
            })
            .filter_map(|entry| {
                let name = format!("{}{AS_WRITTEN}", entry.key);
                (core_type(&name).is_none() && !names.contains(name.as_str()))
                    .then(|| (entry.first, (entry.key.clone(), name)))
            })
            .collect();
        if renamed.is_empty() {
            return BTreeMap::new();
        }
        let key_len: BTreeMap<usize, usize> = self
            .entries
            .iter()
            .map(|entry| (entry.first, entry.key_len))
            .collect();
        let mut text = String::new();
        for number in block {
            let line = self.line(number);
            match (renamed.get(&number), key_len.get(&number)) {
                (Some((_, name)), Some(&len)) => {
                    text.push_str(name);
                    text.push_str(line.get(len..).unwrap_or_default());
                }
                _ => text.push_str(line),
            }
            text.push('\n');
        }
        let reread = specengine_core::parse(path, text.as_bytes(), &setup.project.scheme);
        let Some(node) = reread.document() else {
            return BTreeMap::new();
        };
        renamed
            .into_values()
            .filter_map(|(key, name)| {
                node.extra
                    .iter()
                    .flatten()
                    .find(|entry| entry.key == name)
                    .map(|entry| (key, entry.value.clone()))
            })
            .collect()
    }

    /// The 1-based line holding byte `offset`.
    fn line_of(&self, offset: usize) -> usize {
        self.starts.partition_point(|&start| start <= offset)
    }

    fn text(&self, span: Span) -> &str {
        self.bytes
            .get(span.range())
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .unwrap_or("")
    }

    /// Line `number` (1-based) without its terminator (a CR before the LF
    /// dropped); empty past the end.
    fn line(&self, number: usize) -> &str {
        let start = number
            .checked_sub(1)
            .and_then(|at| self.starts.get(at))
            .copied()
            .unwrap_or(self.bytes.len());
        let end = self
            .starts
            .get(number)
            .map_or(self.bytes.len(), |next| next - 1);
        let line = self
            .bytes
            .get(start..end.max(start))
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .unwrap_or("");
        line.strip_suffix('\r').unwrap_or(line)
    }

    /// The front-matter value core read under `key`: a typed or untyped
    /// key of the document node (`fields`, `extra`), else its `id`, its
    /// front-matter `title`, or another member core keeps on the node
    /// (`rev`, `kind`, `parent`), named by the model's serde.
    fn front_value(&self, key: &str) -> Option<String> {
        if let Some(value) = self.front.get(key) {
            return value.clone();
        }
        let node = self.parsed.document()?;
        if key == CoreKey::Id.name() {
            return node.id.clone();
        }
        if key == CoreKey::Title.name() {
            return self.parsed.front_matter.as_ref().and(node.title.clone());
        }
        core_type(key)?;
        match serde_json::to_value(node) {
            Ok(serde_json::Value::Object(map)) => map.get(key).and_then(json_scalar),
            _ => None,
        }
    }

    /// The scalar core kept in `extra` under `key` (a mistyped typed key's
    /// value), as text; a float keeps its fraction (`2.0`, never an
    /// integer's text; `docs/features/import-layout.md` AC-06).
    fn kept_text(&self, key: &str) -> Option<String> {
        let entry = self
            .parsed
            .document()?
            .extra
            .iter()
            .flatten()
            .rev()
            .find(|entry| entry.key == key)?;
        match &entry.value {
            FmValue::Float(number) => Some(format!("{number:?}")),
            value => fm_scalar(value),
        }
    }

    /// The numbered lines of `span`.
    fn lines(&self, span: Span) -> Vec<(usize, &str)> {
        let first = self.line_of(span.start);
        self.text(span)
            .split('\n')
            .enumerate()
            .map(|(at, line)| (first + at, line.strip_suffix('\r').unwrap_or(line)))
            .collect()
    }

    /// Core could not read the front-matter (or the file).
    fn header_failed(&self) -> bool {
        front_matter_failed(&self.parsed)
    }

    /// The top-level front-matter keys core read: `fields` and `extra`
    /// keys, and the node members of `id`, `title` and the other typed
    /// keys core keeps outside `fields`.
    fn keys(&self) -> BTreeSet<String> {
        let mut keys: BTreeSet<String> = self.front.keys().cloned().collect();
        if let Some(node) = self.parsed.document() {
            let id_written = node.id.is_some()
                || self
                    .parsed
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == DiagnosticCode::IdNotInScheme);
            if id_written {
                keys.insert(CoreKey::Id.name());
            }
            if node.title.is_some() && self.parsed.front_matter.is_some() {
                keys.insert(CoreKey::Title.name());
            }
            // The other typed members, named by the model's serde: those
            // the node holds and a copy without them lacks.
            let mut bare = node.clone();
            bare.kind = None;
            bare.rev = None;
            bare.parent = None;
            if let (Ok(serde_json::Value::Object(full)), Ok(serde_json::Value::Object(stripped))) =
                (serde_json::to_value(node), serde_json::to_value(&bare))
            {
                keys.extend(
                    full.keys()
                        .filter(|key| !stripped.contains_key(*key))
                        .cloned(),
                );
            }
        }
        keys
    }

    /// The front-matter entry of `key`, when the block holds it once.
    fn entry(&self, key: &str) -> Option<&HeaderEntry> {
        let mut found = self.entries.iter().filter(|entry| entry.key == key);
        let first = found.next()?;
        found.next().is_none().then_some(first)
    }

    /// The key of the front-matter entry holding line `number`.
    fn key_at(&self, number: usize) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.lines().contains(&number))
            .map(|entry| entry.key.as_str())
    }

    /// The value core read back under `key` (`docs/features/import-layout.md`
    /// AC-03, AC-05): an untyped or mistyped one as `extra` keeps it; under
    /// a reference or reference-list key, its scalar or each item as
    /// written, as core's YAML reader decodes it (a reference's span text
    /// where core keeps one; `XQ-42@2`, `slug/`, `project:`, `#section`, a
    /// path-form `canon:` with its `#anchor`, an escaped item core keeps
    /// no span of, prose); another typed one as core typed it.
    fn read_back(&self, key: &str) -> ReadBack {
        let Some(node) = self.parsed.document() else {
            return ReadBack::Nothing;
        };
        if let Some(entry) = node
            .extra
            .iter()
            .flatten()
            .rev()
            .find(|entry| entry.key == key)
        {
            return match &entry.value {
                FmValue::Seq(items) => {
                    ReadBack::Items(items.iter().filter_map(fm_scalar).collect())
                }
                value => fm_scalar(value).map_or(ReadBack::Nothing, ReadBack::Scalar),
            };
        }
        let reference = matches!(
            core_type(key),
            Some(CoreType::Reference | CoreType::ReferenceList)
        );
        if reference && let Some(value) = self.written.get(key) {
            return match value {
                FmValue::Seq(items) => {
                    ReadBack::Items(items.iter().filter_map(fm_scalar).collect())
                }
                value => fm_scalar(value).map_or(ReadBack::Nothing, ReadBack::Scalar),
            };
        }
        match self.member(node, key) {
            Some(serde_json::Value::Array(items)) => {
                let mut texts: Vec<String> = items.iter().filter_map(json_scalar).collect();
                if reference {
                    texts.extend(self.unparsed_texts(key));
                }
                ReadBack::Items(texts)
            }
            Some(value) => json_scalar(&value).map_or(ReadBack::Nothing, ReadBack::Scalar),
            None if reference => match self.unparsed_texts(key).as_slice() {
                [text] => ReadBack::Scalar(text.clone()),
                _ => ReadBack::Nothing,
            },
            None => ReadBack::Nothing,
        }
    }

    /// The JSON of the node member core keeps a typed `key` in: `fields`,
    /// the `id`, the front-matter `title`, the other typed members.
    fn member(&self, node: &Node, key: &str) -> Option<serde_json::Value> {
        if let Some(fields) = &node.fields
            && let Ok(serde_json::Value::Object(map)) = serde_json::to_value(fields)
            && let Some(value) = map.get(key)
        {
            return Some(value.clone());
        }
        if key == CoreKey::Id.name() {
            return node.id.clone().map(serde_json::Value::String);
        }
        if key == CoreKey::Title.name() {
            return self
                .parsed
                .front_matter
                .and(node.title.clone())
                .map(serde_json::Value::String);
        }
        core_type(key)?;
        match serde_json::to_value(node) {
            Ok(serde_json::Value::Object(map)) => map.get(key).cloned(),
            _ => None,
        }
    }

    /// The texts of the `unparsed-reference` diagnostics in `key`'s entry.
    fn unparsed_texts(&self, key: &str) -> Vec<String> {
        let Some(lines) = self.entry(key).map(HeaderEntry::lines) else {
            return Vec::new();
        };
        self.parsed
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code == DiagnosticCode::UnparsedReference
                    && lines.contains(&diagnostic.line)
            })
            .filter_map(|diagnostic| diagnostic.span.map(|span| self.text(span).to_owned()))
            .collect()
    }
}

/// Core could not read a file's front-matter (or the file): the before
/// check's `frontmatter-*` failures on a source, the tree's `unparseable`.
fn front_matter_failed(parsed: &ParsedFile) -> bool {
    parsed.diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code,
            DiagnosticCode::NotUtf8
                | DiagnosticCode::FrontmatterUnclosed
                | DiagnosticCode::FrontmatterYaml
                | DiagnosticCode::FrontmatterNotMapping
        )
    })
}

/// One top-level entry of a front-matter block as written, by the
/// verifier's own lenient reading (`docs/features/import-layout.md`
/// AC-05): the key (quotes removed), the bytes of its key token on the
/// first line, the entry's first and last line (the key line, then the
/// indented and blank lines after it, and the column-0 comment lines
/// among them), and everything after the key token (the `:`, the value, a
/// comment, the continuation lines), trailing blanks dropped.
#[derive(Debug, Clone)]
struct HeaderEntry {
    key: String,
    key_len: usize,
    first: usize,
    last: usize,
    rest: String,
}

impl HeaderEntry {
    fn lines(&self) -> RangeInclusive<usize> {
        self.first..=self.last
    }

    /// The value as written: the rest less its `:` and the blanks around.
    fn value(&self) -> &str {
        self.rest
            .trim_start()
            .strip_prefix(':')
            .unwrap_or(&self.rest)
            .trim()
    }
}

/// The entries of a block's lines (`docs/features/import-layout.md` AC-05):
/// a line opening with neither a blank nor `#` nor a `-` item, holding
/// `key:` (a quoted key up to its closing quote), opens one; indented and
/// blank lines continue it, and so do the column-0 comment lines before
/// such a line (`refs:`, `# c`, `  - X`: YAML skips the comment); a
/// top-level item or key line ends it (comments just before it belong to
/// no entry).
fn header_entries<'l>(lines: impl Iterator<Item = (usize, &'l str)>) -> Vec<HeaderEntry> {
    let mut entries: Vec<HeaderEntry> = Vec::new();
    let mut open = false;
    // Comment lines (and the blanks after them) of an open entry, kept
    // once an indented line shows the entry goes on past them.
    let mut held: Vec<&str> = Vec::new();
    for (number, line) in lines {
        let blank = line.trim().is_empty();
        if open && (line.starts_with('#') || (blank && !held.is_empty())) {
            held.push(line);
            continue;
        }
        if blank || line.starts_with([' ', '\t']) {
            if open && let Some(entry) = entries.last_mut() {
                for kept in held.drain(..).chain(std::iter::once(line)) {
                    entry.rest.push('\n');
                    entry.rest.push_str(kept);
                }
            }
            continue;
        }
        held.clear();
        open = false;
        if line.starts_with('#')
            || line
                .strip_prefix('-')
                .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '\t']))
        {
            continue;
        }
        let split = match line.chars().next() {
            Some(quote @ ('"' | '\'')) => line[1..].find(quote).and_then(|close| {
                let after = &line[close + 2..];
                after
                    .trim_start_matches([' ', '\t'])
                    .starts_with(':')
                    .then(|| (line[1..close + 1].to_owned(), after))
            }),
            _ => line
                .match_indices(':')
                .find(|(at, _)| {
                    line[at + 1..].is_empty() || line[at + 1..].starts_with([' ', '\t'])
                })
                .map(|(at, _)| (line[..at].trim_end().to_owned(), &line[at..])),
        };
        if let Some((key, rest)) = split {
            open = true;
            entries.push(HeaderEntry {
                key,
                key_len: line.len() - rest.len(),
                first: number,
                last: number,
                rest: rest.to_owned(),
            });
        }
    }
    // Trailing blank lines are no part of an entry; its lines are
    // consecutive from its key line.
    for entry in &mut entries {
        let kept = entry.rest.trim_end().len();
        entry.rest.truncate(kept);
        entry.last = entry.first + entry.rest.matches('\n').count();
    }
    entries
}

/// A front-matter value core read back (`docs/features/import-layout.md`
/// AC-03, AC-05).
#[derive(Debug, Clone, PartialEq, Eq)]
enum ReadBack {
    /// Absent, or neither a scalar nor a list.
    Nothing,
    Scalar(String),
    /// A list's scalar items.
    Items(Vec<String>),
}

impl ReadBack {
    /// The value as the detail files show it: a list's items comma-joined.
    fn shown(&self) -> Option<String> {
        match self {
            Self::Nothing => None,
            Self::Scalar(text) => Some(text.clone()),
            Self::Items(items) => Some(items.join(", ")),
        }
    }

    /// Whether it is `expected`, carried under `key` (AC-05): a scalar
    /// equal to it; under a reference-list key, a list of its
    /// comma-separated items, trimmed, empty ones dropped, in any order;
    /// else a one-item list.
    fn is(&self, key: &str, expected: &str) -> bool {
        match self {
            Self::Nothing => false,
            Self::Scalar(text) => text == expected,
            Self::Items(items) if core_type(key) == Some(CoreType::ReferenceList) => {
                let mut want: Vec<&str> = expected
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .collect();
                let mut got: Vec<&str> = items.iter().map(String::as_str).collect();
                want.sort_unstable();
                got.sort_unstable();
                got == want
            }
            Self::Items(_) => self.is_one_item(expected),
        }
    }

    /// Whether it is a one-item list holding `expected` whole.
    fn is_one_item(&self, expected: &str) -> bool {
        matches!(self, Self::Items(items) if items.len() == 1 && items[0] == expected)
    }
}

/// A node's front-matter values by key (`extra` over `fields`).
fn front_values(node: &Node) -> BTreeMap<String, Option<String>> {
    let mut values = BTreeMap::new();
    if let Some(fields) = &node.fields
        && let Ok(serde_json::Value::Object(map)) = serde_json::to_value(fields)
    {
        for (key, value) in map {
            values.insert(key, json_scalar(&value));
        }
    }
    for entry in node.extra.iter().flatten() {
        values.insert(entry.key.clone(), fm_scalar(&entry.value));
    }
    values
}

fn fm_scalar(value: &FmValue) -> Option<String> {
    match value {
        FmValue::Str(text) => Some(text.clone()),
        FmValue::Bool(flag) => Some(flag.to_string()),
        FmValue::Int(number) => Some(number.to_string()),
        FmValue::UInt(number) => Some(number.to_string()),
        FmValue::Float(number) => Some(number.to_string()),
        _ => None,
    }
}

/// A typed value as text: a scalar, a reference's ID, or the one item of a
/// one-item list (a list-typed key's carried value, AC-05).
fn json_scalar(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Array(items) if items.len() == 1 => items.first().and_then(json_scalar),
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Bool(flag) => Some(flag.to_string()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Object(map) => map
            .get(&CoreKey::Id.name())
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        _ => None,
    }
}

/// Every file of the tree walk, and every map path outside it that
/// exists, read from disk and parsed with the emitted scheme.
fn read_tree(
    tree: &Path,
    source: &WorkingTree,
    setup: &Setup,
    emitted: &CheckSetup,
    notes: &mut Vec<Note>,
) -> Result<BTreeMap<String, AfterDoc>, String> {
    let listing = source
        .list()
        .map_err(|error| format!("cannot list the tree: {error}"))?;
    let mut paths: BTreeSet<String> = listing.paths.into_iter().collect();
    for file in &setup.layout.files {
        if is_clean_relative(&file.path) && tree.join(&file.path).is_file() {
            paths.insert(file.path.clone());
        }
    }
    let mut docs = BTreeMap::new();
    for path in paths {
        match fs::read(tree.join(&path)) {
            Ok(bytes) => {
                let doc = AfterDoc::new(&path, bytes, emitted);
                docs.insert(path, doc);
            }
            Err(error) => notes.push(Note {
                path,
                line: None,
                message: format!("cannot read the tree file: {error}"),
            }),
        }
    }
    Ok(docs)
}

// ---------------------------------------------------------------------------
// The verifier.

/// One walked document's "before" side.
struct SourceSide<'s> {
    text: &'s str,
    body: SourceBody,
    /// Extents moved out of the document (to a record file, reshaped), by
    /// definition index.
    cut: Vec<(usize, [usize; 2])>,
    /// Rule S on the heading lines of in-place sections: line → (written, id).
    rule_s: BTreeMap<usize, (String, String)>,
}

impl SourceSide<'_> {
    /// The body lines of `range` outside the field table and outside the
    /// moved-out extents nested in it (`own` excepted), rule S applied,
    /// normalised by `document_text`.
    fn text_of(&self, range: RangeInclusive<usize>, own: Option<usize>) -> String {
        let nested: Vec<RangeInclusive<usize>> = self
            .cut
            .iter()
            .filter(|(index, extent)| {
                Some(*index) != own && range.contains(&extent[0]) && range.contains(&extent[1])
            })
            .map(|(_, extent)| extent[0]..=extent[1])
            .collect();
        let lines: Vec<String> = self
            .body
            .lines
            .iter()
            .filter(|(number, _)| {
                range.contains(number)
                    && !self
                        .body
                        .excluded
                        .as_ref()
                        .is_some_and(|excluded| excluded.contains(number))
                    && !nested.iter().any(|extent| extent.contains(number))
            })
            .map(|(number, raw)| match self.rule_s.get(number) {
                Some((written, id)) => s_heading(raw, written, id).unwrap_or_else(|| raw.clone()),
                None => raw.clone(),
            })
            .collect();
        document_text(&lines.join("\n"))
    }

    /// The raw lines of `range`, LF-joined.
    fn raw(&self, range: RangeInclusive<usize>) -> String {
        self.body
            .lines
            .iter()
            .filter(|(number, _)| range.contains(number))
            .map(|(_, raw)| raw.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn raw_line(&self, number: usize) -> &str {
        self.body
            .lines
            .iter()
            .find(|(line, _)| *line == number)
            .map_or("", |(_, raw)| raw.as_str())
    }
}

/// A definition found in the tree: its file, its node, its place.
#[derive(Clone, Copy)]
struct Found<'d> {
    doc: &'d AfterDoc,
    node: &'d Node,
    place: Place,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum HashOutcome {
    Matched,
    Mismatched,
    Missing,
}

/// One definition, verified (`records.json`).
#[derive(Debug, Serialize)]
struct RecordOutcome {
    path: String,
    line: usize,
    id: String,
    form: Form,
    after: Option<String>,
    place: Option<Place>,
    hash: HashOutcome,
    before_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    after_hash: Option<String>,
    /// Compared titles only.
    #[serde(skip_serializing_if = "Option::is_none")]
    title_matched: Option<bool>,
    fields_matched: usize,
    fields_mismatched: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_box_carried: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<Reason>,
}

#[derive(Debug, Serialize)]
struct ProseOutcome {
    source: String,
    after: String,
    matched: bool,
    /// The first differing line of the two normalised texts (1-based).
    #[serde(skip_serializing_if = "Option::is_none")]
    first_difference: Option<usize>,
}

#[derive(Debug, Serialize)]
struct ExtentOutcome {
    path: String,
    line: usize,
    /// `None` for a field table (the header's extent).
    id: Option<String>,
    residue: String,
}

/// A carried header value read back (`headers.json`): a YAML-block entry
/// (`yaml`) or a field-table row (`field-table`).
#[derive(Debug, Serialize)]
struct FieldValue {
    path: String,
    line: usize,
    form: HeaderForm,
    key: String,
    expected: String,
    read: Option<String>,
    matched: bool,
}

#[derive(Debug, Serialize)]
struct DroppedKey {
    path: String,
    key: String,
}

/// What the verifier found.
#[derive(Default)]
struct Verified {
    records: Vec<RecordOutcome>,
    hashes: Hashes,
    titles: Pair,
    fields: Pair,
    task_box: TaskBox,
    prose: Prose,
    prose_detail: Vec<ProseOutcome>,
    extents: Extents,
    extent_detail: Vec<ExtentOutcome>,
    header: Header,
    keys_dropped: Vec<DroppedKey>,
    field_values: Vec<FieldValue>,
    /// (tree path, front-matter key) of a carried value that did not read
    /// back as the corpus wrote it: a record file's field or attribute, a
    /// document record's field, a field-table value, a YAML-block entry.
    mismatched_keys: BTreeSet<(String, String)>,
    /// (tree path, front-matter key) of a carried value compared and read
    /// back as the corpus wrote it: the positive proof the attribution
    /// asks for (`docs/features/import-layout.md` AC-06).
    compared_keys: BTreeSet<(String, String)>,
    /// (tree path, key) of a YAML-block entry the tree holds with the
    /// source's text after its key: carried verbatim, not rewritten
    /// (AC-06).
    verbatim_keys: BTreeSet<(String, String)>,
    /// Residue path → the keys its header carries from its source
    /// document's, under the names they take there (a `key_map` target,
    /// else as written).
    carried_after: BTreeMap<String, BTreeSet<String>>,
    unparseable: Vec<String>,
    /// Latin IDs of the definitions found in the tree.
    found: BTreeSet<String>,
}

impl Verified {
    /// One carried header value compared (`headers.json` `field_values`):
    /// counted in `fields`, its key at `after` proven or mismatched.
    fn tally_value(&mut self, value: FieldValue, after: &str) {
        let entry = (after.to_owned(), value.key.clone());
        if value.matched {
            self.fields.matched += 1;
            self.compared_keys.insert(entry);
        } else {
            self.fields.mismatched += 1;
            self.mismatched_keys.insert(entry);
        }
        self.field_values.push(value);
    }
}

/// Re-reads every definition from the tree (`docs/features/import-layout.md`
/// AC-02 to AC-05): located by the map and the ID in
/// core's parse, compared by identity. The map out of step with the model
/// is an internal failure.
fn verify(setup: &Setup, docs: &BTreeMap<String, AfterDoc>) -> Result<Verified, String> {
    let verifier = Verifier::new(setup, docs)?;
    let mut verified = Verified::default();
    let mut core_headings = BTreeMap::new();
    for index in 0..verifier.definitions.len() {
        let outcome = verifier.compare(index, &mut verified, &mut core_headings);
        verified.records.push(outcome);
    }
    verified.hashes.extra = verifier.extra();
    verifier.prose(&mut verified);
    verifier.headers(&mut verified);
    verifier.header_values(&mut verified);
    verifier.field_tables(&mut verified);
    Ok(verified)
}

/// The verifier's view of the sources and of the tree read back.
struct Verifier<'a> {
    setup: &'a Setup,
    docs: &'a BTreeMap<String, AfterDoc>,
    /// Each definition of the model beside its entry in the emission map.
    definitions: Vec<(&'a ImportRecord, &'a Emitted)>,
    sides: BTreeMap<&'a str, SourceSide<'a>>,
    /// Per definition: its node's index in its tree file's parse.
    located: Vec<Option<usize>>,
    /// Per tree file: the nodes a definition took.
    consumed: BTreeMap<&'a str, BTreeSet<usize>>,
    /// Per tree file: its reshaped blocks, (heading line, last line).
    blocks: BTreeMap<&'a str, Vec<(usize, usize)>>,
}

impl<'a> Verifier<'a> {
    fn new(setup: &'a Setup, docs: &'a BTreeMap<String, AfterDoc>) -> Result<Self, String> {
        let emission = &setup.layout.emission.definitions;
        let definitions: Vec<(&ImportRecord, &Emitted)> = setup
            .import
            .records
            .iter()
            .filter(|record| record.role == Role::Definition)
            .zip(emission)
            .collect();
        let aligned = definitions.len() == emission.len()
            && setup.import.role(Role::Definition) == emission.len()
            && definitions.iter().all(|(record, emitted)| {
                record.path == emitted.path
                    && record.line == emitted.line
                    && record.id == emitted.id
            });
        if !aligned {
            return Err("the emission map does not follow the model's definitions".to_owned());
        }
        let sides = setup
            .sources
            .iter()
            .map(|source| {
                (
                    source.path.as_str(),
                    SourceSide {
                        text: source.text.as_str(),
                        body: source_body(&setup.config, &source.text),
                        cut: Vec::new(),
                        rule_s: BTreeMap::new(),
                    },
                )
            })
            .collect();
        let mut verifier = Self {
            setup,
            docs,
            located: vec![None; definitions.len()],
            definitions,
            sides,
            consumed: BTreeMap::new(),
            blocks: BTreeMap::new(),
        };
        verifier.mark_sources();
        verifier.locate();
        Ok(verifier)
    }

    /// Per source document: the extents moved out of it and the heading
    /// lines rule S rewrites, from the map.
    fn mark_sources(&mut self) {
        for (index, (record, emitted)) in self.definitions.iter().enumerate() {
            let Some(side) = self.sides.get_mut(record.path.as_str()) else {
                continue;
            };
            match (emitted.place, record.form) {
                (_, Form::Document) | (None, _) => {}
                (Some(Place::File | Place::Section), _) => side.cut.push((index, record.extent)),
                (Some(Place::InPlace), _) => {
                    if record.written != record.id {
                        side.rule_s
                            .insert(record.line, (record.written.clone(), record.id.clone()));
                    }
                }
            }
        }
    }

    /// Each placed definition's node in core's parse of its after file: the
    /// document node of a record file or a document record, else the first
    /// `{#ID}` section of its ID no earlier definition took that stands
    /// where its placement puts it ([`placed`], AC-02); in-place sections
    /// before reshaped blocks, so that neither takes the other's node. Then
    /// the lines of every reshaped block found.
    fn locate(&mut self) {
        for reshaped_pass in [false, true] {
            for (index, (record, emitted)) in self.definitions.iter().enumerate() {
                let (Some(after), Some(place)) = (emitted.after.as_deref(), emitted.place) else {
                    continue;
                };
                if (place == Place::Section) != reshaped_pass {
                    continue;
                }
                let Some(doc) = self.docs.get(after) else {
                    continue;
                };
                let side = self.sides.get(record.path.as_str());
                let taken = self.consumed.entry(after).or_default();
                let document = place == Place::File || record.form == Form::Document;
                let node = doc
                    .parsed
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(at, _)| (*at == 0) == document)
                    .find(|(at, node)| {
                        node.id.as_deref() == Some(record.id.as_str())
                            && !taken.contains(at)
                            && placed(doc, node, record, place, side)
                    })
                    .map(|(at, _)| at);
                if let Some(node) = node {
                    taken.insert(node);
                    self.located[index] = Some(node);
                    if place == Place::Section
                        && let Some(node) = doc.parsed.nodes.get(node)
                    {
                        self.blocks
                            .entry(after)
                            .or_default()
                            .push(block_lines(doc, node));
                    }
                }
            }
        }
    }

    fn blocks_of(&self, path: &str) -> &[(usize, usize)] {
        self.blocks.get(path).map_or(&[], Vec::as_slice)
    }

    /// A definition's text on the before side, by the verifier's table:
    /// a section moved to a file less its heading, a container less the
    /// extents moved out of it with rule S applied, any other the model's
    /// `text`.
    fn before_text(&self, index: usize) -> String {
        let (record, emitted) = self.definitions[index];
        let side = self.sides.get(record.path.as_str());
        match (side, emitted.place, record.form) {
            (Some(side), Some(Place::File), Form::Section) => {
                side.text_of(record.extent[0] + 1..=record.extent[1], Some(index))
            }
            (Some(side), Some(Place::InPlace), Form::Section) => {
                side.text_of(record.extent[0]..=record.extent[1], Some(index))
            }
            (Some(side), Some(Place::InPlace), Form::Document) => {
                side.text_of(1..=usize::MAX, None)
            }
            _ => document_text(&record.text),
        }
    }

    /// A definition's text on the after side: a reshaped section's body, an
    /// in-place section's span or a file's body, less the reshaped blocks
    /// in it.
    fn after_text(&self, found: Found<'_>, record: &ImportRecord, after: &str) -> String {
        let Found { doc, node, place } = found;
        match (place, record.form) {
            (Place::Section, _) => node
                .body
                .map_or(String::new(), |body| document_text(doc.text(body))),
            (Place::InPlace, Form::Section) => after_text(doc, node.span, self.blocks_of(after)),
            _ => after_text(doc, doc.parsed.body, self.blocks_of(after)),
        }
    }

    /// Compares one definition; counts into `verified`.
    fn compare(
        &self,
        index: usize,
        verified: &mut Verified,
        core_headings: &mut BTreeMap<&'a str, BTreeSet<usize>>,
    ) -> RecordOutcome {
        let (record, emitted) = self.definitions[index];
        let side = self.sides.get(record.path.as_str());
        let doc = emitted
            .after
            .as_deref()
            .and_then(|after| self.docs.get(after));
        let mut outcome = RecordOutcome {
            path: record.path.clone(),
            line: record.line,
            id: record.id.clone(),
            form: record.form,
            after: emitted.after.clone(),
            place: emitted.place,
            hash: HashOutcome::Missing,
            before_hash: b3_hash(self.before_text(index).as_bytes()),
            after_hash: None,
            title_matched: None,
            fields_matched: 0,
            fields_mismatched: 0,
            task_box_carried: None,
            reason: emitted.reason,
        };
        let found = match (self.located[index], doc, emitted.place) {
            (Some(node), Some(doc), Some(place)) => {
                doc.parsed
                    .nodes
                    .get(node)
                    .map(|node| Found { doc, node, place })
            }
            _ => None,
        };
        let miss = match found {
            Some(found) => {
                verified.found.insert(record.id.clone());
                self.compare_found(record, found, &mut outcome, verified)
            }
            None => {
                verified.hashes.missing += 1;
                if record.task_box.is_some() {
                    outcome.task_box_carried = Some(false);
                    verified.task_box.dropped += 1;
                }
                true
            }
        };
        if miss && outcome.reason.is_none() {
            let header_failed = doc.is_some_and(AfterDoc::header_failed)
                && (emitted.place == Some(Place::File) || record.form == Form::Document);
            outcome.reason = Some(if header_failed {
                Reason::HeaderUnparseable
            } else if side.is_some_and(|side| {
                reader_boundary(side, record, &self.setup.before, core_headings)
            }) {
                Reason::ReaderBoundary
            } else {
                Reason::Unexplained
            });
        }
        // The extent residue of an extent moved out.
        if let (Some(side), Some(Place::File | Place::Section)) = (side, emitted.place)
            && record.form != Form::Document
        {
            verified.extents.total += 1;
            let heading = (record.form == Form::Section)
                .then(|| heading_residue(side, record, found, &self.setup.config));
            if let Some(residue) = extent_residue(
                side,
                record,
                &self.setup.config.import.separators,
                heading.as_deref(),
            ) {
                verified.extents.residue += 1;
                verified.extent_detail.push(ExtentOutcome {
                    path: record.path.clone(),
                    line: record.line,
                    id: Some(record.id.clone()),
                    residue,
                });
            }
        }
        outcome
    }

    /// Hash, title, fields and task box of a definition found; whether any
    /// of them missed.
    fn compare_found(
        &self,
        record: &ImportRecord,
        found: Found<'_>,
        outcome: &mut RecordOutcome,
        verified: &mut Verified,
    ) -> bool {
        let config = &self.setup.config;
        let Found { doc, node, place } = found;
        let mut miss = false;
        let after = outcome.after.as_deref().unwrap_or_default();
        let after_hash = b3_hash(self.after_text(found, record, after).as_bytes());
        if after_hash == outcome.before_hash {
            outcome.hash = HashOutcome::Matched;
            verified.hashes.matched += 1;
        } else {
            outcome.hash = HashOutcome::Mismatched;
            verified.hashes.mismatched += 1;
            miss = true;
        }
        outcome.after_hash = Some(after_hash);

        if let Some(expected) = expected_title(record, place, self.sides.get(record.path.as_str()))
        {
            let read = match place {
                Place::File => node.title.clone(),
                Place::Section => node
                    .heading
                    .and_then(|heading| heading_title(doc.text(heading)))
                    .map(str::to_owned),
                Place::InPlace => None,
            };
            let matched = read.as_deref() == Some(expected.as_str());
            outcome.title_matched = Some(matched);
            if matched {
                verified.titles.matched += 1;
            } else {
                verified.titles.mismatched += 1;
                miss = true;
            }
        }

        // Values carried into front-matter (a record file, a document
        // record) are what the attribution's positive proof reads (AC-06).
        let front = place == Place::File || record.form == Form::Document;
        let after = after.to_owned();
        let mut tally = |key: Option<&str>, matched: bool, outcome: &mut RecordOutcome| {
            let entry = key.map(|key| (after.clone(), key.to_owned()));
            if matched {
                outcome.fields_matched += 1;
                verified.fields.matched += 1;
                if let (true, Some(entry)) = (front, entry) {
                    verified.compared_keys.insert(entry);
                }
            } else {
                outcome.fields_mismatched += 1;
                verified.fields.mismatched += 1;
                if let (true, Some(entry)) = (front, entry) {
                    verified.mismatched_keys.insert(entry);
                }
            }
            matched
        };

        // A moved section's heading attributes its record file carries,
        // read back (AC-03; a header conflict is not compared).
        if record.form == Form::Section
            && place == Place::File
            && let Some(heading) = self
                .sides
                .get(record.path.as_str())
                .and_then(|side| source_heading(side.raw_line(record.extent[0])))
        {
            for token in kept_attributes(&heading, record, config) {
                let (key, expected) = attribute(token);
                let matched = doc.read_back(key).is(key, expected);
                miss |= !tally(Some(key), matched, outcome);
            }
        }

        // A row's ID cell holding text beyond its ID is one of the fields
        // (`docs/features/import-layout.md` AC-03).
        for (field, key) in carried_fields(record)
            .into_iter()
            .zip(field_keys(record, config))
        {
            let matched = key.as_deref().is_some_and(|key| match place {
                Place::Section => {
                    read_value(doc, node, place, key).as_deref() == Some(field.value.as_str())
                }
                _ => doc.read_back(key).is(key, &field.value),
            });
            miss |= !tally(key.as_deref(), matched, outcome);
        }

        if let Some(checked) = record.task_box {
            let key = config.layout.task_box_key.as_deref();
            let carried = key
                .and_then(|key| read_value(doc, node, place, key))
                .is_some_and(|value| value == checked.to_string());
            outcome.task_box_carried = Some(carried);
            if carried {
                verified.task_box.carried += 1;
            } else {
                verified.task_box.dropped += 1;
                miss |= key.is_some();
            }
        }
        miss
    }

    /// ID nodes of the tree no definition took and no record left in
    /// place accounts for.
    fn extra(&self) -> usize {
        let after_of: BTreeMap<&str, &str> = self
            .setup
            .layout
            .emission
            .documents
            .iter()
            .map(|document| (document.source.as_str(), document.after.as_str()))
            .collect();
        let placed: BTreeSet<(&str, usize, &str)> = self
            .definitions
            .iter()
            .filter(|(_, emitted)| emitted.place.is_some())
            .map(|(record, _)| (record.path.as_str(), record.line, record.id.as_str()))
            .collect();
        let mut in_place: BTreeSet<(&str, &str)> = BTreeSet::new();
        for record in &self.setup.import.records {
            if matches!(record.form, Form::Section | Form::Document)
                && !placed.contains(&(record.path.as_str(), record.line, record.id.as_str()))
                && let Some(after) = after_of.get(record.path.as_str())
            {
                in_place.insert((after, record.id.as_str()));
            }
        }
        let mut extra = 0;
        for (path, doc) in self.docs {
            let taken = self.consumed.get(path.as_str());
            for (at, node) in doc.parsed.nodes.iter().enumerate() {
                let Some(id) = &node.id else {
                    continue;
                };
                if !taken.is_some_and(|taken| taken.contains(&at))
                    && !in_place.contains(&(path.as_str(), id.as_str()))
                {
                    extra += 1;
                }
            }
        }
        extra
    }

    /// Per walked document: its residue less the reshaped blocks against
    /// its source body less the extents moved out, rule S applied.
    fn prose(&self, verified: &mut Verified) {
        for document in &self.setup.layout.emission.documents {
            let Some(side) = self.sides.get(document.source.as_str()) else {
                continue;
            };
            let before = side.text_of(1..=usize::MAX, None);
            let after = self
                .docs
                .get(document.after.as_str())
                .map(|doc| after_text(doc, doc.parsed.body, self.blocks_of(&document.after)));
            let first_difference = match &after {
                Some(after) if *after == before => None,
                Some(after) => Some(
                    before
                        .split('\n')
                        .zip(after.split('\n'))
                        .take_while(|(a, b)| a == b)
                        .count()
                        + 1,
                ),
                None => Some(1),
            };
            verified.prose.documents += 1;
            if first_difference.is_some() {
                verified.prose.mismatched += 1;
            }
            verified.prose_detail.push(ProseOutcome {
                source: document.source.clone(),
                after: document.after.clone(),
                matched: first_difference.is_none(),
                first_difference,
            });
        }
    }

    /// Headers read back by core: documents with one, those it cannot
    /// read, and every source key (by the import's reader) it lacks under
    /// its `key_map` target or as written.
    fn headers(&self, verified: &mut Verified) {
        let setup = self.setup;
        let details: BTreeMap<&str, &import::DocumentDetail> = setup
            .import
            .documents_detail
            .iter()
            .map(|detail| (detail.path.as_str(), detail))
            .collect();
        for document in &setup.layout.emission.documents {
            let Some(doc) = self.docs.get(document.after.as_str()) else {
                continue;
            };
            if doc.parsed.front_matter.is_some() {
                verified.header.documents += 1;
            }
            if doc.header_failed() {
                verified.header.unparseable += 1;
                verified.unparseable.push(document.after.clone());
                continue;
            }
            let Some(detail) = details.get(document.source.as_str()) else {
                continue;
            };
            let keys = doc.keys();
            for entry in &detail.keys {
                let target = entry.target.as_deref().unwrap_or(&entry.written);
                if !keys.contains(target) && !keys.contains(&entry.written) {
                    verified.header.keys_dropped += 1;
                    verified.keys_dropped.push(DroppedKey {
                        path: document.source.clone(),
                        key: entry.written.clone(),
                    });
                }
            }
        }
        for file in setup
            .layout
            .files
            .iter()
            .filter(|file| file.kind == FileKind::Record)
        {
            if self
                .docs
                .get(&file.path)
                .is_some_and(AfterDoc::header_failed)
            {
                verified.header.unparseable += 1;
                verified.unparseable.push(file.path.clone());
            }
        }
    }
}

impl Verifier<'_> {
    /// A source document's in-place document record: (ID as written, ID,
    /// the line the import read it from; 1 for the path).
    fn document_record(&self, source: &str) -> Option<(&str, &str, usize)> {
        self.definitions
            .iter()
            .find(|(record, emitted)| {
                record.path == source
                    && record.form == Form::Document
                    && emitted.place == Some(Place::InPlace)
            })
            .map(|(record, _)| (record.written.as_str(), record.id.as_str(), record.line))
    }

    /// The names a source header's YAML keys take in the tree, in order,
    /// each with whether it is kept as written (no value mapping)
    /// (`docs/features/import-layout.md` AC-05): every one as written
    /// where the before check cannot parse the block (carried verbatim);
    /// else its `key_map` target, as written where the header already
    /// holds that target or, for a key reaching `id`, unless the document
    /// record's ID was read from that key and core reads its value as that
    /// ID alone (`document`).
    fn yaml_names(
        &self,
        keys: &[&KeyEntry],
        src: &AfterDoc,
        document: Option<(&str, &str, usize)>,
    ) -> Vec<(String, bool)> {
        if src.header_failed() {
            return keys
                .iter()
                .map(|entry| (entry.written.clone(), true))
                .collect();
        }
        let import = &self.setup.config.import;
        let id_key = CoreKey::Id.name();
        let written: BTreeSet<&str> = keys.iter().map(|entry| entry.written.as_str()).collect();
        let mut names: Vec<(String, bool)> = Vec::new();
        for entry in keys {
            let target = import
                .key_map
                .get(&entry.written)
                .cloned()
                .unwrap_or_else(|| entry.written.clone());
            let alone = document.is_some_and(|(id_written, _, line)| {
                line == entry.line
                    && src.read_back(&entry.written) == ReadBack::Scalar(id_written.to_owned())
            });
            let as_written = target != entry.written && target == id_key && !alone;
            let collides = target != entry.written
                && (names.iter().any(|(name, _)| *name == target)
                    || written.contains(target.as_str()));
            names.push(if as_written || collides {
                (entry.written.clone(), true)
            } else {
                (target, false)
            });
        }
        names
    }

    /// The YAML-block entries of each residue's header
    /// (`docs/features/import-layout.md` AC-05): every key its source
    /// header holds once, under the name the layout gives it
    /// ([`Self::yaml_names`]), read back by core and compared with the
    /// source's value as
    /// core reads it there: its `value_map` mapping where one applies (not
    /// to a key kept as written), rule S on a document record's `id`, else
    /// as written (a retyped scalar keeps its text); a value core reads as
    /// no scalar, by its text after the key. Counted in `fields`, detail
    /// `headers.json`; a key the tree lacks is `keys_dropped`'s.
    fn header_values(&self, verified: &mut Verified) {
        let setup = self.setup;
        let import = &setup.config.import;
        let id_key = CoreKey::Id.name();
        let details: BTreeMap<&str, &import::DocumentDetail> = setup
            .import
            .documents_detail
            .iter()
            .map(|detail| (detail.path.as_str(), detail))
            .collect();
        for document in &setup.layout.emission.documents {
            let source = document.source.as_str();
            let (Some(side), Some(detail)) = (self.sides.get(source), details.get(source)) else {
                continue;
            };
            let rows: BTreeSet<usize> = side.body.field_rows.iter().copied().collect();
            let keys: Vec<_> = detail
                .keys
                .iter()
                .filter(|entry| !rows.contains(&entry.line))
                .collect();
            if keys.is_empty() {
                continue;
            }
            let src = AfterDoc::new(source, side.text.as_bytes().to_vec(), &setup.before);
            let document_record = self.document_record(source);
            // The names the keys take, in order: (name, kept as written).
            let names = self.yaml_names(&keys, &src, document_record);
            verified
                .carried_after
                .entry(document.after.clone())
                .or_default()
                .extend(names.iter().map(|(name, _)| name.clone()));
            let Some(tree) = self.docs.get(document.after.as_str()) else {
                continue;
            };
            if tree.header_failed() || src.header_failed() {
                continue;
            }
            let values: BTreeMap<usize, &str> = detail
                .values
                .iter()
                .map(|value| (value.line, value.written.as_str()))
                .collect();
            for (entry, (name, kept)) in keys.iter().zip(&names) {
                let once = names.iter().filter(|(other, _)| other == name).count() == 1
                    && keys
                        .iter()
                        .filter(|other| other.written == entry.written)
                        .count()
                        == 1;
                let (true, Some(before), Some(after)) = (
                    once,
                    src.entries.iter().find(|before| before.first == entry.line),
                    tree.entry(name),
                ) else {
                    continue;
                };
                // Carried verbatim, a form the layout does not rewrite
                // (`docs/features/import-layout.md` AC-06): the tree holds
                // the source's text after the key, and the source entry is
                // multi-line or opens a block, tag, anchor, alias or flow
                // value; a single-line scalar the layout must retype (D3)
                // is no verbatim carry.
                let verbatim = before.rest == after.rest
                    && (before.first != before.last
                        || before
                            .value()
                            .starts_with(['|', '>', '!', '&', '*', '[', '{']));
                if verbatim {
                    verified
                        .verbatim_keys
                        .insert((document.after.clone(), name.clone()));
                }
                // Rule S on a document record's `id` (AC-05).
                let rule_s = |text: &str| match document_record {
                    Some((written, id, _)) if *name == id_key && written != id => {
                        text.replacen(written, id, 1)
                    }
                    _ => text.to_owned(),
                };
                let (expected, read, matched) = match src.read_back(&entry.written) {
                    ReadBack::Scalar(value) => {
                        let mapped =
                            values
                                .get(&entry.line)
                                .filter(|_| !kept)
                                .and_then(|written| {
                                    import
                                        .value_map
                                        .get(name)
                                        .and_then(|map| map.get(*written))
                                        .filter(|mapped| mapped.as_str() != *written)
                                });
                        let expected = rule_s(mapped.map_or(value.as_str(), String::as_str));
                        let read = tree.read_back(name);
                        // A quoted scalar holding a YAML escape (`\` in
                        // double quotes, `''` in single ones) the layout
                        // keeps one item under a reference-list key: that
                        // item equal to the whole value reads back alike
                        // (`docs/features/import-layout.md` AC-05).
                        let escaped = mapped.is_none()
                            && match before.value().chars().next() {
                                Some('"') => before.value().contains('\\'),
                                Some('\'') => before.value().contains("''"),
                                _ => false,
                            };
                        let matched = read.is(name, &expected)
                            || (escaped
                                && core_type(name) == Some(CoreType::ReferenceList)
                                && read.is_one_item(&expected));
                        (expected, read.shown(), matched)
                    }
                    _ => (
                        rule_s(before.value()),
                        Some(after.value().to_owned()),
                        rule_s(&before.rest) == after.rest,
                    ),
                };
                verified.tally_value(
                    FieldValue {
                        path: source.to_owned(),
                        line: entry.line,
                        form: HeaderForm::Yaml,
                        key: name.clone(),
                        expected,
                        read,
                        matched,
                    },
                    &document.after,
                );
            }
        }
    }

    /// The field tables (`docs/features/import-layout.md` AC-04, AC-05),
    /// one extent each, the header's: every row less the key and value
    /// cells the import reads from it leaves no letter or digit (an
    /// empty-key row, a third cell do); every value, carried under its
    /// key's name (a repeated key as `<key>-<n>`), read back by core,
    /// equals its cell as written or its `value_map` mapping (a field;
    /// detail `headers.json`).
    fn field_tables(&self, verified: &mut Verified) {
        let setup = self.setup;
        let import = &setup.config.import;
        let id_key = CoreKey::Id.name();
        let details: BTreeMap<&str, &import::DocumentDetail> = setup
            .import
            .documents_detail
            .iter()
            .map(|detail| (detail.path.as_str(), detail))
            .collect();
        for document in &setup.layout.emission.documents {
            let source = document.source.as_str();
            let Some(side) = self.sides.get(source) else {
                continue;
            };
            if side.body.field_rows.is_empty() {
                continue;
            }
            // The extent opens at the table's header line (AC-04).
            let first = side
                .body
                .field_table_line
                .or_else(|| side.body.field_rows.first().copied())
                .unwrap_or_default();
            let rows: BTreeSet<usize> = side.body.field_rows.iter().copied().collect();
            let detail = details.get(source);
            let entries = || detail.into_iter().flat_map(|detail| detail.keys.iter());
            let keys: BTreeMap<usize, &str> = entries()
                .filter(|entry| rows.contains(&entry.line))
                .map(|entry| (entry.line, entry.written.as_str()))
                .collect();
            let values: BTreeMap<usize, &str> = detail
                .into_iter()
                .flat_map(|detail| detail.values.iter())
                .filter(|value| rows.contains(&value.line))
                .map(|value| (value.line, value.written.as_str()))
                .collect();
            let target_of = |key: &str| {
                import
                    .key_map
                    .get(key)
                    .cloned()
                    .unwrap_or_else(|| key.to_owned())
            };
            let document_record = self.document_record(source);
            // The names the YAML block's keys take in the tree.
            let header_keys: Vec<&KeyEntry> = entries()
                .filter(|entry| !rows.contains(&entry.line))
                .collect();
            let src = AfterDoc::new(source, side.text.as_bytes().to_vec(), &setup.before);
            let mut seen: BTreeSet<String> = self
                .yaml_names(&header_keys, &src, document_record)
                .into_iter()
                .map(|(name, _)| name)
                .collect();
            let doc = self.docs.get(document.after.as_str());
            verified.extents.total += 1;
            let mut residue = String::new();
            for &line in &side.body.field_rows {
                let cells = table_cells(side.raw_line(line));
                let Some(key) = keys.get(&line) else {
                    residue.push_str(&cells.join(" "));
                    residue.push(' ');
                    continue;
                };
                for cell in cells.iter().skip(2) {
                    residue.push_str(cell);
                    residue.push(' ');
                }
                let target = target_of(key);
                let cell = cells.get(1).copied().unwrap_or_default();
                // A key reaching `id` the import did not read the document's
                // ID from alone keeps its written name; a repeated key is
                // carried as `<key>-<n>`, its cell as written (AC-05).
                let as_written = target != *key
                    && target == id_key
                    && !document_record
                        .is_some_and(|(written, _, defining)| defining == line && cell == written);
                let carried = if as_written {
                    (*key).to_owned()
                } else {
                    target
                };
                let repeat = seen.contains(&carried);
                let name = if repeat {
                    let mut n = 2;
                    while seen.contains(&format!("{carried}-{n}")) {
                        n += 1;
                    }
                    format!("{carried}-{n}")
                } else {
                    carried
                };
                seen.insert(name.clone());
                verified
                    .carried_after
                    .entry(document.after.clone())
                    .or_default()
                    .insert(name.clone());
                let mapped = values
                    .get(&line)
                    .filter(|_| !repeat && !as_written)
                    .and_then(|written| {
                        import
                            .value_map
                            .get(&name)
                            .and_then(|map| map.get(*written))
                            .filter(|mapped| mapped.as_str() != *written)
                    });
                let expected = match (mapped, document_record) {
                    (Some(mapped), _) => mapped.clone(),
                    (None, Some((written, id, _))) if name == id_key && written != id => {
                        cell.replacen(written, id, 1)
                    }
                    _ => cell.to_owned(),
                };
                let read = doc.map_or(ReadBack::Nothing, |doc| doc.read_back(&name));
                verified.tally_value(
                    FieldValue {
                        path: source.to_owned(),
                        line,
                        form: HeaderForm::FieldTable,
                        matched: read.is(&name, &expected),
                        key: name,
                        expected,
                        read: read.shown(),
                    },
                    &document.after,
                );
            }
            if residue
                .chars()
                .any(|c| c.is_alphabetic() || c.is_ascii_digit())
            {
                verified.extents.residue += 1;
                verified.extent_detail.push(ExtentOutcome {
                    path: source.to_owned(),
                    line: first,
                    id: None,
                    residue: residue.trim().to_owned(),
                });
            }
        }
    }
}

/// A reshaped block's lines in its tree file: its heading to its span's end.
fn block_lines(doc: &AfterDoc, node: &Node) -> (usize, usize) {
    let start = doc.line_of(node.heading.unwrap_or(node.span).start);
    let end = doc.line_of(node.span.end.saturating_sub(1).max(node.span.start));
    (start, end.max(start))
}

/// The lines of `span` less the reshaped blocks in it (each with the blank
/// line the emitter put before it), normalised by `document_text`.
fn after_text(doc: &AfterDoc, span: Span, blocks: &[(usize, usize)]) -> String {
    let lines = doc.lines(span);
    let (Some(first), Some(last)) = (
        lines.first().map(|line| line.0),
        lines.last().map(|line| line.0),
    ) else {
        return String::new();
    };
    let mut removed: BTreeSet<usize> = BTreeSet::new();
    for &(heading, end) in blocks {
        if heading < first || heading > last {
            continue;
        }
        removed.extend(heading..=end);
        if let Some((_, line)) = lines.iter().find(|(number, _)| *number + 1 == heading)
            && is_blank(line)
        {
            removed.insert(heading - 1);
        }
    }
    let kept: Vec<&str> = lines
        .iter()
        .filter(|(number, _)| !removed.contains(number))
        .map(|(_, line)| *line)
        .collect();
    document_text(&kept.join("\n"))
}

/// A line of only spaces and tabs.
fn is_blank(line: &str) -> bool {
    line.bytes().all(|byte| byte == b' ' || byte == b'\t')
}

/// The title a definition's after form carries, when it has one to compare:
/// a titled list item's title; a section's title in its record file, read
/// from the source heading's bytes ([`source_heading`]), not through the
/// emitter.
fn expected_title(
    record: &ImportRecord,
    place: Place,
    side: Option<&SourceSide<'_>>,
) -> Option<String> {
    match (record.form, place) {
        (Form::ListItem, Place::File | Place::Section) => {
            record.title.clone().filter(|title| !title.is_empty())
        }
        (Form::Section, Place::File) => side
            .and_then(|side| source_heading(side.raw_line(record.extent[0])))
            .map(|heading| heading.title())
            .filter(|title| !title.is_empty()),
        _ => None,
    }
}

/// A `{#…}` section heading of the source as its bytes read
/// (`docs/features/import-layout.md` AC-03): the text before and after the
/// block the import recognises (the first `{#` to the next `}`), the ATX
/// marker and closing sequence left out, each side trimmed; the block's
/// anchor and its other tokens.
struct SourceHeading<'r> {
    sides: [&'r str; 2],
    anchor: &'r str,
    tokens: Vec<&'r str>,
}

impl SourceHeading<'_> {
    /// The sides joined by one space.
    fn title(&self) -> String {
        self.sides
            .iter()
            .filter(|side| !side.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Reads an ATX heading line holding a `{#…}` block; `None` for any other.
fn source_heading(raw: &str) -> Option<SourceHeading<'_>> {
    let content = raw.trim_start_matches(' ');
    let marks = content.bytes().take_while(|&byte| byte == b'#').count();
    if raw.len() - content.len() > 3 || !(1..=6).contains(&marks) {
        return None;
    }
    let mut rest = &content[marks..];
    if !(rest.is_empty() || rest.starts_with([' ', '\t'])) {
        return None;
    }
    rest = rest.trim_end_matches([' ', '\t']);
    let unclosed = rest.trim_end_matches('#');
    if unclosed.len() < rest.len() && (unclosed.is_empty() || unclosed.ends_with([' ', '\t'])) {
        rest = unclosed;
    }
    let open = rest.find("{#")?;
    let close = open + rest[open..].find('}')?;
    let mut tokens = rest[open + 2..close].split_whitespace();
    let anchor = tokens.next()?;
    Some(SourceHeading {
        sides: [rest[..open].trim(), rest[close + 1..].trim()],
        anchor,
        tokens: tokens.collect(),
    })
}

/// A heading attribute token as `(key, value)`; a bare one reads `true`.
fn attribute(token: &str) -> (&str, &str) {
    token.split_once('=').unwrap_or((token, TRUE))
}

/// The text a bare heading attribute is carried as.
const TRUE: &str = "true";

/// A moved section's heading attributes its record file carries
/// (`docs/features/import-layout.md` AC-03), tokens as written: the first
/// occurrence of each key that is neither a key the record file writes
/// itself (`id`, `class`, `title`, `aliases`, the task key) nor a field's.
/// The others are header conflicts (`header.conflicts`), neither compared
/// nor extent residue.
fn kept_attributes<'h>(
    heading: &SourceHeading<'h>,
    record: &ImportRecord,
    config: &CensusConfig,
) -> Vec<&'h str> {
    let mut taken: BTreeSet<String> = [
        CoreKey::Id,
        CoreKey::Class,
        CoreKey::Title,
        CoreKey::Aliases,
    ]
    .into_iter()
    .map(CoreKey::name)
    .collect();
    taken.extend(config.layout.task_box_key.clone());
    taken.extend(field_keys(record, config).into_iter().flatten());
    heading
        .tokens
        .iter()
        .copied()
        .filter(|token| taken.insert(attribute(token).0.to_owned()))
        .collect()
}

/// Whether a section node stands where its definition's placement puts it
/// (`docs/features/import-layout.md` AC-02): an in-place section at its own
/// heading line as written (rule S applied); a reshaped block at level
/// min(n + 1, 6), n the import scanner's nearest heading above the extent
/// (none: 0), on a line no source heading reads.
fn placed(
    doc: &AfterDoc,
    node: &Node,
    record: &ImportRecord,
    place: Place,
    side: Option<&SourceSide<'_>>,
) -> bool {
    let Some(side) = side else {
        return true;
    };
    let line = node
        .heading
        .map(|heading| doc.line(doc.line_of(heading.start)));
    match (place, record.form) {
        (Place::InPlace, Form::Section) => {
            let raw = side.raw_line(record.line);
            let written = if record.written == record.id {
                None
            } else {
                s_heading(raw, &record.written, &record.id)
            };
            line == Some(written.as_deref().unwrap_or(raw))
        }
        (Place::Section, _) => {
            let above = side
                .body
                .headings
                .iter()
                .rev()
                .find(|(number, _)| *number < record.extent[0])
                .map_or(0, |(_, level)| *level);
            node.level.map(usize::from) == Some((above + 1).min(6))
                && !line.is_some_and(|line| {
                    side.body
                        .headings
                        .iter()
                        .any(|(number, _)| side.raw_line(*number) == line)
                })
        }
        _ => true,
    }
}

/// The bytes of an ATX heading between its marker and ` {#`.
fn heading_title(raw: &str) -> Option<&str> {
    let content = raw.trim_start_matches(' ');
    let rest = content.trim_start_matches('#');
    let rest = rest.strip_prefix([' ', '\t']).unwrap_or(rest);
    rest.find(" {#").map(|end| &rest[..end])
}

/// The value core read under `key`: the record file's front-matter, or the
/// reshaped heading's attribute.
fn read_value(doc: &AfterDoc, node: &Node, place: Place, key: &str) -> Option<String> {
    match place {
        Place::Section => node
            .attrs
            .iter()
            .find(|(name, _)| name == key)
            .and_then(|(_, value)| value.clone()),
        _ => doc.front_value(key),
    }
}

/// Whether core and the import scanner read different heading lines
/// around a definition's extent: from the scanner's nearest heading above
/// it to its first heading below it.
fn reader_boundary<'p>(
    side: &SourceSide<'_>,
    record: &'p ImportRecord,
    before: &CheckSetup,
    cache: &mut BTreeMap<&'p str, BTreeSet<usize>>,
) -> bool {
    let core = cache.entry(record.path.as_str()).or_insert_with(|| {
        let bytes = side.text.as_bytes();
        let parsed = specengine_core::parse(&record.path, bytes, &before.project.scheme);
        let line_of = |offset: usize| {
            bytes
                .get(..offset)
                .map_or(0, |head| head.iter().filter(|byte| **byte == b'\n').count())
                + 1
        };
        let mut lines: BTreeSet<usize> = parsed
            .anchors
            .iter()
            .filter(|anchor| anchor.origin == AnchorOrigin::Slug)
            .map(|anchor| line_of(anchor.span.start))
            .collect();
        lines.extend(
            parsed
                .sections()
                .iter()
                .filter_map(|node| node.heading)
                .map(|heading| line_of(heading.start)),
        );
        lines
    });
    let scanner: BTreeSet<usize> = side.body.headings.iter().map(|(line, _)| *line).collect();
    let from = scanner
        .range(..=record.extent[0])
        .next_back()
        .copied()
        .unwrap_or(1);
    let to = scanner
        .range(record.extent[1] + 1..)
        .next()
        .copied()
        .unwrap_or(usize::MAX);
    let window = from..=to;
    let ours: BTreeSet<usize> = scanner
        .iter()
        .filter(|line| window.contains(line))
        .copied()
        .collect();
    let theirs: BTreeSet<usize> = core
        .iter()
        .filter(|line| window.contains(line))
        .copied()
        .collect();
    ours != theirs
}

/// An extent moved out, less its text, title, written ID, cells, task box
/// and the configured separators, when what is left holds a letter or a
/// digit that is no ordered-list marker.
fn extent_residue(
    side: &SourceSide<'_>,
    record: &ImportRecord,
    separators: &[String],
    heading: Option<&str>,
) -> Option<String> {
    let raw = side.raw(record.extent[0]..=record.extent[1]);
    let mut rest = match (record.form, heading) {
        (Form::TableRow | Form::HeaderlessRow, _) => row_residue(&raw, record),
        (Form::Section, Some(heading)) => {
            // The heading line apart (AC-04), the body less the text after it.
            let mut body = side.raw(record.extent[0] + 1..=record.extent[1]);
            let text = record.text.split_once('\n').map_or("", |(_, body)| body);
            cut(&mut body, text);
            format!("{heading} {body}")
        }
        _ => {
            let mut rest = raw;
            cut(&mut rest, &record.text);
            cut(&mut rest, &record.written);
            if let Some(title) = &record.title {
                cut(&mut rest, title);
            }
            if record.task_box.is_some() {
                for marker in ["[x]", "[X]", "[ ]"] {
                    cut(&mut rest, marker);
                }
            }
            rest
        }
    };
    for separator in separators.iter().filter(|separator| !separator.is_empty()) {
        rest = rest.replace(separator.as_str(), " ");
    }
    // An ordered-list marker opening the first line.
    let trimmed = rest.trim_start();
    let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
    let rest = if digits > 0 && trimmed[digits..].starts_with(['.', ')']) {
        trimmed[digits + 1..].to_owned()
    } else {
        rest
    };
    rest.chars()
        .any(|c| c.is_alphabetic() || c.is_ascii_digit())
        .then(|| rest.trim().to_owned())
}

/// Replaces the first occurrence of `piece` in `text` by a blank.
fn cut(text: &mut String, piece: &str) {
    if !piece.is_empty()
        && let Some(at) = text.find(piece)
    {
        text.replace_range(at..at + piece.len(), " ");
    }
}

/// What of a moved section's heading line its record file does not carry
/// (`docs/features/import-layout.md` AC-04): the line's title less the
/// title read back, its anchor unless it is the written ID, and every
/// carried attribute not read back alike (a header conflict is none);
/// nothing for a section not found.
fn heading_residue(
    side: &SourceSide<'_>,
    record: &ImportRecord,
    found: Option<Found<'_>>,
    config: &CensusConfig,
) -> String {
    let (Some(found), Some(heading)) = (found, source_heading(side.raw_line(record.extent[0])))
    else {
        return String::new();
    };
    let mut rest = heading.title();
    if let Some(title) = found.doc.front_value(&CoreKey::Title.name()) {
        cut(&mut rest, &title);
    }
    if heading.anchor != record.written {
        rest.push(' ');
        rest.push_str(heading.anchor);
    }
    for token in kept_attributes(&heading, record, config) {
        let (key, expected) = attribute(token);
        if !found.doc.read_back(key).is(key, expected) {
            rest.push(' ');
            rest.push_str(token);
        }
    }
    rest
}

/// A table row's cells as written: split at the pipes no backslash
/// escapes, the empty pieces outside the outer pipes dropped, each trimmed.
fn table_cells(row: &str) -> Vec<&str> {
    let mut cells = Vec::new();
    let mut start = 0;
    let mut escaped = false;
    for (at, c) in row.char_indices() {
        if c == '|' && !escaped {
            cells.push(&row[start..at]);
            start = at + 1;
        }
        escaped = c == '\\' && !escaped;
    }
    cells.push(&row[start..]);
    if row.trim_end().ends_with('|') && cells.last().is_some_and(|last| is_blank(last)) {
        cells.pop();
    }
    if row.trim_start().starts_with('|') && cells.first().is_some_and(|first| is_blank(first)) {
        cells.remove(0);
    }
    cells.into_iter().map(str::trim).collect()
}

/// A row less its cells: each cell (split at the pipes no backslash
/// escapes) equal to the text or to a carried field's value (an ID cell
/// with text beyond its ID included, `docs/features/import-layout.md`
/// AC-04) goes whole, the ID is cut from the others; what is left, pipes
/// dropped.
fn row_residue(row: &str, record: &ImportRecord) -> String {
    let mut values: Vec<&str> = std::iter::once(record.text.as_str())
        .chain(
            carried_fields(record)
                .into_iter()
                .map(|field| field.value.as_str()),
        )
        .collect();
    let mut cells: Vec<String> = Vec::new();
    let mut cell = String::new();
    let mut escaped = false;
    for c in row.chars() {
        if c == '|' && !escaped {
            cells.push(std::mem::take(&mut cell));
        } else {
            cell.push(c);
        }
        escaped = c == '\\' && !escaped;
    }
    cells.push(cell);
    let mut rest = String::new();
    for cell in cells {
        let trimmed = cell.trim();
        if let Some(at) = values.iter().position(|value| *value == trimmed) {
            values.swap_remove(at);
            continue;
        }
        let mut left = trimmed.to_owned();
        cut(&mut left, &record.written);
        rest.push_str(&left);
        rest.push(' ');
    }
    rest
}

// ---------------------------------------------------------------------------
// Attribution.

/// Who caused a finding of the tree (`docs/features/import-layout.md`
/// AC-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Cause {
    Source,
    Layout,
    Emitter,
}

struct Attribution<'a> {
    /// Tree path → source document (residues and record files).
    source_of: BTreeMap<&'a str, &'a str>,
    record_files: BTreeSet<&'a str>,
    /// Residue files: each source document's `after` path.
    residues: BTreeSet<&'a str>,
    /// (code, path, subject) of the before check.
    before: BTreeSet<(&'a str, &'a str, &'a str)>,
    texts: BTreeMap<&'a str, &'a str>,
    /// IDs defined twice in the model or by a reference-role section.
    duplicates: BTreeSet<&'a str>,
    found: &'a BTreeSet<String>,
    defined_in: BTreeMap<&'a str, BTreeSet<&'a str>>,
    /// Source document → the after names of its header keys.
    carried: BTreeMap<&'a str, BTreeSet<String>>,
    /// Source documents with a header.
    headers: BTreeSet<&'a str>,
    /// Tree path → field and attribute keys of the definitions placed there.
    fields_at: BTreeMap<&'a str, BTreeSet<String>>,
    /// Tree path → core keys the layout added to its header.
    added: BTreeMap<&'a str, BTreeSet<&'a str>>,
    task_key: Option<&'a str>,
    /// Tree paths holding an in-place document record (its `id` added).
    document_records: BTreeSet<&'a str>,
    /// The tree read back, for a carried value's text.
    docs: &'a BTreeMap<String, AfterDoc>,
    /// The verifier's comparisons: (tree path, key) of carried values that
    /// did not read back alike, that did, those carried verbatim; the keys
    /// each residue carries under their tree names.
    mismatched_keys: &'a BTreeSet<(String, String)>,
    compared_keys: &'a BTreeSet<(String, String)>,
    verbatim_keys: &'a BTreeSet<(String, String)>,
    carried_after: &'a BTreeMap<String, BTreeSet<String>>,
    config: &'a CensusConfig,
    /// Source documents moved, and those a section was cut from.
    moved: BTreeSet<&'a str>,
    section_cut: BTreeSet<&'a str>,
    sources: BTreeSet<&'a str>,
    legacy: &'a BTreeMap<String, String>,
    emitted: &'a CheckSetup,
}

impl<'a> Attribution<'a> {
    fn new(
        setup: &'a Setup,
        before: &'a [Finding],
        verified: &'a Verified,
        emitted: &'a CheckSetup,
        docs: &'a BTreeMap<String, AfterDoc>,
    ) -> Self {
        let layout = &setup.layout;
        let config = &setup.config;
        let mut source_of: BTreeMap<&str, &str> = BTreeMap::new();
        for document in &layout.emission.documents {
            source_of.insert(document.after.as_str(), document.source.as_str());
        }
        let mut fields_at: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        let records: BTreeMap<(&str, usize, &str), &ImportRecord> = setup
            .import
            .records
            .iter()
            .filter(|record| record.role == Role::Definition)
            .map(|record| {
                (
                    (record.path.as_str(), record.line, record.id.as_str()),
                    record,
                )
            })
            .collect();
        let texts: BTreeMap<&str, &str> = setup
            .sources
            .iter()
            .map(|source| (source.path.as_str(), source.text.as_str()))
            .collect();
        for emitted_record in &layout.emission.definitions {
            let Emitted {
                path,
                line,
                id,
                after: Some(after),
                place: Some(place),
                ..
            } = emitted_record
            else {
                continue;
            };
            if *place == Place::File {
                source_of.insert(after.as_str(), path.as_str());
            }
            let Some(record) = records.get(&(path.as_str(), *line, id.as_str())) else {
                continue;
            };
            let keys = fields_at.entry(after.as_str()).or_default();
            keys.extend(field_keys(record, config).into_iter().flatten());
            if *place == Place::File
                && record.form == Form::Section
                && let Some(text) = texts.get(path.as_str())
                && let Some(raw) = text
                    .trim_start_matches('\u{FEFF}')
                    .lines()
                    .nth(line.saturating_sub(1))
                && let Some(heading) = source_heading(raw)
            {
                keys.extend(
                    kept_attributes(&heading, record, config)
                        .into_iter()
                        .map(|token| attribute(token).0.to_owned()),
                );
            }
        }
        let record_files = layout
            .files
            .iter()
            .filter(|file| file.kind == FileKind::Record)
            .map(|file| file.path.as_str())
            .collect();
        let mut duplicates: BTreeSet<&str> = setup
            .import
            .duplicates
            .iter()
            .map(|duplicate| duplicate.id.as_str())
            .collect();
        duplicates.extend(
            setup
                .import
                .records
                .iter()
                .filter(|record| record.role == Role::Reference && record.form == Form::Section)
                .map(|record| record.id.as_str()),
        );
        let mut defined_in: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for record in records.values() {
            defined_in
                .entry(record.id.as_str())
                .or_default()
                .insert(record.path.as_str());
        }
        let mut carried: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        let mut headers = BTreeSet::new();
        for detail in &setup.import.documents_detail {
            if detail.header != HeaderForm::None {
                headers.insert(detail.path.as_str());
            }
            let keys = carried.entry(detail.path.as_str()).or_default();
            for entry in &detail.keys {
                keys.insert(entry.written.clone());
                if let Some(target) = &entry.target {
                    keys.insert(target.clone());
                }
            }
        }
        let added = layout
            .headers
            .iter()
            .map(|header| {
                (
                    header.after.as_str(),
                    header.added.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        let moved = layout
            .emission
            .documents
            .iter()
            .filter(|document| document.moved)
            .map(|document| document.source.as_str())
            .collect();
        let section_cut = layout
            .emission
            .definitions
            .iter()
            .filter(|emitted| emitted.place == Some(Place::File) && emitted.form == Form::Section)
            .map(|emitted| emitted.path.as_str())
            .collect();
        Self {
            source_of,
            record_files,
            residues: layout
                .emission
                .documents
                .iter()
                .map(|document| document.after.as_str())
                .collect(),
            before: before
                .iter()
                .map(|finding| {
                    (
                        finding.code.as_str(),
                        finding.path.as_str(),
                        finding.subject.as_str(),
                    )
                })
                .collect(),
            sources: texts.keys().copied().collect(),
            texts,
            duplicates,
            found: &verified.found,
            defined_in,
            carried,
            headers,
            fields_at,
            added,
            task_key: config.layout.task_box_key.as_deref(),
            document_records: layout
                .emission
                .definitions
                .iter()
                .filter(|emitted| {
                    emitted.place == Some(Place::InPlace) && emitted.form == Form::Document
                })
                .filter_map(|emitted| emitted.after.as_deref())
                .collect(),
            docs,
            mismatched_keys: &verified.mismatched_keys,
            compared_keys: &verified.compared_keys,
            verbatim_keys: &verified.verbatim_keys,
            carried_after: &verified.carried_after,
            config,
            moved,
            section_cut,
            legacy: &config.import.legacy,
            emitted,
        }
    }

    /// The finding's source document and its cause.
    fn cause(&self, finding: &'a Finding) -> (Option<&'a str>, Cause) {
        let path = finding.path.as_str();
        let source = self.source_of.get(path).copied();
        let code = finding.code.as_str();
        let subject = finding.subject.as_str();
        let before_has =
            || source.is_some_and(|source| self.before.contains(&(code, source, subject)));
        let source_if = |caused: bool| {
            if caused {
                Cause::Source
            } else {
                Cause::Emitter
            }
        };
        // The front-matter key of the entry holding the finding's line.
        let line_key = self.docs.get(path).and_then(|doc| doc.key_at(finding.line));
        // `docs/features/import-layout.md` AC-06, first rule: a finding on
        // a key whose carried value the verifier found rewritten is the
        // emitter's, whatever else holds — the key the subject names, else
        // the key of the finding's line (core names `canon-missing`,
        // `shipped-missing` at the `status` line their value decides).
        let mismatched = |key: &str| {
            self.mismatched_keys
                .contains(&(path.to_owned(), key.to_owned()))
        };
        if mismatched(subject) || line_key.is_some_and(mismatched) {
            return (source, Cause::Emitter);
        }
        let cause = match code {
            FILE_NAME | ID_SCOPE => Cause::Emitter,
            _ if code.starts_with(INDEX_CODES) => Cause::Emitter,
            ID_WIDTH => source_if(self.width_differs(subject)),
            HOMOGLYPH => source_if(
                source
                    .and_then(|source| self.texts.get(source))
                    .is_some_and(|text| !subject.is_empty() && text.contains(subject)),
            ),
            ID_TAKEN | DUPLICATE_ID => {
                source_if(self.duplicates.contains(self.latin(subject).as_str()))
            }
            KEY_MISSING | CANON_MISSING | SHIPPED_MISSING => {
                source_if(self.corpus_absence(path, subject))
            }
            // `docs/features/import-layout.md` AC-06: the emitter's where
            // the tree should hold a `class`: a record file's, a glob's, a
            // mapped source class, one the corpus's header carried.
            CLASS_MISSING => {
                let class = CoreKey::Class.name();
                source_if(
                    !self.emitter_adds(path, &class)
                        && !self.written_by_layout(path, &class)
                        && !self.carries(path, &class),
                )
            }
            CLASS_UNKNOWN => Cause::Source,
            FRONTMATTER_TYPE if self.mistyped_carried(path, subject) => Cause::Source,
            // AC-06: prose carried into a reference key, read back as the
            // corpus wrote it (item-wise under a reference-list key).
            UNPARSED_REFERENCE => {
                source_if(before_has() || line_key.is_some_and(|key| self.corpus_value(path, key)))
            }
            // `docs/features/import-layout.md` AC-06: a path-form `canon:`
            // value the corpus wrote, carried and read back alike (the
            // value rule on the key of its line), is the source's; one
            // naming a document the layout moved, or for an anchor one a
            // section was cut from, the layout's.
            CANON_FORM | CANON_FILE | CANON_ANCHOR => {
                if before_has() {
                    Cause::Source
                } else if !line_key.is_some_and(|key| self.corpus_value(path, key)) {
                    Cause::Emitter
                } else if code != CANON_FORM && self.canon_moved(subject, code == CANON_ANCHOR) {
                    Cause::Layout
                } else {
                    Cause::Source
                }
            }
            REF_DANGLING | MENTION_DANGLING => {
                let id = self.latin(subject);
                let defined_at_source = source.is_some_and(|source| {
                    self.defined_in
                        .get(id.as_str())
                        .is_some_and(|paths| paths.contains(source))
                });
                let feature = self.is_feature(&id);
                if !self.found.contains(&id) || (feature && !defined_at_source) {
                    Cause::Source
                } else if code == MENTION_DANGLING
                    && feature
                    && !subject.contains('/')
                    && self.record_files.contains(path)
                {
                    // `docs/features/import-layout.md` AC-06: a bare
                    // feature-scoped citation the layout moved out of the
                    // feature document defining it (the citing record left
                    // for a record file), like a moved link.
                    Cause::Layout
                } else {
                    Cause::Emitter
                }
            }
            // AC-06: a carried or field key (a repeated field-table key
            // under its `<key>-<n>` name too), or the task key.
            UNKNOWN_KEY => source_if(
                source.is_some_and(|source| {
                    self.carried
                        .get(source)
                        .is_some_and(|keys| keys.contains(subject))
                }) || self.carries(path, subject)
                    || self
                        .fields_at
                        .get(path)
                        .is_some_and(|keys| keys.contains(subject))
                    || self.task_key == Some(subject),
            ),
            // AC-06: the before check's, or a residue whose source
            // document's bytes were already over the cap of its slot.
            BUDGET => source_if(before_has() || self.over_cap(path, source, subject)),
            LINK_DANGLING | LINK_ANCHOR => {
                if before_has() {
                    Cause::Source
                } else if self.link_moved(path, source, subject, code == LINK_ANCHOR) {
                    Cause::Layout
                } else {
                    Cause::Emitter
                }
            }
            _ if code.starts_with(FRONTMATTER_CODES) || code == KEY_EXTRA => {
                let carried_header = !self.record_files.contains(path)
                    && source.is_some_and(|source| self.headers.contains(source));
                let carried_key = code != KEY_EXTRA
                    || (source.is_some_and(|source| {
                        self.carried
                            .get(source)
                            .is_some_and(|keys| keys.contains(subject))
                    }) && !self
                        .added
                        .get(path)
                        .is_some_and(|keys| keys.contains(subject)));
                source_if(carried_header && carried_key && before_has())
            }
            // AC-06: else the before check's, or a finding on the line of
            // the key it names whose value is the corpus's (a contract's
            // or a check rule's value).
            _ => source_if(
                before_has()
                    || (self.docs.get(path).is_some_and(|doc| {
                        doc.entry(subject)
                            .is_some_and(|entry| entry.lines().contains(&finding.line))
                    }) && self.corpus_value(path, subject)),
            ),
        };
        (source, cause)
    }

    /// Whether the layout writes `key` into the tree file at `path` itself
    /// (AC-06): `id` and `class` of a record file, `id` of a document
    /// record, `class` where a `[layout] classes` glob matches the path.
    fn emitter_adds(&self, path: &str, key: &str) -> bool {
        let id = key == CoreKey::Id.name();
        let class = key == CoreKey::Class.name();
        (self.record_files.contains(path) && (id || class))
            || (id && self.document_records.contains(path))
            || (class && self.config.layout.class_of(path).is_some())
    }

    /// Whether the layout writes `key` at `path` itself, not the corpus:
    /// a record file's `id`, `class`, `title`, `aliases` and task key; a
    /// key added to a residue's header.
    fn written_by_layout(&self, path: &str, key: &str) -> bool {
        if self.record_files.contains(path) {
            [
                CoreKey::Id,
                CoreKey::Class,
                CoreKey::Title,
                CoreKey::Aliases,
            ]
            .into_iter()
            .any(|core| core.name() == key)
                || self.task_key == Some(key)
        } else {
            self.added.get(path).is_some_and(|keys| keys.contains(key))
        }
    }

    /// Whether the tree file at `path` carries `key` from the corpus: a
    /// record file a field or attribute key of its definition, a residue a
    /// key of its source document's header under its tree name.
    fn carries(&self, path: &str, key: &str) -> bool {
        if self.record_files.contains(path) {
            self.fields_at
                .get(path)
                .is_some_and(|keys| keys.contains(key))
        } else {
            self.carried_after
                .get(path)
                .is_some_and(|keys| keys.contains(key))
        }
    }

    /// A finding on a value the layout carried with the corpus's text
    /// (`docs/features/import-layout.md` AC-06, e.g.
    /// `tier-invalid` of a mapped column, `value-invalid` of a check rule):
    /// the config and the corpus wrote it so, positively — the verifier
    /// compared it and it read back as the corpus wrote it. A value the
    /// layout writes itself, one that did not read back alike and one never
    /// compared stay the emitter's.
    fn corpus_value(&self, path: &str, key: &str) -> bool {
        let entry = (path.to_owned(), key.to_owned());
        !self.written_by_layout(path, key)
            && self.carries(path, key)
            && self.compared_keys.contains(&entry)
            && !self.mismatched_keys.contains(&entry)
    }

    /// A finding about a required key (`key-missing`, `canon-missing`,
    /// `shipped-missing`; AC-06): source-caused when the corpus lacks the
    /// key too — the tree file holds no entry of it, and the layout writes it
    /// neither always ([`Self::emitter_adds`]; a key it added to a
    /// residue's header) nor from the corpus (a carried key); where the
    /// file holds it (an unreadable `canon:`), when its value is the
    /// corpus's ([`Self::corpus_value`]).
    fn corpus_absence(&self, path: &str, key: &str) -> bool {
        let present = self
            .docs
            .get(path)
            .is_some_and(|doc| doc.entries.iter().any(|entry| entry.key == key));
        if present {
            self.corpus_value(path, key)
        } else {
            let added = !self.record_files.contains(path)
                && self.added.get(path).is_some_and(|keys| keys.contains(key));
            !self.emitter_adds(path, key) && !added && !self.carries(path, key)
        }
    }

    /// A `frontmatter-type` of a typed core key whose value is the corpus's
    /// ([`Self::corpus_value`]) and that core rejects for the corpus's sake
    /// (AC-06): carried verbatim (a block scalar, a tag, a multi-line value
    /// the layout did not rewrite), or written from a text that does not
    /// parse as the key's type (AC-05: quoted; a float such as `2.0` under
    /// an integer key). A value the layout wrote from a text that parses
    /// (a quoted canonical integer it failed to unquote, D3) stays the
    /// emitter's.
    fn mistyped_carried(&self, path: &str, key: &str) -> bool {
        core_type(key).is_some()
            && self.corpus_value(path, key)
            && (self
                .verbatim_keys
                .contains(&(path.to_owned(), key.to_owned()))
                || self
                    .docs
                    .get(path)
                    .and_then(|doc| doc.kept_text(key))
                    .is_some_and(|value| !parses_as(key, &value)))
    }

    /// The tree file at `path` is a residue (its document's `after`) whose
    /// source document's bytes exceed the emitted scheme's cap of `slot` (a
    /// `budget` subject; `docs/features/import-layout.md` AC-06). A record
    /// file holds one record of its source, never the whole document: its
    /// source's size says nothing of its own.
    fn over_cap(&self, path: &str, source: Option<&str>, slot: &str) -> bool {
        if !self.residues.contains(path) {
            return false;
        }
        let budgets = &self.emitted.config.budgets;
        // Core's slots: the index, the canon tiers, any other canon, a
        // decision (the latter two named by their class).
        let caps = [
            (INDEX_SLOT, Some(budgets.index_bytes)),
            (TIER0_SLOT, Some(budgets.tier0_bytes)),
            (TIER1_SLOT, Some(budgets.tier1_bytes)),
            (DocClass::Canon.as_str(), budgets.canon_bytes),
            (DocClass::Decision.as_str(), Some(budgets.decision_bytes)),
        ];
        let cap = caps
            .into_iter()
            .find(|(name, _)| *name == slot)
            .and_then(|(_, cap)| cap);
        let bytes = source
            .and_then(|source| self.texts.get(source))
            .map(|text| text.len());
        match (cap, bytes) {
            (Some(cap), Some(bytes)) => u64::try_from(bytes).is_ok_and(|bytes| bytes > cap),
            _ => false,
        }
    }

    /// The subject's Latin ID: a legacy prefix mapped, a `slug/` and a
    /// `#section` or `@rev` dropped.
    fn latin(&self, subject: &str) -> String {
        let id = subject.rsplit('/').next().unwrap_or(subject);
        let id = id.split(['#', '@']).next().unwrap_or(id);
        match id.split_once('-') {
            Some((prefix, rest)) => match self.legacy.get(prefix) {
                Some(target) => format!("{target}-{rest}"),
                None => id.to_owned(),
            },
            None => id.to_owned(),
        }
    }

    fn is_feature(&self, id: &str) -> bool {
        id.split_once('-')
            .and_then(|(prefix, _)| self.emitted.project.scheme.prefix(prefix))
            .is_some_and(|spec| spec.scope == IdScope::Feature)
    }

    /// The ID's digits differ from its prefix's `width`.
    fn width_differs(&self, subject: &str) -> bool {
        let id = self.latin(subject);
        let Some((prefix, number)) = id.split_once('-') else {
            return false;
        };
        let digits = number.bytes().take_while(u8::is_ascii_digit).count();
        self.emitted
            .project
            .scheme
            .prefix(prefix)
            .and_then(|spec| spec.width)
            .is_some_and(|width| usize::try_from(width).is_ok_and(|width| width != digits))
    }

    /// A path-form `canon:` value (`path[#anchor]`, the corpus root's
    /// path) naming a source document the layout moved, or, for an anchor,
    /// one a section was cut from (AC-06).
    fn canon_moved(&self, subject: &str, anchor: bool) -> bool {
        let target = subject.split('#').next().unwrap_or(subject);
        self.sources.contains(target)
            && (self.moved.contains(target) || (anchor && self.section_cut.contains(target)))
    }

    /// A link the layout broke: its citing document moved (a record file
    /// included), its target document moved, or, for an anchor, a section
    /// was cut from its target.
    fn link_moved(&self, path: &str, source: Option<&str>, subject: &str, anchor: bool) -> bool {
        let Some(source) = source else {
            return false;
        };
        if path != source {
            return true;
        }
        let target = subject.split('#').next().unwrap_or(subject);
        let resolved = if target.is_empty() {
            Some(source.to_owned())
        } else {
            resolve_relative(source, target)
        };
        resolved.is_some_and(|target| {
            self.sources.contains(target.as_str())
                && (self.moved.contains(target.as_str())
                    || (anchor && self.section_cut.contains(target.as_str())))
        })
    }
}

/// A file link of a tree file that reaches no tree file while, written in
/// its source document, it named a walked document — one the layout broke
/// that core does not report (`docs/features/import-layout.md` AC-06;
/// `dangling.links.after`): a `.md` target outside core's walk (T5), or a
/// target core never checks, such as a walked document the layout renamed
/// to `.md` or moved.
#[derive(Debug, Serialize)]
struct LeftLink {
    path: String,
    line: usize,
    target: String,
    source: String,
}

/// The links of the tree that leave it because a file moved, resolved as
/// core resolves a file link (the citing file's directory, then
/// `[paths] link_base`), from the tree file and from its source document.
fn links_leaving(
    setup: &Setup,
    docs: &BTreeMap<String, AfterDoc>,
    emitted: &CheckSetup,
) -> Vec<LeftLink> {
    let layout = &setup.layout;
    let mut source_of: BTreeMap<&str, &str> = layout
        .emission
        .documents
        .iter()
        .map(|document| (document.after.as_str(), document.source.as_str()))
        .collect();
    for emitted_record in &layout.emission.definitions {
        if emitted_record.place == Some(Place::File)
            && let Some(after) = &emitted_record.after
        {
            source_of.insert(after.as_str(), emitted_record.path.as_str());
        }
    }
    let sources: BTreeSet<&str> = setup
        .sources
        .iter()
        .map(|source| source.path.as_str())
        .collect();
    let scope = emitted.project.paths.walk_scope();
    // Core's candidates for `target` written in `from`.
    let candidates = |from: &str, base: Option<&str>, target: &str| -> Vec<String> {
        let directory = from.rsplit_once('/').map_or("", |(directory, _)| directory);
        normalise(directory, target)
            .into_iter()
            .chain(base.and_then(|base| normalise(base, target)))
            .collect()
    };
    let after_base = emitted.project.paths.link_base.as_deref();
    let before_base = setup.before.project.paths.link_base.as_deref();
    let mut left = Vec::new();
    for (path, doc) in docs {
        let Some(source) = source_of.get(path.as_str()) else {
            continue;
        };
        for link in &doc.parsed.links {
            let LinkTarget::Path(target) = &link.dst else {
                continue;
            };
            if link.origin != LinkOrigin::Inline
                || link.link_type != MENTIONS
                || target.path.is_empty()
                || target.path.starts_with('/')
            {
                continue;
            }
            let after = candidates(path, after_base, &target.path);
            // Core checks a `.md` target only; one with a candidate in its
            // walk scope it reports itself (`link-dangling`).
            let checked = target.path.ends_with(DOCUMENT_EXTENSION);
            let left_tree = !after.iter().any(|candidate| {
                docs.contains_key(candidate) || (checked && scope.in_walk_scope(candidate))
            });
            let worked = candidates(source, before_base, &target.path)
                .iter()
                .any(|candidate| sources.contains(candidate.as_str()));
            if left_tree && worked {
                left.push(LeftLink {
                    path: path.clone(),
                    line: target.span.map_or(0, |span| doc.line_of(span.start)),
                    target: target.path.clone(),
                    source: (*source).to_owned(),
                });
            }
        }
    }
    left
}

/// `directory` + `path`, `/`-joined: empty and `.` components dropped,
/// `..` pops; `None` when it pops above the root (core's link rule).
fn normalise(directory: &str, path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for component in directory.split('/').chain(path.split('/')) {
        match component {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    Some(parts.join("/"))
}

/// `target` written in `from`, as a corpus-relative path; `None` when it
/// leaves the root.
fn resolve_relative(from: &str, target: &str) -> Option<String> {
    let mut parts: Vec<String> = from.split('/').map(str::to_owned).collect();
    parts.pop();
    for component in Path::new(target).components() {
        match component {
            Component::RootDir => parts.clear(),
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir | Component::Prefix(_) => {}
        }
    }
    Some(parts.join("/"))
}

// ---------------------------------------------------------------------------
// Detail files.

#[derive(Serialize)]
struct HeadersDetail<'a> {
    outcomes: &'a [layout::HeaderOutcome],
    dropped: &'a [DroppedKey],
    unparseable: &'a [String],
    field_values: &'a [FieldValue],
}

#[derive(Serialize)]
struct AttributedFinding<'a> {
    code: &'a str,
    severity: Severity,
    path: &'a str,
    line: usize,
    subject: &'a str,
    message: &'a str,
    source: Option<&'a str>,
    cause: Cause,
}

#[derive(Serialize)]
struct FindingsDetail<'a> {
    tree: Vec<AttributedFinding<'a>>,
    /// Baseline entries the enforced check matched to nothing.
    stale: &'a [DebtEntry],
    /// Links the layout broke that core does not report.
    left_tree: &'a [LeftLink],
}

#[derive(Serialize)]
struct DiagnosticsDetail<'a> {
    layout: &'a [LayoutDiagnostic],
    verifier: &'a [Note],
}

/// The tamper tests of `docs/features/import-layout.md` (AC-02, AC-03,
/// AC-04, AC-06): the tree is changed on disk between writing and reading
/// back ([`run_tampered`]), and the verifier, reading only the written
/// files, must see each change. A verifier that read the emitter's memory
/// would see none of them.
#[cfg(test)]
mod tamper_tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use specengine_import::layout::{FileKind, Layout, Reason};

    use super::{LayoutResult, prepare, run_tampered};
    use crate::harness::Corpus;

    /// A scratch `--out`, removed on drop; unique per process and call.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "specengine-eval-layout-tamper-{name}-{}-{serial}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch directory");
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/import-layout")
            .join(name)
            .canonicalize()
            .expect("the import-layout fixture exists")
    }

    /// `layout` on a fixture with `tamper` applied to the written tree.
    fn tampered(name: &str, tamper: impl FnOnce(&Path, &Layout)) -> LayoutResult {
        let scratch = Scratch::new(name);
        let root = fixture(name);
        let setup =
            prepare(&root, None, None, Some("2026-10-04"), None).expect("the fixture is accepted");
        let corpus = Corpus {
            label: "tamper".to_owned(),
            root,
            out: scratch.0.clone(),
        };
        run_tampered(&corpus, setup, tamper).expect("the run completes")
    }

    /// The one record file of the tree whose bytes hold `needle`.
    fn record_with<'l>(layout: &'l Layout, needle: &str) -> &'l str {
        let hits: Vec<&str> = layout
            .files
            .iter()
            .filter(|file| file.kind == FileKind::Record && file.content.contains(needle))
            .map(|file| file.path.as_str())
            .collect();
        assert_eq!(hits.len(), 1, "record files holding {needle:?}: {hits:?}");
        hits[0]
    }

    fn edit(tree: &Path, path: &str, change: impl FnOnce(String) -> String) {
        let file = tree.join(path);
        let before = fs::read_to_string(&file).expect("tree file readable");
        let after = change(before.clone());
        assert_ne!(before, after, "{path}: the tamper changed nothing");
        fs::write(&file, after).expect("tree file writable");
    }

    /// Not vacuous: untampered, every definition matches.
    #[test]
    fn untampered_every_emitted_definition_matches() {
        for name in ["one", "two"] {
            let result = tampered(name, |_, _| {});
            assert_eq!(result.hashes.mismatched, 0, "{name}");
            assert_eq!(result.hashes.extra, 0, "{name}");
            assert_eq!(result.prose.mismatched, 0, "{name}");
            assert_eq!(result.fields.mismatched, 0, "{name}");
            assert_eq!(result.check.emitter_findings, 0, "{name}");
            assert!(result.hashes.matched > 0, "{name}");
        }
    }

    /// AC-02: one `\|` unescaped on disk is one mismatched hash; nothing
    /// else moves. (M1: a verifier hashing the emitter's output or the
    /// model sees no change and this test fails.)
    #[test]
    fn an_unescaped_pipe_on_disk_is_one_mismatched_hash() {
        let mut target = String::new();
        let result = tampered("one", |tree, layout| {
            target = record_with(layout, "\\|").to_owned();
            edit(tree, &target, |text| text.replacen("\\|", "|", 1));
        });
        assert!(!target.is_empty());
        assert_eq!(result.hashes.mismatched, 1, "{target}");
        assert_eq!(
            result.hashes.matched,
            result.before.definitions - 1,
            "every other definition still matches"
        );
        assert_eq!(result.hashes.missing, 0);
        assert_eq!(result.hashes.extra, 0);
        assert_eq!(
            result.reasons.values().sum::<usize>(),
            1,
            "the miss has one reason"
        );
        assert_eq!(
            result.reasons[&Reason::Unexplained],
            1,
            "{:?}",
            result.reasons
        );
    }

    /// AC-06: a record file renamed on disk is missing at its map path, an
    /// extra node elsewhere, and core's `file-name` finding on it is the
    /// emitter's (M1: `file-name` attributed to the source turns this red).
    #[test]
    fn a_renamed_record_file_is_a_file_name_finding_of_the_emitter() {
        let mut renamed = String::new();
        let result = tampered("one", |tree, layout| {
            let path = record_with(layout, "Does the engine stop on the first error?");
            let target = Path::new(path).with_file_name("renamed.md");
            fs::rename(tree.join(path), tree.join(&target)).expect("rename");
            renamed = target.display().to_string();
        });
        assert!(!renamed.is_empty());
        assert_eq!(result.hashes.missing, 1, "missing at its map path");
        assert_eq!(result.hashes.extra, 1, "an id: node mapped from no record");
        let file_name = result
            .check
            .findings
            .get("file-name")
            .expect("core's file-name finding on the renamed file");
        assert_eq!(file_name.emitter, 1, "{file_name:?}");
        assert_eq!(file_name.source, 0, "{file_name:?}");
        assert_eq!(file_name.layout, 0, "{file_name:?}");
        assert!(result.check.emitter_findings >= 1);
    }

    /// AC-04 M1: a prose line dropped from a residue on disk is one
    /// mismatched document; no definition's hash moves.
    #[test]
    fn a_dropped_prose_line_is_one_mismatched_document() {
        let result = tampered("one", |tree, layout| {
            let residue = layout
                .files
                .iter()
                .find(|file| {
                    file.kind == FileKind::Residue
                        && file
                            .content
                            .contains("Prose after the table names no record.\n")
                })
                .expect("the residue holding the prose line");
            edit(tree, &residue.path, |text| {
                text.replacen("Prose after the table names no record.\n", "", 1)
            });
        });
        assert_eq!(result.prose.mismatched, 1);
        assert_eq!(result.hashes.mismatched, 0);
        assert_eq!(result.hashes.matched, result.before.definitions);
    }

    /// AC-03: a field line dropped from a record file on disk is one
    /// mismatched field; its hash still matches.
    #[test]
    fn a_dropped_field_line_is_one_mismatched_field() {
        let result = tampered("one", |tree, layout| {
            let path = record_with(layout, "The engine keeps every byte it reads.");
            edit(tree, path, |text| text.replacen("Area: \"store\"\n", "", 1));
        });
        assert_eq!(result.fields.mismatched, 1);
        assert_eq!(result.hashes.mismatched, 0);
        assert_eq!(result.hashes.matched, result.before.definitions);
    }

    /// AC-03, fixture two: a headerless row's `col-N` field dropped on disk.
    #[test]
    fn a_dropped_column_field_is_one_mismatched_field_in_the_second_convention() {
        let result = tampered("two", |tree, layout| {
            let path = record_with(layout, "Keep the log append-only.");
            edit(tree, path, |text| text.replacen("col-0: \"2\"\n", "", 1));
        });
        assert_eq!(result.fields.mismatched, 1);
        assert_eq!(result.hashes.mismatched, 0);
    }

    // ------------------------------------------------ iteration 2 (D1-D7)

    /// Copies `from` into `to`, recursively.
    fn copy_tree(from: &Path, to: &Path) {
        fs::create_dir_all(to).expect("copy target");
        let mut entries: Vec<_> = fs::read_dir(from)
            .expect("readable fixture")
            .map(|entry| entry.expect("fixture entry").path())
            .collect();
        entries.sort();
        for entry in entries {
            let target = to.join(entry.file_name().expect("a name"));
            if entry.is_dir() {
                copy_tree(&entry, &target);
            } else {
                fs::copy(&entry, &target).expect("copy");
            }
        }
    }

    /// `layout` on a scratch copy of a fixture changed by `corpus` before the
    /// run, with `tamper` applied to the written tree; the result and the
    /// attributed tree findings of `findings.json`.
    fn tampered_copy(
        name: &str,
        corpus: impl FnOnce(&Path),
        tamper: impl FnOnce(&Path, &Layout),
    ) -> (LayoutResult, Vec<serde_json::Value>) {
        let scratch = Scratch::new(&format!("{name}-copy"));
        let root = scratch.0.join("corpus");
        copy_tree(&fixture(name), &root);
        corpus(&root);
        let root = root.canonicalize().expect("the copy exists");
        let setup =
            prepare(&root, None, None, Some("2026-10-04"), None).expect("the copy is accepted");
        let out = scratch.0.join("out");
        let run = Corpus {
            label: "tamper".to_owned(),
            root,
            out: out.clone(),
        };
        let result = run_tampered(&run, setup, tamper).expect("the run completes");
        let findings = fs::read_to_string(out.join("layout/tamper/findings.json"))
            .expect("findings.json written");
        let findings: serde_json::Value =
            serde_json::from_str(&findings).expect("findings.json is JSON");
        let tree = findings["tree"].as_array().expect("tree findings").clone();
        (result, tree)
    }

    fn write(path: &Path, text: &str) {
        fs::write(path, text).expect("corpus file writable");
    }

    fn replace_in(path: &Path, from: &str, to: &str) {
        let text = fs::read_to_string(path).expect("corpus file readable");
        assert!(text.contains(from), "{from:?} not in {}", path.display());
        write(path, &text.replacen(from, to, 1));
    }

    /// D1 / AC-05 M2: a value carried from the field table, altered in the
    /// written header, is one mismatched field (`headers.json`
    /// `field_values`); no hash moves (the header is not the body).
    #[test]
    fn a_changed_field_table_value_is_one_mismatched_field() {
        let result = tampered("one", |tree, layout| {
            let plan = layout
                .files
                .iter()
                .find(|file| file.kind == FileKind::Residue && file.content.contains("# Plan\n"))
                .expect("the field-table document");
            edit(tree, &plan.path, |text| {
                text.replacen("owner: \"team\"\n", "owner: \"crew\"\n", 1)
            });
        });
        assert_eq!(result.fields.mismatched, 1);
        assert_eq!(result.hashes.mismatched, 0);
        assert_eq!(result.hashes.matched, result.before.definitions);
        assert_eq!(result.extents.residue, 0);
    }

    /// D2: a moved section's heading attribute dropped from its record file
    /// is one mismatched field (attributes are compared after read-back).
    #[test]
    fn a_dropped_heading_attribute_is_one_mismatched_field() {
        let result = tampered("one", |tree, layout| {
            let path = record_with(layout, "The gate text moves to a record file.");
            edit(tree, path, |text| text.replacen("level: \"two\"\n", "", 1));
        });
        assert_eq!(result.fields.mismatched, 1);
        assert_eq!(result.titles.mismatched, 0);
        assert_eq!(result.hashes.mismatched, 0);
    }

    /// D2 / AC-03 M2: a title written cut at the `{…}` block (the words
    /// after it lost) is one mismatched title: the expected title comes from
    /// the source bytes, the read one from the file.
    #[test]
    fn a_title_cut_at_the_anchor_block_is_one_mismatched_title() {
        let (result, _) = tampered_copy(
            "one",
            |corpus| {
                write(
                    &corpus.join("book/titles.md"),
                    "# Titles\n\n## Big {#NEED-42 level=three} trailing words\n\nWords follow the block.\n",
                );
            },
            |tree, layout| {
                let path = record_with(layout, "Words follow the block.");
                edit(tree, path, |text| {
                    text.replacen("title: \"Big trailing words\"\n", "title: \"Big\"\n", 1)
                });
            },
        );
        assert_eq!(result.titles.mismatched, 1);
        assert_eq!(result.hashes.mismatched, 0);
        assert_eq!(result.fields.mismatched, 0);
    }

    /// D3 / Q1: a typed core key's value that did not read back as the
    /// corpus wrote it (here changed in the written header) is the
    /// emitter's finding (`tier-invalid` `emitter`), never the source's;
    /// the same key read back as written is the source's (layout_cli.rs).
    #[test]
    fn a_typed_value_that_does_not_read_back_is_the_emitter_s_finding() {
        let (result, findings) = tampered_copy(
            "one",
            |corpus| {
                replace_in(
                    &corpus.join("census.toml"),
                    "\"Weight\" = \"weight\"\n",
                    "\"Weight\" = \"weight\"\n\"Tier\" = \"tier\"\n",
                );
                write(
                    &corpus.join("book/tier-table.md"),
                    "| Attribute | Value |\n|-----------|-------|\n| Tier | 1 |\n\n# Table tier\n\nA tier from a field table.\n",
                );
            },
            |tree, _| {
                edit(tree, "book/tier-table.md", |text| {
                    text.replacen("tier: 1\n", "tier: 5\n", 1)
                });
            },
        );
        assert_eq!(result.fields.mismatched, 1, "the tier read back as 5");
        let tier: Vec<&serde_json::Value> = findings
            .iter()
            .filter(|finding| finding["code"] == "tier-invalid")
            .collect();
        assert_eq!(tier.len(), 1, "{findings:?}");
        assert_eq!(tier[0]["path"], "book/tier-table.md");
        assert_eq!(tier[0]["cause"], "emitter", "{:?}", tier[0]);
        assert_eq!(result.check.findings["tier-invalid"].emitter, 1);
        assert_eq!(result.check.findings["tier-invalid"].source, 0);
        assert!(result.check.emitter_findings >= 1);
    }

    /// D7: `class-missing` on a residue whose after path a `[layout]
    /// classes` glob matches is the emitter's (the layout writes that class
    /// itself), even though the before check has `class-missing` on the
    /// source document.
    #[test]
    fn class_missing_where_a_classes_glob_matches_is_the_emitter_s() {
        let (result, findings) = tampered_copy(
            "one",
            |_| {},
            |tree, _| {
                edit(tree, "book/glossary.md", |text| {
                    text.replacen("---\nclass: canon\n---\n", "", 1)
                });
            },
        );
        let missing: Vec<&serde_json::Value> = findings
            .iter()
            .filter(|finding| {
                finding["code"] == "class-missing" && finding["path"] == "book/glossary.md"
            })
            .collect();
        assert_eq!(missing.len(), 1, "{findings:?}");
        assert_eq!(missing[0]["cause"], "emitter", "{:?}", missing[0]);
        assert_eq!(result.check.findings["class-missing"].emitter, 1);
        assert!(result.check.emitter_findings >= 1);
    }

    // ------------------------------------- iteration 3 (R1-R5, S1, S2)

    /// The attributed tree findings with `code` on `path`.
    fn on<'f>(
        findings: &'f [serde_json::Value],
        code: &str,
        path: &str,
    ) -> Vec<&'f serde_json::Value> {
        findings
            .iter()
            .filter(|finding| finding["code"] == code && finding["path"] == path)
            .collect()
    }

    /// The one finding with `code` on `path`, and its cause.
    fn cause_of(findings: &[serde_json::Value], code: &str, path: &str) -> String {
        let found = on(findings, code, path);
        assert_eq!(found.len(), 1, "{code} on {path}: {findings:?}");
        found[0]["cause"].as_str().unwrap_or_default().to_owned()
    }

    fn census_edit(corpus: &Path, from: &str, to: &str) {
        replace_in(&corpus.join("census.toml"), from, to);
    }

    const KEY_MAP_ANCHOR: &str = "\"Weight\" = \"weight\"\n";
    const LOGIN: &str = "book/flows/login.md";
    const LOGIN_HEADER: &str = "---\nsort: flow\n---\n";
    const NEED_01: &str = "book/atoms/NEED/NEED-01.md";
    const NEED_01_ROW: &str =
        "| NEED-01 | The engine reads a pipe \\| inside a cell. | high | input |";
    const OWNER_LINE: &str = "owner: \"team\"\n";

    /// R1 / AC-05 M2: a value carried in a document's YAML header (its own
    /// `Stage`, renamed `status`), changed in the written header, is one
    /// mismatched field, and core's `status-invalid` on it is the
    /// emitter's: no positive proof the corpus wrote it. The corpus writing
    /// the same invalid value is the source's (the second run).
    #[test]
    fn a_changed_yaml_header_value_is_one_mismatched_field_and_the_emitter_s_finding() {
        let (result, findings) = tampered_copy(
            "one",
            |corpus| {
                replace_in(
                    &corpus.join(LOGIN),
                    LOGIN_HEADER,
                    "---\nsort: flow\nStage: draft\n---\n",
                );
            },
            |tree, _| {
                edit(tree, LOGIN, |text| {
                    text.replacen("\nstatus: draft\n", "\nstatus: drafty\n", 1)
                });
            },
        );
        assert_eq!(result.fields.mismatched, 1);
        assert_eq!(result.hashes.mismatched, 0);
        assert_eq!(cause_of(&findings, "status-invalid", LOGIN), "emitter");
        assert_eq!(result.check.findings["status-invalid"].source, 0);
        assert!(result.check.emitter_findings >= 1);

        let (result, findings) = tampered_copy(
            "one",
            |corpus| {
                replace_in(
                    &corpus.join(LOGIN),
                    LOGIN_HEADER,
                    "---\nsort: flow\nStage: drafty\n---\n",
                );
            },
            |_, _| {},
        );
        assert_eq!(result.fields.mismatched, 0);
        assert_eq!(cause_of(&findings, "status-invalid", LOGIN), "source");
        assert_eq!(result.check.emitter_findings, 0);
    }

    /// Fixture one with `Tier` renamed `tier` and a document whose header
    /// holds `Tier: <written>`.
    fn with_tier(corpus: &Path, written: &str) {
        census_edit(
            corpus,
            KEY_MAP_ANCHOR,
            "\"Weight\" = \"weight\"\n\"Tier\" = \"tier\"\n",
        );
        write(
            &corpus.join("book/typed-plain.md"),
            &format!("---\nTier: {written}\n---\n# Typed plain\n\nA tier the layout carries.\n"),
        );
    }

    /// R2: a typed core key's value whose corpus text parses as the key's
    /// type (`Tier: 2`), left on disk as one core rejects (`tier: "2"`,
    /// the same text read back) is the emitter's `frontmatter-type`: only
    /// the layout's writing can have mistyped it. (Values carried
    /// verbatim as block, folded or tagged scalars are the source's:
    /// layout_cli.rs.)
    #[test]
    fn a_typed_value_mistyped_in_the_tree_from_a_text_that_parses_is_the_emitter_s() {
        let (_, findings) = tampered_copy(
            "one",
            |corpus| with_tier(corpus, "2"),
            |tree, _| {
                edit(tree, "book/typed-plain.md", |text| {
                    text.replacen("\ntier: 2\n", "\ntier: \"2\"\n", 1)
                });
            },
        );
        assert_eq!(
            cause_of(&findings, "frontmatter-type", "book/typed-plain.md"),
            "emitter"
        );
    }

    /// R2 with D3: the corpus's `Tier: "2"` is a canonical integer the
    /// layout must unquote (AC-05 "YAML"); left quoted in the tree — the
    /// layout skipping its retyping, the tree's text then the corpus's
    /// byte for byte — core's `frontmatter-type` is the emitter's, not a
    /// verbatim carry of the source's (those are block, folded, tagged,
    /// multi-line values the layout does not rewrite).
    #[test]
    fn a_quoted_integer_the_layout_failed_to_unquote_is_the_emitter_s_frontmatter_type() {
        let (_, findings) = tampered_copy(
            "one",
            |corpus| with_tier(corpus, "\"2\""),
            |tree, _| {
                edit(tree, "book/typed-plain.md", |text| {
                    text.replacen("\ntier: 2\n", "\ntier: \"2\"\n", 1)
                });
            },
        );
        assert_eq!(
            cause_of(&findings, "frontmatter-type", "book/typed-plain.md"),
            "emitter"
        );
    }

    /// Fixture one with `Cites` renamed `refs` and a document citing in its
    /// header and in a table column.
    fn with_cites(corpus: &Path) {
        census_edit(
            corpus,
            KEY_MAP_ANCHOR,
            "\"Weight\" = \"weight\"\n\"Cites\" = \"refs\"\n",
        );
        write(
            &corpus.join("book/cites.md"),
            "---\nCites: NEED-01, NEED-02\n---\n# Cites\n\n| Code | Wording | Cites |\n|------|---------|-------|\n| NEED-50 | Cites two needs. | NEED-01, NEED-02 |\n| NEED-53 | Cites one twice. | NEED-01, NEED-01, RULE-01 |\n",
        );
    }

    /// R3: reference lists are read back item-wise as multisets — an item
    /// changed on disk, one of a repeated item dropped on disk (a set
    /// comparison sees nothing), an item added to a header's list: one
    /// mismatched field each, no emitter finding needed; prose put on disk
    /// in place of an ID is the emitter's `unparsed-reference` (the corpus
    /// wrote an ID there).
    #[test]
    fn a_reference_list_changed_on_disk_is_a_mismatch_even_by_one_repeated_item() {
        let untouched = tampered_copy("one", with_cites, |_, _| {}).0;
        assert_eq!(
            untouched.fields.mismatched, 0,
            "the cites corpus reads back"
        );
        for (path, from, to) in [
            (
                "book/atoms/NEED/NEED-50.md",
                "refs: [\"NEED-01\", \"NEED-02\"]",
                "refs: [\"NEED-01\", \"RULE-01\"]",
            ),
            (
                "book/atoms/NEED/NEED-53.md",
                "refs: [\"NEED-01\", \"NEED-01\", \"RULE-01\"]",
                "refs: [\"NEED-01\", \"RULE-01\"]",
            ),
            (
                "book/cites.md",
                "refs: [\"NEED-01\", \"NEED-02\"]",
                "refs: [\"NEED-01\", \"NEED-02\", \"NEED-02\"]",
            ),
        ] {
            let (result, _) = tampered_copy("one", with_cites, |tree, _| {
                edit(tree, path, |text| text.replacen(from, to, 1));
            });
            assert_eq!(result.fields.mismatched, 1, "{path}: {to}");
            assert_eq!(result.hashes.mismatched, 0, "{path}");
        }
        let (result, findings) = tampered_copy("one", with_cites, |tree, _| {
            edit(tree, "book/atoms/NEED/NEED-50.md", |text| {
                text.replacen("\"NEED-02\"]", "\"no such words\"]", 1)
            });
        });
        assert_eq!(result.fields.mismatched, 1);
        assert_eq!(
            cause_of(
                &findings,
                "unparsed-reference",
                "book/atoms/NEED/NEED-50.md"
            ),
            "emitter"
        );
    }

    /// R5: `class` dropped from a record file on disk is the emitter's
    /// `class-missing` — the layout always writes a record file's class
    /// (`record_class`) — even where no `classes` glob matches its path
    /// (the residues' `class-missing` there stays the source's).
    #[test]
    fn class_missing_on_a_record_file_is_the_emitter_s_without_a_glob() {
        let (result, findings) = tampered_copy(
            "one",
            |corpus| {
                census_edit(corpus, ", { glob = \"book/**\", class = \"canon\" }", "");
            },
            |tree, _| {
                edit(tree, NEED_01, |text| {
                    text.replacen("\nclass: canon\n", "\n", 1)
                });
            },
        );
        assert_eq!(cause_of(&findings, "class-missing", NEED_01), "emitter");
        assert_eq!(
            cause_of(&findings, "class-missing", "book/glossary.md"),
            "source"
        );
        assert_eq!(result.check.findings["class-missing"].emitter, 1);
        assert!(result.check.emitter_findings >= 1);
    }

    /// Fixture one with `Weight` and `Area` renamed by `key_map` (the
    /// whole census lines in `key_map`), record files of a `record_class`
    /// (its whole census line in `record_class`), and NEED-01's row as
    /// `row` (whole literals: the genre test reads every literal here).
    fn with_status(corpus: &Path, key_map: &str, record_class: &str, row: &str) {
        census_edit(corpus, KEY_MAP_ANCHOR, key_map);
        census_edit(corpus, "[layout]\n", record_class);
        replace_in(&corpus.join("book/needs.md"), NEED_01_ROW, row);
    }

    /// S1: a required key the corpus carried, lost on disk, is the
    /// emitter's (`canon-missing` of a carried `canon`, `shipped-missing`
    /// of a carried `shipped`, `key-missing` of a field table's `owner`);
    /// a carried `canon` changed on disk into an unreadable text is the
    /// emitter's too, while the corpus's own unreadable `canon` (OLDR-05's)
    /// is the source's.
    #[test]
    fn a_required_key_the_corpus_carried_lost_on_disk_is_the_emitter_s() {
        let canon = |corpus: &Path| {
            with_status(
                corpus,
                "\"Weight\" = \"status\"\n\"Area\" = \"canon\"\n",
                "[layout]\nrecord_class = \"decision\"\n",
                "| NEED-01 | The engine reads a pipe \\| inside a cell. | accepted | RULE-01 |",
            );
            replace_in(
                &corpus.join("book/needs.md"),
                "| high | store |",
                "| accepted | prose words here |",
            );
            // NEED-02's area an ID too: a path-form `canon` does not read
            // back (`a_carried_path_form_canon_reads_back_as_written`).
            replace_in(
                &corpus.join("book/needs.md"),
                "| low | store |",
                "| low | RULE-01 |",
            );
        };
        let (untouched, untouched_findings) = tampered_copy("one", canon, |_, _| {});
        assert_eq!(untouched.fields.mismatched, 0, "{untouched_findings:?}");
        let untouched = untouched_findings;
        assert!(
            on(&untouched, "canon-missing", NEED_01).is_empty(),
            "{untouched:?}"
        );
        let need_05 = "book/atoms/NEED/NEED-05.md";
        assert_eq!(cause_of(&untouched, "canon-missing", need_05), "source");

        let (_, findings) = tampered_copy("one", canon, |tree, _| {
            edit(tree, NEED_01, |text| {
                text.replacen("canon: \"RULE-01\"\n", "", 1)
            });
        });
        assert_eq!(cause_of(&findings, "canon-missing", NEED_01), "emitter");
        assert_eq!(cause_of(&findings, "canon-missing", need_05), "source");

        let (result, findings) = tampered_copy("one", canon, |tree, _| {
            edit(tree, NEED_01, |text| {
                text.replacen("canon: \"RULE-01\"", "canon: \"no reference here\"", 1)
            });
        });
        assert_eq!(result.fields.mismatched, 1);
        assert_eq!(cause_of(&findings, "canon-missing", NEED_01), "emitter");

        let (_, findings) = tampered_copy(
            "one",
            |corpus| {
                with_status(
                    corpus,
                    "\"Weight\" = \"status\"\n\"Area\" = \"shipped\"\n",
                    "[layout]\nrecord_class = \"spec\"\n",
                    "| NEED-01 | The engine reads a pipe \\| inside a cell. | shipped | 2026-01-01 |",
                );
            },
            |tree, _| {
                edit(tree, NEED_01, |text| {
                    text.replacen("shipped: \"2026-01-01\"\n", "", 1)
                });
            },
        );
        assert_eq!(cause_of(&findings, "shipped-missing", NEED_01), "emitter");

        let (_, findings) = tampered_copy(
            "one",
            |_| {},
            |tree, layout| {
                let plan = layout
                    .files
                    .iter()
                    .find(|file| {
                        file.kind == FileKind::Residue && file.content.contains("# Plan\n")
                    })
                    .expect("the field-table document");
                edit(tree, &plan.path, |text| text.replacen(OWNER_LINE, "", 1));
            },
        );
        // The key of `OWNER_LINE`, not a literal of its own (genre test).
        let key = OWNER_LINE.split(':').next().expect("a key");
        let owner: Vec<&serde_json::Value> = on(&findings, "key-missing", "book/plan.md")
            .into_iter()
            .filter(|finding| finding["subject"] == key)
            .collect();
        assert_eq!(owner.len(), 1, "{findings:?}");
        assert_eq!(owner[0]["cause"], "emitter", "{:?}", owner[0]);
    }

    /// S2: a carried value a check rule accepts, changed on disk into one
    /// it rejects, is one mismatched field and the emitter's
    /// `value-invalid`; the corpus's own rejected value in the same run
    /// (NEED-02's `low`) stays the source's.
    #[test]
    fn a_value_a_rule_rejects_after_a_change_on_disk_is_the_emitter_s() {
        let (result, findings) = tampered_copy(
            "one",
            |corpus| {
                let scheme = corpus.join("specengine.toml");
                let text = fs::read_to_string(&scheme).expect("scheme readable");
                write(
                    &scheme,
                    &format!(
                        "{text}\n[[check.rules]]\nkinds = [\"requirement\"]\nvalues = {{ weight = [\"high\"] }}\n"
                    ),
                );
            },
            |tree, _| {
                edit(tree, NEED_01, |text| {
                    text.replacen("weight: \"high\"", "weight: \"low\"", 1)
                });
            },
        );
        assert_eq!(result.fields.mismatched, 1);
        assert_eq!(cause_of(&findings, "value-invalid", NEED_01), "emitter");
        assert_eq!(
            cause_of(&findings, "value-invalid", "book/atoms/NEED/NEED-02.md"),
            "source"
        );
        assert_eq!(result.check.findings["value-invalid"].emitter, 1);
        assert_eq!(result.check.findings["value-invalid"].source, 1);
    }

    // ------------------------------- iteration 4 (#1, #4a, #5, #6, #8, F1)

    /// The 1-based line of the tree file `path` holding `needle`.
    fn line_of(tree: &Path, path: &str, needle: &str) -> u64 {
        let text = fs::read_to_string(tree.join(path)).expect("tree file readable");
        let at = text.find(needle).expect("the needle is in the tree file");
        u64::try_from(text[..at].matches('\n').count() + 1).expect("a line number")
    }

    /// #1 (AC-06, the first attribution rule): a carried `status` rewritten
    /// on disk into one that requires a key the corpus never had
    /// (`rejected` → `accepted` needs `canon`, `draft` → `shipped` needs
    /// `shipped`) is one mismatched field, and core's `canon-missing` /
    /// `shipped-missing`, named at that `status` line, is the emitter's —
    /// though the corpus lacks the required key (the absence rule alone
    /// says source). Untampered, neither finding exists.
    #[test]
    fn a_required_key_finding_on_a_rewritten_status_line_is_the_emitter_s() {
        // Whole census lines, cells and values (the genre test reads every
        // literal here): the record class, NEED-01's status cell, its
        // written value and the value it is rewritten to.
        for (class, cell, from, to, code) in [
            (
                "[layout]\nrecord_class = \"decision\"\n",
                "| rejected |",
                ": \"rejected\"\n",
                ": \"accepted\"\n",
                "canon-missing",
            ),
            (
                "[layout]\nrecord_class = \"spec\"\n",
                "| draft |",
                ": \"draft\"\n",
                ": \"shipped\"\n",
                "shipped-missing",
            ),
        ] {
            let corpus = |corpus: &Path| {
                with_status(
                    corpus,
                    "\"Weight\" = \"status\"\n",
                    class,
                    &NEED_01_ROW.replace("| high |", cell),
                );
            };
            let (untouched, untouched_findings) = tampered_copy("one", corpus, |_, _| {});
            assert_eq!(untouched.fields.mismatched, 0, "{code}");
            assert!(
                on(&untouched_findings, code, NEED_01).is_empty(),
                "{code}: {untouched_findings:?}"
            );
            let mut status_line = 0;
            let (result, findings) = tampered_copy("one", corpus, |tree, _| {
                status_line = line_of(tree, NEED_01, from);
                edit(tree, NEED_01, |text| text.replacen(from, to, 1));
            });
            assert_eq!(result.fields.mismatched, 1, "{code}");
            let found = on(&findings, code, NEED_01);
            assert_eq!(found.len(), 1, "{code}: {findings:?}");
            assert_eq!(found[0]["line"], status_line, "{code}: {:?}", found[0]);
            assert_eq!(found[0]["cause"], "emitter", "{code}: {:?}", found[0]);
        }
    }

    /// #6 (AC-06 `class-missing`): no `classes` glob matches. A residue
    /// whose source header itself carried `class: canon` (no class key)
    /// keeps it as written; `class` dropped from it on disk is the
    /// emitter's `class-missing` (the corpus carried it: only the layout's
    /// writing can have lost it), and so is the one dropped from a residue
    /// whose class the layout writes from the class key (`sort: register`);
    /// the header-less glossary's stays the source's.
    #[test]
    fn class_dropped_from_a_residue_whose_source_carried_it_is_the_emitter_s() {
        let carried = "book/classed.md";
        let mapped = "book/needs.md";
        let corpus = |corpus: &Path| {
            census_edit(corpus, ", { glob = \"book/**\", class = \"canon\" }", "");
            write(
                &corpus.join(carried),
                "---\nclass: canon\n---\n# Classed\n\nA class the corpus wrote itself.\n",
            );
        };
        let (untouched, untouched_findings) = tampered_copy("one", corpus, |tree, _| {
            for path in [carried, mapped] {
                let text = fs::read_to_string(tree.join(path)).expect("residue");
                assert!(text.starts_with("---\nclass: canon\n"), "{path}: {text}");
            }
        });
        assert_eq!(
            untouched.check.findings["class-missing"].emitter, 0,
            "{untouched_findings:?}"
        );
        assert!(on(&untouched_findings, "class-missing", carried).is_empty());
        for path in [carried, mapped] {
            let (result, findings) = tampered_copy("one", corpus, |tree, _| {
                edit(tree, path, |text| {
                    text.replacen("\nclass: canon\n", "\n", 1)
                });
            });
            assert_eq!(
                cause_of(&findings, "class-missing", path),
                "emitter",
                "{path}"
            );
            assert_eq!(
                cause_of(&findings, "class-missing", "book/glossary.md"),
                "source"
            );
            assert_eq!(result.check.findings["class-missing"].emitter, 1, "{path}");
        }
    }

    /// Fixture one with `Cites` renamed `refs`, `Canon` renamed `canon`,
    /// and `files` (path, text) written.
    fn with_cited(corpus: &Path, files: &[(&str, &str)]) {
        census_edit(
            corpus,
            KEY_MAP_ANCHOR,
            "\"Weight\" = \"weight\"\n\"Cites\" = \"refs\"\n\"Canon\" = \"canon\"\n",
        );
        for (path, text) in files {
            write(&corpus.join(path), text);
        }
    }

    /// #8 (AC-05, the header reader): a column-0 comment inside a block
    /// list (`Cites:`, `# c`, `  - X`) belongs to the open entry, so its
    /// items are compared; one item rewritten on disk is one mismatched
    /// field (an entry closed at the comment compares only `:` and misses
    /// it).
    #[test]
    fn an_item_after_a_comment_inside_a_block_list_rewritten_on_disk_is_a_mismatch() {
        let path = "book/listed.md";
        let files = [(
            path,
            "---\nCites:\n# c\n  - NEED-01\n\n  - NEED-02\n---\n# Listed\n\nA block list with a comment.\n",
        )];
        let (untouched, _) = tampered_copy(
            "one",
            |corpus| with_cited(corpus, &files),
            |tree, _| {
                let text = fs::read_to_string(tree.join(path)).expect("listed");
                assert!(
                    text.contains("\nrefs:\n# c\n  - NEED-01\n\n  - NEED-02\n"),
                    "{text}"
                );
            },
        );
        assert_eq!(untouched.fields.mismatched, 0);
        assert_eq!(untouched.check.emitter_findings, 0);
        for (from, to) in [
            ("  - NEED-02\n", "  - RULE-01\n"),
            ("  - NEED-01\n", "  - NEED-02\n"),
        ] {
            let (result, _) = tampered_copy(
                "one",
                |corpus| with_cited(corpus, &files),
                |tree, _| edit(tree, path, |text| text.replacen(from, to, 1)),
            );
            assert_eq!(result.fields.mismatched, 1, "{from:?} -> {to:?}");
        }
    }

    /// #4a (AC-05 "YAML"): a double-quoted scalar without a YAML escape
    /// (`"XQ-42, it's"`) is split like a plain one; the verifier matches a
    /// whole-value single item only when the source holds a real escape,
    /// so the tree's items joined back into one on disk are a mismatch.
    #[test]
    fn an_unescaped_quoted_scalar_joined_into_one_item_on_disk_is_a_mismatch() {
        let path = "book/joined.md";
        let files = [(
            path,
            "---\nCites: \"XQ-42, it's\"\n---\n# Joined\n\nAn apostrophe, no escape.\n",
        )];
        let split = "refs: [\"XQ-42\", \"it's\"]\n";
        let (untouched, _) = tampered_copy(
            "one",
            |corpus| with_cited(corpus, &files),
            |tree, _| {
                let text = fs::read_to_string(tree.join(path)).expect("joined");
                assert!(text.contains(split), "{text}");
            },
        );
        assert_eq!(untouched.fields.mismatched, 0);
        let (result, _) = tampered_copy(
            "one",
            |corpus| with_cited(corpus, &files),
            |tree, _| {
                edit(tree, path, |text| {
                    text.replacen(split, "refs: [\"XQ-42, it's\"]\n", 1)
                });
            },
        );
        assert_eq!(result.fields.mismatched, 1);
    }

    /// #5 (AC-05 "Fields"): a reference reads back as written, not by its
    /// ID: a `@rev` or a `#section` rewritten on disk (`NEED-01@2` →
    /// `NEED-01@3`, `RULE-01#RULE-02` → `RULE-01#RULE-01`, `checkout/CK-01`
    /// → `CK-01`) is one mismatched field each, under a reference-list key
    /// and under the single-reference `canon`.
    #[test]
    fn a_reference_s_rev_section_or_slug_rewritten_on_disk_is_a_mismatch() {
        let files = [
            (
                "book/revs.md",
                "---\nCites: NEED-01@2, checkout/CK-01, RULE-01#RULE-02\n---\n# Revs\n\nReferences as written.\n",
            ),
            (
                "book/canon-rev.md",
                "---\nCanon: RULE-01#RULE-02\n---\n# Canon rev\n\nA canon with a section.\n",
            ),
        ];
        let (untouched, untouched_findings) =
            tampered_copy("one", |corpus| with_cited(corpus, &files), |_, _| {});
        assert_eq!(untouched.fields.mismatched, 0);
        assert_eq!(
            untouched.check.emitter_findings, 0,
            "{untouched_findings:?}"
        );
        for (path, from, to) in [
            ("book/revs.md", "\"NEED-01@2\"", "\"NEED-01@3\""),
            ("book/revs.md", "\"RULE-01#RULE-02\"", "\"RULE-01#RULE-01\""),
            ("book/revs.md", "\"checkout/CK-01\"", "\"CK-01\""),
            ("book/canon-rev.md", "RULE-01#RULE-02", "RULE-01#RULE-01"),
        ] {
            let (result, _) = tampered_copy(
                "one",
                |corpus| with_cited(corpus, &files),
                |tree, _| edit(tree, path, |text| text.replacen(from, to, 1)),
            );
            assert_eq!(result.fields.mismatched, 1, "{path}: {from} -> {to}");
        }
    }

    /// F1 (AC-06 `canon-*`): a valid path-form `canon` the corpus wrote,
    /// rewritten on disk into one core rejects (`#anchor` dropped, the file
    /// renamed, the anchor renamed), is one mismatched field and the
    /// emitter's `canon-form` / `canon-file` / `canon-anchor`: the value
    /// rule no longer holds.
    #[test]
    fn a_valid_path_canon_rewritten_on_disk_is_the_emitter_s() {
        let path = "book/canon-path.md";
        let files = [(
            path,
            "---\nCanon: book/glossary.md#glossary\n---\n# Canon path\n\nA valid canon path.\n",
        )];
        for (to, code) in [
            ("book/glossary.md\n", "canon-form"),
            ("book/glossaries.md#glossary\n", "canon-file"),
            ("book/glossary.md#glossaries\n", "canon-anchor"),
        ] {
            let (result, findings) = tampered_copy(
                "one",
                |corpus| with_cited(corpus, &files),
                |tree, _| {
                    edit(tree, path, |text| {
                        text.replacen("book/glossary.md#glossary\n", to, 1)
                    });
                },
            );
            assert_eq!(result.fields.mismatched, 1, "{to}");
            assert_eq!(cause_of(&findings, code, path), "emitter", "{to}");
        }
    }

    // ------------------------------------------- iteration 5 (P2, P3)

    /// Fixture one with its ID regex unanchored at the end (an ID cell with
    /// a note after the ID still names the ID) and one row whose ID cell
    /// holds such a note.
    fn with_noted_id_cell(corpus: &Path) {
        census_edit(corpus, "{2,3}$'", "{2,3}'");
        replace_in(
            &corpus.join("book/needs.md"),
            "| OLDR-05 | A need still cited under its old prefix. | high | store |\n",
            "| OLDR-05 | A need still cited under its old prefix. | high | store |\n| NEED-03 (draft) | A need with a note in its code cell. | high | input |\n",
        );
    }

    /// P2 (AC-03, AC-04): an ID cell holding text beyond its ID, carried as
    /// a field of its record file under the column's header as written
    /// (`Code: "NEED-03 (draft)"`), then changed or dropped on disk, is one
    /// mismatched field: the verifier compares the source cell with core's
    /// read-back, not the emitter's copy. No hash moves, no residue.
    #[test]
    fn an_id_cell_changed_or_dropped_on_disk_is_one_mismatched_field() {
        let noted = "book/atoms/NEED/NEED-03.md";
        let (untampered, _) = tampered_copy("one", with_noted_id_cell, |tree, _| {
            let text = fs::read_to_string(tree.join(noted)).expect("the noted record file");
            assert!(text.contains("\nCode: \"NEED-03 (draft)\"\n"), "{text}");
        });
        assert_eq!(untampered.fields.mismatched, 0);
        assert_eq!(untampered.extents.residue, 0);
        assert_eq!(untampered.check.emitter_findings, 0);
        for (from, to) in [
            ("\"NEED-03 (draft)\"", "\"NEED-03 (final)\""),
            ("\nCode: \"NEED-03 (draft)\"\n", "\n"),
        ] {
            let (result, _) = tampered_copy("one", with_noted_id_cell, |tree, _| {
                edit(tree, noted, |text| text.replacen(from, to, 1));
            });
            assert_eq!(result.fields.mismatched, 1, "{from:?} -> {to:?}");
            assert_eq!(result.hashes.mismatched, 0, "{from:?}");
            assert_eq!(result.hashes.matched, result.before.definitions, "{from:?}");
            assert_eq!(result.extents.residue, 0, "{from:?}");
        }
    }

    /// Fixture one's field table repeating its `Keeper` row three times.
    fn with_repeated_keeper(corpus: &Path) {
        replace_in(
            &corpus.join("book/plan.md"),
            "| Keeper | team |\n",
            "| Keeper | team |\n| Keeper | crew |\n| Keeper | guild |\n",
        );
    }

    /// P3 (AC-05): a repeated field-table key is carried as `<key>-<n>`
    /// (`owner-2`, `owner-3`, each a header conflict) and compared: its
    /// value changed or its line dropped on disk is one mismatched field.
    #[test]
    fn a_repeated_field_table_key_changed_or_dropped_on_disk_is_one_mismatched_field() {
        let (untampered, _) = tampered_copy("one", with_repeated_keeper, |tree, _| {
            let text = fs::read_to_string(tree.join("book/plan.md")).expect("the plan");
            assert!(
                text.contains("\nowner: \"team\"\nowner-2: \"crew\"\nowner-3: \"guild\"\n"),
                "{text}"
            );
        });
        assert_eq!(untampered.fields.mismatched, 0);
        assert_eq!(untampered.header.conflicts, 2);
        assert_eq!(untampered.extents.residue, 0);
        for (from, to) in [
            ("owner-3: \"guild\"\n", "owner-3: \"gild\"\n"),
            ("owner-2: \"crew\"\n", ""),
        ] {
            let (result, _) = tampered_copy("one", with_repeated_keeper, |tree, _| {
                edit(tree, "book/plan.md", |text| text.replacen(from, to, 1));
            });
            assert_eq!(result.fields.mismatched, 1, "{from:?} -> {to:?}");
            assert_eq!(result.hashes.mismatched, 0, "{from:?}");
            assert_eq!(result.extents.residue, 0, "{from:?}");
        }
    }
}
