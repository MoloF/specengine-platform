//! A project's turn, taken and watched (task spec `slow-requests`,
//! RM-0118), and a hub's per-login lines before it (task spec
//! `hub-turn-fairness`, RM-0119): every holder takes the turn here, a
//! request and a hub's MCP call ([`take`]) or a `--push` sync's merge
//! ([`take_blocking`]), so each hold is measured and watched in one place.
//!
//! - **The line**: a hub's MCP call that takes the turn first waits in its
//!   login's FIFO line for the project ([`Lines::line_up`]) and keeps its
//!   place until the call ends; so a login has at most one call holding or
//!   waiting for the turn, and a page request (which takes no line) waits
//!   for at most one call per active login. An entry lives only while a call
//!   holds or waits in it.
//! - **Wait and run**: a request's record ([`Exchange`]) gets the instant
//!   its line and its turn are sought and acquired, the turn's release, the
//!   holders and waiters of the turn ahead of it ([`Seats`]), and whether
//!   the project's `IndexTrust` walked while it was held.
//! - **The watch**: a hold reaching `--slow-ms` writes its `held` line at
//!   once, while still held (a watcher thread's timer, outside the async
//!   runtime, so a stalled runtime still leaves the line); at 0, at the
//!   acquisition, before the call runs. Once `held` is written, the hold's
//!   end writes `released` with its whole length, before the turn passes on.
//!
//! ```text
//! specengine-http: slow turn at=<T> project=<P> by=<R|sync|L> id=<I> tool=<N> state=held held_ms=<n> class=<page|mcp|sync>
//! specengine-http: slow turn at=<T> project=<P> by=<R|sync|L> id=<I> tool=<N> state=released held_ms=<n> class=<page|mcp|sync>
//! ```
//!
//! A hub's delegated apply ([`crate::auto`]) holds as `by=<login> id=<proposal>
//! tool=auto_apply class=mcp`.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use specengine_cli::IndexTrust;
use specengine_cli::check_memo::TurnHeld;
use tokio::sync::OwnedMutexGuard;

use crate::slow::{self, Exchange};
use crate::start::Project;

/// Who holds a turn, by kind: a slow line's `class`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    /// A request of any route but `/mcp`.
    Page,
    /// A hub's MCP call.
    Mcp,
    /// A `--push` sync's merge.
    Sync,
}

impl Class {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::Mcp => "mcp",
            Self::Sync => "sync",
        }
    }
}

/// Who takes a turn, as its lines name it.
pub(crate) struct Holder {
    /// The project's slug.
    project: String,
    /// A request's or call's route template, or `sync`.
    by: Option<String>,
    /// The raw `{id}` segment.
    id: Option<String>,
    /// An MCP call's tool.
    tool: Option<String>,
    class: Class,
    /// The request's record, which the hold's times go to.
    exchange: Option<Exchange>,
}

impl Holder {
    /// A page request's, named from its record (`None`: no record, the
    /// holder `-`).
    pub(crate) fn page(project: &Project, exchange: Option<Exchange>) -> Self {
        Self::request(project, exchange, Class::Page)
    }

    /// A hub's MCP call's, named from its request's record.
    pub(crate) fn call(project: &Project, exchange: Option<Exchange>) -> Self {
        Self::request(project, exchange, Class::Mcp)
    }

    fn request(project: &Project, exchange: Option<Exchange>, class: Class) -> Self {
        let (by, id, tool) = exchange.as_ref().map(Exchange::holder).unwrap_or_default();
        Self {
            project: project.slug.clone(),
            by,
            id,
            tool,
            class,
            exchange,
        }
    }

    /// A hub's delegated apply ([`crate::auto`]), which no request carries:
    /// `by` the login whose call stored the row (as `project` and `id`
    /// print: else `?`), `id` the proposal, `tool` the library call it runs,
    /// class `mcp`.
    pub(crate) fn delegated(project: &Project, login: &str, id: &str, tool: &str) -> Self {
        Self {
            project: project.slug.clone(),
            by: Some(slow::clean(Some(login), false).to_owned()),
            id: Some(id.to_owned()),
            tool: Some(tool.to_owned()),
            class: Class::Mcp,
            exchange: None,
        }
    }

    /// Its `by`, `id`, `tool` and class, as its lines name them: the units'
    /// seam (`crate::auto`), test builds alone.
    #[cfg(test)]
    pub(crate) fn named(&self) -> (Option<&str>, Option<&str>, Option<&str>, Class) {
        (
            self.by.as_deref(),
            self.id.as_deref(),
            self.tool.as_deref(),
            self.class,
        )
    }

    /// A `--push` sync's merge.
    pub(crate) fn sync(project: &Project) -> Self {
        Self {
            project: project.slug.clone(),
            by: Some("sync".to_owned()),
            id: None,
            tool: None,
            class: Class::Sync,
            exchange: None,
        }
    }
}

/// The holders and waiters of a project's turn, each [`take`] counted from
/// its wait to its release: a request line's `ahead`.
#[derive(Debug, Default)]
pub(crate) struct Seats(Arc<AtomicUsize>);

/// One holder's or waiter's seat at the turn, given back when dropped.
struct Seat(Arc<AtomicUsize>);

impl Seats {
    /// A seat, and how many held one before it.
    fn sit(&self) -> (usize, Seat) {
        let ahead = self.0.fetch_add(1, Ordering::SeqCst);
        (ahead, Seat(Arc::clone(&self.0)))
    }
}

impl Drop for Seat {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// `project`'s turn, awaited by the caller's task: dropped while waiting,
/// nothing is held.
pub(crate) async fn take(project: &Project, holder: Holder) -> Held {
    let (ahead, seat) = project.seats.sit();
    if let Some(exchange) = &holder.exchange {
        exchange.seeking(ahead);
    }
    let guard = Arc::clone(&project.turn).lock_owned().await;
    let memo = project.memo.hold_turn();
    Held::new(guard, seat, project.trust.clone(), holder, memo)
}

/// `project`'s turn, on a blocking-pool thread (never the runtime's).
pub(crate) fn take_blocking(project: &Project, holder: Holder) -> Held {
    let (ahead, seat) = project.seats.sit();
    if let Some(exchange) = &holder.exchange {
        exchange.seeking(ahead);
    }
    let guard = Arc::clone(&project.turn).blocking_lock_owned();
    let memo = project.memo.hold_turn();
    Held::new(guard, seat, project.trust.clone(), holder, memo)
}

/// A held turn: dropped, its hold ends (its lines first, then the turn).
pub(crate) struct Held {
    acquired: Instant,
    exchange: Option<Exchange>,
    watch: Arc<Watch>,
    /// The watcher's key of a pending `held` line.
    key: Option<(Instant, u64)>,
    _seat: Seat,
    /// The project's check memo's turn (`write-cost`): a call runs
    /// [`Held::within`] it on its own thread.
    memo: TurnHeld,
    /// Last: released after [`Held`]'s own drop has written its line.
    _guard: OwnedMutexGuard<()>,
}

impl Held {
    fn new(
        guard: OwnedMutexGuard<()>,
        seat: Seat,
        trust: Option<Arc<IndexTrust>>,
        holder: Holder,
        memo: TurnHeld,
    ) -> Self {
        let acquired = Instant::now();
        let at = SystemTime::now();
        let Holder {
            project,
            by,
            id,
            tool,
            class,
            exchange,
        } = holder;
        if let Some(exchange) = &exchange {
            exchange.acquired(acquired, trust);
        }
        let threshold = slow::threshold();
        let watch = Arc::new(Watch {
            at,
            project,
            by,
            id,
            tool,
            class,
            threshold: slow::millis(threshold),
            shown: Mutex::new(Shown::default()),
        });
        let key = if threshold.is_zero() {
            watch.fire();
            None
        } else {
            watcher().map(|watcher| watcher.watch(acquired + threshold, Arc::clone(&watch)))
        };
        Self {
            acquired,
            exchange,
            watch,
            key,
            _seat: seat,
            memo,
            _guard: guard,
        }
    }

    /// `run` on this thread inside the held turn: the project's check memo
    /// reachable here alone while it runs.
    pub(crate) fn within<T>(&self, run: impl FnOnce() -> T) -> T {
        self.memo.within(run)
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        let released = Instant::now();
        if let Some(exchange) = &self.exchange {
            exchange.released(released);
        }
        if let (Some(key), Some(watcher)) = (self.key, WATCHER.get()) {
            watcher.unwatch(key);
        }
        self.watch
            .release(released.saturating_duration_since(self.acquired));
    }
}

/// A project's lines (`Project.lines`): one FIFO per login for its hub MCP
/// calls, in front of the turn; an entry only while a call holds or waits
/// in it, so the map holds active logins alone.
#[derive(Debug, Default)]
pub(crate) struct Lines(Arc<Mutex<HashMap<String, Line>>>);

/// One login's line: its FIFO, and the calls holding or waiting in it.
#[derive(Debug, Default)]
struct Line {
    fifo: Arc<tokio::sync::Mutex<()>>,
    calls: usize,
}

/// A call's place in its login's line, from joining it to the call's end:
/// dropped, the next call of that login moves up; the last one leaving
/// removes the entry.
pub(crate) struct Place {
    lines: Arc<Mutex<HashMap<String, Line>>>,
    login: String,
    held: Option<OwnedMutexGuard<()>>,
}

impl Lines {
    fn map(&self) -> MutexGuard<'_, HashMap<String, Line>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `login`'s place at the head of its line, awaited in FIFO order by
    /// the caller's task: dropped while waiting, it leaves the line. The
    /// wait goes to the request's record.
    pub(crate) async fn line_up(&self, login: &str, exchange: Option<&Exchange>) -> Place {
        let fifo = {
            let mut map = self.map();
            let line = map.entry(login.to_owned()).or_default();
            line.calls += 1;
            Arc::clone(&line.fifo)
        };
        let mut place = Place {
            lines: Arc::clone(&self.0),
            login: login.to_owned(),
            held: None,
        };
        if let Some(exchange) = exchange {
            exchange.lining(Instant::now());
        }
        place.held = Some(fifo.lock_owned().await);
        if let Some(exchange) = exchange {
            exchange.lined(Instant::now());
        }
        place
    }

    /// The logins with a line now, sorted: TF7's seam (task spec
    /// `hub-turn-fairness`), test builds alone; allowed unused, as a test
    /// build without that unit test still compiles it.
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn logins(&self) -> Vec<String> {
        let mut logins: Vec<String> = self.map().keys().cloned().collect();
        logins.sort();
        logins
    }
}

impl Drop for Place {
    fn drop(&mut self) {
        self.held = None;
        let mut map = self.lines.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(line) = map.get_mut(&self.login) {
            line.calls = line.calls.saturating_sub(1);
            if line.calls == 0 {
                map.remove(&self.login);
            }
        }
    }
}

/// One hold's lines: their fields, the text built only for a line written.
struct Watch {
    /// The hold's start.
    at: SystemTime,
    project: String,
    by: Option<String>,
    id: Option<String>,
    tool: Option<String>,
    class: Class,
    /// `--slow-ms`, the `held` line's `held_ms`.
    threshold: u128,
    shown: Mutex<Shown>,
}

#[derive(Default)]
struct Shown {
    held: bool,
    released: bool,
}

impl Watch {
    fn shown(&self) -> MutexGuard<'_, Shown> {
        self.shown.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The line of `state` with `held_ms`.
    fn line(&self, state: &str, held_ms: u128) -> String {
        format!(
            "specengine-http: slow turn at={} project={} by={} id={} tool={} state={state} \
             held_ms={held_ms} class={}",
            slow::utc(self.at),
            slow::clean(Some(&self.project), false),
            self.by.as_deref().unwrap_or("-"),
            slow::clean(self.id.as_deref(), false),
            slow::clean(self.tool.as_deref(), true),
            self.class.word()
        )
    }

    /// The threshold reached, still held: the `held` line, once.
    fn fire(&self) {
        let mut shown = self.shown();
        if shown.held || shown.released {
            return;
        }
        shown.held = true;
        slow::write_line(&self.line("held", self.threshold));
    }

    /// The hold's end after `held`: the `released` line, once `held` was
    /// written.
    fn release(&self, held: Duration) {
        let mut shown = self.shown();
        if shown.released {
            return;
        }
        shown.released = true;
        if shown.held {
            slow::write_line(&self.line("released", slow::millis(held)));
        }
    }
}

/// The holds still short of the threshold, by when they reach it; one
/// thread fires them.
#[derive(Default)]
struct Watcher {
    due: Mutex<BTreeMap<(Instant, u64), Arc<Watch>>>,
    wake: Condvar,
    next: AtomicU64,
}

static WATCHER: OnceLock<Watcher> = OnceLock::new();

/// Whether the watcher's thread started.
static STARTED: OnceLock<bool> = OnceLock::new();

/// The watcher, its thread started on first use; `None` when it cannot
/// start (no `held` line then, nor `released`).
fn watcher() -> Option<&'static Watcher> {
    let watcher: &'static Watcher = WATCHER.get_or_init(Watcher::default);
    let started = *STARTED.get_or_init(|| {
        std::thread::Builder::new()
            .name("slow-turns".to_owned())
            .spawn(move || watcher.run())
            .is_ok()
    });
    started.then_some(watcher)
}

impl Watcher {
    fn due(&self) -> MutexGuard<'_, BTreeMap<(Instant, u64), Arc<Watch>>> {
        self.due.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `watch` fires at `at` unless unwatched first.
    fn watch(&self, at: Instant, watch: Arc<Watch>) -> (Instant, u64) {
        let key = (at, self.next.fetch_add(1, Ordering::Relaxed));
        self.due().insert(key, watch);
        self.wake.notify_one();
        key
    }

    fn unwatch(&self, key: (Instant, u64)) {
        self.due().remove(&key);
    }

    /// Fires each watch at its time, for the process's life.
    fn run(&self) {
        let mut due = self.due();
        loop {
            let now = Instant::now();
            match due.first_key_value().map(|(key, _)| key.0) {
                None => {
                    due = self.wake.wait(due).unwrap_or_else(PoisonError::into_inner);
                }
                Some(at) if at <= now => {
                    if let Some((_, watch)) = due.pop_first() {
                        drop(due);
                        watch.fire();
                        due = self.due();
                    }
                }
                Some(at) => {
                    due = match self.wake.wait_timeout(due, at - now) {
                        Ok((due, _)) => due,
                        Err(poisoned) => poisoned.into_inner().0,
                    };
                }
            }
        }
    }
}
