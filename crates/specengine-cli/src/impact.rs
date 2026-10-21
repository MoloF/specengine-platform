//! `spec impact REF | --symbol QPATH | --since REV [--depth N]` (canon
//! `spec-impact`, task specs `impact`, `impact-since`): what a change of a
//! node, of a code item or of the code since a revision reaches, in one
//! read — the nodes, their code bindings with their `sync`, the tests bound
//! to them and this repository's open tasks naming them.
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
//!   symbols come from one snapshot with the markers. Or the nodes the code
//!   items changed since `--since`'s commit reach ([`crate::code_delta`]):
//!   the commit's code under the current `[code]` against the files on
//!   disk, untracked ones too, its documents, lock and config never read;
//!   a removed or changed item through the commit's markers, an added or
//!   changed one through the disk's, their IDs judged against the current
//!   corpus. The REV is refused before any git child when it starts with
//!   `-` or holds a control character.
//! - **Nodes**: `spec graph --impact`'s walk from the seeds ([`impact_reach`],
//!   the one walk), `--depth N` as graph's; `left_out` graph's.
//! - **Rows**: one judge ([`set_bindings`]) over every reached node and the
//!   sections within each; `symbols` the rows of any relation but
//!   `verifies`, `tests` the `verifies` rows, each in verify's row order.
//!   The REF form degrades as the reads that show bindings: without
//!   `[code]` both `null`, no note; a code-layer cause one `bindings:` note,
//!   never an exit. `--symbol` and `--since` take the code as their
//!   input: no `[code]`, a bad one or an incomplete walk is exit 2, and
//!   for `--since` no git, a git failure or a blob missing too.
//! - **Tested** (task spec `tested-reads`): from the same read, the
//!   `[[tested]]` blocks of every reached node and the sections within
//!   each, by node bytes, as `spec verify` judges them; `null` without
//!   `[code]`, when the rows were not judged or the blocks could not be
//!   (a note says why), and at exit 1. Never an exit.
//! - **Tasks**: this repository's open tasks whose targets and affected
//!   nodes (each resolved now, with the sections it contains) meet a
//!   reached node or a section within one, by number, each with the
//!   reached nodes it names in radius order; never an exit.
//!
//! ```text
//! [seed symbol <QPATH>: <n> at <tier>]
//! [since <7 hex>: <f> files, <c> changed, <a> added, <r> removed]
//! impact: nodes <n>, symbols <s or ->, tests <t or ->, tasks <k>[; code files <f> (.rs <r>, .ron <o>), skipped <s>][; left out: <g> generated, <a> archived]
//! node <distance> <name> | <kind or -> | <title or -> | <path>:<line>[ | archived]
//! tested | <state>[ <category>] | <node>
//! symbol <row>
//! [  sig <signature>]
//! test <row>
//! task <T> | <state> | <title or -> | <node>, …
//! seed <qpath> | <item_kind> | <path>:<start>-<end>
//! <changed|added|removed> | <file>:<line> | <symbol> <item_kind>
//! [truncated: <parts> not shown; lower --depth N or narrow the REF or --symbol]
//! [truncated: <parts> not shown; lower --depth N or name a later --since REV]
//! ```
//!
//! The cut: the seed header and the summary always print; then nodes,
//! tested blocks, symbol rows, test rows (each with its lines), tasks, the seed matches
//! and the delta's items, each whole, while the text stays within
//! [`crate::OUTPUT_CAP_CHARS`] (the first item whole whatever its size);
//! the first item that does not fit and every later one are not shown.
//! The JSON holds exactly the printed items; `truncated` is `null` uncut.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use specengine_code::query_name;
use specengine_core::CodeConfig;
use specengine_core::check::{CheckInput, NodeAt, SpecGraph};
use specengine_core::lock::LockRelation;
use specengine_model::{BindingRow, TaskStatus, TestedRow};
use specengine_store::{
    CodeSnapshot, GitEnv, SnapshotSymbols, SqliteIndex, StoredSymbol, VerifyGit,
};

use crate::bindings::{SetRow, set_bindings};
use crate::cap::{lines_within, row_lines};
use crate::code_delta::{CodeDelta, DeltaItem, DeltaJson, code_delta};
use crate::code_index::refresh_code;
use crate::code_walk::{CodeTree, read_tree};
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
use crate::tested::tested_line;
use crate::unmapped::{Seeds, reached};
use crate::verify_rows::read_code;
use crate::{CliError, Env, Globals, Message, escape_controls, one_line, store_error};

/// Why a REF's `@rev` is ignored.
const REV_IGNORED: &str = "`spec impact` reads the current files";

/// The command the queue's other-repository note names.
const COMMAND: &str = "spec impact";

/// The hex digits of `--since`'s commit the text prints: a fixed prefix of
/// the full OID, never git's abbreviation (which varies by repository).
const REVISION_PREFIX: usize = 7;

/// `spec impact` options: exactly one of `reference`, `symbol` and `since`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactRequest {
    /// REF as given: any form `spec show` takes.
    pub reference: Option<String>,
    /// `--symbol QPATH` as given: a qpath, a suffix of one or a name.
    pub symbol: Option<String>,
    /// `--since REV` as given: a revision whose code is compared with the
    /// files on disk.
    pub since: Option<String>,
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
    /// `--since` as given.
    pub since: Option<String>,
    /// `--since`'s commit, its full OID; `None` otherwise and at exit 1.
    pub revision: Option<String>,
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
    /// The `[[tested]]` blocks of the reached nodes and the sections
    /// within them, by node bytes, as `spec verify` judges them (task spec
    /// `tested-reads`); `None` without `[code]`, when the rows were not
    /// judged or the blocks could not be (a note says why), and at exit 1.
    pub tested: Option<Vec<TestedRow>>,
    /// The rows of any relation but `verifies`, in verify's row order;
    /// `None` without `[code]` or when no row could be read.
    pub symbols: Option<Vec<ImpactRow>>,
    /// The `verifies` rows, in verify's row order; `None` as `symbols`.
    pub tests: Option<Vec<ImpactRow>>,
    /// By number.
    pub tasks: Vec<ImpactTask>,
    /// `--since`'s code items changed, added and removed; `None` otherwise
    /// and at exit 1.
    pub(crate) delta: Option<CodeDelta>,
    /// The code files the rows' read covered (`set_bindings`'); `None`
    /// without `[code]` or under a cause that leaves no row (a bad
    /// `[code]`, a walk gap, the data directory); `--symbol`'s lookup
    /// walk's, `--since`'s walk on disk, when the rows' read none.
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
    tested: usize,
    symbols: usize,
    tests: usize,
    tasks: usize,
    /// Of the delta's items in their printed order: changed, added,
    /// removed.
    delta: usize,
}

impl ImpactOutcome {
    /// The request echoed, nothing reached yet.
    fn new(request: &ImpactRequest, depth: Option<usize>) -> Self {
        Self {
            reference: request.reference.clone(),
            symbol: request.symbol.clone(),
            since: request.since.clone(),
            revision: None,
            reason: None,
            messages: Vec::new(),
            depth,
            seeds: request.symbol.as_ref().map(|_| Vec::new()),
            left_out: LeftOut::default(),
            nodes: Vec::new(),
            tested: None,
            symbols: None,
            tests: None,
            tasks: Vec::new(),
            delta: None,
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
            || shown.tested < self.tested.as_ref().map_or(0, Vec::len)
            || shown.symbols < self.symbols.as_ref().map_or(0, Vec::len)
            || shown.tests < self.tests.as_ref().map_or(0, Vec::len)
            || shown.tasks < self.tasks.len()
            || shown.delta < self.delta.as_ref().map_or(0, CodeDelta::len)
    }
}

/// `--since`'s REV refused before any git child: a leading `-` (git would
/// read an option) or a control character (U+0000-U+001F, U+007F).
fn refused_revision(rev: &str) -> bool {
    rev.starts_with('-') || rev.chars().any(|c| c <= '\u{1f}' || c == '\u{7f}')
}

/// A REV in a message: every control character, LF, CR and TAB too,
/// written `\u{XX}` (`escape_controls`' form).
fn shown_revision(rev: &str) -> String {
    let mut out = String::with_capacity(rev.len());
    for c in rev.chars() {
        match c {
            '\n' | '\r' | '\t' => out.push_str(&format!("\\u{{{:x}}}", u32::from(c))),
            _ => out.push(c),
        }
    }
    escape_controls(&out)
}

/// The one input as given.
enum Asked<'r> {
    Reference(&'r str),
    Symbol(&'r str),
    Since(&'r str),
}

/// The one input, read before the index: REF's target, or `--symbol`'s
/// query or `--since`'s REV with the `[code]` it needs.
enum Input<'r> {
    Reference(&'r str, Target),
    Symbol(&'r str, CodeConfig),
    Since(&'r str, CodeConfig),
}

/// `spec impact`: reads the input, the config and the index, seeds,
/// walks, reads the rows and the tasks, plans the cut.
pub fn impact(
    env: &Env,
    globals: &Globals,
    request: &ImpactRequest,
) -> Result<ImpactOutcome, CliError> {
    let asked = match (&request.reference, &request.symbol, &request.since) {
        (Some(reference), None, None) => Asked::Reference(reference.trim()),
        (None, Some(symbol), None) => Asked::Symbol(symbol),
        (None, None, Some(since)) => Asked::Since(since),
        _ => {
            return Err(CliError::spec(
                "impact takes exactly one of REF, --symbol and --since",
            ));
        }
    };
    // Before anything is read, git above all.
    if let Asked::Since(rev) = asked
        && refused_revision(rev)
    {
        return Err(CliError::spec(format!(
            "--since `{}` is refused: a revision never starts with `-` or holds a control character",
            shown_revision(rev)
        )));
    }
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
        Asked::Since(rev) => Input::Since(rev, code_config(&project, &text)?),
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
        Input::Since(rev, code) => {
            let seeded = SinceSeeds {
                project: &project,
                code,
                git: &request.git,
                input: &read,
                graph: &graph,
            };
            match since_starts(&seeded, rev, &mut outcome, &mut messages)? {
                Some(starts) => starts,
                None => {
                    let reason = format!("--since `{}` is not a commit", shown_revision(rev));
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

/// What `--since`'s seeds read beside the walk on disk.
struct SinceSeeds<'a> {
    project: &'a ProjectRoot,
    code: &'a CodeConfig,
    git: &'a GitEnv,
    input: &'a CheckInput,
    graph: &'a SpecGraph<'a>,
}

/// `--since`'s seeds, in one git session (none: exit 2): REV's commit
/// (`rev-parse`, `ls-tree`; none: `None`, exit 1), the walk on disk (a
/// gap: exit 2), REV's code half under the current `[code]` (one `cat-file
/// --batch`; a blob missing or a Rust file without a tree: exit 2), the
/// session's end (failing: exit 2); the delta into `outcome.delta`, the
/// commit into `outcome.revision`, the walk's count into `outcome.code`;
/// the nodes the delta reaches, by (path, position).
fn since_starts(
    seeded: &SinceSeeds<'_>,
    rev: &str,
    outcome: &mut ImpactOutcome,
    messages: &mut Vec<Message>,
) -> Result<Option<Vec<NodeAt>>, CliError> {
    let project = seeded.project;
    let mut git =
        VerifyGit::open(&project.root, seeded.git).map_err(|cause| cannot(&cause.message))?;
    let read = since_read(seeded, &mut git, rev, messages);
    let finished = git.finish();
    let read = read?;
    finished.map_err(|cause| cannot(&cause.message))?;
    let Some(SinceRead {
        walk,
        revision,
        tree,
    }) = read
    else {
        return Ok(None);
    };
    let base: &CodeTree = tree.as_ref().unwrap_or(&walk);
    let compared = code_delta(base, &walk);
    let scheme = &project.config.scheme;
    let mut corpus = Corpus::new(seeded.input, scheme, &project.config.paths);
    let keys = compared.seeds(base, &walk, &mut corpus);
    outcome.revision = Some(revision);
    outcome.delta = Some(compared.delta);
    let (rs, ron) = (walk.rust.len(), walk.ron.len());
    outcome.code = Some(CodeCount {
        files: rs + ron,
        rs,
        ron,
        skipped: walk.skipped,
    });
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

/// What `--since` read in its git session.
struct SinceRead {
    /// The walk on disk.
    walk: CodeTree,
    /// REV's commit, its full OID.
    revision: String,
    /// REV's code tree; `None`: the walk's, equal to it.
    tree: Option<CodeTree>,
}

/// REV listed (`None`: no commit), the walk, REV's code half bound.
fn since_read(
    seeded: &SinceSeeds<'_>,
    git: &mut VerifyGit,
    rev: &str,
    messages: &mut Vec<Message>,
) -> Result<Option<SinceRead>, CliError> {
    let Some(listing) = git
        .list_revision(rev)
        .map_err(|cause| cannot(&cause.message))?
    else {
        return Ok(None);
    };
    let walk = read_tree(&seeded.project.root, seeded.code, messages)?;
    let tree = read_code(&listing, seeded.code, git, rev, None)
        .and_then(|code| code.tree(&walk, rev))
        .map_err(|cause| cannot(&cause.message))?;
    Ok(Some(SinceRead {
        walk,
        revision: listing.label().to_owned(),
        tree,
    }))
}

/// Exit 2 for a git or code cause: its line, control characters escaped.
fn cannot(message: &str) -> CliError {
    CliError::spec(escape_controls(&one_line(message)))
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
    // `tested`: every block of the rows' set.
    outcome.tested = rows
        .tested
        .map(|tested| tested.into_iter().map(|entry| entry.row).collect());
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

/// `seed symbol <QPATH>: <n> at <tier>`: `--symbol`'s header; `since <7
/// hex>: <f> files, <c> changed, <a> added, <r> removed`: `--since`'s.
fn header(outcome: &ImpactOutcome) -> Option<String> {
    if let (Some(revision), Some(delta)) = (&outcome.revision, &outcome.delta) {
        let prefix: String = revision.chars().take(REVISION_PREFIX).collect();
        return Some(format!(
            "since {prefix}: {} files, {} changed, {} added, {} removed",
            delta.files,
            delta.changed.len(),
            delta.added.len(),
            delta.removed.len()
        ));
    }
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

/// `<changed|added|removed> | <file>:<line> | <symbol> <item_kind>`.
fn delta_line(word: &str, item: &DeltaItem) -> String {
    escape_controls(&format!(
        "{word} | {}:{} | {} {}",
        one_line(&item.file),
        item.line,
        one_line(&item.symbol),
        one_line(&item.item_kind)
    ))
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
    let lists: [Vec<usize>; 7] = [
        outcome
            .nodes
            .iter()
            .map(|node| node_text(node).chars().count() + 1)
            .collect(),
        outcome
            .tested
            .iter()
            .flatten()
            .map(|row| tested_line(row).chars().count() + 1)
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
        outcome
            .delta
            .iter()
            .flat_map(CodeDelta::items)
            .map(|(word, item)| delta_line(word, item).chars().count() + 1)
            .collect(),
    ];
    let mut used = cost(&fixed);
    let mut cut = false;
    let mut fits = [0usize; 7];
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
    let [nodes, tested, symbols, tests, tasks, seeds, delta] = fits;
    Shown {
        seeds,
        nodes,
        tested,
        symbols,
        tests,
        tasks,
        delta,
    }
}

/// The first `shown` of `items` (all of a shorter list).
fn first<T>(items: &[T], shown: usize) -> &[T] {
    &items[..shown.min(items.len())]
}

/// What the cut left out of each list; `None` for a `null` list.
struct NotShown {
    nodes: usize,
    tested: Option<usize>,
    symbols: Option<usize>,
    tests: Option<usize>,
    tasks: usize,
    seeds: Option<usize>,
    delta: Option<usize>,
}

fn not_shown(outcome: &ImpactOutcome) -> NotShown {
    let shown = outcome.shown;
    let rest = |len: usize, shown: usize| len.saturating_sub(shown);
    NotShown {
        nodes: rest(outcome.nodes.len(), shown.nodes),
        tested: outcome
            .tested
            .as_ref()
            .map(|rows| rest(rows.len(), shown.tested)),
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
        delta: outcome
            .delta
            .as_ref()
            .map(|delta| rest(delta.len(), shown.delta)),
    }
}

/// `[truncated: <parts> not shown; lower --depth N or narrow the REF or
/// --symbol]`, `--since`'s `…; lower --depth N or name a later --since
/// REV]`, zero parts omitted.
fn tail(outcome: &ImpactOutcome) -> String {
    let rest = not_shown(outcome);
    let parts: Vec<String> = [
        (Some(rest.nodes), "nodes"),
        (rest.tested, "tested"),
        (rest.symbols, "symbols"),
        (rest.tests, "tests"),
        (Some(rest.tasks), "tasks"),
        (rest.seeds, "seed matches"),
        (rest.delta, "delta items"),
    ]
    .into_iter()
    .filter_map(|(count, what)| {
        count
            .filter(|&count| count > 0)
            .map(|count| format!("{count} {what}"))
    })
    .collect();
    let narrow = if outcome.since.is_some() {
        "name a later --since REV"
    } else {
        "narrow the REF or --symbol"
    };
    format!(
        "[truncated: {} not shown; lower --depth N or {narrow}]",
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
    if let Some(tested) = &outcome.tested {
        lines.extend(first(tested, shown.tested).iter().map(tested_line));
    }
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
    if let Some(delta) = &outcome.delta {
        lines.extend(
            delta
                .items()
                .take(shown.delta)
                .map(|(word, item)| delta_line(word, item)),
        );
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
    /// `--since`'s commit, its full OID.
    revision: Option<&'a str>,
    depth: Option<usize>,
    seeds: SeedsJson<'a>,
    /// `--since`'s printed delta items.
    delta: Option<DeltaJson<'a>>,
    left_out: LeftOut,
    nodes: Vec<NodeJson<'a>>,
    /// The printed `[[tested]]` blocks.
    tested: Option<&'a [TestedRow]>,
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
    tested: Option<usize>,
    symbols: Option<usize>,
    tests: Option<usize>,
    tasks: usize,
    seeds: Option<usize>,
    /// `--since`'s delta items.
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
                tested: rest.tested,
                symbols: rest.symbols,
                tests: rest.tests,
                tasks: rest.tasks,
                seeds: rest.seeds,
                delta: rest.delta,
            }
        });
        ImpactJson {
            input: InputJson {
                reference: self.reference.as_deref(),
                symbol: self.symbol.as_deref(),
                since: self.since.as_deref(),
            },
            reason: self.reason.as_deref(),
            revision: self.revision.as_deref(),
            depth: self.depth,
            seeds: SeedsJson {
                nodes: nodes
                    .iter()
                    .filter(|node| node.distance == 0)
                    .map(|node| node.id.as_deref().unwrap_or(&node.path))
                    .collect(),
                symbols: self.seeds.as_ref().map(|seeds| first(seeds, shown.seeds)),
            },
            delta: self
                .delta
                .as_ref()
                .map(|delta| DeltaJson::printed(delta, Some(shown.delta))),
            left_out: self.left_out,
            nodes: nodes.iter().map(NodeJson::of).collect(),
            tested: self
                .tested
                .as_deref()
                .map(|tested| first(tested, shown.tested)),
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
