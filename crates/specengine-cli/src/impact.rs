//! `spec impact REF | --symbol QPATH [--depth N]` (canon `spec-impact`,
//! task spec `impact`): what a change of a node or of a code item reaches,
//! in one read — the nodes, their code bindings with their `sync`, the
//! tests bound to them and this repository's open tasks naming them.
//! Nothing it shows changes an exit, a status, a claim or a verdict; it
//! writes nothing under the project root (the index refreshes in the data
//! directory, the queue is read as `spec roadmap` reads it).
//!
//! - **Seeds** (distance 0): REF's holders, as `spec graph REF`'s; or the
//!   nodes bound to the code items `--symbol` names — the lookup's best
//!   non-empty tier, every item there, each reaching the nodes of the
//!   markers bound to it, to an item or inline mod enclosing it, or to its
//!   file's module (`spec unmapped`'s one containment), whatever their
//!   relation, their ID resolved, an alias or ambiguous. A mention, or an
//!   ID naming no node, seeds nothing. The lookup's hits and their files'
//!   symbols come from one snapshot with the markers.
//! - **Nodes**: `spec graph --impact`'s walk from the seeds ([`impact_reach`],
//!   the one walk), `--depth N` as graph's; `left_out` graph's.
//! - **Rows**: one judge ([`set_bindings`]) over every reached node and the
//!   sections within each; `symbols` the rows of any relation but
//!   `verifies`, `tests` the `verifies` rows, each in verify's row order.
//!   The REF form degrades as the reads that show bindings: without
//!   `[code]` both `null`, no note; a code-layer cause one `bindings:` note,
//!   never an exit. `--symbol` takes the code as its input: no `[code]`, a
//!   bad one or an incomplete walk is exit 2.
//! - **Tasks**: this repository's open tasks whose targets and affected
//!   nodes (each resolved now, with the sections it contains) meet a
//!   reached node or a section within one, by number, each with the
//!   reached nodes it names in radius order; never an exit.
//!
//! ```text
//! [seed symbol <QPATH>: <n> at <tier>]
//! impact: nodes <n>, symbols <s or ->, tests <t or ->, tasks <k>[; code files <f> (.rs <r>, .ron <o>), skipped <s>][; left out: <g> generated, <a> archived]
//! node <distance> <name> | <kind or -> | <title or -> | <path>:<line>[ | archived]
//! symbol <row>
//! [  sig <signature>]
//! test <row>
//! task <T> | <state> | <title or -> | <node>, …
//! seed <qpath> | <item_kind> | <path>:<start>-<end>
//! [truncated: <parts> not shown; lower --depth N or narrow the REF or --symbol]
//! ```
//!
//! The cut: the seed header and the summary always print; then nodes,
//! symbol rows, test rows (each with its lines), tasks and the seed
//! matches, each whole, while the text stays within
//! [`crate::OUTPUT_CAP_CHARS`] (the first item whole whatever its size);
//! the first item that does not fit and every later one are not shown.
//! The JSON holds exactly the printed items; `truncated` is `null` uncut.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use specengine_code::query_name;
use specengine_core::CodeConfig;
use specengine_core::check::{CheckInput, NodeAt, SpecGraph};
use specengine_core::lock::LockRelation;
use specengine_model::{BindingRow, TaskStatus};
use specengine_store::{CodeSnapshot, GitEnv, SnapshotSymbols, SqliteIndex, StoredSymbol};

use crate::bindings::{SetRow, set_bindings};
use crate::cap::{lines_within, row_lines};
use crate::code_index::refresh_code;
use crate::corpus::{LeftOut, depth_of, holders_warning, indexed_in, locate};
use crate::graph::{GraphNode, NodeJson, Reach, impact_reach, node_line, node_of};
use crate::location::open_index;
use crate::node_set::named_set;
use crate::project::{ProjectRoot, discover_with_text};
use crate::refs::{CodeCount, Corpus, NodeKey, code_config, stored_count};
use crate::roadmap::{GitPlace, open_tasks, others_note};
use crate::search::notes;
use crate::show::{Target, classify};
use crate::symbols::maps;
use crate::unmapped::{Seeds, reached};
use crate::{CliError, Env, Globals, Message, escape_controls, one_line, store_error};

/// Why a REF's `@rev` is ignored.
const REV_IGNORED: &str = "`spec impact` reads the current files";

/// The command the queue's other-repository note names.
const COMMAND: &str = "spec impact";

/// `spec impact` options: exactly one of `reference` and `symbol`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactRequest {
    /// REF as given: any form `spec show` takes.
    pub reference: Option<String>,
    /// `--symbol QPATH` as given: a qpath, a suffix of one or a name.
    pub symbol: Option<String>,
    /// `--depth N`; `None`: unbounded.
    pub depth: Option<i64>,
    /// Git's environment: the rows are judged with it and the tasks'
    /// repository found with it.
    pub git: GitEnv,
}

/// What `spec impact` reached, or why its input names nothing (exit 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactOutcome {
    /// REF as given.
    pub reference: Option<String>,
    /// `--symbol` as given.
    pub symbol: Option<String>,
    pub reason: Option<String>,
    pub messages: Vec<Message>,
    pub depth: Option<usize>,
    /// `--symbol`'s matches at the lookup's best tier, in the lookup's
    /// order; `None` for REF.
    pub seeds: Option<Vec<SeedSymbol>>,
    /// Edges of a followed type written in left-out files.
    pub left_out: LeftOut,
    /// By (distance, path, position): distance 0 the seeds.
    pub nodes: Vec<GraphNode>,
    /// The rows of any relation but `verifies`, in verify's row order;
    /// `None` without `[code]` or when no row could be read.
    pub symbols: Option<Vec<ImpactRow>>,
    /// The `verifies` rows, in verify's row order; `None` as `symbols`.
    pub tests: Option<Vec<ImpactRow>>,
    /// By number.
    pub tasks: Vec<ImpactTask>,
    /// The code files the rows' read covered (`set_bindings`'); `None`
    /// without `[code]` or under a cause that leaves no row (a bad
    /// `[code]`, a walk gap, the data directory); `--symbol`'s lookup
    /// walk's when the rows' read none.
    pub code: Option<CodeCount>,
    /// How many of each list print (text and JSON alike).
    shown: Shown,
}

/// A code item `--symbol` named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeedSymbol {
    pub qpath: String,
    pub item_kind: String,
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
    /// `exact`, `suffix` or `name`: the best tier the lookup met.
    pub tier: &'static str,
}

/// One binding row and its binding as verify prints it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactRow {
    pub row: BindingRow,
    /// `<kind> <symbol>[ <item_kind>]`, a keyless row's `<word> <category>`.
    pub binding: String,
}

/// An open task of this repository naming a reached node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImpactTask {
    pub id: String,
    pub status: TaskStatus,
    pub title: Option<String>,
    /// The reached nodes it names (ID, else path), in radius order.
    pub nodes: Vec<String>,
}

/// How many of each list print.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Shown {
    seeds: usize,
    nodes: usize,
    symbols: usize,
    tests: usize,
    tasks: usize,
}

impl ImpactOutcome {
    /// The request echoed, nothing reached yet.
    fn new(request: &ImpactRequest, depth: Option<usize>) -> Self {
        Self {
            reference: request.reference.clone(),
            symbol: request.symbol.clone(),
            reason: None,
            messages: Vec::new(),
            depth,
            seeds: request.symbol.as_ref().map(|_| Vec::new()),
            left_out: LeftOut::default(),
            nodes: Vec::new(),
            symbols: None,
            tests: None,
            tasks: Vec::new(),
            code: None,
            shown: Shown::default(),
        }
    }

    /// Exit 1: the reason on one line, the document kept.
    fn not_found(mut self, reason: &str, messages: Vec<Message>) -> Self {
        self.reason = Some(one_line(reason));
        self.messages = messages;
        self
    }

    /// Some list is not printed whole.
    pub fn truncated(&self) -> bool {
        let shown = self.shown;
        shown.seeds < self.seeds.as_ref().map_or(0, Vec::len)
            || shown.nodes < self.nodes.len()
            || shown.symbols < self.symbols.as_ref().map_or(0, Vec::len)
            || shown.tests < self.tests.as_ref().map_or(0, Vec::len)
            || shown.tasks < self.tasks.len()
    }
}

/// The one input as given.
enum Asked<'r> {
    Reference(&'r str),
    Symbol(&'r str),
}

/// The one input, read before the index: REF's target, or `--symbol`'s
/// query with the `[code]` it needs.
enum Input<'r> {
    Reference(&'r str, Target),
    Symbol(&'r str, CodeConfig),
}

/// `spec impact`: reads the input, the config and the index, seeds,
/// walks, reads the rows and the tasks, plans the cut.
pub fn impact(
    env: &Env,
    globals: &Globals,
    request: &ImpactRequest,
) -> Result<ImpactOutcome, CliError> {
    let asked = match (&request.reference, &request.symbol) {
        (Some(reference), None) => Asked::Reference(reference.trim()),
        (None, Some(symbol)) => Asked::Symbol(symbol),
        _ => {
            return Err(CliError::spec(
                "impact takes exactly one of REF and --symbol",
            ));
        }
    };
    let depth = depth_of(request.depth)?;
    let (project, text) = discover_with_text(env, globals)?;
    let slug = project.slug()?.to_owned();
    let scheme = &project.config.scheme;
    let mut messages = Vec::new();
    let mut outcome = ImpactOutcome::new(request, depth);
    // REF's form (a usage error is exit 2, naming nothing exit 1);
    // `--symbol`'s `[code]` (none or a bad one: exit 2).
    let input = match asked {
        Asked::Reference(written) => match classify(written, scheme, &mut messages, REV_IGNORED)? {
            Ok(target) => Input::Reference(written, target),
            Err(reason) => return Ok(outcome.not_found(&reason, messages)),
        },
        Asked::Symbol(query) => Input::Symbol(query, code_config(&project, &text)?),
    };

    // One handle for the spec index and the code index (as `spec bundle`),
    // the spec corpus read again whole (the judge's spec side).
    let mut open = open_index(env, &project)?;
    let read = indexed_in(&mut open.index, &project, &mut messages, true)?;
    let graph = SpecGraph::new(&read, scheme, &project.config.paths);
    let starts = match &input {
        Input::Reference(written, target) => {
            let starts = match locate(&graph, target, written)? {
                Ok(nodes) => nodes,
                Err(reason) => return Ok(outcome.not_found(&reason, messages)),
            };
            messages.extend(holders_warning(
                &graph,
                written,
                &starts,
                "all at distance 0",
            ));
            starts
        }
        Input::Symbol(query, code) => {
            let seeded = SymbolSeeds {
                project: &project,
                code,
                input: &read,
                graph: &graph,
            };
            match symbol_starts(&seeded, &mut open.index, query, &mut outcome, &mut messages)? {
                Some(starts) => starts,
                None => {
                    let reason = format!("--symbol `{query}` names no symbol");
                    return Ok(outcome.not_found(&reason, messages));
                }
            }
        }
    };

    let context = RadiusRead {
        env,
        project: &project,
        slug: &slug,
        text: &text,
        git: &request.git,
        input: &read,
        graph: &graph,
    };
    radius(
        &context,
        &mut open.index,
        &starts,
        &mut outcome,
        &mut messages,
    );
    outcome.messages = messages;
    outcome.shown = plan(&outcome);
    Ok(outcome)
}

/// What `--symbol`'s seeds read beside the code index.
struct SymbolSeeds<'a> {
    project: &'a ProjectRoot,
    code: &'a CodeConfig,
    input: &'a CheckInput,
    graph: &'a SpecGraph<'a>,
}

/// `--symbol`'s seeds: the code index refreshed (a gap: exit 2), one
/// snapshot of the markers, the lookup's hits and the best tier's files'
/// symbols; the matches into `outcome.seeds` and the files read into
/// `outcome.code`; the nodes they seed, by (path, position). `None`: no
/// match (exit 1).
fn symbol_starts(
    seeded: &SymbolSeeds<'_>,
    index: &mut SqliteIndex,
    query: &str,
    outcome: &mut ImpactOutcome,
    messages: &mut Vec<Message>,
) -> Result<Option<Vec<NodeAt>>, CliError> {
    let project = seeded.project;
    let scheme = &project.config.scheme;
    let refreshed = refresh_code(
        index,
        &project.root,
        seeded.code,
        &project.config_label,
        scheme,
        false,
    )?;
    refreshed.require_complete()?;
    messages.extend(refreshed.messages.iter().cloned());
    // One snapshot: the markers, the hits and their files' symbols from one
    // read transaction.
    let CodeSnapshot {
        refs: stored,
        hits,
        symbols,
        ..
    } = index
        .code_snapshot(SnapshotSymbols::LookupWithFiles {
            query,
            name: query_name(query),
        })
        .map_err(store_error)?
        .unwrap_or_default();
    // The hits come by tier, the best first.
    let Some(tier) = hits.first().map(|hit| hit.tier) else {
        return Ok(None);
    };
    outcome.code = Some(stored_count(&stored, refreshed.report.skipped));
    let matches: Vec<&StoredSymbol> = hits
        .iter()
        .filter(|hit| hit.tier == tier)
        .map(|hit| &hit.symbol)
        .collect();
    outcome.seeds = Some(
        matches
            .iter()
            .map(|symbol| SeedSymbol {
                qpath: symbol.qpath.clone(),
                item_kind: symbol.item_kind.clone(),
                path: symbol.path.clone(),
                start_line: symbol.start_line as usize,
                end_line: symbol.end_line as usize,
                tier: tier.as_str(),
            })
            .collect(),
    );
    let mut corpus = Corpus::new(seeded.input, scheme, &project.config.paths);
    let keys = seed_nodes(&matches, &stored.markers, &symbols, &mut corpus)?;
    let mut starts: Vec<NodeAt> = keys
        .iter()
        .filter_map(|&key| {
            let path = corpus.resolver.paths().get(key.file)?;
            seeded
                .graph
                .file_of(path)
                .map(|file| NodeAt { file, ord: key.ord })
        })
        .collect();
    starts.sort_unstable();
    starts.dedup();
    Ok(Some(starts))
}

/// The nodes `--symbol`'s matches seed: per match's file, each stored
/// marker there that maps (`spec unmapped`'s predicate) and whose place
/// reaches a match by containment; its ID's nodes. `symbols`: the
/// matches' files' symbols, by (path, ord).
fn seed_nodes(
    matches: &[&StoredSymbol],
    markers: &[specengine_store::StoredMarker],
    symbols: &[StoredSymbol],
    corpus: &mut Corpus<'_>,
) -> Result<BTreeSet<NodeKey>, CliError> {
    let mut wanted: BTreeMap<&str, BTreeSet<u32>> = BTreeMap::new();
    for symbol in matches {
        wanted
            .entry(symbol.path.as_str())
            .or_default()
            .insert(symbol.ord);
    }
    let mut items: BTreeMap<&str, Vec<&StoredSymbol>> = BTreeMap::new();
    for symbol in symbols {
        items.entry(symbol.path.as_str()).or_default().push(symbol);
    }
    // (path, the place's whole file, RON value, item) → whether that place
    // reaches a match of the file.
    let mut reaches: BTreeMap<(&str, bool, bool, Option<u32>), bool> = BTreeMap::new();
    let mut seeds = BTreeSet::new();
    for marker in markers {
        let path = marker.path.as_str();
        let Some(ords) = wanted.get(path) else {
            continue;
        };
        let judged = corpus.judge(&marker.id);
        let Some(place) = maps(marker, &judged)? else {
            continue;
        };
        let mut placed = Seeds::default();
        placed.add(place);
        let at = (
            path,
            placed.whole,
            placed.ron,
            placed.items.first().copied(),
        );
        let hit = *reaches.entry(at).or_insert_with(|| {
            let file = items.get(path).map_or(&[][..], Vec::as_slice);
            reached(file, &placed)
                .iter()
                .zip(file)
                .any(|(&reached, symbol)| reached && ords.contains(&symbol.ord))
        });
        if hit {
            seeds.extend(judged.nodes);
        }
    }
    Ok(seeds)
}

/// What the radius reads beside the seeds.
struct RadiusRead<'a> {
    env: &'a Env,
    project: &'a ProjectRoot,
    slug: &'a str,
    text: &'a str,
    git: &'a GitEnv,
    /// The spec corpus read again whole, from the handle the code index is
    /// read with.
    input: &'a CheckInput,
    /// Its graph.
    graph: &'a SpecGraph<'a>,
}

/// The walk from `starts`, the reached set's rows and tasks into
/// `outcome`, their notes into `messages`.
fn radius(
    read: &RadiusRead<'_>,
    index: &mut SqliteIndex,
    starts: &[NodeAt],
    outcome: &mut ImpactOutcome,
    messages: &mut Vec<Message>,
) {
    let project = read.project;
    let scheme = &project.config.scheme;
    let graph = read.graph;
    let Reach { walk, left_out } = impact_reach(graph, starts, outcome.depth);
    outcome.left_out = left_out;
    outcome.nodes = walk
        .nodes
        .iter()
        .map(|&(at, distance)| node_of(graph, at, distance))
        .collect();

    // Each reached node as the corpus keys it, with the sections within
    // it: the same input, by path.
    let corpus = Corpus::new(read.input, scheme, &project.config.paths);
    let files: BTreeMap<&str, usize> = corpus
        .resolver
        .paths()
        .iter()
        .enumerate()
        .map(|(file, path)| (*path, file))
        .collect();
    let key = |at: NodeAt| -> Option<NodeKey> {
        let path = graph.paths().get(at.file)?;
        files.get(path).map(|&file| NodeKey { file, ord: at.ord })
    };
    let within: Vec<(NodeAt, BTreeSet<NodeKey>)> = walk
        .nodes
        .iter()
        .map(|&(at, _)| (at, graph.within(at).into_iter().filter_map(key).collect()))
        .collect();
    let keys: Vec<NodeKey> = within
        .iter()
        .flat_map(|(_, keys)| keys.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    // Tasks: this repository's open ones naming a reached node or a
    // section within one.
    let tasks = open_tasks(read.env, project, read.slug, read.git, GitPlace::Unnamed);
    let naming = |task: &specengine_store::Task| -> Vec<String> {
        let refs: Vec<String> = task
            .targets
            .iter()
            .chain(&task.affected_nodes)
            .cloned()
            .collect();
        let named = named_set(&corpus, scheme, &refs);
        within
            .iter()
            .filter(|(_, keys)| !keys.is_disjoint(&named))
            .map(|&(at, _)| graph.name(at))
            .collect()
    };
    outcome.tasks = tasks
        .here
        .iter()
        .filter_map(|task| {
            let nodes = naming(task);
            (!nodes.is_empty()).then(|| ImpactTask {
                id: task.id.clone(),
                status: task.status,
                title: task.title.clone(),
                nodes,
            })
        })
        .collect();
    let others = tasks
        .others
        .iter()
        .filter(|task| !naming(task).is_empty())
        .count();

    // The queue's notes, then the rows' one note.
    messages.extend(tasks.notes.into_iter().map(Message::Note));
    messages.extend(others_note(others, read.slug, COMMAND).map(Message::Note));

    // Rows: one judge over the reached set; without `[code]` none, no
    // note, the code index untouched.
    if matches!(CodeConfig::from_toml(read.text), Ok(None)) {
        return;
    }
    let rows = set_bindings(project, read.text, read.git, index, read.input, &keys);
    messages.extend(rows.note.map(Message::Note));
    // `--symbol`'s lookup walk counts when the judge's read none.
    if rows.code.is_some() {
        outcome.code = rows.code;
    }
    if let Some(rows) = rows.rows {
        let (tests, symbols): (Vec<SetRow>, Vec<SetRow>) = rows.into_iter().partition(|row| {
            LockRelation::from_name(&row.row.relation) == Some(LockRelation::Verifies)
        });
        let shown = |rows: Vec<SetRow>| -> Vec<ImpactRow> {
            rows.into_iter()
                .map(|row| ImpactRow {
                    row: row.row,
                    binding: row.binding,
                })
                .collect()
        };
        outcome.symbols = Some(shown(symbols));
        outcome.tests = Some(shown(tests));
    }
}

// ------------------------------------------------------------- rendering

/// `seed symbol <QPATH>: <n> at <tier>`: `--symbol`'s header.
fn header(outcome: &ImpactOutcome) -> Option<String> {
    let query = outcome.symbol.as_deref()?;
    let seeds = outcome.seeds.as_ref()?;
    let tier = seeds.first().map_or("-", |seed| seed.tier);
    Some(format!(
        "seed symbol {}: {} at {tier}",
        one_line(query),
        seeds.len()
    ))
}

/// `seed <qpath> | <item_kind> | <path>:<start>-<end>`.
fn seed_line(seed: &SeedSymbol) -> String {
    format!(
        "seed {} | {} | {}:{}-{}",
        one_line(&seed.qpath),
        one_line(&seed.item_kind),
        one_line(&seed.path),
        seed.start_line,
        seed.end_line
    )
}

/// `node ` and `spec graph`'s node line.
fn node_text(node: &GraphNode) -> String {
    format!("node {}", node_line(node))
}

/// A row's lines: `<word> <row>`, then its `sig`, old place and candidate
/// lines, two spaces in.
fn row_text(word: &str, row: &ImpactRow) -> Vec<String> {
    row_lines(&row.row, &row.binding)
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let line = line.strip_prefix("  ").unwrap_or(&line);
            if index == 0 {
                format!("{word} {line}")
            } else {
                line.to_owned()
            }
        })
        .collect()
}

/// `task <T> | <state> | <title or -> | <node>, …`.
fn task_line(task: &ImpactTask) -> String {
    let title = task
        .title
        .as_deref()
        .map_or_else(|| "-".to_owned(), one_line);
    escape_controls(&format!(
        "task {} | {} | {title} | {}",
        one_line(&task.id),
        task.status,
        one_line(&task.nodes.join(", "))
    ))
}

/// `impact: nodes <n>, symbols <s or ->, tests <t or ->, tasks <k>[; code
/// files <f> (.rs <r>, .ron <o>), skipped <s>][; left out: <g> generated,
/// <a> archived]`, every list whole, zero left-out parts omitted.
fn summary(outcome: &ImpactOutcome) -> String {
    let count = |rows: &Option<Vec<ImpactRow>>| {
        rows.as_ref()
            .map_or_else(|| "-".to_owned(), |rows| rows.len().to_string())
    };
    let mut line = format!(
        "impact: nodes {}, symbols {}, tests {}, tasks {}",
        outcome.nodes.len(),
        count(&outcome.symbols),
        count(&outcome.tests),
        outcome.tasks.len()
    );
    if let Some(code) = outcome.code {
        line.push_str(&format!(
            "; code files {} (.rs {}, .ron {}), skipped {}",
            code.files, code.rs, code.ron, code.skipped
        ));
    }
    let left_out = outcome.left_out;
    let mut parts = Vec::new();
    if left_out.generated > 0 {
        parts.push(format!("{} generated", left_out.generated));
    }
    if left_out.tier3 > 0 {
        parts.push(format!("{} archived", left_out.tier3));
    }
    if !parts.is_empty() {
        line.push_str("; left out: ");
        line.push_str(&parts.join(", "));
    }
    line
}

/// The characters of `lines`, a line end each.
fn cost(lines: &[String]) -> usize {
    lines.iter().map(|line| line.chars().count() + 1).sum()
}

/// The cut (see the module documentation): the lists in order, each item
/// whole, the first item whole whatever its size ([`lines_within`]); the
/// first item that does not fit and every later one not shown.
fn plan(outcome: &ImpactOutcome) -> Shown {
    if outcome.reason.is_some() {
        return Shown::default();
    }
    let fixed: Vec<String> = header(outcome)
        .into_iter()
        .chain([summary(outcome)])
        .collect();
    let rows = |rows: &Option<Vec<ImpactRow>>, word: &str| -> Vec<usize> {
        rows.iter()
            .flatten()
            .map(|row| cost(&row_text(word, row)))
            .collect()
    };
    let lists: [Vec<usize>; 5] = [
        outcome
            .nodes
            .iter()
            .map(|node| node_text(node).chars().count() + 1)
            .collect(),
        rows(&outcome.symbols, "symbol"),
        rows(&outcome.tests, "test"),
        outcome
            .tasks
            .iter()
            .map(|task| task_line(task).chars().count() + 1)
            .collect(),
        outcome
            .seeds
            .iter()
            .flatten()
            .map(|seed| seed_line(seed).chars().count() + 1)
            .collect(),
    ];
    let mut used = cost(&fixed);
    let mut cut = false;
    let mut fits = [0usize; 5];
    for (at, costs) in lists.iter().enumerate() {
        if cut {
            break;
        }
        let first_item = fits.iter().all(|&fit| fit == 0);
        let (fit, now) = lines_within(costs.iter().copied(), used, first_item);
        fits[at] = fit;
        used = now;
        cut = fit < costs.len();
    }
    let [nodes, symbols, tests, tasks, seeds] = fits;
    Shown {
        seeds,
        nodes,
        symbols,
        tests,
        tasks,
    }
}

/// The first `shown` of `items` (all of a shorter list).
fn first<T>(items: &[T], shown: usize) -> &[T] {
    &items[..shown.min(items.len())]
}

/// What the cut left out of each list; `None` for a `null` list.
struct NotShown {
    nodes: usize,
    symbols: Option<usize>,
    tests: Option<usize>,
    tasks: usize,
    seeds: Option<usize>,
}

fn not_shown(outcome: &ImpactOutcome) -> NotShown {
    let shown = outcome.shown;
    let rest = |len: usize, shown: usize| len.saturating_sub(shown);
    NotShown {
        nodes: rest(outcome.nodes.len(), shown.nodes),
        symbols: outcome
            .symbols
            .as_ref()
            .map(|rows| rest(rows.len(), shown.symbols)),
        tests: outcome
            .tests
            .as_ref()
            .map(|rows| rest(rows.len(), shown.tests)),
        tasks: rest(outcome.tasks.len(), shown.tasks),
        seeds: outcome
            .seeds
            .as_ref()
            .map(|seeds| rest(seeds.len(), shown.seeds)),
    }
}

/// `[truncated: <parts> not shown; lower --depth N or narrow the REF or
/// --symbol]`, zero parts omitted.
fn tail(outcome: &ImpactOutcome) -> String {
    let rest = not_shown(outcome);
    let parts: Vec<String> = [
        (Some(rest.nodes), "nodes"),
        (rest.symbols, "symbols"),
        (rest.tests, "tests"),
        (Some(rest.tasks), "tasks"),
        (rest.seeds, "seed matches"),
    ]
    .into_iter()
    .filter_map(|(count, what)| {
        count
            .filter(|&count| count > 0)
            .map(|count| format!("{count} {what}"))
    })
    .collect();
    format!(
        "[truncated: {} not shown; lower --depth N or narrow the REF or --symbol]",
        parts.join(", ")
    )
}

pub(crate) fn render_text(outcome: &ImpactOutcome) -> String {
    if outcome.reason.is_some() {
        return String::new();
    }
    let shown = outcome.shown;
    let mut lines: Vec<String> = Vec::new();
    lines.extend(header(outcome));
    lines.push(summary(outcome));
    lines.extend(first(&outcome.nodes, shown.nodes).iter().map(node_text));
    for (rows, count, word) in [
        (&outcome.symbols, shown.symbols, "symbol"),
        (&outcome.tests, shown.tests, "test"),
    ] {
        for row in first(rows.as_deref().unwrap_or_default(), count) {
            lines.extend(row_text(word, row));
        }
    }
    lines.extend(first(&outcome.tasks, shown.tasks).iter().map(task_line));
    if let Some(seeds) = &outcome.seeds {
        lines.extend(first(seeds, shown.seeds).iter().map(seed_line));
    }
    if outcome.truncated() {
        lines.push(tail(outcome));
    }
    let mut out = String::new();
    for line in lines {
        out.push_str(&line);
        out.push('\n');
    }
    out
}

#[derive(Serialize)]
struct ImpactJson<'a> {
    input: InputJson<'a>,
    reason: Option<&'a str>,
    /// `--since`'s commit: no form of this build takes one.
    revision: Option<&'a str>,
    depth: Option<usize>,
    seeds: SeedsJson<'a>,
    /// `--since`'s delta: no form of this build takes one.
    delta: Option<()>,
    left_out: LeftOut,
    nodes: Vec<NodeJson<'a>>,
    symbols: Option<&'a [&'a BindingRow]>,
    tests: Option<&'a [&'a BindingRow]>,
    tasks: &'a [ImpactTask],
    notes: Vec<String>,
    truncated: Option<TruncatedJson>,
    code: Option<CodeCount>,
}

#[derive(Serialize)]
struct InputJson<'a> {
    #[serde(rename = "ref")]
    reference: Option<&'a str>,
    symbol: Option<&'a str>,
    since: Option<&'a str>,
}

#[derive(Serialize)]
struct SeedsJson<'a> {
    /// The printed nodes at distance 0, by name.
    nodes: Vec<&'a str>,
    /// `--symbol`'s printed matches; `null` for REF.
    symbols: Option<&'a [SeedSymbol]>,
}

/// The items of each list not printed; `null` for a `null` list.
#[derive(Serialize)]
struct TruncatedJson {
    nodes: usize,
    symbols: Option<usize>,
    tests: Option<usize>,
    tasks: usize,
    seeds: Option<usize>,
    /// `--since`'s delta lines: no form of this build takes one.
    delta: Option<usize>,
}

/// The first `count` rows of a list, as the JSON holds them.
fn printed(rows: &Option<Vec<ImpactRow>>, count: usize) -> Option<Vec<&BindingRow>> {
    rows.as_ref()
        .map(|rows| first(rows, count).iter().map(|row| &row.row).collect())
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`; the lists hold the printed items only.
impl Serialize for ImpactOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let shown = self.shown;
        let nodes = first(&self.nodes, shown.nodes);
        let symbols = printed(&self.symbols, shown.symbols);
        let tests = printed(&self.tests, shown.tests);
        let truncated = self.truncated().then(|| {
            let rest = not_shown(self);
            TruncatedJson {
                nodes: rest.nodes,
                symbols: rest.symbols,
                tests: rest.tests,
                tasks: rest.tasks,
                seeds: rest.seeds,
                delta: None,
            }
        });
        ImpactJson {
            input: InputJson {
                reference: self.reference.as_deref(),
                symbol: self.symbol.as_deref(),
                since: None,
            },
            reason: self.reason.as_deref(),
            revision: None,
            depth: self.depth,
            seeds: SeedsJson {
                nodes: nodes
                    .iter()
                    .filter(|node| node.distance == 0)
                    .map(|node| node.id.as_deref().unwrap_or(&node.path))
                    .collect(),
                symbols: self.seeds.as_ref().map(|seeds| first(seeds, shown.seeds)),
            },
            delta: None,
            left_out: self.left_out,
            nodes: nodes.iter().map(NodeJson::of).collect(),
            symbols: symbols.as_deref(),
            tests: tests.as_deref(),
            tasks: first(&self.tasks, shown.tasks),
            notes: notes(&self.messages),
            truncated,
            code: self.code,
        }
        .serialize(serializer)
    }
}
