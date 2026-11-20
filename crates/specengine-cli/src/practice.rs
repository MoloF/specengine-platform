//! A card's and a task's practices, and the IDs a text cites (task spec
//! `department-practices` and its decision record): read per call from the
//! index after its freshness step (on a hub, its trust), in one snapshot,
//! nothing cached and every list capped. A card's context and a task's
//! package take them from their bundle's own read ([`PracticeAsk`]), so one
//! freshness step and one snapshot serve both; a read with no bundle
//! ([`read`]) makes its own.
//!
//! - **Practices**: the store's one query over `disciplines`
//!   (`practices_of`), live, non-generated documents only, by path: the
//!   first [`PRACTICES_MAX`] whose rows hold a document ([`PracticeFiles`]);
//!   past them one note, `practices: <k> more practice documents of
//!   <discipline, …>, from <first left out>`, counted from the same paths.
//!   Each is served as [`CardPractice`]: the map's canonical name, its
//!   title, path, kept disciplines and rule count (core's
//!   `practice::rule_count`).
//! - **Cited**: core's body-reference scan over the texts in order, each
//!   distinct reference once, at most [`CITED_MAX`] looked up; each is
//!   resolved as `spec show` resolves a REF, to its first holder by (path,
//!   position), and one that resolves to nothing is left out.
//!
//! No file is read from the working tree for them: titles and paths are
//! the index's. Nothing here gates, refuses or judges: nothing is blocked
//! by a discrepancy.

use serde::Serialize;
use specengine_core::check::{Endpoint, GraphRows, NodeAt, SpecGraph};
use specengine_core::map::{CanonicalNames, MapNaming};
use specengine_core::practice::{
    CITED_CHARS_MAX, CITED_MAX, Citation, PRACTICES_CHARS_MAX, PRACTICES_MAX, cited_in,
    document_disciplines, rule_count,
};
use specengine_model::{IdScheme, PackagePractice, Reference};
use specengine_store::{PracticeRow, WorkingTree};

use crate::freshness::step;
use crate::location::open_index;
use crate::project::ProjectRoot;
use crate::subset::practice_input;
use crate::{CliError, Env, Message, one_line, store_error};

/// One practice as a card or a task serves it: the task package's entry,
/// one shape.
pub type CardPractice = PackagePractice;

/// One ID a card's or a proposal's text cites, resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Cited {
    /// The occurrence's bytes, verbatim: what a reader links.
    pub written: String,
    /// The map's canonical name of its first holder.
    pub name: String,
    pub title: Option<String>,
    pub path: String,
}

/// What one practices read gives.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PracticeRead {
    /// By path, at most [`PRACTICES_MAX`].
    pub practices: Vec<CardPractice>,
    /// Past [`PRACTICES_MAX`] or [`PRACTICES_CHARS_MAX`]: how many more,
    /// and from which path.
    pub note: Option<String>,
    /// First-seen order, at most [`CITED_MAX`].
    pub cited: Vec<Cited>,
    /// Past [`CITED_CHARS_MAX`]: how many more, and from which.
    pub cited_note: Option<String>,
    /// The freshness step's warnings; none when a bundle's read gave it
    /// (the bundle's messages carry them).
    pub messages: Vec<Message>,
}

/// The practice files a read keeps: of the distinct paths the index's one
/// query gives, by path, the first [`PRACTICES_MAX`] whose rows hold a
/// document, then how many paths are past them and the first of those. A
/// path whose rows do not hold a document is dropped from both.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PracticeFiles {
    pub listed: Vec<String>,
    pub left: usize,
    pub first_left: Option<String>,
}

impl PracticeFiles {
    /// `all` (distinct, by path) kept in order while `holds` a document,
    /// until [`PRACTICES_MAX`] are kept; `holds` is asked of no path past
    /// them.
    pub(crate) fn kept(all: &[String], mut holds: impl FnMut(&str) -> bool) -> Self {
        let mut listed = Vec::new();
        let mut consumed = 0;
        for path in all {
            if listed.len() == PRACTICES_MAX {
                break;
            }
            consumed += 1;
            if holds(path) {
                listed.push(path.clone());
            }
        }
        Self {
            listed,
            left: all.len() - consumed,
            first_left: all.get(consumed).cloned(),
        }
    }
}

/// The distinct paths of the index's rows, by path (the rows come by path).
pub(crate) fn distinct_paths(rows: &[PracticeRow]) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for row in rows {
        if paths.last() != Some(&row.path) {
            paths.push(row.path.clone());
        }
    }
    paths
}

/// The room a caller's lines leave once they hold a read's practices and
/// citations, in characters, else why there is none.
pub(crate) type Room<'a> = dyn Fn(&PracticeRead) -> Result<usize, String> + 'a;

/// What a card's or a task's read asks of its bundle's own read (task spec
/// `department-practices`): the practices of the bundle's disciplines and
/// the references its texts cite, over the bundle's graph.
pub(crate) struct PracticeAsk<'a> {
    /// Resolved over the bundle's graph; taken by the read.
    pub citations: Vec<Citation>,
    /// The room the caller's lines leave once they hold the practices and
    /// the citations (a card's context: its lines count them), else why
    /// there is none; `None`: the bundle's own ceiling.
    pub room: Option<&'a Room<'a>>,
    /// Set once the bundle's graph is read, before it is fitted; `None`:
    /// the bundle stopped before its read.
    pub read: Option<PracticeRead>,
}

impl PracticeAsk<'_> {
    /// The references to resolve, in order.
    pub(crate) fn references(&self) -> Vec<&Reference> {
        self.citations
            .iter()
            .map(|citation| &citation.reference)
            .collect()
    }
}

/// The citations of `texts` a read resolves (core's scan, at most
/// [`CITED_MAX`]).
pub(crate) fn citations(texts: &[&str], scheme: &IdScheme) -> Vec<Citation> {
    cited_in(texts.iter().copied(), scheme, CITED_MAX)
}

/// The practices of `disciplines` (judged names, sorted, each once; none:
/// no practice) and the resolved `citations`, from one snapshot of the
/// project's index after its freshness step. No discipline and no citation:
/// nothing to look up, and no index is opened.
pub(crate) fn read(
    env: &Env,
    project: &ProjectRoot,
    disciplines: &[String],
    citations: Vec<Citation>,
) -> Result<PracticeRead, CliError> {
    if disciplines.is_empty() && citations.is_empty() {
        return Ok(PracticeRead::default());
    }
    let scheme = &project.config.scheme;
    let paths = &project.config.paths;
    let mut open = open_index(env, project)?;
    let tree = WorkingTree::new(&project.root, paths).map_err(store_error)?;
    let messages = step(&mut open.index, project, &tree)?;
    let snapshot = open.index.snapshot().map_err(store_error)?;
    let references: Vec<&Reference> = citations
        .iter()
        .map(|citation| &citation.reference)
        .collect();
    let (input, files) =
        practice_input(&snapshot, scheme, paths, disciplines, &references).map_err(store_error)?;
    drop(snapshot);
    let graph = SpecGraph::from_rows(&input, scheme, paths, GraphRows::default());
    let mut read = from_graph(&graph, project, disciplines, &files, citations);
    read.messages = messages;
    Ok(read)
}

/// The practices of `files` and the resolved `citations` over `graph`: a
/// bundle's own graph, or [`read`]'s (see the module documentation).
pub(crate) fn from_graph(
    graph: &SpecGraph<'_>,
    project: &ProjectRoot,
    disciplines: &[String],
    files: &PracticeFiles,
    citations: Vec<Citation>,
) -> PracticeRead {
    let naming = MapNaming {
        scheme: &project.config.scheme,
        features: &project.config.paths.features,
    };
    let names = CanonicalNames::new(graph, naming);
    let built: Vec<CardPractice> = files
        .listed
        .iter()
        .filter_map(|path| {
            let file = graph.file_of(path)?;
            let document = graph.document(file)?;
            let node = graph.node(document)?;
            Some(CardPractice {
                name: names.name(document),
                title: node.title.clone(),
                path: path.clone(),
                disciplines: document_disciplines(node),
                rules: rule_count(graph.nodes(file)),
            })
        })
        .collect();
    let (practices, over) = within_chars(built, PRACTICES_CHARS_MAX);
    // One note for both caps: the documents left past the characters, then
    // those past the count, from the first left out.
    let left = over.len() + files.left;
    let first = over
        .first()
        .map(|practice| practice.path.clone())
        .or_else(|| files.first_left.clone());
    let note = first.map(|first| {
        one_line(&format!(
            "practices: {left} more practice documents of {}, from {first}",
            disciplines.join(", ")
        ))
    });
    let resolved_all: Vec<Cited> = citations
        .into_iter()
        .filter_map(|citation| resolved(graph, &names, citation))
        .collect();
    let (cited, over) = within_chars(resolved_all, CITED_CHARS_MAX);
    let cited_note = over.first().map(|first| {
        one_line(&format!(
            "cited: {} more cited IDs past {CITED_CHARS_MAX} characters, from {}",
            over.len(),
            first.written
        ))
    });
    PracticeRead {
        practices,
        note,
        cited,
        cited_note,
        messages: Vec::new(),
    }
}

/// `entries` kept in order while their compact JSON, each with a comma,
/// stays within `chars_max` characters; the rest after the first that does
/// not fit.
fn within_chars<T: Serialize>(entries: Vec<T>, chars_max: usize) -> (Vec<T>, Vec<T>) {
    let mut used = 0usize;
    let mut kept = Vec::new();
    let mut entries = entries.into_iter();
    for entry in entries.by_ref() {
        let chars = serde_json::to_string(&entry).map_or(usize::MAX, |json| json.chars().count());
        let cost = chars.saturating_add(1);
        if used.saturating_add(cost) > chars_max {
            let mut over = vec![entry];
            over.extend(entries);
            return (kept, over);
        }
        used += cost;
        kept.push(entry);
    }
    (kept, Vec::new())
}

/// `citation` resolved to its first holder; `None` when it names nothing.
fn resolved(
    graph: &SpecGraph<'_>,
    names: &CanonicalNames<'_, '_>,
    citation: Citation,
) -> Option<Cited> {
    let Endpoint::Nodes(mut nodes) = graph.locate(&citation.reference, &citation.written) else {
        return None;
    };
    nodes.sort_unstable();
    let first: NodeAt = *nodes.first()?;
    Some(Cited {
        written: citation.written,
        name: names.name(first),
        title: graph.node(first).and_then(|node| node.title.clone()),
        path: graph.paths().get(first.file)?.to_string(),
    })
}
