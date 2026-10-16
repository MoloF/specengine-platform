//! `spec refs REF` (canon `code-bindings`, "`spec refs`"): what in the
//! code names a node — every marker (`@implements`, `@verifies`, `@configures`,
//! `@assumes`) and every comment mention of an ID — read from the code
//! index, refreshed just now from the `[code]` tree
//! (canon `symbol-index`, "spec refs and spec index"), byte for
//! byte what the walk over the tree answers. Read-only: the refreshes
//! write the data directory, nothing else is written.
//!
//! - **Order of work**: the config (`[code]`, else exit 2), `REF` (every
//!   form `spec show` takes, its exits and notes alike), the code index's
//!   refresh ([`crate::code_index::refresh_code`]; a gap of the walk: exit
//!   2), its stored markers and mentions, each stored ID judged once.
//! - **Oracle**: [`collect_rust`] and [`collect_ron`] read the same rows
//!   from a walked [`CodeTree`] (the tests compare the two answers).
//! - **States** (canon `code-bindings`, "ID state", "Revision state"): a
//!   marker's ID is read by the reference grammar
//!   and resolved as `spec show` resolves it: `resolved`, `alias`,
//!   `ambiguous` (several nodes), `homoglyph` (never resolved, listed under
//!   its Latin fix's node), `other_project`, `queue` (`PR-` and digits,
//!   when no node takes it), `dangling`. Its `@N` meets the node's own
//!   revision (core [`revision`]: none written is 1): `unversioned`,
//!   `bad_rev`, `rev_unknown`, `current`, `spec_ahead`, `predated`; judged
//!   per node.
//! - **Mentions** (canon `code-bindings`, "Mentions"): the grammar's
//!   hits in every comment, the
//!   hit at a marker's own token skipped, each with an `id_state` and the
//!   place its comment's binding gives it.
//! - **Blocks**: a block per node `REF` names (several holders: one
//!   `warning:`), listing what names that node, never its sections or
//!   parents; markers by relation, path bytes, line, column, then mentions
//!   by path, line, column; the summary last. Rows are printed while the
//!   text stays within [`OUTPUT_CAP_CHARS`], then a `[truncated: …]` line;
//!   JSON holds exactly the printed rows.

use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

use serde::Serialize;
use specengine_code::{Anchor, Binding, Marker, Name, Relation, markers_in};
use specengine_core::check::{CheckFile, CheckInput, Resolution, Resolver};
use specengine_core::proposal::PROPOSAL_PREFIX;
use specengine_core::{CodeConfig, Paths, Revision, revision};
use specengine_model::{IdScheme, IdScript, Reference, grammar};

use specengine_code::{Ambiguity, ErrorCategory, Level, LevelError, Levels, Unanchored};
use specengine_store::{CodeFileState, CodeRefs, StoredBinding, StoredMarker};

use crate::cap::OUTPUT_CAP_CHARS;
use crate::code_index::{LANG_RON, LANG_RUST, RON_AMBIGUOUS, refresh_code};
use crate::code_walk::CodeTree;
use crate::corpus::indexed_in;
use crate::location::open_index;
use crate::project::{ProjectRoot, config_error, discover_with_text};
use crate::search::notes;
use crate::show::{Target, classify, latin_text, not_indexed, pick_reference, project_qualified};
use crate::{CliError, Env, Globals, Message, one_line, store_error};
// The walk oracle's tests read the tree and the corpus as the walk did.
#[cfg_attr(
    not(test),
    expect(
        unused_imports,
        reason = "RD-8's oracle (the walk) is read by the tests"
    )
)]
use crate::{code_walk::read_tree, corpus::indexed};

/// Why a written `@rev` of `REF` is ignored (the note's end).
const REV_IGNORED: &str = "`spec refs` lists every marker whatever its revision";

/// The binding words of a row (`spec lock accept`'s rows too).
pub(crate) const ITEM: &str = "item";
pub(crate) const MODULE: &str = "module";
pub(crate) const RON: &str = "ron";
pub(crate) const UNANCHORED: &str = "unanchored";
pub(crate) const CANNOT_VERIFY: &str = "cannot_verify";
pub(crate) const FILE: &str = "file";

/// An ambiguous binding's word, before its reason (`spec lock accept`'s
/// state; a `spec refs` row prints ` ambiguous <reason>`).
pub(crate) const AMBIGUOUS: &str = "ambiguous";

/// A RON comment binding no value; a RON container past the depth cap.
pub(crate) const RON_NO_VALUE: &str = "no_value";
pub(crate) const RON_TOO_DEEP: &str = "nesting_too_deep";
/// The one ambiguity of a RON path.
pub(crate) const RON_DUPLICATE: &str = "duplicate";

/// `spec refs` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefsRequest {
    /// `REF` as given.
    pub reference: String,
}

/// What `spec refs` found: a block per node `REF` names, or the reason
/// there is none (exit 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefsOutcome {
    /// `REF` as given.
    pub reference: String,
    /// Why `REF` names nothing; `None` when `nodes` is not empty.
    pub reason: Option<String>,
    pub messages: Vec<Message>,
    /// By (path, position), each with every row; the cap is applied when
    /// rendering.
    pub nodes: Vec<RefsNode>,
    /// The code tree read; `None` when `REF` names nothing (no walk).
    pub code: Option<CodeCount>,
}

/// One node `REF` names and what in the code names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RefsNode {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    /// Root-relative.
    pub path: String,
    /// The 1-based line its span starts on.
    pub line: usize,
    /// Its own revision: written, else 1; `None` when written but
    /// unreadable.
    pub rev: Option<u32>,
    /// A revision is written (readable or not).
    pub rev_written: bool,
    pub markers: Vec<MarkerRow>,
    pub mentions: Vec<MentionRow>,
}

/// One marker naming the node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MarkerRow {
    pub relation: &'static str,
    pub path: String,
    /// 1-based, of the marker's `@`.
    pub line: usize,
    /// 1-based, in bytes, of the marker's `@`.
    pub column: usize,
    /// The ID and the revision as written.
    pub token: String,
    /// The ID as written.
    pub id: String,
    /// `@N`: 1 to 9 digits ending the token, else `None`.
    pub rev: Option<u32>,
    pub id_state: &'static str,
    /// The Latin form of a look-alike ID.
    pub fix: Option<String>,
    pub rev_state: &'static str,
    /// The levels the marker depends on; `None` when its list is invalid.
    pub levels: Option<Vec<&'static str>>,
    /// `default`, `declared` or the invalid list's category.
    pub levels_state: &'static str,
    pub note: Option<String>,
    pub binding: BindingRow,
}

/// One comment mention of the node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MentionRow {
    pub path: String,
    /// 1-based, of the reference's first byte.
    pub line: usize,
    /// 1-based, in bytes.
    pub column: usize,
    /// The reference as written.
    pub written: String,
    pub id_state: &'static str,
    pub fix: Option<String>,
    pub binding: BindingRow,
}

/// What a marker binds, or where a mention belongs: every key present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BindingRow {
    /// `item`, `module`, `ron`, `unanchored`, `cannot_verify` or `file`.
    pub kind: &'static str,
    /// An item's or a module's `qpath`; a RON anchor as `<path>#<field path>`.
    pub qpath: Option<String>,
    /// An item's `kind()`.
    pub item_kind: Option<String>,
    /// An item's first line (its node's, attributes excluded).
    pub line: Option<usize>,
    /// `path_attribute`, `unrooted` or `duplicate`: shown, not resolved.
    pub ambiguity: Option<&'static str>,
    /// Why nothing is bound, or why it cannot be verified.
    pub category: Option<&'static str>,
}

/// The code files read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CodeCount {
    /// `.rs` and `.ron` files read and analysed.
    pub files: usize,
    pub rs: usize,
    pub ron: usize,
    /// Files and names skipped for not being UTF-8.
    pub skipped: usize,
}

/// The rows left out by the cap.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
struct Omitted {
    markers: usize,
    mentions: usize,
}

/// `spec refs`: reads the config, resolves `REF`, walks the code tree.
pub fn refs(env: &Env, globals: &Globals, request: &RefsRequest) -> Result<RefsOutcome, CliError> {
    let (project, text) = discover_with_text(env, globals)?;
    project.slug()?;
    let code = code_config(&project, &text)?;

    let written = request.reference.trim();
    let scheme = &project.config.scheme;
    let mut messages = Vec::new();
    let target = match classify(written, scheme, &mut messages, REV_IGNORED)? {
        Ok(target) => target,
        Err(reason) => return Ok(not_found(request, reason, messages)),
    };
    // One database handle for both refreshes.
    let mut open = open_index(env, &project)?;
    let input = indexed_in(&mut open.index, &project, &mut messages, true)?;
    let mut corpus = Corpus::new(&input, scheme, &project.config.paths);
    let nodes = match corpus.target_nodes(&target, written)? {
        Ok(nodes) => nodes,
        Err(reason) => return Ok(not_found(request, reason, messages)),
    };
    if nodes.len() > 1 {
        let cited: Vec<String> = nodes.iter().map(|&key| corpus.citation(key)).collect();
        messages.push(Message::Warning(format!(
            "`{written}` has {} holders, all shown: {}",
            nodes.len(),
            cited.join(", ")
        )));
    }

    let refreshed = refresh_code(
        &mut open.index,
        &project.root,
        &code,
        &project.config_label,
        scheme,
        false,
    )?;
    refreshed.require_complete()?;
    messages.extend(refreshed.messages.iter().cloned());
    let stored = open
        .index
        .code_refs()
        .map_err(store_error)?
        .unwrap_or_default();
    let mut blocks: BTreeMap<NodeKey, Block> =
        nodes.iter().map(|&key| (key, Block::default())).collect();
    collect_stored(&stored, &mut corpus, &mut blocks)?;

    let nodes = nodes
        .iter()
        .map(|&key| {
            let block = blocks.remove(&key).unwrap_or_default();
            corpus.node(key, block)
        })
        .collect();
    Ok(RefsOutcome {
        reference: request.reference.clone(),
        reason: None,
        messages,
        nodes,
        code: Some(stored_count(&stored, refreshed.report.skipped)),
    })
}

/// The code files the answer read: each stored Rust or RON file that was
/// read and is UTF-8 (a gap refused the answer before), and those skipped
/// (names and bytes not UTF-8).
pub(crate) fn stored_count(stored: &CodeRefs, skipped: usize) -> CodeCount {
    let read = |lang: &str| {
        stored
            .files
            .iter()
            .filter(|file| file.lang == lang && file.utf8 && file.read_error.is_none())
            .count()
    };
    let (rs, ron) = (read(LANG_RUST), read(LANG_RON));
    CodeCount {
        files: rs + ron,
        rs,
        ron,
        skipped,
    }
}

/// The `[code]` table of the config read from `text`; none: exit 2, the
/// code layer is off (`spec lock` too).
pub(crate) fn code_config(project: &ProjectRoot, text: &str) -> Result<CodeConfig, CliError> {
    CodeConfig::from_toml(text)
        .map_err(|error| config_error(&project.config_label, &error))?
        .ok_or_else(|| {
            CliError::spec(format!(
                "{} has no [code] table: the code layer is off (add [code] with roots)",
                project.config_label
            ))
        })
}

/// Exit 1: the reason on one line.
fn not_found(request: &RefsRequest, reason: String, messages: Vec<Message>) -> RefsOutcome {
    RefsOutcome {
        reference: request.reference.clone(),
        reason: Some(one_line(&reason)),
        messages,
        nodes: Vec::new(),
        code: None,
    }
}

/// A node: its file (an index into the resolver's path-ordered files) and
/// its position in that file's parse. Ordered by (path, position).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct NodeKey {
    pub(crate) file: usize,
    pub(crate) ord: usize,
}

/// The rows of one block before sorting.
#[derive(Debug, Default)]
struct Block {
    markers: Vec<(Relation, MarkerRow)>,
    mentions: Vec<MentionRow>,
}

/// How a written ID fared (canon `code-bindings`, "ID state").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IdState {
    Resolved,
    Alias,
    Ambiguous,
    Homoglyph,
    OtherProject,
    Queue,
    Dangling,
}

impl IdState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Alias => "alias",
            Self::Ambiguous => "ambiguous",
            Self::Homoglyph => "homoglyph",
            Self::OtherProject => "other_project",
            Self::Queue => "queue",
            Self::Dangling => "dangling",
        }
    }

    /// The ID names its nodes itself (a revision can be compared): resolved,
    /// an alias or ambiguous — what binds a node and maps a unit (health's
    /// row 5).
    pub(crate) fn names_nodes(self) -> bool {
        matches!(self, Self::Resolved | Self::Alias | Self::Ambiguous)
    }
}

/// A written ID judged: its state, its Latin fix, the nodes it is listed
/// under (a look-alike's: its fix's).
#[derive(Debug, Clone)]
pub(crate) struct Judged {
    pub(crate) state: IdState,
    pub(crate) fix: Option<String>,
    pub(crate) nodes: Vec<NodeKey>,
}

/// The corpus as the index gave it, by the resolver's path order (also
/// `spec lock accept`'s: [`crate::lock`]).
pub(crate) struct Corpus<'a> {
    pub(crate) resolver: Resolver<'a>,
    pub(crate) files: Vec<&'a CheckFile>,
    scheme: &'a IdScheme,
    /// Written text → its judgement: one resolution per distinct text.
    judged: HashMap<String, Judged>,
}

impl<'a> Corpus<'a> {
    pub(crate) fn new(input: &'a CheckInput, scheme: &'a IdScheme, paths: &'a Paths) -> Self {
        let resolver = Resolver::new(input, scheme, paths);
        // The first file of a path, as the resolver keeps it.
        let mut by_path: BTreeMap<&str, &CheckFile> = BTreeMap::new();
        for file in &input.files {
            by_path.entry(file.path.as_str()).or_insert(file);
        }
        let files = resolver
            .paths()
            .iter()
            .filter_map(|path| by_path.get(path).copied())
            .collect();
        Self {
            resolver,
            files,
            scheme,
            judged: HashMap::new(),
        }
    }

    /// The nodes `REF` names, by (path, position); `Ok(Err)`: none, with
    /// the reason (exit 1).
    pub(crate) fn target_nodes(
        &self,
        target: &Target,
        written: &str,
    ) -> Result<Result<Vec<NodeKey>, String>, CliError> {
        match target {
            Target::Path(path) => {
                let Ok(file) = self.resolver.paths().binary_search(&path.as_str()) else {
                    return Ok(Err(not_indexed(path)));
                };
                let has_node = self.files[file]
                    .parsed
                    .as_ref()
                    .is_some_and(|parsed| !parsed.nodes.is_empty());
                Ok(if has_node {
                    Ok(vec![NodeKey { file, ord: 0 }])
                } else {
                    Err(format!(
                        "`{path}` has no node: it could not be read or is not UTF-8"
                    ))
                })
            }
            Target::Reference(reference) => {
                match self.resolver.resolve_detached(reference, written) {
                    Resolution::Resolved(holders) => {
                        let (nodes, _) = self.picked(&holders, reference, written);
                        Ok(if nodes.is_empty() {
                            Err(format!(
                                "`{written}`: its files changed while being read; run it again"
                            ))
                        } else {
                            Ok(nodes)
                        })
                    }
                    Resolution::Dangling(reason) => Ok(Err(format!("`{written}` {reason}"))),
                    Resolution::Skipped => Err(project_qualified(written)),
                }
            }
        }
    }

    /// Per holder file, the nodes the reference names (`spec show`'s
    /// pick), and whether one of them was reached through an alias (an
    /// `aliases_from` prefix, or a file not defining the ID itself).
    fn picked(
        &self,
        holders: &[usize],
        reference: &Reference,
        written: &str,
    ) -> (Vec<NodeKey>, bool) {
        let mut nodes = Vec::new();
        let mut alias = false;
        for &file in holders {
            let Some(parsed) = self.files.get(file).and_then(|file| file.parsed.as_ref()) else {
                continue;
            };
            let ords = pick_reference(parsed, reference, written);
            if ords.is_empty() {
                continue;
            }
            let defines = parsed
                .nodes
                .iter()
                .any(|node| node.id.as_deref() == Some(reference.id.as_str()));
            alias |= reference.alias_of.is_some() || !defines;
            nodes.extend(ords.into_iter().map(|ord| NodeKey { file, ord }));
        }
        (nodes, alias)
    }

    /// `written` (a marker's ID, a mention as written) judged, once per
    /// distinct text.
    pub(crate) fn judge(&mut self, written: &str) -> Judged {
        if let Some(judged) = self.judged.get(written) {
            return judged.clone();
        }
        let judged = self.judge_new(written);
        self.judged.insert(written.to_owned(), judged.clone());
        judged
    }

    fn judge_new(&self, written: &str) -> Judged {
        let none = |state| Judged {
            state,
            fix: None,
            nodes: Vec::new(),
        };
        let Some(found) = (!written.is_empty())
            .then(|| grammar::parse_reference(written, 0, self.scheme))
            .flatten()
        else {
            return none(self.unnamed(written));
        };
        let reference = &found.reference;
        if !found.homoglyphs.is_empty() || reference.script == IdScript::Mixed {
            let fix = latin_text(written, &found.homoglyphs);
            if !fix.is_ascii() || fix == written {
                return none(IdState::Homoglyph);
            }
            let nodes = self.latin_nodes(&fix);
            return Judged {
                state: IdState::Homoglyph,
                fix: Some(fix),
                nodes,
            };
        }
        if reference.project.is_some() {
            return none(IdState::OtherProject);
        }
        match self.resolver.resolve_detached(reference, written) {
            Resolution::Resolved(holders) => {
                let (nodes, alias) = self.picked(&holders, reference, written);
                let state = match nodes.len() {
                    0 => return none(self.unnamed(written)),
                    1 if alias => IdState::Alias,
                    1 => IdState::Resolved,
                    _ => IdState::Ambiguous,
                };
                Judged {
                    state,
                    fix: None,
                    nodes,
                }
            }
            Resolution::Skipped => none(IdState::OtherProject),
            Resolution::Dangling(_) => none(self.unnamed(written)),
        }
    }

    /// The nodes a Latin fix names, when it reads as a plain reference of
    /// this project.
    fn latin_nodes(&self, fix: &str) -> Vec<NodeKey> {
        let Some(found) = grammar::parse_reference(fix, 0, self.scheme) else {
            return Vec::new();
        };
        let reference = &found.reference;
        if !found.homoglyphs.is_empty()
            || reference.script == IdScript::Mixed
            || reference.project.is_some()
        {
            return Vec::new();
        }
        match self.resolver.resolve_detached(reference, fix) {
            Resolution::Resolved(holders) => self.picked(&holders, reference, fix).0,
            Resolution::Dangling(_) | Resolution::Skipped => Vec::new(),
        }
    }

    /// No node takes `written`: the queue's form, else dangling.
    fn unnamed(&self, written: &str) -> IdState {
        let queue = written
            .strip_prefix(PROPOSAL_PREFIX)
            .and_then(|rest| rest.strip_prefix('-'))
            .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()));
        if queue {
            IdState::Queue
        } else {
            IdState::Dangling
        }
    }

    /// A node's own revision.
    pub(crate) fn revision(&self, key: NodeKey) -> Revision {
        match self
            .files
            .get(key.file)
            .and_then(|file| file.parsed.as_ref())
        {
            Some(parsed) => revision(parsed, key.ord),
            None => Revision::Unreadable,
        }
    }

    /// A node's ID, else (a document without one) its path.
    pub(crate) fn node_name(&self, key: NodeKey) -> &str {
        self.files
            .get(key.file)
            .and_then(|file| file.parsed.as_ref())
            .and_then(|parsed| parsed.nodes.get(key.ord))
            .and_then(|node| node.id.as_deref())
            .or_else(|| self.resolver.paths().get(key.file).copied())
            .unwrap_or_default()
    }

    /// `slug/ID` for a feature document's node, else its path.
    pub(crate) fn citation(&self, key: NodeKey) -> String {
        let path = self.resolver.paths()[key.file];
        let id = self.files[key.file]
            .parsed
            .as_ref()
            .and_then(|parsed| parsed.nodes.get(key.ord))
            .and_then(|node| node.id.as_deref());
        match (self.resolver.feature_slug(path), id) {
            (Some(slug), Some(id)) => format!("{slug}/{id}"),
            _ => path.to_owned(),
        }
    }

    /// The block of a node, its rows sorted.
    fn node(&self, key: NodeKey, block: Block) -> RefsNode {
        let file = self.files[key.file];
        let node = file
            .parsed
            .as_ref()
            .and_then(|parsed| parsed.nodes.get(key.ord));
        let start = node.map_or(0, |node| node.span.start);
        let line = file.bytes[..start.min(file.bytes.len())]
            .iter()
            .filter(|&&byte| byte == b'\n')
            .count()
            + 1;
        let revision = self.revision(key);
        let Block {
            mut markers,
            mut mentions,
        } = block;
        markers.sort_by(|(a_relation, a), (b_relation, b)| {
            (a_relation, &a.path, a.line, a.column).cmp(&(b_relation, &b.path, b.line, b.column))
        });
        mentions.sort_by(|a, b| (&a.path, a.line, a.column).cmp(&(&b.path, b.line, b.column)));
        RefsNode {
            id: node.and_then(|node| node.id.clone()),
            kind: node.and_then(|node| node.kind.clone()),
            title: node.and_then(|node| node.title.clone()),
            path: self.resolver.paths()[key.file].to_owned(),
            line,
            rev: revision.number(),
            rev_written: revision != Revision::Unwritten,
            markers: markers.into_iter().map(|(_, row)| row).collect(),
            mentions,
        }
    }
}

/// The `@N` of a marker against a node's revision, the first that applies.
pub(crate) fn rev_state(
    marker: &Marker,
    token: &str,
    state: IdState,
    node: Revision,
) -> &'static str {
    rev_state_of(&marker.id, marker.rev, token, state, node)
}

/// [`rev_state`] of a marker's ID, `@N` and token as written.
pub(crate) fn rev_state_of(
    id: &str,
    rev: Option<u32>,
    token: &str,
    state: IdState,
    node: Revision,
) -> &'static str {
    if token.len() == id.len() {
        return "unversioned";
    }
    let Some(written) = rev else {
        return "bad_rev";
    };
    if !state.names_nodes() {
        return "rev_unknown";
    }
    match node.number() {
        None => "rev_unknown",
        Some(current) => match written.cmp(&current) {
            std::cmp::Ordering::Equal => "current",
            std::cmp::Ordering::Less => "spec_ahead",
            std::cmp::Ordering::Greater => "predated",
        },
    }
}

/// The 1-based line and byte column of offsets in one text.
pub(crate) struct Lines {
    /// The offset each line starts at.
    starts: Vec<usize>,
}

impl Lines {
    pub(crate) fn new(text: &str) -> Self {
        let starts = std::iter::once(0)
            .chain(
                text.bytes()
                    .enumerate()
                    .filter(|(_, byte)| *byte == b'\n')
                    .map(|(at, _)| at + 1),
            )
            .collect();
        Self { starts }
    }

    pub(crate) fn at(&self, offset: usize) -> (usize, usize) {
        let line = self.starts.partition_point(|&start| start <= offset).max(1);
        (line, offset - self.starts[line - 1] + 1)
    }
}

/// The rows a comment's markers and mentions give, under the nodes asked
/// for.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "RD-8's oracle (the walk), read by the tests")
)]
struct CommentRows<'c> {
    path: &'c str,
    /// Where the comment starts in its file.
    start: usize,
    lines: &'c Lines,
    /// What a marker in it binds; where a mention in it belongs: built
    /// only for a row that is listed.
    marker_binding: &'c dyn Fn() -> BindingRow,
    mention_binding: &'c dyn Fn() -> BindingRow,
}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "RD-8's oracle (the walk), read by the tests")
)]
impl CommentRows<'_> {
    fn collect(
        &self,
        comment: &str,
        corpus: &mut Corpus<'_>,
        scheme: &IdScheme,
        blocks: &mut BTreeMap<NodeKey, Block>,
    ) {
        let markers = markers_in(comment);
        let tokens: Vec<std::ops::Range<usize>> = markers
            .iter()
            .map(|marker| marker.token_range(comment))
            .collect();
        for marker in &markers {
            let judged = corpus.judge(&marker.id);
            let under: Vec<NodeKey> = judged
                .nodes
                .iter()
                .copied()
                .filter(|key| blocks.contains_key(key))
                .collect();
            if under.is_empty() {
                continue;
            }
            let token = marker.token(comment);
            let (line, column) = self.lines.at(self.start + marker.offset);
            for key in under {
                let row = MarkerRow {
                    relation: marker.relation.as_str(),
                    path: self.path.to_owned(),
                    line,
                    column,
                    token: token.to_owned(),
                    id: marker.id.clone(),
                    rev: marker.rev,
                    id_state: judged.state.as_str(),
                    fix: judged.fix.clone(),
                    rev_state: rev_state(marker, token, judged.state, corpus.revision(key)),
                    levels: marker
                        .levels
                        .effective()
                        .map(|levels| levels.iter().map(|level| level.as_str()).collect()),
                    levels_state: marker.levels.state(),
                    note: marker.note.clone(),
                    binding: (self.marker_binding)(),
                };
                if let Some(block) = blocks.get_mut(&key) {
                    block.markers.push((marker.relation, row));
                }
            }
        }
        for found in grammar::scan(comment, 0, scheme) {
            let Some(span) = found.reference.span else {
                continue;
            };
            let own_token = tokens.iter().any(|token| token.contains(&span.start));
            if own_token {
                continue;
            }
            let Some(written) = comment.get(span.start..span.end) else {
                continue;
            };
            let judged = corpus.judge(written);
            let (line, column) = self.lines.at(self.start + span.start);
            for key in &judged.nodes {
                let Some(block) = blocks.get_mut(key) else {
                    continue;
                };
                let row = MentionRow {
                    path: self.path.to_owned(),
                    line,
                    column,
                    written: written.to_owned(),
                    id_state: judged.state.as_str(),
                    fix: judged.fix.clone(),
                    binding: (self.mention_binding)(),
                };
                block.mentions.push(row);
            }
        }
    }
}

/// The rows of every Rust comment.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "RD-8's oracle (the walk), read by the tests")
)]
fn collect_rust(
    tree: &CodeTree,
    corpus: &mut Corpus<'_>,
    scheme: &IdScheme,
    blocks: &mut BTreeMap<NodeKey, Block>,
) {
    for (index, file) in tree.rust.iter().enumerate() {
        let lines = Lines::new(&file.text);
        let module = tree.names.modules.get(index);
        let items = tree.names.items.get(index).map_or(&[][..], Vec::as_slice);
        let analysis = &file.binding.analysis;
        let binding_row = |binding: Binding| -> BindingRow {
            let item_name = |item: usize| items.get(item);
            match binding {
                Binding::Item(item) => {
                    let record = analysis.items.get(item);
                    named_row(
                        ITEM,
                        item_name(item),
                        record.map(|record| record.kind.clone()),
                        record.map(|record| record.line),
                    )
                }
                Binding::Module(None) => named_row(MODULE, module, None, None),
                Binding::Module(Some(item)) => named_row(MODULE, item_name(item), None, None),
                Binding::Unanchored(category) => category_row(UNANCHORED, category.as_str()),
                Binding::CannotVerify(category) => category_row(CANNOT_VERIFY, category.as_str()),
                Binding::File => category_row(FILE, ""),
            }
        };
        for comment in &file.binding.comments {
            let Some(text) = file.text.get(comment.range.clone()) else {
                continue;
            };
            let marker_binding = || binding_row(comment.binding);
            let mention_binding = || binding_row(comment.mention);
            CommentRows {
                path: &file.path,
                start: comment.range.start,
                lines: &lines,
                marker_binding: &marker_binding,
                mention_binding: &mention_binding,
            }
            .collect(text, corpus, scheme, blocks);
        }
    }
}

/// The rows of every RON comment.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "RD-8's oracle (the walk), read by the tests")
)]
fn collect_ron(
    tree: &CodeTree,
    corpus: &mut Corpus<'_>,
    scheme: &IdScheme,
    blocks: &mut BTreeMap<NodeKey, Block>,
) {
    for file in &tree.ron {
        let lines = Lines::new(&file.text);
        for anchor in &file.anchors {
            let Some(text) = file.text.get(anchor.comment.clone()) else {
                continue;
            };
            let marker_binding = || ron_row(&file.path, &anchor.anchor, false);
            let mention_binding = || ron_row(&file.path, &anchor.anchor, true);
            CommentRows {
                path: &file.path,
                start: anchor.comment.start,
                lines: &lines,
                marker_binding: &marker_binding,
                mention_binding: &mention_binding,
            }
            .collect(text, corpus, scheme, blocks);
        }
    }
}

/// An item's or a module's row.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "RD-8's oracle (the walk), read by the tests")
)]
fn named_row(
    kind: &'static str,
    name: Option<&Name>,
    item_kind: Option<String>,
    line: Option<usize>,
) -> BindingRow {
    BindingRow {
        kind,
        qpath: name.map(|name| name.qpath.clone()),
        item_kind,
        line,
        ambiguity: name.and_then(|name| name.ambiguity.map(|ambiguity| ambiguity.as_str())),
        category: None,
    }
}

/// An unanchored, `cannot_verify` or file row (`category` empty: none).
fn category_row(kind: &'static str, category: &'static str) -> BindingRow {
    BindingRow {
        kind,
        qpath: None,
        item_kind: None,
        line: None,
        ambiguity: None,
        category: (!category.is_empty()).then_some(category),
    }
}

/// A RON comment's row: its path, ambiguous or not; a mention in an
/// unanchored comment belongs to the file (RON has no items).
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "RD-8's oracle (the walk), read by the tests")
)]
fn ron_row(path: &str, anchor: &Anchor, mention: bool) -> BindingRow {
    match anchor {
        Anchor::Path { path: field, .. } | Anchor::Ambiguous { path: field, .. } => BindingRow {
            kind: RON,
            qpath: Some(format!("{path}#{field}")),
            item_kind: None,
            line: None,
            ambiguity: matches!(anchor, Anchor::Ambiguous { .. }).then_some(RON_DUPLICATE),
            category: None,
        },
        Anchor::Unanchored if mention => category_row(FILE, ""),
        Anchor::Unanchored => category_row(UNANCHORED, RON_NO_VALUE),
        Anchor::CannotVerify => category_row(CANNOT_VERIFY, RON_TOO_DEEP),
    }
}

/// The rows of every stored marker and mention under the nodes asked for:
/// each stored ID judged once ([`Corpus::judge`]), a row built only when
/// it is listed. A stored word the reads do not know is an error (the
/// index was written by another build: `spec index --full`).
fn collect_stored(
    stored: &CodeRefs,
    corpus: &mut Corpus<'_>,
    blocks: &mut BTreeMap<NodeKey, Block>,
) -> Result<(), CliError> {
    let files: BTreeMap<&str, &CodeFileState> = stored
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    for marker in &stored.markers {
        let judged = corpus.judge(&marker.id);
        let under: Vec<NodeKey> = judged
            .nodes
            .iter()
            .copied()
            .filter(|key| blocks.contains_key(key))
            .collect();
        if under.is_empty() {
            continue;
        }
        let MarkerWords {
            relation,
            levels,
            levels_state,
        } = marker_words(marker)?;
        let binding = stored_binding(&marker.path, &marker.binding, &files)?;
        for key in under {
            let row = MarkerRow {
                relation: relation.as_str(),
                path: marker.path.clone(),
                line: marker.line as usize,
                column: marker.column as usize,
                token: marker.token.clone(),
                id: marker.id.clone(),
                rev: marker.rev,
                id_state: judged.state.as_str(),
                fix: judged.fix.clone(),
                rev_state: rev_state_of(
                    &marker.id,
                    marker.rev,
                    &marker.token,
                    judged.state,
                    corpus.revision(key),
                ),
                levels: levels.clone(),
                levels_state,
                note: marker.note.clone(),
                binding: binding.clone(),
            };
            if let Some(block) = blocks.get_mut(&key) {
                block.markers.push((relation, row));
            }
        }
    }
    for mention in &stored.mentions {
        let judged = corpus.judge(&mention.written);
        if !judged.nodes.iter().any(|key| blocks.contains_key(key)) {
            continue;
        }
        let binding = stored_binding(&mention.path, &mention.binding, &files)?;
        for key in &judged.nodes {
            let Some(block) = blocks.get_mut(key) else {
                continue;
            };
            block.mentions.push(MentionRow {
                path: mention.path.clone(),
                line: mention.line as usize,
                column: mention.column as usize,
                written: mention.written.clone(),
                id_state: judged.state.as_str(),
                fix: judged.fix.clone(),
                binding: binding.clone(),
            });
        }
    }
    Ok(())
}

/// The words a stored row may hold besides a relation and a level, read
/// from the code crate's closed lists (`ALL`): a new variant there is
/// known here. Built once per process.
struct StoredWords {
    /// Every `levels_state` of a marker row.
    levels_states: Vec<&'static str>,
    /// Every category a binding row names: why a comment binds nothing,
    /// why it cannot be verified, a RON comment's.
    categories: Vec<&'static str>,
    /// Every ambiguity of an item or a module.
    ambiguities: Vec<&'static str>,
}

static STORED_WORDS: LazyLock<StoredWords> = LazyLock::new(|| StoredWords {
    levels_states: [Levels::Default, Levels::Declared(Vec::new())]
        .iter()
        .map(|levels| levels.state())
        .chain(LevelError::ALL.map(LevelError::as_str))
        .collect(),
    categories: Unanchored::ALL
        .map(Unanchored::as_str)
        .into_iter()
        .chain(ErrorCategory::ALL.map(ErrorCategory::as_str))
        .chain([RON_NO_VALUE, RON_TOO_DEEP])
        .collect(),
    ambiguities: Ambiguity::ALL.map(Ambiguity::as_str).to_vec(),
});

/// A stored marker's relation and levels as this build knows them.
pub(crate) struct MarkerWords {
    pub(crate) relation: Relation,
    /// The levels the marker depends on; `None` when its list is invalid.
    pub(crate) levels: Option<Vec<&'static str>>,
    /// `default`, `declared` or the invalid list's category.
    pub(crate) levels_state: &'static str,
}

/// The words of a stored marker; one this build does not write is an
/// error (`spec index --full`).
pub(crate) fn marker_words(marker: &StoredMarker) -> Result<MarkerWords, CliError> {
    let relation = Relation::ALL
        .into_iter()
        .find(|relation| relation.as_str() == marker.relation)
        .ok_or_else(|| unknown_word("relation", &marker.relation))?;
    let levels = match &marker.levels {
        Some(json) => {
            let words: Vec<String> =
                serde_json::from_str(json).map_err(|_| unknown_word("level list", json))?;
            Some(
                words
                    .iter()
                    .map(|word| {
                        Level::ALL
                            .into_iter()
                            .map(Level::as_str)
                            .find(|level| level == word)
                            .ok_or_else(|| unknown_word("level", word))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
        }
        None => None,
    };
    let levels_state = known(
        &marker.levels_state,
        &STORED_WORDS.levels_states,
        "level state",
    )?;
    Ok(MarkerWords {
        relation,
        levels,
        levels_state,
    })
}

/// A stored ambiguity as this build knows it.
pub(crate) fn known_ambiguity(value: &str) -> Result<&'static str, CliError> {
    known(value, &STORED_WORDS.ambiguities, "ambiguity")
}

/// `value` as one of `words`, or the error naming it.
fn known(value: &str, words: &[&'static str], what: &str) -> Result<&'static str, CliError> {
    words
        .iter()
        .copied()
        .find(|word| *word == value)
        .ok_or_else(|| unknown_word(what, value))
}

/// Exit 2: the code index holds a word this build does not write.
pub(crate) fn unknown_word(what: &str, value: &str) -> CliError {
    CliError::spec(format!(
        "the code index holds an unknown {what} `{value}`; run `spec index --full`"
    ))
}

/// What a stored marker or mention of the file at `path` binds, as a row.
fn stored_binding(
    path: &str,
    stored: &StoredBinding,
    files: &BTreeMap<&str, &CodeFileState>,
) -> Result<BindingRow, CliError> {
    let ambiguity = |value: Option<&String>| -> Result<Option<&'static str>, CliError> {
        value.map(|value| known_ambiguity(value)).transpose()
    };
    let category = |kind: &'static str| -> Result<BindingRow, CliError> {
        let word = stored.anchor.as_deref().unwrap_or_default();
        Ok(category_row(
            kind,
            known(word, &STORED_WORDS.categories, "category")?,
        ))
    };
    let symbol = || {
        stored
            .symbol
            .as_ref()
            .ok_or_else(|| unknown_word("binding without its item", &stored.binding))
    };
    match stored.binding.as_str() {
        ITEM => {
            let symbol = symbol()?;
            Ok(BindingRow {
                kind: ITEM,
                qpath: Some(symbol.qpath.clone()),
                item_kind: Some(symbol.item_kind.clone()),
                line: Some(symbol.start_line as usize),
                ambiguity: ambiguity(symbol.ambiguity.as_ref())?,
                category: None,
            })
        }
        MODULE if stored.symbol.is_some() => {
            let symbol = symbol()?;
            Ok(BindingRow {
                kind: MODULE,
                qpath: Some(symbol.qpath.clone()),
                item_kind: None,
                line: None,
                ambiguity: ambiguity(symbol.ambiguity.as_ref())?,
                category: None,
            })
        }
        MODULE => {
            let module = files
                .get(path)
                .and_then(|file| file.module.as_ref())
                .ok_or_else(|| unknown_word("binding of a file without a name", MODULE))?;
            Ok(BindingRow {
                kind: MODULE,
                qpath: Some(module.qpath.clone()),
                item_kind: None,
                line: None,
                ambiguity: ambiguity(module.ambiguity.as_ref())?,
                category: None,
            })
        }
        RON | RON_AMBIGUOUS => Ok(BindingRow {
            kind: RON,
            qpath: Some(format!(
                "{path}#{}",
                stored.anchor.as_deref().unwrap_or_default()
            )),
            item_kind: None,
            line: None,
            ambiguity: (stored.binding == RON_AMBIGUOUS).then_some(RON_DUPLICATE),
            category: None,
        }),
        UNANCHORED => category(UNANCHORED),
        CANNOT_VERIFY => category(CANNOT_VERIFY),
        FILE => Ok(category_row(FILE, "")),
        other => Err(unknown_word("binding", other)),
    }
}

// ------------------------------------------------------------- rendering

/// The binding as the text prints it.
fn binding_text(binding: &BindingRow) -> String {
    let qpath = binding.qpath.as_deref().map(one_line).unwrap_or_default();
    let ambiguous = binding
        .ambiguity
        .map(|reason| format!(" {AMBIGUOUS} {reason}"))
        .unwrap_or_default();
    match binding.kind {
        ITEM => format!(
            "{ITEM} {qpath} {}{ambiguous}",
            binding.item_kind.as_deref().unwrap_or_default()
        ),
        MODULE | RON => format!("{} {qpath}{ambiguous}", binding.kind),
        kind => match binding.category {
            Some(category) => format!("{kind} {category}"),
            None => kind.to_owned(),
        },
    }
}

/// A marker's levels as a row prints them: `default`, `[a, b]` or
/// `invalid:<category>`.
pub(crate) fn levels_text(levels: Option<&[&'static str]>, state: &str) -> String {
    match (levels, state) {
        (_, "default") => "default".to_owned(),
        (Some(levels), _) => format!("[{}]", levels.join(", ")),
        (None, category) => format!("invalid:{category}"),
    }
}

/// `<relation> | <path>:<line>:<column> | <token> | <id_state>[ <fix>] |
/// <rev_state> | <levels> | <binding>[ | <note>]`.
fn marker_line(row: &MarkerRow) -> String {
    let fix = row
        .fix
        .as_deref()
        .map(|fix| format!(" {fix}"))
        .unwrap_or_default();
    let levels = levels_text(row.levels.as_deref(), row.levels_state);
    let mut line = format!(
        "{} | {}:{}:{} | {} | {}{fix} | {} | {levels} | {}",
        row.relation,
        one_line(&row.path),
        row.line,
        row.column,
        one_line(&row.token),
        row.id_state,
        row.rev_state,
        binding_text(&row.binding)
    );
    if let Some(note) = &row.note {
        line.push_str(" | ");
        line.push_str(&one_line(note));
    }
    line
}

/// `mention | <path>:<line>:<column> | <written> | <id_state>[ <fix>] |
/// <binding>`.
fn mention_line(row: &MentionRow) -> String {
    let fix = row
        .fix
        .as_deref()
        .map(|fix| format!(" {fix}"))
        .unwrap_or_default();
    format!(
        "mention | {}:{}:{} | {} | {}{fix} | {}",
        one_line(&row.path),
        row.line,
        row.column,
        one_line(&row.written),
        row.id_state,
        binding_text(&row.binding)
    )
}

/// Search's first four fields and ` | rev <R>`.
fn header(node: &RefsNode) -> String {
    let name = node.id.as_deref().unwrap_or(&node.path);
    let rev = match (node.rev, node.rev_written) {
        (Some(rev), true) => format!("rev {rev}"),
        (Some(rev), false) => format!("rev {rev} (none written)"),
        (None, _) => "rev unknown".to_owned(),
    };
    format!(
        "{} | {} | {} | {}:{} | {rev}",
        one_line(name),
        one_line(node.kind.as_deref().unwrap_or("-")),
        node.title
            .as_deref()
            .map_or_else(|| "-".to_owned(), one_line),
        one_line(&node.path),
        node.line
    )
}

/// Where the cap falls: per shown node, its shown markers and mentions.
struct Cut {
    shown: Vec<(usize, usize)>,
    omitted: Option<Omitted>,
}

/// The text's cut: headers and rows while the text before the tail stays
/// within [`OUTPUT_CAP_CHARS`] characters (the first header always), then
/// nothing more.
fn cut(outcome: &RefsOutcome) -> Cut {
    let mut used = 0usize;
    let mut shown = Vec::new();
    let mut omitted = Omitted::default();
    let mut cut = false;
    // The line and its line end within the cap.
    let fits = |used: usize, line: &str| used + line.chars().count() < OUTPUT_CAP_CHARS;
    for (index, node) in outcome.nodes.iter().enumerate() {
        if !cut {
            let separator = usize::from(index > 0);
            let head = header(node);
            if index > 0 && !fits(used + separator, &head) {
                cut = true;
            } else {
                used += separator + head.chars().count() + 1;
                let mut markers = 0;
                for row in &node.markers {
                    let line = marker_line(row);
                    if !fits(used, &line) {
                        cut = true;
                        break;
                    }
                    used += line.chars().count() + 1;
                    markers += 1;
                }
                let mut mentions = 0;
                if !cut {
                    for row in &node.mentions {
                        let line = mention_line(row);
                        if !fits(used, &line) {
                            cut = true;
                            break;
                        }
                        used += line.chars().count() + 1;
                        mentions += 1;
                    }
                }
                omitted.markers += node.markers.len() - markers;
                omitted.mentions += node.mentions.len() - mentions;
                shown.push((markers, mentions));
                continue;
            }
        }
        omitted.markers += node.markers.len();
        omitted.mentions += node.mentions.len();
    }
    Cut {
        shown,
        omitted: Some(omitted),
    }
}

/// The text: the blocks within the cap, the tail when cut, the summary;
/// nothing for exit 1.
pub(crate) fn render_text(outcome: &RefsOutcome) -> String {
    let Some(code) = outcome.code.filter(|_| outcome.reason.is_none()) else {
        return String::new();
    };
    let cut = cut(outcome);
    let mut out = String::new();
    for (index, (node, &(markers, mentions))) in outcome.nodes.iter().zip(&cut.shown).enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&header(node));
        out.push('\n');
        for row in &node.markers[..markers] {
            out.push_str(&marker_line(row));
            out.push('\n');
        }
        for row in &node.mentions[..mentions] {
            out.push_str(&mention_line(row));
            out.push('\n');
        }
    }
    if let Some(omitted) = cut.omitted
        && (omitted.markers > 0 || omitted.mentions > 0)
    {
        out.push_str(&format!(
            "[truncated: {} markers, {} mentions not shown]\n",
            omitted.markers, omitted.mentions
        ));
    }
    let markers: usize = outcome.nodes.iter().map(|node| node.markers.len()).sum();
    let mentions: usize = outcome.nodes.iter().map(|node| node.mentions.len()).sum();
    out.push_str(&format!(
        "refs: markers {markers}, mentions {mentions}; code files {} (.rs {}, .ron {}), skipped {}\n",
        code.files, code.rs, code.ron, code.skipped
    ));
    out
}

/// The JSON document: the text's rows, every key present.
#[derive(Serialize)]
struct RefsJson<'a> {
    #[serde(rename = "ref")]
    reference: &'a str,
    reason: Option<&'a str>,
    notes: Vec<String>,
    nodes: Vec<NodeJson<'a>>,
    code: Option<CodeCount>,
    omitted: Option<Omitted>,
}

#[derive(Serialize)]
struct NodeJson<'a> {
    id: Option<&'a str>,
    kind: Option<&'a str>,
    title: Option<&'a str>,
    path: &'a str,
    line: usize,
    rev: Option<u32>,
    rev_written: bool,
    markers: &'a [MarkerRow],
    mentions: &'a [MentionRow],
}

impl Serialize for RefsOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let answered = self.reason.is_none() && self.code.is_some();
        let cut = answered.then(|| cut(self));
        let nodes = match &cut {
            Some(cut) => self
                .nodes
                .iter()
                .zip(&cut.shown)
                .map(|(node, &(markers, mentions))| NodeJson {
                    id: node.id.as_deref(),
                    kind: node.kind.as_deref(),
                    title: node.title.as_deref(),
                    path: &node.path,
                    line: node.line,
                    rev: node.rev,
                    rev_written: node.rev_written,
                    markers: &node.markers[..markers],
                    mentions: &node.mentions[..mentions],
                })
                .collect(),
            None => Vec::new(),
        };
        RefsJson {
            reference: &self.reference,
            reason: self.reason.as_deref(),
            notes: notes(&self.messages),
            nodes,
            code: if answered { self.code } else { None },
            omitted: cut.and_then(|cut| cut.omitted),
        }
        .serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    //! Task spec `code-refs` TE-3 ("States"): every marker of
    //! `fixtures/code-a` (its `expected.json`), its generated files included,
    //! judged as `spec refs` judges it — the `id_state` and, per node it
    //! names, the `rev_state` — through this module's own resolver, so the
    //! states `spec refs` lists under no node (`other_project`, `queue`,
    //! `dangling`) are checked as well as the listed ones. Mentions' states
    //! alike. The U+0420 look-alike is written at test time from an escape.
    //! No node path is written here: each is read from `expected.json` (the
    //! CLI's genre scan, `tests/genre.rs`, reads every source whole).

    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_json::Value;
    use specengine_code::markers_in;
    use specengine_model::grammar;

    use super::{Corpus, Lines, Revision, rev_state};
    use crate::code_walk::read_tree;
    use crate::corpus::indexed;
    use crate::project::discover_with_text;
    use crate::{Env, Globals};

    const ID_STATES: [&str; 7] = [
        "resolved",
        "alias",
        "ambiguous",
        "homoglyph",
        "other_project",
        "queue",
        "dangling",
    ];
    const REV_STATES: [&str; 6] = [
        "unversioned",
        "bad_rev",
        "rev_unknown",
        "current",
        "spec_ahead",
        "predated",
    ];

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// A scratch directory, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "specengine-refs-states-{name}-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");
            Self(fs::canonicalize(&path).expect("canonical scratch"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/code-a")
    }

    fn expected() -> Value {
        let text = fs::read_to_string(fixture().join("expected.json")).expect("expected.json");
        serde_json::from_str(&text).expect("expected.json is JSON")
    }

    fn copy_dir(from: &Path, to: &Path) {
        fs::create_dir_all(to).expect("copy target");
        for entry in fs::read_dir(from).expect("fixture directory") {
            let entry = entry.expect("entry");
            let kind = entry.file_type().expect("file type");
            let target = to.join(entry.file_name());
            if kind.is_dir() {
                copy_dir(&entry.path(), &target);
            } else if kind.is_file() {
                fs::write(&target, fs::read(entry.path()).expect("read")).expect("write");
            }
        }
    }

    /// `expected.json`'s `generated.files`, written under `root`.
    fn write_generated(root: &Path, expected: &Value) {
        for file in expected["generated"]["files"].as_array().expect("files") {
            let path = root.join(file["path"].as_str().expect("path"));
            let mut text = String::new();
            for part in file["parts"].as_array().expect("parts") {
                let times = usize::try_from(part["times"].as_u64().expect("times")).expect("usize");
                text.push_str(&part["text"].as_str().expect("text").repeat(times));
            }
            fs::create_dir_all(path.parent().expect("parent")).expect("dir");
            fs::write(path, text).expect("generated file");
        }
    }

    /// What the resolver says of one marker: its id_state, its fix and, per
    /// node it names (`-` for none), the rev_state.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Judgement {
        token: String,
        id_state: String,
        fix: Option<String>,
        rev_states: BTreeMap<String, String>,
    }

    /// Every marker (`(path, line)`) and mention (`(path, line, written)`) of
    /// the code tree under `root`, judged.
    #[allow(clippy::type_complexity)]
    fn judge_all(
        root: &Path,
        home: &Path,
    ) -> (
        BTreeMap<(String, usize), Judgement>,
        BTreeMap<(String, usize, String), (String, Option<String>)>,
    ) {
        let env = Env {
            cwd: root.to_path_buf(),
            home: Some(home.as_os_str().to_owned()),
            xdg_data_home: None,
        };
        let globals = Globals {
            root: Some(root.to_path_buf()),
            config: None,
        };
        let (project, text) = discover_with_text(&env, &globals).expect("the project");
        let code = specengine_core::CodeConfig::from_toml(&text)
            .expect("a valid [code]")
            .expect("a [code] table");
        let mut messages = Vec::new();
        let input = indexed(&env, &project, &mut messages, true).expect("the index");
        let scheme = &project.config.scheme;
        let mut corpus = Corpus::new(&input, scheme, &project.config.paths);
        let tree = read_tree(&project.root, &code, &mut messages).expect("the code tree");

        let mut comments: Vec<(String, String, usize)> = Vec::new();
        for file in &tree.rust {
            for comment in &file.binding.comments {
                comments.push((
                    file.path.clone(),
                    file.text[comment.range.clone()].to_owned(),
                    comment.range.start,
                ));
            }
        }
        for file in &tree.ron {
            for anchor in &file.anchors {
                comments.push((
                    file.path.clone(),
                    file.text[anchor.comment.clone()].to_owned(),
                    anchor.comment.start,
                ));
            }
        }
        let texts: BTreeMap<String, String> = tree
            .rust
            .iter()
            .map(|file| (file.path.clone(), file.text.clone()))
            .chain(
                tree.ron
                    .iter()
                    .map(|file| (file.path.clone(), file.text.clone())),
            )
            .collect();

        let mut markers = BTreeMap::new();
        let mut mentions = BTreeMap::new();
        for (path, comment, start) in comments {
            let lines = Lines::new(&texts[&path]);
            let found = markers_in(&comment);
            let tokens: Vec<std::ops::Range<usize>> = found
                .iter()
                .map(|marker| marker.token_range(&comment))
                .collect();
            for marker in &found {
                let judged = corpus.judge(&marker.id);
                let token = marker.token(&comment);
                let (line, _) = lines.at(start + marker.offset);
                let mut rev_states = BTreeMap::new();
                if judged.nodes.is_empty() {
                    rev_states.insert(
                        "-".to_owned(),
                        rev_state(marker, token, judged.state, Revision::Unreadable).to_owned(),
                    );
                }
                for &key in &judged.nodes {
                    let node_path = corpus.resolver.paths()[key.file].to_owned();
                    let state = rev_state(marker, token, judged.state, corpus.revision(key));
                    rev_states.insert(node_path, state.to_owned());
                }
                let previous = markers.insert(
                    (path.clone(), line),
                    Judgement {
                        token: token.to_owned(),
                        id_state: judged.state.as_str().to_owned(),
                        fix: judged.fix.clone(),
                        rev_states,
                    },
                );
                assert!(previous.is_none(), "two markers on {path}:{line}");
            }
            for hit in grammar::scan(&comment, 0, scheme) {
                let Some(span) = hit.reference.span else {
                    continue;
                };
                if tokens.iter().any(|token| token.contains(&span.start)) {
                    continue;
                }
                let written = comment[span.start..span.end].to_owned();
                let judged = corpus.judge(&written);
                let (line, _) = lines.at(start + span.start);
                mentions.insert(
                    (path.clone(), line, written),
                    (judged.state.as_str().to_owned(), judged.fix.clone()),
                );
            }
        }
        (markers, mentions)
    }

    /// A row of `expected.json`: `(path, line)`, the token, id_state, fix and
    /// rev_state (`row`) or rev_states by holder (`rows_by_holder`).
    struct ExpectedMarker {
        path: String,
        line: usize,
        token: String,
        id_state: String,
        fix: Option<String>,
        /// Node path (`-`: none) → rev_state.
        rev_states: BTreeMap<String, String>,
    }

    fn location(field: &str) -> (String, usize) {
        let mut parts = field.rsplitn(3, ':');
        let _column = parts.next().expect("column");
        let line = parts.next().expect("line").parse().expect("a line");
        (parts.next().expect("path").to_owned(), line)
    }

    fn state_and_fix(field: &str) -> (String, Option<String>) {
        match field.split_once(' ') {
            Some((state, fix)) => (state.to_owned(), Some(fix.to_owned())),
            None => (field.to_owned(), None),
        }
    }

    fn expected_markers(
        rows: &Value,
        nodes: &BTreeMap<String, Vec<String>>,
    ) -> Vec<ExpectedMarker> {
        let mut out = Vec::new();
        for marker in rows.as_array().expect("markers") {
            let listed: Vec<String> = marker["listed_under"]
                .as_array()
                .expect("listed_under")
                .iter()
                .map(|id| id.as_str().expect("id").to_owned())
                .collect();
            let (row, by_holder) = match marker["row"].as_str() {
                Some(row) => (row.to_owned(), None),
                None => {
                    let holders = marker["rows_by_holder"]
                        .as_object()
                        .expect("rows_by_holder");
                    let first = holders.values().next().expect("a holder");
                    (
                        first.as_str().expect("row").to_owned(),
                        Some(holders.clone()),
                    )
                }
            };
            let fields: Vec<&str> = row.split(" | ").collect();
            let (path, line) = location(fields[1]);
            let (id_state, fix) = state_and_fix(fields[3]);
            let mut rev_states = BTreeMap::new();
            match by_holder {
                Some(holders) => {
                    for (holder, row) in holders {
                        let fields: Vec<&str> = row.as_str().expect("row").split(" | ").collect();
                        rev_states.insert(holder, fields[4].to_owned());
                    }
                }
                None if listed.is_empty() => {
                    rev_states.insert("-".to_owned(), fields[4].to_owned());
                }
                None => {
                    for id in &listed {
                        for node_path in &nodes[id] {
                            rev_states.insert(node_path.clone(), fields[4].to_owned());
                        }
                    }
                }
            }
            out.push(ExpectedMarker {
                path,
                line,
                token: fields[2].to_owned(),
                id_state,
                fix,
                rev_states,
            });
        }
        out
    }

    /// Node ID → the paths holding it, from `expected.json` `nodes`.
    fn node_paths(nodes: &[&Value]) -> BTreeMap<String, Vec<String>> {
        let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for node in nodes {
            out.entry(node["id"].as_str().expect("id").to_owned())
                .or_default()
                .push(node["path"].as_str().expect("path").to_owned());
        }
        out
    }

    /// Judges the fixture (with its generated files when `generated`) and
    /// compares every marker and mention with `expected.json`; returns the
    /// judgements.
    fn check_against_expected(generated: bool) -> BTreeMap<(String, usize), Judgement> {
        let expected = expected();
        let scratch = Scratch::new(if generated { "generated" } else { "base" });
        let root = scratch.0.join("code-a");
        copy_dir(&fixture(), &root);
        let home = scratch.0.join("home");
        fs::create_dir_all(&home).expect("home");
        let mut rows: Vec<Value> = expected["markers"].as_array().expect("markers").clone();
        let mut node_values: Vec<&Value> = expected["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .collect();
        if generated {
            write_generated(&root, &expected);
            rows.extend(
                expected["generated"]["markers"]
                    .as_array()
                    .expect("generated")
                    .clone(),
            );
            node_values.extend(expected["generated"]["nodes"].as_array().expect("nodes"));
        }
        let nodes = node_paths(&node_values);
        let want = expected_markers(&Value::Array(rows), &nodes);
        let (got, mentions) = judge_all(&root, &home);

        let mut failures = Vec::new();
        let mut seen = BTreeSet::new();
        for marker in &want {
            let key = (marker.path.clone(), marker.line);
            seen.insert(key.clone());
            let Some(judged) = got.get(&key) else {
                failures.push(format!("{}:{}: no marker judged", marker.path, marker.line));
                continue;
            };
            let wanted = Judgement {
                token: marker.token.clone(),
                id_state: marker.id_state.clone(),
                fix: marker.fix.clone(),
                rev_states: marker.rev_states.clone(),
            };
            if *judged != wanted {
                failures.push(format!(
                    "{}:{}:\n   got: {judged:?}\n  want: {wanted:?}",
                    marker.path, marker.line
                ));
            }
        }
        for key in got.keys() {
            if !seen.contains(key) {
                failures.push(format!(
                    "{}:{}: a marker expected.json does not list",
                    key.0, key.1
                ));
            }
        }
        for mention in expected["mentions"].as_array().expect("mentions") {
            let row = mention["row"].as_str().expect("row");
            let fields: Vec<&str> = row.split(" | ").collect();
            let (path, line) = location(fields[1]);
            let key = (path, line, fields[2].to_owned());
            let want = state_and_fix(fields[3]);
            match mentions.get(&key) {
                Some(got) if *got == want => {}
                other => failures.push(format!("mention {key:?}: got {other:?}, want {want:?}")),
            }
        }
        let listed = expected["mentions"].as_array().expect("mentions").len();
        if mentions.len() != listed {
            failures.push(format!(
                "{} mentions judged, expected.json lists {listed}: {:?}",
                mentions.len(),
                mentions.keys().collect::<Vec<_>>()
            ));
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        got
    }

    #[test]
    fn every_marker_and_mention_of_code_a_has_its_states() {
        check_against_expected(false);
    }

    /// The path of the node `id` titled `title`, from `expected.json`.
    fn node_path(id: &str, title: &str) -> String {
        let expected = expected();
        let nodes = expected["nodes"].as_array().expect("nodes");
        let generated = expected["generated"]["nodes"].as_array().expect("nodes");
        nodes
            .iter()
            .chain(generated)
            .find(|node| node["id"] == id && node["title"] == title)
            .and_then(|node| node["path"].as_str())
            .unwrap_or_else(|| panic!("expected.json has no node {id} titled {title}"))
            .to_owned()
    }

    /// The generated cases: the U+0420 look-alike (`homoglyph`, its Latin fix,
    /// `rev_unknown`), two holders of one ID (`ambiguous`, compared per node:
    /// `current` under rev 2, `predated` under the unwritten rev 1), a RON
    /// container past the depth cap.
    #[test]
    fn the_generated_cases_have_their_states() {
        let got = check_against_expected(true);
        let look = &got[&("crates/tally/src/look.rs".to_owned(), 1)];
        assert_eq!(look.token, "\u{0420}IECE-COUNTER@3");
        assert_eq!(look.id_state, "homoglyph");
        assert_eq!(look.fix.as_deref(), Some("PIECE-COUNTER"));
        assert_eq!(
            look.rev_states.values().collect::<Vec<_>>(),
            ["rev_unknown"],
            "a look-alike is never resolved, its @3 never compared"
        );
        let twin = &got[&("crates/tally/src/look.rs".to_owned(), 4)];
        assert_eq!(twin.id_state, "ambiguous");
        assert_eq!(
            twin.rev_states,
            BTreeMap::from([
                (node_path("GOAL-03", "Second holder"), "predated".to_owned()),
                (node_path("GOAL-03", "First holder"), "current".to_owned()),
            ])
        );
    }

    /// TE-3's list: every id_state and every rev_state occurs; `X@3a` is
    /// `bad_rev` with its ID resolved; `@1` on a node without a written
    /// revision is `current`.
    #[test]
    fn every_id_state_and_rev_state_occurs() {
        let got = check_against_expected(true);
        let ids: BTreeSet<&str> = got.values().map(|j| j.id_state.as_str()).collect();
        let revs: BTreeSet<&str> = got
            .values()
            .flat_map(|j| j.rev_states.values().map(String::as_str))
            .collect();
        assert_eq!(ids, ID_STATES.into_iter().collect::<BTreeSet<_>>());
        assert_eq!(revs, REV_STATES.into_iter().collect::<BTreeSet<_>>());

        let at = |path: &str, line: usize| &got[&(path.to_owned(), line)];
        let three_a = at("crates/tally/src/counter.rs", 45);
        assert_eq!(
            (three_a.token.as_str(), three_a.id_state.as_str()),
            ("LAW-BOUND@3a", "resolved")
        );
        assert_eq!(three_a.rev_states.values().collect::<Vec<_>>(), ["bad_rev"]);
        let one = at("crates/tally/src/lib.rs", 21);
        assert_eq!(one.token, "GOAL-01@1");
        assert_eq!(
            one.rev_states,
            BTreeMap::from([(
                node_path("GOAL-01", "The count stops at the cap"),
                "current".to_owned()
            )])
        );
    }
}

#[cfg(test)]
mod index_oracle {
    //! Task spec `symbol-index` RD-8, TE-2's refs half. M: index ≠ walk.
    //! `spec refs` from the index (`super::refs`) prints what the walk
    //! prints (`walk_refs`): stdout, stderr and exit alike, text and JSON,
    //! for every REF of code-a (its generated files written), code-index
    //! and verify-ecs (a project of another nature), then after
    //! one edit of every kind TE-2 draws on a code-index copy (a file
    //! added, removed, renamed, edited, a same-size edit, the package
    //! renamed, a manifest between, `[code] exclude`, an `[ids]` prefix, a
    //! CRLF file, twins inserted, removed and `cfg`-edited, a `#[path]`
    //! dropped, a cross-file twin renamed, a ` #` in a file name, an
    //! unrooted twin).

    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::{Env, Globals, Outcome, render_json, render_text};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// A scratch directory, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "specengine-refs-oracle-{name}-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");
            Self(fs::canonicalize(&path).expect("canonical scratch"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    fn copy_dir(from: &Path, to: &Path) {
        fs::create_dir_all(to).expect("copy target");
        for entry in fs::read_dir(from).expect("fixture").flatten() {
            let target = to.join(entry.file_name());
            if entry.file_type().expect("type").is_dir() {
                copy_dir(&entry.path(), &target);
            } else if entry.file_name() != "expected.json" {
                fs::write(&target, fs::read(entry.path()).expect("read")).expect("write");
            }
        }
    }

    fn write(root: &Path, relative: &str, text: impl AsRef<[u8]>) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("parent");
        fs::write(path, text).expect("write");
    }

    fn read(root: &Path, relative: &str) -> String {
        fs::read_to_string(root.join(relative)).expect("read")
    }

    fn replace(root: &Path, relative: &str, from: &str, to: &str) {
        let text = read(root, relative);
        assert_eq!(text.matches(from).count(), 1, "{relative}: {from:?}");
        write(root, relative, text.replacen(from, to, 1));
    }

    /// code-a's generated files (`expected.json` `generated`).
    fn write_generated(root: &Path) {
        let expected: serde_json::Value =
            serde_json::from_str(&read(&fixture("code-a"), "expected.json")).expect("JSON");
        for file in expected["generated"]["files"].as_array().expect("files") {
            let mut text = String::new();
            for part in file["parts"].as_array().expect("parts") {
                let times = usize::try_from(part["times"].as_u64().expect("times")).unwrap();
                text.push_str(&part["text"].as_str().expect("text").repeat(times));
            }
            write(root, file["path"].as_str().expect("path"), text);
        }
    }

    /// `refs` as shipped before the code index: the walk, read now.
    fn walk_refs(
        env: &Env,
        globals: &Globals,
        request: &RefsRequest,
    ) -> Result<RefsOutcome, CliError> {
        let (project, text) = discover_with_text(env, globals)?;
        project.slug()?;
        let code = code_config(&project, &text)?;
        let written = request.reference.trim();
        let scheme = &project.config.scheme;
        let mut messages = Vec::new();
        let target = match classify(written, scheme, &mut messages, REV_IGNORED)? {
            Ok(target) => target,
            Err(reason) => return Ok(not_found(request, reason, messages)),
        };
        let input = indexed(env, &project, &mut messages, true)?;
        let mut corpus = Corpus::new(&input, scheme, &project.config.paths);
        let nodes = match corpus.target_nodes(&target, written)? {
            Ok(nodes) => nodes,
            Err(reason) => return Ok(not_found(request, reason, messages)),
        };
        if nodes.len() > 1 {
            let cited: Vec<String> = nodes.iter().map(|&key| corpus.citation(key)).collect();
            messages.push(Message::Warning(format!(
                "`{written}` has {} holders, all shown: {}",
                nodes.len(),
                cited.join(", ")
            )));
        }
        let tree = read_tree(&project.root, &code, &mut messages)?;
        let mut blocks: BTreeMap<NodeKey, Block> =
            nodes.iter().map(|&key| (key, Block::default())).collect();
        collect_rust(&tree, &mut corpus, scheme, &mut blocks);
        collect_ron(&tree, &mut corpus, scheme, &mut blocks);
        let nodes = nodes
            .iter()
            .map(|&key| {
                let block = blocks.remove(&key).unwrap_or_default();
                corpus.node(key, block)
            })
            .collect();
        Ok(RefsOutcome {
            reference: request.reference.clone(),
            reason: None,
            messages,
            nodes,
            code: Some(CodeCount {
                files: tree.rust.len() + tree.ron.len(),
                rs: tree.rust.len(),
                ron: tree.ron.len(),
                skipped: tree.skipped,
            }),
        })
    }

    /// (exit, stdout, stderr, JSON) of one answer, or the error's text.
    fn printed(result: Result<RefsOutcome, CliError>) -> String {
        match result {
            Ok(outcome) => {
                let outcome = Outcome::Refs(Box::new(outcome));
                format!(
                    "exit {:?}\n{}\n{}\n{}",
                    outcome.exit(),
                    render_text(&outcome),
                    outcome.stderr_lines().join("\n"),
                    render_json(&outcome)
                )
            }
            Err(error) => format!("error {error:?}"),
        }
    }

    /// The index's and the walk's answers to every REF, alike.
    fn assert_alike(root: &Path, home: &Path, references: &[&str], step: &str) {
        let env = Env {
            cwd: root.to_path_buf(),
            home: Some(home.as_os_str().to_owned()),
            xdg_data_home: None,
        };
        let globals = Globals::default();
        for reference in references {
            let request = RefsRequest {
                reference: (*reference).to_owned(),
            };
            let from_index = printed(refs(&env, &globals, &request));
            let from_walk = printed(walk_refs(&env, &globals, &request));
            assert_eq!(from_index, from_walk, "{step}: refs {reference}");
        }
    }

    const CODE_A_REFS: [&str; 9] = [
        "PIECE-COUNTER",
        "LAW-BOUND",
        "LAW-STEP",
        "LAW-ODD",
        "GOAL-01",
        "PREM-001",
        "cap-check/PASS-01",
        "GOAL-03",
        "GOAL-99",
    ];

    const CODE_INDEX_REFS: [&str; 9] = [
        "TENET-WIDTH",
        "TENET-ORDER",
        "TENET-FLOW",
        "SLAB-PROBE",
        "SLAB-SHEET",
        "SLAB-INK",
        "QUILL-01",
        "QUILL-02",
        "NIB-07",
    ];

    const ECS_REFS: [&str; 8] = [
        "PERK-STAMINA",
        "PERK-REGEN",
        "PERK-DRAIN",
        "PERK-MOVE",
        "PERK-SPRINT",
        "TUNE-01",
        "TUNE-02",
        "TUNE-03",
    ];

    #[test]
    fn refs_from_index_equals_refs_from_the_walk() {
        for (name, references) in [
            ("code-a", &CODE_A_REFS[..]),
            ("code-index", &CODE_INDEX_REFS[..]),
            ("verify-ecs", &ECS_REFS[..]),
        ] {
            let scratch = Scratch::new(name);
            let root = scratch.0.join("root");
            copy_dir(&fixture(name), &root);
            if name == "code-a" {
                write_generated(&root);
            }
            let home = scratch.0.join("home");
            fs::create_dir_all(&home).expect("home");
            assert_alike(&root, &home, references, name);
        }
    }

    const SRC: &str = "crates/inkwell/src";

    /// Edit `at` of the sequence on a code-index copy, its name; `None`
    /// past the last.
    fn edit(root: &Path, at: usize) -> Option<&'static str> {
        let twins = format!("{SRC}/twins.rs");
        let lib = format!("{SRC}/lib.rs");
        let name = match at {
            0 => {
                write(
                    root,
                    &format!("{SRC}/added.rs"),
                    "// @implements TENET-FLOW@1\npub fn added() {}\n",
                );
                "a file added"
            }
            1 => {
                fs::create_dir_all(root.join(format!("{SRC}/sub"))).unwrap();
                fs::rename(
                    root.join(format!("{SRC}/added.rs")),
                    root.join(format!("{SRC}/sub/mod.rs")),
                )
                .unwrap();
                "a file renamed"
            }
            2 => {
                let path = format!("{SRC}/page.rs");
                let text = read(root, &path);
                write(root, &path, format!("\n\n{text}\n// see TENET-FLOW\n"));
                "a file edited"
            }
            3 => {
                fs::remove_file(root.join(format!("{SRC}/sub/mod.rs"))).unwrap();
                "a file removed"
            }
            4 => {
                replace(root, &twins, "'/'", "'|'");
                "a same-size edit"
            }
            5 => {
                replace(
                    root,
                    "crates/inkwell/Cargo.toml",
                    "name = \"inkwell-core\"",
                    "name = \"inkwell-press\"",
                );
                "the package renamed"
            }
            6 => {
                write(
                    root,
                    "crates/inkwell/vendor/side/src/lib.rs",
                    "// @implements TENET-FLOW@1\npub fn side() {}\n",
                );
                write(
                    root,
                    "crates/inkwell/vendor/side/Cargo.toml",
                    "[package]\nname = \"side\"\n",
                );
                "a manifest between"
            }
            7 => {
                replace(
                    root,
                    "specengine.toml",
                    "roots = [\"crates\", \"data\"]\n",
                    "roots = [\"crates\", \"data\"]\nexclude = [\"crates/inkwell/src/split/**\"]\n",
                );
                "[code] exclude"
            }
            8 => {
                replace(
                    root,
                    "specengine.toml",
                    "QUILL = { kind = \"quill\", width = 2 }\n",
                    "QUILL = { kind = \"quill\", width = 2 }\nNIB   = { kind = \"nib\", width = 2 }\n",
                );
                write(
                    root,
                    &format!("{SRC}/nib.rs"),
                    "// a nib: NIB-07\npub fn nib() {}\n",
                );
                "an [ids] prefix"
            }
            9 => {
                let text = read(root, &lib);
                write(root, &lib, text.replace('\n', "\r\n"));
                "a CRLF file"
            }
            10 => {
                let text = read(root, &twins);
                write(
                    root,
                    &twins,
                    format!(
                        "{text}\n// @implements TENET-FLOW@1\nimpl Sheet {{\n    pub fn third(&self) {{}}\n}}\n\n#[cfg(feature = \"fast\")]\npub fn separator() -> char {{\n    '+'\n}}\n"
                    ),
                );
                "twins inserted"
            }
            11 => {
                replace(root, &twins, "#[cfg(not(unix))]\n", "");
                "a cfg edited (rule 1 to rule 2)"
            }
            12 => {
                replace(
                    root,
                    &twins,
                    "#[cfg(not(test))]\nmod probe {",
                    "#[cfg(not(test))]\nmod other_probe {",
                );
                "a twin removed"
            }
            13 => {
                write(
                    root,
                    &format!("{SRC}/extra/moved.rs"),
                    "// @implements TENET-FLOW@1\npub fn shift() {}\n",
                );
                let text = read(root, &lib).replace("\r\n", "\n");
                write(
                    root,
                    &lib,
                    format!("{text}#[path = \"extra/moved.rs\"]\nmod moved;\n"),
                );
                "a #[path] written"
            }
            14 => {
                replace(root, &lib, "#[path = \"extra/moved.rs\"]\n", "");
                "the #[path] dropped"
            }
            15 => {
                replace(
                    root,
                    "specengine.toml",
                    "exclude = [\"crates/inkwell/src/split/**\"]\n",
                    "",
                );
                replace(
                    root,
                    &format!("{SRC}/split/mod.rs"),
                    "pub fn joined()",
                    "pub fn apart()",
                );
                "a cross-file twin renamed, the exclude dropped"
            }
            16 => {
                let text = read(root, &lib);
                write(
                    root,
                    &lib,
                    format!("{text}// @implements TENET-FLOW@1\npub mod note;\n"),
                );
                write(root, &format!("{SRC}/note #2.rs"), "pub fn kept() {}\n");
                write(
                    root,
                    "crates/inkwell/vendor/loose.rs",
                    "pub fn twins() {}\n",
                );
                "a ` #` in a file name, an unrooted twin"
            }
            _ => return None,
        };
        Some(name)
    }

    /// One edit of every kind TE-2 draws, each followed by the comparison.
    #[test]
    fn every_step_kind_keeps_refs_equal_to_the_walk() {
        let scratch = Scratch::new("steps");
        let root = scratch.0.join("root");
        copy_dir(&fixture("code-index"), &root);
        let home = scratch.0.join("home");
        fs::create_dir_all(&home).expect("home");
        assert_alike(&root, &home, &CODE_INDEX_REFS, "before any edit");
        let mut at = 0;
        while let Some(step) = edit(&root, at) {
            assert_alike(&root, &home, &CODE_INDEX_REFS, step);
            at += 1;
        }
        assert_eq!(at, 17, "every edit ran");
    }
}
