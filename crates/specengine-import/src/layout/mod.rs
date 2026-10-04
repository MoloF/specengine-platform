//! The import layout emitter (`docs/features/import-layout.md`): the
//! "before" model projected into the target layout
//! (`docs/canon/architecture.md` "Spec layout in a project", ADR-0026) as an
//! in-memory after-tree, plus the emission map the verifier locates each
//! record by. Pure: source bytes, the import model, the census config with
//! its `[layout]` and the before scheme (plain data, [`Scheme`]) in; files,
//! the map, the emitted `specengine.toml`, header outcomes and diagnostics
//! out. Nothing is written to disk and nothing of `specengine-core` is used:
//! the caller writes the tree and re-reads it through core.
//!
//! - **Definitions only**: a row, headerless row, list item or `{#ID}`
//!   section of a `file` prefix → `<records>/<P>/<ID>.md` (later duplicates
//!   `<ID>-2.md`, `-3`…); a row or item of a `section` prefix → a reshaped
//!   `{#ID}` section of its document; a section of a `section` prefix and a
//!   document record stay in place (the document gains `id`).
//! - **Feature documents** (holding a feature-scoped definition) move to
//!   `<features>/<slug>.md` unless already directly under `<features>` with a
//!   slug stem; a non-slug, shared or walked target keeps the document where
//!   it is and its feature-scoped definitions missing (`slug`).
//! - **Residue**: every walked document at its (moved) path, header
//!   normalised, every body line outside the extents carried elsewhere,
//!   reshaped blocks placed; LF-joined, no BOM, final LF.
//! - **Rule S**: a carried `{#written}` or `id:` value of a legacy-mapped or
//!   look-alike-fixed record becomes the Latin ID; no other byte changes.
//!
//! Every convention comes from the config (ADR-0008); the target format's
//! words (core's keys, classes, targets, scope) come from serde-named enums.
//! Output order is path, line, source order; no hash-map order reaches it.

mod body;
mod header;
mod records;
mod scheme;
mod toml_out;
mod typed;
mod yaml;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::RangeInclusive;
use std::path::Path;

use serde::Serialize;

use crate::config::{CensusConfig, Target};
use crate::frontmatter::{self, FrontMatter};
use crate::import::{
    Form, HeaderForm, Import, ImportRecord, Role, Scope, document_text, field_table, is_blank,
};
use crate::markdown;

pub use records::{SectionHeading, carried_fields, field_keys, section_heading};
pub use scheme::{Scheme, emit_scheme};
pub use typed::{CoreType, core_type, parses_as, typed_keys};

/// TOML text of a table, keys sorted at every level, final LF: the
/// hand-written writer of the emitted `specengine.toml`, for the caller's
/// other emitted configs (`.spec-debt.toml`).
pub fn toml_text(table: &toml::Table) -> String {
    toml_out::write(table)
}

/// One walked document as read: corpus-relative `/`-separated path, text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDocument {
    pub path: String,
    pub text: String,
    /// The before check could not parse its front-matter: the caller's
    /// core reading (`false` from [`read_sources`]); the header is then
    /// carried verbatim (`docs/features/import-layout.md` AC-05).
    pub header_unparsed: bool,
    /// The before check read its front-matter as no mapping
    /// (`frontmatter-not-mapping`; the caller's core reading, `false` from
    /// [`read_sources`]): carried verbatim with no core key added (AC-05).
    pub header_not_mapping: bool,
}

/// The walked documents that read as UTF-8, in walk order; the others are
/// the import's `files_skipped` and have no residue.
pub fn read_sources(root: &Path, documents: &[String]) -> Vec<SourceDocument> {
    documents
        .iter()
        .filter_map(|relative| {
            crate::census::read_document(root, relative)
                .ok()
                .map(|text| SourceDocument {
                    path: relative.clone(),
                    text,
                    header_unparsed: false,
                    header_not_mapping: false,
                })
        })
        .collect()
}

/// A walked document's body as the import reads it: the verifier's
/// "before" side of every container, of the prose comparison and of the
/// field table (`docs/features/import-layout.md` AC-02, AC-04, AC-05), the
/// reading the emitter splits, never its output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceBody {
    /// The body lines after the header (a YAML block; a leading BOM is no
    /// content): 1-based number, the line as written without its
    /// terminator (one CR before LF dropped).
    pub lines: Vec<(usize, String)>,
    /// The lines that leave the body: a field table becoming the header,
    /// with the blank lines right after it.
    pub excluded: Option<RangeInclusive<usize>>,
    /// The import scanner's headings: (line, level), in source order.
    pub headings: Vec<(usize, usize)>,
    /// The field table's lines the import reads a key and a value from:
    /// the header row's when `header_row_field`, then every data row's
    /// (an empty-key row's too), in order.
    pub field_rows: Vec<usize>,
    /// The field table's header line, where its extent opens
    /// (`docs/features/import-layout.md` AC-04).
    pub field_table_line: Option<usize>,
}

/// [`SourceBody`] of one document's text.
pub fn source_body(config: &CensusConfig, text: &str) -> SourceBody {
    let front = frontmatter::read(text, config.class_key.as_deref());
    let (body_start, body_line) = body_of(&front);
    let scan = markdown::scan_import(text, body_start, body_line, false);
    let found = field_table(config, &scan);
    SourceBody {
        lines: scan
            .lines
            .iter()
            .map(|line| (line.number, line.raw.to_owned()))
            .collect(),
        excluded: excluded_lines(&scan, found.as_ref(), body_line),
        headings: scan
            .headings
            .iter()
            .map(|heading| (heading.line, heading.level))
            .collect(),
        field_rows: found
            .as_ref()
            .and_then(|(index, _)| scan.tables.get(*index))
            .map(|table| {
                let header = (config.import.header_row_field && table.header.is_some())
                    .then_some(table.header_line);
                header
                    .into_iter()
                    .chain(table.rows.iter().map(|row| row.line))
                    .collect()
            })
            .unwrap_or_default(),
        field_table_line: found
            .as_ref()
            .and_then(|(index, _)| scan.tables.get(*index))
            .map(|table| table.header_line),
    }
}

/// Where the body starts: its byte offset and 1-based line.
fn body_of(front: &FrontMatter) -> (usize, usize) {
    match front {
        FrontMatter::Present {
            body_start,
            body_line,
            ..
        } => (*body_start, *body_line),
        _ => (0, 1),
    }
}

/// The field table and the blank lines right after it, which leave the
/// body (as `document` text does): the header carries the table.
fn excluded_lines(
    scan: &markdown::Scan<'_>,
    found: Option<&(usize, RangeInclusive<usize>)>,
    body_line: usize,
) -> Option<RangeInclusive<usize>> {
    let last_line = scan.lines.last().map_or(body_line, |line| line.number);
    found.map(|(_, lines)| {
        let end = *lines.end();
        let resume = scan
            .lines
            .iter()
            .find(|line| line.number > end && !is_blank(line.raw))
            .map(|line| line.number);
        *lines.start()..=resume.map_or(last_line, |number| number - 1)
    })
}

/// Where a definition landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Place {
    /// A record file of its own.
    File,
    /// A reshaped `{#ID}` section of its document.
    Section,
    /// Where it stood: a `{#ID}` section or a document record.
    InPlace,
}

/// Why a definition is missing or mismatched: the closed list of
/// `docs/canon/import-layout-verifier.md` "Output" (stdout `reasons`). The
/// emitter gives the first four; the verifier the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// Its prefix is not in the before scheme: no record emitted.
    PrefixUnknown,
    /// Its feature document has no usable `<features>/<slug>.md`.
    Slug,
    /// Its record path is a walked document's (or another record's).
    PathTaken,
    /// A field (or the task box) a reshaped heading cannot carry.
    SectionFields,
    /// The after file's front-matter does not parse.
    HeaderUnparseable,
    /// Core and the import scanner read different heading lines there.
    ReaderBoundary,
    Unexplained,
}

impl Reason {
    pub const ALL: [Reason; 7] = [
        Reason::PrefixUnknown,
        Reason::Slug,
        Reason::PathTaken,
        Reason::SectionFields,
        Reason::HeaderUnparseable,
        Reason::ReaderBoundary,
        Reason::Unexplained,
    ];
}

/// One definition in the emission map: its source identity → its after path
/// and place. No text, no hash: the verifier reads both from the tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Emitted {
    /// Source path, `/`-separated.
    pub path: String,
    /// Source line (the model's `line`).
    pub line: usize,
    pub id: String,
    pub form: Form,
    /// The after path; `None` when missing.
    pub after: Option<String>,
    pub place: Option<Place>,
    pub reason: Option<Reason>,
    /// Keys of its record file dropped as repeats (`header.conflicts`).
    pub conflicts: usize,
}

/// One walked document in the emission map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocumentPlace {
    pub source: String,
    pub after: String,
    pub moved: bool,
    /// A feature document of the tree: under `<features>`, holding an
    /// emitted feature-scoped definition.
    pub feature: bool,
}

/// The emission map.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Emission {
    /// Every definition of the model, in model order.
    pub definitions: Vec<Emitted>,
    /// Every source document, by source path.
    pub documents: Vec<DocumentPlace>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileKind {
    /// A walked document's residue.
    Residue,
    /// A record file.
    Record,
}

/// One file of the after-tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeFile {
    /// Root-relative, `/`-separated.
    pub path: String,
    pub kind: FileKind,
    /// LF-only, no BOM, final LF (empty for an empty residue).
    pub content: String,
}

/// A residue document's header, normalised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HeaderOutcome {
    /// Source path.
    pub path: String,
    pub after: String,
    /// The source's header form.
    pub source: HeaderForm,
    /// The after document opens with a header block.
    pub written: bool,
    /// Core keys the layout added (`id`, `class`, `aliases`), in order.
    pub added: Vec<String>,
    /// Keys renamed through `key_map`.
    pub renamed: usize,
    /// Values replaced through `value_map`.
    pub mapped: usize,
    /// A key not renamed, not added, or carried as `<key>-<n>` (a repeated
    /// field-table key) because it would repeat one.
    pub conflicts: usize,
    /// The field table's last row opens a comment its next line continues
    /// (`docs/canon/import.md` "Known limits").
    pub last_row_comment: bool,
}

/// A layout diagnostic; never fatal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LayoutDiagnostic {
    /// Source path; `None` for the configs.
    pub path: Option<String>,
    pub line: Option<usize>,
    pub reason: Option<Reason>,
    pub message: String,
}

impl LayoutDiagnostic {
    fn config(message: String) -> Self {
        Self {
            path: None,
            line: None,
            reason: None,
            message,
        }
    }

    fn at(path: &str, line: Option<usize>, reason: Option<Reason>, message: String) -> Self {
        Self {
            path: Some(path.to_owned()),
            line,
            reason,
            message,
        }
    }
}

/// Everything the emitter produced.
#[derive(Debug, Clone, Default)]
pub struct Layout {
    /// Sorted by path.
    pub files: Vec<TreeFile>,
    pub emission: Emission,
    /// One per residue document, by source path.
    pub headers: Vec<HeaderOutcome>,
    /// The after-tree's `specengine.toml` ([`emit_scheme`] over `files`).
    pub scheme_toml: String,
    pub diagnostics: Vec<LayoutDiagnostic>,
}

impl Layout {
    /// Whether a tree file has this path.
    pub fn holds(&self, path: &str) -> bool {
        self.files
            .binary_search_by(|file| file.path.as_str().cmp(path))
            .is_ok()
    }

    pub fn record_files(&self) -> usize {
        self.files
            .iter()
            .filter(|file| file.kind == FileKind::Record)
            .count()
    }

    /// Documents whose after path differs from their source path.
    pub fn moved(&self) -> usize {
        self.emission
            .documents
            .iter()
            .filter(|document| document.moved)
            .count()
    }

    pub fn feature_documents(&self) -> usize {
        self.emission
            .documents
            .iter()
            .filter(|document| document.feature)
            .count()
    }

    /// Definitions placed per place.
    pub fn placed(&self, place: Place) -> usize {
        self.emission
            .definitions
            .iter()
            .filter(|emitted| emitted.place == Some(place))
            .count()
    }

    /// Definitions per reason.
    pub fn reasons(&self, reason: Reason) -> usize {
        self.emission
            .definitions
            .iter()
            .filter(|emitted| emitted.reason == Some(reason))
            .count()
    }

    /// Header conflicts of residue documents and record files.
    pub fn conflicts(&self) -> usize {
        self.headers
            .iter()
            .map(|header| header.conflicts)
            .chain(
                self.emission
                    .definitions
                    .iter()
                    .map(|emitted| emitted.conflicts),
            )
            .sum()
    }
}

/// Rule S on a heading line: the first `{#written` anchor (followed by `}`
/// or a blank) becomes `{#id`; `None` when the line holds none.
pub fn s_heading(raw: &str, written: &str, id: &str) -> Option<String> {
    let needle = format!("{{#{written}");
    let mut from = 0;
    while let Some(found) = raw.get(from..).and_then(|rest| rest.find(&needle)) {
        let at = from + found;
        let after = at + needle.len();
        if raw[after..]
            .chars()
            .next()
            .is_none_or(|c| c == '}' || c.is_whitespace())
        {
            return Some(format!("{}{{#{id}{}", &raw[..at], &raw[after..]));
        }
        from = after;
    }
    None
}

/// A definition on its way through the emitter.
struct Plan<'m> {
    record: &'m ImportRecord,
    place: Option<Place>,
    after: Option<String>,
    reason: Option<Reason>,
    conflicts: usize,
}

/// The extension of every emitted record file and feature document.
const DOCUMENT_EXTENSION: &str = ".md";

/// Projects the model into the after-tree
/// (`docs/canon/import-layout.md` "Emission"). `sources` are the walked
/// documents as read ([`read_sources`]); a definition whose document is not
/// among them is missing (diagnosed). Never fails: every problem is a
/// diagnostic or a [`Reason`].
pub fn emit(
    config: &CensusConfig,
    import: &Import,
    sources: &[SourceDocument],
    scheme: &Scheme,
) -> Layout {
    let layout = &config.layout;
    let mut diagnostics = Vec::new();
    let sources: BTreeMap<&str, &SourceDocument> = sources
        .iter()
        .map(|source| (source.path.as_str(), source))
        .collect();

    // Each definition's intended place.
    let mut plans: Vec<Plan<'_>> = Vec::new();
    for record in import
        .records
        .iter()
        .filter(|record| record.role == Role::Definition)
    {
        let mut plan = Plan {
            record,
            place: None,
            after: None,
            reason: None,
            conflicts: 0,
        };
        if !sources.contains_key(record.path.as_str()) {
            diagnostics.push(LayoutDiagnostic::at(
                &record.path,
                Some(record.line),
                None,
                format!("`{}`: its document was not read; not emitted", record.id),
            ));
        } else if !scheme.has_prefix(&record.prefix) {
            plan.reason = Some(Reason::PrefixUnknown);
            diagnostics.push(LayoutDiagnostic::at(
                &record.path,
                Some(record.line),
                plan.reason,
                format!(
                    "`{}`: prefix `{}` is not in the scheme's `[ids]`; not emitted",
                    record.id, record.prefix
                ),
            ));
        } else {
            let target = layout.target(&record.prefix, record.scope == Scope::Feature);
            plan.place = Some(match (record.form, target) {
                (Form::Document, _) | (Form::Section, Target::Section) => Place::InPlace,
                (_, Target::Section) => Place::Section,
                (_, Target::File) => Place::File,
            });
        }
        plans.push(plan);
    }

    // Feature documents and the documents' after paths.
    let feature_documents: BTreeSet<&str> = plans
        .iter()
        .filter(|plan| plan.place.is_some() && plan.record.scope == Scope::Feature)
        .map(|plan| plan.record.path.as_str())
        .collect();
    let mut after: BTreeMap<&str, String> = sources
        .keys()
        .map(|path| (*path, (*path).to_owned()))
        .collect();
    let mut wanted: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let mut unslugged: BTreeSet<&str> = BTreeSet::new();
    for &path in &feature_documents {
        if feature_stem(&layout.features, path).is_some() {
            continue;
        }
        let slug = slug_of(config, path);
        if is_slug(&slug) {
            wanted
                .entry(format!("{}/{slug}{DOCUMENT_EXTENSION}", layout.features))
                .or_default()
                .push(path);
        } else {
            unslugged.insert(path);
            diagnostics.push(LayoutDiagnostic::at(
                path,
                None,
                Some(Reason::Slug),
                format!("`{slug}` is not a slug (`[a-z][a-z0-9-]*`): the feature document stays, its feature-scoped definitions are missing"),
            ));
        }
    }
    for (target, paths) in &wanted {
        let problem = if paths.len() > 1 {
            Some(format!(
                "{} feature documents would move to `{target}`: each stays, its feature-scoped definitions missing",
                paths.len()
            ))
        } else if sources.contains_key(target.as_str()) {
            Some(format!(
                "`{target}` is a walked document: the feature document stays, its feature-scoped definitions missing"
            ))
        } else {
            None
        };
        match problem {
            Some(message) => {
                for &path in paths {
                    unslugged.insert(path);
                    diagnostics.push(LayoutDiagnostic::at(
                        path,
                        None,
                        Some(Reason::Slug),
                        message.clone(),
                    ));
                }
            }
            None => {
                for &path in paths {
                    after.insert(path, target.clone());
                }
            }
        }
    }
    for plan in &mut plans {
        if plan.place.is_some()
            && plan.record.scope == Scope::Feature
            && unslugged.contains(plan.record.path.as_str())
        {
            plan.place = None;
            plan.reason = Some(Reason::Slug);
        }
    }

    // A residue without the document extension takes it (`moved`), so that
    // core walks it (AC-01); a name another document holds keeps it as it is.
    // Names compare case-insensitively: on a case-insensitive file system
    // (macOS) `a.md` and `A.md` are one file (`docs/features/import-layout.md`
    // AC-01).
    let current: BTreeSet<String> = after.values().map(|path| path.to_lowercase()).collect();
    let mut renames: BTreeMap<String, Vec<(&str, String)>> = BTreeMap::new();
    for (&path, after_path) in &after {
        if !after_path.ends_with(DOCUMENT_EXTENSION) {
            let target = with_document_extension(after_path);
            renames
                .entry(target.to_lowercase())
                .or_default()
                .push((path, target));
        }
    }
    for (folded, paths) in renames {
        match paths.as_slice() {
            [(path, target)] if !current.contains(&folded) => {
                after.insert(path, target.clone());
            }
            _ => {
                for (path, target) in &paths {
                    diagnostics.push(LayoutDiagnostic::at(
                        path,
                        None,
                        None,
                        format!("`{target}` is taken: the document keeps its name, which core does not walk"),
                    ));
                }
            }
        }
    }

    // Record paths: later duplicates numbered, a held path not written.
    let held: BTreeSet<String> = after.values().cloned().collect();
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for plan in &mut plans {
        let record = plan.record;
        match plan.place {
            Some(Place::File) => {
                let count = seen.entry(record.id.as_str()).or_default();
                *count += 1;
                let name = if *count == 1 {
                    record.id.clone()
                } else {
                    format!("{}-{count}", record.id)
                };
                let path = format!(
                    "{}/{}/{name}{DOCUMENT_EXTENSION}",
                    layout.records, record.prefix
                );
                if !file_name_safe(&record.id)
                    || !file_name_safe(&record.prefix)
                    || held.contains(&path)
                    || !used.insert(path.clone())
                {
                    plan.place = None;
                    plan.reason = Some(Reason::PathTaken);
                    diagnostics.push(LayoutDiagnostic::at(
                        &record.path,
                        Some(record.line),
                        plan.reason,
                        format!(
                            "`{}`: `{path}` is taken or no file path; not written",
                            record.id
                        ),
                    ));
                } else {
                    plan.after = Some(path);
                }
            }
            Some(Place::Section | Place::InPlace) => {
                plan.after = after.get(record.path.as_str()).cloned();
            }
            None => {}
        }
    }

    // Per document: residue, moved sections, reshaped blocks.
    let mut by_path: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, plan) in plans.iter().enumerate() {
        by_path
            .entry(plan.record.path.as_str())
            .or_default()
            .push(index);
    }
    let mut files: Vec<TreeFile> = Vec::new();
    let mut headers: Vec<HeaderOutcome> = Vec::new();
    let mut documents: Vec<DocumentPlace> = Vec::new();
    for (&path, source) in &sources {
        let after_path = after.get(path).cloned().unwrap_or_else(|| path.to_owned());
        let indices = by_path.get(path).cloned().unwrap_or_default();
        let output = document(
            config,
            source,
            &after_path,
            &mut plans,
            &indices,
            &mut diagnostics,
        );
        files.extend(output.records);
        files.push(TreeFile {
            path: after_path.clone(),
            kind: FileKind::Residue,
            content: output.residue,
        });
        headers.push(output.header);
        documents.push(DocumentPlace {
            source: path.to_owned(),
            moved: after_path != path,
            feature: feature_documents.contains(path) && !unslugged.contains(path),
            after: after_path,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let (scheme_toml, scheme_diagnostics) =
        emit_scheme(scheme, config, files.iter().map(|file| file.path.as_str()));
    diagnostics.extend(scheme_diagnostics);

    let definitions = plans
        .into_iter()
        .map(|plan| Emitted {
            path: plan.record.path.clone(),
            line: plan.record.line,
            id: plan.record.id.clone(),
            form: plan.record.form,
            after: plan.after,
            place: plan.place,
            reason: plan.reason,
            conflicts: plan.conflicts,
        })
        .collect();
    Layout {
        files,
        emission: Emission {
            definitions,
            documents,
        },
        headers,
        scheme_toml,
        diagnostics,
    }
}

/// One source document's share of the tree.
struct DocumentOutput {
    residue: String,
    records: Vec<TreeFile>,
    header: HeaderOutcome,
}

/// The residue of one document and the record files of its definitions.
fn document(
    config: &CensusConfig,
    source: &SourceDocument,
    after: &str,
    plans: &mut [Plan<'_>],
    indices: &[usize],
    diagnostics: &mut Vec<LayoutDiagnostic>,
) -> DocumentOutput {
    let text = source.text.as_str();
    let front = frontmatter::read(text, config.class_key.as_deref());
    let (body_start, body_line) = body_of(&front);
    let scan = markdown::scan_import(text, body_start, body_line, false);
    let found = field_table(config, &scan);
    let excluded = excluded_lines(&scan, found.as_ref(), body_line);

    // Plan indices of the moved sections, reshaped rows and items, in-place
    // sections, in source order.
    let mut moved: Vec<usize> = Vec::new();
    let mut reshaped_plans: Vec<usize> = Vec::new();
    let mut in_place: Vec<usize> = Vec::new();
    let mut dropped: Vec<[usize; 2]> = Vec::new();
    let mut reshaped: Vec<body::Reshaped> = Vec::new();
    let mut rule_s: BTreeMap<usize, (String, String)> = BTreeMap::new();
    let mut in_place_document: Option<usize> = None;
    for &index in indices {
        let plan = &mut plans[index];
        let record = plan.record;
        match (plan.place, record.form) {
            (Some(Place::File), Form::Section) => moved.push(index),
            (Some(Place::File), _) => dropped.push(record.extent),
            (Some(Place::Section), _) => {
                dropped.push(record.extent);
                let (heading, lost) = records::reshaped_heading(record, config);
                if lost > 0 {
                    plan.reason = Some(Reason::SectionFields);
                    diagnostics.push(LayoutDiagnostic::at(
                        &record.path,
                        Some(record.line),
                        plan.reason,
                        format!(
                            "`{}`: {lost} field(s) a heading attribute cannot carry (a blank, a brace or a repeated key)",
                            record.id
                        ),
                    ));
                }
                reshaped.push(body::Reshaped {
                    extent: record.extent,
                    heading,
                    text: record.text.clone(),
                });
                reshaped_plans.push(index);
            }
            (Some(Place::InPlace), Form::Section) => {
                if record.written != record.id {
                    rule_s.insert(record.line, (record.written.clone(), record.id.clone()));
                }
                in_place.push(index);
            }
            (Some(Place::InPlace), Form::Document) => in_place_document = Some(index),
            _ => {}
        }
    }

    let split = body::split(&body::BodyPlan {
        scan: &scan,
        excluded,
        moved: moved
            .iter()
            .map(|&index| body::MovedSection {
                extent: plans[index].record.extent,
            })
            .collect(),
        dropped,
        reshaped,
        rule_s,
        in_place: in_place
            .iter()
            .map(|&index| plans[index].record.line)
            .collect(),
    });
    // A block or section nested in a moved section lands in its record file.
    let landed: Vec<(usize, Option<usize>)> = reshaped_plans
        .iter()
        .copied()
        .zip(split.reshaped_in.iter().copied())
        .chain(
            in_place
                .iter()
                .copied()
                .zip(split.in_place_in.iter().copied()),
        )
        .collect();
    for (index, section) in landed {
        if let Some(slot) = section {
            plans[index].after = moved
                .get(slot)
                .and_then(|&holder| plans[holder].after.clone());
        }
    }

    let mut record_files = Vec::new();
    let heading_raw = |line: usize| {
        scan.lines
            .iter()
            .find(|candidate| candidate.number == line)
            .map_or("", |candidate| candidate.raw)
    };
    for (slot, &index) in moved.iter().enumerate() {
        let plan = &mut plans[index];
        let heading = section_heading(heading_raw(plan.record.line));
        let body = split
            .sections
            .get(slot)
            .map_or(String::new(), |lines| document_text(&lines.join("\n")));
        let (content, conflicts) = records::render(
            &records::RecordFile {
                record: plan.record,
                title: heading.as_ref().map(|heading| heading.title.clone()),
                attributes: heading
                    .map(|heading| heading.attributes)
                    .unwrap_or_default(),
                body,
            },
            config,
        );
        plan.conflicts = conflicts;
        if let Some(path) = &plan.after {
            record_files.push(TreeFile {
                path: path.clone(),
                kind: FileKind::Record,
                content,
            });
        }
    }
    for &index in indices {
        let plan = &mut plans[index];
        if plan.place != Some(Place::File) || plan.record.form == Form::Section {
            continue;
        }
        let (content, conflicts) = records::render(
            &records::RecordFile {
                record: plan.record,
                title: plan.record.title.clone(),
                attributes: Vec::new(),
                body: document_text(&plan.record.text),
            },
            config,
        );
        plan.conflicts = conflicts;
        if let Some(path) = &plan.after {
            record_files.push(TreeFile {
                path: path.clone(),
                kind: FileKind::Record,
                content,
            });
        }
    }

    let document = in_place_document.map(|index| {
        let record = plans[index].record;
        (record.id.as_str(), record.written.as_str())
    });
    let document_line = in_place_document.map(|index| plans[index].record.line);
    let normalised = header::normalise(
        &header::HeaderPlan {
            source: &source.path,
            after,
            text,
            front: &front,
            scan: &scan,
            field_table: found.as_ref().map(|(index, _)| *index),
            document,
            document_line,
            header_unparsed: source.header_unparsed,
            header_not_mapping: source.header_not_mapping,
        },
        config,
        diagnostics,
    );
    let mut lines = normalised.lines;
    lines.extend(split.residue);
    let residue = if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    };
    DocumentOutput {
        residue,
        records: record_files,
        header: normalised.outcome,
    }
}

/// A slug, `[a-z][a-z0-9-]*` (core's rule for a feature document's stem).
fn is_slug(text: &str) -> bool {
    let mut bytes = text.bytes();
    bytes.next().is_some_and(|first| first.is_ascii_lowercase())
        && bytes.all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-'))
}

/// The stem of `<features>/<stem>.md` when it is a slug.
fn feature_stem<'p>(features: &str, path: &'p str) -> Option<&'p str> {
    let stem = path
        .strip_prefix(features)?
        .strip_prefix('/')?
        .strip_suffix(DOCUMENT_EXTENSION)?;
    is_slug(stem).then_some(stem)
}

/// A feature document's slug: `[layout] slug`'s group over its source path,
/// else its file name without the extension.
fn slug_of(config: &CensusConfig, path: &str) -> String {
    if let Some(slug) = config
        .layout
        .slug
        .as_ref()
        .and_then(|pattern| pattern.captures(path))
        .and_then(|captures| captures.name("slug"))
    {
        return slug.as_str().to_owned();
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.')
        .map_or(name, |(stem, _)| stem)
        .to_owned()
}

/// `path` with the document extension: its file name's extension replaced
/// (a name without one, or only one, gains it).
fn with_document_extension(path: &str) -> String {
    let name = path.rfind('/').map_or(0, |at| at + 1);
    let stem_end = path[name..]
        .rfind('.')
        .filter(|&at| at > 0)
        .map_or(path.len(), |at| name + at);
    format!("{}{DOCUMENT_EXTENSION}", &path[..stem_end])
}

/// A path component the record path may use: not empty, no separator, no
/// dot leading it.
fn file_name_safe(text: &str) -> bool {
    !text.is_empty() && !text.starts_with('.') && !text.contains(['/', '\\'])
}
