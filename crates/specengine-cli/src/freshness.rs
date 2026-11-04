//! The reads' freshness step and a hub's trust in its index (the
//! `read-trust` spec, "CLI API").
//!
//! Every read's first step brings the index up to date with the files: the
//! store's stat-keyed `update` ([`crate::refresh`]). On a hub alone, whose
//! tree only its own confirms and merges write, a read may trust the index
//! instead: inside a [`trusted`] scope whose [`IndexTrust`] names the read's
//! root, the step reads the last walk's notes and both fingerprints from the
//! index ([`SqliteIndex::trusted_notes`]), prints that walk's warnings, and
//! updates nothing. It walks when the trust has not walked since start, is
//! dirty, or began its last walk [`RECONCILE_EVERY`] or more ago (the
//! reconcile, lazy: no timer, no watcher), and when the notes do not read.
//!
//! Doubt (T4) is a dirty mark: any evidence that the index may differ from
//! the files sets it, and the next read walks. A walk clears it as it begins
//! and sets it again if it fails, so a doubt raised meanwhile survives. A
//! doubt outside a scope does nothing, and no status, check or gate reads a
//! trust: it never refuses a read.
//!
//! Only reads trust. A hub's call that writes anything or authorizes a
//! write (a confirm, a card or queue write) runs in a [`walking`] scope:
//! its freshness steps walk, the files judged as they are now, while its
//! doubts and walks still reach the trust. A read inside such a scope may
//! opt in with [`trusting`].
//!
//! The scope is a thread-local of the calling thread, restored on return or
//! unwind: a call runs whole on one thread and hands the index to no other.
//! The CLI, stdio MCP, a local daemon and eval's walking reads enter none.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use specengine_model::IdScheme;
use specengine_store::{IndexWriter as _, Source, SqliteIndex, WorkingTree};

use crate::project::ProjectRoot;
use crate::refresh::{refresh_in, walk_warnings};
use crate::{CliError, Message};

/// A trusted read walks once its trust's last walk began this long ago.
pub const RECONCILE_EVERY: Duration = Duration::from_secs(300);

/// A served project's trust in its index; a hub's daemon keeps one per
/// project for the process's life, and enters it with [`trusted`] around
/// each call in the project's turn.
#[derive(Debug)]
pub struct IndexTrust {
    inner: Arc<Inner>,
}

/// What a scope holds of a trust.
#[derive(Debug)]
struct Inner {
    /// Canonical.
    root: PathBuf,
    every: Duration,
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    /// A walk inside a scope succeeded since start.
    walked: bool,
    /// The dirty mark (T4).
    dirty: bool,
    /// When the last successful walk began.
    since: Option<Instant>,
    /// Walks begun inside a scope.
    walks: u64,
}

impl IndexTrust {
    /// Not walked yet; its reconcile every [`RECONCILE_EVERY`]. `root` is
    /// canonicalised (as given when it cannot be).
    pub fn new(root: PathBuf) -> Self {
        Self::with_every(root, RECONCILE_EVERY)
    }

    /// [`Self::new`] with another reconcile interval (the tests').
    #[doc(hidden)]
    pub fn with_every(root: PathBuf, every: Duration) -> Self {
        Self {
            inner: Arc::new(Inner {
                root: canonical(&root),
                every,
                state: Mutex::new(State::default()),
            }),
        }
    }

    /// The dirty mark (T4): the next read inside a scope walks.
    pub fn doubt(&self) {
        self.inner.doubt();
    }

    /// Walks begun inside a scope.
    #[doc(hidden)]
    pub fn walks(&self) -> u64 {
        self.inner.state().walks
    }

    /// The dirty mark is set.
    #[doc(hidden)]
    pub fn is_dirty(&self) -> bool {
        self.inner.state().dirty
    }
}

impl Inner {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn doubt(&self) {
        self.state().dirty = true;
    }

    /// The next read must walk: never walked, dirty, or the last walk began
    /// `every` or more ago.
    fn due(&self) -> bool {
        let state = self.state();
        !state.walked
            || state.dirty
            || state
                .since
                .is_none_or(|since| since.elapsed() >= self.every)
    }
}

/// A walk's books inside a scope (R9): begun, the mark cleared and the walk
/// counted; [`Walk::end`] records how it ended. Dropped without an end (a
/// panic in the update), it sets the mark again.
pub(crate) struct Walk {
    trust: Arc<Inner>,
    began: Instant,
    ended: bool,
}

impl Walk {
    /// The walk of `root`'s index begins, when a scope's trust matches it.
    pub(crate) fn begin(root: &Path) -> Option<Self> {
        let trust = matching(root)?;
        let began = Instant::now();
        {
            let mut state = trust.state();
            state.dirty = false;
            state.walks += 1;
        }
        Some(Self {
            trust,
            began,
            ended: false,
        })
    }

    /// Success: walked, its start the reconcile's clock; failure: dirty.
    pub(crate) fn end(mut self, ok: bool) {
        self.ended = true;
        let mut state = self.trust.state();
        if ok {
            state.walked = true;
            state.since = Some(self.began);
        } else {
            state.dirty = true;
        }
    }
}

impl Drop for Walk {
    fn drop(&mut self) {
        if !self.ended {
            self.trust.doubt();
        }
    }
}

thread_local! {
    /// The trusts in scope on this thread, and whether its reads trust.
    static SCOPE: RefCell<Scope> = const {
        RefCell::new(Scope {
            trusts: Vec::new(),
            reads_trust: false,
        })
    };
}

/// A scope: its trusts, which doubts and walks reach, and whether a read's
/// freshness step may trust them ([`trusted`]) or always walks
/// ([`walking`]).
#[derive(Clone)]
struct Scope {
    trusts: Vec<Arc<Inner>>,
    reads_trust: bool,
}

/// Restores the scope it replaced when dropped, on return or unwind.
struct Restore(Option<Scope>);

impl Drop for Restore {
    fn drop(&mut self) {
        if let Some(previous) = self.0.take() {
            let _ = SCOPE.try_with(|scope| *scope.borrow_mut() = previous);
        }
    }
}

/// `run` with `scope` in force on this thread, the previous one restored on
/// return or unwind.
fn within<T>(scope: Scope, run: impl FnOnce() -> T) -> T {
    let _restore = Restore(Some(
        SCOPE.with_borrow_mut(|current| std::mem::replace(current, scope)),
    ));
    run()
}

fn inners(trusts: &[&IndexTrust]) -> Vec<Arc<Inner>> {
    trusts
        .iter()
        .map(|trust| Arc::clone(&trust.inner))
        .collect()
}

/// `run` with `trusts` in scope on this thread, restored on return or
/// unwind: a read whose trust is due trusts the index (R8). A nested scope
/// replaces the outer one for its extent.
pub fn trusted<T>(trusts: &[&IndexTrust], run: impl FnOnce() -> T) -> T {
    within(
        Scope {
            trusts: inners(trusts),
            reads_trust: true,
        },
        run,
    )
}

/// [`trusted`], but every freshness step walks: a hub's call that writes or
/// authorizes a write, which must judge the files as they are now. Its
/// doubts and walks still reach `trusts`.
pub fn walking<T>(trusts: &[&IndexTrust], run: impl FnOnce() -> T) -> T {
    within(
        Scope {
            trusts: inners(trusts),
            reads_trust: false,
        },
        run,
    )
}

/// `run` inside the current scope's trusts with its reads trusting them: a
/// read called inside a [`walking`] scope (a hub's MCP read tool). Outside
/// a scope, `run` alone.
pub fn trusting<T>(run: impl FnOnce() -> T) -> T {
    let trusts = SCOPE.with_borrow(|scope| scope.trusts.clone());
    within(
        Scope {
            trusts,
            reads_trust: true,
        },
        run,
    )
}

/// The scope's trust whose root is `root` (canonical).
fn matching(root: &Path) -> Option<Arc<Inner>> {
    SCOPE.with_borrow(|scope| {
        scope
            .trusts
            .iter()
            .find(|trust| trust.root == root)
            .cloned()
    })
}

/// The scope's trust of `root` when the scope's reads trust it.
fn trusted_for(root: &Path) -> Option<Arc<Inner>> {
    if SCOPE.with_borrow(|scope| scope.reads_trust) {
        matching(root)
    } else {
        None
    }
}

/// The reads' freshness step (R8): the walk ([`refresh_in`]) with no scope,
/// a [`walking`] one, no trust of `project`'s root, or a trust due to walk;
/// else the last
/// walk's warnings from the index, nothing updated, or the walk when the
/// index cannot say them (`None` or an error).
pub(crate) fn step(
    index: &mut SqliteIndex,
    project: &ProjectRoot,
    tree: &WorkingTree,
) -> Result<Vec<Message>, CliError> {
    if let Some(trust) = trusted_for(&project.root)
        && !trust.due()
        && let Ok(Some(notes)) = index.trusted_notes(&project.config.scheme, &project.config.paths)
    {
        return Ok(walk_warnings(project, &notes));
    }
    refresh_in(index, project, tree, false).map(|(_, messages)| messages)
}

/// Marks the scope's trust of `root` dirty (T4); outside a scope, or with no
/// trust of `root`, nothing. `root` is canonicalised (as given when it
/// cannot be).
pub(crate) fn doubt(root: &Path) {
    if SCOPE.with_borrow(|scope| scope.trusts.is_empty()) {
        return;
    }
    if let Some(trust) = matching(&canonical(root)) {
        trust.doubt();
    }
}

/// Marks every trust in this thread's scope dirty (T4); outside a scope,
/// nothing. For a caller that knows the call it ran in the scope could not
/// run (exit 2) but not which root it read: a hub's MCP call.
pub fn doubt_in_scope() {
    let trusts = SCOPE.with_borrow(|scope| scope.trusts.clone());
    for trust in trusts {
        trust.doubt();
    }
}

fn canonical(root: &Path) -> PathBuf {
    fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
}

/// What a post-commit update follows, as its warning names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum After {
    Commit,
    Completion,
}

/// The index brought up to date for `paths` after a commit or a completion
/// (R10): `update_paths` through `source`; a failure is today's warning, and
/// the index's root is doubted.
pub(crate) fn post_commit_update(
    index: &mut SqliteIndex,
    source: &dyn Source,
    scheme: &IdScheme,
    paths: &[&str],
    after: After,
) -> Option<Message> {
    match index.update_paths(source, scheme, paths) {
        Ok(_) => None,
        Err(error) => Some(post_commit_failed(index.root(), after, &error.to_string())),
    }
}

/// The warning of a post-commit update that failed, or could not open the
/// index, with `root` doubted.
pub(crate) fn post_commit_failed(root: &Path, after: After, error: &str) -> Message {
    doubt(root);
    let after = match after {
        After::Commit => "commit",
        After::Completion => "completion",
    };
    Message::Warning(format!(
        "the index was not updated after the {after}: {error}; the next command updates it"
    ))
}
