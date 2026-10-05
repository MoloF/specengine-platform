//! The pure half of a proposal of kind `update` (task spec
//! `proposal-apply`, "Rules and edge cases"): one node's span found by its
//! ID in a fresh parse, the span's raw bytes, the new text spliced into
//! exactly that span, the structure check of the result, the creation
//! refusals that need only the parse and the scheme, and the findings an
//! edit introduces. Nothing is read, written or hashed here: the caller
//! reads the bytes, the store hashes them (`b3_hash`) and writes.
//!
//! - **Span**: a node's `span` (`Node::span`): a document's whole file, a
//!   section's heading to the next heading of the same or a higher level,
//!   nested sections included, trailing whitespace excluded. Its bytes are
//!   the file's own: no line ending added, no U+FFFD.
//! - **Text** ([`update_text`]): the new text verbatim (A8), but a
//!   section's trailing whitespace (`' '`, `'\t'`, `'\r'`, `'\n'`, the
//!   parser's rule) dropped, since no section span ends in it.
//! - **Structure** ([`check_structure`]): the patched file, parsed afresh,
//!   keeps the ordered list of (ID, heading level) of its nodes, and the
//!   target's span there is exactly the inserted text: an `{#ID}` dropped or
//!   added, a level changed, or a heading of the same or a higher level
//!   added inside the text is refused.
//! - **Validation** ([`PatchCheck::introduced`]): the check over the
//!   unpatched input and over the same input with the one file replaced,
//!   judged as the check against `HEAD` judges (canon `spec-check-git.md`,
//!   "The base"): the unpatched run, no baseline, is the base; only the
//!   findings it lacks are kept. Never a refusal (ADR-0012).

use specengine_model::{IdScheme, Node, ParsedFile, RefForm, Reference, Span};

use crate::Paths;
// Under another name: core's sources are scanned for file access by a text
// match that a `CheckFile` path call would trip (`specengine-eval`'s build
// graph test).
use crate::check::CheckFile as InputEntry;
use crate::check::{
    Base, Baseline, CheckConfig, CheckInput, DocClass, Finding, Resolution, Resolver, judge, run,
};

/// The characters a section's span never ends in (the parser's trimming).
pub const SECTION_TRAILING_WHITESPACE: [char; 4] = [' ', '\t', '\r', '\n'];

/// One entry of a file's structure: a node's ID (`None`: a document
/// without one) and its heading level (`None`: the document).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructureEntry {
    pub id: Option<String>,
    pub level: Option<u8>,
}

impl std::fmt::Display for StructureEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let id = self.id.as_deref().unwrap_or("(no ID)");
        match self.level {
            Some(level) => write!(f, "{id} (level {level})"),
            None => write!(f, "{id} (the document)"),
        }
    }
}

/// The ordered (ID, heading level) list of `parsed`'s nodes: the document,
/// then every `{#ID}` section in source order. A file that is not UTF-8
/// has none.
pub fn structure(parsed: &ParsedFile) -> Vec<StructureEntry> {
    parsed
        .nodes
        .iter()
        .map(|node| StructureEntry {
            id: node.id.clone(),
            level: node.level,
        })
        .collect()
}

/// Why no single node of a file holds an ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocateError {
    /// No node holds it.
    Absent,
    /// This many nodes hold it.
    Repeated(usize),
}

/// The position in `parsed.nodes` of the one node whose ID is exactly
/// `id` (the Latin ID; for a feature-scoped one, without its `slug/`).
pub fn locate(parsed: &ParsedFile, id: &str) -> Result<usize, LocateError> {
    let mut found = parsed
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.id.as_deref() == Some(id))
        .map(|(ord, _)| ord);
    let first = found.next().ok_or(LocateError::Absent)?;
    match found.count() {
        0 => Ok(first),
        more => Err(LocateError::Repeated(more + 1)),
    }
}

/// The raw bytes of `node`'s span in `bytes` (the file it was parsed
/// from): what `spec show`'s `span_hash` and a proposal's `base_hash`
/// hash. Empty when the span lies outside `bytes` (never for a parse of
/// them).
pub fn span_bytes<'b>(bytes: &'b [u8], node: &Node) -> &'b [u8] {
    bytes.get(node.span.range()).unwrap_or_default()
}

/// The node is a section (it has a heading level), not the document.
pub fn is_section(node: &Node) -> bool {
    node.level.is_some()
}

/// `text` as spliced and stored: verbatim, but a section's trailing
/// [`SECTION_TRAILING_WHITESPACE`] dropped.
pub fn update_text(text: &str, section: bool) -> &str {
    if section {
        text.trim_end_matches(SECTION_TRAILING_WHITESPACE)
    } else {
        text
    }
}

/// `bytes` with `span` replaced by `text`, everything else byte for byte.
/// A span outside `bytes` is clamped to them.
pub fn splice(bytes: &[u8], span: Span, text: &str) -> Vec<u8> {
    let end = span.end.min(bytes.len());
    let start = span.start.min(end);
    let mut out = Vec::with_capacity(bytes.len() - (end - start) + text.len());
    out.extend_from_slice(&bytes[..start]);
    out.extend_from_slice(text.as_bytes());
    out.extend_from_slice(&bytes[end..]);
    out
}

/// Why a patched file was refused by the structure check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructureError {
    /// The ordered (ID, heading level) list changed; `at` is the first
    /// position that differs, `before` and `after` the entries there
    /// (`None`: the list ended).
    Changed {
        at: usize,
        before: Option<StructureEntry>,
        after: Option<StructureEntry>,
    },
    /// The target's span in the patched file is not exactly the inserted
    /// text (a heading of the same or a higher level inside it, or text
    /// that runs into what follows).
    SpanDiffers { expected: Span, found: Span },
}

impl std::fmt::Display for StructureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let entry = |entry: &Option<StructureEntry>| {
            entry
                .as_ref()
                .map_or_else(|| "nothing".to_owned(), ToString::to_string)
        };
        match self {
            Self::Changed { at, before, after } => write!(
                f,
                "the edit changes the file's structure: node {} was {}, would be {}; an update \
                 keeps every `{{#ID}}` heading and its level",
                at + 1,
                entry(before),
                entry(after)
            ),
            Self::SpanDiffers { expected, found } => write!(
                f,
                "the edit does not stay one node: the new text is bytes {}-{} but the node \
                 would span {}-{} (a heading of the same or a higher level inside the text, or \
                 text running into what follows)",
                expected.start, expected.end, found.start, found.end
            ),
        }
    }
}

impl std::error::Error for StructureError {}

/// `after` (the patched file, parsed afresh) keeps `before`'s structure,
/// and its node `ord` (the target, the same position in both) spans exactly
/// `inserted`.
pub fn check_structure(
    before: &ParsedFile,
    after: &ParsedFile,
    ord: usize,
    inserted: Span,
) -> Result<(), StructureError> {
    let old = structure(before);
    let new = structure(after);
    if old != new {
        let at = old
            .iter()
            .zip(&new)
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| old.len().min(new.len()));
        return Err(StructureError::Changed {
            at,
            before: old.get(at).cloned(),
            after: new.get(at).cloned(),
        });
    }
    let found = after
        .nodes
        .get(ord)
        .map_or(Span::default(), |node| node.span);
    if found != inserted {
        return Err(StructureError::SpanDiffers {
            expected: inserted,
            found,
        });
    }
    Ok(())
}

/// One node's span replaced in one file's bytes, checked.
#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    /// The target's position in both parses.
    pub ord: usize,
    /// The target's span in the unpatched bytes.
    pub base_span: Span,
    /// The text spliced in ([`update_text`]): what a proposal stores as
    /// `new_text`.
    pub text: String,
    /// The patched bytes.
    pub bytes: Vec<u8>,
    /// Their fresh parse.
    pub parsed: ParsedFile,
}

/// Node `ord` of `parsed` (the parse of `bytes`) replaced by `new_text`:
/// [`update_text`], [`splice`], a fresh parse of the result under
/// `scheme` (named `path`), [`check_structure`].
pub fn update(
    path: &str,
    bytes: &[u8],
    parsed: &ParsedFile,
    ord: usize,
    new_text: &str,
    scheme: &IdScheme,
) -> Result<Update, StructureError> {
    let Some(node) = parsed.nodes.get(ord) else {
        return Err(StructureError::Changed {
            at: ord,
            before: None,
            after: None,
        });
    };
    let base_span = node.span;
    let text = update_text(new_text, is_section(node));
    let patched = splice(bytes, base_span, text);
    let reparsed = crate::parse(path, &patched, scheme);
    let inserted = Span::new(base_span.start, base_span.start + text.len());
    check_structure(parsed, &reparsed, ord, inserted)?;
    Ok(Update {
        ord,
        base_span,
        text: text.to_owned(),
        bytes: patched,
        parsed: reparsed,
    })
}

/// Why a resolved node may not be the target of an update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Its file is a `class: generated` document: only its generator
    /// writes it (`#apply`).
    Generated,
    /// The ID's prefix (the node's, or its document's for a section) has
    /// `immutable_text` (owner's Q5).
    ImmutableText { id: String, prefix: String },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Generated => f.write_str(
                "its file is a `class: generated` document: only its registered generator \
                 writes it, never a proposal",
            ),
            Self::ImmutableText { id, prefix } => write!(
                f,
                "`{id}` has the prefix `{prefix}`, whose text is immutable \
                 (`immutable_text` in `[ids]`): it is never updated"
            ),
        }
    }
}

/// Why node `ord` of `parsed` may not be updated: its document is
/// `class: generated`, or its ID's prefix, or its document's ID's prefix,
/// has `immutable_text` in `scheme`. `None`: it may.
pub fn update_refusal(parsed: &ParsedFile, ord: usize, scheme: &IdScheme) -> Option<Refusal> {
    let document = parsed.document()?;
    let class = document
        .fields
        .as_ref()
        .and_then(|fields| fields.class.as_deref());
    if class == Some(DocClass::Generated.as_str()) {
        return Some(Refusal::Generated);
    }
    let mut ids = vec![parsed.nodes.get(ord)?.id.as_deref()];
    if ord > 0 {
        ids.push(document.id.as_deref());
    }
    for id in ids.into_iter().flatten() {
        let Some((prefix, _)) = id.split_once('-') else {
            continue;
        };
        if scheme
            .prefix(prefix)
            .is_some_and(|spec| spec.immutable_text)
        {
            return Some(Refusal::ImmutableText {
                id: id.to_owned(),
                prefix: prefix.to_owned(),
            });
        }
    }
    None
}

/// How a written target departs from the canonical form `ID` or
/// `slug/ID`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetForm {
    /// `ID` or `slug/ID`.
    Canonical,
    /// A legacy `aliases_from` prefix: the canonical ID is `PREFIX-body`.
    Alias { canonical: String },
    /// `ID#SECTION`: the canonical target is the section's own ID (or its
    /// `slug/ID`, which only resolution can tell).
    Section { section: String },
    /// `ID@rev`: the canonical target is the ID alone.
    Revision { canonical: String },
    /// `[[…]]`: the canonical target is the reference inside.
    Wiki { canonical: String },
    /// `project:ID`: not supported (exit 2, as `spec show`).
    Project,
}

/// The form of `reference` as a target: [`TargetForm::Canonical`] for a
/// bare `ID` or `slug/ID` of a configured prefix; else what it departs by,
/// with the canonical text when the reference alone tells it. An
/// `aliases:` entry of a document parses as an ID too: only resolution
/// shows it (the resolved node's ID differs from the written one).
pub fn target_form(reference: &Reference) -> TargetForm {
    let qualified = |id: &str| match &reference.scope {
        Some(scope) => format!("{scope}/{id}"),
        None => id.to_owned(),
    };
    if reference.project.is_some() {
        return TargetForm::Project;
    }
    if let Some(prefix) = &reference.alias_of {
        let body = reference.id.split_once('-').map_or("", |(_, body)| body);
        return TargetForm::Alias {
            canonical: qualified(&format!("{prefix}-{body}")),
        };
    }
    if let Some(section) = &reference.section {
        return TargetForm::Section {
            section: section.clone(),
        };
    }
    if reference.rev.is_some() {
        return TargetForm::Revision {
            canonical: qualified(&reference.id),
        };
    }
    if reference.form != RefForm::Bare {
        return TargetForm::Wiki {
            canonical: qualified(&reference.id),
        };
    }
    TargetForm::Canonical
}

/// Why a target reference has no single holder file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HolderError {
    /// The resolver's reason (unknown ID, a scope that does not hold it…).
    Dangling(String),
    /// Several files hold it: their root-relative paths.
    Several(Vec<String>),
    /// A `project:` reference: not resolved here.
    Project,
}

/// The one file holding `reference` as `spec show` resolves it
/// ([`Resolver::resolve_detached`]): its index in the resolver's paths.
pub fn one_holder(
    resolver: &Resolver<'_>,
    reference: &Reference,
    written: &str,
) -> Result<usize, HolderError> {
    match resolver.resolve_detached(reference, written) {
        Resolution::Resolved(files) => match files.as_slice() {
            [file] => Ok(*file),
            files => Err(HolderError::Several(
                files
                    .iter()
                    .map(|&file| resolver.paths()[file].to_owned())
                    .collect(),
            )),
        },
        Resolution::Dangling(reason) => Err(HolderError::Dangling(reason)),
        Resolution::Skipped => Err(HolderError::Project),
    }
}

/// The rules a patched file is validated under: the project's scheme,
/// `[paths]`, check tables and baseline, and the date.
#[derive(Debug, Clone, Copy)]
pub struct PatchCheck<'a> {
    pub scheme: &'a IdScheme,
    pub paths: &'a Paths,
    pub config: &'a CheckConfig,
    pub baseline: &'a Baseline,
    /// `YYYY-MM-DD`.
    pub today: &'a str,
}

impl PatchCheck<'_> {
    /// The findings the edit introduces: the check over `input` with the
    /// file `path` replaced by `patched` (parsed here; added in path order
    /// when `input` lacks it), judged against the check over `input`
    /// itself with no baseline ([`judge`]: a finding whose (code, path,
    /// subject) the unpatched run lacks). In the patched run's order;
    /// pre-existing findings are never listed.
    pub fn introduced(&self, input: &CheckInput, path: &str, patched: Vec<u8>) -> Vec<Finding> {
        let unpatched = run(
            input,
            self.scheme,
            self.paths,
            self.config,
            &Baseline::default(),
            self.today,
        );
        let patched_input = with_file(input, InputEntry::parse(path, patched, self.scheme));
        let report = run(
            &patched_input,
            self.scheme,
            self.paths,
            self.config,
            self.baseline,
            self.today,
        );
        let base = Base {
            findings: unpatched.findings,
            baseline: None,
            mode: None,
        };
        judge(report, self.baseline, &base)
            .findings
            .into_iter()
            .filter(|finding| finding.introduced == Some(true))
            .collect()
    }
}

/// `input` with `file` in place of the file at its path, or inserted in
/// path order when there is none.
fn with_file(input: &CheckInput, file: InputEntry) -> CheckInput {
    let mut patched = input.clone();
    match patched
        .files
        .iter()
        .position(|other| other.path == file.path)
    {
        Some(at) => patched.files[at] = file,
        None => {
            let at = patched
                .files
                .partition_point(|other| other.path.as_bytes() < file.path.as_bytes());
            patched.files.insert(at, file);
        }
    }
    patched
}
