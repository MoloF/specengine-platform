//! `spec lock accept REF [--editorial]` (canon `spec-lock-accept`,
//! "Steps", "The lock commit", "`--editorial`", "A gone node"): what the
//! owner verified, written to `<root>/spec.lock` on the owner's terminal
//! and committed alone (its writers: canon `architecture-code`,
//! "`spec.lock` writers"). One library function, [`lock_accept`], is the
//! accept, the editorial refresh and a gone node's drop; it takes the
//! owner's [`LockConsent`] and extra trailers, so a `drift` item's approve
//! (canon `proposal-drift`) runs this same accept: its request names the
//! item ([`LockAcceptRequest::proposal`]: each question says ` for
//! PR-0012`, the commit carries `Proposal: PR-0012` after `Decided-by:`,
//! the refusals name `spec approve PR-0012`) and the branch its place was
//! checked on ([`LockAcceptRequest::branch`]: another one refuses, exit 1),
//! and its outcome says what was written ([`LockOutcome::written`]), never
//! read from a text.
//!
//! In order (`main` refuses a stdin that is no terminal before anything is
//! read):
//!
//! 1. the project, its root `specengine.toml` (`--config` naming another
//!    is refused), `[code]` (none: exit 2, `spec refs`' text); `REF` as
//!    `spec show` reads it (several nodes: exit 1);
//! 2. git: a worktree, `HEAD` born and on a branch, no operation in
//!    progress; the lock read (exit 2 when unreadable, format 1 or of
//!    other recipes: core's text names `spec lock rebase`, but for another
//!    `norm_recipe`). `REF`'s bytes (a written `@rev` dropped) naming entries
//!    while no node has that canonical ID: the gone node's drop, once
//!    `HEAD` agrees (a changed spec file whose `HEAD` blob still holds the
//!    ID is a dirty input, exit 1); else `REF` naming nothing: exit 1;
//! 3. accept: the code walk ([`crate::code_walk`]) finds the node's
//!    markers; the inputs tracked and clean, else each named, exit 1: the
//!    node's file, its markers' files, `specengine.toml`, `spec.lock`, and
//!    every `.rs` or `.ron` file the walk would read and every `Cargo.toml`
//!    that names its files that differs from `HEAD` (names and `qpath`s
//!    depend on them all: one commit, one lock); each marker file bound
//!    and its lockable bindings hashed from its `HEAD` blob into lock
//!    format 2 (task spec `lock-levels`, "Accept": an item's and a
//!    module's levels and fingerprints, a RON value's hash; a malformed
//!    level list `bad_levels`, a module file with a parse error or a RON
//!    file its analysis rejects `cannot_verify`), the node's revision and
//!    `norm_hash` from its file's `HEAD` blob; a binding whose key core's
//!    [`key_problem`] refuses is not locked; merged into the node's
//!    entries (core [`merge_accept`]);
//! 4. nothing to change: no question, exit 0 when some binding is locked
//!    as it is, else exit 1 saying why none can be; else the rows, the
//!    committer identity (none: exit 2), an ignored first lock (exit 1),
//!    the data directory, then the one `[y/N]`;
//! 5. `HEAD`, branch and the lock's bytes read again (a change: exit 1),
//!    the lock written atomically (a first one created and given its
//!    intent-to-add entry), `git commit --only spec.lock` with hooks run,
//!    its message `spec: lock <ID>` and the trailers `Lock:`, `Lock-mode:`,
//!    `Decided-by:` and the caller's; a failure restores the bytes (`spec
//!    lock rebase`, [`crate::lock_rebase`], takes this same commit under its
//!    own [`LockSubject`]: `spec: lock rebase`, no `Lock:`);
//! 6. the commit, the branch's tip (`refs/heads/<branch>`), verified as
//!    `spec approve` verifies its own, and more: one parent, the old
//!    `HEAD`; only `spec.lock` changed; a `Lock:` trailer naming the node;
//!    its `spec.lock` the bytes the owner confirmed (a commit without it
//!    fails that check); each trailer of the caller's (and `Proposal:`)
//!    present with its value (a hook dropping one fails it). Success is
//!    only a verified commit. A commit made
//!    that fails a check: exit 1 naming the commit, what is wrong and `git
//!    reset --soft <commit>^` (history never rewritten); one that cannot be
//!    read, or a branch that cannot: exit 1, unverified. After a commit git
//!    reported made, a branch that did not move: the bytes and the lock's
//!    index entry restored, exit 1; the branch unmoved but `HEAD` moved:
//!    unverified, nothing restored, exit 1. A failed commit whose branch
//!    moved to a commit that fails a check is not this run's: the bytes
//!    restored, exit 1; one whose moved branch cannot be verified, or whose
//!    branch cannot be read or names no commit: unverified, the bytes left
//!    as written, exit 1; an unmoved branch: the bytes restored, exit 1.
//!    Bytes are restored only while the file still holds what this run
//!    wrote; else they and the lock's index entry are left as they are,
//!    and so said. A git command a warning or a refusal prints names its
//!    path as a quoted literal pathspec ([`crate::pathspec_word`]), the
//!    worktree it runs in named.
//!
//! A node's `[[tested]]` block (format 3, task spec `tested-evidence`,
//! "Other lock writers"): accept keeps it byte for byte (`kept | tested`)
//! and refuses one at the node's revision with another `spec_hash` as it
//! refuses an entry; `--editorial` refreshes one at R (an entry at R); a
//! gone node's drop drops it (a block alone makes a node gone). Its row
//! reads `<change> | tested` after the entries', counted as one entry in
//! the question; `--json`'s ninth key `tested` is its change. The lock is
//! written format 3 iff a block remains. `spec verify --tests` commits its
//! evidence through this same commit under [`LockSubject::Tests`]
//! (`Lock-mode: tests`, outside [`LockMode::TRAILERS`]).
//!
//! Writes the data directory (the index refresh, the commit message's
//! scratch file), `spec.lock` and one commit; no queue row or event.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::ops::Range;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use serde::Serialize;
use specengine_code::{
    Anchor, Binding, Digest, FileAnalysis, LEVEL_RECIPE, Level, LevelHashes, LevelState, Levels,
    MOD_KIND, Marker, Name, Relation, RonValues, RustBinding, RustParser, bind_tree,
    comment_anchors, empty_deps_digest, grammar_info, item_levels, markers_in, module_levels,
    value_hashes,
};
use specengine_core::check::resolve::feature_stem;
use specengine_core::check::{CheckInput, Resolver, canonical_id, feature_scoped};
use specengine_core::lock::{
    AcceptInput, AcceptRefusal, AcceptRow, Change, EntryCode, EntryKey, Fingerprints, HASH_PREFIX,
    LOCK_FILE, LevelCode, Lock, LockKind, LockLevel, LockRelation, Place as RowPlace, Recipes,
    Verified, drop_node, key_problem, merge_accept, parse_lock, refresh_editorial, render_lock,
};
use specengine_core::proposal::{EDITORIAL, LOCK_MODE_TRAILER, LOCK_TRAILER, PROPOSAL_TRAILER};
use specengine_core::verify::BAD_LEVELS;
use specengine_core::{
    CodeConfig, DOCUMENT_EXTENSION, NORM_RECIPE, Revision, is_under, norm_input, revision,
};
use specengine_model::{IdScheme, ParsedFile};
use specengine_store::{
    CreateFileError, GitEnv, PlaceError, WorktreeGit, b3_hash, create_file, replace_file,
};

use crate::code_walk::{CodeTree, MANIFEST, RON_EXTENSION, RUST_EXTENSION, read_tree};
use crate::corpus::indexed;
use crate::location::prepared_data_dir;
use crate::project::{CONFIG_FILE, ProjectRoot, discover_with_text};
use crate::proposals::require_root_config;
use crate::refs::{
    AMBIGUOUS, CANNOT_VERIFY, Corpus, IdState, NodeKey, RON_DUPLICATE, RON_NO_VALUE, RON_TOO_DEEP,
    UNANCHORED, code_config, rev_state,
};
use crate::revisions::Restored;
use crate::show::classify;
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line, pathspec_word};

/// Why a written `@rev` is ignored (the note's end).
const REV_IGNORED: &str = "`spec lock accept` locks the node's revision at HEAD";

/// The commit's trailer naming the committer who decided (the others are
/// core's: `Lock:`, `Lock-mode:`, `Proposal:`).
const DECIDED_BY_TRAILER: &str = "Decided-by";

/// Step 9's refusal (the spec's words): `spec.lock changed while being
/// accepted; nothing written`, a rebase's `… rebased …`.
fn changed_while(subject: &LockSubject<'_>) -> String {
    format!(
        "{LOCK_FILE} changed while being {}; nothing written",
        subject.being()
    )
}

/// A dirty input's states.
const MODIFIED: &str = "modified";
const STAGED: &str = "staged";
const UNTRACKED: &str = "untracked";
const DELETED: &str = "deleted";
pub(crate) const NOT_TRACKED: &str = "not tracked";

/// `spec lock accept` options.
#[derive(Debug, Clone)]
pub struct LockAcceptRequest {
    /// `REF` as given.
    pub reference: String,
    /// `--editorial`: only `spec_hash` set on the node's entries at its
    /// revision.
    pub editorial: bool,
    /// The injected clock, `YYYY-MM-DDTHH:MM:SSZ`: its UTC date is
    /// `verified_at`.
    pub now: String,
    pub git: GitEnv,
    /// The `drift` item whose approve runs this accept (`PR-…`, canon
    /// `proposal-drift`): each question gains ` for <it>` before ` at
    /// <commit>`, the commit `Proposal: <it>` after `Decided-by:` (checked
    /// at step 10 as a caller's trailer), the refusals name `spec approve
    /// <it>`; `None`: `spec lock accept`.
    pub proposal: Option<String>,
    /// The branch the item's place was checked on: step 3 refuses another
    /// (exit 1, nothing written); `None`: any.
    pub branch: Option<String>,
}

/// A trailer after the lock commit's own (canon `proposal-drift`: a
/// caller's own, checked at step 10 with its value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockTrailer {
    pub key: String,
    pub value: String,
}

/// What the owner is shown before the one question: the rows (text:
/// stdout), then the question (stderr).
#[derive(Debug, Clone, Copy)]
pub struct LockPrompt<'a> {
    pub rows: &'a str,
    pub question: &'a str,
}

/// The owner's answer (`main`: the rows printed, the question asked on
/// the terminal; only `y` or `yes` consents).
pub type LockConsent<'a> = &'a mut dyn FnMut(&LockPrompt<'_>) -> bool;

/// Which write an accept is; or a rebase, whose commit takes the same path
/// ([`crate::lock_rebase`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LockMode {
    Accept,
    Editorial,
    /// A gone node's entries dropped (`Lock-mode: accept`).
    Gone,
    /// `spec lock rebase` (`Lock-mode: rebase`): no accept's, so outside
    /// [`LockMode::TRAILERS`].
    Rebase,
    /// `spec verify --tests`' evidence (`Lock-mode: tests`): no accept's,
    /// so outside [`LockMode::TRAILERS`] (no drift item completes on it).
    Tests,
}

impl LockMode {
    /// Every `Lock-mode:` value an accept's commit carries (a drift item's
    /// completion takes these alone; never a rebase's or a test run's).
    pub(crate) const TRAILERS: [&'static str; 2] =
        [Self::Accept.trailer(), Self::Editorial.trailer()];

    /// The `Lock-mode:` trailer's value.
    const fn trailer(self) -> &'static str {
        match self {
            Self::Accept | Self::Gone => "accept",
            Self::Editorial => EDITORIAL,
            Self::Rebase => "rebase",
            Self::Tests => "tests",
        }
    }
}

/// The nodes a test run's evidence commit names before ` and <n> more`.
const SUBJECT_NODES: usize = 3;

/// What a lock commit is of: its message's subject line and `Lock:`
/// trailer, its third check, the words of a refusal when the lock changed
/// meanwhile, and the rows the owner is shown.
#[derive(Debug, Clone, Copy)]
pub(crate) enum LockSubject<'a> {
    /// An accept, an editorial refresh, a gone node's drop: `spec: lock
    /// <node>`, `Lock: <node>`, checked for that trailer; the outcome's
    /// rows.
    Node(&'a str),
    /// A rebase: `spec: lock rebase`, no `Lock:` trailer, checked for
    /// `Lock-mode: rebase` and no `Lock:`; its own rows' text.
    Rebase { rows: &'a str },
    /// A test run's evidence (task spec `tested-evidence`, "The commit"):
    /// `spec: verify tests A, B, C and <n> more` over `nodes` (those whose
    /// block changed, by bytes), a `Lock:` per node, checked for each and
    /// for `Lock-mode: tests`; its own rows' text.
    Tests { nodes: &'a [String], rows: &'a str },
}

impl LockSubject<'_> {
    /// The message up to its `Lock-mode:` trailer: the subject line, the
    /// empty line, an accept's `Lock:` trailer (a test run's, one per
    /// node).
    fn message_head(&self) -> String {
        match self {
            Self::Node(node) => format!("spec: lock {node}\n\n{LOCK_TRAILER}: {node}\n"),
            Self::Rebase { .. } => "spec: lock rebase\n\n".to_owned(),
            Self::Tests { nodes, .. } => {
                let mut named = nodes
                    .iter()
                    .take(SUBJECT_NODES)
                    .map(|node| one_line(node))
                    .collect::<Vec<_>>()
                    .join(", ");
                if nodes.len() > SUBJECT_NODES {
                    named.push_str(&format!(" and {} more", nodes.len() - SUBJECT_NODES));
                }
                let mut head = format!("spec: verify tests {named}\n\n");
                for node in *nodes {
                    head.push_str(&format!("{LOCK_TRAILER}: {}\n", one_line(node)));
                }
                head
            }
        }
    }

    /// The write's participle in "changed while being …".
    const fn being(&self) -> &'static str {
        match self {
            Self::Node(_) => "accepted",
            Self::Rebase { .. } => "rebased",
            Self::Tests { .. } => "written",
        }
    }
}

/// What a run left written (a `drift` item's approve maps it, never a
/// text): nothing, a verified commit, the bytes put back, a commit made but
/// not verified, or the bytes left as written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LockWritten {
    /// Nothing written: refused, not confirmed, or the write failed.
    #[default]
    None,
    /// A verified commit (exit 0).
    Committed,
    /// Written, its commit failed or kept nothing on the branch, and its
    /// bytes put back (`Restored::Back`).
    Restored,
    /// A commit made that fails a check or cannot be verified.
    Unverified,
    /// Written and not put back (changed meanwhile, a restore step failed,
    /// or nothing tells whether a commit was made).
    Left,
}

/// One row: `<change> | <relation> | <file> | <kind> <symbol>[
/// <item_kind>][ | <state>]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LockRow {
    /// `new`, `refreshed`, `unchanged`, `kept`, `dropped`, `not locked`.
    pub change: &'static str,
    pub relation: &'static str,
    pub file: String,
    /// `item`, `module`, `ron`; a binding not locked: `unanchored`,
    /// `cannot_verify`.
    pub kind: String,
    /// The `qpath` or field path; a binding not locked: its category.
    pub symbol: String,
    pub item_kind: Option<String>,
    /// Why it is not locked (`kept`, `not locked`).
    pub state: Option<String>,
}

/// What `spec lock accept` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LockOutcome {
    /// `REF` as given.
    pub reference: String,
    /// The canonical ID whose entries changed.
    pub node: Option<String>,
    /// Its revision at `HEAD` (`None`: a gone node's).
    pub rev: Option<u32>,
    pub mode: LockMode,
    /// In key order.
    pub rows: Vec<LockRow>,
    /// The node's `[[tested]]` block's row, after the entries' (`<change>
    /// | tested`): accept `kept`, `--editorial` `refreshed` or `unchanged`,
    /// a gone node `dropped`; `None` without one.
    pub tested: Option<&'static str>,
    /// The lock's commit, when one was made and named: also when it fails
    /// step 10's check or cannot be verified (exit 1, the refusal naming
    /// it).
    pub commit: Option<String>,
    /// Exit 0 with nothing written: nothing to change.
    pub unchanged: bool,
    /// Why nothing was written (exit 1).
    pub refusal: Option<String>,
    #[serde(skip)]
    pub messages: Vec<Message>,
    /// The rows' text still to print with the outcome's; `None` once the
    /// consent prompt showed it.
    #[serde(skip)]
    pub report: Option<String>,
    /// What the run left written (not in `--json`: its nine keys stay).
    #[serde(skip)]
    pub written: LockWritten,
    /// Step 7: nothing to change, and why (every binding locked as it is,
    /// none lockable, every entry at R holding its hash): no question asked
    /// (not in `--json`).
    #[serde(skip)]
    pub nothing: Option<String>,
}

impl LockOutcome {
    fn new(request: &LockAcceptRequest, mode: LockMode) -> Self {
        Self::blank(request.reference.clone(), mode)
    }

    /// The outcome a rebase's commit runs under ([`crate::lock_rebase`]
    /// maps what it gets back): no reference, node or rows.
    pub(crate) fn of_rebase() -> Self {
        Self::blank(String::new(), LockMode::Rebase)
    }

    /// The outcome a test run's evidence commit runs under
    /// ([`crate::verify_tests`] maps what it gets back).
    pub(crate) fn of_tests() -> Self {
        Self::blank(String::new(), LockMode::Tests)
    }

    fn blank(reference: String, mode: LockMode) -> Self {
        Self {
            reference,
            node: None,
            rev: None,
            mode,
            rows: Vec::new(),
            tested: None,
            commit: None,
            unchanged: false,
            refusal: None,
            messages: Vec::new(),
            report: None,
            written: LockWritten::None,
            nothing: None,
        }
    }

    /// Exit 1 with `reason`, nothing written.
    fn refused(mut self, reason: &str) -> Self {
        self.refusal = Some(one_line(reason));
        self
    }

    /// 1 when refused, else 0.
    pub fn exit(&self) -> Exit {
        if self.refusal.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }
}

/// The recipes of this build: `norm_recipe` the spec side's,
/// `code_recipe` `<LEVEL_RECIPE> tree-sitter-rust/<version> abi/<n>`
/// (format 2's levels).
pub(crate) fn build_recipes() -> Recipes {
    let grammar = grammar_info();
    Recipes {
        norm: NORM_RECIPE.to_owned(),
        code: format!(
            "{LEVEL_RECIPE} tree-sitter-rust/{} abi/{}",
            grammar.tree_sitter_rust, grammar.abi
        ),
    }
}

/// The repository the lock is committed in.
pub(crate) struct Repo {
    /// Git at the worktree's top: every path given or printed is
    /// top-relative.
    pub git: WorktreeGit,
    /// The worktree's top, canonical.
    pub worktree: String,
    /// The project root inside it, `""` at its top.
    pub root_rel: String,
    pub branch: String,
    pub head: String,
}

impl Repo {
    /// Git in the worktree holding `project`: `HEAD` born, on a branch,
    /// no operation in progress (exit 2 otherwise).
    pub(crate) fn open(project: &ProjectRoot, env: &GitEnv) -> Result<Self, CliError> {
        let at_root = WorktreeGit::new(&project.root, env)
            .map_err(|error| CliError::spec(format!("cannot run git: {error}")))?;
        let place = at_root.place(&project.root).map_err(|error| match error {
            PlaceError::Git(error) => CliError::spec(format!(
                "the project root {} lies in no git worktree ({error}): spec.lock is committed \
                 there",
                project.root.display()
            )),
            other => CliError::spec(format!("cannot lock: {other}; nothing written")),
        })?;
        let git = WorktreeGit::new(&place.worktree, env)
            .map_err(|error| CliError::spec(format!("cannot run git: {error}")))?;
        if let Some(operation) = git.operation_in_progress().map_err(git_error)? {
            return Err(CliError::spec(format!(
                "a {operation} is in progress in {}: finish or abort it first; nothing written",
                place.worktree
            )));
        }
        Ok(Self {
            git,
            worktree: place.worktree,
            root_rel: place.root_rel,
            branch: place.branch,
            head: place.base_commit,
        })
    }

    /// `path` (root-relative) relative to the worktree's top.
    pub fn top(&self, path: &str) -> String {
        if self.root_rel.is_empty() {
            path.to_owned()
        } else {
            format!("{}/{path}", self.root_rel)
        }
    }

    /// A top-relative path relative to the root; `None` outside it.
    fn root_relative<'p>(&self, top: &'p str) -> Option<&'p str> {
        if self.root_rel.is_empty() {
            Some(top)
        } else {
            top.strip_prefix(self.root_rel.as_str())?.strip_prefix('/')
        }
    }

    /// The committed bytes of the root-relative `path` at `HEAD`.
    fn blob(&self, path: &str) -> Result<Vec<u8>, CliError> {
        self.git
            .blob_at(&self.head, &self.top(path))
            .map_err(|error| CliError::spec(format!("cannot read `{path}` at HEAD: {error}")))
    }

    /// The blob OID of the root-relative `path` at `HEAD`; none: exit 2.
    fn blob_oid(&self, path: &str) -> Result<String, CliError> {
        self.git
            .blob_oid(&self.head, &self.top(path))
            .map_err(git_error)?
            .ok_or_else(|| CliError::spec(format!("`{path}` is no file at HEAD")))
    }
}

/// A git failure: exit 2.
pub(crate) fn git_error(error: specengine_store::GitError) -> CliError {
    CliError::spec(format!("git: {error}"))
}

/// The lock as read from the worktree: its bytes (`None`: no file) and
/// its entries.
pub(crate) struct ReadLock {
    pub bytes: Option<Vec<u8>>,
    pub lock: Lock,
}

/// `<root>/spec.lock` read and parsed against this build's recipes (an
/// absent file: an empty lock); exit 2 when it cannot be read, is format 1
/// or of another recipe (core's text, naming `spec lock rebase` but for
/// another `norm_recipe`).
pub(crate) fn read_lock(root: &Path, recipes: &Recipes) -> Result<ReadLock, CliError> {
    let bytes = read_lock_bytes(root)?;
    let lock = match &bytes {
        None => Lock::empty(recipes.clone()),
        Some(bytes) => {
            let text = std::str::from_utf8(bytes).map_err(|_| {
                CliError::cannot(format!("{LOCK_FILE}: not UTF-8: SpecEngine writes it"))
            })?;
            parse_lock(text, Some(recipes)).map_err(|error| CliError::cannot(error.to_string()))?
        }
    };
    Ok(ReadLock { bytes, lock })
}

/// The lock file's bytes; `None` when there is none.
pub(crate) fn read_lock_bytes(root: &Path) -> Result<Option<Vec<u8>>, CliError> {
    match fs::read(root.join(LOCK_FILE)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(CliError::spec(format!("cannot read {LOCK_FILE}: {error}"))),
    }
}

/// `spec lock accept REF [--editorial]` (see the module documentation).
pub fn lock_accept(
    env: &Env,
    globals: &Globals,
    request: &LockAcceptRequest,
    consent: LockConsent<'_>,
    trailers: &[LockTrailer],
) -> Result<LockOutcome, CliError> {
    let mode = if request.editorial {
        LockMode::Editorial
    } else {
        LockMode::Accept
    };
    let mut outcome = LockOutcome::new(request, mode);

    // The project, its config, `[code]`; `REF`.
    let (project, text) = discover_with_text(env, globals)?;
    require_root_config(env, globals, &project)?;
    project.slug()?;
    let code = code_config(&project, &text)?;
    let written = request.reference.trim();
    let scheme = &project.config.scheme;
    let target = classify(written, scheme, &mut outcome.messages, REV_IGNORED)?;
    let input = indexed(env, &project, &mut outcome.messages, false)?;
    let mut corpus = Corpus::new(&input, scheme, &project.config.paths);
    let named = match &target {
        Ok(target) => corpus.target_nodes(target, written)?,
        Err(reason) => Err(reason.clone()),
    };
    if let Ok(nodes) = &named
        && nodes.len() > 1
    {
        let reason = format!(
            "`{written}` names {} nodes: name one (`slug/ID`, `ID#SECTION` or its file)",
            nodes.len()
        );
        return Ok(outcome.refused(&reason));
    }

    // Git and the lock; a drift item's approve on the branch its place was
    // checked on.
    let repo = Repo::open(&project, &request.git)?;
    if let Some(branch) = &request.branch
        && *branch != repo.branch
    {
        let whose = request
            .proposal
            .as_deref()
            .map_or_else(|| "the caller's".to_owned(), |id| format!("{id}'s"));
        let reason =
            format!("HEAD left `{branch}` after {whose} place was checked; nothing written");
        return Ok(outcome.refused(&reason));
    }
    // `Proposal:` after `Decided-by:`, then the caller's: each checked at
    // step 10 with its value.
    let trailers: Vec<LockTrailer> = request
        .proposal
        .iter()
        .map(|id| LockTrailer {
            key: PROPOSAL_TRAILER.to_owned(),
            value: id.clone(),
        })
        .chain(trailers.iter().cloned())
        .collect();
    let trailers = trailers.as_slice();
    let proposal = request.proposal.as_deref();
    let recipes = build_recipes();
    let read = read_lock(&project.root, &recipes)?;
    let gone_id = without_rev(written);
    // A block alone makes a node gone too.
    let gone =
        read.lock.holds(gone_id) && !canonical_ids(&input, scheme, &project).contains(gone_id);
    if gone {
        // Gone only when `HEAD` agrees: a node deleted from the working
        // tree alone is an uncommitted change of its file.
        let held = head_holders(&repo, &project, gone_id)?;
        if !held.is_empty() {
            return Ok(outcome.refused(&dirty_reason(&held)));
        }
        if request.editorial {
            let plain = match proposal {
                Some(id) => format!("spec approve {id}"),
                None => format!("spec lock accept {gone_id}"),
            };
            let reason = format!(
                "`{gone_id}`: no node has this ID any more; `{plain}` without --editorial drops \
                 its entries"
            );
            return Ok(outcome.refused(&reason));
        }
        outcome.mode = LockMode::Gone;
        return drop_gone(
            env, &project, &repo, read, gone_id, proposal, outcome, consent, trailers,
        );
    }
    let nodes = match named {
        Ok(nodes) => nodes,
        Err(reason) => return Ok(outcome.refused(&reason)),
    };
    let key = nodes[0];
    let node_path = corpus.resolver.paths()[key.file].to_owned();
    let Some(node_id) = corpus.files[key.file]
        .parsed
        .as_ref()
        .and_then(|parsed| parsed.nodes.get(key.ord))
        .and_then(|node| node.id.clone())
    else {
        let reason = format!("`{written}`: the node has no ID, and spec.lock records nodes by ID");
        return Ok(outcome.refused(&reason));
    };
    let canonical = canonical_id(&corpus.resolver, scheme, &node_path, &node_id, None);
    outcome.node = Some(canonical.clone());
    let node = NodeAt {
        key,
        path: node_path,
        id: node_id,
        canonical,
    };
    if request.editorial {
        editorial(
            env, &project, &repo, read, &node, proposal, outcome, consent, trailers,
        )
    } else {
        accept(
            env,
            &project,
            &code,
            &repo,
            read,
            &mut corpus,
            &node,
            request,
            outcome,
            consent,
            trailers,
        )
    }
}

/// The node `REF` names.
pub(crate) struct NodeAt {
    pub(crate) key: NodeKey,
    /// Its file, root-relative.
    pub(crate) path: String,
    /// Its own ID.
    pub(crate) id: String,
    /// The canonical cited ID (`slug/ID` when feature-scoped).
    pub(crate) canonical: String,
}

/// `written` without a trailing `@rev` (1 to 9 digits), which `REF`'s read
/// ignores: the canonical ID a gone node's entries carry.
pub(crate) fn without_rev(written: &str) -> &str {
    match written.rsplit_once('@') {
        Some((id, rev))
            if !id.is_empty()
                && (1..=9).contains(&rev.len())
                && rev.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            id
        }
        _ => written,
    }
}

/// The spec files (`.md` under the root) that differ from `HEAD` and whose
/// `HEAD` blob still holds a node of the canonical ID `id`, each with its
/// state: the node is not gone at `HEAD`.
fn head_holders(
    repo: &Repo,
    project: &ProjectRoot,
    id: &str,
) -> Result<Vec<(String, &'static str)>, CliError> {
    let root = if repo.root_rel.is_empty() {
        ".".to_owned()
    } else {
        repo.root_rel.clone()
    };
    let entries = repo
        .git
        .status_entries(&[root.as_str()], false)
        .map_err(git_error)?;
    let scheme = &project.config.scheme;
    let mut held = Vec::new();
    for entry in &entries {
        let Some(path) = repo.root_relative(&entry.path) else {
            continue;
        };
        if !path.ends_with(DOCUMENT_EXTENSION)
            || repo
                .git
                .blob_oid(&repo.head, &entry.path)
                .map_err(git_error)?
                .is_none()
        {
            continue;
        }
        let bytes = repo.blob(path)?;
        // Unread is never "gone": exit 2, as the node's own file at HEAD.
        let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
            specengine_core::parse(path, &bytes, scheme)
        }))
        .map_err(|_| CliError::spec(format!("the spec parser failed on `{path}` at HEAD")))?;
        let holds = parsed
            .nodes
            .iter()
            .filter_map(|node| node.id.as_deref())
            .any(
                |own| match feature_stem(&project.config.paths.features, path) {
                    Some(slug) if feature_scoped(scheme, own) => {
                        id.strip_prefix(slug)
                            .and_then(|rest| rest.strip_prefix('/'))
                            == Some(own)
                    }
                    _ => own == id,
                },
            );
        if holds {
            held.push((path.to_owned(), state_of(entry)));
        }
    }
    Ok(held)
}

/// Every canonical ID a node of the corpus has.
pub(crate) fn canonical_ids(
    input: &CheckInput,
    scheme: &IdScheme,
    project: &ProjectRoot,
) -> BTreeSet<String> {
    let resolver = Resolver::new(input, scheme, &project.config.paths);
    let mut ids = BTreeSet::new();
    for file in &input.files {
        let Some(parsed) = &file.parsed else {
            continue;
        };
        for node in &parsed.nodes {
            if let Some(id) = &node.id {
                ids.insert(canonical_id(&resolver, scheme, &file.path, id, None));
            }
        }
    }
    ids
}

// ---------------------------------------------------------------- accept

/// One marker of the node, as its file's committed bytes bind it.
struct Found {
    relation: LockRelation,
    /// The first of the rows' states that refuses it (`id_state`,
    /// `rev_state`, its level list), `None` when all allow.
    refused: Option<String>,
    target: Target,
    /// Its effective levels (a marker without a list: `sig`, `body`);
    /// empty for a malformed list, which [`Found::refused`] names.
    levels: BTreeSet<LockLevel>,
    /// Its comment's byte range in the file: a RON value's hash is paired
    /// with its binding by it.
    comment: Range<usize>,
}

/// What a marker binds, and the lock key it gives: the one key builder of
/// `spec lock accept` and `spec verify`
/// (canon `spec-verify`, "Rows": rows keyed as accept keys entries).
pub(crate) enum Target {
    /// An item (its index), named.
    Item {
        index: usize,
        qpath: String,
        item_kind: String,
        ambiguity: Option<&'static str>,
    },
    /// A module, named: the file's (`item` none) or an inline `mod`'s (its
    /// `mod_item`'s index, hashed as that item).
    Module {
        item: Option<usize>,
        qpath: String,
        ambiguity: Option<&'static str>,
    },
    /// A RON value by its field path.
    Ron {
        path: String,
        ambiguity: Option<&'static str>,
    },
    /// No key: unanchored or `cannot_verify`, with the category.
    Keyless {
        state: &'static str,
        category: &'static str,
    },
}

impl Target {
    /// What a Rust comment's `binding` binds: `analysis` the bound file's
    /// items, `names` the walk's names of those items, `module` the file's
    /// module's.
    pub(crate) fn of_rust(
        binding: Binding,
        analysis: &FileAnalysis,
        names: &[Name],
        module: Option<&Name>,
    ) -> Self {
        let ambiguity = |name: Option<&Name>| {
            name.and_then(|name| name.ambiguity.map(|ambiguity| ambiguity.as_str()))
        };
        match binding {
            Binding::Item(item) => {
                let name = names.get(item);
                Self::Item {
                    index: item,
                    qpath: name.map(|name| name.qpath.clone()).unwrap_or_default(),
                    item_kind: analysis
                        .items
                        .get(item)
                        .map(|record| record.kind.clone())
                        .unwrap_or_default(),
                    ambiguity: ambiguity(name),
                }
            }
            Binding::Module(None) => Self::Module {
                item: None,
                qpath: module.map(|name| name.qpath.clone()).unwrap_or_default(),
                ambiguity: ambiguity(module),
            },
            Binding::Module(Some(item)) => {
                let name = names.get(item);
                Self::Module {
                    item: Some(item),
                    qpath: name.map(|name| name.qpath.clone()).unwrap_or_default(),
                    ambiguity: ambiguity(name),
                }
            }
            Binding::Unanchored(category) => Self::Keyless {
                state: UNANCHORED,
                category: category.as_str(),
            },
            Binding::CannotVerify(category) => Self::Keyless {
                state: CANNOT_VERIFY,
                category: category.as_str(),
            },
            Binding::File => Self::Keyless {
                state: UNANCHORED,
                category: "",
            },
        }
    }

    /// What a RON comment's `anchor` binds.
    pub(crate) fn of_ron(anchor: &Anchor) -> Self {
        match anchor {
            Anchor::Path { path, .. } => Self::Ron {
                path: path.clone(),
                ambiguity: None,
            },
            Anchor::Ambiguous { path, .. } => Self::Ron {
                path: path.clone(),
                ambiguity: Some(RON_DUPLICATE),
            },
            Anchor::Unanchored => Self::Keyless {
                state: UNANCHORED,
                category: RON_NO_VALUE,
            },
            Anchor::CannotVerify => Self::Keyless {
                state: CANNOT_VERIFY,
                category: RON_TOO_DEEP,
            },
        }
    }

    /// The binding's ambiguity, if it has a key.
    pub(crate) fn ambiguity(&self) -> Option<&'static str> {
        match self {
            Self::Item { ambiguity, .. }
            | Self::Module { ambiguity, .. }
            | Self::Ron { ambiguity, .. } => *ambiguity,
            Self::Keyless { .. } => None,
        }
    }

    /// The lock key a marker of `node` (the canonical cited ID) with
    /// `relation` in `file` gives; `Err`: no key, the keyless binding's row
    /// place (kind its state, symbol its category).
    pub(crate) fn key(
        &self,
        node: &str,
        relation: LockRelation,
        file: &str,
    ) -> Result<EntryKey, RowPlace> {
        let key = |kind: LockKind, symbol: &str, item_kind: Option<&str>| EntryKey {
            node: node.to_owned(),
            relation,
            file: file.to_owned(),
            kind,
            symbol: symbol.to_owned(),
            item_kind: item_kind.map(str::to_owned),
        };
        match self {
            Self::Item {
                qpath, item_kind, ..
            } => Ok(key(LockKind::Item, qpath, Some(item_kind))),
            Self::Module { qpath, .. } => Ok(key(LockKind::Module, qpath, None)),
            Self::Ron { path, .. } => Ok(key(LockKind::Ron, path, None)),
            Self::Keyless { state, category } => Err(RowPlace {
                relation,
                file: file.to_owned(),
                kind: (*state).to_owned(),
                symbol: (*category).to_owned(),
                item_kind: None,
            }),
        }
    }
}

/// `spec lock accept` without `--editorial`.
#[allow(clippy::too_many_arguments)]
fn accept(
    env: &Env,
    project: &ProjectRoot,
    code: &CodeConfig,
    repo: &Repo,
    read: ReadLock,
    corpus: &mut Corpus<'_>,
    node: &NodeAt,
    request: &LockAcceptRequest,
    mut outcome: LockOutcome,
    consent: LockConsent<'_>,
    trailers: &[LockTrailer],
) -> Result<LockOutcome, CliError> {
    let scheme = &project.config.scheme;
    // The walk finds the node's markers.
    let tree = read_tree(&project.root, code, &mut outcome.messages)?;
    let mut marker_files: BTreeSet<String> = BTreeSet::new();
    for file in &tree.rust {
        let marks = file.binding.comments.iter().any(|comment| {
            file.text
                .get(comment.range.clone())
                .is_some_and(|text| marks_node(text, corpus, node.key))
        });
        if marks {
            marker_files.insert(file.path.clone());
        }
    }
    for file in &tree.ron {
        let marks = file.anchors.iter().any(|anchor| {
            file.text
                .get(anchor.comment.clone())
                .is_some_and(|text| marks_node(text, corpus, node.key))
        });
        if marks {
            marker_files.insert(file.path.clone());
        }
    }

    // The inputs, tracked and clean.
    let mut inputs: BTreeSet<String> = marker_files.clone();
    inputs.insert(node.path.clone());
    inputs.insert(CONFIG_FILE.to_owned());
    inputs.insert(LOCK_FILE.to_owned());
    let mut dirty = dirty_inputs(repo, &inputs, read.bytes.is_some())?;
    dirty.extend(
        changed_code(repo, code)?
            .into_iter()
            .filter(|(path, _)| !inputs.contains(path)),
    );
    dirty.sort();
    if !dirty.is_empty() {
        return Ok(outcome.refused(&dirty_reason(&dirty)));
    }

    // The node at `HEAD`.
    let (rev, spec_hash) = match node_at_head(repo, scheme, node)? {
        Ok(found) => found,
        Err(reason) => return Ok(outcome.refused(&reason)),
    };
    outcome.rev = Some(rev);

    // Each marker file's committed bytes, bound and hashed.
    let today = request.now.get(..10).unwrap_or(&request.now).to_owned();
    let facts = Facts {
        rev,
        spec_hash: &spec_hash,
        commit: &repo.head,
        today: &today,
    };
    let Bindings {
        fresh,
        unlockable,
        keyless,
        any_marker,
    } = committed_bindings(repo, &tree, &marker_files, corpus, node, &read.lock, &facts)?;

    // The merge.
    let merge_input = AcceptInput {
        node: node.canonical.clone(),
        rev,
        spec_hash: spec_hash.clone(),
        fresh: fresh.into_iter().collect(),
        unlockable: unlockable.into_iter().collect(),
        keyless: keyless.into_iter().collect(),
    };
    let merge = match merge_accept(&read.lock, &merge_input) {
        Ok(merge) => merge,
        Err(refusal) => {
            // A drift item's approve names its own way (canon
            // `proposal-drift`).
            let editorial = match &request.proposal {
                Some(id) => format!("spec approve {id} --editorial"),
                None => format!("spec lock accept {} --editorial", node.canonical),
            };
            // What vouched for the old text: an entry, or the node's
            // `[[tested]]` block.
            let vouched = match refusal {
                AcceptRefusal::TextChanged {
                    key,
                    locked,
                    current,
                } => format!(
                    "{} {} `{}`: spec_hash {locked}, now {current}",
                    key.relation.as_str(),
                    key.kind.as_str(),
                    key.symbol,
                ),
                AcceptRefusal::TestedChanged { locked, current } => {
                    format!("[[tested]]: spec_hash {locked}, now {current}")
                }
            };
            let reason = format!(
                "`{}` rev {rev}: its text changed since spec.lock vouched for it at this revision \
                 ({vouched}): raise its `rev` by hand, commit, then accept; or `--editorial` for \
                 an editorial change (`{editorial}`); nothing written",
                node.canonical,
            );
            return Ok(outcome.refused(&reason));
        }
    };
    outcome.rows = merge.rows.iter().map(accept_row).collect();
    outcome.tested = merge.tested.map(Change::as_str);
    if !merge.writes() {
        let rows = outcome_rows(&outcome);
        outcome.report = Some(rows);
        if merge.count(Change::Unchanged) > 0 {
            outcome.nothing = Some("every lockable binding is locked as it is".to_owned());
            outcome.unchanged = true;
            return Ok(outcome);
        }
        let why = if any_marker {
            format!(
                "none of `{}`'s bindings can be locked (the rows say why)",
                node.canonical
            )
        } else {
            format!("no marker names `{}` in the [code] tree", node.canonical)
        };
        let reason = format!("{why}; nothing written");
        outcome.nothing = Some(why);
        return Ok(outcome.refused(&reason));
    }
    let question = format!(
        "lock {} rev {rev} (accept: {} new, {} refreshed, {} kept, {} dropped){} at {} on {} in \
         {}? [y/N]",
        node.canonical,
        merge.count(Change::New),
        merge.count(Change::Refreshed),
        merge.count(Change::Kept),
        merge.count(Change::Dropped),
        for_item(request.proposal.as_deref()),
        repo.head,
        repo.branch,
        repo.worktree
    );
    let message_node = node.canonical.clone();
    write_and_commit(
        env,
        project,
        repo,
        &read,
        &merge.lock,
        &LockSubject::Node(&message_node),
        question,
        outcome,
        consent,
        trailers,
    )
}

/// The node's bindings over its marker files' committed bytes, sorted by
/// what they allow.
#[derive(Default)]
pub(crate) struct Bindings {
    /// Lockable, one per key.
    pub(crate) fresh: BTreeMap<EntryKey, Verified>,
    /// Bound with a key, not lockable: the first refusal of its markers.
    pub(crate) unlockable: BTreeMap<EntryKey, String>,
    /// Bound with no key, and the state.
    pub(crate) keyless: BTreeSet<(RowPlace, String)>,
    /// Some marker names the node.
    pub(crate) any_marker: bool,
}

/// What every fresh entry of one accept shares.
pub(crate) struct Facts<'a> {
    pub(crate) rev: u32,
    pub(crate) spec_hash: &'a str,
    pub(crate) commit: &'a str,
    pub(crate) today: &'a str,
}

/// Each marker file read from `HEAD` (`blob_at`, no filter), bound (its
/// items checked against the walk's, whose names they take), its markers
/// of the node sorted ("Lockability": `id_state`, `rev_state`, the level
/// list, the binding), what lockable markers bind hashed and no more: an
/// item's levels (`item_levels`, an inline `mod` as its `mod_item`), the
/// file's module's (`module_levels`), a RON file's value hashes paired
/// with their comments by range (task spec `lock-levels`, "Accept").
/// `spec verify --tests` calls it as accept's own predicate (canon
/// `spec-verify-tests`, "Runnable and verifiable"); its fresh entries are
/// what the run's evidence hashes (task spec `tested-evidence`, "Bound
/// hash"). [`node_bindings`] over [`Source::Head`].
pub(crate) fn committed_bindings(
    repo: &Repo,
    tree: &CodeTree,
    marker_files: &BTreeSet<String>,
    corpus: &mut Corpus<'_>,
    node: &NodeAt,
    old: &Lock,
    facts: &Facts<'_>,
) -> Result<Bindings, CliError> {
    let one = [NodeFiles {
        node,
        facts,
        files: marker_files,
    }];
    let bindings = node_bindings(&Source::Head(repo), tree, &one, corpus, old)?;
    Ok(bindings.into_iter().next().unwrap_or_default())
}

/// Where [`node_bindings`] reads a marker file.
pub(crate) enum Source<'a> {
    /// Each file's `HEAD` blob (`blob_at`, no filter), bound afresh and its
    /// items checked against the walk's: accept, `spec verify --tests`.
    Head(&'a Repo),
    /// The working tree as walked: each file's text and binding the
    /// walk's, `blobs` its blob OIDs on disk (`hash-object`, every marker
    /// file hashed). A lockable binding whose entry in the old lock holds
    /// that blob takes the entry's digests (verify's cascade step 2); a
    /// file whose every lockable binding does is never parsed. A read's
    /// bound hash (task spec `tested-evidence`, "Bound hash").
    Disk { blobs: &'a BTreeMap<String, String> },
}

/// One node whose bindings [`node_bindings`] sorts: the node, what each
/// fresh entry of it shares (`rev` the revision its markers are judged
/// against), its marker files.
pub(crate) struct NodeFiles<'a> {
    pub(crate) node: &'a NodeAt,
    pub(crate) facts: &'a Facts<'a>,
    pub(crate) files: &'a BTreeSet<String>,
}

/// [`committed_bindings`] over either [`Source`] and any number of nodes,
/// one [`Bindings`] each, in `nodes`' order: the same markers sorted by
/// the same rows, hashed by the same functions. Each marker file is read
/// and bound once, its units of every node hashed in one pass (each Rust
/// file parsed at most once, by one parser): a node's result is the one a
/// call for it alone gives.
pub(crate) fn node_bindings(
    source: &Source<'_>,
    tree: &CodeTree,
    nodes: &[NodeFiles<'_>],
    corpus: &mut Corpus<'_>,
    old: &Lock,
) -> Result<Vec<Bindings>, CliError> {
    let file_index: BTreeMap<&str, usize> = tree
        .rust
        .iter()
        .enumerate()
        .map(|(index, file)| (file.path.as_str(), index))
        .collect();
    let ron_index: BTreeMap<&str, usize> = tree
        .ron
        .iter()
        .enumerate()
        .map(|(index, file)| (file.path.as_str(), index))
        .collect();
    let mut parser: Option<RustParser> = None;
    let mut all: Vec<Bindings> = nodes.iter().map(|_| Bindings::default()).collect();
    let paths: BTreeSet<&String> = nodes.iter().flat_map(|node| node.files.iter()).collect();
    for path in paths {
        // The nodes this file holds a marker of, by their place in `nodes`.
        let holders: Vec<usize> = nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.files.contains(path))
            .map(|(at, _)| at)
            .collect();
        // The file's text and blob: `HEAD`'s, or the walk's and the disk's.
        let head_text: String;
        let (text, blob): (&str, String) = match source {
            Source::Head(repo) => {
                let bytes = repo.blob(path)?;
                head_text = String::from_utf8(bytes).map_err(|_| {
                    CliError::spec(format!(
                        "`{path}` at HEAD is not UTF-8 (a git filter?): its committed bytes \
                         cannot be bound"
                    ))
                })?;
                (head_text.as_str(), repo.blob_oid(path)?)
            }
            Source::Disk { blobs } => {
                let walked = file_index
                    .get(path.as_str())
                    .map(|&index| tree.rust[index].text.as_str())
                    .or_else(|| {
                        ron_index
                            .get(path.as_str())
                            .map(|&index| tree.ron[index].text.as_str())
                    });
                match (walked, blobs.get(path)) {
                    (Some(text), Some(blob)) => (text, blob.clone()),
                    (walked, blob) => {
                        // Unreachable: a read's marker files are files of the
                        // walk, and it hashes each of them.
                        debug_assert!(
                            walked.is_some() && blob.is_some(),
                            "`{path}`: a marker file with no walked text or no blob"
                        );
                        continue;
                    }
                }
            }
        };
        // Each holder's markers in the file, in `holders`' order.
        let mut found: Vec<Vec<Found>> = holders.iter().map(|_| Vec::new()).collect();
        let mut codes = FileCodes::default();
        // The keys of this file's items with a parse error: a marker on
        // one binds `cannot_verify`, and the old entry of its key is kept.
        let mut broken: Vec<(LockKind, String, Option<String>)> = Vec::new();
        if path.ends_with(RUST_EXTENSION) {
            let Some(&index) = file_index.get(path.as_str()) else {
                continue;
            };
            let walked = &tree.rust[index];
            let head_binding: RustBinding;
            let (binding, mut parsed) = match source {
                Source::Head(_) => {
                    let parsed = grammar(&mut parser)?.parse(text).ok_or_else(|| {
                        CliError::spec(format!(
                            "the code walk is incomplete: `{path}` at HEAD cannot be parsed (the \
                             parser gave no tree)"
                        ))
                    })?;
                    head_binding = bind_tree(&parsed, text);
                    if !same_items(&head_binding, &walked.binding) {
                        return Err(CliError::spec(format!(
                            "`{path}`: its bytes at HEAD and in the working tree give other items \
                             (a git filter?): cannot name them; nothing written"
                        )));
                    }
                    (&head_binding, Some(parsed))
                }
                Source::Disk { .. } => (&walked.binding, None),
            };
            let module = tree.names.modules.get(index);
            let names = tree.names.items.get(index).map_or(&[][..], Vec::as_slice);
            for (item, name) in binding.analysis.items.iter().zip(names) {
                if !item.has_error {
                    continue;
                }
                broken.push((LockKind::Item, name.qpath.clone(), Some(item.kind.clone())));
                if item.kind == MOD_KIND {
                    broken.push((LockKind::Module, name.qpath.clone(), None));
                }
            }
            for comment in &binding.comments {
                let Some(comment_text) = text.get(comment.range.clone()) else {
                    continue;
                };
                let target = || Target::of_rust(comment.binding, &binding.analysis, names, module);
                let at = CommentAt {
                    comment: comment_text,
                    range: comment.range.clone(),
                    target: &target,
                };
                for (slot, &held) in holders.iter().enumerate() {
                    let node = &nodes[held];
                    node_markers(&at, corpus, node.node.key, node.facts.rev, &mut found[slot]);
                }
            }
            if let Source::Disk { .. } = source {
                codes.reused = reused_codes(nodes, &holders, &found, path, &blob, old);
            }
            // Only what a lockable marker binds is hashed, every holder's
            // in one pass: an item's or an inline `mod`'s levels by its
            // index, the file's module's once (on disk, none an entry gives
            // at this blob).
            let shared = &codes;
            let lockable = || {
                holders.iter().zip(&found).flat_map(|(&held, found)| {
                    let canonical = nodes[held].node.canonical.as_str();
                    found.iter().filter(move |found| {
                        found.refused.is_none()
                            && found.target.ambiguity().is_none()
                            && !shared.reuses(found, canonical, path)
                    })
                })
            };
            let wanted: Vec<usize> = lockable()
                .filter_map(|found| match found.target {
                    Target::Item { index, .. }
                    | Target::Module {
                        item: Some(index), ..
                    } => Some(index),
                    _ => None,
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let module_wanted =
                lockable().any(|found| matches!(found.target, Target::Module { item: None, .. }));
            if (!wanted.is_empty() || module_wanted) && parsed.is_none() {
                parsed = grammar(&mut parser)?.parse(text);
            }
            // On disk the walk read it: no tree leaves its units unhashed.
            if let Some(parsed) = &parsed {
                let mut items = BTreeMap::new();
                for (index, levels) in wanted.iter().zip(item_levels(parsed, text, &wanted)) {
                    if let Some(levels) = levels {
                        items.insert(*index, levels);
                    }
                }
                codes.items = items;
                if module_wanted {
                    codes.module = Some(module_levels(parsed, text));
                }
            }
        } else if path.ends_with(RON_EXTENSION) {
            let head_anchors;
            let anchors = match source {
                Source::Head(_) => {
                    head_anchors = comment_anchors(text);
                    head_anchors.as_slice()
                }
                Source::Disk { .. } => match ron_index.get(path.as_str()) {
                    Some(&index) => tree.ron[index].anchors.as_slice(),
                    None => continue,
                },
            };
            for anchor in anchors {
                let Some(comment_text) = text.get(anchor.comment.clone()) else {
                    continue;
                };
                let target = || Target::of_ron(&anchor.anchor);
                let at = CommentAt {
                    comment: comment_text,
                    range: anchor.comment.clone(),
                    target: &target,
                };
                for (slot, &held) in holders.iter().enumerate() {
                    let node = &nodes[held];
                    node_markers(&at, corpus, node.node.key, node.facts.rev, &mut found[slot]);
                }
            }
            if let Source::Disk { .. } = source {
                codes.reused = reused_codes(nodes, &holders, &found, path, &blob, old);
            }
            let wanted = holders.iter().zip(&found).any(|(&held, found)| {
                let canonical = nodes[held].node.canonical.as_str();
                found.iter().any(|found| {
                    found.refused.is_none()
                        && matches!(
                            found.target,
                            Target::Ron {
                                ambiguity: None,
                                ..
                            }
                        )
                        && !codes.reuses(found, canonical, path)
                })
            });
            if wanted {
                codes.ron = Some(value_hashes(text));
            }
        }
        for (&held, found) in holders.iter().zip(found) {
            let node = &nodes[held];
            let Some(bindings) = all.get_mut(held) else {
                continue;
            };
            let canonical = node.node.canonical.as_str();
            bindings.any_marker |= !found.is_empty();
            keep_broken(&found, &broken, canonical, path, old, bindings);
            let pair = Pair {
                facts: node.facts,
                blob: &blob,
            };
            for found in found {
                classify_found(found, canonical, path, &pair, &codes, bindings);
            }
        }
    }
    for bindings in &mut all {
        let Bindings {
            fresh, unlockable, ..
        } = bindings;
        for key in unlockable.keys() {
            fresh.remove(key);
        }
    }
    Ok(all)
}

/// The Rust grammar, loaded once.
fn grammar(parser: &mut Option<RustParser>) -> Result<&mut RustParser, CliError> {
    if parser.is_none() {
        *parser = Some(RustParser::new().map_err(|error| {
            CliError::spec(format!("the Rust grammar cannot be loaded: {error}"))
        })?);
    }
    parser
        .as_mut()
        .ok_or_else(|| CliError::spec("the Rust grammar cannot be loaded"))
}

/// A file's items at `HEAD` and in the working tree are the same: kind,
/// label, module path and owner, in order.
fn same_items(committed: &RustBinding, worktree: &RustBinding) -> bool {
    committed.analysis.items.len() == worktree.analysis.items.len()
        && committed
            .analysis
            .items
            .iter()
            .zip(&worktree.analysis.items)
            .all(|(committed, worktree)| {
                (
                    &committed.kind,
                    &committed.label,
                    &committed.mod_path,
                    &committed.owner,
                ) == (
                    &worktree.kind,
                    &worktree.label,
                    &worktree.mod_path,
                    &worktree.owner,
                )
            })
}

/// On disk, the code of each keyed marker of each holder (`found` in step
/// with `holders`, places in `nodes`) whose entry in `old` holds the
/// file's `blob` (format 2's digests): its bytes are the ones the entry
/// was hashed from.
fn reused_codes(
    nodes: &[NodeFiles<'_>],
    holders: &[usize],
    found: &[Vec<Found>],
    path: &str,
    blob: &str,
    old: &Lock,
) -> BTreeMap<EntryKey, LevelCode> {
    let mut reused = BTreeMap::new();
    let marked = holders.iter().zip(found).flat_map(|(&held, found)| {
        found
            .iter()
            .map(move |found| (nodes[held].node.canonical.as_str(), found))
    });
    for (node, found) in marked {
        let Ok(key) = found.target.key(node, found.relation, path) else {
            continue;
        };
        if let Some(Verified {
            code: EntryCode::Levels(code),
            file_blob,
            ..
        }) = old.entries.get(&key)
            && file_blob == blob
        {
            reused.insert(key, code.clone());
        }
    }
    reused
}

/// A `cannot_verify` marker in a file keeps the old entries of the keys
/// of its items with a parse error, of the marker's relation: the item is
/// there, its hash cannot be made.
fn keep_broken(
    found: &[Found],
    broken: &[(LockKind, String, Option<String>)],
    node: &str,
    path: &str,
    old: &Lock,
    bindings: &mut Bindings,
) {
    for found in found {
        let Target::Keyless {
            state: CANNOT_VERIFY,
            category,
        } = found.target
        else {
            continue;
        };
        for (kind, symbol, item_kind) in broken {
            let key = EntryKey {
                node: node.to_owned(),
                relation: found.relation,
                file: path.to_owned(),
                kind: *kind,
                symbol: symbol.clone(),
                item_kind: item_kind.clone(),
            };
            if old.entries.contains_key(&key) {
                // The marker's own refusal (its ID, its revision) first, as
                // a keyed binding's row says.
                let state = found
                    .refused
                    .clone()
                    .unwrap_or_else(|| format!("{CANNOT_VERIFY} {category}"));
                bindings.unlockable.entry(key).or_insert(state);
            }
        }
    }
}

/// The `Verified` facts every fresh entry of one file shares.
struct Pair<'a> {
    facts: &'a Facts<'a>,
    blob: &'a str,
}

/// What one file's lockable bindings hashed to, by what they bind.
#[derive(Default)]
struct FileCodes {
    /// An item's levels, or an inline `mod`'s (its `mod_item`), by index.
    items: BTreeMap<usize, LevelState>,
    /// The file's module's levels.
    module: Option<LevelState>,
    /// A RON file's value hashes by comment range, or its refusal.
    ron: Option<RonValues>,
    /// On disk ([`Source::Disk`]): the code of each key whose entry holds
    /// the file's blob, taken instead of hashing.
    reused: BTreeMap<EntryKey, LevelCode>,
}

/// A lockable binding's digests: an item's or a module's levels and
/// fingerprints, or a RON value's hash.
pub(crate) enum Hashed<'a> {
    Levels(&'a LevelHashes),
    Ron(Digest),
}

impl FileCodes {
    /// `found`'s key takes an entry's code ([`FileCodes::reused`]).
    fn reuses(&self, found: &Found, node: &str, path: &str) -> bool {
        !self.reused.is_empty()
            && found
                .target
                .key(node, found.relation, path)
                .is_ok_and(|key| self.reused.contains_key(&key))
    }

    /// What `found` binds, hashed; `Err`: its `cannot_verify` state (a
    /// parse error in the item, or anywhere in a module's file; a RON file
    /// its analysis rejects).
    fn of(&self, found: &Found) -> Result<Hashed<'_>, String> {
        let refused = |category: Option<&str>| match category {
            Some(category) => format!("{CANNOT_VERIFY} {category}"),
            None => CANNOT_VERIFY.to_owned(),
        };
        let levels = match &found.target {
            Target::Item { index, .. }
            | Target::Module {
                item: Some(index), ..
            } => self.items.get(index),
            Target::Module { item: None, .. } => self.module.as_ref(),
            Target::Ron { .. } => {
                return match &self.ron {
                    Some(RonValues::Hashed(values)) => paired_value(values, &found.comment)
                        .map(Hashed::Ron)
                        .ok_or_else(|| refused(None)),
                    Some(RonValues::CannotVerify(category)) => Err(refused(Some(category))),
                    None => Err(refused(None)),
                };
            }
            Target::Keyless { .. } => None,
        };
        match levels {
            Some(LevelState::Hashed(levels)) => Ok(Hashed::Levels(levels)),
            Some(LevelState::CannotVerify { categories }) => Err(refused(
                categories.first().map(|category| category.as_str()),
            )),
            None => Err(refused(None)),
        }
    }
}

/// The digest `value_hashes` gives the RON value of the comment at
/// `comment`, paired by the comment's byte range (canon `code-hash-ron`,
/// "Entry point"); the first entry at that range; `None`: no entry has it.
/// Accept, verify and rebase pair by this one function.
pub(crate) fn paired_value(
    values: &[(Range<usize>, Digest)],
    comment: &Range<usize>,
) -> Option<Digest> {
    values
        .iter()
        .find(|(range, _)| range == comment)
        .map(|(_, digest)| *digest)
}

/// `b3:` and a digest's lowercase hex, as the lock writes it.
pub(crate) fn lock_digest(digest: &Digest) -> String {
    format!("{HASH_PREFIX}{}", digest.to_hex())
}

/// The lock's name of a marker's level.
pub(crate) fn lock_level(level: Level) -> LockLevel {
    match level {
        Level::Path => LockLevel::Path,
        Level::Sig => LockLevel::Sig,
        Level::Body => LockLevel::Body,
        Level::Deps => LockLevel::Deps,
    }
}

/// A format-2 entry's code: `levels` and every level's digest (a RON
/// value's `sig` and `body` its hash, its `deps` the empty set's, no
/// fingerprint; canon `code-hash-ron`, "RON value").
pub(crate) fn level_code(levels: BTreeSet<LockLevel>, hashed: &Hashed<'_>) -> LevelCode {
    match hashed {
        Hashed::Levels(hashes) => LevelCode {
            levels,
            sig_hash: lock_digest(&hashes.sig),
            body_hash: lock_digest(&hashes.body),
            deps_hash: lock_digest(&hashes.deps),
            fingerprints: Some(Fingerprints {
                skeleton_hash: lock_digest(&hashes.skeleton),
                quote_hash: lock_digest(&hashes.quote),
                body_bytes: hashes.body_bytes,
            }),
        },
        Hashed::Ron(value) => LevelCode {
            levels,
            sig_hash: lock_digest(value),
            body_hash: lock_digest(value),
            deps_hash: lock_digest(&empty_deps_digest()),
            fingerprints: None,
        },
    }
}

/// A marker of the node sorted into a fresh entry, a key not lockable or
/// a keyless binding. Markers giving one fresh key are one entry, its
/// `levels` the union of theirs.
fn classify_found(
    found: Found,
    node: &str,
    path: &str,
    pair: &Pair<'_>,
    codes: &FileCodes,
    bindings: &mut Bindings,
) {
    let key = match found.target.key(node, found.relation, path) {
        Ok(key) => key,
        Err(place) => {
            let state = found.refused.clone().unwrap_or_else(|| place.kind.clone());
            bindings.keyless.insert((place, state));
            return;
        }
    };
    let verified = |code: LevelCode| Verified {
        rev: pair.facts.rev,
        spec_hash: pair.facts.spec_hash.to_owned(),
        code: EntryCode::Levels(code),
        file_blob: pair.blob.to_owned(),
        commit: pair.facts.commit.to_owned(),
        verified_at: pair.facts.today.to_owned(),
    };
    let result = match (&found.refused, found.target.ambiguity()) {
        (Some(state), _) => Err(state.clone()),
        (None, Some(reason)) => Err(format!("{AMBIGUOUS} {reason}")),
        // On disk, an entry at this blob: its digests, the markers' levels.
        (None, None) => match codes.reused.get(&key) {
            Some(code) => Ok(verified(LevelCode {
                levels: found.levels,
                ..code.clone()
            })),
            None => codes
                .of(&found)
                .map(|hashed| verified(level_code(found.levels, &hashed))),
        },
    };
    // A key the lock could not read back (a path with a TAB) is never
    // written: core's one check, the reader's too.
    let result = result.and_then(|verified| match key_problem(&key) {
        Some(problem) => Err(problem.to_string()),
        None => Ok(verified),
    });
    match result {
        // Markers giving one key are one entry: lockable only when each
        // allows; the first refusal names it.
        Ok(verified) => {
            if bindings.unlockable.contains_key(&key) {
                return;
            }
            match bindings.fresh.get_mut(&key) {
                Some(held) => {
                    if let (EntryCode::Levels(held), EntryCode::Levels(now)) =
                        (&mut held.code, verified.code)
                    {
                        held.levels.extend(now.levels);
                    }
                }
                None => {
                    bindings.fresh.insert(key, verified);
                }
            }
        }
        Err(state) => {
            bindings.fresh.remove(&key);
            bindings.unlockable.entry(key).or_insert(state);
        }
    }
}

/// One comment of a marker file: its text, its byte range, and what it
/// binds.
struct CommentAt<'a> {
    comment: &'a str,
    range: Range<usize>,
    target: &'a dyn Fn() -> Target,
}

/// The markers of a comment naming the node `key`, each with its target
/// and its effective levels.
fn node_markers(
    at: &CommentAt<'_>,
    corpus: &mut Corpus<'_>,
    key: NodeKey,
    rev: u32,
    found: &mut Vec<Found>,
) {
    for marker in markers_in(at.comment) {
        let judged = corpus.judge(&marker.id);
        if !judged.nodes.contains(&key) {
            continue;
        }
        let token = marker.token(at.comment);
        let levels = marker
            .levels
            .effective()
            .unwrap_or_default()
            .iter()
            .copied()
            .map(lock_level)
            .collect();
        found.push(Found {
            relation: relation_of(&marker),
            refused: refusal(&marker, token, judged.state, rev),
            target: (at.target)(),
            levels,
            comment: at.range.clone(),
        });
    }
}

/// `comment` holds a marker naming the node `key`.
fn marks_node(comment: &str, corpus: &mut Corpus<'_>, key: NodeKey) -> bool {
    markers_in(comment)
        .iter()
        .any(|marker| corpus.judge(&marker.id).nodes.contains(&key))
}

/// The lock's relation of a marker's.
pub(crate) fn relation_of(marker: &Marker) -> LockRelation {
    match marker.relation {
        Relation::Implements => LockRelation::Implements,
        Relation::Verifies => LockRelation::Verifies,
        Relation::Configures => LockRelation::Configures,
        Relation::Assumes => LockRelation::Assumes,
    }
}

/// The first of `id_state`, `rev_state` (against the node's revision at
/// `HEAD`) and the level list that refuses a lock ("Lockability"): a
/// malformed list (`unclosed`, `unknown`, `duplicate`, `empty`) is
/// `bad_levels`, never the default levels.
fn refusal(marker: &Marker, token: &str, state: IdState, rev: u32) -> Option<String> {
    if !matches!(state, IdState::Resolved | IdState::Alias) {
        return Some(state.as_str().to_owned());
    }
    match rev_state(marker, token, state, Revision::Written(rev)) {
        "current" | "unversioned" => {}
        other => return Some(other.to_owned()),
    }
    matches!(marker.levels, Levels::Invalid(_)).then(|| BAD_LEVELS.to_owned())
}

/// The node's revision and `norm_hash` from its file's `HEAD` blob;
/// `Ok(Err)`: unreadable or null (exit 1).
pub(crate) fn node_at_head(
    repo: &Repo,
    scheme: &IdScheme,
    node: &NodeAt,
) -> Result<Result<(u32, String), String>, CliError> {
    let bytes = repo.blob(&node.path)?;
    let Some((parsed, ord)) = parsed_node(&node.path, &bytes, scheme, node)? else {
        return Ok(Err(format!(
            "`{}`: its file `{}` at HEAD holds no such node; nothing written",
            node.canonical, node.path
        )));
    };
    let Some(rev) = revision(&parsed, ord).number() else {
        return Ok(Err(format!(
            "`{}`: its revision at HEAD cannot be read (a written `rev` that is no number from \
             1); nothing written",
            node.canonical
        )));
    };
    let Some(spec_hash) = norm_input(&parsed, &bytes, ord).map(|input| b3_hash(&input)) else {
        return Ok(Err(format!(
            "`{}`: its norm_hash at HEAD is null (`spec show` says why); nothing written",
            node.canonical
        )));
    };
    Ok(Ok((rev, spec_hash)))
}

/// The parse of `bytes` and the node's position in it: the document for a
/// document, else the first node with its ID.
fn parsed_node(
    path: &str,
    bytes: &[u8],
    scheme: &IdScheme,
    node: &NodeAt,
) -> Result<Option<(ParsedFile, usize)>, CliError> {
    let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
        specengine_core::parse(path, bytes, scheme)
    }))
    .map_err(|_| CliError::spec(format!("the spec parser failed on `{path}` at HEAD")))?;
    let ord = if node.key.ord == 0 {
        (!parsed.nodes.is_empty()).then_some(0)
    } else {
        parsed
            .nodes
            .iter()
            .position(|found| found.id.as_deref() == Some(node.id.as_str()))
    };
    Ok(ord.map(|ord| (parsed, ord)))
}

/// Each input that is not tracked and clean, with its state. `spec.lock`
/// absent and not at `HEAD` is clean (a first lock).
pub(crate) fn dirty_inputs(
    repo: &Repo,
    inputs: &BTreeSet<String>,
    lock_exists: bool,
) -> Result<Vec<(String, &'static str)>, CliError> {
    let tops: Vec<String> = inputs.iter().map(|path| repo.top(path)).collect();
    let specs: Vec<&str> = tops.iter().map(String::as_str).collect();
    let entries = repo.git.status_entries(&specs, true).map_err(git_error)?;
    let mut states: BTreeMap<String, &'static str> = BTreeMap::new();
    for entry in &entries {
        let Some(path) = repo.root_relative(&entry.path) else {
            continue;
        };
        if inputs.contains(path) {
            states.entry(path.to_owned()).or_insert(state_of(entry));
        }
    }
    for path in inputs {
        if states.contains_key(path) {
            continue;
        }
        let at_head = repo
            .git
            .blob_oid(&repo.head, &repo.top(path))
            .map_err(git_error)?;
        if at_head.is_none() && (path != LOCK_FILE || lock_exists) {
            states.insert(path.clone(), NOT_TRACKED);
        }
    }
    Ok(states.into_iter().collect())
}

/// A status entry's state: untracked, deleted, staged, else modified.
pub(crate) fn state_of(entry: &specengine_store::StatusEntry) -> &'static str {
    match (entry.index, entry.worktree) {
        ('?', _) | (_, '?') => UNTRACKED,
        (_, 'D') | ('D', _) => DELETED,
        (' ', _) => MODIFIED,
        _ => STAGED,
    }
}

/// Every `.rs` or `.ron` file the walk would read, and every `Cargo.toml`
/// that can name its files (one under a root, or in a directory above
/// one, the project root's own included), that differs from `HEAD`
/// (modified, staged, deleted, untracked), each with its state. A marker's
/// `qpath` and its ambiguity depend on them all (the package, its targets,
/// `#[path]`, a twin elsewhere in the tree), and a holder that lost its
/// marker in the working tree is one of them: the lock is made of one
/// commit's code. One `status` over the roots and those manifests.
fn changed_code(repo: &Repo, code: &CodeConfig) -> Result<Vec<(String, &'static str)>, CliError> {
    let mut specs: BTreeSet<String> = BTreeSet::new();
    for root in &code.roots {
        specs.insert(repo.top(root));
        let mut dir = root.as_str();
        while let Some(slash) = dir.rfind('/') {
            dir = &dir[..slash];
            specs.insert(repo.top(&format!("{dir}/{MANIFEST}")));
        }
        specs.insert(repo.top(MANIFEST));
    }
    let specs: Vec<&str> = specs.iter().map(String::as_str).collect();
    let entries = repo.git.status_entries(&specs, true).map_err(git_error)?;
    let mut changed = Vec::new();
    for entry in &entries {
        let Some(path) = repo.root_relative(&entry.path) else {
            continue;
        };
        if is_code_path(code, path) || is_manifest(code, path) {
            changed.push((path.to_owned(), state_of(entry)));
        }
    }
    Ok(changed)
}

/// A root-relative `Cargo.toml` that can name a walked file: under a root,
/// or in a directory a root lies under (the project root's included).
pub(crate) fn is_manifest(code: &CodeConfig, path: &str) -> bool {
    let Some(dir) = path.strip_suffix(MANIFEST) else {
        return false;
    };
    let dir = match dir.strip_suffix('/') {
        Some(dir) => dir,
        None if dir.is_empty() => "",
        None => return false,
    };
    code.roots
        .iter()
        .any(|root| is_under(path, root) || dir.is_empty() || is_under(root, dir))
}

/// A root-relative path the walk would read: `.rs` or `.ron`, a root or
/// under one, no `.`-name below it, not excluded.
pub(crate) fn is_code_path(code: &CodeConfig, path: &str) -> bool {
    (path.ends_with(RUST_EXTENSION) || path.ends_with(RON_EXTENSION))
        && code.roots.iter().any(|root| {
            path == root
                || (is_under(path, root)
                    && !path[root.len() + 1..]
                        .split('/')
                        .any(|part| part.starts_with('.')))
        })
        && !code.is_excluded(path)
}

/// Exit 1's reason for dirty inputs.
pub(crate) fn dirty_reason(dirty: &[(String, &'static str)]) -> String {
    let named: Vec<String> = dirty
        .iter()
        .map(|(path, state)| format!("`{path}` {state}"))
        .collect();
    format!(
        "the lock hashes committed bytes, and {}: commit them first; nothing written",
        named.join(", ")
    )
}

// -------------------------------------------------------------- editorial

/// ` for PR-0012` when a drift item's approve runs the accept, before ` at
/// <commit>` in each question; else nothing.
fn for_item(proposal: Option<&str>) -> String {
    proposal.map(|id| format!(" for {id}")).unwrap_or_default()
}

/// `--editorial`: `spec_hash` set on the node's entries at its revision.
#[allow(clippy::too_many_arguments)]
fn editorial(
    env: &Env,
    project: &ProjectRoot,
    repo: &Repo,
    read: ReadLock,
    node: &NodeAt,
    proposal: Option<&str>,
    mut outcome: LockOutcome,
    consent: LockConsent<'_>,
    trailers: &[LockTrailer],
) -> Result<LockOutcome, CliError> {
    let inputs: BTreeSet<String> = [
        node.path.clone(),
        CONFIG_FILE.to_owned(),
        LOCK_FILE.to_owned(),
    ]
    .into_iter()
    .collect();
    let dirty = dirty_inputs(repo, &inputs, read.bytes.is_some())?;
    if !dirty.is_empty() {
        return Ok(outcome.refused(&dirty_reason(&dirty)));
    }
    let (rev, spec_hash) = match node_at_head(repo, &project.config.scheme, node)? {
        Ok(found) => found,
        Err(reason) => return Ok(outcome.refused(&reason)),
    };
    outcome.rev = Some(rev);
    let refreshed = refresh_editorial(&read.lock, &node.canonical, rev, &spec_hash, None);
    // The node's block at R is an entry at R.
    if refreshed.is_empty() {
        let plain = match proposal {
            Some(id) => format!("spec approve {id}"),
            None => format!("spec lock accept {}", node.canonical),
        };
        let reason = format!(
            "`{}` has no entry at rev {rev} (its revision was raised, or it was never locked): \
             `{plain}` locks it; nothing written",
            node.canonical
        );
        return Ok(outcome.refused(&reason));
    }
    outcome.rows = refreshed
        .at_rev
        .iter()
        .map(|(key, changed)| AcceptRow {
            change: if *changed {
                Change::Refreshed
            } else {
                Change::Unchanged
            },
            place: RowPlace::of(key),
            state: None,
        })
        .map(|row| accept_row(&row))
        .collect();
    outcome.tested = refreshed.tested.map(|changed| {
        if changed {
            Change::Refreshed.as_str()
        } else {
            Change::Unchanged.as_str()
        }
    });
    let count = refreshed.refreshed();
    if count == 0 {
        outcome.report = Some(outcome_rows(&outcome));
        outcome.nothing = Some(format!(
            "every entry at rev {rev} holds the node's spec_hash"
        ));
        outcome.unchanged = true;
        return Ok(outcome);
    }
    let question = format!(
        "lock {} rev {rev} (editorial: spec_hash of {count} entries){} at {} on {} in {}? [y/N]",
        node.canonical,
        for_item(proposal),
        repo.head,
        repo.branch,
        repo.worktree
    );
    let message_node = node.canonical.clone();
    write_and_commit(
        env,
        project,
        repo,
        &read,
        &refreshed.lock,
        &LockSubject::Node(&message_node),
        question,
        outcome,
        consent,
        trailers,
    )
}

// -------------------------------------------------------------- gone node

/// A gone node's entries dropped: no walk, no hash.
#[allow(clippy::too_many_arguments)]
fn drop_gone(
    env: &Env,
    project: &ProjectRoot,
    repo: &Repo,
    read: ReadLock,
    written: &str,
    proposal: Option<&str>,
    mut outcome: LockOutcome,
    consent: LockConsent<'_>,
    trailers: &[LockTrailer],
) -> Result<LockOutcome, CliError> {
    outcome.node = Some(written.to_owned());
    let inputs: BTreeSet<String> = [CONFIG_FILE.to_owned(), LOCK_FILE.to_owned()]
        .into_iter()
        .collect();
    let dirty = dirty_inputs(repo, &inputs, read.bytes.is_some())?;
    if !dirty.is_empty() {
        return Ok(outcome.refused(&dirty_reason(&dirty)));
    }
    let (lock, dropped) = drop_node(&read.lock, written);
    outcome.rows = dropped
        .iter()
        .map(|key| {
            accept_row(&AcceptRow {
                change: Change::Dropped,
                place: RowPlace::of(key),
                state: None,
            })
        })
        .collect();
    let block = read.lock.tested.contains_key(written);
    outcome.tested = block.then_some(Change::Dropped.as_str());
    let question = format!(
        "lock {written} (gone: {} dropped){} at {} on {} in {}? [y/N]",
        dropped.len() + usize::from(block),
        for_item(proposal),
        repo.head,
        repo.branch,
        repo.worktree
    );
    write_and_commit(
        env,
        project,
        repo,
        &read,
        &lock,
        &LockSubject::Node(written),
        question,
        outcome,
        consent,
        trailers,
    )
}

// ------------------------------------------------------------ the commit

/// Steps 8 to 10: the identity, an ignored first lock, the question, the
/// place and the bytes read again, the write and the commit; `subject`
/// what the commit is of (an accept's node, or a rebase: its message, its
/// third check, its rows).
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_and_commit(
    env: &Env,
    project: &ProjectRoot,
    repo: &Repo,
    read: &ReadLock,
    lock: &Lock,
    subject: &LockSubject<'_>,
    question: String,
    mut outcome: LockOutcome,
    consent: LockConsent<'_>,
    trailers: &[LockTrailer],
) -> Result<LockOutcome, CliError> {
    let rows = match subject {
        LockSubject::Node(_) => outcome_rows(&outcome),
        LockSubject::Rebase { rows } | LockSubject::Tests { rows, .. } => (*rows).to_owned(),
    };
    let decided_by = repo.git.committer_ident().map_err(|error| {
        CliError::spec(format!(
            "no committer identity in {} ({error}): set user.name and user.email",
            repo.worktree
        ))
    })?;
    let top_path = repo.top(LOCK_FILE);
    if read.bytes.is_none()
        && repo
            .git
            .blob_oid(&repo.head, &top_path)
            .map_err(git_error)?
            .is_none()
        && repo.git.is_ignored(&top_path).map_err(git_error)?
    {
        outcome.report = Some(rows);
        let reason = format!(
            "{LOCK_FILE} is ignored by git in {}: the lock is committed, so un-ignore it first; \
             nothing written",
            repo.worktree
        );
        return Ok(outcome.refused(&reason));
    }
    // The commit message's scratch directory, before the question: nothing
    // after the write may fail unrestored.
    let data_dir = prepared_data_dir(env, project)?;
    let escaped_rows = escape_controls(&rows);
    let escaped_question = escape_controls(&question);
    let agreed = consent(&LockPrompt {
        rows: &escaped_rows,
        question: &escaped_question,
    });
    if !agreed {
        return Ok(outcome.refused("not confirmed; nothing written"));
    }

    // The place and the bytes, read again.
    let head = repo.git.head().map_err(git_error)?;
    let branch = repo.git.branch().map_err(git_error)?;
    let bytes = read_lock_bytes(&project.root)?;
    if head.as_deref() != Some(repo.head.as_str())
        || branch.as_deref() != Some(repo.branch.as_str())
        || bytes != read.bytes
    {
        return Ok(outcome.refused(&changed_while(subject)));
    }

    // The write.
    let rendered = render_lock(lock);
    let path = project.root.join(LOCK_FILE);
    let created = match &read.bytes {
        Some(_) => {
            replace_file(&path, rendered.as_bytes()).map_err(|error| {
                CliError::spec(format!(
                    "cannot write {LOCK_FILE}: {error}; nothing written"
                ))
            })?;
            None
        }
        None => match create_file(&project.root, LOCK_FILE, rendered.as_bytes()) {
            Ok(created) => Some(created),
            Err(CreateFileError::Exists) => {
                return Ok(outcome.refused(&format!(
                    "{LOCK_FILE} appeared while being {}; nothing written",
                    subject.being()
                )));
            }
            Err(error) => {
                return Err(CliError::spec(format!(
                    "cannot create {LOCK_FILE}: {error}; nothing written"
                )));
            }
        },
    };

    // The commit.
    let mut message = format!(
        "{}{LOCK_MODE_TRAILER}: {}\n{DECIDED_BY_TRAILER}: {decided_by}\n",
        subject.message_head(),
        outcome.mode.trailer()
    );
    for trailer in trailers {
        message.push_str(&format!(
            "{}: {}\n",
            one_line(&trailer.key),
            one_line(&trailer.value)
        ));
    }
    let committed = match &created {
        Some(_) => repo
            .git
            .intent_to_add(&top_path)
            .and_then(|()| repo.git.commit_only(&data_dir, &message, &top_path)),
        None => repo.git.commit_only(&data_dir, &message, &top_path),
    };
    // Step 10's check, as `spec approve` verifies its own commit: the
    // commit is the branch's tip (never `HEAD`: a checkout meanwhile would
    // name another), and success is only ever a verified commit.
    let old = repo.head.as_str();
    let branch = repo.branch.as_str();
    let tip = || repo.git.commit_of(&format!("refs/heads/{branch}"));
    let check =
        |commit: &str| check_lock_commit(repo, commit, &top_path, subject, &rendered, trailers);
    let undo =
        |commit: &str| format!("`git reset --soft {commit}^` undoes it, its changes kept staged");
    let undo_back = Undo {
        repo,
        read,
        path: &path,
        created: created.as_ref(),
        top_path: &top_path,
        rendered: rendered.as_bytes(),
    };
    // What was written, for a caller mapping it (a drift item's approve);
    // each branch below sets it.
    match committed {
        Ok(()) => match tip() {
            Ok(Some(commit)) if commit != old => {
                outcome.commit = Some(commit.clone());
                match check(&commit) {
                    Check::Ours => {
                        outcome.written = LockWritten::Committed;
                        Ok(outcome)
                    }
                    // Made, and not the lock alone: never rewritten here.
                    Check::NotAlone(wrong) => {
                        outcome.written = LockWritten::Unverified;
                        Ok(outcome.refused(&format!(
                            "the lock commit {commit} on `{branch}` is not this run's \
                             {LOCK_FILE} alone ({}); it stays as git made it: {}",
                            wrong.join("; "),
                            undo(&commit)
                        )))
                    }
                    Check::Unverified(why) => {
                        outcome.written = LockWritten::Unverified;
                        Ok(outcome.refused(&format!(
                            "the lock commit {commit} on `{branch}` was made but cannot be \
                             verified ({}); it stays as git made it: if it is not the lock \
                             alone, {}",
                            why.join("; "),
                            undo(&commit)
                        )))
                    }
                    // This run's, a caller's trailer dropped by a hook.
                    Check::Untrailed(missing) => {
                        outcome.written = LockWritten::Unverified;
                        Ok(outcome.refused(&untrailed(&commit, branch, &missing, &undo(&commit))))
                    }
                }
            }
            Ok(Some(_)) => {
                // The branch did not move: the commit is not on it.
                if let Ok(Some(head)) = repo.git.head()
                    && head != old
                {
                    outcome.written = LockWritten::Unverified;
                    return Ok(outcome.refused(&format!(
                        "git reported the commit of {LOCK_FILE} made, yet `{branch}` did not move \
                         from {old} and HEAD is {head}: the commit is unverified and {LOCK_FILE} \
                         is left as written"
                    )));
                }
                // Neither moved (a hook reset it): the bytes put back.
                let restored = undo_back.restore(true, &mut outcome);
                Ok(outcome.refused(&format!(
                    "git reported the commit of {LOCK_FILE} made, yet `{branch}` did not move from \
                     {old}: {restored}; nothing committed"
                )))
            }
            Ok(None) => {
                outcome.written = LockWritten::Unverified;
                Ok(outcome.refused(&format!(
                    "git reported the commit of {LOCK_FILE} made, yet `{branch}` names no commit \
                     now: it is unverified{}",
                    head_hint(repo)
                )))
            }
            Err(error) => {
                outcome.written = LockWritten::Unverified;
                Ok(outcome.refused(&format!(
                    "git reported the commit of {LOCK_FILE} made, but `{branch}` cannot be read \
                     ({error}): it is unverified{}",
                    head_hint(repo)
                )))
            }
        },
        Err(error) => match tip() {
            Ok(Some(commit)) if commit != old => match check(&commit) {
                Check::Ours => {
                    // Git made this run's commit anyway (it failed after
                    // the ref update).
                    outcome.messages.push(Message::Warning(format!(
                        "the commit reported a failure, yet {commit} is this lock commit: {error}"
                    )));
                    outcome.commit = Some(commit);
                    outcome.written = LockWritten::Committed;
                    Ok(outcome)
                }
                Check::NotAlone(wrong) => {
                    // The branch moved to another commit: not this run's.
                    let restored = undo_back.restore(false, &mut outcome);
                    Ok(outcome.refused(&format!(
                        "the commit of {LOCK_FILE} failed ({error}); `{branch}` moved to {commit}, \
                         which is not this lock commit ({}): {restored}",
                        wrong.join("; ")
                    )))
                }
                Check::Unverified(why) => {
                    outcome.written = LockWritten::Unverified;
                    Ok(outcome.refused(&format!(
                        "the commit of {LOCK_FILE} failed ({error}); `{branch}` moved to {commit}, \
                         which cannot be verified ({}): {LOCK_FILE} is left as written",
                        why.join("; ")
                    )))
                }
                // Git made this run's commit anyway, a caller's trailer
                // dropped: nothing put back, as when the commit succeeds.
                Check::Untrailed(missing) => {
                    let reason = format!(
                        "the commit of {LOCK_FILE} failed ({error}); {}",
                        untrailed(&commit, branch, &missing, &undo(&commit))
                    );
                    outcome.commit = Some(commit);
                    outcome.written = LockWritten::Unverified;
                    Ok(outcome.refused(&reason))
                }
            },
            Ok(Some(_)) => {
                // The branch did not move: no commit of this run is on it.
                let restored = undo_back.restore(false, &mut outcome);
                Ok(outcome.refused(&format!(
                    "the commit of {LOCK_FILE} failed, {restored}: {error}"
                )))
            }
            // The commit may have been made: nothing is restored.
            Ok(None) => {
                outcome.written = LockWritten::Unverified;
                Ok(outcome.refused(&format!(
                    "the commit of {LOCK_FILE} failed ({error}), yet `{branch}` names no commit \
                     now: it is unverified and {LOCK_FILE} is left as written{}",
                    head_hint(repo)
                )))
            }
            Err(read) => {
                outcome.written = LockWritten::Unverified;
                Ok(outcome.refused(&format!(
                    "the commit of {LOCK_FILE} failed ({error}), and `{branch}` cannot be read \
                     ({read}): it is unverified and {LOCK_FILE} is left as written{}",
                    head_hint(repo)
                )))
            }
        },
    }
}

/// Why this run's lock commit is unverified though the four checks hold: a
/// caller's trailer is not on it; the commit stays as git made it.
fn untrailed(commit: &str, branch: &str, missing: &[String], undo: &str) -> String {
    format!(
        "the lock commit {commit} on `{branch}` is this run's {LOCK_FILE}, yet {}: it is \
         unverified and stays as git made it: {undo}",
        missing.join("; ")
    )
}

/// `; HEAD names <commit>` when `HEAD` can be read: what an unverified
/// commit's refusal can still name.
fn head_hint(repo: &Repo) -> String {
    match repo.git.head() {
        Ok(Some(head)) if head != repo.head => format!("; HEAD names {head}"),
        _ => String::new(),
    }
}

/// What step 10 found a commit to be.
enum Check {
    /// This run's lock commit: every check holds.
    Ours,
    /// One of the four "is it ours" checks fails: the reasons (a caller's
    /// trailer missing too, if so).
    NotAlone(Vec<String>),
    /// No check fails, but one could not be read: the reasons (a caller's
    /// trailer missing too, if so).
    Unverified(Vec<String>),
    /// The four hold, read: this run's lock commit, yet a caller's trailer
    /// is missing or holds another value (a hook dropped or rewrote it):
    /// the reasons. Never put back; unverified.
    Untrailed(Vec<String>),
}

/// `commit` against this run's lock commit (`spec approve`'s three checks
/// and two more): its parents the old `HEAD` alone, `spec.lock` the only
/// path it changes, a `Lock:` trailer naming the node (a rebase's: a
/// `Lock-mode: rebase` trailer and no `Lock:`), its `spec.lock` blob the
/// bytes the owner confirmed (a hook that rewrote and staged the lock, a
/// concurrent accept of the node, fail it); then, apart from these four,
/// each of `trailers` (the caller's, `Proposal:` of a drift item's
/// approve) present with its value (a hook dropping one fails it, the
/// commit still this run's: [`Check::Untrailed`]).
fn check_lock_commit(
    repo: &Repo,
    commit: &str,
    top_path: &str,
    subject: &LockSubject<'_>,
    rendered: &str,
    trailers: &[LockTrailer],
) -> Check {
    let mut wrong = Vec::new();
    let mut unread = Vec::new();
    match repo.git.parents(commit) {
        Ok(parents) if parents == [repo.head.as_str()] => {}
        Ok(parents) => wrong.push(format!(
            "its parents are {}, not {} alone",
            if parents.is_empty() {
                "none".to_owned()
            } else {
                parents.join(" ")
            },
            repo.head
        )),
        Err(error) => unread.push(format!("its parents cannot be read: {error}")),
    }
    match repo.git.changed_paths(&repo.head, commit) {
        Ok(changed) if changed == [top_path] => {}
        Ok(changed) => wrong.push(format!(
            "it changes {}, not {top_path} alone",
            if changed.is_empty() {
                "nothing".to_owned()
            } else {
                changed.join(", ")
            }
        )),
        Err(error) => unread.push(format!("what it changes cannot be read: {error}")),
    }
    match subject {
        LockSubject::Node(node) => match repo.git.trailer_values(commit, LOCK_TRAILER) {
            Ok(values) if values.iter().any(|value| value == node) => {}
            Ok(_) => wrong.push(format!("it has no `{LOCK_TRAILER}: {node}` trailer")),
            Err(error) => unread.push(format!("its trailers cannot be read: {error}")),
        },
        LockSubject::Rebase { .. } => {
            let mode = LockMode::Rebase.trailer();
            match (
                repo.git.trailer_values(commit, LOCK_MODE_TRAILER),
                repo.git.trailer_values(commit, LOCK_TRAILER),
            ) {
                (Ok(modes), Ok(nodes)) => {
                    if !modes.iter().any(|value| value == mode) {
                        wrong.push(format!("it has no `{LOCK_MODE_TRAILER}: {mode}` trailer"));
                    }
                    if !nodes.is_empty() {
                        wrong.push(format!("it has a `{LOCK_TRAILER}:` trailer"));
                    }
                }
                (Err(error), _) | (_, Err(error)) => {
                    unread.push(format!("its trailers cannot be read: {error}"));
                }
            }
        }
        // A test run's: a `Lock:` per node whose block changed, and
        // `Lock-mode: tests`.
        LockSubject::Tests { nodes, .. } => {
            let mode = LockMode::Tests.trailer();
            match (
                repo.git.trailer_values(commit, LOCK_TRAILER),
                repo.git.trailer_values(commit, LOCK_MODE_TRAILER),
            ) {
                (Ok(named), Ok(modes)) => {
                    for node in *nodes {
                        let node = one_line(node);
                        if !named.contains(&node) {
                            wrong.push(format!("it has no `{LOCK_TRAILER}: {node}` trailer"));
                        }
                    }
                    if !modes.iter().any(|value| value == mode) {
                        wrong.push(format!("it has no `{LOCK_MODE_TRAILER}: {mode}` trailer"));
                    }
                }
                (Err(error), _) | (_, Err(error)) => {
                    unread.push(format!("its trailers cannot be read: {error}"));
                }
            }
        }
    }
    // A commit deleting the file fails this check (it may pass the
    // changed paths', which a deletion of `spec.lock` alone satisfies).
    match repo.git.blob_oid(commit, top_path) {
        Ok(None) => wrong.push(format!("it has no {top_path}")),
        Ok(Some(_)) => match repo.git.blob_at(commit, top_path) {
            Ok(bytes) if bytes == rendered.as_bytes() => {}
            Ok(_) => wrong.push(format!(
                "its {top_path} is not the bytes the owner confirmed"
            )),
            Err(error) => unread.push(format!("its {top_path} cannot be read: {error}")),
        },
        Err(error) => unread.push(format!("its {top_path} cannot be read: {error}")),
    }
    // The caller's trailers, apart: a missing one never makes the commit
    // another run's.
    let mut untrailed = Vec::new();
    for trailer in trailers {
        let (key, value) = (one_line(&trailer.key), one_line(&trailer.value));
        match repo.git.trailer_values(commit, &key) {
            Ok(values) if values.contains(&value) => {}
            Ok(_) => untrailed.push(format!("it has no `{key}: {value}` trailer")),
            Err(error) => unread.push(format!("its trailers cannot be read: {error}")),
        }
    }
    if !wrong.is_empty() {
        wrong.extend(untrailed);
        Check::NotAlone(wrong)
    } else if !unread.is_empty() {
        unread.extend(untrailed);
        Check::Unverified(unread)
    } else if !untrailed.is_empty() {
        Check::Untrailed(untrailed)
    } else {
        Check::Ours
    }
}

/// What a failed or unkept commit puts back: the old lock's bytes, or the
/// new file and its index entry removed; only while the file still holds
/// the bytes this run wrote.
struct Undo<'a> {
    repo: &'a Repo,
    read: &'a ReadLock,
    path: &'a Path,
    created: Option<&'a specengine_store::CreatedFile>,
    top_path: &'a str,
    rendered: &'a [u8],
}

impl Undo<'_> {
    /// Puts back what the run changed: the old lock's bytes (`index`: its
    /// index entry set back to `HEAD`'s too, which a `commit --only` that
    /// left no commit on the branch updated), or a first lock's file and
    /// index entry removed. A file someone changed meanwhile is never
    /// touched. Sets [`LockOutcome::written`] (`Restored::Back`: restored;
    /// else left); the refusal's clause. Each git command it prints names
    /// its path as a quoted literal pathspec, a warning the worktree too.
    fn restore(&self, index: bool, outcome: &mut LockOutcome) -> String {
        let top_path = self.top_path;
        let word = pathspec_word(top_path);
        let worktree = &self.repo.worktree;
        let now = fs::read(self.path).ok();
        if now.as_deref() != Some(self.rendered) {
            outcome.written = LockWritten::Left;
            // The index entry this run made stays too: a first lock's
            // intent-to-add entry, or the confirmed bytes a `commit --only`
            // that left no commit staged.
            let entry = match (self.created, index) {
                (Some(_), _) => format!(
                    ", and so is its intent-to-add entry in the index (`git rm --cached -- \
                     {word}` drops it)"
                ),
                (None, true) => format!(
                    ", and so is its index entry, which holds the bytes confirmed (`git reset -q \
                     -- {word}` sets it back to HEAD's)"
                ),
                (None, false) => String::new(),
            };
            return format!(
                "{LOCK_FILE} was changed meanwhile, so it is left as it is, not restored{entry}"
            );
        }
        let mut failed = false;
        match (self.created, &self.read.bytes) {
            (Some(created), _) => {
                if let Err(error) = self.repo.git.remove_cached(top_path) {
                    failed = true;
                    outcome.messages.push(Message::Warning(format!(
                        "cannot drop the index entry of {LOCK_FILE} in {worktree}: {error}; `git \
                         rm --cached -- {word}` drops it"
                    )));
                }
                for left in created.remove() {
                    failed = true;
                    outcome.messages.push(Message::Warning(left));
                }
            }
            (None, Some(bytes)) => {
                if let Err(error) = replace_file(self.path, bytes) {
                    failed = true;
                    outcome.messages.push(Message::Warning(format!(
                        "cannot restore {LOCK_FILE} in {worktree}: {error}; `git checkout -- \
                         {word}` restores it"
                    )));
                }
                if index && let Err(error) = self.repo.git.reset_path(top_path) {
                    failed = true;
                    outcome.messages.push(Message::Warning(format!(
                        "cannot set the index entry of {LOCK_FILE} in {worktree} back to HEAD's: \
                         {error}; `git reset -q -- {word}` does"
                    )));
                }
            }
            (None, None) => {}
        }
        let restored = if failed {
            Restored::Failed
        } else {
            Restored::Back
        };
        outcome.written = match restored {
            Restored::Back => LockWritten::Restored,
            Restored::Left | Restored::Failed => LockWritten::Left,
        };
        if failed {
            "it is not restored (see the warning)".to_owned()
        } else {
            "its bytes restored".to_owned()
        }
    }
}

// ----------------------------------------------------------------- rows

/// A merge row as printed.
fn accept_row(row: &AcceptRow) -> LockRow {
    LockRow {
        change: row.change.as_str(),
        relation: row.place.relation.as_str(),
        file: row.place.file.clone(),
        kind: row.place.kind.clone(),
        symbol: row.place.symbol.clone(),
        item_kind: row.place.item_kind.clone(),
        state: row.state.clone(),
    }
}

/// The outcome's rows: the entries' ([`rows_text`]), then the node's
/// `[[tested]]` block's, `<change> | tested`.
fn outcome_rows(outcome: &LockOutcome) -> String {
    let mut out = rows_text(&outcome.rows);
    if let Some(change) = outcome.tested {
        out.push_str(&format!("{change} | tested\n"));
    }
    out
}

/// `<change> | <relation> | <file> | <kind> <symbol>[ <item_kind>][ |
/// <state>]`, one line each.
fn rows_text(rows: &[LockRow]) -> String {
    let mut out = String::new();
    for row in rows {
        let mut binding = one_line(&row.kind);
        if !row.symbol.is_empty() {
            binding.push(' ');
            binding.push_str(&one_line(&row.symbol));
        }
        if let Some(item_kind) = &row.item_kind {
            binding.push(' ');
            binding.push_str(&one_line(item_kind));
        }
        out.push_str(&format!(
            "{} | {} | {} | {binding}",
            row.change,
            row.relation,
            one_line(&row.file)
        ));
        if let Some(state) = &row.state {
            out.push_str(" | ");
            out.push_str(&one_line(state));
        }
        out.push('\n');
    }
    out
}

/// The outcome as stdout text: the rows not shown yet, then `committed
/// <commit>` (a verified commit) or `nothing to change`. A refusal
/// (stderr) ends nothing here, even one naming a commit made.
pub(crate) fn render_text(outcome: &LockOutcome) -> String {
    let mut out = outcome
        .report
        .as_deref()
        .map(escape_controls)
        .unwrap_or_default();
    if outcome.refusal.is_some() {
        return out;
    }
    if let Some(commit) = &outcome.commit {
        out.push_str(&format!("committed {commit}\n"));
    } else if outcome.unchanged {
        out.push_str("nothing to change\n");
    }
    out
}
