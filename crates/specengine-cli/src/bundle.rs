//! `spec bundle REF… [--budget N]` (task spec `spec-cli-bundle`; 05 §6):
//! one call that gives the context around named targets within a budget of
//! estimated tokens and names the rest by ID, over the index-fed spec
//! graph; MCP `get_context_bundle` wraps the same function.
//!
//! - **Targets**: `spec show`'s `REF` forms and resolution; several holders
//!   → all, one warning; REFs naming one node count once; a target within
//!   another merges into it, one note.
//! - **Layers**: core's [`bundle_layers`], by link type and `[ids]` scope
//!   only. Targets print as `spec show` prints them; open questions as a
//!   header, the summary and `working answer: <header>`; criteria as a
//!   header and their text; ancestors, decisions, neighbours and terms as a
//!   header and a document's summary (a section: its header alone).
//! - **Code rows** (task spec `bundle-bindings`): with a `[code]` table,
//!   the binding rows of one judge ([`set_bindings`], `sync` as `spec
//!   verify --node` gives it) over the targets, the nodes within them and
//!   layer 4's candidates, kept by node, never by file: layer 5 (bindings)
//!   the rows of the targets and the nodes within them whose relation is
//!   not `verifies`, layer 9 (tests) the `verifies` rows of those and of
//!   layer 4's candidates. A row prints as `spec show --bindings`' row
//!   line without its indent, then `  sig <signature>`; left out, it is a
//!   not-included line named by its symbol (else its file), titled
//!   `<relation> <node>`. A code layer that cannot be read is one `note:
//!   bindings: <cause>`, never the bundle's failure; without `[code]`
//!   nothing is read and nothing said. `spec task show` reads its
//!   package's rows from the same judge ([`Beside`]).
//! - **`tested`** (task spec `tested-reads`): the same read's
//!   `[[tested]]` blocks, as `spec verify` judges them, one `tested |
//!   <state>[ <category>] | <node>` line each, by node bytes, under the
//!   header of a target in its text or outline form (its blocks and those
//!   of the nodes within it) and of a placed criterion (its own); never
//!   under a target's header alone nor in the frame, so the minimum never
//!   depends on them; part of the item they are under, in its fit. No
//!   JSON key: the body, its counts and `bundle_hash` move with them.
//! - **Budget**: `--budget N`, else `[budgets] bundle_node` (read alone,
//!   [`bundle_node_from_toml`]), else [`DEFAULT_BUNDLE_BUDGET`]. The body as
//!   printed stays within it ([`tokens_est`] of the whole text) and within
//!   [`OUTPUT_CAP_CHARS`]: the frame (title, target headers marked
//!   ` | outline`, the not-included reserve, its count every candidate and
//!   row) is the minimum (below it: exit 2); then each item and row, in
//!   layer order, enters iff the body with it, the frame
//!   parts not placed yet and the reserve still fits. A target degrades
//!   (text → outline → its header alone), never cut; the not-included list
//!   (an outlined target's child ID sections first) prints at most
//!   [`BUNDLE_TAIL_LINES`] lines, each while the body fits, the rest as
//!   `- <k> more`. The CLI never cuts a bundle.
//! - **`bundle_hash`**: `b3:` and the hex BLAKE3 of the body's bytes as
//!   printed; the text and the JSON carry the same value.
//!
//! ```text
//! # Bundle: <target names>
//!
//! ## Targets
//! <spec show's header>[ | outline]
//! [tested | <state>[ <category>] | <node>]
//! <text, or a document's summary when outlined>
//!
//! ## <Layer>
//! <name> | <kind or -> | <title or -> | <path>:<line>[ | status <s>][ | via <type> <in|out>, …]
//! [tested | <state>[ <category>] | <node>]
//! <summary or text>
//! [working answer: <header>]
//!
//! ## Bindings
//! <node> | <relation> | <place> | <binding> | layer <layer> | sync <sync>[ <category>][ <detail>]
//! [  sig <signature>]
//!
//! ## Tests
//! <node> | <relation> | <place> | <binding> | layer <layer> | sync <sync>[ <category>][ <detail>]
//! [  sig <signature>]
//!
//! ## Not included
//! - <name> | <title or -> | <n> tokens
//! - <k> more
//! bundle_hash b3:<64 hex digits>
//! tokens <t> of <budget>, chars <c>, bytes <b>, not included <n>
//! ```

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde::ser::SerializeMap as _;
use specengine_core::check::{
    BundleCandidate, BundleLayer, BundleLayers, CheckInput, Endpoint, LinkState, NodeAt, SpecGraph,
    bundle_layers, bundle_node_from_toml,
};
use specengine_core::lock::LockRelation;
use specengine_core::{CodeConfig, tokens_est};
use specengine_model::{BindingRow, TestedRow};
use specengine_store::{GitEnv, SqliteIndex, b3_hash};

use crate::bindings::{SetRow, set_bindings_with, tested_of};
use crate::cap::{OUTPUT_CAP_CHARS, header as show_header, row_line};
use crate::corpus::{holders_warning, indexed_in, locate};
use crate::graph::FollowedType;
use crate::links::state_suffix;
use crate::location::open_index;
use crate::node_set::named_set;
use crate::project::{ProjectRoot, discover_with_text};
use crate::refs::{Corpus, NodeKey};
use crate::search::notes;
use crate::show::{ShownNode, classify, shown};
use crate::tested::tested_line;
use crate::verify::unprefixed;
use crate::{CliError, Env, Globals, Message, one_line};

/// The budget without `--budget` or `[budgets] bundle_node`, in estimated
/// tokens (05 §6: a node bundle, 2k).
pub const DEFAULT_BUNDLE_BUDGET: u32 = 2_000;

/// The most not-included lines a bundle prints before `- <k> more`.
pub const BUNDLE_TAIL_LINES: usize = 20;

/// `spec bundle` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleRequest {
    /// Each `REF` as given, in order; at least one.
    pub references: Vec<String>,
    /// `--budget N`, estimated tokens; `None`: the config's, else the
    /// default.
    pub budget: Option<i64>,
    /// Git's environment: layers 5 and 9 judge their rows with it (task
    /// spec `bundle-bindings`).
    pub git: GitEnv,
}

/// What `spec bundle` answered: a bundle, or why a `REF` names nothing
/// (exit 1, no bundle).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleOutcome {
    /// Each `REF` as given.
    pub references: Vec<String>,
    pub reason: Option<String>,
    pub messages: Vec<Message>,
    /// `None` exactly when `reason` is set.
    pub bundle: Option<Bundle>,
}

/// One bundle: its body as printed and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    /// Estimated tokens.
    pub budget: u32,
    /// [`tokens_est`] of `body`.
    pub tokens: u32,
    /// Unicode scalar values of `body`.
    pub chars: usize,
    /// UTF-8 bytes of `body`.
    pub bytes: usize,
    /// `b3:` and the lower-case hex BLAKE3 of `body`.
    pub bundle_hash: String,
    /// The text printed before the two closing lines.
    pub body: String,
    /// Every layer in print order, with the items placed in it (layers 5
    /// and 9 hold rows, never items: [`Self::bindings`], [`Self::tests`]).
    pub layers: Vec<(BundleLayer, Vec<BundleItem>)>,
    /// Layer 5's rows placed in the body, in verify's row order.
    pub bindings: Vec<BundleRow>,
    /// Layer 9's rows placed in the body, in verify's row order.
    pub tests: Vec<BundleRow>,
    /// The not-included lines printed.
    pub tail: Vec<TailEntry>,
    /// Not included and not listed: the `- <k> more` line's `k` (0: none).
    pub more: usize,
}

impl Bundle {
    /// Every node and row not included: the tail lines and the `more`
    /// count.
    pub fn not_included(&self) -> usize {
        self.tail.len() + self.more
    }

    /// The items or rows placed in `layer`.
    pub fn placed(&self, layer: BundleLayer) -> usize {
        let items: usize = self
            .layers
            .iter()
            .filter(|(placed, _)| *placed == layer)
            .map(|(_, items)| items.len())
            .sum();
        let rows = match layer {
            BundleLayer::Bindings => self.bindings.len(),
            BundleLayer::Tests => self.tests.len(),
            _ => 0,
        };
        items + rows
    }
}

/// A code binding row placed in layer 5 or 9 (task spec
/// `bundle-bindings`): the row as `spec show --bindings` gives it, and
/// what its lines cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleRow {
    pub row: BindingRow,
    /// The row's lines' estimate, as printed in the body.
    pub tokens_est: u32,
}

/// How an item is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemForm {
    /// Its whole text.
    Text,
    /// A target's header marked ` | outline` and its document's summary.
    Outline,
    /// Its header alone (a target's marked ` | outline`).
    Header,
    /// Its header and its document's summary.
    Summary,
}

/// A node placed in the body; no text (the body holds it once).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleItem {
    /// Its ID, else its path.
    pub name: String,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub path: String,
    pub line: usize,
    pub form: ItemForm,
    /// A target's or an open question's `status:`, verbatim.
    pub status: Option<String>,
    /// The links that put it in its layer; `None` for targets and
    /// ancestors.
    pub via: Option<Vec<FollowedType>>,
    /// An open question's working answer.
    pub working_answer: Option<WorkingAnswer>,
    /// The node's own estimate.
    pub tokens_est: u32,
    /// Its file is Tier 3 (only a named target can be).
    pub archived: bool,
}

/// An open question's `working_answer:`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingAnswer {
    /// The answer's name; `None` when unresolved.
    pub name: Option<String>,
    /// As written.
    pub written: String,
    /// Where the `working_answer:` is written, in every state.
    pub path: String,
    pub line: usize,
    pub state: LinkState,
}

/// A not-included line: a node, or a code row (its symbol, else its file;
/// titled `<relation> <node>`; its place; its lines' estimate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailEntry {
    pub name: String,
    pub title: Option<String>,
    pub path: String,
    pub line: usize,
    /// The node's own estimate: what reading it costs.
    pub tokens_est: u32,
    /// The layer it stood in (an outlined target's section: targets).
    pub layer: BundleLayer,
}

/// A node set read beside a bundle from the bundle's one code read (task
/// spec `bundle-bindings`, "Task package": `spec task show` refreshes the
/// code index once, its bundle and its package's rows from one judge).
pub(crate) struct Beside<'a> {
    /// The REFs naming it, each with every section it contains.
    pub refs: &'a [String],
    /// More REFs whose `[[tested]]` blocks it shows, never their rows
    /// (task spec `tested-reads`: the package's criteria refs, so its
    /// `tested` is `spec verify --task`'s set's), each with every section
    /// it contains; judged only when the lock holds a block.
    pub tested_refs: &'a [String],
    /// Its rows, set once the bundle reads the code layer (without
    /// `[code]` too); `None`: the bundle stopped before, [`beside_rows`]
    /// reads them.
    pub rows: Option<BesideRows>,
}

/// A node set's rows read beside a bundle, the blocks of it and of its
/// `tested_refs`, and the read's one note.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BesideRows {
    /// In verify's row order; none without `[code]` or when no row could
    /// be read.
    pub rows: Vec<BindingRow>,
    /// By node bytes, as `spec verify` judges them; `None` without
    /// `[code]`, when the rows were not judged or the blocks could not
    /// be (the note says why).
    pub tested: Option<Vec<TestedRow>>,
    /// `bindings: <cause>`, or `tested: <cause>`; none without `[code]`.
    pub note: Option<String>,
}

/// Where the budget came from, named in the minimum's error.
enum BudgetSource {
    Flag,
    Config { label: String, line: usize },
    Default,
}

impl BudgetSource {
    fn describe(&self, budget: u32) -> String {
        match self {
            Self::Flag => format!("--budget {budget}"),
            Self::Config { label, line } => {
                format!("`[budgets] bundle_node = {budget}` ({label}:{line})")
            }
            Self::Default => format!("the default budget of {budget}"),
        }
    }
}

/// `spec bundle`: updates the index, resolves every `REF`, reads the code
/// rows, assembles.
pub fn bundle(
    env: &Env,
    globals: &Globals,
    request: &BundleRequest,
) -> Result<BundleOutcome, CliError> {
    bundle_with(env, globals, request, None)
}

/// [`bundle`], with `beside`'s rows read by its code read too.
pub(crate) fn bundle_with(
    env: &Env,
    globals: &Globals,
    request: &BundleRequest,
    beside: Option<&mut Beside<'_>>,
) -> Result<BundleOutcome, CliError> {
    let flag = budget_of(request.budget)?;
    if request.references.is_empty() {
        return Err(CliError::spec(
            "name at least one REF: an ID, an alias, `slug/ID`, `ID#SECTION` or a \
             root-relative `.md` path",
        ));
    }
    let (project, text) = discover_with_text(env, globals)?;
    project.slug()?;
    let (budget, source) = match flag {
        Some(budget) => (budget, BudgetSource::Flag),
        None => match bundle_node_from_toml(&text) {
            Ok(Some(node)) => (
                node.tokens,
                BudgetSource::Config {
                    label: project.config_label.clone(),
                    line: node.line,
                },
            ),
            Ok(None) => (DEFAULT_BUNDLE_BUDGET, BudgetSource::Default),
            Err(error) => {
                let message = error.at(&project.config_label);
                return Err(match error.line {
                    Some(_) => CliError::cannot(message),
                    None => CliError::spec(message),
                });
            }
        },
    };

    let scheme = &project.config.scheme;
    let mut messages = Vec::new();
    let mut outcome = BundleOutcome {
        references: request.references.clone(),
        reason: None,
        messages: Vec::new(),
        bundle: None,
    };
    // Every REF classified first: a usage error of any of them is exit 2.
    let mut asked = Vec::new();
    let mut reason = None;
    for reference in &request.references {
        let written = reference.trim();
        match classify(
            written,
            scheme,
            &mut messages,
            "`spec bundle` reads the current files",
        )? {
            Ok(target) => asked.push((written, target)),
            Err(why) => {
                reason.get_or_insert(why);
            }
        }
    }
    if let Some(reason) = reason {
        return Ok(not_found(outcome, reason, messages));
    }

    // One handle for the spec index and the code index (as `spec show
    // --bindings`).
    let mut open = open_index(env, &project)?;
    let input = indexed_in(&mut open.index, &project, &mut messages, true)?;
    let graph = SpecGraph::new(&input, scheme, &project.config.paths);
    let mut targets = Vec::new();
    for (written, target) in &asked {
        match locate(&graph, target, written)? {
            Ok(nodes) => {
                messages.extend(holders_warning(
                    &graph,
                    written,
                    &nodes,
                    "all in the bundle",
                ));
                targets.extend(nodes);
            }
            Err(why) => return Ok(not_found(outcome, why, messages)),
        }
    }
    let layers = bundle_layers(&graph, &targets);
    for &(inner, outer) in &layers.merged {
        messages.push(Message::Note(format!(
            "`{}` is within `{}`: bundled as part of it",
            graph.name(inner),
            graph.name(outer)
        )));
    }
    // Layers 5 and 9, the read's note after the bundle's own messages.
    let read = CodeRead {
        project: &project,
        text: &text,
        git: &request.git,
        input: &input,
    };
    let code = code_rows(&read, &mut open.index, &graph, &layers, beside);
    messages.extend(code.note.clone().map(Message::Note));
    outcome.bundle = Some(assemble(&graph, &layers, &code, budget, &source)?);
    outcome.messages = messages;
    Ok(outcome)
}

/// What the code rows read beside the graph: the project, its config's
/// text, git's environment, the spec corpus read again (every file's
/// bytes, from the handle the code index is read with).
struct CodeRead<'a> {
    project: &'a ProjectRoot,
    text: &'a str,
    git: &'a GitEnv,
    input: &'a CheckInput,
}

/// Layers 5 and 9's rows, the `[[tested]]` blocks printed under the
/// targets' and the criteria's headers, and the read's one note.
#[derive(Default)]
struct CodeRows {
    bindings: Vec<SetRow>,
    tests: Vec<SetRow>,
    /// Per target: the blocks of it and of the nodes within it; per
    /// layer-4 candidate: its own. None when not judged.
    tested: BTreeMap<(BundleLayer, NodeAt), Vec<TestedRow>>,
    note: Option<String>,
}

impl CodeRows {
    /// The lines under the header of `at` in `layer`: one `tested | …` per
    /// block, each with its line end.
    fn tested_lines(&self, layer: BundleLayer, at: NodeAt) -> String {
        self.tested
            .get(&(layer, at))
            .into_iter()
            .flatten()
            .map(|row| format!("{}\n", tested_line(row)))
            .collect()
    }
}

/// The rows of layers 5 and 9 (see the module documentation) from one
/// judge over the targets, the nodes within them and layer 4's candidates
/// (and `beside`'s set, its rows given to it); without `[code]` none, no
/// note, the code index untouched.
fn code_rows(
    read: &CodeRead<'_>,
    index: &mut SqliteIndex,
    graph: &SpecGraph<'_>,
    layers: &BundleLayers,
    beside: Option<&mut Beside<'_>>,
) -> CodeRows {
    if matches!(CodeConfig::from_toml(read.text), Ok(None)) {
        if let Some(beside) = beside {
            beside.rows = Some(BesideRows::default());
        }
        return CodeRows::default();
    }
    let project = read.project;
    let scheme = &project.config.scheme;
    let corpus = Corpus::new(read.input, scheme, &project.config.paths);
    // A graph node as the corpus keys it: the same input, by path.
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
    let targets: BTreeSet<NodeKey> = layers
        .targets
        .iter()
        .flat_map(|&at| graph.within(at))
        .filter_map(key)
        .collect();
    let criteria: BTreeSet<NodeKey> = layers
        .candidates
        .iter()
        .filter(|candidate| candidate.layer == BundleLayer::Criteria)
        .filter_map(|candidate| key(candidate.node))
        .collect();
    let package: BTreeSet<NodeKey> = beside
        .as_ref()
        .map(|beside| named_set(&corpus, scheme, beside.refs))
        .unwrap_or_default();
    let also: Vec<NodeKey> = beside
        .as_ref()
        .map(|beside| named_set(&corpus, scheme, beside.tested_refs))
        .unwrap_or_default()
        .into_iter()
        .collect();
    let package_tested: BTreeSet<NodeKey> = package.iter().chain(&also).copied().collect();
    // Each target's nodes and each criterion, for their blocks.
    let headed: Vec<((BundleLayer, NodeAt), BTreeSet<NodeKey>)> = layers
        .targets
        .iter()
        .map(|&at| {
            let within = graph.within(at).into_iter().filter_map(key).collect();
            ((BundleLayer::Targets, at), within)
        })
        .chain(
            layers
                .candidates
                .iter()
                .filter(|candidate| candidate.layer == BundleLayer::Criteria)
                .map(|candidate| {
                    let own = key(candidate.node).into_iter().collect();
                    ((BundleLayer::Criteria, candidate.node), own)
                }),
        )
        .collect();
    drop(corpus);
    let keys: Vec<NodeKey> = targets
        .iter()
        .chain(&criteria)
        .chain(&package)
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let read_rows = set_bindings_with(
        project, read.text, read.git, index, read.input, &keys, &also,
    );
    let tested = match &read_rows.tested {
        Some(tested) => headed
            .into_iter()
            .map(|(at, nodes)| (at, tested_of(tested, &nodes)))
            .filter(|(_, rows)| !rows.is_empty())
            .collect(),
        None => BTreeMap::new(),
    };
    if let Some(beside) = beside {
        beside.rows = Some(BesideRows {
            rows: read_rows
                .rows
                .iter()
                .flatten()
                .filter(|row| !row.nodes.is_disjoint(&package))
                .map(|row| row.row.clone())
                .collect(),
            tested: read_rows.tested_of(&package_tested),
            note: read_rows.note.clone(),
        });
    }
    let rows = read_rows.rows.unwrap_or_default();
    let (tests, bindings): (Vec<SetRow>, Vec<SetRow>) = rows.into_iter().partition(|row| {
        LockRelation::from_name(&row.row.relation) == Some(LockRelation::Verifies)
    });
    CodeRows {
        bindings: bindings
            .into_iter()
            .filter(|row| !row.nodes.is_disjoint(&targets))
            .collect(),
        tests: tests
            .into_iter()
            .filter(|row| !row.nodes.is_disjoint(&targets) || !row.nodes.is_disjoint(&criteria))
            .collect(),
        tested,
        note: read_rows.note,
    }
}

/// [`Beside`]'s rows when its bundle stopped before reading the code
/// layer: the set's own read of it, the command's one code refresh (the
/// spec index read again, its messages not given: the command's are its
/// own). Never a failure: what cannot be read is the note.
pub(crate) fn beside_rows(
    env: &Env,
    globals: &Globals,
    git: &GitEnv,
    refs: &[String],
    tested_refs: &[String],
) -> BesideRows {
    let unread = |error: CliError| BesideRows {
        rows: Vec::new(),
        tested: None,
        note: Some(format!("bindings: {}", one_line(&unprefixed(&error)))),
    };
    let (project, text) = match discover_with_text(env, globals) {
        Ok(found) => found,
        Err(error) => return unread(error),
    };
    if matches!(CodeConfig::from_toml(&text), Ok(None)) {
        return BesideRows::default();
    }
    let mut open = match open_index(env, &project) {
        Ok(open) => open,
        Err(error) => return unread(error),
    };
    let mut ignored = Vec::new();
    let input = match indexed_in(&mut open.index, &project, &mut ignored, true) {
        Ok(input) => input,
        Err(error) => return unread(error),
    };
    let scheme = &project.config.scheme;
    let corpus = Corpus::new(&input, scheme, &project.config.paths);
    let set = named_set(&corpus, scheme, refs);
    let also = named_set(&corpus, scheme, tested_refs);
    drop(corpus);
    let keys: Vec<NodeKey> = set.iter().copied().collect();
    let also_keys: Vec<NodeKey> = also.iter().copied().collect();
    let read = set_bindings_with(
        &project,
        &text,
        git,
        &mut open.index,
        &input,
        &keys,
        &also_keys,
    );
    let all: BTreeSet<NodeKey> = set.union(&also).copied().collect();
    BesideRows {
        tested: read.tested_of(&all),
        rows: read
            .rows
            .unwrap_or_default()
            .into_iter()
            .filter(|row| !row.nodes.is_disjoint(&set))
            .map(|row| row.row)
            .collect(),
        note: read.note,
    }
}

/// `--budget N`: a whole number from 1 to `u32::MAX`.
fn budget_of(budget: Option<i64>) -> Result<Option<u32>, CliError> {
    budget
        .map(|budget| {
            u32::try_from(budget)
                .ok()
                .filter(|&budget| budget >= 1)
                .ok_or_else(|| {
                    CliError::spec(format!(
                        "--budget {budget}: the budget is a whole number of estimated tokens \
                         from 1 to {}",
                        u32::MAX
                    ))
                })
        })
        .transpose()
}

fn not_found(mut outcome: BundleOutcome, reason: String, messages: Vec<Message>) -> BundleOutcome {
    outcome.reason = Some(one_line(&reason));
    outcome.messages = messages;
    outcome
}

/// A target ready to fit: its three forms.
struct TargetForms {
    item: BundleItem,
    /// `spec show`'s header, then its text.
    full: String,
    /// The header marked ` | outline`, without its line end.
    marked: String,
    /// The marked header and its document's summary.
    outline: Option<String>,
    /// Its direct child ID sections: the not-included list's head when it
    /// is outlined.
    children: Vec<TailEntry>,
}

/// A candidate or a code row ready to fit.
struct Prepared {
    layer: BundleLayer,
    placed: Placed,
    text: String,
    entry: TailEntry,
}

/// What a placed candidate adds to its layer.
enum Placed {
    Item(BundleItem),
    Row(BundleRow),
}

/// The frame, the minimum, then the greedy fit (see the module
/// documentation).
fn assemble(
    graph: &SpecGraph<'_>,
    layers: &BundleLayers,
    code: &CodeRows,
    budget: u32,
    source: &BudgetSource,
) -> Result<Bundle, CliError> {
    let targets: Vec<TargetForms> = layers
        .targets
        .iter()
        .map(|&at| target_forms(graph, at, &code.tested_lines(BundleLayer::Targets, at)))
        .collect();
    let mut candidates: Vec<Prepared> = layers
        .candidates
        .iter()
        .map(|candidate| {
            let tested = code.tested_lines(candidate.layer, candidate.node);
            prepare(graph, candidate, &tested)
        })
        .chain(
            code.bindings
                .iter()
                .map(|row| prepare_row(row, BundleLayer::Bindings)),
        )
        .chain(
            code.tests
                .iter()
                .map(|row| prepare_row(row, BundleLayer::Tests)),
        )
        .collect();
    // Layers 1-4, 5, 6-8, 9; within a layer the order given (stable).
    candidates.sort_by_key(|candidate| candidate.layer);

    let names: Vec<String> = layers
        .targets
        .iter()
        .map(|&at| one_line(&graph.name(at)))
        .collect();
    let mut body = format!("# Bundle: {}\n\n## Targets\n", names.join(", "));
    let reserved = candidates.len()
        + targets
            .iter()
            .map(|target| target.children.len())
            .sum::<usize>();
    let reserved_more = format!("- {reserved} more\n");
    let reserve = if reserved > 0 {
        format!("\n## Not included\n{reserved_more}")
    } else {
        String::new()
    };
    // The frame parts of targets `i..`: their marked headers (none past the
    // end, so an empty target list cannot panic).
    let rest = |from: usize| -> String {
        targets
            .get(from..)
            .unwrap_or_default()
            .iter()
            .map(|target| format!("\n{}\n", target.marked))
            .collect()
    };
    let fits = |parts: &[&str]| -> bool {
        let text: String = parts.concat();
        text.chars().count() <= OUTPUT_CAP_CHARS && tokens_est(&text) <= budget
    };

    // The minimum: the frame.
    let frame = format!(
        "{body}{}\n{}{reserve}",
        targets.first().map_or("", |target| target.marked.as_str()),
        rest(1)
    );
    let minimum = tokens_est(&frame);
    let frame_chars = frame.chars().count();
    if frame_chars > OUTPUT_CAP_CHARS {
        return Err(CliError::spec(format!(
            "this bundle's frame (its title, target headers and not-included line) is \
             {frame_chars} characters, over the {OUTPUT_CAP_CHARS}-character ceiling, \
             whatever the budget ({}); its minimum is {minimum} tokens: name fewer targets",
            source.describe(budget)
        )));
    }
    if budget < minimum {
        return Err(CliError::spec(format!(
            "{} is below this bundle's minimum of {minimum} tokens (its title, target \
             headers and not-included line): raise the budget to {minimum} or more",
            source.describe(budget)
        )));
    }

    // Layer 1: each target in its fullest form that fits; the header alone
    // always does (it is the frame's part).
    let mut placed: Vec<(BundleLayer, Vec<BundleItem>)> = BundleLayer::ALL
        .iter()
        .map(|&layer| (layer, Vec::new()))
        .collect();
    let mut not_included: Vec<TailEntry> = Vec::new();
    let mut outlined: Vec<TailEntry> = Vec::new();
    for (index, target) in targets.iter().enumerate() {
        let separator = if index > 0 { "\n" } else { "" };
        let after = rest(index + 1);
        let header = format!("{}\n", target.marked);
        let mut forms = vec![(ItemForm::Text, &target.full)];
        if let Some(outline) = &target.outline {
            forms.push((ItemForm::Outline, outline));
        }
        let (form, text) = forms
            .into_iter()
            .find(|(_, text)| fits(&[body.as_str(), separator, text.as_str(), &after, &reserve]))
            .unwrap_or((ItemForm::Header, &header));
        body.push_str(separator);
        body.push_str(text);
        if form != ItemForm::Text {
            outlined.extend(target.children.iter().cloned());
        }
        let mut item = target.item.clone();
        item.form = form;
        placed[0].1.push(item);
    }

    // Layers 2 to 9, in order: an item or a row enters iff the body with
    // it and the reserve fits.
    let mut current: Option<BundleLayer> = None;
    let mut bindings = Vec::new();
    let mut tests = Vec::new();
    for candidate in candidates {
        let heading = if current == Some(candidate.layer) {
            "\n".to_owned()
        } else {
            format!("\n## {}\n", layer_heading(candidate.layer))
        };
        if fits(&[body.as_str(), &heading, &candidate.text, &reserve]) {
            body.push_str(&heading);
            body.push_str(&candidate.text);
            current = Some(candidate.layer);
            match candidate.placed {
                Placed::Item(item) => {
                    if let Some((_, items)) = placed
                        .iter_mut()
                        .find(|(layer, _)| *layer == candidate.layer)
                    {
                        items.push(item);
                    }
                }
                Placed::Row(row) if candidate.layer == BundleLayer::Tests => tests.push(row),
                Placed::Row(row) => bindings.push(row),
            }
        } else {
            not_included.push(candidate.entry);
        }
    }

    // The not-included list: lines while the body fits, the rest counted.
    outlined.extend(not_included);
    let listed = outlined;
    let mut tail = Vec::new();
    if !listed.is_empty() {
        let section = "\n## Not included\n";
        let mut lines = String::new();
        for (index, entry) in listed.iter().enumerate().take(BUNDLE_TAIL_LINES) {
            let line = tail_line(entry);
            let more = if index + 1 < listed.len() {
                reserved_more.as_str()
            } else {
                ""
            };
            if !fits(&[body.as_str(), section, &lines, &line, more]) {
                break;
            }
            lines.push_str(&line);
            tail.push(entry.clone());
        }
        body.push_str(section);
        body.push_str(&lines);
        let more = listed.len() - tail.len();
        if more > 0 {
            body.push_str(&format!("- {more} more\n"));
        }
    }
    let more = listed.len() - tail.len();

    Ok(Bundle {
        budget,
        tokens: tokens_est(&body),
        chars: body.chars().count(),
        bytes: body.len(),
        bundle_hash: b3_hash(body.as_bytes()),
        body,
        layers: placed,
        bindings,
        tests,
        tail,
        more,
    })
}

/// A layer's JSON key.
pub fn layer_key(layer: BundleLayer) -> &'static str {
    match layer {
        BundleLayer::Targets => "targets",
        BundleLayer::OpenQuestions => "open_questions",
        BundleLayer::Ancestors => "ancestors",
        BundleLayer::Criteria => "criteria",
        BundleLayer::Bindings => "bindings",
        BundleLayer::Decisions => "decisions",
        BundleLayer::Neighbours => "neighbours",
        BundleLayer::Terms => "terms",
        BundleLayer::Tests => "tests",
    }
}

/// A layer's heading in the body.
pub fn layer_heading(layer: BundleLayer) -> &'static str {
    match layer {
        BundleLayer::Targets => "Targets",
        BundleLayer::OpenQuestions => "Open questions",
        BundleLayer::Ancestors => "Ancestors",
        BundleLayer::Criteria => "Criteria",
        BundleLayer::Bindings => "Bindings",
        BundleLayer::Decisions => "Decisions",
        BundleLayer::Neighbours => "Neighbours",
        BundleLayer::Terms => "Terms",
        BundleLayer::Tests => "Tests",
    }
}

/// `- <name> | <title or -> | <n> tokens` and its line end.
fn tail_line(entry: &TailEntry) -> String {
    format!(
        "- {} | {} | {} tokens\n",
        one_line(&entry.name),
        entry
            .title
            .as_deref()
            .map_or_else(|| "-".to_owned(), one_line),
        entry.tokens_est
    )
}

/// A target's forms: `spec show`'s header and text, the marked header, the
/// outline; `tested` (its blocks' lines, task spec `tested-reads`) under
/// the header of the text and of the outline, never of the header alone.
fn target_forms(graph: &SpecGraph<'_>, at: NodeAt, tested: &str) -> TargetForms {
    let node = shown_node(graph, at);
    let head = show_header(&node);
    let full = format!("{head}\n{tested}{}", with_line_end(&node.text));
    let marked = format!("{head} | outline");
    let outline = summary(graph, at).map(|summary| format!("{marked}\n{tested}{summary}"));
    let children = graph
        .children(at)
        .iter()
        .copied()
        .filter(|child| child.file == at.file && child.ord > 0)
        .map(|child| tail_entry(graph, child, BundleLayer::Targets))
        .collect();
    let item = BundleItem {
        name: graph.name(at),
        kind: node.kind.clone(),
        title: node.title.clone(),
        path: node.path.clone(),
        line: node.line,
        form: ItemForm::Text,
        status: node.status.clone(),
        via: None,
        working_answer: None,
        tokens_est: node.tokens_est,
        archived: node.archived,
    };
    TargetForms {
        item,
        full,
        marked,
        outline,
        children,
    }
}

/// The target as `spec show` reads it, from the graph's bytes and parse
/// (re-read from the working tree after the update).
fn shown_node(graph: &SpecGraph<'_>, at: NodeAt) -> ShownNode {
    let path = graph.paths()[at.file];
    let file = graph.file(at.file);
    let bytes = file.map_or(&[][..], |file| file.bytes.as_slice());
    let archived = graph.is_tier3(at.file);
    let utf8 = std::str::from_utf8(bytes).is_ok();
    match file.and_then(|file| file.parsed.as_ref()) {
        Some(parsed) if at.ord < parsed.nodes.len() => {
            shown(path, bytes, parsed, at.ord, archived, utf8)
        }
        // Not reached: a target is a node of the graph, so of a parse.
        _ => ShownNode {
            id: None,
            kind: None,
            title: None,
            path: path.to_owned(),
            line: 1,
            end_line: 1,
            status: None,
            rev: None,
            tokens_est: 0,
            archived,
            utf8,
            sections: Vec::new(),
            norm_hash: None,
            span_hash: b3_hash(&[]),
            text: String::new(),
            links: None,
            bindings: None,
        },
    }
}

/// A candidate's item, text and not-included entry; `tested` (a
/// criterion's blocks' lines, task spec `tested-reads`) under its header.
fn prepare(graph: &SpecGraph<'_>, candidate: &BundleCandidate, tested: &str) -> Prepared {
    let at = candidate.node;
    let layer = candidate.layer;
    let node = graph.node(at);
    let status = (layer == BundleLayer::OpenQuestions)
        .then(|| node.and_then(|node| node.fields.as_ref()?.status.clone()))
        .flatten();
    let via = (layer != BundleLayer::Ancestors).then(|| {
        candidate
            .via
            .iter()
            .map(|(link_type, direction)| FollowedType {
                link_type: link_type.clone(),
                direction: *direction,
            })
            .collect::<Vec<_>>()
    });
    let mut header = base_header(graph, at);
    if let Some(status) = &status {
        header.push_str(&format!(" | status {}", one_line(status)));
    }
    if let Some(via) = via.as_ref().filter(|via| !via.is_empty()) {
        let pairs: Vec<String> = via
            .iter()
            .map(|pair| format!("{} {}", one_line(&pair.link_type), pair.direction.as_str()))
            .collect();
        header.push_str(&format!(" | via {}", pairs.join(", ")));
    }
    let mut text = format!("{header}\n{tested}");
    let form = if layer == BundleLayer::Criteria {
        text.push_str(&with_line_end(&node_text(graph, at)));
        ItemForm::Text
    } else {
        match summary(graph, at) {
            Some(summary) => {
                text.push_str(&summary);
                ItemForm::Summary
            }
            None => ItemForm::Header,
        }
    };
    let working_answer = candidate
        .working_answer
        .map(|index| working_answer(graph, index));
    if let Some(index) = candidate.working_answer {
        text.push_str(&format!(
            "working answer: {}\n",
            working_answer_line(graph, index)
        ));
    }
    let item = BundleItem {
        name: graph.name(at),
        kind: node.and_then(|node| node.kind.clone()),
        title: node.and_then(|node| node.title.clone()),
        path: graph.paths()[at.file].to_owned(),
        line: graph.line(at),
        form,
        status,
        via,
        working_answer,
        tokens_est: node.map_or(0, |node| node.tokens_est),
        archived: graph.is_tier3(at.file),
    };
    Prepared {
        layer,
        placed: Placed::Item(item),
        text,
        entry: tail_entry(graph, at, layer),
    }
}

/// A code row's text (`spec show --bindings`' row line without its
/// indent, then `  sig <signature>`; one form, in or out), its placed row
/// and its not-included entry (its symbol, else its file; `<relation>
/// <node>`; its line, else 1).
fn prepare_row(row: &SetRow, layer: BundleLayer) -> Prepared {
    let binding = &row.row;
    let mut text = format!("{}\n", row_line(binding, &row.binding));
    if let Some(signature) = &binding.signature {
        text.push_str(&format!("  sig {}\n", one_line(signature)));
    }
    let tokens = tokens_est(&text);
    let entry = TailEntry {
        name: binding
            .symbol
            .clone()
            .unwrap_or_else(|| binding.file.clone()),
        title: Some(format!("{} {}", binding.relation, binding.node)),
        path: binding.file.clone(),
        line: binding
            .line
            .and_then(|line| usize::try_from(line).ok())
            .unwrap_or(1),
        tokens_est: tokens,
        layer,
    };
    Prepared {
        layer,
        placed: Placed::Row(BundleRow {
            row: binding.clone(),
            tokens_est: tokens,
        }),
        text,
        entry,
    }
}

fn tail_entry(graph: &SpecGraph<'_>, at: NodeAt, layer: BundleLayer) -> TailEntry {
    let node = graph.node(at);
    TailEntry {
        name: graph.name(at),
        title: node.and_then(|node| node.title.clone()),
        path: graph.paths()[at.file].to_owned(),
        line: graph.line(at),
        tokens_est: node.map_or(0, |node| node.tokens_est),
        layer,
    }
}

/// `<name> | <kind or -> | <title or -> | <path>:<line>`: no token count,
/// so an edit elsewhere in its file leaves the line alone.
fn base_header(graph: &SpecGraph<'_>, at: NodeAt) -> String {
    let node = graph.node(at);
    let field = |value: Option<&String>| value.map_or_else(|| "-".to_owned(), |v| one_line(v));
    format!(
        "{} | {} | {} | {}:{}",
        one_line(&graph.name(at)),
        field(node.and_then(|node| node.kind.as_ref())),
        field(node.and_then(|node| node.title.as_ref())),
        one_line(graph.paths()[at.file]),
        graph.line(at)
    )
}

/// The open question's first `working_answer:` edge: the answer it names,
/// and where it is written, in every state (as `--links` places a link).
fn working_answer(graph: &SpecGraph<'_>, index: usize) -> WorkingAnswer {
    let edge = &graph.edges()[index];
    let answer = match &edge.target {
        Endpoint::Nodes(nodes) => nodes.first().copied(),
        _ => None,
    };
    WorkingAnswer {
        name: answer.map(|answer| graph.name(answer)),
        written: edge.written.clone(),
        path: graph.paths()[edge.file].to_owned(),
        line: edge.line,
        state: edge.state(),
    }
}

/// The resolved answer's header, else its written form, where it is
/// written and its state.
fn working_answer_line(graph: &SpecGraph<'_>, index: usize) -> String {
    let edge = &graph.edges()[index];
    let answer = match &edge.target {
        Endpoint::Nodes(nodes) => nodes.first().copied(),
        _ => None,
    };
    match answer {
        Some(answer) => base_header(graph, answer),
        None => format!(
            "{} | {}:{}{}",
            one_line(&edge.written),
            one_line(graph.paths()[edge.file]),
            edge.line,
            state_suffix(edge.state(), edge.reason())
        ),
    }
}

/// A document's summary as written, its line end added; `None` for a
/// section or a document without one.
fn summary(graph: &SpecGraph<'_>, at: NodeAt) -> Option<String> {
    if at.ord != 0 {
        return None;
    }
    let span = graph.node(at)?.summary?;
    let bytes = graph.file(at.file)?.bytes.get(span.range())?;
    let text = String::from_utf8_lossy(bytes);
    (!text.trim().is_empty()).then(|| with_line_end(&text))
}

/// A node's span as written (a document: the whole file).
fn node_text(graph: &SpecGraph<'_>, at: NodeAt) -> String {
    let span = graph.node(at).map(|node| node.span).unwrap_or_default();
    let bytes = graph
        .file(at.file)
        .and_then(|file| file.bytes.get(span.range()))
        .unwrap_or_default();
    String::from_utf8_lossy(bytes).into_owned()
}

/// `text` with a line end, as `spec show` prints it (an empty text stays
/// empty).
fn with_line_end(text: &str) -> String {
    if text.is_empty() || text.ends_with('\n') {
        text.to_owned()
    } else {
        format!("{text}\n")
    }
}

pub(crate) fn render_text(outcome: &BundleOutcome) -> String {
    let Some(bundle) = &outcome.bundle else {
        return String::new();
    };
    format!(
        "{}bundle_hash {}\ntokens {} of {}, chars {}, bytes {}, not included {}\n",
        bundle.body,
        bundle.bundle_hash,
        bundle.tokens,
        bundle.budget,
        bundle.chars,
        bundle.bytes,
        bundle.not_included()
    )
}

#[derive(Serialize)]
struct BundleJson<'a> {
    refs: &'a [String],
    reason: Option<&'a str>,
    notes: Vec<String>,
    /// Phase 2's `--task`: always `null`.
    task: Option<()>,
    budget: Option<u32>,
    tokens: Option<u32>,
    chars: Option<usize>,
    bytes: Option<usize>,
    bundle_hash: Option<&'a str>,
    body: Option<&'a str>,
    layers: Option<LayersJson<'a>>,
    tail: Option<Vec<TailJson<'a>>>,
    more: Option<usize>,
}

/// Every layer's key, in print order, its items (`[]` when empty); layers
/// 5 and 9 their placed rows.
struct LayersJson<'a>(&'a Bundle);

impl Serialize for LayersJson<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(BundleLayer::ALL.len()))?;
        for layer in BundleLayer::ALL {
            let rows = match layer {
                BundleLayer::Bindings => Some(&self.0.bindings),
                BundleLayer::Tests => Some(&self.0.tests),
                _ => None,
            };
            if let Some(rows) = rows {
                let rows: Vec<RowJson<'_>> = rows
                    .iter()
                    .map(|placed| RowJson {
                        row: &placed.row,
                        tokens_est: placed.tokens_est,
                    })
                    .collect();
                map.serialize_entry(layer_key(layer), &rows)?;
                continue;
            }
            let items: Vec<ItemJson<'_>> = self
                .0
                .layers
                .iter()
                .filter(|(placed, _)| *placed == layer)
                .flat_map(|(_, items)| items)
                .map(item_json)
                .collect();
            map.serialize_entry(layer_key(layer), &items)?;
        }
        map.end()
    }
}

/// A placed row: its 14 keys, then `tokens_est`.
#[derive(Serialize)]
struct RowJson<'a> {
    #[serde(flatten)]
    row: &'a BindingRow,
    tokens_est: u32,
}

#[derive(Serialize)]
struct ItemJson<'a> {
    name: &'a str,
    kind: Option<&'a str>,
    title: Option<&'a str>,
    path: &'a str,
    line: usize,
    form: ItemForm,
    status: Option<&'a str>,
    via: Option<&'a [FollowedType]>,
    working_answer: Option<WorkingAnswerJson<'a>>,
    tokens_est: u32,
    archived: bool,
}

#[derive(Serialize)]
struct WorkingAnswerJson<'a> {
    name: Option<&'a str>,
    written: &'a str,
    path: &'a str,
    line: usize,
    state: &'static str,
}

#[derive(Serialize)]
struct TailJson<'a> {
    name: &'a str,
    title: Option<&'a str>,
    path: &'a str,
    line: usize,
    tokens_est: u32,
    layer: &'static str,
}

fn item_json(item: &BundleItem) -> ItemJson<'_> {
    ItemJson {
        name: &item.name,
        kind: item.kind.as_deref(),
        title: item.title.as_deref(),
        path: &item.path,
        line: item.line,
        form: item.form,
        status: item.status.as_deref(),
        via: item.via.as_deref(),
        working_answer: item
            .working_answer
            .as_ref()
            .map(|answer| WorkingAnswerJson {
                name: answer.name.as_deref(),
                written: &answer.written,
                path: &answer.path,
                line: answer.line,
                state: answer.state.as_str(),
            }),
        tokens_est: item.tokens_est,
        archived: item.archived,
    }
}

fn view(outcome: &BundleOutcome) -> BundleJson<'_> {
    let bundle = outcome.bundle.as_ref();
    BundleJson {
        refs: &outcome.references,
        reason: outcome.reason.as_deref(),
        notes: notes(&outcome.messages),
        task: None,
        budget: bundle.map(|bundle| bundle.budget),
        tokens: bundle.map(|bundle| bundle.tokens),
        chars: bundle.map(|bundle| bundle.chars),
        bytes: bundle.map(|bundle| bundle.bytes),
        bundle_hash: bundle.map(|bundle| bundle.bundle_hash.as_str()),
        body: bundle.map(|bundle| bundle.body.as_str()),
        layers: bundle.map(LayersJson),
        tail: bundle.map(|bundle| {
            bundle
                .tail
                .iter()
                .map(|entry| TailJson {
                    name: &entry.name,
                    title: entry.title.as_deref(),
                    path: &entry.path,
                    line: entry.line,
                    tokens_est: entry.tokens_est,
                    layer: layer_key(entry.layer),
                })
                .collect()
        }),
        more: bundle.map(|bundle| bundle.more),
    }
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`.
impl Serialize for BundleOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        view(self).serialize(serializer)
    }
}
