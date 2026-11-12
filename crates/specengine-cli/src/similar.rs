//! A create's similar step (task spec `intake-similar`, IS1 to IS9): after
//! step 5 (validation) and before the insert, over the index this call
//! refreshed and the queue. Its list names what the create may repeat,
//! each entry with its source and reason; only the exact three (an equal
//! title, an alias equal to the title, a live create's or amendment's
//! title) gate. The create stores once `distinct_from` names each gating
//! entry and there are at most [`SIMILAR_GATING_MAX`]; otherwise nothing is
//! stored, no row, no event, no ID held, and the answer says why. Search
//! and neighbour entries advise. The engine finds candidates; the agent's
//! own model judges them.
//!
//! - **New nodes**: a new file's every parsed node, a section form's added
//!   sections, each named by its canonical ID, else (an ID-less document)
//!   by the create's path; titles normalized as the intake's dedup
//!   normalizes a summary.
//! - **Probes 1, 2, 4, 5**: the store's ([`SpecIndex::similar`]); the
//!   section form's target excluded, the new document's kind and its
//!   front-matter link targets (not `mentions`) resolved by core's
//!   resolution over the corpus with the new file in it.
//! - **Probe 3, the queue's**: each live create or amendment of the
//!   project ([`ProposalQueue::live_adds`]), by ID number, its text parsed
//!   under this root's scheme (a new file's every node, a section form's
//!   nodes among its new IDs; a text that does not parse gives none),
//!   listed after the alias entries, read outside the write lock (two
//!   creates of one title in the same instant may both store).
//!
//! The kind words come from the store's [`ProposalKind::as_str`], core's
//! constants; no kind, prefix or directory of a project is written here.

use std::collections::{BTreeMap, BTreeSet};

use specengine_core::check::resolve::feature_stem;
use specengine_core::check::{CheckFile, CheckInput, Endpoint, resolve_files};
use specengine_core::intake::normalized_summary;
use specengine_core::similar::{
    SIMILAR_GATING_MAX, SIMILAR_SHARED_MIN, SimilarEntry, SimilarSource, named_list, similar_terms,
};
use specengine_model::{IdScheme, LinkOrigin, Node, ParsedFile, is_weak_link};
use specengine_store::{
    LiveAdd, ProposalQueue, SimilarQuery, SimilarRow, SimilarTarget, SimilarTitle, SpecIndex,
};

use crate::create::{Checked, canonical_in, parsed_text};
use crate::project::ProjectRoot;
use crate::proposals::queue_cannot;
use crate::{CliError, store_error};

/// What the similar step found and whether the create stores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SimilarOutcome {
    /// The list: gating entries first (title, alias, queue; at most
    /// [`SIMILAR_GATING_MAX`]), then search, then neighbours.
    pub entries: Vec<SimilarEntry>,
    /// `None`: the create stores; else the not-stored note, the answer's
    /// first (after a dry run's).
    pub unstored: Option<String>,
}

/// The note of a create a gating entry it does not name keeps unstored.
pub(crate) fn gated_note() -> String {
    "not stored: a similar entry gates; propose an update of it, or name each gating entry in \
     `distinct_from` (--distinct-from) to store it anyway"
        .to_owned()
}

/// The note of a create more gating entries keep unstored than
/// `distinct_from` can name.
pub(crate) fn unnameable_note(gating: usize) -> String {
    format!(
        "not stored: {gating} similar entries gate and `distinct_from` names at most \
         {SIMILAR_GATING_MAX}, so this create cannot be stored (a known limit)"
    )
}

/// One new node of the create.
struct NewNode {
    /// Its canonical ID, else the create's path.
    name: String,
    /// Its parsed title, normalized; `None` when it has none.
    normalized: Option<String>,
}

/// The similar step of a create `checked` over `input` (the corpus as this
/// call indexed it, its new file absent), `index` and `queue`.
pub(crate) fn similar_step(
    index: &impl SpecIndex,
    queue: &impl ProposalQueue,
    project: &ProjectRoot,
    input: &mut CheckInput,
    checked: &Checked,
    keywords: &[String],
    distinct_from: &[String],
) -> Result<SimilarOutcome, CliError> {
    let scheme = &project.config.scheme;
    let new_nodes = new_nodes(checked, scheme);
    let titles: Vec<SimilarTitle> = new_nodes
        .iter()
        .filter_map(|node| {
            node.normalized.as_ref().map(|normalized| SimilarTitle {
                new: node.name.clone(),
                normalized: normalized.clone(),
            })
        })
        .collect();
    let terms = {
        let normalized: Vec<&str> = titles
            .iter()
            .map(|title| title.normalized.as_str())
            .collect();
        similar_terms(&normalized, keywords)
    };
    // The neighbour probe's inputs: the new document's kind, and its link
    // targets resolved only when there is a kind and enough links to share
    // (resolution indexes the whole corpus).
    let kind = checked
        .parsed
        .as_ref()
        .and_then(|parsed| parsed.document())
        .and_then(|node| node.kind.clone());
    let links = match &checked.parsed {
        Some(parsed) if kind.is_some() && own_links(parsed) >= SIMILAR_SHARED_MIN => {
            link_targets(project, input, &checked.path, parsed)
        }
        _ => Vec::new(),
    };
    let query = SimilarQuery {
        titles,
        terms,
        kind,
        links,
        exclude: excluded(input, checked),
    };
    let rows = index
        .similar(&query)
        .map_err(store_error)?
        .into_iter()
        .map(|row| canonical_row(row, project))
        .collect();
    let adds = queue.live_adds().map_err(queue_cannot)?;
    let queued = queue_entries(&adds, &query.titles, project);
    Ok(listed(rows, queued, distinct_from))
}

/// `row` with its ID canonical, as a queue target names a node: `slug/ID`
/// for a feature-scoped one in a feature document.
fn canonical_row(mut row: SimilarRow, project: &ProjectRoot) -> SimilarRow {
    let slug = feature_stem(&project.config.paths.features, &row.path);
    row.id = row
        .id
        .map(|id| canonical_in(&project.config.scheme, slug, &id));
    row
}

/// The new nodes of `checked`, in text order.
fn new_nodes(checked: &Checked, scheme: &IdScheme) -> Vec<NewNode> {
    let slug = checked.slug.as_deref();
    let of = |node: &Node| NewNode {
        name: node
            .id
            .as_deref()
            .map_or_else(|| checked.path.clone(), |id| canonical_in(scheme, slug, id)),
        normalized: node
            .title
            .as_deref()
            .map(normalized_summary)
            .filter(|title| !title.is_empty()),
    };
    if let Some(parsed) = &checked.parsed {
        return parsed.nodes.iter().map(of).collect();
    }
    let Some(found) = checked
        .span
        .as_ref()
        .and_then(|span| span.sections.as_ref())
    else {
        return Vec::new();
    };
    found
        .added
        .iter()
        .filter_map(|&at| found.parsed.nodes.get(at))
        .map(of)
        .collect()
}

/// The section form's target in the index, `(path, ord)`: the indexed
/// node whose span is the one the text replaces; `None` for a new file.
fn excluded(input: &CheckInput, checked: &Checked) -> Option<(String, usize)> {
    let span = checked.span.as_ref()?;
    let found = span.sections.as_ref()?;
    let file = input.files.iter().find(|file| file.path == span.path)?;
    let parsed = file.parsed.as_ref()?;
    let ord = parsed
        .nodes
        .iter()
        .position(|node| node.span == found.base_span)?;
    Some((span.path.clone(), ord))
}

/// The new document's own front-matter links of any type but `mentions`
/// (not a `superseded-by` source's): at least its link targets.
fn own_links(parsed: &ParsedFile) -> usize {
    parsed
        .links
        .iter()
        .filter(|link| {
            link.origin == LinkOrigin::Frontmatter
                && !is_weak_link(&link.link_type)
                && link.src_span.is_none()
        })
        .count()
}

/// The new document's front-matter link targets of any type but
/// `mentions` (its own, not a `superseded-by` source), resolved by core
/// over `input` with the new file in it, each named by its first end's
/// canonical ID, else that end's path; in written order, each once.
fn link_targets(
    project: &ProjectRoot,
    input: &mut CheckInput,
    path: &str,
    parsed: &ParsedFile,
) -> Vec<SimilarTarget> {
    let scheme = &project.config.scheme;
    let paths = &project.config.paths;
    input.files.push(CheckFile {
        path: path.to_owned(),
        size: 0,
        parsed: Some(parsed.clone()),
        read_error: None,
        bytes: Vec::new(),
    });
    let mut targets: Vec<SimilarTarget> = Vec::new();
    {
        let only = BTreeSet::from([path]);
        let resolved = resolve_files(input, scheme, paths, Some(&only));
        // Path → its parse, built once for the ends' names.
        let parses: BTreeMap<&str, &ParsedFile> = input
            .files
            .iter()
            .filter_map(|file| Some((file.path.as_str(), file.parsed.as_ref()?)))
            .collect();
        for file in &resolved.files {
            for edge in &file.edges {
                if edge.origin != LinkOrigin::Frontmatter
                    || is_weak_link(&edge.link_type)
                    || edge.names_source
                    || edge.holder.ord != 0
                {
                    continue;
                }
                let Endpoint::Nodes(nodes) = &edge.target else {
                    continue;
                };
                let ends: Vec<(String, usize)> = nodes
                    .iter()
                    .filter_map(|node| {
                        let end = resolved.paths.get(node.file)?;
                        Some(((*end).to_owned(), node.ord))
                    })
                    .collect();
                let Some(name) = ends
                    .first()
                    .map(|(end, ord)| node_name(&parses, scheme, &paths.features, end, *ord))
                else {
                    continue;
                };
                if !targets.iter().any(|target| target.name == name) {
                    targets.push(SimilarTarget { name, ends });
                }
            }
        }
    }
    input.files.pop();
    targets
}

/// The canonical ID of the node `ord` of the file `path` among `parses`,
/// else the path.
fn node_name(
    parses: &BTreeMap<&str, &ParsedFile>,
    scheme: &IdScheme,
    features: &str,
    path: &str,
    ord: usize,
) -> String {
    parses
        .get(path)
        .and_then(|parsed| parsed.nodes.get(ord))
        .and_then(|node| node.id.as_deref())
        .map_or_else(
            || path.to_owned(),
            |id| canonical_in(scheme, feature_stem(features, path), id),
        )
}

/// Probe 3: each live create or amendment adding a node titled as a new
/// one, by ID number; its first such node in text order, matched to the
/// first new node in text order.
fn queue_entries(
    adds: &[LiveAdd],
    titles: &[SimilarTitle],
    project: &ProjectRoot,
) -> Vec<SimilarEntry> {
    if titles.is_empty() {
        return Vec::new();
    }
    let scheme = &project.config.scheme;
    let features = &project.config.paths.features;
    let mut entries = Vec::new();
    for add in adds {
        let Some(parsed) = parsed_text(&add.target_path, &add.new_text, scheme) else {
            continue;
        };
        let slug = feature_stem(features, &add.target_path);
        let found = parsed.nodes.iter().find_map(|node| {
            let name = node.id.as_deref().map(|id| canonical_in(scheme, slug, id));
            let adds_it =
                add.new_file || name.as_ref().is_some_and(|name| add.new_ids.contains(name));
            if !adds_it {
                return None;
            }
            let title = node.title.as_deref()?;
            let normalized = normalized_summary(title);
            let new = titles.iter().find(|new| new.normalized == normalized)?;
            Some((node, name, new))
        });
        let Some((node, name, new)) = found else {
            continue;
        };
        let added = name.unwrap_or_else(|| add.target_path.clone());
        entries.push(SimilarEntry {
            id: Some(add.id.clone()),
            path: add.target_path.clone(),
            title: node.title.clone(),
            kind: node.kind.clone(),
            source: SimilarSource::Queue,
            reason: format!(
                "`{}` ({}), a live {}, adds `{added}` titled as new `{}`",
                add.id,
                add.status.as_str(),
                add.kind.as_str(),
                new.new
            ),
            gates: SimilarSource::Queue.gates(),
        });
    }
    entries
}

/// The list from the store's `rows` and the queue's entries, the queue's
/// after the alias rows, gating entries past [`SIMILAR_GATING_MAX`] left
/// out; and the gate: stored when every listed gating entry is named in
/// `distinct_from` (byte for byte; other names ignored) and none was left
/// out.
pub(crate) fn listed(
    rows: Vec<SimilarRow>,
    queued: Vec<SimilarEntry>,
    distinct_from: &[String],
) -> SimilarOutcome {
    // The store's gating rows (title, then alias) come first in its order.
    let (gating, advising): (Vec<SimilarEntry>, Vec<SimilarEntry>) = rows
        .into_iter()
        .map(entry_of)
        .partition(|entry| entry.gates);
    let mut gating = gating;
    gating.extend(queued);
    let total = gating.len();
    gating.truncate(SIMILAR_GATING_MAX);
    let unstored = if total > SIMILAR_GATING_MAX {
        Some(unnameable_note(total))
    } else if gating
        .iter()
        .any(|entry| !distinct_from.iter().any(|name| name == entry.name()))
    {
        Some(gated_note())
    } else {
        None
    };
    gating.extend(advising);
    SimilarOutcome {
        entries: gating,
        unstored,
    }
}

/// One store row as an entry, its reason written.
fn entry_of(row: SimilarRow) -> SimilarEntry {
    let new = row.new.as_deref().unwrap_or_default();
    let reason = match row.source {
        SimilarSource::Title => format!("title equal to new `{new}`'s"),
        SimilarSource::Alias => format!(
            "alias `{}` equal to new `{new}`'s title",
            row.detail.first().map(String::as_str).unwrap_or_default()
        ),
        SimilarSource::Search => format!(
            "search rank {} on {}",
            row.rank.unwrap_or_default(),
            named_list(&row.detail)
        ),
        SimilarSource::Neighbours => format!(
            "same kind `{}`, {} link targets shared: {}",
            row.kind.as_deref().unwrap_or_default(),
            row.detail.len(),
            named_list(&row.detail)
        ),
        // The store never gives one: the queue's entries are made above.
        SimilarSource::Queue => String::new(),
    };
    SimilarEntry {
        id: row.id,
        path: row.path,
        title: row.title,
        kind: row.kind,
        source: row.source,
        reason,
        gates: row.source.gates(),
    }
}

/// One entry as a text line: `similar: <id or path> | <source> |
/// gates|advises | <kind or -> | <path> | <title or -> | <reason>`.
pub(crate) fn similar_line(entry: &SimilarEntry) -> String {
    let judged = if entry.gates { "gates" } else { "advises" };
    crate::one_line(&format!(
        "similar: {} | {} | {judged} | {} | {} | {} | {}",
        entry.name(),
        entry.source.as_str(),
        entry.kind.as_deref().unwrap_or("-"),
        entry.path,
        entry.title.as_deref().unwrap_or("-"),
        entry.reason
    ))
}
