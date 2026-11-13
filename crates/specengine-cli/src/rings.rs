//! `spec rings [--json]` (task spec `scope-rings` SR2 to SR4): the
//! project's nodes on the scope rings its `[[map.ring]]` tables configure,
//! innermost first, each sector filled with its node's work state. Computed
//! per read and never stored; it moves, refuses and gates nothing.
//!
//! One call: the config read once ([`MapConfig`]); with a ring, the index
//! refreshed and read once, then, per ring in order, the lines `spec tree
//! [<root>] [--kind <k>]…` lists for its root and kinds, live files only,
//! less the nodes an inner ring holds (core's [`ring_members`]); then the
//! trace's reading of every sector, built once as `spec catalog` builds it
//! ([`node_work`]). A root that names no node, or is refused as a REF,
//! leaves its ring empty with a note naming the ring and `spec tree ROOT`'s
//! reason ([`ring_note`]; [`unresolved_root`] for a root located at no
//! node), never an exit. With no ring, nothing past the config is read.
//!
//! ```text
//! rings <n> | sectors <m>
//! ring <i> | <title>[ | root <REF>][ | kinds <a, b>] | sectors <k>
//!   <name> | <kind or -> | <state> | <title or -> | <path>:<line>[ | status <s>]
//! [truncated: <k> of <n> sectors not shown]
//! ```
//!
//! Every value is on one line, every line's control characters escaped;
//! the JSON holds the values raw. The header prints, then each ring's line
//! and its sectors' lines while they fit in [`crate::OUTPUT_CAP_CHARS`]
//! (the first always): rings are whole until the cut, which falls at a
//! line. The JSON holds what the text prints ([`rings_with_view`] with the
//! daemon's [`View::Browser`]: nothing cut).
//!
//! The notes (the config's, named `<config>:<line>:`, a root's, the
//! index's, the tree's and the trace's) go to stderr as `note:` lines and
//! into the JSON's `notes`, sorted, each once; the tree's warnings follow
//! them on stderr alone. None changes the exit. Exit 2: no project, a
//! config `ProjectConfig` refuses, an index or a queue that cannot be read.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use specengine_core::check::{NodeAt, SpecGraph};
use specengine_core::rings::{
    Ring, RingSector, RingsDocument, ring_members, ring_note, unresolved_root,
};
use specengine_core::trace::Rollup;
use specengine_core::{MapConfig, RingConfig};

use crate::cap::{View, lines_within};
use crate::corpus::{Admission, LeftOut, holders_warning, indexed_in, locate};
use crate::location::open_index;
use crate::project::discover_with_text;
use crate::show::classify;
use crate::trace::{node_work, queue_read};
use crate::tree::{TreeNode, direction_lines, lines_of};
use crate::{CliError, Env, Globals, Message, escape_controls, one_line, store_error};

/// What a text line shows for no kind or no title.
const ABSENT: &str = "-";

/// What `@rev` on a ring's root is ignored for.
const IGNORED_REV: &str = "`spec rings` reads the current files";

/// What a warning on a root with several holders says is taken.
const HOLDERS_TAKEN: &str = "each holder's subtree in the ring";

/// What a command's error opens with ([`CliError`]'s `spec`), dropped from a
/// root's refusal: the note is no command's error.
const COMMAND_PREFIX: &str = "spec: ";

/// What `spec rings` read and how much of it prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingsOutcome {
    /// Every ring whole, innermost first; the notes sorted, each once.
    pub document: RingsDocument,
    /// The tree's, in the order met, each once: stderr alone.
    pub warnings: Vec<String>,
    /// How many lines after the header print, the first ones: each ring's
    /// line, then its sectors' lines.
    pub shown: usize,
}

impl RingsOutcome {
    /// Every sector of every ring.
    pub fn sectors(&self) -> usize {
        self.document
            .rings
            .iter()
            .map(|ring| ring.sectors.len())
            .sum()
    }

    /// Every line after the header: a ring's and its sectors'.
    fn lines(&self) -> usize {
        self.document.rings.len() + self.sectors()
    }

    /// A line does not print.
    pub fn truncated(&self) -> bool {
        self.shown < self.lines()
    }

    /// The rings that print, each with the sectors that print.
    fn shown_rings(&self) -> Vec<(&Ring, &[RingSector])> {
        let mut left = self.shown;
        let mut out = Vec::new();
        for ring in &self.document.rings {
            if left == 0 {
                break;
            }
            left -= 1;
            let sectors = left.min(ring.sectors.len());
            left -= sectors;
            out.push((ring, &ring.sectors[..sectors]));
        }
        out
    }

    /// One `note:` line per note, then one `warning:` line per warning,
    /// escaped.
    pub(crate) fn stderr_lines(&self) -> Vec<String> {
        let notes = self
            .document
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

/// The JSON document: core's [`RingsDocument`] as the text cuts it.
#[derive(Serialize)]
struct RingsJson<'a> {
    notes: &'a [String],
    rings: Vec<RingJson<'a>>,
}

/// A ring's 4 keys, its sectors those that print.
#[derive(Serialize)]
struct RingJson<'a> {
    title: &'a str,
    root: Option<&'a str>,
    kinds: Option<&'a [String]>,
    sectors: &'a [RingSector],
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`.
impl Serialize for RingsOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        RingsJson {
            notes: &self.document.notes,
            rings: self
                .shown_rings()
                .into_iter()
                .map(|(ring, sectors)| RingJson {
                    title: &ring.title,
                    root: ring.root.as_deref(),
                    kinds: ring.kinds.as_deref(),
                    sectors,
                })
                .collect(),
        }
        .serialize(serializer)
    }
}

/// `spec rings`, cut at the output cap.
pub fn rings(env: &Env, globals: &Globals) -> Result<RingsOutcome, CliError> {
    rings_with_view(env, globals, View::Capped)
}

/// [`rings`], its lines bounded by `view` (the daemon's [`View::Browser`]:
/// every ring and sector).
pub fn rings_with_view(env: &Env, globals: &Globals, view: View) -> Result<RingsOutcome, CliError> {
    let (project, text) = discover_with_text(env, globals)?;
    let slug = project.slug()?.to_owned();
    let config = MapConfig::from_toml(&text);
    let mut messages: Vec<Message> = config
        .notes_at(&project.config_label)
        .into_iter()
        .map(Message::Note)
        .collect();
    let mut rings = Vec::with_capacity(config.rings.len());
    if !config.rings.is_empty() {
        let mut open = open_index(env, &project)?;
        let input = indexed_in(&mut open.index, &project, &mut messages, true)?;
        let graph = SpecGraph::new(&input, &project.config.scheme, &project.config.paths);
        let mut lines: BTreeMap<NodeAt, TreeNode> = BTreeMap::new();
        let listed: Vec<Vec<NodeAt>> = config
            .rings
            .iter()
            .enumerate()
            .map(|(index, ring)| {
                ring_lines(
                    &graph,
                    &project.config.paths.spec,
                    index,
                    ring,
                    &mut messages,
                )
                .into_iter()
                .map(|(at, line)| {
                    lines.entry(at).or_insert(line);
                    at
                })
                .collect()
            })
            .collect();
        let members = ring_members(listed);
        let nodes: Vec<NodeAt> = members.iter().flatten().copied().collect();
        let read = queue_read(env, &slug)?;
        let stored = open.index.node_norms().map_err(store_error)?;
        let work = node_work(&graph, &read, &stored, &nodes, &mut messages);
        // `work` is in `nodes`' order, one per node: each ring takes the
        // slice as long as its members, so a member pairs with its own.
        let mut rest = work.as_slice();
        for (ring, members) in config.rings.into_iter().zip(members) {
            let (own, after) = rest.split_at(members.len().min(rest.len()));
            rest = after;
            let sectors = members
                .into_iter()
                .zip(own)
                .filter_map(|(at, work)| lines.get(&at).map(|line| sector_of(line, work.rollup)))
                .collect();
            rings.push(Ring {
                title: ring.title,
                root: ring.root,
                kinds: ring.kinds,
                sectors,
            });
        }
    }
    let mut notes: BTreeSet<String> = BTreeSet::new();
    let mut warnings: Vec<String> = Vec::new();
    for message in messages {
        match message {
            Message::Note(text) => {
                notes.insert(one_line(&text));
            }
            Message::Warning(text) => {
                let text = one_line(&text);
                if !warnings.contains(&text) {
                    warnings.push(text);
                }
            }
        }
    }
    let mut outcome = RingsOutcome {
        document: RingsDocument {
            notes: notes.into_iter().collect(),
            rings,
        },
        warnings,
        shown: 0,
    };
    outcome.shown = match view {
        View::Capped => fitting(&outcome),
        View::Browser => outcome.lines(),
    };
    Ok(outcome)
}

/// Ring `index`'s lines, each with its node: `spec tree [<root>] [--kind
/// <k>]…`'s, live files only (task spec `scope-rings` SR2). A root that
/// names no node, or is refused as a REF, gives none and a note naming the
/// ring with the resolver's reason, as `spec tree ROOT` words it (a
/// look-alike's Latin fix among them: IDs are Latin only, task spec
/// `scope-rings` SR4); a root located at no node,
/// core's [`unresolved_root`] (SR4). The resolver's own notes (an ignored
/// `@rev`) name the ring too.
fn ring_lines(
    graph: &SpecGraph<'_>,
    spec: &str,
    index: usize,
    ring: &RingConfig,
    messages: &mut Vec<Message>,
) -> Vec<(NodeAt, TreeNode)> {
    let kinds: &[String] = ring.kinds.as_deref().unwrap_or_default();
    let Some(written) = ring.root.as_deref() else {
        return direction_lines(graph, spec, kinds, false, messages);
    };
    let mut resolved = Vec::new();
    let roots = root_nodes(graph, written, &mut resolved);
    messages.extend(resolved.into_iter().map(|message| match message {
        Message::Note(text) => Message::Note(ring_note(index + 1, &ring.title, &text)),
        warning @ Message::Warning(_) => warning,
    }));
    let roots = match roots {
        Ok(roots) if !roots.is_empty() => roots,
        unresolved => {
            let note = match unresolved {
                Err(reason) => ring_note(index + 1, &ring.title, &reason),
                Ok(_) => unresolved_root(index + 1, &ring.title, written),
            };
            messages.push(Message::Note(note));
            return Vec::new();
        }
    };
    messages.extend(holders_warning(graph, written, &roots, HOLDERS_TAKEN));
    let admission = Admission::new(graph, false, roots.iter().map(|at| at.file));
    let mut left_out = LeftOut::default();
    lines_of(
        graph,
        &admission,
        &roots,
        None,
        kinds,
        &mut left_out,
        messages,
    )
}

/// The nodes `written` names, as `spec tree ROOT` locates them; else why
/// not, as `spec tree ROOT` says it: its reason for naming nothing, or its
/// refusal of the REF without the `spec: ` of a command's error.
fn root_nodes(
    graph: &SpecGraph<'_>,
    written: &str,
    messages: &mut Vec<Message>,
) -> Result<Vec<NodeAt>, String> {
    let refusal = |error: CliError| {
        error
            .message
            .strip_prefix(COMMAND_PREFIX)
            .map_or_else(|| error.message.clone(), str::to_owned)
    };
    let target = classify(written, graph.scheme(), messages, IGNORED_REV).map_err(refusal)??;
    locate(graph, &target, written).map_err(refusal)?
}

/// A sector of `line`, filled with `state`.
fn sector_of(line: &TreeNode, state: Rollup) -> RingSector {
    RingSector {
        name: line.id.clone().unwrap_or_else(|| line.path.clone()),
        id: line.id.clone(),
        kind: line.kind.clone(),
        title: line.title.clone(),
        path: line.path.clone(),
        line: line.line,
        status: line.status.clone(),
        state,
    }
}

/// The header line, no line end: counts alone.
fn header(outcome: &RingsOutcome) -> String {
    format!(
        "rings {} | sectors {}",
        outcome.document.rings.len(),
        outcome.sectors()
    )
}

/// Ring `index`'s (1-based) line, no line end, escaped.
fn ring_line(index: usize, ring: &Ring) -> String {
    let mut line = format!("ring {index} | {}", one_line(&ring.title));
    if let Some(root) = &ring.root {
        line.push_str(&format!(" | root {}", one_line(root)));
    }
    if let Some(kinds) = &ring.kinds {
        let kinds: Vec<String> = kinds.iter().map(|kind| one_line(kind)).collect();
        line.push_str(&format!(" | kinds {}", kinds.join(", ")));
    }
    line.push_str(&format!(" | sectors {}", ring.sectors.len()));
    escape_controls(&line)
}

/// A sector's line, no line end, escaped.
fn sector_line(sector: &RingSector) -> String {
    let held =
        |text: Option<&String>| text.map_or_else(|| ABSENT.to_owned(), |text| one_line(text));
    let mut line = format!(
        "  {} | {} | {} | {} | {}:{}",
        one_line(&sector.name),
        held(sector.kind.as_ref()),
        sector.state.as_str(),
        held(sector.title.as_ref()),
        one_line(&sector.path),
        sector.line
    );
    if let Some(status) = &sector.status {
        line.push_str(&format!(" | status {}", one_line(status)));
    }
    escape_controls(&line)
}

/// Every line after the header, in order.
fn body_lines(outcome: &RingsOutcome) -> impl Iterator<Item = String> + '_ {
    outcome
        .document
        .rings
        .iter()
        .enumerate()
        .flat_map(|(index, ring)| {
            std::iter::once(ring_line(index + 1, ring)).chain(ring.sectors.iter().map(sector_line))
        })
}

/// How many lines print within [`crate::OUTPUT_CAP_CHARS`] after the
/// header, each whole, the first always.
fn fitting(outcome: &RingsOutcome) -> usize {
    let head = header(outcome).chars().count() + 1;
    let costs = body_lines(outcome).map(|line| line.chars().count() + 1);
    lines_within(costs, head, true).0
}

/// The header, the lines that print, then the tail when one does not.
pub(crate) fn render_text(outcome: &RingsOutcome) -> String {
    let mut out = header(outcome);
    out.push('\n');
    for line in body_lines(outcome).take(outcome.shown) {
        out.push_str(&line);
        out.push('\n');
    }
    if outcome.truncated() {
        let shown: usize = outcome
            .shown_rings()
            .iter()
            .map(|(_, sectors)| sectors.len())
            .sum();
        let sectors = outcome.sectors();
        out.push_str(&format!(
            "[truncated: {} of {sectors} sectors not shown]\n",
            sectors - shown
        ));
    }
    out
}
