//! `spec health` (task spec `health-left`): the unapplied amendments of the
//! working tree, row 3 of Health's "What is left". Core's
//! [`unapplied_amendments`] over the tree walked as `spec check` walks it
//! (`load_config`, [`WorkingTree`], `check_input`); no index, database,
//! queue, git, clock or baseline, the data directory never resolved;
//! nothing is written. Uncut.
//!
//! Each amendment is a walked live document that names a node it amends,
//! listed with its `amends` out links exactly as `spec show <path>
//! --links` lists them (the spec graph over the same walk, the shown
//! document's own file admitted, no other resolution). `documents` is the
//! walk's file count, `spec check`'s `counts.documents` on the same tree.
//! A document whose front-matter does not read is not judged: no row, one
//! note `<path>:<line>: <code>: <message>; not judged`.
//!
//! Exit 0 whatever is listed, none too; exit 2 when it cannot run: no
//! project, a config that cannot be used (one `<config>:<line>: message`
//! line per cause), a root that cannot be read, an incomplete walk (its
//! first gap by path, worded as `spec export index` words it: never a
//! partial list). Never 1.
//!
//! ```text
//! <path>[ | id <id>][ | class <class>][ | status <status>][ | <title>]
//!   out <type> <name, else written> | <path>:<line>[ | as <written>][ | dangling: <reason> | …]
//! spec health: <d> documents, <n> unapplied amendments
//! ```
//!
//! Amendments by path (byte order), links in `show --links` order, notes
//! by path: one tree and config give the same bytes, whatever the root or
//! the walk's order.

use serde::Serialize;
use specengine_core::check::{SpecGraph, unapplied_amendments};
use specengine_model::AMENDS;
use specengine_store::{NamedBytes, WorkingTree, check_input, load_config};

use crate::corpus::Admission;
use crate::export::{config_errors, incomplete, io_message};
use crate::links::{LinkJson, ShownLink, link_json, link_line, node_links};
use crate::project::locate;
use crate::search::notes;
use crate::{CliError, Env, Globals, Message, one_line};

/// What `spec health` found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthOutcome {
    /// The walked files: `spec check`'s `counts.documents`.
    pub documents: usize,
    /// By path.
    pub amendments: Vec<UnappliedAmendment>,
    /// A note per file not judged, by path.
    pub messages: Vec<Message>,
}

/// One unapplied amendment: a live document that names a node it amends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnappliedAmendment {
    /// Root-relative.
    pub path: String,
    /// The document's ID; `None` without one.
    pub id: Option<String>,
    /// Its `title:`, else its first H1; `None` without either.
    pub title: Option<String>,
    /// `class:` as written; `None` without one.
    pub class: Option<String>,
    /// `status:` as written; `None` without one.
    pub status: Option<String>,
    /// Its `amends` out links, every state, as `spec show <path> --links`
    /// lists them.
    pub links: Vec<ShownLink>,
}

/// Why nothing was measured, the end of every refusal after discovery.
const NOT_MEASURED: &str = "unapplied amendments not measured";

/// `spec health`: walks the working tree and lists its unapplied
/// amendments.
pub fn health(env: &Env, globals: &Globals) -> Result<HealthOutcome, CliError> {
    let located = locate(env, globals)?;
    let label = located.config_label.as_str();
    let config = NamedBytes::read(label, &located.config_file);
    let (project, _) = load_config(&config).map_err(|report| config_errors(label, &report))?;
    let tree = WorkingTree::new(&located.root, &project.paths).map_err(|error| {
        CliError::spec(format!(
            "the root cannot be read: {}: {NOT_MEASURED}",
            io_message(&error)
        ))
    })?;
    let input = check_input(&tree, &project.scheme);
    if let Some(gap) = incomplete(&input, &project.paths) {
        return Err(CliError::spec(format!(
            "the walk is incomplete: {gap}: {NOT_MEASURED}"
        )));
    }

    let found = unapplied_amendments(&input);
    let messages = found
        .not_judged
        .iter()
        .map(|(file, failure)| {
            Message::Note(format!(
                "{}:{}: {}: {}; not judged",
                file.path,
                failure.line,
                failure.code.as_str(),
                failure.message
            ))
        })
        .collect();
    // The graph resolves every link of the corpus: built only when a
    // document is listed.
    let graph = (!found.amendments.is_empty())
        .then(|| SpecGraph::new(&input, &project.scheme, &project.paths));
    let amendments = found
        .amendments
        .iter()
        .map(|file| {
            let document = file.parsed.as_ref().and_then(|parsed| parsed.document());
            let fields = document.and_then(|node| node.fields.as_ref());
            UnappliedAmendment {
                path: file.path.clone(),
                id: document.and_then(|node| node.id.clone()),
                title: document.and_then(|node| node.title.clone()),
                class: fields.and_then(|fields| fields.class.clone()),
                status: fields.and_then(|fields| fields.status.clone()),
                links: graph
                    .as_ref()
                    .map(|graph| amends_links(graph, &file.path))
                    .unwrap_or_default(),
            }
        })
        .collect();
    Ok(HealthOutcome {
        documents: input.files.len(),
        amendments,
        messages,
    })
}

/// The `amends` out links of the document at `path`, as `spec show <path>
/// --links` lists them: its own file admitted, no Tier 3 one.
fn amends_links(graph: &SpecGraph<'_>, path: &str) -> Vec<ShownLink> {
    let Some(at) = graph.file_of(path).and_then(|file| graph.document(file)) else {
        return Vec::new();
    };
    let admission = Admission::new(graph, false, [at.file]);
    node_links(graph, at, &admission)
        .outgoing
        .into_iter()
        .filter(|link| link.link_type == AMENDS)
        .collect()
}

/// The amendment's header line: its path, then each part it has.
fn header_line(amendment: &UnappliedAmendment) -> String {
    let mut line = one_line(&amendment.path);
    for (label, value) in [
        ("id ", &amendment.id),
        ("class ", &amendment.class),
        ("status ", &amendment.status),
        ("", &amendment.title),
    ] {
        if let Some(value) = value {
            line.push_str(&format!(" | {label}{}", one_line(value)));
        }
    }
    line
}

/// Per amendment its header line and its links' lines, then the count
/// line, always.
pub(crate) fn render_text(outcome: &HealthOutcome) -> String {
    let mut out = String::new();
    for amendment in &outcome.amendments {
        out.push_str(&header_line(amendment));
        out.push('\n');
        for link in &amendment.links {
            out.push_str(&link_line(link));
            out.push('\n');
        }
    }
    out.push_str(&format!(
        "spec health: {} documents, {} unapplied amendments\n",
        outcome.documents,
        outcome.amendments.len()
    ));
    out
}

#[derive(Serialize)]
struct HealthJson<'a> {
    documents: usize,
    amendments: Vec<AmendmentJson<'a>>,
    notes: Vec<String>,
}

#[derive(Serialize)]
struct AmendmentJson<'a> {
    path: &'a str,
    id: Option<&'a str>,
    title: Option<&'a str>,
    class: Option<&'a str>,
    status: Option<&'a str>,
    links: Vec<LinkJson<'a>>,
}

fn view(outcome: &HealthOutcome) -> HealthJson<'_> {
    HealthJson {
        documents: outcome.documents,
        amendments: outcome
            .amendments
            .iter()
            .map(|amendment| AmendmentJson {
                path: &amendment.path,
                id: amendment.id.as_deref(),
                title: amendment.title.as_deref(),
                class: amendment.class.as_deref(),
                status: amendment.status.as_deref(),
                links: amendment.links.iter().map(link_json).collect(),
            })
            .collect(),
        notes: notes(&outcome.messages),
    }
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`, empty `[]`.
impl Serialize for HealthOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        view(self).serialize(serializer)
    }
}
