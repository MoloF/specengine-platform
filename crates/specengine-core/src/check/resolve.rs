//! Reference resolution over one corpus (docs/canon/spec-check.md,
//! "Rules", References): a reference resolves when its ID is defined (a
//! document `id:` or a `{#ID}` section), or its text is in a document's
//! `aliases:`, or, through `aliases_from`, the configured prefix + the
//! written body is defined (no re-padding); a `#Y` section must be defined
//! in a file holding the ID. `project:` and `slug/` references are skipped
//! (a later increment). An inline mention of a `shape = "name"` prefix that
//! does not resolve retries with its last `-segment` dropped, while the
//! prefix and one segment remain: the first resolving ID wins, its section
//! checked there. Width is never checked on recognition.
//!
//! The store keeps a link's `dst` as written; [`Resolver`] is the one
//! resolution the check, and later `spec refs` and `get_impact`, share.

use std::collections::{BTreeMap, BTreeSet};

use specengine_model::grammar;
use specengine_model::{
    CanonTarget, Fields, IdScheme, IdScript, Node, ParentRef, Reference, Shape,
};

use super::input::{CheckFile, CheckInput};
use super::text::FileText;

/// The resolution index of one corpus: every file in path order.
#[derive(Debug, Clone)]
pub struct Resolver<'a> {
    scheme: &'a IdScheme,
    paths: Vec<&'a str>,
    /// Latin ID → the files defining it (document or section), path order.
    pub(crate) defined: BTreeMap<String, Vec<usize>>,
    /// Legacy ID as written in `aliases:` → the files declaring it.
    aliases: BTreeMap<String, Vec<usize>>,
    /// Per file: its section IDs.
    pub(crate) sections: Vec<BTreeSet<String>>,
}

/// How a reference fared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// The files holding the ID (with the section, when one is written), as
    /// indexes into [`Resolver::paths`], in path order; never empty.
    Resolved(Vec<usize>),
    /// `project:` or `slug/`: resolved in a later increment.
    Skipped,
    /// Why the reference resolves to nothing.
    Dangling(String),
}

impl<'a> Resolver<'a> {
    /// The index of `input`'s files, whatever their order.
    pub fn new(input: &'a CheckInput, scheme: &'a IdScheme) -> Self {
        let mut files: Vec<&CheckFile> = input.files.iter().collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Self::of_sorted(&files, scheme)
    }

    /// The index of `files`, already in path order.
    pub(crate) fn of_sorted(files: &[&'a CheckFile], scheme: &'a IdScheme) -> Self {
        let mut resolver = Resolver {
            scheme,
            paths: files.iter().map(|file| file.path.as_str()).collect(),
            defined: BTreeMap::new(),
            aliases: BTreeMap::new(),
            sections: Vec::with_capacity(files.len()),
        };
        for (index, file) in files.iter().enumerate() {
            let mut sections = BTreeSet::new();
            if let Some(parsed) = &file.parsed {
                if let Some(document) = parsed.document() {
                    if let Some(id) = &document.id {
                        push_unique(resolver.defined.entry(id.clone()).or_default(), index);
                    }
                    if let Some(aliases) = document.fields.as_ref().and_then(|f| f.aliases.as_ref())
                    {
                        for alias in aliases {
                            push_unique(resolver.aliases.entry(alias.clone()).or_default(), index);
                        }
                    }
                }
                for section in parsed.sections() {
                    if let Some(id) = &section.id {
                        sections.insert(id.clone());
                        push_unique(resolver.defined.entry(id.clone()).or_default(), index);
                    }
                }
            }
            resolver.sections.push(sections);
        }
        resolver
    }

    /// The files, in path order: what [`Resolution::Resolved`] indexes.
    pub fn paths(&self) -> &[&'a str] {
        &self.paths
    }

    /// A declared (front-matter) reference: `written` is its text as
    /// written (qualifiers, section and revision included).
    pub fn resolve(&self, reference: &Reference, written: &str) -> Resolution {
        if skipped(reference) {
            return Resolution::Skipped;
        }
        match self.holders(&reference.id, reference.alias_of.as_deref(), bare(written)) {
            Some(holders) => self.in_section(holders, reference.section.as_deref()),
            None => dangling(),
        }
    }

    /// An inline mention: [`Resolver::resolve`], then for a `shape = "name"`
    /// prefix the ID with its last `-segment` dropped, while the prefix and
    /// one segment remain; the first resolving ID wins, its section checked
    /// there.
    pub fn resolve_mention(&self, reference: &Reference, written: &str) -> Resolution {
        if skipped(reference) {
            return Resolution::Skipped;
        }
        let alias_of = reference.alias_of.as_deref();
        if let Some(holders) = self.holders(&reference.id, alias_of, bare(written)) {
            return self.in_section(holders, reference.section.as_deref());
        }
        if self.name_shaped(reference)
            && let Some((prefix, mut body)) = reference.id.split_once('-')
        {
            while let Some((shorter, _)) = body.rsplit_once('-') {
                body = shorter;
                let candidate = format!("{prefix}-{body}");
                if let Some(holders) = self.holders(&candidate, alias_of, &candidate) {
                    return self.in_section(holders, reference.section.as_deref());
                }
            }
        }
        dangling()
    }

    /// The files holding the reference's ID, its section ignored, without
    /// the name fallback; `None` when it resolves to nothing or is skipped.
    pub fn holders_of(&self, reference: &Reference, written: &str) -> Option<&[usize]> {
        if skipped(reference) {
            return None;
        }
        self.holders(&reference.id, reference.alias_of.as_deref(), bare(written))
            .map(Vec::as_slice)
    }

    fn holders(&self, id: &str, alias_of: Option<&str>, bare: &str) -> Option<&Vec<usize>> {
        let mut holders = None;
        if alias_of.is_none() {
            holders = self.defined.get(id);
        }
        if holders.is_none() {
            holders = self.aliases.get(id).or_else(|| self.aliases.get(bare));
        }
        if holders.is_none()
            && let Some(prefix) = alias_of
            && let Some((_, body)) = id.split_once('-')
        {
            holders = self.defined.get(&format!("{prefix}-{body}"));
        }
        holders
    }

    fn in_section(&self, holders: &[usize], section: Option<&str>) -> Resolution {
        let Some(section) = section else {
            return Resolution::Resolved(holders.to_vec());
        };
        let files: Vec<usize> = holders
            .iter()
            .copied()
            .filter(|&index| self.sections[index].contains(section))
            .collect();
        if files.is_empty() {
            Resolution::Dangling(format!("has no section `#{section}` in its file"))
        } else {
            Resolution::Resolved(files)
        }
    }

    /// The reference's prefix (its `aliases_from` target for a legacy one)
    /// has `shape = "name"`.
    fn name_shaped(&self, reference: &Reference) -> bool {
        let prefix = match &reference.alias_of {
            Some(prefix) => Some(prefix.as_str()),
            None => reference.id.split_once('-').map(|(prefix, _)| prefix),
        };
        prefix
            .and_then(|prefix| self.scheme.prefix(prefix))
            .is_some_and(|spec| spec.shape == Shape::Name)
    }
}

fn skipped(reference: &Reference) -> bool {
    reference.project.is_some() || reference.scope.is_some()
}

fn dangling() -> Resolution {
    Resolution::Dangling("resolves to no ID and no alias".to_owned())
}

/// The written text before its section or revision.
fn bare(written: &str) -> &str {
    written.split(['#', '@']).next().unwrap_or(written)
}

fn push_unique(holders: &mut Vec<usize>, index: usize) {
    if holders.last() != Some(&index) {
        holders.push(index);
    }
}

/// A front-matter reference and the key it is written under.
pub(crate) struct DeclaredRef {
    /// `supersedes`, `status`, `adrs`, `refs`, `working_answer`, `parent`,
    /// `links` or `canon`.
    pub key: &'static str,
    /// The link type of a `links:` item.
    pub link_type: Option<String>,
    pub reference: Reference,
}

/// Every reference a document declares: `supersedes`, `adrs`, `refs`, the
/// target of `status: superseded-by`, `working_answer`, `parent` (an alias
/// re-read through the scheme), `links.*`, a reference-form `canon:`.
pub(crate) fn declared_references(document: &Node, scheme: &IdScheme) -> Vec<DeclaredRef> {
    let mut references = Vec::new();
    let mut push = |key: &'static str, link_type: Option<&str>, reference: Reference| {
        references.push(DeclaredRef {
            key,
            link_type: link_type.map(str::to_owned),
            reference,
        });
    };
    let none = Fields::default();
    let fields = document.fields.as_ref().unwrap_or(&none);
    for (key, list) in [
        ("supersedes", &fields.supersedes),
        ("adrs", &fields.adrs),
        ("refs", &fields.refs),
    ] {
        for reference in list.iter().flatten() {
            push(key, None, reference.clone());
        }
    }
    if let Some(status) = &fields.status
        && let Some((_, target)) = grammar::split_superseded_by(status)
        && let Some(found) = grammar::parse_reference(target, 0, scheme)
    {
        let mut reference = found.reference;
        reference.span = None;
        push("status", None, reference);
    }
    if let Some(reference) = &fields.working_answer {
        push("working_answer", None, reference.clone());
    }
    if let Some(parent) = &document.parent {
        push("parent", None, parent_reference(parent, scheme));
    }
    if let Some(links) = &fields.links {
        for (link_type, list) in links.iter() {
            for reference in list {
                push("links", Some(link_type), reference.clone());
            }
        }
    }
    if let Some(CanonTarget::Reference(reference)) = &fields.canon {
        push("canon", None, reference.clone());
    }
    references
}

/// `ParentRef` keeps no `alias_of`: the ID is read again.
fn parent_reference(parent: &ParentRef, scheme: &IdScheme) -> Reference {
    let reference = grammar::parse_reference(&parent.id, 0, scheme).map_or_else(
        || Reference {
            id: parent.id.clone(),
            alias_of: None,
            script: IdScript::of(&parent.id),
            project: None,
            scope: None,
            section: None,
            rev: None,
            form: Default::default(),
            label: None,
            span: None,
        },
        |found| found.reference,
    );
    Reference {
        span: parent.span,
        ..reference
    }
}

/// The reference as written: the text under its span, else rebuilt.
pub(crate) fn written(text: &FileText<'_>, reference: &Reference) -> String {
    if let Some(span) = reference.span
        && !text.is_empty()
    {
        let written = text.text(span);
        if !written.is_empty() {
            return written;
        }
    }
    match &reference.section {
        Some(section) => format!("{}#{section}", reference.id),
        None => reference.id.clone(),
    }
}

/// The line of a reference: its span's, else the line of `key` (1 when not
/// written or without bytes).
pub(crate) fn reference_line(text: &FileText<'_>, reference: &Reference, key: &str) -> usize {
    reference.span.filter(|_| !text.is_empty()).map_or_else(
        || text.key_line(key).unwrap_or(1),
        |span| text.line(span.start),
    )
}
