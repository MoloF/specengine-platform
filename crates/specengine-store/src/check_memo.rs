//! A write's check, remembered (`write-cost`, RM-0125, "C and D"): the
//! as-is findings of one tree state (C) and a write's introduced findings
//! (D), in memory, per process, never stored.
//!
//! - **Keys** (computed, never stored): C by the root and the walk-state
//!   digest (the index stamp, the worktree's fingerprints, every file row,
//!   the walk notes, the config's and the baseline's hashes, today); D by
//!   that digest, the written path, the BLAKE3 of the new bytes and the
//!   login (`-` off a hub), no root: a memo serves one root. So no finding
//!   crosses logins, and C holds the corpus's own findings, no login's data.
//! - **Home**: a daemon keeps one memo per served project, a stdio MCP
//!   server one per root ([`PerRoot`], made at the root's first write), so
//!   no lookup crosses roots; the terminal none. A call reaches it through
//!   [`scoped`] or [`scoped_per_root`], entered inside the project's turn:
//!   the scope is a thread-local of the calling thread, restored on return
//!   or unwind. Outside a scope nothing is remembered.
//! - **The lock** is a leaf: held for one lookup or one insert, never while
//!   computing or waiting on a turn, SQLite or git.
//! - **Budget**: [`BUDGET`] bytes of heap for C and D together, counted as
//!   held: each entry's share of its map's nodes, its boxed key and value,
//!   and every string, list and path it owns by capacity, each allocation
//!   rounded as the allocator rounds it. D is evicted first, least recently
//!   used, then C; an entry larger than the budget is not kept. A miss is
//!   slower, never wrong.
//! - **Doubt** (T4) empties D; C is used again only under the digest the
//!   call's own walk gives.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use specengine_core::check::{AsIs, Debt, Finding, Fix};

use crate::queue::ProposalFinding;

/// C and D's bytes of heap per memo: 32 MiB.
pub const BUDGET: usize = 32 * 1024 * 1024;

/// A BLAKE3 digest.
pub(crate) type Digest = [u8; 32];

/// D's key: what the findings of one write depend on. No root: a memo
/// serves one root (a daemon's project, a [`PerRoot`] entry).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FindingsKey {
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
    trees: BTreeMap<PathBuf, Box<Tree>>,
    /// D, its keys and values boxed: a node holds two pointers an entry, so
    /// an entry's share of the nodes stays small at any fill.
    findings: BTreeMap<Box<FindingsKey>, Box<Remembered>>,
    /// C's and D's entries' bytes ([`tree_bytes`], [`findings_bytes`]).
    bytes: usize,
    /// Each D use's tick: least recently used first out.
    clock: u64,
    /// The config hash each root's last whole check ran under: a call
    /// under another is the cold path.
    warm: BTreeMap<PathBuf, Digest>,
    /// D's lookups by the login asking: (hits, misses).
    by_login: BTreeMap<String, (u64, u64)>,
}

impl State {
    /// The bytes the budget holds to: the entries' and, while D holds any,
    /// D's root node. (An emptied map keeps an empty leaf: constant, and
    /// negligible.)
    fn held(&self) -> usize {
        let root = if self.findings.is_empty() { 0 } else { D_ROOT };
        self.bytes + root
    }
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

    /// Bytes of heap held, as the budget counts them.
    pub fn bytes(&self) -> usize {
        self.state().held()
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
                    .is_some_and(|scope| scope.home.holds(self.id))
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
        let mut remembered = Box::new(Remembered {
            findings,
            bytes: 0,
            used: 0,
        });
        let bytes = remembered_bytes(&key, &remembered);
        if bytes + D_ROOT > self.budget {
            return;
        }
        let mut state = self.state();
        state.clock += 1;
        remembered.bytes = bytes;
        remembered.used = state.clock;
        // The old entry out first: `insert` would keep its key, and the
        // bytes are this key's.
        if let Some(old) = state.findings.remove(&key) {
            state.bytes = state.bytes.saturating_sub(old.bytes);
        }
        state.findings.insert(Box::new(key), remembered);
        state.bytes += bytes;
        self.fit(&mut state, None);
    }

    /// C of `root` when held under `digest`, taken out (the call puts it
    /// back, grown); a C under another digest is dropped.
    pub(crate) fn take_tree(&self, root: &Path, digest: &Digest) -> Option<AsIs> {
        self.reached();
        let mut state = self.state();
        let Tree {
            digest: held,
            as_is,
            bytes,
        } = *state.trees.remove(root)?;
        state.bytes = state.bytes.saturating_sub(bytes);
        (held == *digest).then_some(as_is)
    }

    /// C of `root` under `digest`, when it fits.
    pub(crate) fn keep_tree(&self, root: &Path, digest: Digest, as_is: AsIs) {
        self.reached();
        let owned = root.to_path_buf();
        let mut tree = Box::new(Tree {
            digest,
            as_is,
            bytes: 0,
        });
        let bytes = tree_bytes(&owned, &tree);
        if bytes > self.budget {
            return;
        }
        tree.bytes = bytes;
        let mut state = self.state();
        // The old entry out first: `insert` would keep its key, and the
        // bytes are this key's.
        if let Some(old) = state.trees.remove(root) {
            state.bytes = state.bytes.saturating_sub(old.bytes);
        }
        state.trees.insert(owned, tree);
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
    /// C, another root's before `keep`'s. At the cap each eviction scans
    /// every D entry for the oldest: O(n) a write.
    fn fit(&self, state: &mut State, keep: Option<&Path>) {
        while state.held() > self.budget {
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

// The heap an entry holds, as the budget counts it: a fair estimate that
// stays at or above what the entry allocates. Each allocation is counted as
// the block the allocator gives it (glibc's chunk on Linux with glibc, the
// hub's Debian; size classes elsewhere); a map's nodes are std's
// `BTreeMap`'s, shared out by entry, and D's root node is counted once while
// D holds any.

/// The allocator's quantum: 16 bytes, macOS's and glibc's alike.
const QUANTUM: usize = 16;

/// Size classes: up to this size an allocation is rounded to the quantum;
/// above it, to a quarter of its power of two (macOS's).
const QUANTUM_UP_TO: usize = 128;

/// glibc's chunk header, beside the bytes asked for.
const GLIBC_HEADER: usize = 8;

/// glibc's least chunk.
const GLIBC_LEAST: usize = 32;

/// A `BTreeMap` node's entries at most (std's `CAPACITY`) and, in every node
/// but the root, at least (its `MIN_LEN`).
const NODE_MOST: usize = 11;
const NODE_LEAST: usize = 5;

/// A node's fields beside its entries: the parent link, the index there and
/// the length, padded.
const NODE_HEADER: usize = 16;

/// D's entry in a node: its boxed key and its boxed value.
const D_SLOT: usize = size_of::<Box<FindingsKey>>() + size_of::<Box<Remembered>>();

/// D's root node, counted whole while D holds any entry ([`State::held`]).
const D_ROOT: usize = internal(D_SLOT);

/// The heap one allocation of `size` bytes holds; none for no allocation.
const fn heap(size: usize) -> usize {
    if size == 0 {
        0
    } else if cfg!(all(target_os = "linux", target_env = "gnu")) {
        glibc_chunk(size)
    } else {
        size_class(size)
    }
}

/// glibc's chunk for `size` bytes: the size and its header rounded up to the
/// quantum, at least [`GLIBC_LEAST`] (mmapped blocks, 128 KiB and up, aside).
const fn glibc_chunk(size: usize) -> usize {
    let chunk = (size + GLIBC_HEADER).div_ceil(QUANTUM) * QUANTUM;
    if chunk < GLIBC_LEAST {
        GLIBC_LEAST
    } else {
        chunk
    }
}

/// A size-class allocator's block for `size` bytes (macOS's).
const fn size_class(size: usize) -> usize {
    if size <= QUANTUM_UP_TO {
        return size.div_ceil(QUANTUM) * QUANTUM;
    }
    let power = 1 << (usize::BITS - 1 - (size - 1).leading_zeros());
    let step = power / 4;
    size.div_ceil(step) * step
}

/// A leaf node of a map whose entry takes `slot` bytes in a node.
const fn leaf(slot: usize) -> usize {
    heap(NODE_MOST * slot + NODE_HEADER)
}

/// An internal node: a leaf's fields and an edge per child.
const fn internal(slot: usize) -> usize {
    heap(NODE_MOST * slot + NODE_HEADER + (NODE_MOST + 1) * size_of::<usize>())
}

/// One entry's share of a map's nodes, every node at its least fill: per
/// leaf of [`NODE_LEAST`] entries, a fifth of an internal node holding as
/// many again. An upper bound, the root node aside.
const fn node_share(slot: usize) -> usize {
    (leaf(slot) + internal(slot).div_ceil(NODE_LEAST)).div_ceil(NODE_LEAST + 1)
}

/// D's bytes of an entry about to be kept: [`findings_bytes`] of its key and
/// its value's list. Every field of the value is named, so a new one fails
/// to compile here until it is counted.
fn remembered_bytes(key: &FindingsKey, remembered: &Remembered) -> usize {
    let Remembered {
        findings,
        bytes: _,
        used: _,
    } = remembered;
    findings_bytes(key, findings)
}

/// D's bytes: the entry's share of D's nodes, its boxed key and value, and
/// the strings and the list they own, by capacity.
#[allow(clippy::ptr_arg)] // the list's capacity, not its length
fn findings_bytes(key: &FindingsKey, findings: &Vec<ProposalFinding>) -> usize {
    let FindingsKey {
        tree: _,
        path,
        bytes: _,
        login,
    } = key;
    node_share(D_SLOT)
        + heap(size_of::<FindingsKey>())
        + heap(path.capacity())
        + heap(login.capacity())
        + heap(size_of::<Remembered>())
        + heap(findings.capacity() * size_of::<ProposalFinding>())
        + findings
            .iter()
            .map(|finding| {
                let ProposalFinding {
                    code,
                    severity: _,
                    path,
                    line: _,
                    subject,
                    message,
                } = finding;
                heap(code.capacity())
                    + heap(path.capacity())
                    + heap(subject.capacity())
                    + heap(message.capacity())
            })
            .sum::<usize>()
}

/// C's bytes: a whole leaf of C's map (a memo serves one root, so its C is
/// one leaf), the root's path, the boxed tree and its findings. Every field
/// of the tree is named, so a new one fails to compile here until counted.
fn tree_bytes(root: &PathBuf, tree: &Tree) -> usize {
    let Tree {
        digest: _,
        as_is,
        bytes: _,
    } = tree;
    leaf(size_of::<PathBuf>() + size_of::<Box<Tree>>())
        + heap(root.capacity())
        + heap(size_of::<Tree>())
        + as_is_bytes(as_is)
}

/// An as-is state's bytes: its by-path map (the root node counted whole),
/// each path and list, and the global passes' lists.
fn as_is_bytes(as_is: &AsIs) -> usize {
    let AsIs {
        by_path,
        cycles,
        parents,
        index,
    } = as_is;
    let slot = size_of::<String>() + size_of::<Vec<Finding>>();
    let root = if by_path.is_empty() {
        0
    } else {
        internal(slot)
    };
    root + by_path
        .iter()
        .map(|(path, found)| node_share(slot) + heap(path.capacity()) + list_bytes(found))
        .sum::<usize>()
        + [cycles, parents, index]
            .into_iter()
            .flatten()
            .map(list_bytes)
            .sum::<usize>()
}

/// A list of findings' bytes: its buffer by capacity and each finding's
/// strings.
#[allow(clippy::ptr_arg)] // the list's capacity, not its length
fn list_bytes(found: &Vec<Finding>) -> usize {
    heap(found.capacity() * size_of::<Finding>()) + found.iter().map(finding_bytes).sum::<usize>()
}

/// A finding's strings, its fix's and its debt's among them.
fn finding_bytes(finding: &Finding) -> usize {
    let Finding {
        code,
        severity: _,
        path,
        line: _,
        subject,
        message,
        fix,
        debt,
        introduced: _,
    } = finding;
    let fix = fix.as_ref().map_or(0, |fix| {
        let Fix { span: _, text } = fix;
        heap(text.capacity())
    });
    let debt = debt.as_ref().map_or(0, |debt| {
        let Debt {
            reason,
            expires,
            expired: _,
        } = debt;
        heap(reason.capacity()) + heap(expires.capacity())
    });
    heap(code.capacity())
        + heap(path.capacity())
        + heap(subject.capacity())
        + heap(message.capacity())
        + fix
        + debt
}

/// A stdio server's check memos: one [`CheckMemo`] of [`BUDGET`] bytes per
/// project root, made at that root's first write, so D, whose key holds no
/// root, never answers one root's write from another's.
#[derive(Debug, Default)]
pub struct PerRoot {
    memos: Mutex<BTreeMap<PathBuf, Arc<CheckMemo>>>,
}

impl PerRoot {
    /// No memo yet.
    pub fn new() -> Self {
        Self::default()
    }

    fn memos(&self) -> MutexGuard<'_, BTreeMap<PathBuf, Arc<CheckMemo>>> {
        self.memos.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `root`'s memo, made at its first use.
    fn memo(&self, root: &Path) -> Arc<CheckMemo> {
        let mut memos = self.memos();
        if let Some(memo) = memos.get(root) {
            return Arc::clone(memo);
        }
        let memo = Arc::new(CheckMemo::new());
        memos.insert(root.to_path_buf(), Arc::clone(&memo));
        memo
    }

    /// The memo numbered `id` is one of these.
    fn holds(&self, id: u64) -> bool {
        self.memos().values().any(|memo| memo.id == id)
    }

    /// A doubt reaching every root's memo, each lock taken alone.
    fn doubt(&self) {
        let memos: Vec<Arc<CheckMemo>> = self.memos().values().cloned().collect();
        for memo in memos {
            memo.doubt();
        }
    }
}

/// Where a scope's memo comes from.
#[derive(Clone)]
enum Home {
    /// One memo: a daemon's project's.
    One(Arc<CheckMemo>),
    /// A stdio server's: the memo of the root a write reaches.
    PerRoot(Arc<PerRoot>),
}

impl Home {
    fn memo(&self, root: &Path) -> Arc<CheckMemo> {
        match self {
            Self::One(memo) => Arc::clone(memo),
            Self::PerRoot(memos) => memos.memo(root),
        }
    }

    fn holds(&self, id: u64) -> bool {
        match self {
            Self::One(memo) => memo.id == id,
            Self::PerRoot(memos) => memos.holds(id),
        }
    }

    fn doubt(&self) {
        match self {
            Self::One(memo) => memo.doubt(),
            Self::PerRoot(memos) => memos.doubt(),
        }
    }
}

/// The memo in force on a thread and the login its writes are keyed by.
#[derive(Clone)]
struct Scope {
    home: Home,
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
    enter(Home::One(Arc::clone(memo)), login, run)
}

/// `run` with `memos` in force on this thread, as [`scoped`]: a write
/// reaches the memo of its own root (a stdio server's, which has no turn).
pub fn scoped_per_root<T>(memos: &Arc<PerRoot>, login: &str, run: impl FnOnce() -> T) -> T {
    enter(Home::PerRoot(Arc::clone(memos)), login, run)
}

fn enter<T>(home: Home, login: &str, run: impl FnOnce() -> T) -> T {
    if let Home::One(memo) = &home {
        debug_assert!(
            memo.in_turn(),
            "the check memo's scope is entered outside its project's turn"
        );
    }
    let previous = SCOPE.with_borrow_mut(|scope| {
        scope.replace(Scope {
            home,
            login: login.to_owned(),
        })
    });
    let _restore = Restore(previous);
    run()
}

/// `run` with the scope's writes keyed by `login`: a call that knows its
/// login (a confirm's `[team]` entry). Outside a scope, `run` alone.
pub fn as_login<T>(login: &str, run: impl FnOnce() -> T) -> T {
    match home() {
        Some(home) => enter(home, login, run),
        None => run(),
    }
}

/// A doubt (T4) reaching the memo in force (a stdio server's: each root's):
/// D emptied. Outside a scope, nothing.
pub fn doubt() {
    if let Some(home) = home() {
        home.doubt();
    }
}

/// Where the scope in force takes its memo from.
fn home() -> Option<Home> {
    SCOPE
        .try_with(|scope| scope.borrow().as_ref().map(|scope| scope.home.clone()))
        .ok()
        .flatten()
}

/// The memo in force for a write under `root`, and its login.
pub(crate) fn current(root: &Path) -> Option<(Arc<CheckMemo>, String)> {
    SCOPE
        .try_with(|scope| {
            scope
                .borrow()
                .as_ref()
                .map(|scope| (scope.home.memo(root), scope.login.clone()))
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
        // Two entries and D's root node.
        let budget = size * 2 + super::D_ROOT;
        let memo = Arc::new(CheckMemo::with_budget(budget));
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
            assert!(memo.bytes() <= budget);
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

    /// WC6: one memo keeps each root's C apart (C is keyed by its root) and
    /// holds C and D within its one budget. D's key holds no root
    /// (write-memory): a stdio server keeps D apart by root with a memo per
    /// root ([`super::PerRoot`]), pinned by `tests/check_memo_heap.rs`
    /// `wm7_the_stdio_memos_keep_two_roots_apart`.
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
                memo.keep_findings(key(&format!("{n}.md"), "-"), vec![finding(&"m".repeat(64))]);
                assert!(memo.bytes() <= 2048, "C and D within the one budget");
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
