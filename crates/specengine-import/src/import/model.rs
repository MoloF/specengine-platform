//! The record model and the detail of the "before" report
//! (`docs/features/import-records.md` AC-03, AC-07); aggregates are methods
//! of [`Import`].

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::census::Diagnostic;
use crate::script::IdScript;

use super::code::CodeScan;
use super::header::{KeyEntry, MapOutcome, ValueEntry};

/// Where a record is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Form {
    TableRow,
    HeaderlessRow,
    ListItem,
    Section,
}

impl Form {
    pub const ALL: [Form; 4] = [
        Form::TableRow,
        Form::HeaderlessRow,
        Form::ListItem,
        Form::Section,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    Definition,
    Reference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    Project,
    /// Unique per document (`ids.feature_prefixes`, ADR-0026).
    Feature,
}

/// One cell of a record row besides the ID and the text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field {
    /// The header cell as written; `col-N` (0-based) without a header, for
    /// an empty header cell or a repeated one.
    pub header: String,
    pub value: String,
}

/// One record of the model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportRecord {
    /// Corpus-relative, `/`-separated.
    pub path: String,
    /// 1-based line of the ID.
    pub line: usize,
    pub form: Form,
    /// Latin: legacy prefix mapped, then look-alikes normalised.
    pub id: String,
    pub prefix: String,
    /// The ID as written when a legacy prefix was mapped.
    pub aliases: Vec<String>,
    /// Script of the ID as written.
    pub script: IdScript,
    pub role: Role,
    pub scope: Scope,
    /// Verbatim text: bytes as written, lines joined by LF.
    pub text: String,
    /// BLAKE3 of `text`, hex.
    pub hash: String,
    /// Table rows only.
    pub fields: Vec<Field>,
}

/// A position in the corpus with what is written there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Token {
    pub path: String,
    pub line: usize,
    pub token: String,
}

/// Why an ID-like token is not claimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TokenCause {
    /// It names no ID, or an ID no document defines.
    Unclaimed,
    /// A feature-scoped ID cited from a document that does not define it,
    /// defined only in others.
    FeatureOutside,
}

/// An ID-like token that is not claimed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnclaimedToken {
    pub path: String,
    pub line: usize,
    pub token: String,
    pub cause: TokenCause,
}

/// Why a record-table row holds no record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RowCause {
    /// The ID cell matches `tables.local_number`.
    LocalNumber,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RowWithoutId {
    pub path: String,
    pub line: usize,
    pub cause: RowCause,
    /// The ID cell, cleaned of decoration.
    pub cell: String,
}

/// A definition of an ID already defined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Duplicate {
    pub path: String,
    pub line: usize,
    pub id: String,
    pub scope: Scope,
    pub first_path: String,
    pub first_line: usize,
}

/// A record ID mapped or normalised at import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdChange {
    pub path: String,
    pub line: usize,
    pub written: String,
    pub id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HyphenlessRole {
    /// At a strong lead-in of a list item or a paragraph.
    Definition,
    Mention,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HyphenlessMatch {
    pub path: String,
    pub line: usize,
    /// Index into `ids.hyphenless`.
    pub pattern: usize,
    pub token: String,
    pub role: HyphenlessRole,
}

/// Legacy prefixes, look-alikes and hyphenless codes.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Legacy {
    pub mapped: Vec<IdChange>,
    pub unmapped: Vec<Token>,
    pub homoglyph_fixes: Vec<IdChange>,
    pub hyphenless: Vec<HyphenlessMatch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkProblem {
    /// A Markdown file link whose file does not exist.
    File,
    /// A wiki link that resolves nowhere.
    Wiki,
    /// Missing from its document, found from `links.base`.
    ResolvedByBase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportLink {
    pub path: String,
    pub line: usize,
    pub target: String,
    pub problem: LinkProblem,
}

/// How a document's header is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HeaderForm {
    Yaml,
    FieldTable,
    /// No header, or the document was skipped.
    None,
    Unclosed,
}

/// Per-document detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocumentDetail {
    pub path: String,
    pub bytes: usize,
    pub skipped: bool,
    pub header: HeaderForm,
    pub class: Option<String>,
    pub non_latin_keys: bool,
    pub keys: Vec<KeyEntry>,
    pub values: Vec<ValueEntry>,
    pub records: usize,
    pub unclaimed: usize,
}

/// Everything the import found: the record model and the detail of the
/// "before" report. Aggregates are methods.
#[derive(Debug, Clone, Default)]
pub struct Import {
    /// Documents walked, the skipped ones included.
    pub documents: usize,
    pub files_skipped: usize,
    /// Configured corpus roots that could not be read.
    pub roots_missing: usize,
    pub documents_detail: Vec<DocumentDetail>,
    /// In path, line, form order.
    pub records: Vec<ImportRecord>,
    pub rows_without_id: Vec<RowWithoutId>,
    pub duplicates: Vec<Duplicate>,
    /// References whose ID no definition carries.
    pub unresolved: Vec<Token>,
    pub legacy: Legacy,
    /// ID-like tokens whose ID is defined (a feature-scoped one in the
    /// citing document).
    pub claimed: usize,
    /// The other ID-like tokens, `unclaimed` and `feature-outside`.
    pub unclaimed: Vec<UnclaimedToken>,
    pub links: Vec<ImportLink>,
    pub code: CodeScan,
    pub diagnostics: Vec<Diagnostic>,
}

impl Import {
    /// Documents per class value (`None`: no class, skipped ones included).
    pub fn per_class(&self) -> BTreeMap<Option<String>, usize> {
        let mut counts = BTreeMap::new();
        for document in &self.documents_detail {
            *counts.entry(document.class.clone()).or_default() += 1;
        }
        counts
    }

    pub fn header_forms(&self, form: HeaderForm) -> usize {
        self.documents_detail
            .iter()
            .filter(|document| document.header == form)
            .count()
    }

    pub fn non_latin_keys(&self) -> usize {
        self.documents_detail
            .iter()
            .filter(|document| document.non_latin_keys)
            .count()
    }

    /// Header keys per outcome.
    pub fn keys(&self, outcome: MapOutcome) -> usize {
        self.documents_detail
            .iter()
            .flat_map(|document| &document.keys)
            .filter(|key| key.outcome == outcome)
            .count()
    }

    /// Header values per outcome.
    pub fn values(&self, outcome: MapOutcome) -> usize {
        self.documents_detail
            .iter()
            .flat_map(|document| &document.values)
            .filter(|value| value.outcome == outcome)
            .count()
    }

    pub fn empty_text(&self) -> usize {
        self.records
            .iter()
            .filter(|record| record.text.is_empty())
            .count()
    }

    pub fn per_form(&self, form: Form) -> usize {
        self.records
            .iter()
            .filter(|record| record.form == form)
            .count()
    }

    /// Records (definitions and references) per Latin prefix.
    pub fn per_prefix(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for record in &self.records {
            *counts.entry(record.prefix.clone()).or_default() += 1;
        }
        counts
    }

    pub fn role(&self, role: Role) -> usize {
        self.records
            .iter()
            .filter(|record| record.role == role)
            .count()
    }

    pub fn rows_without_id(&self, cause: RowCause) -> usize {
        self.rows_without_id
            .iter()
            .filter(|row| row.cause == cause)
            .count()
    }

    pub fn hyphenless(&self, role: HyphenlessRole) -> usize {
        self.legacy
            .hyphenless
            .iter()
            .filter(|found| found.role == role)
            .count()
    }

    /// Hyphenless matches per pattern index, every configured pattern.
    pub fn per_pattern(&self, patterns: usize) -> Vec<usize> {
        let mut counts = vec![0; patterns];
        for found in &self.legacy.hyphenless {
            if let Some(count) = counts.get_mut(found.pattern) {
                *count += 1;
            }
        }
        counts
    }

    /// ID-like tokens not claimed, per cause.
    pub fn unclaimed(&self, cause: TokenCause) -> usize {
        self.unclaimed
            .iter()
            .filter(|token| token.cause == cause)
            .count()
    }

    /// Documents holding a token of cause `unclaimed`.
    pub fn files_with_unclaimed(&self) -> usize {
        self.unclaimed
            .iter()
            .filter(|token| token.cause == TokenCause::Unclaimed)
            .map(|token| token.path.as_str())
            .collect::<BTreeSet<_>>()
            .len()
    }

    pub fn links(&self, problem: LinkProblem) -> usize {
        self.links
            .iter()
            .filter(|link| link.problem == problem)
            .count()
    }
}
