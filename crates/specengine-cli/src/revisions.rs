//! Revisions at apply (task spec `rev-discipline`, slice 4b-1; canon
//! `node-revision`, "Raising"): what `spec approve` adds to
//! steps 3, 6, 8, 9, 10 and the completion of an update, an
//! interpretation and a section-form create, over core's pure half
//! ([`specengine_core::raise`]).
//!
//! - **Step 6, creation's check 3**: a text changing a revision as core's
//!   `revision` reads it refuses ([`revision_refusal`]): approval sets
//!   revisions, a proposal never.
//! - **Step 6, the raise** ([`step_6`]): each node with an ID in both the
//!   file as read (step 3) and the file step 5 gives whose meaning hash
//!   changes is raised from its current revision, never lowered. An
//!   unreadable revision refuses any approve (exit 1, before the prompt;
//!   the review's preview `unavailable`). Every node is judged in one pass:
//!   one at `REV_MAX`, a document of a closed class that admits no `rev`,
//!   a revision that cannot be written is that node's refusal, which
//!   refuses a meaning approve before the prompt ([`Revisions::refusal`])
//!   and which the review lists, its preview the text an editorial approve
//!   writes (task spec `rev-discipline` Q15). `--editorial` keeps them.
//! - **Step 3 under `--editorial`** ([`step_3_lock`]): `<root>/spec.lock`
//!   read: none, the text alone; else tracked and equal to `HEAD` (exit
//!   1), parsed without the build's recipes and its `norm_recipe` compared
//!   (exit 2); `code_recipe` is not read: the refresh writes no code field.
//!   At step 6 each changed node's vouching entries (at its revision, its
//!   old `spec_hash`) take the new one (core `refresh_editorial` with the
//!   vouching filter); any other entry stays byte for byte.
//! - **Steps 8 to 10**: the lock re-read and written after the text, both
//!   committed in one commit, restored by accept's rules on a failure
//!   ([`restore`]), and checked at step 10 ([`lock_in_commit`]).
//! - **Completion** ([`raised_span`]): the text step 5 gives on a commit's
//!   parent, raised as an apply would raise it.
//! - **Review** ([`rows`]): the document's `revisions`.

use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use specengine_core::check::CheckConfig;
use specengine_core::lock::{LOCK_FILE, Lock, parse_lock, refresh_editorial, render_lock, vouches};
use specengine_core::patch::{is_section, span_bytes, splice, update_text};
use specengine_core::proposal::{
    ApplyTrailers, EDITORIAL, LOCK_MODE_TRAILER, LOCK_TRAILER, Revised,
};
use specengine_core::raise::{
    RaiseRefusal, RevisionRow, closed_against_rev, meaning_changes, raised, revision_change,
    write_revisions,
};
use specengine_core::{NORM_RECIPE, norm_input};
use specengine_model::{IdScheme, Node, ParsedFile};
use specengine_store::{WorktreeGit, b3_hash, replace_file};

use crate::preflight::StepFailure;
use crate::project::CONFIG_FILE;
use crate::{Message, one_line, pathspec_word};

/// Step 8's refusal when the lock changed after step 3.
const LOCK_CHANGED: &str = "spec.lock changed while being applied; nothing written";

/// The `norm_hash` of node `ord` of `parsed` (the parse of `bytes`);
/// `None` when null.
pub(crate) fn norm_hash(parsed: &ParsedFile, bytes: &[u8], ord: usize) -> Option<String> {
    norm_input(parsed, bytes, ord).map(|input| b3_hash(&input))
}

/// Creation's check 3 and apply's step 6 (task spec `rev-discipline` Q7):
/// the refusal of a text (`after`) changing a revision of `before`, its
/// node named by `name` (the canonical ID of a Latin ID); `None` when no
/// revision changes.
pub(crate) fn revision_refusal(
    before: &ParsedFile,
    after: &ParsedFile,
    name: &dyn Fn(&str) -> String,
) -> Option<String> {
    revision_change(before, after).map(|change| change.refusal(&name(&change.id)))
}

/// One node whose meaning an apply changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NodeRevision {
    /// The canonical ID.
    pub id: String,
    /// Its revision in the file as read.
    pub rev: u32,
    /// A meaning approve's raise, or why step 6 refuses it; `None` under
    /// `--editorial` (kept).
    pub raise: Option<Result<u32, RaiseRefusal>>,
    /// Its position in the file written.
    pub ord: usize,
    /// Its `norm_hash` in the file as read and in the file written.
    pub old_hash: Option<String>,
    pub new_hash: Option<String>,
}

impl NodeRevision {
    /// What a meaning approve raises it to; `None` under `--editorial` or
    /// when the raise is refused.
    pub(crate) fn to(&self) -> Option<u32> {
        match &self.raise {
            Some(Ok(to)) => Some(*to),
            _ => None,
        }
    }

    /// Why a meaning approve refuses its raise: step 6's reason, as a
    /// refusal prints it after `(step 6): `; `None` when it raises or
    /// under `--editorial`.
    pub(crate) fn refused(&self) -> Option<String> {
        match &self.raise {
            Some(Err(refusal)) => Some(one_line(&refusal.message(&self.id))),
            _ => None,
        }
    }

    /// Its old and new `norm_hash` when both are known: an editorial
    /// approve refreshes its vouching entries; else it has none to refresh.
    fn refreshable(&self) -> Option<(&str, &str)> {
        Some((self.old_hash.as_deref()?, self.new_hash.as_deref()?))
    }
}

/// `spec.lock` as step 3 read it under `--editorial`.
#[derive(Debug, Clone)]
pub(crate) struct LockRead {
    /// On disk.
    pub path: PathBuf,
    /// From the worktree's top.
    pub top_path: String,
    pub bytes: Vec<u8>,
    pub lock: Lock,
}

/// The lock an editorial apply writes beside its text.
#[derive(Debug, Clone)]
pub(crate) struct LockWrite {
    pub path: PathBuf,
    pub top_path: String,
    /// As step 3 read them.
    pub bytes: Vec<u8>,
    /// What step 8 writes.
    pub rendered: String,
    /// The nodes whose entries change, in file order.
    pub locked: Vec<String>,
    /// How many entries change.
    pub entries: usize,
}

/// What step 6 decided about revisions: per node whose meaning changes,
/// its raise or why not, and the bytes a meaning approve writes.
#[derive(Debug, Clone, Default)]
pub(crate) struct Revisions {
    pub editorial: bool,
    /// The nodes whose meaning changes, in file order.
    pub nodes: Vec<NodeRevision>,
    /// Under `--editorial` with entries to refresh.
    pub lock: Option<LockWrite>,
    /// Step 5's text with every node's raise written: what a meaning
    /// approve writes. `None` under `--editorial`, when no node is raised
    /// (step 5's text is written as it is), or when a raise is refused.
    pub raised: Option<Vec<u8>>,
}

impl Revisions {
    /// A meaning approve's refusal at step 6, before the prompt: the first
    /// node, in file order, whose raise is refused; `None` when every one
    /// raises, or under `--editorial`.
    pub(crate) fn refusal(&self) -> Option<StepFailure> {
        self.nodes
            .iter()
            .find_map(NodeRevision::refused)
            .map(|reason| StepFailure::refused(6, reason))
    }

    /// The question's part after the preview: `; raises R-12 3 -> 4, R-13
    /// 1 -> 2`, or `, editorial: R-12 stays 3; spec.lock: 2 entries`; empty
    /// when no node's meaning changes.
    pub(crate) fn question(&self) -> String {
        if self.nodes.is_empty() {
            return String::new();
        }
        if !self.editorial {
            let raised: Vec<String> = self
                .nodes
                .iter()
                .map(|node| match node.to() {
                    Some(to) => format!("{} {} -> {to}", node.id, node.rev),
                    None => format!("{} stays {}", node.id, node.rev),
                })
                .collect();
            return format!("; raises {}", raised.join(", "));
        }
        let kept: Vec<String> = self
            .nodes
            .iter()
            .map(|node| format!("{} stays {}", node.id, node.rev))
            .collect();
        let mut part = format!(", {EDITORIAL}: {}", kept.join(", "));
        if let Some(lock) = &self.lock {
            part.push_str(&format!("; {LOCK_FILE}: {}", entries(lock.entries)));
        }
        part
    }

    /// Under `--editorial` with no node's meaning changing: the note.
    pub(crate) fn editorial_note(&self) -> Option<String> {
        (self.editorial && self.nodes.is_empty())
            .then(|| format!("no node's meaning changes; --{EDITORIAL} changes nothing"))
    }

    /// The commit's `Revision:` trailers' facts, in file order.
    pub(crate) fn revised(&self) -> Vec<Revised<'_>> {
        self.nodes
            .iter()
            .map(|node| Revised {
                id: &node.id,
                rev: node.rev,
                to: node.to(),
            })
            .collect()
    }

    /// The nodes the commit's `Lock:` trailers name.
    pub(crate) fn locked(&self) -> Vec<&str> {
        self.lock
            .iter()
            .flat_map(|lock| lock.locked.iter().map(String::as_str))
            .collect()
    }
}

/// `N entry` or `N entries`.
fn entries(count: usize) -> String {
    if count == 1 {
        "1 entry".to_owned()
    } else {
        format!("{count} entries")
    }
}

/// The trailers of [`ApplyTrailers`] over `revised` and `locked`.
pub(crate) fn apply_trailers<'a>(
    revised: &'a [Revised<'a>],
    locked: &'a [&'a str],
) -> ApplyTrailers<'a> {
    ApplyTrailers { revised, locked }
}

/// Step 3 under `--editorial`: `<root>/spec.lock` (`top_path` from the
/// worktree's top), its state against `head`. None in the worktree nor at
/// `head`: `None`, the text alone. Tracked and equal to `head`, else exit
/// 1 naming its state; read by core's `parse_lock` without the build's
/// recipes, its `norm_recipe` this build's, else exit 2 (`code_recipe` is
/// not read: the refresh writes no code field).
pub(crate) fn step_3_lock(
    git: &WorktreeGit,
    head: &str,
    root: &Path,
    top_path: &str,
) -> Result<Option<LockRead>, StepFailure> {
    let path = root.join(LOCK_FILE);
    let cannot = |error: &dyn std::fmt::Display| StepFailure::cannot(3, error.to_string());
    let at_head = git
        .blob_oid(head, top_path)
        .map_err(|error| cannot(&error))?;
    let bytes = match fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(StepFailure::cannot(
                3,
                format!("cannot read {LOCK_FILE}: {error}"),
            ));
        }
    };
    let dirty = |state: &str| {
        StepFailure::refused(
            3,
            format!(
                "`{LOCK_FILE}` {state}: --{EDITORIAL} sets its entries from its committed \
                 bytes; commit it first; nothing written"
            ),
        )
    };
    let entries = git
        .status_entries(&[top_path], true)
        .map_err(|error| cannot(&error))?;
    if let Some(entry) = entries.iter().find(|entry| entry.path == top_path) {
        return Err(dirty(status_state(entry)));
    }
    let Some(bytes) = bytes else {
        return match at_head {
            Some(_) => Err(dirty("deleted")),
            None => Ok(None),
        };
    };
    if at_head.is_none() {
        return Err(dirty("not tracked"));
    }
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Err(StepFailure::cannot(
            3,
            format!("{LOCK_FILE}: not UTF-8: SpecEngine writes it"),
        ));
    };
    let lock = parse_lock(text, None).map_err(|error| cannot(&error))?;
    if lock.recipes.norm != NORM_RECIPE {
        return Err(StepFailure::cannot(
            3,
            format!(
                "{LOCK_FILE}: its norm_recipe is `{}`, this build's `{NORM_RECIPE}`: its \
                 spec_hash values are not this build's; nothing written",
                lock.recipes.norm
            ),
        ));
    }
    Ok(Some(LockRead {
        path,
        top_path: top_path.to_owned(),
        bytes,
        lock,
    }))
}

/// A status entry's state, as accept names a dirty input.
fn status_state(entry: &specengine_store::StatusEntry) -> &'static str {
    match (entry.index, entry.worktree) {
        ('?', _) | (_, '?') => "untracked",
        (_, 'D') | ('D', _) => "deleted",
        (' ', _) => "modified",
        _ => "staged",
    }
}

/// What step 6 is given.
pub(crate) struct Step6<'a> {
    /// The target's file, root-relative.
    pub path: &'a str,
    /// The file as read (step 3) and its parse.
    pub before: &'a [u8],
    pub before_parsed: &'a ParsedFile,
    /// Step 5's text in the file, and its parse.
    pub after: &'a [u8],
    pub after_parsed: &'a ParsedFile,
    pub scheme: &'a IdScheme,
    /// The canonical ID of a Latin ID of this file.
    pub name: &'a dyn Fn(&str) -> String,
    /// The recorded root: its `specengine.toml`'s `[classes]`.
    pub root: &'a Path,
    pub editorial: bool,
    /// Step 3's lock under `--editorial`.
    pub lock: Option<LockRead>,
}

/// Step 6's revisions, every node judged in one pass: per node whose
/// meaning changes, a meaning approve's raise or why it is refused (none
/// under `--editorial`), and the bytes it writes when nothing is refused
/// ([`Revisions::raised`]); step 5's text is what an editorial approve
/// writes. Else the refusal of any approve: an unreadable current
/// revision, a `specengine.toml` that cannot be read, a parser failure.
pub(crate) fn step_6(given: Step6<'_>) -> Result<Revisions, StepFailure> {
    let changes = meaning_changes(
        given.before_parsed,
        given.before,
        given.after_parsed,
        given.after,
    );
    let mut nodes = Vec::with_capacity(changes.len());
    let mut classes = None;
    for change in &changes {
        let id = (given.name)(&change.id);
        // Unreadable: refused in either mode (nothing to keep or raise).
        let Some(rev) = change.current.number() else {
            return Err(StepFailure::refused(
                6,
                RaiseRefusal::Unreadable.message(&id),
            ));
        };
        let raise = if given.editorial {
            None
        } else {
            Some(match raised(change.current) {
                Ok(to) if change.after == 0 => {
                    let classes = match &classes {
                        Some(classes) => classes,
                        None => classes.insert(read_classes(given.root)?),
                    };
                    match closed_against_rev(given.after_parsed, &classes.classes) {
                        Some(class) => Err(RaiseRefusal::ClosedClass { class }),
                        None => Ok(to),
                    }
                }
                judged => judged,
            })
        };
        nodes.push(NodeRevision {
            id,
            rev,
            raise,
            ord: change.after,
            old_hash: norm_hash(given.before_parsed, given.before, change.before),
            new_hash: norm_hash(given.after_parsed, given.after, change.after),
        });
    }
    // The raises judged writable so far, written together; a node whose
    // revision cannot be written refused by name.
    let raises: Vec<(usize, u32)> = nodes
        .iter()
        .filter_map(|node| node.to().map(|to| (node.ord, to)))
        .collect();
    let mut raised_bytes = None;
    if !raises.is_empty() {
        let written = panic::catch_unwind(AssertUnwindSafe(|| {
            write_revisions(
                given.path,
                given.after,
                given.after_parsed,
                given.scheme,
                &raises,
            )
        }))
        .map_err(|_| {
            StepFailure::refused(6, format!("the spec parser failed on `{}`", given.path))
        })?;
        match written {
            Ok(written) => raised_bytes = Some(written.bytes),
            Err(errors) => {
                for error in errors {
                    if let Some(node) = nodes.iter_mut().find(|node| node.ord == error.ord) {
                        node.raise = Some(Err(RaiseRefusal::Unwritable { why: error.why }));
                    }
                }
            }
        }
    }
    let refused = nodes.iter().any(|node| matches!(node.raise, Some(Err(_))));
    let lock = match (&given.lock, given.editorial) {
        (Some(read), true) => editorial_refresh(read, &nodes),
        _ => None,
    };
    Ok(Revisions {
        editorial: given.editorial,
        nodes,
        lock,
        raised: raised_bytes.filter(|_| !refused),
    })
}

/// The recorded root's `[classes]`; exit 2 when its `specengine.toml`
/// cannot be read as the check reads it.
fn read_classes(root: &Path) -> Result<CheckConfig, StepFailure> {
    let file = root.join(CONFIG_FILE);
    let label = file.display().to_string();
    let text = fs::read_to_string(&file)
        .map_err(|error| StepFailure::cannot(6, format!("cannot read {label}: {error}")))?;
    CheckConfig::from_toml(&text)
        .map_err(|error| StepFailure::cannot(6, format!("{label}: {error}")))
}

/// Each changed node's vouching entries (at its revision, its old
/// `spec_hash`) set to its new `norm_hash`; `None` when none changes (the
/// text alone is committed). A node whose old or new hash is null has
/// none to refresh.
fn editorial_refresh(read: &LockRead, nodes: &[NodeRevision]) -> Option<LockWrite> {
    let mut lock = read.lock.clone();
    let mut locked = Vec::new();
    let mut count = 0;
    for node in nodes {
        let Some((old, new)) = node.refreshable() else {
            continue;
        };
        let refreshed = refresh_editorial(&lock, &node.id, node.rev, new, Some(old));
        if refreshed.refreshed() > 0 {
            count += refreshed.refreshed();
            locked.push(node.id.clone());
            lock = refreshed.lock;
        }
    }
    (count > 0).then(|| LockWrite {
        path: read.path.clone(),
        top_path: read.top_path.clone(),
        bytes: read.bytes.clone(),
        rendered: render_lock(&lock),
        locked,
        entries: count,
    })
}

/// Step 8 under `--editorial`: the lock read again, still step 3's bytes;
/// else the refusal.
pub(crate) fn lock_unchanged(lock: &LockWrite) -> Result<(), StepFailure> {
    match fs::read(&lock.path) {
        Ok(bytes) if bytes == lock.bytes => Ok(()),
        _ => Err(StepFailure::refused(8, LOCK_CHANGED)),
    }
}

/// How one file fared when put back after a failed commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Restored {
    /// Its old bytes are back.
    Back,
    /// It no longer held this run's bytes: left as it is.
    Left,
    /// Restoring failed: a warning names the git command.
    Failed,
}

/// `path` (`top_path` from the top of `worktree`) put back to `old` only
/// while it still holds `written`, this run's bytes (accept's rule, CLI
/// `lock.rs` `Undo`); a failure is a warning naming the git command that
/// restores it and the worktree it runs in.
pub(crate) fn restore(
    path: &Path,
    top_path: &str,
    worktree: &str,
    written: &[u8],
    old: &[u8],
    messages: &mut Vec<Message>,
) -> Restored {
    if fs::read(path).ok().as_deref() != Some(written) {
        return Restored::Left;
    }
    match replace_file(path, old) {
        Ok(()) => Restored::Back,
        Err(error) => {
            messages.push(Message::Warning(format!(
                "cannot restore `{top_path}` in {worktree}: {error}; `git checkout -- {}` \
                 restores it",
                pathspec_word(top_path)
            )));
            Restored::Failed
        }
    }
}

/// The refusal's clause for the files a failed editorial apply put back
/// (the text and the lock after a failed commit, the text alone after a
/// failed lock write): `their bytes restored` (one file: `` `<path>`
/// restored ``), else each named: left as it is (changed meanwhile) or not
/// restored (see the warning); never "restored" for one that is not.
pub(crate) fn restored_clause(files: &[(&str, Restored)]) -> String {
    if files.len() > 1
        && files
            .iter()
            .all(|(_, restored)| *restored == Restored::Back)
    {
        return "their bytes restored".to_owned();
    }
    let parts: Vec<String> = files
        .iter()
        .map(|(path, restored)| match restored {
            Restored::Back => format!("`{path}` restored"),
            Restored::Left => {
                format!("`{path}` was changed meanwhile, so it is left as it is, not restored")
            }
            Restored::Failed => format!("`{path}` is not restored (see the warning)"),
        })
        .collect();
    parts.join(", ")
}

/// Step 10's checks of an editorial commit beside one parent, the old
/// `HEAD`, and its `Proposal:` trailer: exactly the text's path and the
/// lock's changed; a `Lock:` trailer per node whose entries changed; its
/// lock blob the bytes confirmed (a hook rewriting the lock fails it, a
/// commit deleting it too); each refreshed `spec_hash` the committed text
/// blob's `norm_hash`. Else why not.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lock_in_commit(
    git: &WorktreeGit,
    old_head: &str,
    commit: &str,
    text_top: &str,
    lock: &LockWrite,
    nodes: &[NodeRevision],
    path: &str,
    scheme: &IdScheme,
) -> Result<(), String> {
    let mut changed = git
        .changed_paths(old_head, commit)
        .map_err(|error| error.to_string())?;
    changed.sort();
    let mut wanted = vec![text_top.to_owned(), lock.top_path.clone()];
    wanted.sort();
    if changed != wanted {
        return Err(format!(
            "it changes {}, not {} alone",
            if changed.is_empty() {
                "nothing".to_owned()
            } else {
                changed.join(", ")
            },
            wanted.join(" and ")
        ));
    }
    let named = git
        .trailer_values(commit, LOCK_TRAILER)
        .map_err(|error| error.to_string())?;
    if let Some(missing) = lock.locked.iter().find(|id| !named.contains(id)) {
        return Err(format!("it has no `{LOCK_TRAILER}: {missing}` trailer"));
    }
    let top = &lock.top_path;
    match git
        .blob_oid(commit, top)
        .map_err(|error| error.to_string())?
    {
        None => return Err(format!("it has no {top}")),
        Some(_) => {
            let bytes = git
                .blob_at(commit, top)
                .map_err(|error| error.to_string())?;
            if bytes != lock.rendered.as_bytes() {
                return Err(format!("its {top} is not the bytes the owner confirmed"));
            }
        }
    }
    let text = git
        .blob_at(commit, text_top)
        .map_err(|error| error.to_string())?;
    let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
        specengine_core::parse(path, &text, scheme)
    }))
    .map_err(|_| format!("the spec parser failed on its `{text_top}`"))?;
    for node in nodes.iter().filter(|node| lock.locked.contains(&node.id)) {
        if norm_hash(&parsed, &text, node.ord) != node.new_hash {
            return Err(format!(
                "the spec_hash it sets for `{}` is not the norm_hash of its `{text_top}`",
                node.id
            ));
        }
    }
    Ok(())
}

/// A commit's `Lock-mode:` trailer says `editorial`: the commit of an
/// editorial apply may change `spec.lock` beside its text (completion,
/// reject's lookup).
pub(crate) fn editorial_commit(git: &WorktreeGit, commit: &str) -> bool {
    git.trailer_values(commit, LOCK_MODE_TRAILER)
        .is_ok_and(|values| values.iter().any(|value| value == EDITORIAL))
}

/// Completion (task spec `rev-discipline` Q12): node `ord` of `parsed`
/// (the parse of `bytes`, named `path`) with `text` spliced in as step 5
/// does, its revisions then raised as a meaning approve raises them; the
/// node's span there. `None` when nothing is raised or the raise is
/// refused (the text as spliced is then the only one).
pub(crate) fn raised_span(
    path: &str,
    bytes: &[u8],
    parsed: &ParsedFile,
    ord: usize,
    text: &str,
    scheme: &IdScheme,
) -> Option<Vec<u8>> {
    let node: &Node = parsed.nodes.get(ord)?;
    let spliced = splice(bytes, node.span, update_text(text, is_section(node)));
    panic::catch_unwind(AssertUnwindSafe(|| {
        let after = specengine_core::parse(path, &spliced, scheme);
        let raises: Vec<(usize, u32)> = meaning_changes(parsed, bytes, &after, &spliced)
            .iter()
            .map(|change| raised(change.current).ok().map(|to| (change.after, to)))
            .collect::<Option<_>>()?;
        if raises.is_empty() {
            return None;
        }
        let written = write_revisions(path, &spliced, &after, scheme, &raises).ok()?;
        let target = written.parsed.nodes.get(ord)?;
        Some(span_bytes(&written.bytes, target).to_vec())
    }))
    .ok()
    .flatten()
}

/// The lock a review counts `lock_entries` in: `<root>/spec.lock` as it is
/// in the worktree (none: `Ok(None)`), read as step 3 reads it under
/// `--editorial` but for its state; else why not.
pub(crate) fn lock_for_review(root: &Path) -> Result<Option<Lock>, String> {
    let bytes = match fs::read(root.join(LOCK_FILE)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot read {LOCK_FILE}: {error}")),
    };
    let text = std::str::from_utf8(&bytes).map_err(|_| format!("{LOCK_FILE}: not UTF-8"))?;
    let lock = parse_lock(text, None).map_err(|error| error.to_string())?;
    if lock.recipes.norm != NORM_RECIPE {
        return Err(format!(
            "{LOCK_FILE}: its norm_recipe is `{}`, this build's `{NORM_RECIPE}`",
            lock.recipes.norm
        ));
    }
    Ok(Some(lock))
}

/// The review document's `revisions` (task spec `rev-discipline`, "Review
/// document, stage, queue"): per node whose meaning changes (step 6's
/// plan of a meaning approve), its revision now, a meaning approve's
/// (`None` when refused), the entries an editorial one refreshes (`lock`:
/// none, 0; unreadable, `None`; a node with a null `norm_hash` before or
/// after, 0, as the refresh skips it) and the refusal of its raise.
pub(crate) fn rows(
    nodes: &[NodeRevision],
    lock: &Result<Option<Lock>, String>,
) -> Vec<RevisionRow> {
    nodes
        .iter()
        .map(|node| RevisionRow {
            id: node.id.clone(),
            rev: node.rev,
            to: node.to(),
            lock_entries: match lock {
                Err(_) => None,
                Ok(None) => Some(0),
                Ok(Some(lock)) => Some(match node.refreshable() {
                    None => 0,
                    Some((old, _)) => lock
                        .entries_of(&node.id)
                        .filter(|(key, verified)| vouches(key, verified, &node.id, node.rev, old))
                        .count(),
                }),
            },
            refused: node.refused(),
        })
        .collect()
}

/// `revisions: R-12 3 -> 4 (2 lock entries), R-13 1 -> refused (0 lock
/// entries)` without its key: the review's text; `none` for no row.
pub(crate) fn rows_text(rows: &[RevisionRow]) -> String {
    if rows.is_empty() {
        return "none".to_owned();
    }
    rows.iter()
        .map(|row| {
            let lock = match row.lock_entries {
                Some(1) => "1 lock entry".to_owned(),
                Some(count) => format!("{count} lock entries"),
                None => "lock entries unknown".to_owned(),
            };
            let to = row
                .to
                .map_or_else(|| "refused".to_owned(), |to| to.to_string());
            format!("{} {} -> {to} ({lock})", row.id, row.rev)
        })
        .collect::<Vec<_>>()
        .join(", ")
}
