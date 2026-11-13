//! `spec map [--archive]` (task spec `corpus-map` CM1 to CM3): the whole
//! corpus as one map, over `spec tree`'s input (one index update, the whole
//! input, the spec graph built once). Core's [`corpus_map`] gives the
//! nodes, the merged edges, the parents and the unresolved targets; core's
//! `usage_of` the uses. It moves, refuses and gates nothing.
//!
//! ```text
//! map | nodes <n> (<d> documents, <s> sections), edges <e>, parents <p>, ghosts <g>, orphans <o>
//! <name> | <kind or -> | used by <u>, mentioned by <m>[ | of <document>][ | orphan] | <title or -> | <path>:<line>[ | archived]
//! <src> --<type>--> <dst>[ | places <p>]
//! <child> in <parent>
//! ghost <written> | from <source>, … | places <p>
//! [truncated: <k> of <n> lines not shown; the daemon's map is whole]
//! ```
//!
//! A name is a node's canonical reference (below). Every value is on one
//! line, every line's control characters escaped; the JSON holds the
//! values raw. The header prints, then the lines, each whole, while they
//! fit in [`crate::OUTPUT_CAP_CHARS`] (the first always), then the tail.
//! The JSON is never cut: its edges, parents and ghosts name nodes by
//! index.
//!
//! A node's name is its canonical reference (core's `node_names`, over
//! `[ids]` and `[paths] features`): `slug/ID` in a feature document for a
//! feature-scoped ID or a repeated one, else its ID, else its path.
//!
//! The notes are the walk's warnings as `spec tree` words them (a
//! `parent:` cycle, a parent's several holders), a name several mapped
//! nodes hold and the input's own notes and warnings (a file that does not read),
//! sorted, each once: the JSON's `notes`. On stderr, escaped, as `spec
//! catalog` prints its own: a `note:` line per note that is no warning,
//! then a `warning:` line per warning, each group sorted. None changes the
//! exit: 0 when answered, no node included; 2 when it cannot run.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use specengine_core::check::{NodeAt, SpecGraph};
use specengine_core::map::{
    CorpusMap, MapEdge, MapGhost, MapNaming, MapParent, corpus_map, node_names,
};

use crate::cap::{OUTPUT_CAP_CHARS, lines_fit};
use crate::corpus::{holders_warning, indexed};
use crate::project::discover;
use crate::tree::parent_warning;
use crate::{CliError, Env, Globals, Message, escape_controls, one_line};

/// What a text line shows for no value.
const ABSENT: &str = "-";

/// How a name several mapped nodes hold is taken (task spec `corpus-map`
/// CM1): each is a node, a link lands on the first of them.
const SHARED_ID: &str = "each mapped, its links on the first";

/// `spec map` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapRequest {
    /// `--archive`: Tier 3 files' nodes and links too.
    pub archive: bool,
}

/// One node of the map, as the JSON serves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapRow {
    /// Its canonical reference (core's `node_names`): `slug/ID` in a
    /// feature document for a feature-scoped or repeated ID, else its ID,
    /// else its file's path.
    pub name: String,
    pub id: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub path: String,
    pub line: usize,
    /// A section's document, an index into [`MapOutcome::rows`]; `None` for
    /// a document.
    pub of: Option<usize>,
    /// A document's `status:`.
    pub status: Option<String>,
    /// Its file is Tier 3.
    pub archived: bool,
    /// Core's `usage_of` (task spec `usage-orphans` UO1 to UO3): live
    /// sources alone, whatever `--archive` admits.
    pub used_by: u32,
    pub mentioned_by: u32,
    /// `None` for a section and for a document whose file is not live.
    pub orphan: Option<bool>,
}

/// What `spec map` read: every node, edge, parent and ghost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapOutcome {
    pub archive: bool,
    /// Sorted, each once: the notes and the warnings.
    pub notes: Vec<String>,
    /// Of `notes`, the warnings (the input's, the walk's, an ID several
    /// nodes hold), sorted: stderr's `warning:` lines.
    pub warnings: Vec<String>,
    /// In (path, position) order.
    pub rows: Vec<MapRow>,
    /// By (source, target, type).
    pub edges: Vec<MapEdge>,
    /// By child.
    pub parents: Vec<MapParent>,
    /// By written text.
    pub ghosts: Vec<MapGhost>,
}

impl MapOutcome {
    /// One `note:` line per note that is no warning, then one `warning:`
    /// line per warning, escaped.
    pub(crate) fn stderr_lines(&self) -> Vec<String> {
        let notes = self
            .notes
            .iter()
            .filter(|note| self.warnings.binary_search(note).is_err())
            .map(|note| Message::Note(note.clone()));
        let warnings = self
            .warnings
            .iter()
            .map(|warning| Message::Warning(warning.clone()));
        notes
            .chain(warnings)
            .map(|message| escape_controls(&message.line()))
            .collect()
    }

    /// The text cut at `cap` characters (task spec `corpus-map` CM3): the
    /// header, then the lines, each whole, while the two fit in `cap` (the
    /// first line always), then the tail when a line does not print. `spec
    /// map` cuts at [`crate::OUTPUT_CAP_CHARS`].
    pub fn text_within(&self, cap: usize) -> String {
        let header = self.header();
        let lines = self.lines();
        let (shown, _) = lines_fit(
            lines.iter().map(|line| line.chars().count() + 1),
            header.chars().count() + 1,
            true,
            cap,
        );
        let mut out = header;
        out.push('\n');
        for line in &lines[..shown] {
            out.push_str(line);
            out.push('\n');
        }
        if shown < lines.len() {
            out.push_str(&format!(
                "[truncated: {} of {} lines not shown; the daemon's map is whole]\n",
                lines.len() - shown,
                lines.len()
            ));
        }
        out
    }

    fn counts(&self) -> MapCounts {
        let documents = self.rows.iter().filter(|row| row.of.is_none()).count();
        MapCounts {
            documents,
            sections: self.rows.len() - documents,
            edges: self.edges.len(),
            parents: self.parents.len(),
            ghosts: self.ghosts.len(),
            orphans: self
                .rows
                .iter()
                .filter(|row| row.orphan == Some(true))
                .count(),
        }
    }

    fn header(&self) -> String {
        let counts = self.counts();
        format!(
            "map | nodes {} ({} documents, {} sections), edges {}, parents {}, ghosts {}, orphans {}",
            self.rows.len(),
            counts.documents,
            counts.sections,
            counts.edges,
            counts.parents,
            counts.ghosts,
            counts.orphans
        )
    }

    /// Every line under the header, escaped, no line end: the nodes, the
    /// edges, the parents, then the ghosts.
    fn lines(&self) -> Vec<String> {
        let name = |index: usize| {
            self.rows
                .get(index)
                .map_or_else(|| ABSENT.to_owned(), |row| one_line(&row.name))
        };
        let nodes = self.rows.iter().map(|row| {
            let mut marks = String::new();
            if let Some(of) = row.of {
                marks.push_str(&format!(" | of {}", name(of)));
            }
            if row.orphan == Some(true) {
                marks.push_str(" | orphan");
            }
            // The tail `spec tree` and `spec graph` end a Tier 3 node with.
            let archived = if row.archived { " | archived" } else { "" };
            format!(
                "{} | {} | used by {}, mentioned by {}{marks} | {} | {}:{}{archived}",
                one_line(&row.name),
                row.kind
                    .as_deref()
                    .map_or_else(|| ABSENT.to_owned(), one_line),
                row.used_by,
                row.mentioned_by,
                row.title
                    .as_deref()
                    .map_or_else(|| ABSENT.to_owned(), one_line),
                one_line(&row.path),
                row.line
            )
        });
        let edges = self.edges.iter().map(|edge| {
            let places = if edge.places > 1 {
                format!(" | places {}", edge.places)
            } else {
                String::new()
            };
            format!(
                "{} --{}--> {}{places}",
                name(edge.src),
                one_line(&edge.link_type),
                name(edge.dst)
            )
        });
        let parents = self
            .parents
            .iter()
            .map(|parent| format!("{} in {}", name(parent.child), name(parent.parent)));
        let ghosts = self.ghosts.iter().map(|ghost| {
            let sources: Vec<String> = ghost.sources.iter().map(|&source| name(source)).collect();
            format!(
                "ghost {} | from {} | places {}",
                one_line(&ghost.written),
                sources.join(", "),
                ghost.places
            )
        });
        nodes
            .chain(edges)
            .chain(parents)
            .chain(ghosts)
            .map(|line| escape_controls(&line))
            .collect()
    }
}

/// `spec map`: updates the index, then maps every admitted node.
pub fn map(env: &Env, globals: &Globals, request: &MapRequest) -> Result<MapOutcome, CliError> {
    let project = discover(env, globals)?;
    project.slug()?;
    let mut messages = Vec::new();
    let input = indexed(env, &project, &mut messages, true)?;
    let graph = SpecGraph::new(&input, &project.config.scheme, &project.config.paths);
    let map = corpus_map(&graph, request.archive);
    let naming = MapNaming {
        scheme: &project.config.scheme,
        features: &project.config.paths.features,
    };
    let names = node_names(&graph, &map, &naming);
    let CorpusMap {
        nodes,
        edges,
        parents,
        ghosts,
    } = map;

    // A name more than one mapped node holds: an ID held more than once
    // outside feature documents, or twice in one (an ID-less document's
    // path is its own).
    let mut holders: BTreeMap<&str, Vec<NodeAt>> = BTreeMap::new();
    for (node, name) in nodes.iter().zip(&names) {
        messages.extend(parent_warning(&graph, node.at));
        if graph.node(node.at).is_some_and(|found| found.id.is_some()) {
            holders.entry(name.as_str()).or_default().push(node.at);
        }
    }
    for (name, held) in &holders {
        messages.extend(holders_warning(&graph, name, held, SHARED_ID));
    }
    let mut notes: BTreeSet<String> = BTreeSet::new();
    let mut warnings: BTreeSet<String> = BTreeSet::new();
    for message in &messages {
        match message {
            Message::Note(text) => {
                notes.insert(one_line(text));
            }
            Message::Warning(text) => {
                notes.insert(one_line(text));
                warnings.insert(one_line(text));
            }
        }
    }

    let rows = nodes
        .iter()
        .zip(names)
        .map(|(node, name)| {
            let found = graph.node(node.at);
            MapRow {
                name,
                id: found.and_then(|found| found.id.clone()),
                kind: found.and_then(|found| found.kind.clone()),
                title: found.and_then(|found| found.title.clone()),
                path: graph.paths()[node.at.file].to_owned(),
                line: graph.line(node.at),
                of: node.of,
                status: found
                    .filter(|_| node.at.ord == 0)
                    .and_then(|found| found.fields.as_ref())
                    .and_then(|fields| fields.status.clone()),
                archived: graph.is_tier3(node.at.file),
                used_by: node.usage.used_by,
                mentioned_by: node.usage.mentioned_by,
                orphan: node.usage.orphan,
            }
        })
        .collect();
    Ok(MapOutcome {
        archive: request.archive,
        notes: notes.into_iter().collect(),
        warnings: warnings.into_iter().collect(),
        rows,
        edges,
        parents,
        ghosts,
    })
}

/// The header, the lines that fit in [`OUTPUT_CAP_CHARS`], then the tail
/// when one does not.
pub(crate) fn render_text(outcome: &MapOutcome) -> String {
    outcome.text_within(OUTPUT_CAP_CHARS)
}

/// The JSON document's 7 keys, in this order (task spec `corpus-map` CM3).
#[derive(Serialize)]
struct MapJson<'a> {
    archive: bool,
    notes: &'a [String],
    counts: MapCounts,
    nodes: Vec<NodeJson<'a>>,
    edges: Vec<EdgeJson<'a>>,
    parents: Vec<ParentJson>,
    ghosts: Vec<GhostJson<'a>>,
}

/// The counts' 6 keys.
#[derive(Serialize)]
struct MapCounts {
    documents: usize,
    sections: usize,
    edges: usize,
    parents: usize,
    ghosts: usize,
    orphans: usize,
}

/// A node's 12 keys.
#[derive(Serialize)]
struct NodeJson<'a> {
    name: &'a str,
    id: Option<&'a str>,
    kind: Option<&'a str>,
    title: Option<&'a str>,
    path: &'a str,
    line: usize,
    of: Option<usize>,
    status: Option<&'a str>,
    archived: bool,
    used_by: u32,
    mentioned_by: u32,
    orphan: Option<bool>,
}

/// An edge's 5 keys.
#[derive(Serialize)]
struct EdgeJson<'a> {
    src: usize,
    dst: usize,
    #[serde(rename = "type")]
    link_type: &'a str,
    weak: bool,
    places: usize,
}

/// A parent's 2 keys.
#[derive(Serialize)]
struct ParentJson {
    child: usize,
    parent: usize,
}

/// A ghost's 4 keys.
#[derive(Serialize)]
struct GhostJson<'a> {
    written: &'a str,
    reason: &'a str,
    sources: &'a [usize],
    places: usize,
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`, nothing cut.
impl Serialize for MapOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        MapJson {
            archive: self.archive,
            notes: &self.notes,
            counts: self.counts(),
            nodes: self
                .rows
                .iter()
                .map(|row| NodeJson {
                    name: &row.name,
                    id: row.id.as_deref(),
                    kind: row.kind.as_deref(),
                    title: row.title.as_deref(),
                    path: &row.path,
                    line: row.line,
                    of: row.of,
                    status: row.status.as_deref(),
                    archived: row.archived,
                    used_by: row.used_by,
                    mentioned_by: row.mentioned_by,
                    orphan: row.orphan,
                })
                .collect(),
            edges: self
                .edges
                .iter()
                .map(|edge| EdgeJson {
                    src: edge.src,
                    dst: edge.dst,
                    link_type: &edge.link_type,
                    weak: edge.weak,
                    places: edge.places,
                })
                .collect(),
            parents: self
                .parents
                .iter()
                .map(|parent| ParentJson {
                    child: parent.child,
                    parent: parent.parent,
                })
                .collect(),
            ghosts: self
                .ghosts
                .iter()
                .map(|ghost| GhostJson {
                    written: &ghost.written,
                    reason: &ghost.reason,
                    sources: &ghost.sources,
                    places: ghost.places,
                })
                .collect(),
        }
        .serialize(serializer)
    }
}
