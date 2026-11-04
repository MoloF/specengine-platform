//! A hub's `--push` sync (task spec `hub-git`): one task per git repository
//! the hub serves, keyed by the worktrees' common directory, so two roots in
//! one checkout share one sync. The CLI library runs each sync
//! ([`specengine_cli::sync_once`]) on the blocking pool; its merge phase alone
//! takes turns, those of every project served in the repository, in
//! `--root` order, so a slow remote never delays a read or a confirm; it
//! runs in the CLI's `walking` scope of their trusts (a write: nothing in
//! it trusts the index), so its failures' doubts reach them (the
//! `read-trust` spec, "The daemon").
//!
//! - **When**: once at start, after `listening`; after a confirm that
//!   committed ([`RepoSync::signal`], never awaited); every `--push-every`.
//!   One sync at a time per repository; signals while one runs coalesce
//!   into one more run.
//! - **Retries**: `retrying` doubles the timer's wait per consecutive
//!   failure, up to eight intervals; a signal runs at once. `held` and
//!   `stopped` wait one interval (the library keeps a `held` while neither
//!   tip moved).
//! - **Memory**: the last outcome and its end, `at`, nothing stored; each
//!   end's `at` differs from the one before. A change of state or error is
//!   one stderr line, `specengine-http: sync <worktree>: <state>[: <error>]`.
//! - **`GET .../sync`** ([`RepoSync::status`]): `ahead` and `behind` from
//!   git now, the rest from memory; no turn.

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as Held};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use specengine_cli::{
    Env, GitEnv, HubSync, IndexTrust, SyncOutcome, SyncRepository, SyncState, hub_sync,
    process_git, sync_key, sync_line, sync_once, utc_now, walking,
};
use tokio::sync::{Mutex, Notify};

use crate::start::Project;

/// The most intervals a `retrying` sync waits.
const BACKOFF_MAX: u32 = 8;

/// `--push`'s settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Push {
    /// `--push-every`.
    pub every: Duration,
    /// `--push-deadline`: each git child of a sync.
    pub deadline: Duration,
}

/// The daemon's memory of one repository's sync.
#[derive(Debug, Default)]
struct Memory {
    last: SyncOutcome,
    /// The last sync's end, `YYYY-MM-DDTHH:MM:SSZ`.
    at: Option<String>,
}

/// One repository's sync.
#[derive(Debug)]
pub(crate) struct RepoSync {
    /// The checkout's top (a root that lies in no repository: that root).
    top: PathBuf,
    /// The repository and its served roots; `Err`: why no sync can run.
    repository: Result<SyncRepository, String>,
    /// The turns of its projects, in `--root` order.
    turns: Vec<Arc<Mutex<()>>>,
    /// Their trusts (a hub's), which the merge runs in: its update keeps
    /// the indexes, its failures doubt them.
    trusts: Vec<Arc<IndexTrust>>,
    signal: Notify,
    memory: Held<Memory>,
}

impl RepoSync {
    /// A sync to run as soon as possible: one more run if one is running,
    /// never awaited.
    pub(crate) fn signal(&self) {
        self.signal.notify_one();
    }

    /// The status document now, `ahead` and `behind` read from git on the
    /// blocking pool.
    pub(crate) async fn status(self: Arc<Self>, deadline: Duration) -> Option<HubSync> {
        let (last, at) = {
            let memory = self.memory.lock().ok()?;
            (memory.last.clone(), memory.at.clone())
        };
        tokio::task::spawn_blocking(move || {
            let git = Env::from_process()
                .map(|env| process_git(&env))
                .unwrap_or_else(|_| GitEnv::new(self.top.clone(), std::env::vars_os()));
            hub_sync(&self.top, &git, deadline, &last, at.as_deref())
        })
        .await
        .ok()
    }

    /// One sync on the blocking pool, the merge inside the turns.
    async fn once(self: &Arc<Self>, deadline: Duration) -> SyncOutcome {
        let last = self
            .memory
            .lock()
            .map(|memory| memory.last.clone())
            .unwrap_or_default();
        let sync = Arc::clone(self);
        let joined = tokio::task::spawn_blocking(move || {
            let repository = match &sync.repository {
                Ok(repository) => repository,
                Err(why) => return stopped(why.clone()),
            };
            let env = match Env::from_process() {
                Ok(env) => env,
                Err(error) => return stopped(error.message),
            };
            let git = process_git(&env);
            let turns = &sync.turns;
            let trusts: Vec<&IndexTrust> = sync.trusts.iter().map(Arc::as_ref).collect();
            let mut in_turns = |merge: &mut dyn FnMut()| {
                // In `--root` order; a request holding one is awaited, a
                // request waiting for one waits for the merge.
                let held: Vec<_> = turns.iter().map(|turn| turn.blocking_lock()).collect();
                walking(&trusts, &mut *merge);
                drop(held);
            };
            sync_once(repository, &env, &git, deadline, &last, &mut in_turns)
        })
        .await;
        joined.unwrap_or_else(|_| stopped("internal error: the sync failed".to_owned()))
    }

    /// `outcome` kept as the last, its end at a time unlike the one before;
    /// a change of state or error told on stderr.
    async fn remember(&self, outcome: SyncOutcome) {
        let previous = self
            .memory
            .lock()
            .map(|memory| memory.at.clone())
            .unwrap_or_default();
        let mut at = utc_now();
        while previous.as_deref() == Some(at.as_str()) {
            let into = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.subsec_millis());
            tokio::time::sleep(Duration::from_millis(u64::from(1_000 - into.min(999)) + 1)).await;
            at = utc_now();
        }
        let changed = {
            let Ok(mut memory) = self.memory.lock() else {
                return;
            };
            let changed = memory.last.state != outcome.state || memory.last.error != outcome.error;
            memory.last = outcome.clone();
            memory.at = Some(at);
            changed
        };
        if changed {
            let _ = writeln!(
                std::io::stderr().lock(),
                "{}",
                sync_line(&self.top, &outcome)
            );
        }
    }
}

/// A `stopped` outcome of `why`.
fn stopped(why: String) -> SyncOutcome {
    SyncOutcome::stopped(why)
}

/// The projects of one worktree while [`Syncs::new`] groups them.
struct Group {
    /// The common directory and the worktree's top; `None`: no repository
    /// found.
    key: Option<(PathBuf, PathBuf)>,
    top: PathBuf,
    repository: Result<SyncRepository, String>,
    turns: Vec<Arc<Mutex<()>>>,
    trusts: Vec<Arc<IndexTrust>>,
}

/// Every repository's sync, and which serves each project.
#[derive(Debug)]
pub(crate) struct Syncs {
    push: Push,
    /// In the order their first project was given.
    all: Vec<Arc<RepoSync>>,
    /// Each slug and its sync's place in `all`, in `--root` order.
    of: Vec<(String, usize)>,
}

impl Syncs {
    /// The projects grouped by their repository's common directory (git
    /// asked once per root), the sync run in the first served root's
    /// worktree: a project served from another worktree of that repository
    /// is no part of it (its roots out of the look, its turn not taken) and
    /// reads `stopped`, `this worktree is not synced: the sync runs in
    /// <top>`, as a root git finds no repository for reads git's words.
    pub(crate) fn new(projects: &[Arc<Project>], push: Push) -> Self {
        let git = Env::from_process().map(|env| process_git(&env));
        let mut groups: Vec<Group> = Vec::new();
        let mut of = Vec::new();
        for project in projects {
            let place = match &git {
                Ok(git) => sync_key(git, &project.root),
                Err(error) => Err(error.message.clone()),
            };
            let turn = Arc::clone(&project.turn);
            let trust = project.trust.clone();
            let slot = match place {
                Ok((top, common)) => {
                    let key = Some((common.clone(), top.clone()));
                    if let Some(slot) = groups.iter().position(|group| group.key == key) {
                        let group = &mut groups[slot];
                        if let Ok(repository) = &mut group.repository {
                            repository.roots.push(project.root.clone());
                        }
                        group.turns.push(turn);
                        group.trusts.extend(trust);
                        slot
                    } else {
                        // The repository's sync, if one runs in another
                        // worktree already.
                        let synced = groups.iter().find(|group| {
                            group.repository.is_ok()
                                && group
                                    .key
                                    .as_ref()
                                    .is_some_and(|(other, _)| *other == common)
                        });
                        let repository = match synced {
                            Some(synced) => Err(format!(
                                "this worktree is not synced: the sync runs in {}",
                                synced.top.display()
                            )),
                            None => Ok(SyncRepository {
                                top: top.clone(),
                                roots: vec![project.root.clone()],
                            }),
                        };
                        groups.push(Group {
                            key,
                            top,
                            repository,
                            turns: vec![turn],
                            trusts: trust.into_iter().collect(),
                        });
                        groups.len() - 1
                    }
                }
                Err(why) => {
                    groups.push(Group {
                        key: None,
                        top: project.root.clone(),
                        repository: Err(format!("{}: {why}", project.root.display())),
                        turns: vec![turn],
                        trusts: trust.into_iter().collect(),
                    });
                    groups.len() - 1
                }
            };
            of.push((project.slug.clone(), slot));
        }
        let all = groups
            .into_iter()
            .map(|group| {
                Arc::new(RepoSync {
                    top: group.top,
                    repository: group.repository,
                    turns: group.turns,
                    trusts: group.trusts,
                    signal: Notify::new(),
                    memory: Held::new(Memory::default()),
                })
            })
            .collect();
        Self { push, all, of }
    }

    /// The sync of the project served as `slug`.
    pub(crate) fn of(&self, slug: &str) -> Option<&Arc<RepoSync>> {
        self.of
            .iter()
            .find(|(served, _)| served == slug)
            .and_then(|(_, slot)| self.all.get(*slot))
    }

    /// `--push-deadline`.
    pub(crate) fn deadline(&self) -> Duration {
        self.push.deadline
    }

    /// One task per repository, each running its first sync now.
    pub(crate) fn start(&self) {
        for sync in &self.all {
            tokio::spawn(run(Arc::clone(sync), self.push));
        }
    }
}

/// A repository's syncs, for as long as the daemon runs (see the module
/// documentation).
async fn run(sync: Arc<RepoSync>, push: Push) {
    let mut failures: u32 = 0;
    loop {
        let outcome = sync.once(push.deadline).await;
        let factor = if outcome.state == SyncState::Retrying {
            failures = failures.saturating_add(1);
            2u32.saturating_pow(failures).min(BACKOFF_MAX)
        } else {
            failures = 0;
            1
        };
        sync.remember(outcome).await;
        let wait = push.every.saturating_mul(factor);
        tokio::select! {
            () = sync.signal.notified() => {}
            () = tokio::time::sleep(wait) => {}
        }
    }
}
