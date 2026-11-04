//! A hub's sync (task spec `hub-git`): its checkout's branch carried to the
//! upstream in four phases, the look, the fetch, the merge and the push
//! ([`sync_once`]), git's own verbs spawned by store `sync.rs` alone. Only
//! the merge takes turns: the caller's closure ([`sync_once`]'s `in_turns`)
//! runs it inside every served project's turn of the repository, so a slow
//! remote never delays a read or a confirm.
//!
//! - **The merge**: nothing when the tracking ref `R` is `HEAD` (`L`) or an
//!   ancestor of it; a fast-forward when `L` is an ancestor of `R`; `held`
//!   with no merge base; else `merge-tree --write-tree` in memory. A clean
//!   merge, or one whose every conflict is an output of a served root's
//!   `index = true` entry by the merged, unconflicted config, becomes a merge
//!   commit, parents `L` then `R`, git's default message and no trailer,
//!   under the checkout's identity, its outputs rendered from the merged
//!   tree (a conflicted one read at `L`'s bytes first); reached by `merge
//!   --ff-only`. Any other conflict is `held`, the paths named, nothing
//!   touched. No `MERGE_HEAD` ever exists.
//! - **The push**: accepted, `ok`; not a fast-forward, phases 2-4 again, at
//!   most three rounds; refused by the remote, `held`; anything else,
//!   `retrying`. Pushed is never stored: [`hub_sync`] computes `ahead` and
//!   `behind` from git on each read.
//! - **`held`** keeps its `(L, R)`: a later sync fetches, and merges or
//!   pushes only once a tip moved.
//!
//! - **The index**: a fast-forward that moved `HEAD`, inside the turns,
//!   updates each served root's index over the paths `git diff` names
//!   between the old `HEAD` and the new, by the merged config (a moved
//!   `[paths]` or `[ids]` walks); a failed diff doubts every root, a failed
//!   discovery or update its own, and a failed fast-forward every root (the
//!   `read-trust` spec, R11 and R12; [`crate::freshness`]).
//!
//! The sync authors no text and is no apply: its merge commit's only
//! SpecEngine bytes are the rendered outputs. Nothing here blocks, and no
//! status, check or gate reads it: a discrepancy blocks nothing.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use specengine_core::card::is_format_character;
use specengine_store::{
    CONFIG_FILE, CommitFailure, GitEnv, GitError, IndexWriter as _, Looked, NamedBytes, NetFailure,
    Pushed, Relation, Replacement, STDERR_MAX, SyncGit, Upstream, WorkingTree, ahead_behind,
    file_mode, load_config, named, sync_place,
};

use crate::freshness;
use crate::location::open_index;
use crate::outputs::{TreeRender, index_outputs, render_tree};
use crate::project::discover;
use crate::{Env, Globals};

/// Phases 2-4 at most this many times when the remote moves meanwhile.
const ROUNDS: usize = 3;

/// The state of a hub's sync, as `GET .../sync` names it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SyncState {
    /// No sync ended since start.
    #[default]
    None,
    /// The remote branch holds `HEAD`.
    Ok,
    /// A fetch or push failed, no ref refused; tried again with backoff.
    Retrying,
    /// A conflict, unrelated histories, a remote's refusal: tried once a tip
    /// moved.
    Held,
    /// The look failed, a merge lacked an identity, or the root's worktree
    /// is not the synced one: looked at again on every tick and signal.
    Stopped,
}

impl SyncState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Ok => "ok",
            Self::Retrying => "retrying",
            Self::Held => "held",
            Self::Stopped => "stopped",
        }
    }
}

/// One repository a hub syncs: the top of its checkout, and the canonical
/// roots served in that repository, in `--root` order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncRepository {
    pub top: PathBuf,
    pub roots: Vec<PathBuf>,
}

/// What a sync ended in; the daemon keeps the last one in memory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncOutcome {
    pub state: SyncState,
    /// The daemon's words, then git's stderr or a refusal's reason where git
    /// spoke, the git part cut so that the shown error stays within 4096
    /// bytes ([`bounded`]); raw (escaped when shown, [`shown_error`]), never a
    /// URL.
    pub error: Option<String>,
    /// `held`: the tips `(L, R)` it held on.
    pub held: Option<(String, String)>,
}

impl SyncOutcome {
    /// `stopped`, `why` its error.
    pub fn stopped(why: impl Into<String>) -> Self {
        Self::ended(SyncState::Stopped, why)
    }

    fn ended(state: SyncState, error: impl Into<String>) -> Self {
        Self {
            state,
            error: Some(bounded(error.into())),
            held: None,
        }
    }

    fn ok() -> Self {
        Self {
            state: SyncState::Ok,
            error: None,
            held: None,
        }
    }

    fn held(error: impl Into<String>, tips: (String, String)) -> Self {
        Self {
            state: SyncState::Held,
            error: Some(bounded(error.into())),
            held: Some(tips),
        }
    }
}

/// `error` cut at its end, on a character, so that its shown form
/// ([`shown_error`], an escape counted at its written length) and the
/// ellipsis marking the cut stay within [`STDERR_MAX`] bytes. The daemon's
/// words come first, so the cut falls in git's part.
fn bounded(error: String) -> String {
    let shown = |c: char| escaped(c.encode_utf8(&mut [0; 4]), false).len();
    if error.chars().map(shown).sum::<usize>() <= STDERR_MAX {
        return error;
    }
    let budget = STDERR_MAX - '…'.len_utf8();
    let mut used = 0;
    let mut out = String::new();
    for c in error.chars() {
        used += shown(c);
        if used > budget {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

/// `GET .../sync` (daemon-keys set `HubSync`): every key always present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HubSync {
    /// Computed per request; `null` with no tracking ref.
    pub ahead: Option<u64>,
    pub behind: Option<u64>,
    pub state: &'static str,
    /// Escaped ([`shown_error`]); `null` with none.
    pub error: Option<String>,
    /// The last sync's end, RFC 3339 UTC; `null` before one.
    pub at: Option<String>,
}

/// The top of the worktree holding `root` and its repository's common
/// directory, both canonical: one sync per common directory. Else why
/// not, git's words.
pub fn sync_key(git: &GitEnv, root: &Path) -> Result<(PathBuf, PathBuf), String> {
    sync_place(root, git).map_err(|error| error.to_string())
}

/// The status document of the repository whose checkout is `top`: `ahead`
/// and `behind` from git now (each child killed past `deadline`), the rest
/// as the daemon's memory gives it.
pub fn hub_sync(
    top: &Path,
    git: &GitEnv,
    deadline: Duration,
    last: &SyncOutcome,
    at: Option<&str>,
) -> HubSync {
    let counts = ahead_behind(top, git, deadline);
    HubSync {
        ahead: counts.map(|(ahead, _)| ahead),
        behind: counts.map(|(_, behind)| behind),
        state: last.state.as_str(),
        error: last.error.as_deref().map(shown_error),
        at: at.map(str::to_owned),
    }
}

/// The stderr line of a changed outcome: `specengine-http: sync
/// <worktree>: <state>[: <error>]`, on one line (every control escaped).
pub fn sync_line(top: &Path, outcome: &SyncOutcome) -> String {
    let mut line = format!(
        "specengine-http: sync {}: {}",
        top.display(),
        outcome.state.as_str()
    );
    if let Some(error) = &outcome.error {
        line.push_str(": ");
        line.push_str(&escaped(error, true));
    }
    line
}

/// `error` as the status shows it: every control but LF, and every format
/// character (Unicode Cf), written `\u{XX}`, as a confirm's question is.
pub fn shown_error(error: &str) -> String {
    escaped(error, false)
}

fn escaped(text: &str, line_feed_too: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if (c == '\n' && !line_feed_too) || !(c.is_control() || is_format_character(c)) {
            out.push(c);
        } else {
            out.push_str(&format!("\\u{{{:x}}}", u32::from(c)));
        }
    }
    out
}

/// One sync of `repository` (see the module documentation), git with
/// `git`'s variables, each child killed past `deadline`. `last` is the
/// daemon's last outcome: a `held` one is kept while neither tip moved.
/// `in_turns` runs the merge it is given inside the turns of every project
/// served in the repository, in `--root` order; `env` finds each root's
/// project and index for the merge's update.
pub fn sync_once(
    repository: &SyncRepository,
    env: &Env,
    git: &GitEnv,
    deadline: Duration,
    last: &SyncOutcome,
    in_turns: &mut dyn FnMut(&mut dyn FnMut()),
) -> SyncOutcome {
    let stopped = |why: String| SyncOutcome::ended(SyncState::Stopped, why);
    let sync = match SyncGit::open(&repository.top, git, deadline) {
        Ok(sync) => sync,
        Err(error) => return stopped(error.to_string()),
    };
    let (upstream, mut provisional) = match sync.look(&repository.roots) {
        Ok(Looked::Ready { upstream, unclean }) => (upstream, unclean),
        Ok(Looked::Stopped(why)) => return stopped(why),
        Err(error) => return stopped(local_failure(&error, &sync)),
    };
    let seconds = sync.deadline_seconds();
    let remote = upstream.remote.clone();
    for round in 0..ROUNDS {
        match sync.fetch(&upstream) {
            Ok(()) => {}
            Err(NetFailure::Failed(stderr)) => {
                return SyncOutcome::ended(
                    SyncState::Retrying,
                    format!("fetch from `{remote}` failed: {stderr}"),
                );
            }
            Err(NetFailure::Deadline) => {
                return SyncOutcome::ended(
                    SyncState::Retrying,
                    format!("the fetch passed its deadline of {seconds} s"),
                );
            }
        }
        let (left, right) = match tips(&sync, &upstream) {
            Ok(tips) => tips,
            Err(outcome) => return outcome,
        };
        if round == 0
            && last.state == SyncState::Held
            && last.held.as_ref() == Some(&(left.clone(), right.clone()))
        {
            return last.clone();
        }
        // An unclean look is provisional: the turns judge the worktree
        // before anything is called pushed.
        if left == right && !provisional {
            return SyncOutcome::ok();
        }
        let mut merged = None;
        in_turns(&mut || {
            merged = Some(merge(env, &sync, repository, &upstream));
        });
        provisional = false;
        match merged {
            Some(Merged::Same) => return SyncOutcome::ok(),
            Some(Merged::Done) => {}
            Some(Merged::Ended(outcome)) => return outcome,
            None => return stopped("the merge did not run".to_owned()),
        }
        match sync.push(&upstream) {
            Pushed::Accepted => return SyncOutcome::ok(),
            Pushed::NotFastForward => {}
            Pushed::Refused { reason, stderr } => {
                let error = format!(
                    "the remote refused `{}`: {reason}; {stderr}",
                    upstream.merge
                );
                return match tips(&sync, &upstream) {
                    Ok(tips) => SyncOutcome::held(error, tips),
                    Err(outcome) => outcome,
                };
            }
            Pushed::Failed(stderr) => {
                return SyncOutcome::ended(
                    SyncState::Retrying,
                    format!("push to `{remote}` failed: {stderr}"),
                );
            }
            Pushed::Deadline => {
                return SyncOutcome::ended(
                    SyncState::Retrying,
                    format!("the push passed its deadline of {seconds} s"),
                );
            }
        }
    }
    SyncOutcome::ended(
        SyncState::Retrying,
        format!(
            "`{}` moved during three rounds: trying again later",
            upstream.shown()
        ),
    )
}

/// `L` and `R` after a fetch, both present; else the outcome.
fn tips(sync: &SyncGit, upstream: &Upstream) -> Result<(String, String), SyncOutcome> {
    match sync.tips(upstream) {
        Ok((Some(left), Some(right))) => Ok((left, right)),
        Ok((None, _)) => Err(SyncOutcome::ended(
            SyncState::Stopped,
            format!(
                "`{}` has no commit: a sync carries commits",
                upstream.branch
            ),
        )),
        Ok((_, None)) => Err(SyncOutcome::ended(
            SyncState::Retrying,
            format!(
                "fetch from `{}` failed: `{}` is not there after it",
                upstream.remote,
                upstream.shown()
            ),
        )),
        Err(error) => Err(SyncOutcome::ended(
            SyncState::Stopped,
            local_failure(&error, sync),
        )),
    }
}

/// A local git child's failure in words: its deadline named when that is
/// what stopped it.
fn local_failure(error: &GitError, sync: &SyncGit) -> String {
    match error {
        GitError::Deadline => format!(
            "a git command passed the sync's deadline of {} s and was stopped",
            sync.deadline_seconds()
        ),
        error => error.to_string(),
    }
}

/// What the merge phase left.
enum Merged {
    /// `HEAD` is `R`: nothing to push.
    Same,
    /// `HEAD` holds `R`: push it.
    Done,
    /// The sync ends here.
    Ended(SyncOutcome),
}

/// The merge phase, inside the turns: the worktree judged again, `L` read
/// again, then nothing, a fast-forward or a merge commit; else `held` or
/// `stopped`, nothing touched. A fast-forward updates the served roots'
/// indexes ([`reindex`]); one that fails doubts them all.
fn merge(env: &Env, sync: &SyncGit, repository: &SyncRepository, upstream: &Upstream) -> Merged {
    let top = repository.top.display();
    let stopped = |why: String| Merged::Ended(SyncOutcome::ended(SyncState::Stopped, why));
    match sync.still_ready(upstream) {
        Ok(None) => {}
        Ok(Some(why)) => return stopped(why),
        Err(error) => return stopped(local_failure(&error, sync)),
    }
    let (left, right) = match tips(sync, upstream) {
        Ok(tips) => tips,
        Err(outcome) => return Merged::Ended(outcome),
    };
    let relation = match sync.relation(&left, &right) {
        Ok(relation) => relation,
        Err(error) => return stopped(local_failure(&error, sync)),
    };
    let target = match relation {
        Relation::Contained if left == right => return Merged::Same,
        Relation::Contained => return Merged::Done,
        Relation::FastForward => right.clone(),
        Relation::Unrelated => {
            return Merged::Ended(SyncOutcome::held(
                format!(
                    "`{}` shares no history with `{}`: nothing merged",
                    upstream.shown(),
                    upstream.branch
                ),
                (left, right),
            ));
        }
        Relation::Diverged => match merge_commit(sync, repository, upstream, &left, &right) {
            Ok(commit) => commit,
            Err(merged) => return merged,
        },
    };
    match sync.fast_forward(&target) {
        Ok(()) => {
            reindex(env, sync, repository, &left, &target);
            Merged::Done
        }
        Err(error) => {
            // Its files may be half moved.
            for root in &repository.roots {
                freshness::doubt(root);
            }
            stopped(format!(
                "the merge of `{}` could not move `{}` in {top}: {}",
                upstream.shown(),
                upstream.branch,
                local_failure(&error, sync)
            ))
        }
    }
}

/// After the fast-forward from `left` to `target`, inside the turns: each
/// served root's index updated over the paths the diff names under the
/// root's path from the top (that prefix stripped), through a
/// `WorkingTree` of the merged `[paths]` with the merged scheme, the
/// project discovered again. A failed diff, or one that left out a name it
/// could not read as UTF-8, doubts every root; a root whose path from the
/// top cannot be told, or whose discovery, open or update fails, is
/// doubted on its own.
fn reindex(env: &Env, sync: &SyncGit, repository: &SyncRepository, left: &str, target: &str) {
    let changed = match sync.changed(left, target) {
        Ok(changed) if changed.skipped_names == 0 => changed.paths,
        Ok(changed) => {
            for root in &repository.roots {
                freshness::doubt(root);
            }
            changed.paths
        }
        Err(_) => {
            for root in &repository.roots {
                freshness::doubt(root);
            }
            return;
        }
    };
    for root in &repository.roots {
        let Some(prefix) = top_prefix(&repository.top, root) else {
            freshness::doubt(root);
            continue;
        };
        let paths: Vec<&str> = changed
            .iter()
            .filter_map(|path| path.strip_prefix(prefix.as_str()))
            .collect();
        if paths.is_empty() {
            continue;
        }
        let globals = Globals {
            root: Some(root.clone()),
            config: None,
        };
        let updated = discover(env, &globals).ok().and_then(|project| {
            let mut open = open_index(env, &project).ok()?;
            let tree = WorkingTree::new(&project.root, &project.config.paths).ok()?;
            open.index
                .update_paths(&tree, &project.config.scheme, &paths)
                .ok()
        });
        if updated.is_none() {
            freshness::doubt(root);
        }
    }
}

/// The path of `root` from `top`: `""`, or its components each followed by
/// `/`, as [`MergedRoot::prefix`]; `None` when it does not lie below `top`.
fn top_prefix(top: &Path, root: &Path) -> Option<String> {
    let relative = root.strip_prefix(top).ok()?;
    let mut prefix = String::new();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return None;
        };
        prefix.push_str(name.to_str()?);
        prefix.push('/');
    }
    Some(prefix)
}

/// A served root in the merged tree: its path from the top (`""` or
/// `<dir>/`) and, by the merged config, its index outputs from the top.
struct MergedRoot {
    root: PathBuf,
    prefix: String,
    outputs: Vec<String>,
}

/// The merge commit of `left` and `right` (see the module documentation),
/// its OID; else what the merge phase ends in.
fn merge_commit(
    sync: &SyncGit,
    repository: &SyncRepository,
    upstream: &Upstream,
    left: &str,
    right: &str,
) -> Result<String, Merged> {
    let top = repository.top.display();
    let stopped = |why: String| Merged::Ended(SyncOutcome::ended(SyncState::Stopped, why));
    let tips = || (left.to_owned(), right.to_owned());
    let merged = sync
        .merge_tree(left, right)
        .map_err(|error| stopped(local_failure(&error, sync)))?;
    let roots: Vec<MergedRoot> = repository
        .roots
        .iter()
        .filter_map(|root| merged_root(sync, &repository.top, root, &merged.tree))
        .collect();
    let conflicts = &merged.conflicts;
    let config_conflicts = roots
        .iter()
        .any(|root| conflicts.contains(&format!("{}{CONFIG_FILE}", root.prefix)));
    let plain = conflicts
        .iter()
        .any(|path| !roots.iter().any(|root| root.outputs.contains(path)));
    if !conflicts.is_empty() && (config_conflicts || plain) {
        let paths: Vec<&str> = conflicts.iter().map(String::as_str).collect();
        return Err(Merged::Ended(SyncOutcome::held(
            format!(
                "merging `{}` conflicts in {}: resolve it on the host's terminal in {top}",
                upstream.shown(),
                named(&paths)
            ),
            tips(),
        )));
    }
    // A conflicted output at `L`'s bytes, then each root's render.
    let mut at_left = BTreeMap::new();
    for path in conflicts {
        let entry = sync
            .entry(left, path)
            .map_err(|error| stopped(local_failure(&error, sync)))?;
        let change = match entry {
            Some(entry) if entry.kind == "blob" => Replacement::Blob {
                mode: entry.mode,
                oid: entry.oid,
            },
            _ => Replacement::Remove,
        };
        at_left.insert(path.clone(), change);
    }
    let tree = sync
        .replaced_tree(&merged.tree, &at_left)
        .map_err(|error| stopped(local_failure(&error, sync)))?;
    let mut rendered = BTreeMap::new();
    for root in &roots {
        let outputs = match render_tree(&root.root, &sync.env_in(&root.root), &tree) {
            TreeRender::NoIndex => continue,
            TreeRender::Rendered(outputs) => outputs,
            TreeRender::Gap(gap) => {
                let conflicted = root.outputs.iter().any(|path| conflicts.contains(path));
                if conflicted {
                    return Err(Merged::Ended(SyncOutcome::held(
                        format!("the index's outputs conflict and cannot be rendered: {gap}"),
                        tips(),
                    )));
                }
                // A clean merge keeps git's outputs.
                continue;
            }
        };
        for (path, bytes) in outputs {
            let path = format!("{}{path}", root.prefix);
            let current = sync
                .blob(&tree, &path)
                .map_err(|error| stopped(local_failure(&error, sync)))?;
            if current.as_deref() == Some(bytes.as_slice()) {
                continue;
            }
            let entry = sync
                .entry(&tree, &path)
                .map_err(|error| stopped(local_failure(&error, sync)))?;
            let oid = sync
                .write_blob(&bytes)
                .map_err(|error| stopped(local_failure(&error, sync)))?;
            rendered.insert(
                path,
                Replacement::Blob {
                    mode: file_mode(entry.as_ref()),
                    oid,
                },
            );
        }
    }
    let tree = sync
        .replaced_tree(&tree, &rendered)
        .map_err(|error| stopped(local_failure(&error, sync)))?;
    let message = format!("Merge remote-tracking branch '{}'\n", upstream.shown());
    sync.commit_merge(&tree, left, right, &message)
        .map_err(|failure| match failure {
            CommitFailure::Identity(_) => stopped(format!(
                "a merge commit needs an identity: set user.name and user.email in {top}"
            )),
            CommitFailure::Git(error) => stopped(format!(
                "the merge commit of `{}` could not be made in {top}: {}",
                upstream.shown(),
                local_failure(&error, sync)
            )),
        })
}

/// The served root `root` in the merged tree `tree` of the checkout at
/// `top`: its prefix and its index outputs by the merged config (none when
/// that config is absent, unreadable or invalid: the sync never judges it).
/// `None` when the root does not lie below `top`.
fn merged_root(sync: &SyncGit, top: &Path, root: &Path, tree: &str) -> Option<MergedRoot> {
    let prefix = top_prefix(top, root)?;
    let config = format!("{prefix}{CONFIG_FILE}");
    let outputs = match sync.blob(tree, &config) {
        Ok(Some(bytes)) => {
            let named = NamedBytes {
                name: config,
                bytes: Ok(bytes),
            };
            match load_config(&named) {
                Ok((project, check)) => index_outputs(&project, &check)
                    .into_iter()
                    .map(|path| format!("{prefix}{path}"))
                    .collect(),
                Err(_) => Vec::new(),
            }
        }
        Ok(None) | Err(_) => Vec::new(),
    };
    Some(MergedRoot {
        root: root.to_path_buf(),
        prefix,
        outputs,
    })
}
