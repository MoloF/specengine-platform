//! `spec health` (task spec `health-left`): the unapplied amendments of the
//! working tree, row 3 of Health's "What is left". Core's
//! [`unapplied_amendments`] over the tree walked as `spec check` walks it
//! (`load_config`, [`WorkingTree`], `check_input`); no index, database,
//! queue, git, clock or baseline for it. Uncut.
//!
//! Rows 4-5 (task spec `health-code`) come after it, from the
//! same config bytes: [`crate::health_code`] reads the code index the call
//! refreshes, as `spec refs` does (the data directory's two indexes are
//! written, nothing else; without `[code]` nothing is opened). Their
//! states and causes never change row 3 or the exit; the refreshes' notes
//! and a `cannot` row's causes go to stderr, never into `notes`.
//!
//! Row 6 (task spec `health-drift`) comes last: [`crate::health_drift`]
//! runs plain `spec verify`'s judge in-process over the handle rows 4-5
//! read (no walk of its own; git through the caller's [`GitEnv`], read
//! only), its rows but `ok` in `drift`; never a mode, a verdict or the
//! exit.
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
//! <rows 4-6's rows, their summaries or causes, `code:` when read>
//! spec health: <d> documents, <n> unapplied amendments
//! ```
//!
//! Amendments by path (byte order), links in `show --links` order, notes
//! by path: one tree and config give the same bytes, whatever the root or
//! the walk's order.
//!
//! Row 8 (task spec `usage-orphans` UO4): `orphans`, every live document of
//! the same walk that nothing links (core's `usage_of` over the spec graph,
//! now built on every call), wherever it sits; by kind in code-point order
//! (none last), then by path. Each row's tasks are `spec trace`'s count of
//! its cards ([`node_work`], cancelled ones never read), from the queue
//! read only when a row is listed; a queue that is not there or does not
//! read leaves both `null` with one note in `orphans.notes`, never a
//! failure or another exit, and the card read's own notes join it. Each
//! note is also a stderr `note:` line, escaped, after rows 4-6's (as
//! `spec catalog` prints its own). The block comes after rows 4-6's lines,
//! uncut:
//!
//! ```text
//! orphans: <n> of <live> live documents link to nothing
//!   <name> | <kind or -> | <title or -> | <path>:<line>[ | tasks <t>, open <o>]
//! ```

use std::collections::BTreeSet;

use serde::Serialize;
use specengine_core::check::{NodeAt, SpecGraph, unapplied_amendments};
use specengine_core::usage::usage_of;
use specengine_model::AMENDS;
use specengine_store::{GitEnv, NamedBytes, WorkingTree, check_input, load_config};

use crate::corpus::Admission;
use crate::export::{config_errors, incomplete, io_message};
use crate::health_code::{
    AssumesCounts, AssumesRow, CodeRow, CodeRows, UnboundCounts, UnboundRow, code_line, code_rows,
    row_lines, summary_lines,
};
use crate::health_drift::{HealthDrift, drift_lines, drift_row, drift_summary};
use crate::links::{LinkJson, ShownLink, link_json, link_line, node_links};
use crate::project::{ProjectRoot, locate};
use crate::refs::CodeCount;
use crate::search::notes;
use crate::trace::{NodeWork, node_work, queue_read};
use crate::{CliError, Env, Globals, Message, escape_controls, one_line};

/// What `spec health` found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthOutcome {
    /// The walked files: `spec check`'s `counts.documents`.
    pub documents: usize,
    /// By path.
    pub amendments: Vec<UnappliedAmendment>,
    /// A note per file not judged, by path.
    pub messages: Vec<Message>,
    /// Rows 4-5 (task spec `health-code`).
    pub code_rows: CodeRows,
    /// Row 6 (task spec `health-drift`).
    pub drift: HealthDrift,
    /// Row 8 (task spec `usage-orphans` UO4).
    pub orphans: HealthOrphans,
}

/// Row 8: the live documents nothing links (task spec `usage-orphans` UO4).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct HealthOrphans {
    /// The live documents judged.
    pub live: usize,
    /// By `kind` in code-point order (`null` last), then by path.
    pub rows: Vec<HealthOrphan>,
    /// Only with a row, sorted, each once, `orphans: `-prefixed, raw: why
    /// no task was read, or the card read's own notes. Each is also a
    /// `note:` line on stderr, escaped.
    pub notes: Vec<String>,
}

/// One orphan document: its place and its tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HealthOrphan {
    /// Its ID, else its path.
    pub name: String,
    pub id: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    /// Root-relative.
    pub path: String,
    pub line: usize,
    /// `status:` as written.
    pub status: Option<String>,
    /// Its cards, cancelled ones never read, and of them the open; both
    /// `null` when the queue was not read.
    pub tasks: Option<usize>,
    pub tasks_open: Option<usize>,
}

/// The note of a queue no queue step has run on (row 8).
const ORPHANS_NO_QUEUE: &str = "orphans: no queue yet: no task read";

/// `spec health` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthRequest {
    /// The caller's git environment: row 6 reads git as plain `spec
    /// verify` does (read-only).
    pub git: GitEnv,
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
/// amendments; then rows 4-6.
pub fn health(
    env: &Env,
    globals: &Globals,
    request: &HealthRequest,
) -> Result<HealthOutcome, CliError> {
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
    // The graph resolves every link of the corpus: row 3's links and row
    // 8's uses (task spec `usage-orphans` UO4) read it.
    let graph = SpecGraph::new(&input, &project.scheme, &project.paths);
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
                links: amends_links(&graph, &file.path),
            }
        })
        .collect();
    let slug = project.slug().map_err(|error| error.message.clone());
    let orphans = orphans_of(env, &graph, slug);

    // Rows 4-5 read row 3's config bytes: one config for every row.
    // `load_config` has refused bytes that do not read or are not UTF-8
    // (exit 2 above), so these are its checked text.
    let Ok(Ok(text)) = config.bytes.as_deref().map(std::str::from_utf8) else {
        return Err(CliError::spec(format!(
            "{label}: the config cannot be used"
        )));
    };
    let root = ProjectRoot {
        root: located.root.clone(),
        config_label: label.to_owned(),
        config: project,
    };
    // Row 6 over the handle rows 4-5 read: one refresh of each index.
    let (code_rows, share) = code_rows(env, &root, text);
    let drift = drift_row(&root, text, &request.git, share);
    Ok(HealthOutcome {
        documents: input.files.len(),
        amendments,
        messages,
        code_rows,
        drift,
        orphans,
    })
}

/// Row 8 over `graph` (task spec `usage-orphans` UO4): every live document
/// judged, the orphans in row order, their tasks read when one is listed.
fn orphans_of(env: &Env, graph: &SpecGraph<'_>, slug: Result<&str, String>) -> HealthOrphans {
    let usage = usage_of(graph);
    let mut live = 0;
    let mut found: Vec<(NodeAt, HealthOrphan)> = Vec::new();
    for at in graph.documents() {
        let Some(orphan) = usage.get(&at).and_then(|uses| uses.orphan) else {
            continue;
        };
        live += 1;
        if !orphan {
            continue;
        }
        let node = graph.node(at);
        found.push((
            at,
            HealthOrphan {
                name: graph.name(at),
                id: node.and_then(|node| node.id.clone()),
                kind: node.and_then(|node| node.kind.clone()),
                title: node.and_then(|node| node.title.clone()),
                path: graph.paths()[at.file].to_owned(),
                line: graph.line(at),
                status: node
                    .and_then(|node| node.fields.as_ref())
                    .and_then(|fields| fields.status.clone()),
                tasks: None,
                tasks_open: None,
            },
        ));
    }
    found.sort_by(|(_, a), (_, b)| {
        (a.kind.is_none(), &a.kind, &a.path).cmp(&(b.kind.is_none(), &b.kind, &b.path))
    });
    // Sorted, each once, on one line: one queue gives one list whatever
    // order its rows were read in.
    let mut notes = BTreeSet::new();
    if !found.is_empty() {
        let nodes: Vec<NodeAt> = found.iter().map(|(at, _)| *at).collect();
        match orphan_work(env, graph, slug, &nodes) {
            Ok((work, read_notes)) => {
                for ((_, row), work) in found.iter_mut().zip(work) {
                    row.tasks = Some(work.cards);
                    row.tasks_open = Some(work.open);
                }
                notes.extend(read_notes.iter().map(|note| one_line(note)));
            }
            Err(note) => {
                notes.insert(one_line(&note));
            }
        }
    }
    HealthOrphans {
        live,
        rows: found.into_iter().map(|(_, row)| row).collect(),
        notes: notes.into_iter().collect(),
    }
}

/// `spec trace`'s count of each node's cards ([`node_work`]), with the card
/// read's own notes (a row that does not read, an accept read as
/// unsettled), each `orphans: `-prefixed; the counts read no stored norm,
/// so none is passed and the marks' notes are dropped. `Err`: the note of
/// a queue not read.
fn orphan_work(
    env: &Env,
    graph: &SpecGraph<'_>,
    slug: Result<&str, String>,
    nodes: &[NodeAt],
) -> Result<(Vec<NodeWork>, Vec<String>), String> {
    let not_read = |cause: &str| {
        let cause = cause.strip_prefix("spec: ").unwrap_or(cause);
        format!("orphans: tasks not read: {cause}")
    };
    let slug = slug.map_err(|cause| not_read(&cause))?;
    let read = queue_read(env, slug).map_err(|error| not_read(&error.message))?;
    if read.is_unborn() {
        return Err(ORPHANS_NO_QUEUE.to_owned());
    }
    let mut dropped = Vec::new();
    let work = node_work(graph, &read, &[], nodes, &mut dropped);
    let notes = read
        .notes
        .iter()
        .map(|note| format!("orphans: {note}"))
        .collect();
    Ok((work, notes))
}

/// Row 8's block: the count line, then one line per orphan, escaped.
fn orphan_lines(orphans: &HealthOrphans, out: &mut String) {
    out.push_str(&format!(
        "orphans: {} of {} live documents link to nothing\n",
        orphans.rows.len(),
        orphans.live
    ));
    let held = |text: Option<&String>| text.map_or_else(|| "-".to_owned(), |text| one_line(text));
    for row in &orphans.rows {
        let mut line = format!(
            "  {} | {} | {} | {}:{}",
            one_line(&row.name),
            held(row.kind.as_ref()),
            held(row.title.as_ref()),
            one_line(&row.path),
            row.line
        );
        if let (Some(tasks), Some(open)) = (row.tasks, row.tasks_open)
            && tasks > 0
        {
            line.push_str(&format!(" | tasks {tasks}, open {open}"));
        }
        out.push_str(&escape_controls(&line));
        out.push('\n');
    }
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

/// Per amendment its header line and its links' lines, rows 4-6's rows,
/// their summaries, `code:` when read, then the count line, always.
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
    row_lines(&outcome.code_rows, &mut out);
    drift_lines(&outcome.drift, &mut out);
    summary_lines(&outcome.code_rows, &mut out);
    drift_summary(&outcome.drift, &mut out);
    code_line(&outcome.code_rows, &mut out);
    orphan_lines(&outcome.orphans, &mut out);
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
    code: Option<CodeCount>,
    assumes: &'a CodeRow<AssumesCounts, AssumesRow>,
    unbound: &'a CodeRow<UnboundCounts, UnboundRow>,
    drift: &'a HealthDrift,
    orphans: &'a HealthOrphans,
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
        code: outcome.code_rows.code,
        assumes: &outcome.code_rows.assumes,
        unbound: &outcome.code_rows.unbound,
        drift: &outcome.drift,
        orphans: &outcome.orphans,
    }
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`, empty `[]`.
impl Serialize for HealthOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        view(self).serialize(serializer)
    }
}
