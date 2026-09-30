//! Reference resolution over one corpus (docs/canon/spec-check.md,
//! "Rules", References; ADR-0026 for scopes): a reference resolves when its
//! ID is defined (a document `id:` or a `{#ID}` section), or its text is in
//! a document's `aliases:`, or, through `aliases_from`, the configured
//! prefix + the written body is defined (no re-padding); a `#Y` section must
//! be defined in a file holding the ID. Where it may be defined depends on
//! its scope: `slug/ID` only in the feature document `<features>/<slug>.md`
//! (any prefix); a bare ID of a `scope = "feature"` prefix only in the
//! citing file, when that file is a feature document; any other bare ID
//! anywhere. A feature document is a walked `.md` file directly under
//! `[paths] features` whose stem is a slug ([`grammar::is_slug`]).
//! `project:` references are skipped (a later increment). An inline mention
//! of a `shape = "name"` prefix that does not resolve retries with its last
//! `-segment` dropped, while the prefix and one segment remain, in the same
//! place: the first resolving ID wins, its section checked there. Width is
//! never checked on recognition.
//!
//! The store keeps a link's `dst` as written; [`Resolver`] is the one
//! resolution the check, `spec show` and later `spec refs` and
//! `get_impact` share. With no citing file ([`Resolver::resolve_detached`],
//! `spec show`) a bare feature-scoped ID resolves wherever it is defined.

use std::collections::{BTreeMap, BTreeSet};

use specengine_model::grammar;
use specengine_model::{
    CanonTarget, Fields, IdScheme, IdScope, IdScript, Node, ParentRef, Reference, Shape,
};

use super::input::{CheckFile, CheckInput};
use super::text::FileText;
use crate::{DOCUMENT_EXTENSION, Paths};

/// The resolution index of one corpus: every file in path order.
#[derive(Debug, Clone)]
pub struct Resolver<'a> {
    scheme: &'a IdScheme,
    /// `[paths] features`: the directory of the feature documents.
    features: &'a str,
    paths: Vec<&'a str>,
    /// Path → its file (the first, should a path repeat).
    by_path: BTreeMap<&'a str, usize>,
    /// Per file: its slug when it is a feature document.
    slugs: Vec<Option<&'a str>>,
    /// Slug → its feature document.
    by_slug: BTreeMap<&'a str, usize>,
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
    /// `project:`: resolved in a later increment.
    Skipped,
    /// Why the reference resolves to nothing.
    Dangling(String),
}

/// Where a reference may resolve.
#[derive(Debug, Clone, Copy)]
enum Place<'r> {
    /// A bare ID of a project-scoped prefix: any file.
    Anywhere,
    /// `slug/ID`: the feature document of the slug, when there is one.
    Feature(&'r str, Option<usize>),
    /// A bare ID of a feature-scoped prefix: the citing file, when it is a
    /// feature document.
    Own(Option<usize>),
}

impl<'a> Resolver<'a> {
    /// The index of `input`'s files, whatever their order; `paths` gives
    /// the feature documents (`[paths] features`).
    pub fn new(input: &'a CheckInput, scheme: &'a IdScheme, paths: &'a Paths) -> Self {
        let mut files: Vec<&CheckFile> = input.files.iter().collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Self::of_sorted(&files, scheme, &paths.features)
    }

    /// The index of `files`, already in path order; `features` is
    /// `[paths] features`.
    pub(crate) fn of_sorted(
        files: &[&'a CheckFile],
        scheme: &'a IdScheme,
        features: &'a str,
    ) -> Self {
        let mut resolver = Resolver {
            scheme,
            features,
            paths: files.iter().map(|file| file.path.as_str()).collect(),
            by_path: BTreeMap::new(),
            slugs: Vec::with_capacity(files.len()),
            by_slug: BTreeMap::new(),
            defined: BTreeMap::new(),
            aliases: BTreeMap::new(),
            sections: Vec::with_capacity(files.len()),
        };
        for (index, file) in files.iter().enumerate() {
            let path = file.path.as_str();
            resolver.by_path.entry(path).or_insert(index);
            let slug = feature_stem(features, path);
            if let Some(slug) = slug {
                resolver.by_slug.entry(slug).or_insert(index);
            }
            resolver.slugs.push(slug);
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

    /// The slug of `path` when it is a feature document: a walked `.md`
    /// file directly under `[paths] features` whose stem is a slug. `""` or
    /// an unwalked path: `None`.
    pub fn feature_slug(&self, path: &str) -> Option<&'a str> {
        self.by_path.get(path).and_then(|&index| self.slugs[index])
    }

    /// The file at `index` is a feature document.
    pub(crate) fn is_feature(&self, index: usize) -> bool {
        self.slugs.get(index).is_some_and(Option::is_some)
    }

    /// A defined ID (a configured prefix; an `aliases_from` entry read as
    /// its target) belongs to a `scope = "feature"` prefix.
    pub(crate) fn feature_scoped_id(&self, id: &str) -> bool {
        id.split_once('-')
            .is_some_and(|(prefix, _)| self.feature_scoped_prefix(prefix))
    }

    /// A declared (front-matter) reference cited from the file `from`:
    /// `written` is its text as written (qualifiers, section and revision
    /// included).
    pub fn resolve(&self, from: &str, reference: &Reference, written: &str) -> Resolution {
        self.resolve_in(from, reference, written, false)
    }

    /// An inline mention cited from the file `from`: [`Resolver::resolve`],
    /// then for a `shape = "name"` prefix the ID with its last `-segment`
    /// dropped, while the prefix and one segment remain, in the same place;
    /// the first resolving ID wins, its section checked there.
    pub fn resolve_mention(&self, from: &str, reference: &Reference, written: &str) -> Resolution {
        self.resolve_in(from, reference, written, true)
    }

    /// A reference with no citing file (`spec show`, a reader asking by
    /// ID): [`Resolver::resolve`], except that a bare ID of a
    /// `scope = "feature"` prefix resolves wherever it is defined, as a
    /// project-scoped one does (the check resolves it only inside its own
    /// feature document). `slug/ID` resolves in its feature document,
    /// `project:` is skipped, no name fallback.
    pub fn resolve_detached(&self, reference: &Reference, written: &str) -> Resolution {
        let place = if reference.project.is_some() {
            None
        } else if let Some(slug) = reference.scope.as_deref() {
            Some(Place::Feature(slug, self.by_slug.get(slug).copied()))
        } else {
            Some(Place::Anywhere)
        };
        self.resolve_at(place, reference, written, false)
    }

    /// The files holding the reference's ID as cited from the file `from`,
    /// its section ignored, without the name fallback; `None` when it
    /// resolves to nothing or is skipped.
    pub fn holders_of(
        &self,
        from: &str,
        reference: &Reference,
        written: &str,
    ) -> Option<Vec<usize>> {
        let place = self.place(from, reference)?;
        let bare = bare(reference, written);
        self.holders_in(place, &reference.id, reference.alias_of.as_deref(), bare)
    }

    fn resolve_in(
        &self,
        from: &str,
        reference: &Reference,
        written: &str,
        fallback: bool,
    ) -> Resolution {
        self.resolve_at(self.place(from, reference), reference, written, fallback)
    }

    /// Resolution in `place` (`None`: `project:`, skipped).
    fn resolve_at(
        &self,
        place: Option<Place<'_>>,
        reference: &Reference,
        written: &str,
        fallback: bool,
    ) -> Resolution {
        let Some(place) = place else {
            return Resolution::Skipped;
        };
        let alias_of = reference.alias_of.as_deref();
        let section = reference.section.as_deref();
        let bare = bare(reference, written);
        if let Some(holders) = self.holders_in(place, &reference.id, alias_of, bare) {
            return self.in_section(place, holders, section);
        }
        if fallback
            && self.name_shaped(reference)
            && let Some((prefix, mut body)) = reference.id.split_once('-')
        {
            while let Some((shorter, _)) = body.rsplit_once('-') {
                body = shorter;
                let candidate = format!("{prefix}-{body}");
                if let Some(holders) = self.holders_in(place, &candidate, alias_of, &candidate) {
                    return self.in_section(place, holders, section);
                }
            }
        }
        Resolution::Dangling(self.unresolved(place, reference, bare))
    }

    /// Where the reference may resolve; `None` for `project:` (skipped).
    fn place<'r>(&self, from: &str, reference: &'r Reference) -> Option<Place<'r>> {
        if reference.project.is_some() {
            return None;
        }
        if let Some(slug) = reference.scope.as_deref() {
            return Some(Place::Feature(slug, self.by_slug.get(slug).copied()));
        }
        if self.feature_scoped(reference) {
            let own = self
                .by_path
                .get(from)
                .copied()
                .filter(|&index| self.is_feature(index));
            return Some(Place::Own(own));
        }
        Some(Place::Anywhere)
    }

    /// The files defining `id` in `place`: anywhere, the first of the
    /// defined ID, the `aliases:` entry and the `aliases_from` target that
    /// has holders; in one file, that file when it holds any of them. Each
    /// list is looked up only when the ones before it did not decide.
    fn holders_in(
        &self,
        place: Place<'_>,
        id: &str,
        alias_of: Option<&str>,
        bare: &str,
    ) -> Option<Vec<usize>> {
        let file = match place {
            Place::Anywhere => {
                return self
                    .defined_holders(id, alias_of)
                    .or_else(|| self.alias_holders(id, bare))
                    .or_else(|| self.target_holders(id, alias_of))
                    .cloned();
            }
            Place::Feature(_, file) | Place::Own(file) => file?,
        };
        let holds = |holders: Option<&Vec<usize>>| {
            holders.is_some_and(|holders| holders.binary_search(&file).is_ok())
        };
        let held = holds(self.defined_holders(id, alias_of))
            || holds(self.alias_holders(id, bare))
            || holds(self.target_holders(id, alias_of));
        held.then(|| vec![file])
    }

    /// The files defining `id` itself; never for an alias.
    fn defined_holders(&self, id: &str, alias_of: Option<&str>) -> Option<&Vec<usize>> {
        match alias_of {
            None => self.defined.get(id),
            Some(_) => None,
        }
    }

    /// The files whose `aliases:` hold the ID, else the bare written text.
    fn alias_holders(&self, id: &str, bare: &str) -> Option<&Vec<usize>> {
        self.aliases.get(id).or_else(|| self.aliases.get(bare))
    }

    /// The files defining the `aliases_from` target: the configured prefix
    /// + the body as written.
    fn target_holders(&self, id: &str, alias_of: Option<&str>) -> Option<&Vec<usize>> {
        let prefix = alias_of?;
        let (_, body) = id.split_once('-')?;
        self.defined.get(&format!("{prefix}-{body}"))
    }

    fn in_section(
        &self,
        place: Place<'_>,
        mut holders: Vec<usize>,
        section: Option<&str>,
    ) -> Resolution {
        let Some(section) = section else {
            return Resolution::Resolved(holders);
        };
        holders.retain(|&index| self.sections[index].contains(section));
        if !holders.is_empty() {
            return Resolution::Resolved(holders);
        }
        Resolution::Dangling(match place {
            Place::Feature(_, Some(file)) => {
                format!("has no section `#{section}` in `{}`", self.paths[file])
            }
            _ => format!("has no section `#{section}` in its file"),
        })
    }

    /// Why the reference, as written (not a fallback candidate), resolves
    /// to nothing in `place`.
    fn unresolved(&self, place: Place<'_>, reference: &Reference, bare: &str) -> String {
        match place {
            Place::Anywhere => "resolves to no ID and no alias".to_owned(),
            Place::Feature(slug, None) => format!(
                "resolves to no feature document `{}/{slug}{DOCUMENT_EXTENSION}`",
                self.features
            ),
            Place::Feature(_, Some(file)) => {
                format!("is not defined in `{}`", self.paths[file])
            }
            Place::Own(_) => {
                let id = &reference.id;
                let alias_of = reference.alias_of.as_deref();
                let defining: BTreeSet<usize> = [
                    self.defined_holders(id, alias_of),
                    self.alias_holders(id, bare),
                    self.target_holders(id, alias_of),
                ]
                .into_iter()
                .flatten()
                .flatten()
                .copied()
                .filter(|&index| self.is_feature(index))
                .collect();
                let cited: Vec<String> = defining
                    .iter()
                    .filter_map(|&index| self.slugs[index])
                    .map(|slug| format!("`{slug}/{id}`"))
                    .collect();
                if cited.is_empty() {
                    "is feature-scoped and no feature document defines it".to_owned()
                } else {
                    format!("is feature-scoped: cite it as {}", cited.join(" or "))
                }
            }
        }
    }

    /// The reference's prefix (its `aliases_from` target for a legacy one)
    /// has `shape = "name"`.
    fn name_shaped(&self, reference: &Reference) -> bool {
        self.reference_prefix(reference)
            .and_then(|prefix| self.scheme.prefix(prefix))
            .is_some_and(|spec| spec.shape == Shape::Name)
    }

    /// The reference's prefix (its `aliases_from` target for a legacy one)
    /// has `scope = "feature"`.
    fn feature_scoped(&self, reference: &Reference) -> bool {
        self.reference_prefix(reference)
            .is_some_and(|prefix| self.feature_scoped_prefix(prefix))
    }

    /// The configured prefix of a reference: its `alias_of`, else the text
    /// of its ID before the first `-`.
    fn reference_prefix<'r>(&self, reference: &'r Reference) -> Option<&'r str> {
        match &reference.alias_of {
            Some(prefix) => Some(prefix.as_str()),
            None => reference.id.split_once('-').map(|(prefix, _)| prefix),
        }
    }

    fn feature_scoped_prefix(&self, prefix: &str) -> bool {
        self.scheme
            .prefix(prefix)
            .or_else(|| self.scheme.alias(prefix))
            .is_some_and(|spec| spec.scope == IdScope::Feature)
    }
}

/// The slug of `path` when it is `<features>/<slug>.md`, the stem a slug.
fn feature_stem<'p>(features: &str, path: &'p str) -> Option<&'p str> {
    let stem = path
        .strip_prefix(features)?
        .strip_prefix('/')?
        .strip_suffix(DOCUMENT_EXTENSION)?;
    grammar::is_slug(stem).then_some(stem)
}

/// The written text before its section or revision, its `slug/` qualifier
/// dropped.
fn bare<'w>(reference: &Reference, written: &'w str) -> &'w str {
    let text = written.split(['#', '@']).next().unwrap_or(written);
    reference
        .scope
        .as_deref()
        .and_then(|slug| text.strip_prefix(slug)?.strip_prefix('/'))
        .unwrap_or(text)
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
/// target of `status: superseded-by`, `working_answer`, `parent` (re-read
/// through the scheme from `text`, the document's bytes), `links.*`, a
/// reference-form `canon:`.
pub(crate) fn declared_references(
    document: &Node,
    scheme: &IdScheme,
    text: &FileText<'_>,
) -> Vec<DeclaredRef> {
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
        push("parent", None, parent_reference(parent, scheme, text));
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

/// `ParentRef` keeps only the ID and the span: the reference is read again,
/// from the text under its span when the bytes give back the same ID (the
/// `project:` and `slug/` qualifiers, the section and `alias_of` kept),
/// else from the ID alone (a value not written verbatim, no bytes).
fn parent_reference(parent: &ParentRef, scheme: &IdScheme, text: &FileText<'_>) -> Reference {
    let verbatim = parent
        .span
        .filter(|_| !text.is_empty())
        .and_then(|span| grammar::parse_reference(&text.text(span), span.start, scheme))
        .map(|found| found.reference)
        .filter(|reference| reference.id == parent.id);
    if let Some(reference) = verbatim {
        return Reference {
            span: parent.span,
            ..reference
        };
    }
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

/// The reference as written: the text under its span, else rebuilt from its
/// qualifiers, ID and section.
pub(crate) fn written(text: &FileText<'_>, reference: &Reference) -> String {
    if let Some(span) = reference.span
        && !text.is_empty()
    {
        let written = text.text(span);
        if !written.is_empty() {
            return written;
        }
    }
    let mut rebuilt = String::new();
    if let Some(project) = &reference.project {
        rebuilt.push_str(project);
        rebuilt.push(':');
    }
    if let Some(slug) = &reference.scope {
        rebuilt.push_str(slug);
        rebuilt.push('/');
    }
    rebuilt.push_str(&reference.id);
    if let Some(section) = &reference.section {
        rebuilt.push('#');
        rebuilt.push_str(section);
    }
    rebuilt
}

/// The line of a reference: its span's, else the line of `key` (1 when not
/// written or without bytes).
pub(crate) fn reference_line(text: &FileText<'_>, reference: &Reference, key: &str) -> usize {
    reference.span.filter(|_| !text.is_empty()).map_or_else(
        || text.key_line(key).unwrap_or(1),
        |span| text.line(span.start),
    )
}
