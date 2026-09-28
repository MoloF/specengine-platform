//! Measurement `ron`: `.ron` marker extraction of `specengine-code` by its own
//! lexer — the parse-clean share, rejected-construct categories, comment byte
//! ranges and marker resolution. Per-file detail goes to `--out`, aggregates
//! to stdout.
//!
//! The `tree-sitter-ron` comparison path was removed after the spike verdict
//! (`lexer`): its build linked a second tree-sitter runtime. The envelope keeps
//! its shape — every grammar-side field is `null`, `grammar.note` says why.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;
use specengine_code::ron::{self, Anchor, Rejected, RonAnalysis};

use crate::harness::{self, Corpus, percent};

/// Fixture directory (relative to `fixtures/`) used without `--pilot`.
pub const FIXTURE: &str = "ron";

/// Rejected constructs listed per category in `rejected.json`, at most.
const SAMPLES_PER_CATEGORY: usize = 20;

/// The `result` object: the rows of the "Results" table plus `detail`.
#[derive(Serialize)]
pub struct RonResult {
    pub grammar: GrammarStatus,
    pub files: usize,
    pub files_with_comments: usize,
    pub parse_clean: ByApproach<usize>,
    pub parse_clean_pct: ByApproach<f64>,
    /// Categories present in the corpus, sorted; never a file name.
    pub rejected_categories: ByApproach<Vec<&'static str>>,
    pub comment_byte_ranges: CommentRanges,
    pub markers: MarkerSummary,
    /// Every marker anchored at depth ≥ 2 resolves to such a path; `null`
    /// when the corpus has no such marker.
    pub nested_marker_resolves: ByApproach<Option<bool>>,
    /// Always `lexer`: the Phase 0 verdict (05 §9 "AST" row).
    pub recommendation: &'static str,
    pub detail: Detail,
}

/// The removed comparison grammar: kept so the envelope's shape is stable.
#[derive(Serialize)]
pub struct GrammarStatus {
    pub built: bool,
    pub loads: bool,
    pub version: Option<&'static str>,
    pub abi: Option<usize>,
    pub second_runtime: Option<bool>,
    pub note: &'static str,
}

const GRAMMAR_REMOVED: GrammarStatus = GrammarStatus {
    built: false,
    loads: false,
    version: None,
    abi: None,
    second_runtime: None,
    note: "removed after the spike verdict (`lexer`): the comparison path linked the tree-sitter 0.20 runtime next to 0.27",
};

/// `grammar` is always `null` (the comparison path was removed).
#[derive(Serialize)]
pub struct ByApproach<T> {
    pub grammar: Option<T>,
    pub lexer: T,
}

impl<T> ByApproach<T> {
    fn lexer(lexer: T) -> Self {
        Self {
            grammar: None,
            lexer,
        }
    }
}

#[derive(Serialize)]
pub struct CommentRanges {
    /// Always `null` (the comparison path was removed).
    pub grammar: Option<bool>,
    pub lexer: bool,
    /// Always `null` (the comparison path was removed).
    pub files_agreeing_pct: Option<f64>,
}

#[derive(Serialize)]
pub struct MarkerSummary {
    pub total: usize,
    pub by_relation: BTreeMap<&'static str, usize>,
    pub anchored: ByApproach<usize>,
    pub unanchored: ByApproach<usize>,
    pub cannot_verify: ByApproach<usize>,
    /// Markers anchored at depth ≥ 2.
    pub nested: usize,
    /// Always `null` (the comparison path was removed).
    pub agree_pct: Option<f64>,
    /// IDs outside the Latin script (ADR-0009); reported, never fatal.
    pub id_not_latin: usize,
}

/// Side information that explains the headline numbers. No names, no paths.
#[derive(Serialize)]
pub struct Detail {
    pub files_skipped: usize,
    pub bytes: usize,
    pub lexer_ms: u128,
    /// Always `null` (the comparison path was removed).
    pub grammar_ms: Option<u128>,
    pub comments: ByApproach<usize>,
    pub files_with_markers: usize,
    /// Files per rejected category.
    pub rejected_files: ByApproach<BTreeMap<&'static str, usize>>,
}

/// One row of `files.json`.
#[derive(Serialize)]
struct FileRow {
    path: String,
    bytes: usize,
    comments: ByApproach<usize>,
    clean: ByApproach<bool>,
    categories: ByApproach<Vec<&'static str>>,
    markers: usize,
}

/// One row of `markers.json`.
#[derive(Serialize)]
struct MarkerRow {
    path: String,
    line: usize,
    relation: &'static str,
    id: String,
    rev: Option<u32>,
    id_latin: bool,
    note: Option<String>,
    lexer: String,
    /// Always `null` (the comparison path was removed).
    grammar: Option<String>,
}

/// One sample of `rejected.json`.
#[derive(Serialize)]
struct RejectedRow {
    path: String,
    line: usize,
    snippet: String,
}

struct Analyzed {
    relative: PathBuf,
    text: String,
    lexer: RonAnalysis,
}

pub fn run(corpus: &Corpus) -> Result<RonResult, String> {
    let out_dir = corpus.out.join("ron").join(&corpus.label);
    fs::create_dir_all(&out_dir)
        .map_err(|error| format!("cannot create {}: {error}", out_dir.display()))?;

    // 1. Read the corpus.
    let paths = harness::ron_files(&corpus.root)
        .map_err(|error| format!("cannot list {}: {error}", corpus.root.display()))?;
    let mut sources = Vec::with_capacity(paths.len());
    let mut files_skipped = 0;
    for relative in paths {
        let absolute = corpus.root.join(&relative);
        match fs::read(&absolute).map(String::from_utf8) {
            Ok(Ok(text)) => sources.push((relative, text)),
            Ok(Err(_)) => {
                eprintln!("ron: skipped {}: not UTF-8", relative.display());
                files_skipped += 1;
            }
            Err(error) => {
                eprintln!("ron: skipped {}: {error}", relative.display());
                files_skipped += 1;
            }
        }
    }

    // 2. The lexer approach, timed.
    let started = Instant::now();
    let lexer: Vec<RonAnalysis> = sources.iter().map(|(_, text)| ron::analyze(text)).collect();
    let lexer_ms = started.elapsed().as_millis();
    let files: Vec<Analyzed> = sources
        .into_iter()
        .zip(lexer)
        .map(|((relative, text), lexer)| Analyzed {
            relative,
            text,
            lexer,
        })
        .collect();

    // 3. Aggregates.
    let total = files.len();
    let bytes = files.iter().map(|f| f.text.len()).sum();
    let files_with_comments = files
        .iter()
        .filter(|f| !f.lexer.comments.is_empty())
        .count();
    let lexer_clean = files.iter().filter(|f| !f.lexer.has_error).count();
    let mut lexer_categories = BTreeSet::new();
    let mut lexer_rejected_files: BTreeMap<&'static str, usize> = BTreeMap::new();
    for file in &files {
        for category in &file.lexer.error_categories {
            lexer_categories.insert(*category);
            *lexer_rejected_files.entry(category).or_default() += 1;
        }
    }
    let lexer_comments: usize = files.iter().map(|f| f.lexer.comments.len()).sum();

    // 4. Markers.
    let mut by_relation: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut lexer_anchors = Counts::default();
    let mut nested = 0;
    let mut id_not_latin = 0;
    let mut files_with_markers = 0;
    let mut marker_rows = Vec::new();
    for file in &files {
        if !file.lexer.markers.is_empty() {
            files_with_markers += 1;
        }
        for marker in &file.lexer.markers {
            *by_relation
                .entry(marker.marker.relation.as_str())
                .or_default() += 1;
            lexer_anchors.count(&marker.anchor);
            if !marker.marker.id_latin {
                id_not_latin += 1;
            }
            if matches!(marker.anchor, Anchor::Path { depth, .. } if depth >= 2) {
                nested += 1;
            }
            marker_rows.push(MarkerRow {
                path: relative_string(&file.relative),
                line: marker.line,
                relation: marker.marker.relation.as_str(),
                id: marker.marker.id.clone(),
                rev: marker.marker.rev,
                id_latin: marker.marker.id_latin,
                note: marker.marker.note.clone(),
                lexer: marker.anchor.as_str().to_owned(),
                grammar: None,
            });
        }
    }
    let markers_total = marker_rows.len();
    // Every nested marker is counted by the lexer's own depth, so it resolves.
    let nested_lexer = (nested > 0).then_some(true);

    // 5. Per-file detail to --out.
    let file_rows: Vec<FileRow> = files
        .iter()
        .map(|file| FileRow {
            path: relative_string(&file.relative),
            bytes: file.text.len(),
            comments: ByApproach::lexer(file.lexer.comments.len()),
            clean: ByApproach::lexer(!file.lexer.has_error),
            categories: ByApproach::lexer(file.lexer.error_categories.clone()),
            markers: file.lexer.markers.len(),
        })
        .collect();
    let rejected_samples = BTreeMap::from([
        (
            "lexer",
            samples(
                files
                    .iter()
                    .map(|f| (&f.relative, &f.text, &f.lexer.rejected[..])),
            ),
        ),
        // The removed grammar side: kept empty so the file's shape is stable.
        ("grammar", BTreeMap::new()),
    ]);
    write_json(&out_dir.join("files.json"), &file_rows)?;
    write_json(&out_dir.join("markers.json"), &marker_rows)?;
    write_json(&out_dir.join("rejected.json"), &rejected_samples)?;

    let result = RonResult {
        grammar: GRAMMAR_REMOVED,
        files: total,
        files_with_comments,
        parse_clean: ByApproach::lexer(lexer_clean),
        parse_clean_pct: ByApproach::lexer(percent(lexer_clean, total)),
        rejected_categories: ByApproach::lexer(lexer_categories.into_iter().collect()),
        comment_byte_ranges: CommentRanges {
            grammar: None,
            lexer: true,
            files_agreeing_pct: None,
        },
        markers: MarkerSummary {
            total: markers_total,
            by_relation,
            anchored: ByApproach::lexer(lexer_anchors.anchored),
            unanchored: ByApproach::lexer(lexer_anchors.unanchored),
            cannot_verify: ByApproach::lexer(lexer_anchors.cannot_verify),
            nested,
            agree_pct: None,
            id_not_latin,
        },
        nested_marker_resolves: ByApproach::lexer(nested_lexer),
        recommendation: "lexer",
        detail: Detail {
            files_skipped,
            bytes,
            lexer_ms,
            grammar_ms: None,
            comments: ByApproach::lexer(lexer_comments),
            files_with_markers,
            rejected_files: ByApproach::lexer(lexer_rejected_files),
        },
    };

    // 6. Human summary.
    summarize(&result, &corpus.label, &out_dir);
    Ok(result)
}

#[derive(Default)]
struct Counts {
    anchored: usize,
    unanchored: usize,
    cannot_verify: usize,
}

impl Counts {
    fn count(&mut self, anchor: &Anchor) {
        match anchor {
            Anchor::Path { .. } => self.anchored += 1,
            Anchor::Unanchored => self.unanchored += 1,
            Anchor::CannotVerify => self.cannot_verify += 1,
        }
    }
}

/// Up to [`SAMPLES_PER_CATEGORY`] rejected constructs per category.
fn samples<'a>(
    files: impl Iterator<Item = (&'a PathBuf, &'a String, &'a [Rejected])>,
) -> BTreeMap<&'static str, Vec<RejectedRow>> {
    let mut out: BTreeMap<&'static str, Vec<RejectedRow>> = BTreeMap::new();
    for (relative, text, rejected) in files {
        for item in rejected {
            let rows = out.entry(item.category).or_default();
            if rows.len() >= SAMPLES_PER_CATEGORY {
                continue;
            }
            rows.push(RejectedRow {
                path: relative_string(relative),
                line: line_of(text, item.offset),
                snippet: snippet(text, item.offset),
            });
        }
    }
    out
}

/// Called at most [`SAMPLES_PER_CATEGORY`] times per category and corpus.
fn line_of(text: &str, offset: usize) -> usize {
    text.as_bytes()[..offset.min(text.len())]
        .iter()
        .filter(|b| **b == b'\n')
        .count()
        + 1
}

/// The rest of the line from `offset`, at most 80 characters.
fn snippet(text: &str, offset: usize) -> String {
    let start = (0..=offset.min(text.len()))
        .rev()
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(0);
    text[start..]
        .lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(80)
        .collect()
}

fn summarize(result: &RonResult, label: &str, out_dir: &Path) {
    let categories = |list: &[&'static str]| {
        if list.is_empty() {
            "none".to_owned()
        } else {
            list.join(", ")
        }
    };
    eprintln!(
        "ron [{label}]: {} files, {} with comments, {} skipped",
        result.files, result.files_with_comments, result.detail.files_skipped
    );
    eprintln!("  grammar: {}", result.grammar.note);
    eprintln!(
        "  parse-clean: lexer {:.1} % ({} files)",
        result.parse_clean_pct.lexer, result.parse_clean.lexer
    );
    eprintln!(
        "  rejected categories: lexer {}",
        categories(&result.rejected_categories.lexer)
    );
    eprintln!(
        "  markers: {} total, anchored {}, unanchored {}, cannot_verify {}, nested {} (resolve: {})",
        result.markers.total,
        result.markers.anchored.lexer,
        result.markers.unanchored.lexer,
        result.markers.cannot_verify.lexer,
        result.markers.nested,
        result
            .nested_marker_resolves
            .lexer
            .map_or("n/a", |v| if v { "yes" } else { "no" })
    );
    eprintln!(
        "  recommendation: {} (lexer {} ms)",
        result.recommendation, result.detail.lexer_ms
    );
    eprintln!("  detail: {}", out_dir.display());
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| format!("cannot serialize {}: {error}", path.display()))?;
    fs::write(path, json).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn relative_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
