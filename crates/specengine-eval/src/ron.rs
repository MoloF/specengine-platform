//! Measurement `ron`: `.ron` marker extraction of `specengine-code` by its own
//! lexer — the parse-clean share, rejected-construct categories, comment byte
//! ranges, marker resolution (ambiguous paths apart), marker level lists and
//! empty IDs. Per-file detail goes to `--out`, aggregates to stdout.
//!
//! The `tree-sitter-ron` comparison path was removed after the spike verdict
//! (`lexer`): its build linked a second tree-sitter runtime. The envelope keeps
//! its shape — every grammar-side field is `null`, `grammar.note` says why.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;
use specengine_code::ron::{self, Anchor, Rejected, RonAnalysis, RonValues};
use specengine_code::{Digest, Levels};

use crate::harness::{self, Corpus, percent, stability_percent};

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
    /// The RON value hash (5a-2, [`ron::value_hashes`]).
    pub value_hash: ValueHash,
    pub detail: Detail,
}

/// `result.value_hash`, as `docs/features/hash-levels-eval.md`
/// "`ron`: `result.value_hash`" defines it: over every comment anchored to
/// a path, marked or not, then over the markers.
#[derive(Serialize, Default)]
pub struct ValueHash {
    /// Comments whose anchor is a path ([`ron::comment_anchors`]).
    pub anchored: usize,
    /// Those with a value hash.
    pub hashed: usize,
    /// Those of files [`ron::analyze`] rejects: every value refused.
    pub cannot_verify: usize,
    /// Of `hashed`, those whose hash an in-memory CRLF copy keeps, matched
    /// by comment order.
    pub crlf_stable_pct: f64,
    /// Markers whose comment has a value hash, in the original and in a
    /// copy stripped of every comment that holds no marker (paired in
    /// source order).
    pub bound: usize,
    /// Of `bound`, those whose hash the stripped copy keeps; `null` when none
    /// is bound.
    pub bound_stable_pct: Option<f64>,
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
    /// Bound to a path that names more than one value (colliding siblings);
    /// never counted `anchored`.
    pub ambiguous: ByApproach<usize>,
    /// Markers anchored (an `Anchor::Path`, not an ambiguous one) at depth ≥ 2.
    pub nested: usize,
    /// Always `null` (the comparison path was removed).
    pub agree_pct: Option<f64>,
    /// IDs outside the Latin script (ADR-0009); reported, never fatal. An
    /// empty ID is not counted here.
    pub id_not_latin: usize,
    /// Markers whose keyword ends the line: no ID at all.
    pub id_empty: usize,
    /// Markers by the state of their level list.
    pub levels: LevelCounts,
}

#[derive(Serialize, Default)]
pub struct LevelCounts {
    /// No list: `[sig, body]`.
    pub default: usize,
    pub declared: usize,
    /// A malformed list (categories in `detail.levels_invalid`).
    pub invalid: usize,
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
    /// Invalid level lists per category (`empty`, `duplicate`, `unknown`, `unclosed`).
    pub levels_invalid: BTreeMap<&'static str, usize>,
    /// Sibling groups of a struct or map sharing a segment, marked or not.
    pub colliding_groups: usize,
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
    /// The levels the binding depends on (the default `[sig, body]` when
    /// none are written); `null` when the list is invalid.
    levels: Option<Vec<&'static str>>,
    /// `default`, `declared` or the invalid list's category.
    levels_state: &'static str,
    note: Option<String>,
    /// `lexer` holds the path text of an ambiguous anchor.
    ambiguous: bool,
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
    let colliding_groups: usize = files.iter().map(|f| f.lexer.colliding_groups).sum();

    // 4. Markers.
    let mut by_relation: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut lexer_anchors = Counts::default();
    let mut nested = 0;
    let mut id_not_latin = 0;
    let mut id_empty = 0;
    let mut levels = LevelCounts::default();
    let mut levels_invalid: BTreeMap<&'static str, usize> = BTreeMap::new();
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
            if marker.marker.id.is_empty() {
                id_empty += 1;
            }
            match &marker.marker.levels {
                Levels::Default => levels.default += 1,
                Levels::Declared(_) => levels.declared += 1,
                Levels::Invalid(error) => {
                    levels.invalid += 1;
                    *levels_invalid.entry(error.as_str()).or_default() += 1;
                }
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
                levels: marker
                    .marker
                    .levels
                    .effective()
                    .map(|list| list.iter().map(|level| level.as_str()).collect()),
                levels_state: marker.marker.levels.state(),
                note: marker.marker.note.clone(),
                ambiguous: matches!(marker.anchor, Anchor::Ambiguous { .. }),
                lexer: marker.anchor.as_str().to_owned(),
                grammar: None,
            });
        }
    }
    let markers_total = marker_rows.len();
    // Every nested marker is counted by the lexer's own depth, so it resolves.
    let nested_lexer = (nested > 0).then_some(true);
    let value_hash = value_hash(&files);

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
            ambiguous: ByApproach::lexer(lexer_anchors.ambiguous),
            nested,
            agree_pct: None,
            id_not_latin,
            id_empty,
            levels,
        },
        nested_marker_resolves: ByApproach::lexer(nested_lexer),
        recommendation: "lexer",
        value_hash,
        detail: Detail {
            files_skipped,
            bytes,
            lexer_ms,
            grammar_ms: None,
            comments: ByApproach::lexer(lexer_comments),
            files_with_markers,
            rejected_files: ByApproach::lexer(lexer_rejected_files),
            levels_invalid,
            colliding_groups,
        },
    };

    // 6. Human summary.
    summarize(&result, &corpus.label, &out_dir);
    Ok(result)
}

/// `result.value_hash`: each file's values ([`ron::value_hashes`]) beside
/// its anchors ([`ron::comment_anchors`]); a CRLF copy of each file with a
/// value hashed; a copy stripped of every comment holding no marker, of each
/// file with a marker bound.
fn value_hash(files: &[Analyzed]) -> ValueHash {
    let mut out = ValueHash::default();
    let mut crlf_equal = 0;
    let mut bound_equal = 0;
    for file in files {
        let anchored = ron::comment_anchors(&file.text)
            .iter()
            .filter(|comment| matches!(comment.anchor, Anchor::Path { .. }))
            .count();
        out.anchored += anchored;
        let entries = match ron::value_hashes(&file.text) {
            RonValues::Hashed(entries) => entries,
            RonValues::CannotVerify(_) => {
                out.cannot_verify += anchored;
                continue;
            }
        };
        if entries.is_empty() {
            continue;
        }
        out.hashed += entries.len();
        let crlf = digests(ron::value_hashes(&crlf_copy(&file.text)));
        crlf_equal += entries
            .iter()
            .zip(&crlf)
            .filter(|((_, before), after)| before == *after)
            .count();

        let by_comment: HashMap<&Range<usize>, Digest> = entries
            .iter()
            .map(|(range, digest)| (range, *digest))
            .collect();
        let before: Vec<Option<Digest>> = file
            .lexer
            .markers
            .iter()
            .map(|marker| by_comment.get(&marker.comment).copied())
            .collect();
        if before.iter().all(Option::is_none) {
            continue;
        }
        let marked: HashSet<&Range<usize>> = file
            .lexer
            .markers
            .iter()
            .map(|marker| &marker.comment)
            .collect();
        let stripped = strip_unmarked(&file.text, &file.lexer.comments, &marked);
        let stripped_markers = ron::analyze(&stripped).markers;
        let stripped_values: HashMap<Range<usize>, Digest> = match ron::value_hashes(&stripped) {
            RonValues::Hashed(entries) => entries.into_iter().collect(),
            RonValues::CannotVerify(_) => HashMap::new(),
        };
        // Bound: a value hash for the marker's comment in the original and in
        // the stripped copy, the markers paired in source order.
        for (index, before) in before.iter().enumerate() {
            let after = stripped_markers
                .get(index)
                .and_then(|marker| stripped_values.get(&marker.comment));
            let (Some(before), Some(after)) = (before, after) else {
                continue;
            };
            out.bound += 1;
            if before == after {
                bound_equal += 1;
            }
        }
    }
    out.crlf_stable_pct = stability_percent(crlf_equal, out.hashed);
    out.bound_stable_pct = (out.bound > 0).then(|| percent(bound_equal, out.bound));
    out
}

/// The digests of a file's values in comment order; none when refused.
fn digests(values: RonValues) -> Vec<Digest> {
    match values {
        RonValues::Hashed(entries) => entries.into_iter().map(|(_, digest)| digest).collect(),
        RonValues::CannotVerify(_) => Vec::new(),
    }
}

/// `text` with every LF not after a CR made a CRLF.
fn crlf_copy(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 16);
    let mut previous = '\0';
    for character in text.chars() {
        if character == '\n' && previous != '\r' {
            out.push('\r');
        }
        out.push(character);
        previous = character;
    }
    out
}

/// `text` with each of `comments` (in source order) outside `kept` replaced
/// by a single space, as Rust's `strip_comments` does.
fn strip_unmarked(text: &str, comments: &[Range<usize>], kept: &HashSet<&Range<usize>>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut position = 0;
    for comment in comments {
        if kept.contains(comment) || comment.start < position {
            continue;
        }
        let (Some(before), Some(_)) =
            (text.get(position..comment.start), text.get(comment.clone()))
        else {
            continue;
        };
        out.push_str(before);
        out.push(' ');
        position = comment.end;
    }
    out.push_str(text.get(position..).unwrap_or_default());
    out
}

#[derive(Default)]
struct Counts {
    anchored: usize,
    unanchored: usize,
    cannot_verify: usize,
    ambiguous: usize,
}

impl Counts {
    fn count(&mut self, anchor: &Anchor) {
        match anchor {
            Anchor::Path { .. } => self.anchored += 1,
            Anchor::Ambiguous { .. } => self.ambiguous += 1,
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
        "  markers: {} total, anchored {}, ambiguous {}, unanchored {}, cannot_verify {}, nested {} (resolve: {})",
        result.markers.total,
        result.markers.anchored.lexer,
        result.markers.ambiguous.lexer,
        result.markers.unanchored.lexer,
        result.markers.cannot_verify.lexer,
        result.markers.nested,
        result
            .nested_marker_resolves
            .lexer
            .map_or("n/a", |v| if v { "yes" } else { "no" })
    );
    eprintln!(
        "  levels: default {}, declared {}, invalid {}; empty IDs {}; colliding sibling groups {}",
        result.markers.levels.default,
        result.markers.levels.declared,
        result.markers.levels.invalid,
        result.markers.id_empty,
        result.detail.colliding_groups
    );
    let value_hash = &result.value_hash;
    eprintln!(
        "  value hash: {} anchored, {} hashed, {} cannot_verify, CRLF-stable {:.1} %; {} markers bound, stable {}",
        value_hash.anchored,
        value_hash.hashed,
        value_hash.cannot_verify,
        value_hash.crlf_stable_pct,
        value_hash.bound,
        value_hash
            .bound_stable_pct
            .map_or("n/a".to_owned(), |pct| format!("{pct:.1} %"))
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
