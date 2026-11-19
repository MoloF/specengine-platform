//! Every document as a library, the pure half (task spec `doc-library`,
//! "`spec library`"): one row per file holding a document, with its
//! front-matter facets, its uses, the `[[module]]` tables holding it and
//! the decisions whose `canon:` lands on it, grouped by their status.
//! Computed per read, never stored. No clock, I/O, queue or git: the CLI
//! joins each file's last change and judges `newer` and `behind` through
//! [`newer`] and [`behind`].
//!
//! - **Rows**: each file of the graph with a document node, live or
//!   generated, Tier 3 too under [`LibraryRequest::archive`]; by path, in
//!   byte order.
//! - **Names**: [`CanonicalNames`], the map's.
//! - **Parent**: the canonical name of the node the document is listed
//!   under ([`SpecGraph::listed_under`], the first step of its
//!   `ancestors`); none when it has no `parent:`, a dangling one, another
//!   project's or a cycle broken at it.
//! - **Modules**: a direction holds a row whose `kind` it lists; a rooted
//!   module holds a row whose document or one of its `ancestors` is the
//!   module's root, resolved as `spec show` resolves a REF, its first
//!   holder by (path, position). A root naming nothing is a note, and so
//!   is its `@rev`, ignored as `spec show` ignores it. Each row's
//!   `ancestors` chain is walked once, whatever the modules.
//! - **Decisions**: a resolved `canon` link of front-matter origin whose
//!   source document has `class: decision` (live or Tier 3, whatever the
//!   reader admits) lists that decision under the row of the document it
//!   lands on, or of the document holding the ID section it lands on (its
//!   first admitted holder by (path, position), as the map's edges land).
//!   Each decision once, by its first such link; grouped by
//!   [`decision_group`], then by `date:` descending as text (none last),
//!   then by name.
//! - **Tags** ([`kept_tags`]): the kept entries of `tags:`; `spec check`
//!   gives a `tag-invalid` warning for each entry left out.
//!
//! The core names no class beyond the convention's four and reads only the
//! decision status words [`decision_group`] names (ADR-0008): every kind,
//! tag, module and status is data, compared only as written.

use std::collections::{BTreeMap, BTreeSet};

use specengine_model::{Fields, IdScript, LinkOrigin, grammar};

use crate::check::{CANON_LINK, Endpoint, LinkState, NodeAt, SpecGraph, Standing};
use crate::map::{CanonicalNames, MapNaming};
use crate::revision::revision;
use crate::usage::usage_of;
use crate::walk_scope::{DOCUMENT_EXTENSION, is_clean_relative};
use crate::workspace_toml::WorkspaceConfig;

/// The longest tag, in UTF-8 bytes.
pub const TAG_MAX_BYTES: usize = 64;

/// The most tags a document keeps; a passing entry past them is reported.
pub const TAGS_KEPT_MAX: usize = 32;

/// Why a module root's `@rev` is ignored, as `classify`'s note gives it.
const IGNORED_REV: &str = "`spec library` reads the current files";

/// The class whose `canon:` lists a document under the row it lands on:
/// the convention's word (`documentation-system.md` §2).
const DECISION_CLASS: &str = "decision";

/// One `tags:` entry left out, its 1-based position and why, as `spec
/// check`'s `tag-invalid` words it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagProblem {
    /// 1-based, in written order.
    pub entry: usize,
    pub message: String,
}

/// `tags:` judged: the entries kept, in written order, and each one left
/// out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tags {
    pub kept: Vec<String>,
    pub problems: Vec<TagProblem>,
}

/// Judges `written`, the string entries of `tags:` in written order (task
/// spec `doc-library`, "Front-matter `tags`"). An entry fails, the first of
/// these rules it breaks naming it: it is empty or has whitespace at an end;
/// it is over [`TAG_MAX_BYTES`] bytes; it holds a control character; it is
/// byte-equal to an earlier entry. A passing entry is kept while fewer than
/// [`TAGS_KEPT_MAX`] are kept; each one after them is left out too. No case
/// folding, no normalisation: a tag is compared byte for byte. `spec
/// check`'s `tag-invalid` words each problem; [`kept_tags`] is the same
/// judgement with no words, the library's.
pub fn judge_tags(written: &[String]) -> Tags {
    let mut tags = Tags::default();
    judge_each(written, |entry, tag, verdict| match verdict {
        Verdict::Kept => tags.kept.push(tag.to_owned()),
        failed => tags.problems.push(TagProblem {
            entry,
            message: failed.message(entry),
        }),
    });
    tags
}

/// [`judge_tags`]' kept entries alone, no problem worded: what a library
/// row serves.
pub fn kept_tags(written: &[String]) -> Vec<String> {
    let mut kept = Vec::new();
    judge_each(written, |_, tag, verdict| {
        if verdict == Verdict::Kept {
            kept.push(tag.to_owned());
        }
    });
    kept
}

/// One entry's verdict, by the first rule it breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Kept,
    /// Empty, or whitespace at an end.
    Blank,
    /// Its length in bytes, over [`TAG_MAX_BYTES`].
    Long(usize),
    Control,
    /// The 1-based entry it repeats, the first byte-equal one.
    Repeats(usize),
    /// Past [`TAGS_KEPT_MAX`] kept entries.
    Past,
}

impl Verdict {
    /// `spec check`'s words for a failed entry.
    fn message(self, entry: usize) -> String {
        match self {
            Self::Kept => String::new(),
            Self::Blank => format!("tags: entry {entry} is empty or has a space at an end"),
            Self::Long(bytes) => {
                format!("tags: entry {entry} is {bytes} bytes; a tag has at most {TAG_MAX_BYTES}")
            }
            Self::Control => format!("tags: entry {entry} holds a control character"),
            Self::Repeats(earlier) => format!("tags: entry {entry} repeats entry {earlier}"),
            Self::Past => format!(
                "tags: entry {entry} is past the {TAGS_KEPT_MAX}nd kept tag; a document keeps \
                 {TAGS_KEPT_MAX}"
            ),
        }
    }
}

/// Calls `each` with every entry's 1-based position, text and verdict, in
/// written order.
fn judge_each<'w>(written: &'w [String], mut each: impl FnMut(usize, &'w str, Verdict)) {
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    let mut kept = 0;
    for (index, tag) in written.iter().enumerate() {
        let entry = index + 1;
        let first = *seen.entry(tag.as_str()).or_insert(entry);
        let ends_blank = tag.chars().next().is_some_and(char::is_whitespace)
            || tag.chars().next_back().is_some_and(char::is_whitespace);
        let verdict = if tag.is_empty() || ends_blank {
            Verdict::Blank
        } else if tag.len() > TAG_MAX_BYTES {
            Verdict::Long(tag.len())
        } else if tag.chars().any(char::is_control) {
            Verdict::Control
        } else if first != entry {
            Verdict::Repeats(first)
        } else if kept >= TAGS_KEPT_MAX {
            Verdict::Past
        } else {
            kept += 1;
            Verdict::Kept
        };
        each(entry, tag, verdict);
    }
}

/// A decision's group by its `status:`, in the order a row lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DecisionGroup {
    /// `accepted`, exactly.
    Accepted,
    /// `superseded-by ` and a target.
    Superseded,
    /// `rejected`, exactly.
    Rejected,
    /// Anything else, an absent status included.
    Other,
}

impl DecisionGroup {
    /// As the JSON and the text give it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Superseded => "superseded",
            Self::Rejected => "rejected",
            Self::Other => "other",
        }
    }
}

/// The group of a decision whose `status:` is `status` (the convention's
/// words, `documentation-system.md` §8): exactly `accepted`, a status
/// starting `superseded-by `, exactly `rejected`; anything else, none
/// included, is [`DecisionGroup::Other`].
pub fn decision_group(status: Option<&str>) -> DecisionGroup {
    match status {
        Some("accepted") => DecisionGroup::Accepted,
        Some("rejected") => DecisionGroup::Rejected,
        Some(status) if status.starts_with("superseded-by ") => DecisionGroup::Superseded,
        _ => DecisionGroup::Other,
    }
}

/// Whether a decision's last change is newer than its document's: `at`
/// are committer times as `YYYY-MM-DDTHH:MM:SSZ` (one fixed width, so text
/// order is time order). `None` unless both are read; one commit holding
/// both is not newer.
pub fn newer(decision_at: Option<&str>, document_at: Option<&str>) -> Option<bool> {
    Some(decision_at? > document_at?)
}

/// A row's mark from its decisions' groups and `newer`: `true` when an
/// accepted one is newer; else `None` when an accepted one is not known to
/// be or not; else `false`, a row with no accepted decision included. A
/// hint, never a finding (ADR-0012).
pub fn behind(decisions: impl IntoIterator<Item = (DecisionGroup, Option<bool>)>) -> Option<bool> {
    let mut unknown = false;
    for (group, newer) in decisions {
        if group != DecisionGroup::Accepted {
            continue;
        }
        match newer {
            Some(true) => return Some(true),
            None => unknown = true,
            Some(false) => {}
        }
    }
    if unknown { None } else { Some(false) }
}

/// What the library reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LibraryRequest {
    /// Tier 3 documents too.
    pub archive: bool,
}

/// One decision listed under a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryDecision {
    /// The decision's document.
    pub at: NodeAt,
    pub name: String,
    pub id: Option<String>,
    pub title: Option<String>,
    pub path: String,
    pub status: Option<String>,
    pub group: DecisionGroup,
    /// `date:` as written.
    pub date: Option<String>,
    /// The ID section the link lands on, named; `None` on the document.
    pub section: Option<String>,
}

/// One document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRow {
    /// Its document node (ord 0).
    pub at: NodeAt,
    pub name: String,
    pub id: Option<String>,
    pub path: String,
    pub title: Option<String>,
    pub class: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
    /// Its file is Tier 3.
    pub archived: bool,
    pub tier: Option<i64>,
    pub owner: Option<String>,
    /// As written, a date or not.
    pub reviewed: Option<String>,
    pub scope: Option<Vec<String>>,
    /// [`judge_tags`]' kept entries; none when `tags:` is absent or
    /// mistyped.
    pub tags: Vec<String>,
    /// Its revision's number: 1 when none is written, `None` unreadable.
    pub rev: Option<u32>,
    pub parent: Option<String>,
    /// The `[[module]]` names holding it, in config order.
    pub modules: Vec<String>,
    pub used_by: u32,
    pub mentioned_by: u32,
    pub orphan: Option<bool>,
    /// In group, date and name order.
    pub decisions: Vec<LibraryDecision>,
}

/// The library of one graph and config.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Library {
    /// By path, in byte order.
    pub rows: Vec<LibraryRow>,
    /// Sorted, each once: a module root naming nothing.
    pub notes: Vec<String>,
}

/// The library of `graph` (task spec `doc-library`, "`spec library`"): its
/// rows, names by `naming`, modules from `workspace`'s `[[module]]`
/// tables (see the module documentation).
pub fn library(
    graph: &SpecGraph<'_>,
    naming: &MapNaming<'_>,
    workspace: &WorkspaceConfig,
    request: LibraryRequest,
) -> Library {
    let admitted = |file: usize| match graph.standing(file) {
        Standing::Live | Standing::Generated => true,
        Standing::Tier3 => request.archive,
    };
    let names = CanonicalNames::new(graph, *naming);
    let usage = usage_of(graph);
    let mut notes: BTreeSet<String> = BTreeSet::new();

    // A rooted module's root, resolved once; a direction's kinds.
    let modules: Vec<(&str, Reach<'_>)> = workspace
        .modules
        .iter()
        .filter_map(|module| match (&module.root, &module.kinds) {
            (Some(root), _) => {
                let at = root_of(graph, &module.name, root, &mut notes);
                if at.is_none() {
                    notes.insert(format!(
                        "library: module `{}`: its root `{root}` names nothing; no row is in it",
                        module.name
                    ));
                }
                Some((module.name.as_str(), Reach::Root(at)))
            }
            (None, Some(kinds)) => Some((module.name.as_str(), Reach::Kinds(kinds))),
            (None, None) => None,
        })
        .collect();

    let none = Fields::default();
    let mut rows: Vec<LibraryRow> = Vec::new();
    for document in graph.documents().filter(|at| admitted(at.file)) {
        let Some(node) = graph.node(document) else {
            continue;
        };
        let fields = node.fields.as_ref().unwrap_or(&none);
        let ancestors = graph.ancestors(document);
        let held = |root: NodeAt| root == document || ancestors.contains(&root);
        let modules = modules
            .iter()
            .filter(|(_, reach)| match reach {
                Reach::Root(root) => root.is_some_and(held),
                Reach::Kinds(kinds) => node.kind.as_ref().is_some_and(|kind| kinds.contains(kind)),
            })
            .map(|(name, _)| (*name).to_owned())
            .collect();
        let uses = usage.get(&document).copied().unwrap_or_default();
        rows.push(LibraryRow {
            at: document,
            name: names.name(document),
            id: node.id.clone(),
            path: graph.paths()[document.file].to_owned(),
            title: node.title.clone(),
            class: fields.class.clone(),
            kind: node.kind.clone(),
            status: fields.status.clone(),
            archived: graph.is_tier3(document.file),
            tier: fields.tier,
            owner: fields.owner.clone(),
            reviewed: fields.reviewed.clone(),
            scope: fields.scope.clone(),
            tags: fields.tags.as_deref().map(kept_tags).unwrap_or_default(),
            rev: graph
                .file(document.file)
                .and_then(|file| file.parsed.as_ref())
                .and_then(|parsed| revision(parsed, 0).number()),
            parent: graph
                .listed_under(document)
                .map(|parent| names.name(parent)),
            modules,
            used_by: uses.used_by,
            mentioned_by: uses.mentioned_by,
            orphan: uses.orphan,
            decisions: Vec::new(),
        });
    }
    rows.sort_by(|a, b| a.path.cmp(&b.path));

    let by_file: BTreeMap<usize, usize> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| (row.at.file, index))
        .collect();
    let mut placed: BTreeSet<usize> = BTreeSet::new();
    for edge in graph.edges() {
        if edge.link_type != CANON_LINK
            || edge.origin != LinkOrigin::Frontmatter
            || edge.state() != LinkState::Resolved
            || placed.contains(&edge.file)
        {
            continue;
        }
        let Endpoint::Nodes(targets) = &edge.target else {
            continue;
        };
        let Some(source) = graph.document(edge.file) else {
            continue;
        };
        let Some(decision) = graph.node(source) else {
            continue;
        };
        let fields = decision.fields.as_ref().unwrap_or(&none);
        if fields.class.as_deref() != Some(DECISION_CLASS) {
            continue;
        }
        // The first holder a row admits, by (path, position).
        let Some((&row, target)) = targets
            .iter()
            .filter_map(|at| by_file.get(&at.file).map(|row| (row, *at)))
            .min_by_key(|(_, at)| *at)
        else {
            continue;
        };
        placed.insert(edge.file);
        let status = fields.status.clone();
        rows[row].decisions.push(LibraryDecision {
            at: source,
            name: names.name(source),
            id: decision.id.clone(),
            title: decision.title.clone(),
            path: graph.paths()[source.file].to_owned(),
            group: decision_group(status.as_deref()),
            status,
            date: fields.date.clone(),
            section: (target.ord > 0).then(|| names.name(target)),
        });
    }
    for row in &mut rows {
        row.decisions.sort_by(|a, b| {
            a.group
                .cmp(&b.group)
                .then_with(|| match (&a.date, &b.date) {
                    (Some(a), Some(b)) => b.cmp(a),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                })
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.path.cmp(&b.path))
        });
    }

    Library {
        rows,
        notes: notes.into_iter().collect(),
    }
}

/// How a module reaches its rows.
enum Reach<'w> {
    /// Its root's first holder; `None` when it names nothing.
    Root(Option<NodeAt>),
    /// A direction's kinds.
    Kinds(&'w [String]),
}

/// The first holder by (path, position) of `written`, resolved as `spec
/// show` resolves a REF (its `classify`, then its `locate`): a clean
/// root-relative `.md` path names its document; any other form is one
/// reference of the graph's scheme, never another project's, with no
/// look-alike; its `@rev`, as there, is a note in `notes` naming `module`
/// and the root as written, and resolves as the reference without it.
/// `None`: it names nothing.
fn root_of(
    graph: &SpecGraph<'_>,
    module: &str,
    written: &str,
    notes: &mut BTreeSet<String>,
) -> Option<NodeAt> {
    if written.ends_with(DOCUMENT_EXTENSION) {
        if !is_clean_relative(written) {
            return None;
        }
        return graph.file_of(written).and_then(|file| graph.document(file));
    }
    let found = grammar::parse_reference(written, 0, graph.scheme())?;
    let reference = found.reference;
    if !found.homoglyphs.is_empty()
        || reference.script == IdScript::Mixed
        || reference.project.is_some()
    {
        return None;
    }
    if let Some(rev) = reference.rev {
        notes.insert(format!(
            "library: module `{module}`: its root `{written}`: `@{rev}` is ignored: {IGNORED_REV}"
        ));
    }
    match graph.locate(&reference, written) {
        Endpoint::Nodes(nodes) => nodes.into_iter().min(),
        _ => None,
    }
}
