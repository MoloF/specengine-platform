//! Measurement `census`: the dry-run counter of `specengine-import` over a
//! spec corpus. The convention comes from `--config` (default: `census.toml`
//! at the corpus root); the fixture and the pilots take the same code path.
//!
//! stdout carries counts only: per-class and per-prefix buckets are keyed by
//! anonymous labels (`class-1`, `prefix-1`, … by descending count), never by
//! a string of the corpus. The label mapping, the hash manifest and every
//! per-file list go to `--out`.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Instant;

use serde::Serialize;
use specengine_import::{Census, CensusConfig};

use crate::harness::Corpus;

/// Fixture directory (relative to `fixtures/`) used without `--pilot`.
pub const FIXTURE: &str = "corpus-mini";

/// Config file looked up at the corpus root when `--config` is absent.
pub const DEFAULT_CONFIG: &str = "census.toml";

/// Diagnostics echoed on stderr; the rest are in `diagnostics.json`.
const DIAGNOSTICS_ON_STDERR: usize = 10;

/// Bucket of documents whose front-matter has no class key.
const UNCLASSIFIED: &str = "unclassified";

/// The `result` object: the rows of the "Results" table plus `detail`.
#[derive(Serialize)]
pub struct CensusResult {
    pub documents: usize,
    pub with_front_matter: ClassCounts,
    pub id_rows: PrefixCounts,
    /// Rows of record tables whose ID cell is empty or holds no ID.
    pub rows_without_id: usize,
    pub id_sections: usize,
    /// Rows and sections: ASCII letters next to a look-alike (ADR-0009).
    pub mixed_script_ids: usize,
    /// Rows and sections: no ASCII letter, foreign letters only (ADR-0009).
    pub non_latin_ids: usize,
    /// Links and images whose local target does not exist.
    pub broken_links: usize,
    /// ID'd rows + `{#ID}` sections, BLAKE3 over verbatim text.
    pub records_hashed: usize,
    pub detail: Detail,
}

#[derive(Serialize)]
pub struct ClassCounts {
    pub total: usize,
    /// `class-N` by descending count, plus `unclassified`.
    pub per_class: BTreeMap<String, usize>,
}

#[derive(Serialize)]
pub struct PrefixCounts {
    /// Rows with an ID of any script.
    pub total: usize,
    /// `prefix-N` by descending count of the normalized prefix.
    pub per_prefix: BTreeMap<String, usize>,
}

/// Side information that explains the headline numbers. No names, no paths.
#[derive(Serialize)]
pub struct Detail {
    pub files_skipped: usize,
    pub front_matter_unclosed: usize,
    pub roots_missing: usize,
    pub bytes: usize,
    pub tables: usize,
    /// Blocks of `|` rows without a header row (not tables in GFM).
    pub headerless_blocks: usize,
    /// Rows of such blocks with an ID; records only with `tables.headerless`.
    pub headerless_id_rows: usize,
    pub record_tables: usize,
    pub links_checked: usize,
    /// `{#…}` heading anchors that do not match the ID pattern.
    pub other_anchors: usize,
    /// Records whose normalized ID already occurred.
    pub duplicate_ids: usize,
    pub diagnostics: usize,
    pub census_ms: u128,
}

/// One row of `labels.json`: what an anonymous label stands for.
#[derive(Serialize)]
struct LabelRow {
    label: String,
    value: String,
    count: usize,
}

#[derive(Serialize)]
struct Labels {
    classes: Vec<LabelRow>,
    prefixes: Vec<LabelRow>,
}

/// Reads the census config before anything is written; an error refuses the run.
pub fn load_config(config: Option<&Path>, root: &Path) -> Result<CensusConfig, String> {
    let path = match config {
        Some(path) => path.to_path_buf(),
        None => {
            let path = root.join(DEFAULT_CONFIG);
            if !path.is_file() {
                return Err(format!(
                    "no --config given and no {DEFAULT_CONFIG} at the corpus root"
                ));
            }
            path
        }
    };
    CensusConfig::load(&path).map_err(|error| error.to_string())
}

pub fn run(corpus: &Corpus, config: CensusConfig) -> Result<CensusResult, String> {
    let out_dir = corpus.out.join("census").join(&corpus.label);
    fs::create_dir_all(&out_dir)
        .map_err(|error| format!("cannot create {}: {error}", out_dir.display()))?;

    let started = Instant::now();
    let census = specengine_import::run(&corpus.root, &config)?;
    let census_ms = started.elapsed().as_millis();

    let mut classes = BTreeMap::new();
    let mut unclassified = 0;
    for (class, count) in census.per_class() {
        match class {
            Some(class) => {
                classes.insert(class, count);
            }
            None => unclassified = count,
        }
    }
    let (mut per_class, class_labels) = anonymize(classes, "class");
    if unclassified > 0 {
        per_class.insert(UNCLASSIFIED.to_owned(), unclassified);
    }
    let (per_prefix, prefix_labels) = anonymize(census.per_prefix(), "prefix");

    write_json(&out_dir.join("records.json"), &census.records)?;
    write_json(&out_dir.join("documents.json"), &census.documents_detail)?;
    write_json(
        &out_dir.join("rows_without_id.json"),
        &census.rows_without_id,
    )?;
    write_json(&out_dir.join("broken_links.json"), &census.broken_links)?;
    write_json(&out_dir.join("diagnostics.json"), &census.diagnostics)?;
    write_json(
        &out_dir.join("labels.json"),
        &Labels {
            classes: class_labels,
            prefixes: prefix_labels,
        },
    )?;

    let result = CensusResult {
        documents: census.documents,
        with_front_matter: ClassCounts {
            total: census.with_front_matter(),
            per_class,
        },
        id_rows: PrefixCounts {
            total: census.id_rows(),
            per_prefix,
        },
        rows_without_id: census.rows_without_id.len(),
        id_sections: census.id_sections(),
        mixed_script_ids: census.mixed_script_ids(),
        non_latin_ids: census.non_latin_ids(),
        broken_links: census.broken_links.len(),
        records_hashed: census.records.len(),
        detail: Detail {
            files_skipped: census.files_skipped,
            front_matter_unclosed: census.front_matter_unclosed(),
            roots_missing: census.roots_missing,
            bytes: census.bytes,
            tables: census.tables(),
            headerless_blocks: census.headerless_blocks(),
            headerless_id_rows: census.headerless_id_rows,
            record_tables: census.record_tables(),
            links_checked: census.links_checked(),
            other_anchors: census.other_anchors,
            duplicate_ids: census.duplicate_ids(),
            diagnostics: census.diagnostics.len(),
            census_ms,
        },
    };
    summarize(&result, &census, &corpus.label, &out_dir);
    Ok(result)
}

/// Replaces corpus strings by `<stem>-N`, N by descending count (ties by the
/// string), zero-padded to a common width; returns the mapping for `--out`.
fn anonymize(
    counts: BTreeMap<String, usize>,
    stem: &str,
) -> (BTreeMap<String, usize>, Vec<LabelRow>) {
    let mut ordered: Vec<(String, usize)> = counts.into_iter().collect();
    ordered.sort_by(|a, b| (Reverse(a.1), &a.0).cmp(&(Reverse(b.1), &b.0)));
    let width = ordered.len().to_string().len();
    let mut anonymous = BTreeMap::new();
    let mut labels = Vec::with_capacity(ordered.len());
    for (index, (value, count)) in ordered.into_iter().enumerate() {
        let label = format!("{stem}-{:0width$}", index + 1);
        anonymous.insert(label.clone(), count);
        labels.push(LabelRow {
            label,
            value,
            count,
        });
    }
    (anonymous, labels)
}

fn summarize(result: &CensusResult, census: &Census, label: &str, out_dir: &Path) {
    eprintln!(
        "census [{label}]: {} documents, {} with front-matter ({} classes), {} skipped",
        result.documents,
        result.with_front_matter.total,
        result.with_front_matter.per_class.len(),
        result.detail.files_skipped
    );
    eprintln!(
        "  ID'd rows {} ({} prefixes), rows without ID {}, {{#ID}} sections {}",
        result.id_rows.total,
        result.id_rows.per_prefix.len(),
        result.rows_without_id,
        result.id_sections
    );
    eprintln!(
        "  record tables {} of {}, headerless blocks {} ({} rows with an ID)",
        result.detail.record_tables,
        result.detail.tables,
        result.detail.headerless_blocks,
        result.detail.headerless_id_rows
    );
    eprintln!(
        "  mixed-script IDs {}, non-Latin IDs {}, duplicate IDs {}",
        result.mixed_script_ids, result.non_latin_ids, result.detail.duplicate_ids
    );
    eprintln!(
        "  broken links {} of {} checked, records hashed {} ({} ms)",
        result.broken_links,
        result.detail.links_checked,
        result.records_hashed,
        result.detail.census_ms
    );
    for diagnostic in census.diagnostics.iter().take(DIAGNOSTICS_ON_STDERR) {
        match diagnostic.line {
            Some(line) => eprintln!("  {}:{line}: {}", diagnostic.path, diagnostic.message),
            None => eprintln!("  {}: {}", diagnostic.path, diagnostic.message),
        }
    }
    if census.diagnostics.len() > DIAGNOSTICS_ON_STDERR {
        eprintln!(
            "  … {} more in diagnostics.json",
            census.diagnostics.len() - DIAGNOSTICS_ON_STDERR
        );
    }
    eprintln!("  detail: {}", out_dir.display());
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| format!("cannot serialize {}: {error}", path.display()))?;
    fs::write(path, json).map_err(|error| format!("cannot write {}: {error}", path.display()))
}
