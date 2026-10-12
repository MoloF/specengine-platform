//! `spec lock accept REF [--editorial]` (task spec `spec-lock`, "spec lock
//! accept", "--editorial", "A gone node"): what the owner verified,
//! written to `<root>/spec.lock` on the owner's terminal and committed
//! alone (its writers: canon `architecture-code`, "`spec.lock` writers").
//! One library function, [`lock_accept`], is the accept, the editorial
//! refresh and a gone node's drop; it takes the owner's [`LockConsent`]
//! and extra trailers, so a `drift` item's approve (slice 4c, task spec
//! `drift-queue`) runs this same accept with its `Proposal:`.
//!
//! In order (`main` refuses a stdin that is no terminal before anything is
//! read):
//!
//! 1. the project, its root `specengine.toml` (`--config` naming another
//!    is refused), `[code]` (none: exit 2, `spec refs`' text); `REF` as
//!    `spec show` reads it (several nodes: exit 1);
//! 2. git: a worktree, `HEAD` born and on a branch, no operation in
//!    progress; the lock read (exit 2 when unreadable or of other
//!    recipes). `REF`'s bytes (a written `@rev` dropped) naming entries
//!    while no node has that canonical ID: the gone node's drop, once
//!    `HEAD` agrees (a changed spec file whose `HEAD` blob still holds the
//!    ID is a dirty input, exit 1); else `REF` naming nothing: exit 1;
//! 3. accept: the code walk ([`crate::code_walk`]) finds the node's
//!    markers; the inputs tracked and clean, else each named, exit 1: the
//!    node's file, its markers' files, `specengine.toml`, `spec.lock`, and
//!    every `.rs` or `.ron` file the walk would read and every `Cargo.toml`
//!    that names its files that differs from `HEAD` (names and `qpath`s
//!    depend on them all: one commit, one lock); each marker file bound
//!    and its items hashed from its `HEAD` blob, the node's revision and
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
//!    `Decided-by:` and the caller's; a failure restores the bytes;
//! 6. the commit, the branch's tip (`refs/heads/<branch>`), verified as
//!    `spec approve` verifies its own, and more: one parent, the old
//!    `HEAD`; only `spec.lock` changed; a `Lock:` trailer naming the node;
//!    its `spec.lock` the bytes the owner confirmed. Success is only a
//!    verified commit. A commit made that fails a check: exit 1 naming the
//!    commit, what is wrong and `git reset --soft <commit>^` (history never
//!    rewritten); one that cannot be read, or a branch that cannot: exit 1,
//!    unverified. A failed commit whose branch moved to a commit that fails
//!    a check is not this run's: the bytes restored, exit 1; a branch that
//!    did not move after a commit git reported made: the bytes and the
//!    lock's index entry restored, exit 1. Bytes are restored only while
//!    the file still holds what this run wrote.
//!
//! Writes the data directory (the index refresh, the commit message's
//! scratch file), `spec.lock` and one commit; no queue row or event.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use serde::Serialize;
use specengine_code::{
    Anchor, Binding, HashState, MOD_KIND, Marker, RECIPE, Relation, RustParser, bind_tree,
    comment_anchors, grammar_info, hash_bound_items, markers_in,
};
use specengine_core::check::resolve::feature_stem;
use specengine_core::check::{CheckInput, Resolver};
use specengine_core::lock::{
    AcceptInput, AcceptRefusal, AcceptRow, Change, EntryKey, LOCK_FILE, Lock, LockKind,
    LockRelation, Place as RowPlace, Recipes, Verified, drop_node, key_problem, merge_accept,
    parse_lock, refresh_editorial, render_lock,
};
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
use crate::propose::{canonical_id, feature_scoped};
use crate::refs::{
    AMBIGUOUS, CANNOT_VERIFY, Corpus, IdState, NodeKey, RON_DUPLICATE, RON_NO_VALUE, RON_TOO_DEEP,
    UNANCHORED, code_config, rev_state,
};
use crate::show::classify;
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// Why a written `@rev` is ignored (the note's end).
const REV_IGNORED: &str = "`spec lock accept` locks the node's revision at HEAD";

/// The commit's trailers of its own.
const LOCK_TRAILER: &str = "Lock";
const MODE_TRAILER: &str = "Lock-mode";
const DECIDED_BY_TRAILER: &str = "Decided-by";

/// Step 9's refusal (the spec's words).
const CHANGED_WHILE_ACCEPTING: &str = "spec.lock changed while being accepted; nothing written";

/// A dirty input's states.
const MODIFIED: &str = "modified";
const STAGED: &str = "staged";
const UNTRACKED: &str = "untracked";
const DELETED: &str = "deleted";
const NOT_TRACKED: &str = "not tracked";

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
}

/// A trailer after the lock commit's own three (a `drift` item's approve,
/// slice 4c: `Proposal: PR-…`).
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

/// Which write an accept is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LockMode {
    Accept,
    Editorial,
    /// A gone node's entries dropped (`Lock-mode: accept`).
    Gone,
}

impl LockMode {
    /// The `Lock-mode:` trailer's value.
    fn trailer(self) -> &'static str {
        match self {
            Self::Accept | Self::Gone => "accept",
            Self::Editorial => "editorial",
        }
    }
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
}

impl LockOutcome {
    fn new(request: &LockAcceptRequest, mode: LockMode) -> Self {
        Self {
            reference: request.reference.clone(),
            node: None,
            rev: None,
            mode,
            rows: Vec::new(),
            commit: None,
            unchanged: false,
            refusal: None,
            messages: Vec::new(),
            report: None,
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
/// `code_recipe` `<RECIPE> tree-sitter-rust/<version> abi/<n>`.
pub(crate) fn build_recipes() -> Recipes {
    let grammar = grammar_info();
    Recipes {
        norm: NORM_RECIPE.to_owned(),
        code: format!(
            "{RECIPE} tree-sitter-rust/{} abi/{}",
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
    fn open(project: &ProjectRoot, env: &GitEnv) -> Result<Self, CliError> {
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
fn git_error(error: specengine_store::GitError) -> CliError {
    CliError::spec(format!("git: {error}"))
}

/// The lock as read from the worktree: its bytes (`None`: no file) and
/// its entries.
pub(crate) struct ReadLock {
    pub bytes: Option<Vec<u8>>,
    pub lock: Lock,
}

/// `<root>/spec.lock` read and parsed against this build's recipes (an
/// absent file: an empty lock); exit 2 when it cannot be read.
fn read_lock(root: &Path, recipes: &Recipes) -> Result<ReadLock, CliError> {
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

    // Git and the lock.
    let repo = Repo::open(&project, &request.git)?;
    let recipes = build_recipes();
    let read = read_lock(&project.root, &recipes)?;
    let gone_id = without_rev(written);
    let gone = read.lock.entries_of(gone_id).next().is_some()
        && !canonical_ids(&input, scheme, &project).contains(gone_id);
    if gone {
        // Gone only when `HEAD` agrees: a node deleted from the working
        // tree alone is an uncommitted change of its file.
        let held = head_holders(&repo, &project, gone_id)?;
        if !held.is_empty() {
            return Ok(outcome.refused(&dirty_reason(&held)));
        }
        if request.editorial {
            let reason = format!(
                "`{gone_id}`: no node has this ID any more; `spec lock accept {gone_id}` without \
                 --editorial drops its entries"
            );
            return Ok(outcome.refused(&reason));
        }
        outcome.mode = LockMode::Gone;
        return drop_gone(
            env, &project, &repo, read, gone_id, outcome, consent, trailers,
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
            env, &project, &repo, read, &node, outcome, consent, trailers,
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
struct NodeAt {
    key: NodeKey,
    /// Its file, root-relative.
    path: String,
    /// Its own ID.
    id: String,
    /// The canonical cited ID (`slug/ID` when feature-scoped).
    canonical: String,
}

/// `written` without a trailing `@rev` (1 to 9 digits), which `REF`'s read
/// ignores: the canonical ID a gone node's entries carry.
fn without_rev(written: &str) -> &str {
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
fn canonical_ids(input: &CheckInput, scheme: &IdScheme, project: &ProjectRoot) -> BTreeSet<String> {
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
    /// `rev_state`), `None` when both allow.
    refused: Option<String>,
    target: Target,
}

/// What a marker binds.
enum Target {
    /// An item (its index), named.
    Item {
        index: usize,
        qpath: String,
        item_kind: String,
        ambiguity: Option<&'static str>,
    },
    /// A module (the file's or an inline `mod`'s), named.
    Module {
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
        Err(AcceptRefusal::TextChanged {
            key,
            locked,
            current,
        }) => {
            let reason = format!(
                "`{}` rev {rev}: its text changed since spec.lock vouched for it at this revision \
                 ({} {} `{}`: spec_hash {locked}, now {current}): raise its revision through a \
                 proposal, or `spec lock accept {} --editorial` for an editorial change; nothing \
                 written",
                node.canonical,
                key.relation.as_str(),
                key.kind.as_str(),
                key.symbol,
                node.canonical
            );
            return Ok(outcome.refused(&reason));
        }
    };
    outcome.rows = merge.rows.iter().map(accept_row).collect();
    if !merge.writes() {
        let rows = rows_text(&outcome.rows);
        outcome.report = Some(rows);
        if merge.count(Change::Unchanged) > 0 {
            outcome.unchanged = true;
            return Ok(outcome);
        }
        let reason = if any_marker {
            format!(
                "none of `{}`'s bindings can be locked (the rows say why); nothing written",
                node.canonical
            )
        } else {
            format!(
                "no marker names `{}` in the [code] tree; nothing written",
                node.canonical
            )
        };
        return Ok(outcome.refused(&reason));
    }
    let question = format!(
        "lock {} rev {rev} (accept: {} new, {} refreshed, {} kept, {} dropped) at {} on {} in {}? \
         [y/N]",
        node.canonical,
        merge.count(Change::New),
        merge.count(Change::Refreshed),
        merge.count(Change::Kept),
        merge.count(Change::Dropped),
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
        &message_node,
        question,
        outcome,
        consent,
        trailers,
    )
}

/// The node's bindings over its marker files' committed bytes, sorted by
/// what they allow.
#[derive(Default)]
struct Bindings {
    /// Lockable, one per key.
    fresh: BTreeMap<EntryKey, Verified>,
    /// Bound with a key, not lockable: the first refusal of its markers.
    unlockable: BTreeMap<EntryKey, String>,
    /// Bound with no key, and the state.
    keyless: BTreeSet<(RowPlace, String)>,
    /// Some marker names the node.
    any_marker: bool,
}

/// What every fresh entry of one accept shares.
struct Facts<'a> {
    rev: u32,
    spec_hash: &'a str,
    commit: &'a str,
    today: &'a str,
}

/// Each marker file read from `HEAD` (`blob_at`, no filter), bound (its
/// items checked against the walk's, whose names they take), its markers
/// of the node sorted ("Lockability"), the items lockable markers bind
/// hashed: no other item is.
fn committed_bindings(
    repo: &Repo,
    tree: &CodeTree,
    marker_files: &BTreeSet<String>,
    corpus: &mut Corpus<'_>,
    node: &NodeAt,
    old: &Lock,
    facts: &Facts<'_>,
) -> Result<Bindings, CliError> {
    let rev = facts.rev;
    let file_index: BTreeMap<&str, usize> = tree
        .rust
        .iter()
        .enumerate()
        .map(|(index, file)| (file.path.as_str(), index))
        .collect();
    let mut parser = RustParser::new()
        .map_err(|error| CliError::spec(format!("the Rust grammar cannot be loaded: {error}")))?;
    let mut bindings = Bindings::default();
    for path in marker_files {
        let bytes = repo.blob(path)?;
        let text = String::from_utf8(bytes).map_err(|_| {
            CliError::spec(format!(
                "`{path}` at HEAD is not UTF-8 (a git filter?): its committed bytes cannot be \
                 bound"
            ))
        })?;
        let blob = repo.blob_oid(path)?;
        let mut found = Vec::new();
        let mut hashes: BTreeMap<usize, HashState> = BTreeMap::new();
        // The keys of this file's items with a parse error: a marker on
        // one binds `cannot_verify`, and the old entry of its key is kept.
        let mut broken: Vec<(LockKind, String, Option<String>)> = Vec::new();
        if path.ends_with(RUST_EXTENSION) {
            let Some(&index) = file_index.get(path.as_str()) else {
                continue;
            };
            let walked = &tree.rust[index];
            let parsed = parser.parse(&text).ok_or_else(|| {
                CliError::spec(format!(
                    "the code walk is incomplete: `{path}` at HEAD cannot be parsed (the parser \
                     gave no tree)"
                ))
            })?;
            let binding = bind_tree(&parsed, &text);
            let same_items = binding.analysis.items.len() == walked.binding.analysis.items.len()
                && binding
                    .analysis
                    .items
                    .iter()
                    .zip(&walked.binding.analysis.items)
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
                    });
            if !same_items {
                return Err(CliError::spec(format!(
                    "`{path}`: its bytes at HEAD and in the working tree give other items (a git \
                     filter?): cannot name them; nothing written"
                )));
            }
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
                let target = || match comment.binding {
                    Binding::Item(item) => {
                        let name = names.get(item);
                        Target::Item {
                            index: item,
                            qpath: name.map(|name| name.qpath.clone()).unwrap_or_default(),
                            item_kind: binding
                                .analysis
                                .items
                                .get(item)
                                .map(|record| record.kind.clone())
                                .unwrap_or_default(),
                            ambiguity: name.and_then(|name| {
                                name.ambiguity.map(|ambiguity| ambiguity.as_str())
                            }),
                        }
                    }
                    Binding::Module(None) => Target::Module {
                        qpath: module.map(|name| name.qpath.clone()).unwrap_or_default(),
                        ambiguity: module
                            .and_then(|name| name.ambiguity.map(|ambiguity| ambiguity.as_str())),
                    },
                    Binding::Module(Some(item)) => {
                        let name = names.get(item);
                        Target::Module {
                            qpath: name.map(|name| name.qpath.clone()).unwrap_or_default(),
                            ambiguity: name.and_then(|name| {
                                name.ambiguity.map(|ambiguity| ambiguity.as_str())
                            }),
                        }
                    }
                    Binding::Unanchored(category) => Target::Keyless {
                        state: UNANCHORED,
                        category: category.as_str(),
                    },
                    Binding::CannotVerify(category) => Target::Keyless {
                        state: CANNOT_VERIFY,
                        category: category.as_str(),
                    },
                    Binding::File => Target::Keyless {
                        state: UNANCHORED,
                        category: "",
                    },
                };
                node_markers(comment_text, corpus, node.key, rev, &target, &mut found);
            }
            let wanted: Vec<usize> = found
                .iter()
                .filter_map(|found| match found.target {
                    Target::Item {
                        index,
                        ambiguity: None,
                        ..
                    } if found.refused.is_none() => Some(index),
                    _ => None,
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            for (index, state) in wanted.iter().zip(hash_bound_items(&parsed, &text, &wanted)) {
                if let Some(state) = state {
                    hashes.insert(*index, state);
                }
            }
        } else if path.ends_with(RON_EXTENSION) {
            for anchor in comment_anchors(&text) {
                let Some(comment_text) = text.get(anchor.comment.clone()) else {
                    continue;
                };
                let target = || match &anchor.anchor {
                    Anchor::Path { path, .. } => Target::Ron {
                        path: path.clone(),
                        ambiguity: None,
                    },
                    Anchor::Ambiguous { path, .. } => Target::Ron {
                        path: path.clone(),
                        ambiguity: Some(RON_DUPLICATE),
                    },
                    Anchor::Unanchored => Target::Keyless {
                        state: UNANCHORED,
                        category: RON_NO_VALUE,
                    },
                    Anchor::CannotVerify => Target::Keyless {
                        state: CANNOT_VERIFY,
                        category: RON_TOO_DEEP,
                    },
                };
                node_markers(comment_text, corpus, node.key, rev, &target, &mut found);
            }
        }
        bindings.any_marker |= !found.is_empty();
        keep_broken(&found, &broken, &node.canonical, path, old, &mut bindings);
        let pair = Pair { facts, blob: &blob };
        for found in found {
            classify_found(found, &node.canonical, path, &pair, &hashes, &mut bindings);
        }
    }
    let Bindings {
        fresh, unlockable, ..
    } = &mut bindings;
    for key in unlockable.keys() {
        fresh.remove(key);
    }
    Ok(bindings)
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

/// A marker of the node sorted into a fresh entry, a key not lockable or
/// a keyless binding.
fn classify_found(
    found: Found,
    node: &str,
    path: &str,
    pair: &Pair<'_>,
    hashes: &BTreeMap<usize, HashState>,
    bindings: &mut Bindings,
) {
    let key = |kind: LockKind, symbol: &str, item_kind: Option<&str>| EntryKey {
        node: node.to_owned(),
        relation: found.relation,
        file: path.to_owned(),
        kind,
        symbol: symbol.to_owned(),
        item_kind: item_kind.map(str::to_owned),
    };
    let verified = |ast_hash: Option<String>| Verified {
        rev: pair.facts.rev,
        spec_hash: pair.facts.spec_hash.to_owned(),
        ast_hash,
        file_blob: pair.blob.to_owned(),
        commit: pair.facts.commit.to_owned(),
        verified_at: pair.facts.today.to_owned(),
    };
    let ambiguous = |reason: &str| format!("{AMBIGUOUS} {reason}");
    let (key, result): (EntryKey, Result<Verified, String>) = match &found.target {
        Target::Keyless { state, category } => {
            let place = RowPlace {
                relation: found.relation,
                file: path.to_owned(),
                kind: (*state).to_owned(),
                symbol: (*category).to_owned(),
                item_kind: None,
            };
            let state = found.refused.clone().unwrap_or_else(|| (*state).to_owned());
            bindings.keyless.insert((place, state));
            return;
        }
        Target::Item {
            index,
            qpath,
            item_kind,
            ambiguity,
        } => {
            let key = key(LockKind::Item, qpath, Some(item_kind));
            let result = match (&found.refused, ambiguity) {
                (Some(state), _) => Err(state.clone()),
                (None, Some(reason)) => Err(ambiguous(reason)),
                (None, None) => match hashes.get(index) {
                    Some(HashState::Hashed(digest)) => {
                        Ok(verified(Some(format!("b3:{}", digest.to_hex()))))
                    }
                    Some(HashState::CannotVerify { categories }) => {
                        Err(categories.first().map_or_else(
                            || CANNOT_VERIFY.to_owned(),
                            |category| format!("{CANNOT_VERIFY} {}", category.as_str()),
                        ))
                    }
                    Some(HashState::NotHashed) | None => Err(CANNOT_VERIFY.to_owned()),
                },
            };
            (key, result)
        }
        Target::Module { qpath, ambiguity } => {
            let key = key(LockKind::Module, qpath, None);
            let result = match (&found.refused, ambiguity) {
                (Some(state), _) => Err(state.clone()),
                (None, Some(reason)) => Err(ambiguous(reason)),
                (None, None) => Ok(verified(None)),
            };
            (key, result)
        }
        Target::Ron {
            path: field,
            ambiguity,
        } => {
            let key = key(LockKind::Ron, field, None);
            let result = match (&found.refused, ambiguity) {
                (Some(state), _) => Err(state.clone()),
                (None, Some(reason)) => Err(ambiguous(reason)),
                (None, None) => Ok(verified(None)),
            };
            (key, result)
        }
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
            if !bindings.unlockable.contains_key(&key) {
                bindings.fresh.insert(key, verified);
            }
        }
        Err(state) => {
            bindings.fresh.remove(&key);
            bindings.unlockable.entry(key).or_insert(state);
        }
    }
}

/// The markers of `comment` naming the node `key`, each with its target.
fn node_markers(
    comment: &str,
    corpus: &mut Corpus<'_>,
    key: NodeKey,
    rev: u32,
    target: &dyn Fn() -> Target,
    found: &mut Vec<Found>,
) {
    for marker in markers_in(comment) {
        let judged = corpus.judge(&marker.id);
        if !judged.nodes.contains(&key) {
            continue;
        }
        let token = marker.token(comment);
        found.push(Found {
            relation: relation_of(&marker),
            refused: refusal(&marker, token, judged.state, rev),
            target: target(),
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
fn relation_of(marker: &Marker) -> LockRelation {
    match marker.relation {
        Relation::Implements => LockRelation::Implements,
        Relation::Verifies => LockRelation::Verifies,
        Relation::Configures => LockRelation::Configures,
        Relation::Assumes => LockRelation::Assumes,
    }
}

/// The first of `id_state` and `rev_state` (against the node's revision
/// at `HEAD`) that refuses a lock ("Lockability").
fn refusal(marker: &Marker, token: &str, state: IdState, rev: u32) -> Option<String> {
    if !matches!(state, IdState::Resolved | IdState::Alias) {
        return Some(state.as_str().to_owned());
    }
    match rev_state(marker, token, state, Revision::Written(rev)) {
        "current" | "unversioned" => None,
        other => Some(other.to_owned()),
    }
}

/// The node's revision and `norm_hash` from its file's `HEAD` blob;
/// `Ok(Err)`: unreadable or null (exit 1).
fn node_at_head(
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
fn dirty_inputs(
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
fn state_of(entry: &specengine_store::StatusEntry) -> &'static str {
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
fn is_manifest(code: &CodeConfig, path: &str) -> bool {
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
fn is_code_path(code: &CodeConfig, path: &str) -> bool {
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
fn dirty_reason(dirty: &[(String, &'static str)]) -> String {
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

/// `--editorial`: `spec_hash` set on the node's entries at its revision.
#[allow(clippy::too_many_arguments)]
fn editorial(
    env: &Env,
    project: &ProjectRoot,
    repo: &Repo,
    read: ReadLock,
    node: &NodeAt,
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
    let refreshed = refresh_editorial(&read.lock, &node.canonical, rev, &spec_hash);
    if refreshed.at_rev.is_empty() {
        let reason = format!(
            "`{}` has no entry at rev {rev} (its revision was raised, or it was never locked): \
             `spec lock accept {}` locks it; nothing written",
            node.canonical, node.canonical
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
    let count = refreshed.refreshed();
    if count == 0 {
        outcome.report = Some(rows_text(&outcome.rows));
        outcome.unchanged = true;
        return Ok(outcome);
    }
    let question = format!(
        "lock {} rev {rev} (editorial: spec_hash of {count} entries) at {} on {} in {}? [y/N]",
        node.canonical, repo.head, repo.branch, repo.worktree
    );
    let message_node = node.canonical.clone();
    write_and_commit(
        env,
        project,
        repo,
        &read,
        &refreshed.lock,
        &message_node,
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
    let question = format!(
        "lock {written} (gone: {} dropped) at {} on {} in {}? [y/N]",
        dropped.len(),
        repo.head,
        repo.branch,
        repo.worktree
    );
    write_and_commit(
        env, project, repo, &read, &lock, written, question, outcome, consent, trailers,
    )
}

// ------------------------------------------------------------ the commit

/// Steps 8 to 10: the identity, an ignored first lock, the question, the
/// place and the bytes read again, the write and the commit.
#[allow(clippy::too_many_arguments)]
fn write_and_commit(
    env: &Env,
    project: &ProjectRoot,
    repo: &Repo,
    read: &ReadLock,
    lock: &Lock,
    node: &str,
    question: String,
    mut outcome: LockOutcome,
    consent: LockConsent<'_>,
    trailers: &[LockTrailer],
) -> Result<LockOutcome, CliError> {
    let rows = rows_text(&outcome.rows);
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
        return Ok(outcome.refused(CHANGED_WHILE_ACCEPTING));
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
                    "{LOCK_FILE} appeared while being accepted; nothing written"
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
        "spec: lock {node}\n\n{LOCK_TRAILER}: {node}\n{MODE_TRAILER}: {}\n{DECIDED_BY_TRAILER}: \
         {decided_by}\n",
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
    let check = |commit: &str| check_lock_commit(repo, commit, &top_path, node, &rendered);
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
    match committed {
        Ok(()) => match tip() {
            Ok(Some(commit)) if commit != old => {
                outcome.commit = Some(commit.clone());
                match check(&commit) {
                    Check::Ours => Ok(outcome),
                    // Made, and not the lock alone: never rewritten here.
                    Check::NotAlone(wrong) => Ok(outcome.refused(&format!(
                        "the lock commit {commit} on `{branch}` is not this run's {LOCK_FILE} \
                         alone ({}); it stays as git made it: {}",
                        wrong.join("; "),
                        undo(&commit)
                    ))),
                    Check::Unverified(why) => Ok(outcome.refused(&format!(
                        "the lock commit {commit} on `{branch}` was made but cannot be verified \
                         ({}); it stays as git made it: if it is not the lock alone, {}",
                        why.join("; "),
                        undo(&commit)
                    ))),
                }
            }
            Ok(Some(_)) => {
                // The branch did not move: the commit is not on it.
                if let Ok(Some(head)) = repo.git.head()
                    && head != old
                {
                    return Ok(outcome.refused(&format!(
                        "git reported the commit of {LOCK_FILE} made, yet `{branch}` did not move \
                         from {old} and HEAD is {head}: the commit is unverified and {LOCK_FILE} \
                         is left as written"
                    )));
                }
                let restored = undo_back.restore(true, &mut outcome);
                Ok(outcome.refused(&format!(
                    "git reported the commit of {LOCK_FILE} made, yet `{branch}` did not move from \
                     {old}: {restored}; nothing committed"
                )))
            }
            Ok(None) => Ok(outcome.refused(&format!(
                "git reported the commit of {LOCK_FILE} made, yet `{branch}` names no commit now: \
                 it is unverified{}",
                head_hint(repo)
            ))),
            Err(error) => Ok(outcome.refused(&format!(
                "git reported the commit of {LOCK_FILE} made, but `{branch}` cannot be read \
                 ({error}): it is unverified{}",
                head_hint(repo)
            ))),
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
                Check::Unverified(why) => Ok(outcome.refused(&format!(
                    "the commit of {LOCK_FILE} failed ({error}); `{branch}` moved to {commit}, \
                     which cannot be verified ({}): {LOCK_FILE} is left as written",
                    why.join("; ")
                ))),
            },
            _ => {
                let restored = undo_back.restore(false, &mut outcome);
                Ok(outcome.refused(&format!(
                    "the commit of {LOCK_FILE} failed, {restored}: {error}"
                )))
            }
        },
    }
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
    /// This run's lock commit: all four checks hold.
    Ours,
    /// Some check fails: the reasons.
    NotAlone(Vec<String>),
    /// No check fails, but one could not be read: the reasons.
    Unverified(Vec<String>),
}

/// `commit` against this run's lock commit (`spec approve`'s three checks
/// and one more): its parents the old `HEAD` alone, `spec.lock` the only
/// path it changes, a `Lock:` trailer naming the node, and its `spec.lock`
/// blob the bytes the owner confirmed (a hook that rewrote and staged the
/// lock, a concurrent accept of the node, fail it).
fn check_lock_commit(
    repo: &Repo,
    commit: &str,
    top_path: &str,
    node: &str,
    rendered: &str,
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
    match repo.git.trailer_values(commit, LOCK_TRAILER) {
        Ok(values) if values.iter().any(|value| value == node) => {}
        Ok(_) => wrong.push(format!("it has no `{LOCK_TRAILER}: {node}` trailer")),
        Err(error) => unread.push(format!("its trailers cannot be read: {error}")),
    }
    match repo.git.blob_at(commit, top_path) {
        Ok(bytes) if bytes == rendered.as_bytes() => {}
        Ok(_) => wrong.push(format!(
            "its {top_path} is not the bytes the owner confirmed"
        )),
        // A commit without the file fails the changed paths above already.
        Err(error) => unread.push(format!("its {top_path} cannot be read: {error}")),
    }
    if !wrong.is_empty() {
        Check::NotAlone(wrong)
    } else if !unread.is_empty() {
        Check::Unverified(unread)
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
    /// touched. The refusal's clause.
    fn restore(&self, index: bool, outcome: &mut LockOutcome) -> String {
        let now = fs::read(self.path).ok();
        if now.as_deref() != Some(self.rendered) {
            return format!(
                "{LOCK_FILE} was changed meanwhile, so it is left as it is, not restored"
            );
        }
        let top_path = self.top_path;
        match (self.created, &self.read.bytes) {
            (Some(created), _) => {
                if let Err(error) = self.repo.git.remove_cached(top_path) {
                    outcome.messages.push(Message::Warning(format!(
                        "cannot drop the index entry of {LOCK_FILE}: {error}; `git rm --cached -- \
                         {top_path}` drops it"
                    )));
                }
                for left in created.remove() {
                    outcome.messages.push(Message::Warning(left));
                }
            }
            (None, Some(bytes)) => {
                if let Err(error) = replace_file(self.path, bytes) {
                    outcome.messages.push(Message::Warning(format!(
                        "cannot restore {LOCK_FILE}: {error}; `git checkout -- {top_path}` \
                         restores it"
                    )));
                }
                if index && let Err(error) = self.repo.git.reset_path(top_path) {
                    outcome.messages.push(Message::Warning(format!(
                        "cannot set the index entry of {LOCK_FILE} back to HEAD's: {error}; `git \
                         reset -q -- {top_path}` does"
                    )));
                }
            }
            (None, None) => {}
        }
        "its bytes restored".to_owned()
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
