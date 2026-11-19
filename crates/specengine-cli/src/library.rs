//! `spec library [--archive]` (task spec `doc-library`, "`spec library`"):
//! every document of the project as a library, one row per file holding a
//! document, with its front-matter facets, its uses, the `[[module]]`
//! tables holding it, the decisions whose `canon:` lands on it and its
//! file's last change; computed per read, never stored. It moves, refuses
//! and gates nothing, and judges no status: "behind its decision" is a
//! hint, never a finding, and nothing is blocked by it.
//!
//! Two reads: the index after its update (one update, the spec graph built
//! once over its decoded input) and the config's `[[module]]` tables,
//! joined by core's [`library`](specengine_core::library::library); then
//! git once, as `spec catalog` reads it ([`git_read`]), for the rows' files
//! and their decisions' files together, through the daemon's one
//! [`LastChangeCache`] when one is given. No queue and no trace: on a hub
//! the read trusts the index (`run_read`). It prints no file, so no file is
//! read again from the working tree: a file changed beside the index (a
//! commit made in a hub's checkout) is served as indexed until the trust's
//! next check, and never doubts it, as `spec tree`'s re-read would.
//!
//! ```text
//! library | documents <n>, archived <a> | decisions <d> | behind <b> | orphans <o>[ [archive]]
//! <name> | <class or -> | <kind or -> | <status or -> | version <rev or -> | tier <t or -> | owner <o or -> | reviewed <r or -> | tags <t, … or -> | used by <u>, mentioned by <m>[, orphan] | <at or -> | <by_name or -> | decisions <k> accepted, <s> superseded, <r> rejected, <x> other[ | behind] | <title or ->
//!     decision <name> | <group> | <date or -> | <at or -> | <section or ->[ | newer]
//! [truncated: <k> of <n> rows not shown; the daemon's library is whole]
//! ```
//!
//! The counts are over the rows shown. Every value is on one line, every
//! line's control characters escaped; the JSON holds the values raw. The
//! header prints, then the rows, each whole with its decision lines, while
//! they fit in [`crate::OUTPUT_CAP_CHARS`] (the first always); the JSON
//! holds the rows the text prints ([`library_with_view`] with the daemon's
//! [`View::Browser`]: nothing cut, `truncated` false, `omitted` `null`).
//!
//! The notes go to stderr as `note:` lines and into the JSON's `notes`,
//! sorted, each once: `spec catalog`'s git notes worded `library:`, scoped
//! to the rows shown and their decisions, and a module root naming
//! nothing, or the `@rev` of a module root ignored. The input's warnings
//! and the parents' (a `parent:` cycle, a parent's several holders, each
//! holder named by its path and name: the index holds no line) follow on
//! stderr alone.
//! None changes the exit. Exit 2: no project, a config that cannot be
//! used, an index that cannot be read (a newer build's).

use std::collections::BTreeSet;

use serde::Serialize;
use specengine_core::catalog::LastChange;
use specengine_core::check::{NodeAt, Parent, SpecGraph};
use specengine_core::library::{
    DecisionGroup, LibraryDecision, LibraryRow, behind, library as shelf, newer,
};
use specengine_core::map::MapNaming;
use specengine_core::workspace_toml::WorkspaceConfig;

use crate::cap::{View, lines_within};
use crate::catalog::{GitRead, LastChangeCache, git_read};
use crate::corpus::indexed;
use crate::project::discover_with_text;
use crate::tree::parent_warning;
use crate::{CliError, Env, Globals, Message, escape_controls, one_line, process_git};

/// What a text line shows for no value.
const ABSENT: &str = "-";

/// The word a git note opens with.
const WORD: &str = "library";

/// `spec library` options.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LibraryRequest {
    /// `--archive`: Tier 3 documents too.
    pub archive: bool,
}

/// One decision under a row, with its file's last change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryEntry {
    /// Core's: name, ID, title, path, status, group, date and section.
    pub decision: LibraryDecision,
    /// Its file's newest commit on `HEAD`'s first-parent line.
    pub last_change: Option<LastChange>,
    /// Core's `newer`: its commit later than its document's; `None` unless
    /// both are read.
    pub newer: Option<bool>,
}

/// One document, with its file's last change and its decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryDocument {
    /// Core's row; its `decisions` are [`Self::decisions`].
    pub row: LibraryRow,
    pub last_change: Option<LastChange>,
    pub decisions: Vec<LibraryEntry>,
    /// Core's `behind`.
    pub behind: Option<bool>,
}

/// What `spec library` read and how much of it prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryOutcome {
    pub archive: bool,
    /// Sorted, each once.
    pub notes: Vec<String>,
    /// The input's and the tree's, in the order met, each once: stderr
    /// alone.
    pub warnings: Vec<String>,
    /// Every row, by path.
    pub rows: Vec<LibraryDocument>,
    /// How many rows print, the first ones.
    pub shown: usize,
}

/// The counts over the rows shown, in this order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct LibraryCounts {
    pub documents: usize,
    pub archived: usize,
    pub decisions: usize,
    pub behind: usize,
    pub orphans: usize,
}

impl LibraryCounts {
    fn of(rows: &[LibraryDocument]) -> Self {
        let mut counts = Self::default();
        for row in rows {
            counts.documents += 1;
            counts.archived += usize::from(row.row.archived);
            counts.decisions += row.decisions.len();
            counts.behind += usize::from(row.behind == Some(true));
            counts.orphans += usize::from(row.row.orphan == Some(true));
        }
        counts
    }
}

impl LibraryOutcome {
    /// A row does not print.
    pub fn truncated(&self) -> bool {
        self.shown < self.rows.len()
    }

    /// The rows that print.
    fn shown_rows(&self) -> &[LibraryDocument] {
        &self.rows[..self.shown.min(self.rows.len())]
    }

    /// The counts over the rows that print.
    pub fn counts(&self) -> LibraryCounts {
        LibraryCounts::of(self.shown_rows())
    }

    /// One `note:` line per note, then one `warning:` line per warning,
    /// escaped.
    pub(crate) fn stderr_lines(&self) -> Vec<String> {
        let notes = self
            .notes
            .iter()
            .map(|note| Message::Note(note.clone()).line());
        let warnings = self
            .warnings
            .iter()
            .map(|warning| Message::Warning(warning.clone()).line());
        notes
            .chain(warnings)
            .map(|line| escape_controls(&line))
            .collect()
    }
}

/// `spec library`, cut at the output cap; no cache.
pub fn library(
    env: &Env,
    globals: &Globals,
    request: &LibraryRequest,
) -> Result<LibraryOutcome, CliError> {
    library_with_view(env, globals, request, View::Capped, None)
}

/// [`library`], its rows bounded by `view` (the daemon's [`View::Browser`]:
/// every row, `truncated` false, `omitted` `null`), the last changes taken
/// from `cache` where it holds them at this `HEAD` and kept there.
pub fn library_with_view(
    env: &Env,
    globals: &Globals,
    request: &LibraryRequest,
    view: View,
    cache: Option<&LastChangeCache>,
) -> Result<LibraryOutcome, CliError> {
    let (project, text) = discover_with_text(env, globals)?;
    project.slug()?;
    let mut messages = Vec::new();
    // No bytes: no line or written form is served, and a re-read file that
    // differs from its row (a commit made in a hub's checkout) would doubt
    // the hub's trust, so each such commit would cost the next page a walk.
    let input = indexed(env, &project, &mut messages, false)?;
    let graph = SpecGraph::new(&input, &project.config.scheme, &project.config.paths);
    let naming = MapNaming {
        scheme: &project.config.scheme,
        features: &project.config.paths.features,
    };
    let workspace = WorkspaceConfig::from_toml(&text);
    let read = shelf(
        &graph,
        &naming,
        &workspace,
        specengine_core::library::LibraryRequest {
            archive: request.archive,
        },
    );
    for row in &read.rows {
        messages.extend(parent_warning_unlined(&graph, row.at));
    }

    // Git once, for every row's file and every decision's.
    let paths: BTreeSet<String> = read
        .rows
        .iter()
        .flat_map(|row| {
            std::iter::once(row.path.clone())
                .chain(row.decisions.iter().map(|decision| decision.path.clone()))
        })
        .collect();
    let git = if paths.is_empty() {
        GitRead::default()
    } else {
        git_read(&project, &process_git(env), &paths, cache)
    };
    let rows: Vec<LibraryDocument> = read.rows.into_iter().map(|row| joined(row, &git)).collect();

    let shown = match view {
        View::Capped => fitting(request.archive, &rows),
        View::Browser => rows.len(),
    };
    let mut notes: BTreeSet<String> = read.notes.iter().map(|note| one_line(note)).collect();
    // In the order met, each once: the set for membership, the list for
    // order.
    let mut warnings: Vec<String> = Vec::new();
    let mut met: BTreeSet<String> = BTreeSet::new();
    for message in messages {
        match message {
            Message::Note(text) => {
                notes.insert(one_line(&text));
            }
            Message::Warning(text) => {
                let text = one_line(&text);
                if met.insert(text.clone()) {
                    warnings.push(text);
                }
            }
        }
    }
    if shown > 0 {
        let shown_paths = rows[..shown].iter().flat_map(|row| {
            std::iter::once(row.row.path.as_str()).chain(
                row.decisions
                    .iter()
                    .map(|entry| entry.decision.path.as_str()),
            )
        });
        notes.extend(git.notes_of(WORD, shown_paths));
    }
    Ok(LibraryOutcome {
        archive: request.archive,
        notes: notes.into_iter().collect(),
        warnings,
        rows,
        shown,
    })
}

/// [`parent_warning`] over an input with no bytes: a cycle as `spec tree`
/// words it (it names no line); a parent's several holders each by its path
/// and its name (its ID, else its path), never a line, which the index does
/// not hold.
fn parent_warning_unlined(graph: &SpecGraph<'_>, at: NodeAt) -> Option<Message> {
    match graph.parent(at)? {
        Parent::Node { node, others } if !others.is_empty() => {
            let holders: Vec<String> = std::iter::once(node)
                .chain(others)
                .map(|holder| {
                    let path = graph.paths()[holder.file];
                    let name = graph.name(holder);
                    if name == path {
                        path.to_owned()
                    } else {
                        format!("{path} ({name})")
                    }
                })
                .collect();
            Some(Message::Warning(format!(
                "the parent of `{}` has {} holders; listed under the first: {}",
                graph.name(at),
                holders.len(),
                holders.join(", ")
            )))
        }
        Parent::Cycle { .. } => parent_warning(graph, at),
        _ => None,
    }
}

/// A core row joined with its file's last change, each decision's, their
/// `newer` and the row's `behind`.
fn joined(mut row: LibraryRow, git: &GitRead) -> LibraryDocument {
    let last_change = git.change(&row.path).cloned();
    let decisions: Vec<LibraryEntry> = std::mem::take(&mut row.decisions)
        .into_iter()
        .map(|decision| {
            let change = git.change(&decision.path).cloned();
            let newer = newer(
                change.as_ref().map(|change| change.at.as_str()),
                last_change.as_ref().map(|change| change.at.as_str()),
            );
            LibraryEntry {
                decision,
                last_change: change,
                newer,
            }
        })
        .collect();
    let behind = behind(
        decisions
            .iter()
            .map(|entry| (entry.decision.group, entry.newer)),
    );
    LibraryDocument {
        row,
        last_change,
        decisions,
        behind,
    }
}

// -------------------------------------------------------------- the text

/// The header line over `counts`, no line end, escaped.
fn header(counts: LibraryCounts, archive: bool) -> String {
    let mut line = format!(
        "library | documents {}, archived {} | decisions {} | behind {} | orphans {}",
        counts.documents, counts.archived, counts.decisions, counts.behind, counts.orphans
    );
    if archive {
        line.push_str(" [archive]");
    }
    escape_controls(&line)
}

/// A value on one line, or [`ABSENT`].
fn held(text: Option<&str>) -> String {
    text.map_or_else(|| ABSENT.to_owned(), one_line)
}

/// A row's line and its decisions' lines, no line ends, escaped.
fn row_lines(document: &LibraryDocument) -> Vec<String> {
    let row = &document.row;
    let number = |value: Option<String>| value.unwrap_or_else(|| ABSENT.to_owned());
    let tags = if row.tags.is_empty() {
        ABSENT.to_owned()
    } else {
        row.tags
            .iter()
            .map(|tag| one_line(tag))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let orphan = if row.orphan == Some(true) {
        ", orphan"
    } else {
        ""
    };
    let (at, by) = match &document.last_change {
        Some(change) => (one_line(&change.at), one_line(&change.by_name)),
        None => (ABSENT.to_owned(), ABSENT.to_owned()),
    };
    let in_group = |group: DecisionGroup| {
        document
            .decisions
            .iter()
            .filter(|entry| entry.decision.group == group)
            .count()
    };
    let behind = if document.behind == Some(true) {
        " | behind"
    } else {
        ""
    };
    let mut lines = vec![escape_controls(&format!(
        "{} | {} | {} | {} | version {} | tier {} | owner {} | reviewed {} | tags {tags} | used by \
         {}, mentioned by {}{orphan} | {at} | {by} | decisions {} accepted, {} superseded, {} \
         rejected, {} other{behind} | {}",
        one_line(&row.name),
        held(row.class.as_deref()),
        held(row.kind.as_deref()),
        held(row.status.as_deref()),
        number(row.rev.map(|rev| rev.to_string())),
        number(row.tier.map(|tier| tier.to_string())),
        held(row.owner.as_deref()),
        held(row.reviewed.as_deref()),
        row.used_by,
        row.mentioned_by,
        in_group(DecisionGroup::Accepted),
        in_group(DecisionGroup::Superseded),
        in_group(DecisionGroup::Rejected),
        in_group(DecisionGroup::Other),
        held(row.title.as_deref()),
    ))];
    for entry in &document.decisions {
        let decision = &entry.decision;
        let newer = if entry.newer == Some(true) {
            " | newer"
        } else {
            ""
        };
        lines.push(escape_controls(&format!(
            "    decision {} | {} | {} | {} | {}{newer}",
            one_line(&decision.name),
            decision.group.as_str(),
            held(decision.date.as_deref()),
            held(entry.last_change.as_ref().map(|change| change.at.as_str())),
            held(decision.section.as_deref()),
        )));
    }
    lines
}

/// How many rows print within [`crate::OUTPUT_CAP_CHARS`] after the header,
/// each whole with its decision lines, the first always. The header is
/// costed over every row: a count over fewer rows is never longer.
fn fitting(archive: bool, rows: &[LibraryDocument]) -> usize {
    let head = header(LibraryCounts::of(rows), archive).chars().count() + 1;
    let costs = rows.iter().map(|row| {
        row_lines(row)
            .iter()
            .map(|line| line.chars().count() + 1)
            .sum()
    });
    lines_within(costs, head, true).0
}

/// The header, the rows that print with their decisions, then the tail
/// when one does not.
pub(crate) fn render_text(outcome: &LibraryOutcome) -> String {
    let mut out = header(outcome.counts(), outcome.archive);
    out.push('\n');
    for row in outcome.shown_rows() {
        for line in row_lines(row) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    if outcome.truncated() {
        out.push_str(&format!(
            "[truncated: {} of {} rows not shown; the daemon's library is whole]\n",
            outcome.rows.len() - outcome.shown,
            outcome.rows.len()
        ));
    }
    out
}

// -------------------------------------------------------------- the JSON

/// The document's 6 keys, in this order.
#[derive(Serialize)]
struct LibraryJson<'a> {
    archive: bool,
    notes: &'a [String],
    counts: LibraryCounts,
    rows: Vec<RowJson<'a>>,
    truncated: bool,
    /// The rows cut; `null` uncut.
    omitted: Option<usize>,
}

/// A row's 22 keys, in this order.
#[derive(Serialize)]
struct RowJson<'a> {
    name: &'a str,
    id: Option<&'a str>,
    path: &'a str,
    title: Option<&'a str>,
    class: Option<&'a str>,
    kind: Option<&'a str>,
    status: Option<&'a str>,
    archived: bool,
    tier: Option<i64>,
    owner: Option<&'a str>,
    reviewed: Option<&'a str>,
    scope: Option<&'a [String]>,
    tags: &'a [String],
    rev: Option<u32>,
    parent: Option<&'a str>,
    modules: &'a [String],
    used_by: u32,
    mentioned_by: u32,
    orphan: Option<bool>,
    last_change: Option<ChangeJson<'a>>,
    decisions: Vec<DecisionJson<'a>>,
    behind: Option<bool>,
}

/// A decision's 10 keys, in this order.
#[derive(Serialize)]
struct DecisionJson<'a> {
    name: &'a str,
    id: Option<&'a str>,
    title: Option<&'a str>,
    path: &'a str,
    status: Option<&'a str>,
    group: &'a str,
    date: Option<&'a str>,
    section: Option<&'a str>,
    last_change: Option<ChangeJson<'a>>,
    newer: Option<bool>,
}

/// A last change's 5 keys, `spec catalog`'s without `of_file`.
#[derive(Serialize)]
struct ChangeJson<'a> {
    at: &'a str,
    by: &'a str,
    by_name: &'a str,
    commit: &'a str,
    proposal: Option<&'a str>,
}

impl<'a> ChangeJson<'a> {
    fn of(change: Option<&'a LastChange>) -> Option<Self> {
        change.map(|change| Self {
            at: &change.at,
            by: &change.by,
            by_name: &change.by_name,
            commit: &change.commit,
            proposal: change.proposal.as_deref(),
        })
    }
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`.
impl Serialize for LibraryOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let truncated = self.truncated();
        LibraryJson {
            archive: self.archive,
            notes: &self.notes,
            counts: self.counts(),
            rows: self
                .shown_rows()
                .iter()
                .map(|document| {
                    let row = &document.row;
                    RowJson {
                        name: &row.name,
                        id: row.id.as_deref(),
                        path: &row.path,
                        title: row.title.as_deref(),
                        class: row.class.as_deref(),
                        kind: row.kind.as_deref(),
                        status: row.status.as_deref(),
                        archived: row.archived,
                        tier: row.tier,
                        owner: row.owner.as_deref(),
                        reviewed: row.reviewed.as_deref(),
                        scope: row.scope.as_deref(),
                        tags: &row.tags,
                        rev: row.rev,
                        parent: row.parent.as_deref(),
                        modules: &row.modules,
                        used_by: row.used_by,
                        mentioned_by: row.mentioned_by,
                        orphan: row.orphan,
                        last_change: ChangeJson::of(document.last_change.as_ref()),
                        decisions: document
                            .decisions
                            .iter()
                            .map(|entry| {
                                let decision = &entry.decision;
                                DecisionJson {
                                    name: &decision.name,
                                    id: decision.id.as_deref(),
                                    title: decision.title.as_deref(),
                                    path: &decision.path,
                                    status: decision.status.as_deref(),
                                    group: decision.group.as_str(),
                                    date: decision.date.as_deref(),
                                    section: decision.section.as_deref(),
                                    last_change: ChangeJson::of(entry.last_change.as_ref()),
                                    newer: entry.newer,
                                }
                            })
                            .collect(),
                        behind: document.behind,
                    }
                })
                .collect(),
            truncated,
            omitted: truncated.then(|| self.rows.len() - self.shown),
        }
        .serialize(serializer)
    }
}
