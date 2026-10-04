//! Measurement `import`: the import engine of `specengine-import` over a
//! spec corpus, read-only — the "before" report
//! (`docs/features/import-records.md` AC-03, AC-09). The convention comes from
//! `--config`, resolved as for `census` (`SPECENGINE_CENSUS_CONFIG_A` / `_B`
//! for `--label pilot-a` / `pilot-b` when set, else `census.toml` at the
//! corpus root).
//!
//! stdout carries counts only: classes, prefixes and hyphenless patterns are
//! keyed by anonymous labels (`class-N`, `prefix-N`, `pattern-N`, by
//! descending count) and `unclassified`. The mapping, the record model and
//! every per-file list go to `--out/import/<label>/`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Instant;

use serde::Serialize;
use specengine_import::CensusConfig;
use specengine_import::import::{
    self, Form, HeaderForm, HyphenlessRole, Import, LinkProblem, MapOutcome, Role, RowCause,
    TokenCause,
};

use crate::census::{
    CLASS_STEM, LabelRow, PATTERN_STEM, PREFIX_STEM, UNCLASSIFIED, anonymize, write_json,
};
use crate::harness::Corpus;

/// Fixture directory (relative to `fixtures/`) used without `--pilot`.
pub const FIXTURE: &str = "import-one";

/// Diagnostics echoed on stderr; the rest are in `diagnostics.json`.
const DIAGNOSTICS_ON_STDERR: usize = 10;

/// The `result` object: exactly the keys of the whitelist in
/// `crates/specengine-import/README.md` "Before report".
#[derive(Serialize)]
pub struct ImportResult {
    pub documents: Documents,
    pub front_matter: FrontMatter,
    pub records: Records,
    pub definitions: usize,
    pub references: References,
    pub duplicate_definitions: usize,
    pub rows_without_id: RowsWithoutId,
    pub legacy: Legacy,
    pub id_like: IdLike,
    pub broken_links: BrokenLinks,
    pub code: Code,
    pub detail: Detail,
}

#[derive(Serialize)]
pub struct Documents {
    pub total: usize,
    /// `class-N` by descending count, plus `unclassified`.
    pub per_class: BTreeMap<String, usize>,
}

#[derive(Serialize)]
pub struct FrontMatter {
    pub yaml: usize,
    pub field_table: usize,
    pub none: usize,
    pub unclosed: usize,
    pub non_latin_keys: usize,
    pub keys: MapCounts,
    pub values: MapCounts,
}

#[derive(Serialize)]
pub struct MapCounts {
    pub mapped: usize,
    pub kept: usize,
    pub unmapped: usize,
}

#[derive(Serialize)]
pub struct Records {
    pub total: usize,
    pub empty_text: usize,
    /// Records carrying a title (titled list items).
    pub titled: usize,
    pub per_form: PerForm,
    /// `prefix-N` by descending count of the Latin prefix.
    pub per_prefix: BTreeMap<String, usize>,
}

#[derive(Serialize)]
pub struct PerForm {
    pub table_row: usize,
    pub headerless_row: usize,
    pub list_item: usize,
    pub section: usize,
    pub document: usize,
}

#[derive(Serialize)]
pub struct References {
    pub total: usize,
    pub unresolved: usize,
    /// Record-position definitions a document demoted, within `total`.
    pub by_document: usize,
}

#[derive(Serialize)]
pub struct RowsWithoutId {
    pub local_number: usize,
    pub none: usize,
}

#[derive(Serialize)]
pub struct Legacy {
    pub mapped: usize,
    pub unmapped: usize,
    pub homoglyph_fixes: usize,
    pub hyphenless: Hyphenless,
}

#[derive(Serialize)]
pub struct Hyphenless {
    pub definitions: usize,
    pub mentions: usize,
    /// `pattern-N` by descending count, every configured pattern.
    pub per_pattern: BTreeMap<String, usize>,
}

#[derive(Serialize)]
pub struct IdLike {
    pub claimed: usize,
    pub unclaimed: usize,
    /// Feature-scoped IDs cited outside every document defining them.
    pub feature_outside: usize,
    pub files_with_unclaimed: usize,
}

#[derive(Serialize)]
pub struct BrokenLinks {
    pub file: usize,
    pub wiki: usize,
    pub resolved_by_base: usize,
}

#[derive(Serialize)]
pub struct Code {
    pub files: usize,
    pub documents_cited: usize,
    pub citations: usize,
    pub roots_missing: usize,
}

#[derive(Serialize)]
pub struct Detail {
    pub files_skipped: usize,
    pub roots_missing: usize,
    pub diagnostics: usize,
    pub import_ms: u128,
}

#[derive(Serialize)]
struct Labels {
    classes: Vec<LabelRow>,
    prefixes: Vec<LabelRow>,
    patterns: Vec<LabelRow>,
}

/// `duplicates.json`: definitions of an ID already defined, and references
/// no definition answers.
#[derive(Serialize)]
struct Duplicates<'a> {
    duplicate_definitions: &'a [import::Duplicate],
    unresolved_references: &'a [import::Token],
}

pub fn run(corpus: &Corpus, config: CensusConfig) -> Result<ImportResult, String> {
    let out_dir = corpus.out.join("import").join(&corpus.label);
    fs::create_dir_all(&out_dir)
        .map_err(|error| format!("cannot create {}: {error}", out_dir.display()))?;

    let started = Instant::now();
    let found = import::run(&corpus.root, &config)?;
    let import_ms = started.elapsed().as_millis();

    let mut classes = BTreeMap::new();
    let mut unclassified = 0;
    for (class, count) in found.per_class() {
        match class {
            Some(class) => {
                classes.insert(class, count);
            }
            None => unclassified = count,
        }
    }
    let (mut per_class, class_labels) = anonymize(classes, CLASS_STEM);
    if unclassified > 0 {
        per_class.insert(UNCLASSIFIED.to_owned(), unclassified);
    }
    let (per_prefix, prefix_labels) = anonymize(found.per_prefix(), PREFIX_STEM);
    let patterns = &config.import.hyphenless;
    let mut pattern_counts = BTreeMap::new();
    for (pattern, count) in patterns.iter().zip(found.per_pattern(patterns.len())) {
        *pattern_counts.entry(pattern.source.clone()).or_default() += count;
    }
    let (per_pattern, pattern_labels) = anonymize(pattern_counts, PATTERN_STEM);

    write_json(
        &out_dir.join("labels.json"),
        &Labels {
            classes: class_labels,
            prefixes: prefix_labels,
            patterns: pattern_labels,
        },
    )?;
    write_json(&out_dir.join("documents.json"), &found.documents_detail)?;
    write_json(&out_dir.join("records.json"), &found.records)?;
    write_json(
        &out_dir.join("rows_without_id.json"),
        &found.rows_without_id,
    )?;
    write_json(
        &out_dir.join("duplicates.json"),
        &Duplicates {
            duplicate_definitions: &found.duplicates,
            unresolved_references: &found.unresolved,
        },
    )?;
    write_json(&out_dir.join("legacy.json"), &found.legacy)?;
    write_json(&out_dir.join("unclaimed.json"), &found.unclaimed)?;
    write_json(&out_dir.join("broken_links.json"), &found.links)?;
    write_json(&out_dir.join("code_citations.json"), &found.code.citations)?;
    write_json(&out_dir.join("diagnostics.json"), &found.diagnostics)?;

    let result = ImportResult {
        documents: Documents {
            total: found.documents,
            per_class,
        },
        front_matter: FrontMatter {
            yaml: found.header_forms(HeaderForm::Yaml),
            field_table: found.header_forms(HeaderForm::FieldTable),
            none: found.header_forms(HeaderForm::None),
            unclosed: found.header_forms(HeaderForm::Unclosed),
            non_latin_keys: found.non_latin_keys(),
            keys: MapCounts {
                mapped: found.keys(MapOutcome::Mapped),
                kept: found.keys(MapOutcome::Kept),
                unmapped: found.keys(MapOutcome::Unmapped),
            },
            values: MapCounts {
                mapped: found.values(MapOutcome::Mapped),
                kept: found.values(MapOutcome::Kept),
                unmapped: found.values(MapOutcome::Unmapped),
            },
        },
        records: Records {
            total: found.records.len(),
            empty_text: found.empty_text(),
            titled: found.titled(),
            per_form: PerForm {
                table_row: found.per_form(Form::TableRow),
                headerless_row: found.per_form(Form::HeaderlessRow),
                list_item: found.per_form(Form::ListItem),
                section: found.per_form(Form::Section),
                document: found.per_form(Form::Document),
            },
            per_prefix,
        },
        definitions: found.role(Role::Definition),
        references: References {
            total: found.role(Role::Reference),
            unresolved: found.unresolved.len(),
            by_document: found.by_document,
        },
        duplicate_definitions: found.duplicates.len(),
        rows_without_id: RowsWithoutId {
            local_number: found.rows_without_id(RowCause::LocalNumber),
            none: found.rows_without_id(RowCause::None),
        },
        legacy: Legacy {
            mapped: found.legacy.mapped.len(),
            unmapped: found.legacy.unmapped.len(),
            homoglyph_fixes: found.legacy.homoglyph_fixes.len(),
            hyphenless: Hyphenless {
                definitions: found.hyphenless(HyphenlessRole::Definition),
                mentions: found.hyphenless(HyphenlessRole::Mention),
                per_pattern,
            },
        },
        id_like: IdLike {
            claimed: found.claimed,
            unclaimed: found.unclaimed(TokenCause::Unclaimed),
            feature_outside: found.unclaimed(TokenCause::FeatureOutside),
            files_with_unclaimed: found.files_with_unclaimed(),
        },
        broken_links: BrokenLinks {
            file: found.links(LinkProblem::File),
            wiki: found.links(LinkProblem::Wiki),
            resolved_by_base: found.links(LinkProblem::ResolvedByBase),
        },
        code: Code {
            files: found.code.files,
            documents_cited: found.code.documents_cited(),
            citations: found.code.citations.len(),
            roots_missing: found.code.roots_missing,
        },
        detail: Detail {
            files_skipped: found.files_skipped,
            roots_missing: found.roots_missing,
            diagnostics: found.diagnostics.len(),
            import_ms,
        },
    };
    summarize(&result, &found, &corpus.label, &out_dir);
    Ok(result)
}

fn summarize(result: &ImportResult, found: &Import, label: &str, out_dir: &Path) {
    let forms = &result.records.per_form;
    eprintln!(
        "import [{label}]: {} documents ({} skipped), {} records: {} table rows, {} headerless rows, {} list items ({} titled), {} sections, {} documents",
        result.documents.total,
        result.detail.files_skipped,
        result.records.total,
        forms.table_row,
        forms.headerless_row,
        forms.list_item,
        result.records.titled,
        forms.section,
        forms.document
    );
    eprintln!(
        "  definitions {}, references {} ({} unresolved, {} demoted by a document), duplicate definitions {}",
        result.definitions,
        result.references.total,
        result.references.unresolved,
        result.references.by_document,
        result.duplicate_definitions
    );
    eprintln!(
        "  rows without ID: {} local numbers, {} none; legacy: {} mapped, {} unmapped, {} look-alike fixes",
        result.rows_without_id.local_number,
        result.rows_without_id.none,
        result.legacy.mapped,
        result.legacy.unmapped,
        result.legacy.homoglyph_fixes
    );
    eprintln!(
        "  hyphenless: {} definitions, {} mentions; ID-like: {} claimed, {} unclaimed in {} files, {} feature-scoped outside",
        result.legacy.hyphenless.definitions,
        result.legacy.hyphenless.mentions,
        result.id_like.claimed,
        result.id_like.unclaimed,
        result.id_like.files_with_unclaimed,
        result.id_like.feature_outside
    );
    eprintln!(
        "  broken links: {} file, {} wiki, {} resolved by base; code: {} files, {} documents cited {} times ({} ms)",
        result.broken_links.file,
        result.broken_links.wiki,
        result.broken_links.resolved_by_base,
        result.code.files,
        result.code.documents_cited,
        result.code.citations,
        result.detail.import_ms
    );
    for diagnostic in found.diagnostics.iter().take(DIAGNOSTICS_ON_STDERR) {
        match diagnostic.line {
            Some(line) => eprintln!("  {}:{line}: {}", diagnostic.path, diagnostic.message),
            None => eprintln!("  {}: {}", diagnostic.path, diagnostic.message),
        }
    }
    if found.diagnostics.len() > DIAGNOSTICS_ON_STDERR {
        eprintln!(
            "  ... {} more in diagnostics.json",
            found.diagnostics.len() - DIAGNOSTICS_ON_STDERR
        );
    }
    eprintln!("  detail: {}", out_dir.display());
}
