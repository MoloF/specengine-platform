//! A write's check, remembered (`write-cost`, RM-0125, "C and D"): the
//! as-is findings of one tree state (C) and a write's introduced findings
//! (D), in memory, per process, never stored.
//!
//! - **Keys** (computed, never stored): C by the root and the walk-state
//!   digest (the index stamp, the worktree's fingerprints, every file row,
//!   the walk notes, the config's and the baseline's hashes, today); D by
//!   that digest, the root, the written path, the BLAKE3 of the new bytes
//!   and the login (`-` off a hub). So no finding crosses logins, and C
//!   holds the corpus's own findings, no login's data.
//! - **Home**: a daemon keeps one memo per served project, a stdio MCP
//!   server one for its roots (the root is in every key); the terminal
//!   none. A call reaches it through [`scoped`], entered inside the
//!   project's turn: the scope is a thread-local of the calling thread,
//!   restored on return or unwind. Outside a scope nothing is remembered.
//! - **The lock** is a leaf: held for one lookup or one insert, never while
//!   computing or waiting on a turn, SQLite or git.
//! - **Budget**: [`BUDGET`] bytes for C and D together; D is evicted first,
//!   least recently used, then C; an entry larger than the budget is not
//!   kept. A miss is slower, never wrong.
//! - **Doubt** (T4) empties D; C is used again only under the digest the
//!   call's own walk gives.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use specengine_core::check::AsIs;

use crate::queue::ProposalFinding;

/// C and D's bytes per memo: 32 MiB.
pub const BUDGET: usize = 32 * 1024 * 1024;

/// A BLAKE3 digest.
pub(crate) type Digest = [u8; 32];

/// D's key: what the findings of one write depend on.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FindingsKey {
    pub root: PathBuf,
    pub tree: Digest,
    pub path: String,
    pub bytes: Digest,
    pub login: String,
}

/// One served project's (or one process's) memo.
#[derive(Debug)]
pub struct CheckMemo {
    /// Unique in the process: the thread-local flags name the memo by it.
    id: u64,
    budget: usize,
    state: Mutex<State>,
    /// A daemon's memo: reached only on a thread running inside its
    /// project's turn ([`TurnHeld::within`]); a stdio server's has no turn.
    turned: bool,
    hits: AtomicU64,
    misses: AtomicU64,
}

/// Memos of the process, numbered.
static MEMOS: AtomicU64 = AtomicU64::new(0);

/// The project's turn held over a memo ([`CheckMemo::hold_turn`]): a call
/// runs [`TurnHeld::within`] it on its own thread.
#[derive(Debug)]
pub struct TurnHeld {
    memo: u64,
}

impl TurnHeld {
    /// `run` on this thread inside the turn: the memo's in-turn flag set
    /// for this thread alone, restored on return or unwind.
    pub fn within<T>(&self, run: impl FnOnce() -> T) -> T {
        IN_TURN.with_borrow_mut(|held| held.push(self.memo));
        let _restore = LeaveTurn(self.memo);
        run()
    }
}

/// Takes the thread's in-turn mark of one memo back.
struct LeaveTurn(u64);

impl Drop for LeaveTurn {
    fn drop(&mut self) {
        let _ = IN_TURN.try_with(|held| {
            let mut held = held.borrow_mut();
            if let Some(at) = held.iter().rposition(|memo| *memo == self.0) {
                held.remove(at);
            }
        });
    }
}

#[derive(Debug, Default)]
struct State {
    /// C, by root: one tree state each.
    trees: BTreeMap<PathBuf, Tree>,
    /// D.
    findings: BTreeMap<FindingsKey, Remembered>,
    bytes: usize,
    /// Each D use's tick: least recently used first out.
    clock: u64,
    /// The config hash each root's last whole check ran under: a call
    /// under another is the cold path.
    warm: BTreeMap<PathBuf, Digest>,
    /// D's lookups by the login asking: (hits, misses).
    by_login: BTreeMap<String, (u64, u64)>,
}

#[derive(Debug)]
struct Tree {
    digest: Digest,
    as_is: AsIs,
    bytes: usize,
}

#[derive(Debug)]
struct Remembered {
    findings: Vec<ProposalFinding>,
    bytes: usize,
    used: u64,
}

impl Default for CheckMemo {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckMemo {
    /// An empty memo of [`BUDGET`] bytes.
    pub fn new() -> Self {
        Self::with_budget(BUDGET)
    }

    /// An empty memo of `budget` bytes (0: nothing is kept), with no turn
    /// (a stdio server's).
    pub fn with_budget(budget: usize) -> Self {
        Self {
            id: MEMOS.fetch_add(1, Ordering::Relaxed),
            budget,
            state: Mutex::new(State::default()),
            turned: false,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// A daemon's memo of `budget` bytes: reached only on a thread running
    /// inside its project's turn ([`TurnHeld::within`]), asserted in debug
    /// builds.
    pub fn turned(budget: usize) -> Self {
        Self {
            turned: true,
            ..Self::with_budget(budget)
        }
    }

    /// The project's turn, just taken: the holder runs its call
    /// [`TurnHeld::within`] it.
    pub fn hold_turn(&self) -> TurnHeld {
        TurnHeld { memo: self.id }
    }

    /// The in-turn flag of this thread: it runs inside this memo's turn,
    /// or the memo has no turn.
    pub fn in_turn(&self) -> bool {
        !self.turned
            || IN_TURN
                .try_with(|held| held.borrow().contains(&self.id))
                .unwrap_or(false)
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Entries held: C's trees and D's findings.
    pub fn entries(&self) -> usize {
        let state = self.state();
        state.trees.len() + state.findings.len()
    }

    /// Bytes held, as the budget counts them.
    pub fn bytes(&self) -> usize {
        self.state().bytes
    }

    /// D's lookups that found their findings.
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// D's lookups that did not.
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// D's lookups by the login that asked: `(hits, misses)` each (`-` off
    /// a hub). A hit is always under the login that stored the entry, the
    /// login being part of D's key.
    pub fn lookups_by_login(&self) -> BTreeMap<String, (u64, u64)> {
        self.state().by_login.clone()
    }

    /// Everything forgotten.
    pub fn clear(&self) {
        let mut state = self.state();
        *state = State::default();
    }

    /// A doubt (T4): D emptied, C kept for the digest of the next walk.
    pub fn doubt(&self) {
        let mut state = self.state();
        let freed: usize = state.findings.values().map(|entry| entry.bytes).sum();
        state.findings.clear();
        state.bytes = state.bytes.saturating_sub(freed);
    }

    /// This thread's scope is over this memo.
    pub fn in_scope(&self) -> bool {
        SCOPE
            .try_with(|scope| {
                scope
                    .borrow()
                    .as_ref()
                    .is_some_and(|scope| scope.memo.id == self.id)
            })
            .unwrap_or(false)
    }

    /// Every access but the counters: only inside a scope, and a daemon's
    /// only inside its project's turn.
    fn reached(&self) {
        debug_assert!(
            self.in_scope(),
            "the check memo is reached outside its scope"
        );
        debug_assert!(
            self.in_turn(),
            "the check memo is reached outside its project's turn"
        );
    }

    /// D's findings for `key`, its use counted.
    pub(crate) fn findings(&self, key: &FindingsKey) -> Option<Vec<ProposalFinding>> {
        self.reached();
        let mut state = self.state();
        state.clock += 1;
        let clock = state.clock;
        let found = state.findings.get_mut(key).map(|entry| {
            entry.used = clock;
            entry.findings.clone()
        });
        let counted = state.by_login.entry(key.login.clone()).or_default();
        if found.is_some() {
            counted.0 += 1;
        } else {
            counted.1 += 1;
        }
        drop(state);
        let counter = if found.is_some() {
            &self.hits
        } else {
            &self.misses
        };
        counter.fetch_add(1, Ordering::Relaxed);
        found
    }

    /// D's findings kept under `key`, when they fit.
    pub(crate) fn keep_findings(&self, key: FindingsKey, findings: Vec<ProposalFinding>) {
        self.reached();
        let bytes = findings_bytes(&key, &findings);
        if bytes > self.budget {
            return;
        }
        let mut state = self.state();
        state.clock += 1;
        let used = state.clock;
        if let Some(old) = state.findings.insert(
            key,
            Remembered {
                findings,
                bytes,
                used,
            },
        ) {
            state.bytes = state.bytes.saturating_sub(old.bytes);
        }
        state.bytes += bytes;
        self.fit(&mut state, None);
    }

    /// C of `root` when held under `digest`, taken out (the call puts it
    /// back, grown); a C under another digest is dropped.
    pub(crate) fn take_tree(&self, root: &Path, digest: &Digest) -> Option<AsIs> {
        self.reached();
        let mut state = self.state();
        let tree = state.trees.remove(root)?;
        state.bytes = state.bytes.saturating_sub(tree.bytes);
        (tree.digest == *digest).then_some(tree.as_is)
    }

    /// C of `root` under `digest`, when it fits.
    pub(crate) fn keep_tree(&self, root: &Path, digest: Digest, as_is: AsIs) {
        self.reached();
        let bytes = root.as_os_str().len() + as_is.bytes();
        if bytes > self.budget {
            return;
        }
        let mut state = self.state();
        if let Some(old) = state.trees.insert(
            root.to_path_buf(),
            Tree {
                digest,
                as_is,
                bytes,
            },
        ) {
            state.bytes = state.bytes.saturating_sub(old.bytes);
        }
        state.bytes += bytes;
        self.fit(&mut state, Some(root));
    }

    /// The last whole check of `root` ran under `config`: a call under it
    /// takes the rows.
    pub(crate) fn is_warm(&self, root: &Path, config: &Digest) -> bool {
        self.reached();
        self.state().warm.get(root) == Some(config)
    }

    /// `root`'s whole check ran under `config`.
    pub(crate) fn warmed(&self, root: &Path, config: Digest) {
        self.reached();
        self.state().warm.insert(root.to_path_buf(), config);
    }

    /// Evicts until within the budget: D least recently used first, then
    /// C, another root's before `keep`'s.
    fn fit(&self, state: &mut State, keep: Option<&Path>) {
        while state.bytes > self.budget {
            let oldest = state
                .findings
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(key, _)| key.clone());
            if let Some(key) = oldest {
                if let Some(entry) = state.findings.remove(&key) {
                    state.bytes = state.bytes.saturating_sub(entry.bytes);
                }
                continue;
            }
            let other = state
                .trees
                .keys()
                .find(|root| Some(root.as_path()) != keep)
                .cloned()
                .or_else(|| state.trees.keys().next().cloned());
            let Some(root) = other else {
                state.bytes = 0;
                break;
            };
            if let Some(tree) = state.trees.remove(&root) {
                state.bytes = state.bytes.saturating_sub(tree.bytes);
            }
        }
    }
}

/// D's bytes: the key's strings, every finding's strings and fixed size.
fn findings_bytes(key: &FindingsKey, findings: &[ProposalFinding]) -> usize {
    let key_bytes = key.root.as_os_str().len() + key.path.len() + key.login.len() + 64;
    key_bytes
        + findings
            .iter()
            .map(|finding| {
                std::mem::size_of::<ProposalFinding>()
                    + finding.code.len()
                    + finding.path.len()
                    + finding.subject.len()
                    + finding.message.len()
            })
            .sum::<usize>()
}

/// The memo in force on a thread and the login its writes are keyed by.
#[derive(Clone)]
struct Scope {
    memo: Arc<CheckMemo>,
    login: String,
}

thread_local! {
    /// The memo in force on this thread.
    static SCOPE: RefCell<Option<Scope>> = const { RefCell::new(None) };
    /// The memos whose project's turn this thread runs inside.
    static IN_TURN: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
}

/// Puts back the scope it replaced, on return or unwind.
struct Restore(Option<Scope>);

impl Drop for Restore {
    fn drop(&mut self) {
        let previous = self.0.take();
        let _ = SCOPE.try_with(|scope| *scope.borrow_mut() = previous);
    }
}

/// `run` with `memo` in force on this thread, its writes keyed by `login`
/// (`-` for none), the previous scope restored on return or unwind. A
/// daemon enters it inside the project's turn, on the thread that runs the
/// call ([`TurnHeld::within`]).
pub fn scoped<T>(memo: &Arc<CheckMemo>, login: &str, run: impl FnOnce() -> T) -> T {
    debug_assert!(
        memo.in_turn(),
        "the check memo's scope is entered outside its project's turn"
    );
    let previous = SCOPE.with_borrow_mut(|scope| {
        scope.replace(Scope {
            memo: Arc::clone(memo),
            login: login.to_owned(),
        })
    });
    let _restore = Restore(previous);
    run()
}

/// `run` with the scope's writes keyed by `login`: a call that knows its
/// login (a confirm's `[team]` entry). Outside a scope, `run` alone.
pub fn as_login<T>(login: &str, run: impl FnOnce() -> T) -> T {
    match current() {
        Some((memo, _)) => scoped(&memo, login, run),
        None => run(),
    }
}

/// A doubt (T4) reaching the memo in force: D emptied. Outside a scope,
/// nothing.
pub fn doubt() {
    if let Some((memo, _)) = current() {
        memo.doubt();
    }
}

/// The memo in force and its login.
pub(crate) fn current() -> Option<(Arc<CheckMemo>, String)> {
    SCOPE
        .try_with(|scope| {
            scope
                .borrow()
                .as_ref()
                .map(|scope| (Arc::clone(&scope.memo), scope.login.clone()))
        })
        .ok()
        .flatten()
}

#[cfg(test)]
mod tests {
    //! The budget by bytes, D's eviction least recently used, the oversized
    //! entry left out, doubt emptying D alone, the login in D's key.

    use std::path::PathBuf;
    use std::sync::Arc;

    use specengine_core::check::AsIs;
    use specengine_model::Severity;

    use super::{CheckMemo, FindingsKey, scoped};
    use crate::queue::ProposalFinding;

    fn key(path: &str, login: &str) -> FindingsKey {
        FindingsKey {
            root: PathBuf::from("root"),
            tree: [1; 32],
            path: path.to_owned(),
            bytes: [2; 32],
            login: login.to_owned(),
        }
    }

    fn finding(message: &str) -> ProposalFinding {
        ProposalFinding {
            code: "ref-dangling".to_owned(),
            severity: Severity::Error,
            path: "a.md".to_owned(),
            line: 1,
            subject: "X".to_owned(),
            message: message.to_owned(),
        }
    }

    #[test]
    fn the_budget_is_bytes_and_d_goes_least_recently_used() {
        let one = vec![finding(&"m".repeat(100))];
        let size = super::findings_bytes(&key("a.md", "-"), &one);
        let memo = Arc::new(CheckMemo::with_budget(size * 2));
        scoped(&memo, "-", || {
            memo.keep_findings(key("a.md", "-"), one.clone());
            memo.keep_findings(key("b.md", "-"), one.clone());
            assert!(memo.findings(&key("a.md", "-")).is_some(), "a used again");
            memo.keep_findings(key("c.md", "-"), one.clone());
            assert!(
                memo.findings(&key("b.md", "-")).is_none(),
                "b, least recent"
            );
            assert!(memo.findings(&key("a.md", "-")).is_some());
            assert!(memo.bytes() <= size * 2);
            let big = vec![finding(&"m".repeat(size * 3))];
            memo.keep_findings(key("d.md", "-"), big);
            assert!(
                memo.findings(&key("d.md", "-")).is_none(),
                "over the budget"
            );
        });
    }

    #[test]
    fn a_login_misses_another_logins_findings() {
        let memo = Arc::new(CheckMemo::new());
        scoped(&memo, "-", || {
            memo.keep_findings(key("a.md", "ann"), vec![finding("x")]);
            assert!(memo.findings(&key("a.md", "bob")).is_none());
            assert!(memo.findings(&key("a.md", "ann")).is_some());
        });
    }

    #[test]
    fn a_doubt_empties_d_and_keeps_c() {
        let memo = Arc::new(CheckMemo::new());
        scoped(&memo, "-", || {
            memo.keep_findings(key("a.md", "-"), vec![finding("x")]);
            memo.keep_tree(&PathBuf::from("root"), [3; 32], AsIs::default());
            assert_eq!(memo.entries(), 2);
            memo.doubt();
            assert_eq!(memo.entries(), 1);
            assert!(memo.take_tree(&PathBuf::from("root"), &[3; 32]).is_some());
        });
    }

    /// The in-turn and in-scope flags are the calling thread's: another
    /// thread inside the turn leaves this one outside it.
    #[test]
    fn the_turn_and_the_scope_are_the_threads_own() {
        let memo = Arc::new(CheckMemo::turned(super::BUDGET));
        let held = memo.hold_turn();
        assert!(!memo.in_turn(), "taken, not entered on this thread");
        let (entered, release) = (std::sync::Barrier::new(2), std::sync::Barrier::new(2));
        std::thread::scope(|threads| {
            threads.spawn(|| {
                held.within(|| {
                    assert!(memo.in_turn());
                    scoped(&memo, "-", || assert!(memo.in_scope()));
                    entered.wait();
                    release.wait();
                });
            });
            entered.wait();
            assert!(!memo.in_turn(), "another thread's turn");
            assert!(!memo.in_scope(), "another thread's scope");
            release.wait();
        });
        held.within(|| {
            scoped(&memo, "ann", || {
                memo.keep_findings(key("a.md", "ann"), vec![finding("x")]);
                assert!(memo.findings(&key("a.md", "ann")).is_some());
            });
        });
        assert!(!memo.in_turn());
        assert_eq!(memo.lookups_by_login().get("ann"), Some(&(1, 0)));
    }

    /// Outside its turn, a daemon's memo refuses its scope (debug builds).
    #[cfg(debug_assertions)]
    #[test]
    fn a_scope_outside_the_turn_is_caught() {
        let memo = Arc::new(CheckMemo::turned(super::BUDGET));
        let caught = std::panic::catch_unwind(|| scoped(&memo, "-", || ()));
        assert!(caught.is_err());
    }

    /// WC6, the stdio half: one process memo keeps each root's C apart
    /// (the root is in every key) and holds them all within its one budget.
    #[test]
    fn one_memo_keeps_each_roots_tree_apart_within_its_budget() {
        let memo = Arc::new(CheckMemo::with_budget(2048));
        scoped(&memo, "-", || {
            memo.keep_tree(&PathBuf::from("one"), [1; 32], AsIs::default());
            memo.keep_tree(&PathBuf::from("two"), [1; 32], AsIs::default());
            assert_eq!(memo.entries(), 2, "a tree per root");
            assert!(memo.take_tree(&PathBuf::from("two"), &[1; 32]).is_some());
            assert!(memo.take_tree(&PathBuf::from("one"), &[1; 32]).is_some());
            for n in 0..64 {
                let mut key = key(&format!("{n}.md"), "-");
                key.root = PathBuf::from(if n % 2 == 0 { "one" } else { "two" });
                memo.keep_findings(key, vec![finding(&"m".repeat(64))]);
                assert!(memo.bytes() <= 2048, "bounded across roots");
            }
        });
    }

    #[test]
    fn a_budget_of_zero_keeps_nothing() {
        let memo = Arc::new(CheckMemo::with_budget(0));
        scoped(&memo, "-", || {
            memo.keep_findings(key("a.md", "-"), vec![finding("x")]);
            memo.keep_tree(&PathBuf::from("root"), [3; 32], AsIs::default());
        });
        assert_eq!(memo.entries(), 0);
        assert_eq!(memo.bytes(), 0);
    }
}
