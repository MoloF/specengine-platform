//! docs/features/queue-export.md, the store half: `SqliteQueue`'s raw rows
//! for the queue's backup. [`PROPOSAL_COLUMNS`] and [`EVENT_COLUMNS`] are
//! the tables' columns in table order (checked against `dump()`, which reads
//! `SELECT *`); `open_existing` creates nothing and reads a DB the queue's
//! steps have not reached as an empty queue; `stored_rows` gives every
//! project's rows raw (an unreadable row too), `proposals` by ID number and
//! `events` by `seq`, whatever the insertion order; `restore` runs the
//! schema steps, inserts every row as given in one transaction (a failing
//! row leaves nothing), refuses an occupied queue (any project's rows) and
//! another project's row, logs no event of its own, and the next ID and
//! `seq` follow the highest restored.
//!
//! Through the public API only (`api.rs`: no test but `format.rs` names the
//! SQL crate); the one raw step (a newer `user_version`) uses the system's
//! `sqlite3` client.
//!
//! Iteration 2, the git side of `export state`'s destination check
//! (`WorktreeGit`): `top_if_repository` is `None` only where git finds no
//! repository (its own untranslated words: a German locale changes
//! nothing, with the real git or one that translates), and an error for a
//! broken `.git` file, a dubious owner, a git directory, a missing
//! directory; `worktrees()` lists the main worktree first (a bare
//! repository itself, flagged), then every linked one, a deleted one never
//! pruned included as git prints it, a path with a space and a newline
//! whole (`-z`). Every git process runs in the sandbox of the CLI tests'
//! `common::git`.

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use common::{Scratch, blake3_hex};
use git::Sandbox;
use specengine_core::proposal::Author;
use specengine_store::{
    EVENT_COLUMNS, GitEnv, GitError, ListedWorktree, NewProposal, PROPOSAL_COLUMNS, Place,
    ProposalKind, ProposalQueue as _, QueueCounts, QueueError, Restore, SqliteQueue, StoredEvent,
    StoredProposal, StoredQueue, WorktreeGit, patch_hash,
};

const T0: &str = "2026-10-05T21:14:03Z";
const PROJECT: &str = "demo";

fn new_proposal(target: &str, common_dir: &str) -> NewProposal {
    let base_hash = format!("b3:{}", blake3_hex(b"base"));
    NewProposal {
        kind: ProposalKind::Update,
        target_id: target.to_owned(),
        target_path: "docs/x.md".to_owned(),
        place: Place {
            git_common_dir: common_dir.to_owned(),
            worktree: "/w/t1".to_owned(),
            root_rel: String::new(),
            branch: "t1".to_owned(),
            base_commit: "0".repeat(40),
        },
        patch_hash: patch_hash(target, &base_hash, "new"),
        base_hash,
        base_text: "base".to_owned(),
        new_text: "new".to_owned(),
        rationale: "Why.".to_owned(),
        author: Author::human(),
        diagnostics: Vec::new(),
    }
}

/// A `proposals` row of `project` as stored: the given `id`, every other
/// column `<column>-<id>`, `decision_note` `NULL`, `base_commit` `HEAD`
/// (a value `get` refuses) when `unreadable`.
fn row(id: &str, project: &str, unreadable: bool) -> StoredProposal {
    let mut columns: [Option<String>; PROPOSAL_COLUMNS.len()] = std::array::from_fn(|_| None);
    for (slot, column) in columns.iter_mut().zip(PROPOSAL_COLUMNS) {
        *slot = Some(format!("{column}-{id}"));
    }
    columns[0] = Some(id.to_owned());
    columns[1] = Some(project.to_owned());
    columns[3] = Some("open".to_owned());
    let at = |name: &str| {
        PROPOSAL_COLUMNS
            .iter()
            .position(|column| *column == name)
            .unwrap_or_else(|| panic!("no column {name}"))
    };
    columns[at("decision_note")] = None;
    if unreadable {
        columns[at("base_commit")] = Some("HEAD".to_owned());
    }
    StoredProposal { columns }
}

fn event(seq: i64, project: &str) -> StoredEvent {
    StoredEvent {
        seq,
        columns: [
            Some(project.to_owned()),
            Some("proposal.created".to_owned()),
            Some(format!("{{\"id\":\"PR-{seq:04}\"}}")),
            Some(T0.to_owned()),
        ],
    }
}

fn ids(state: &StoredQueue) -> Vec<&str> {
    state
        .proposals
        .iter()
        .map(|row| row.id().unwrap_or("NULL"))
        .collect()
}

fn seqs(state: &StoredQueue) -> Vec<i64> {
    state.events.iter().map(|row| row.seq).collect()
}

/// `sqlite3 <db> <sql>`, which must succeed.
fn sqlite3(db: &Path, sql: &str) {
    let client = ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
        .expect("a sqlite3 client");
    let output = Command::new(client)
        .arg(db)
        .arg(sql)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The column lists are the tables' columns in table order: a row whose
/// every value is its own column's name, restored by name, reads back from
/// `dump()` (`SELECT *`, table order) as exactly the lists. M: a column
/// dropped from or moved in either list.
#[test]
fn the_column_lists_are_the_tables_columns_in_table_order() {
    let scratch = Scratch::new("qs-columns");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, "project").expect("open");
    let mut proposal = StoredProposal {
        columns: std::array::from_fn(|_| None),
    };
    for (slot, column) in proposal.columns.iter_mut().zip(PROPOSAL_COLUMNS) {
        *slot = Some(column.to_owned());
    }
    let mut named = StoredEvent {
        seq: 1,
        columns: Default::default(),
    };
    for (slot, column) in named.columns.iter_mut().zip(&EVENT_COLUMNS[1..]) {
        *slot = Some((*column).to_owned());
    }
    let state = StoredQueue {
        proposals: vec![proposal],
        events: vec![named],
    };
    assert_eq!(queue.restore(&state).expect("restore"), Restore::Restored);
    let mut events = vec![serde_json::json!(1)];
    events.extend(
        EVENT_COLUMNS[1..]
            .iter()
            .map(|column| serde_json::json!(column)),
    );
    let want = format!(
        "proposals\t{}\nevents\t{}\n",
        serde_json::json!(PROPOSAL_COLUMNS.as_slice()),
        serde_json::Value::Array(events)
    );
    assert_eq!(queue.dump().expect("dump"), want);
    assert_eq!(EVENT_COLUMNS[0], "seq");
    assert_eq!(queue.stored_rows().expect("stored rows"), state);
}

/// `open_existing`: no file → `None`, nothing created (not even its
/// directory); a DB without the queue's tables (an index DB) reads as an
/// empty queue and gets no table from reading (`dump()` still fails);
/// `restore` on that handle runs the schema steps first. M: `open_existing`
/// creating the file; `restore` without its schema step.
#[test]
fn open_existing_creates_nothing_and_restore_runs_the_schema_steps() {
    let scratch = Scratch::new("qs-existing");
    let missing = scratch.join("no/such/dir/q.db");
    assert!(
        SqliteQueue::open_existing(&missing, PROJECT)
            .expect("a missing file is no error")
            .is_none()
    );
    assert!(!scratch.join("no").exists(), "nothing created");

    let db = scratch.db("index-only");
    let root = scratch.join("root");
    std::fs::create_dir_all(&root).expect("root");
    drop(common::open(&db, &root));
    let mut queue = SqliteQueue::open_existing(&db, PROJECT)
        .expect("open_existing")
        .expect("the file exists");
    assert_eq!(
        queue.stored_rows().expect("stored rows"),
        StoredQueue::default()
    );
    assert_eq!(queue.counts().expect("counts"), QueueCounts::default());
    assert!(queue.counts().unwrap().is_empty());
    assert!(
        queue.dump().is_err(),
        "reading made no queue table: {:?}",
        queue.dump()
    );

    let state = StoredQueue {
        proposals: vec![row("PR-0001", PROJECT, false)],
        events: vec![event(1, PROJECT)],
    };
    assert_eq!(queue.restore(&state).expect("restore"), Restore::Restored);
    assert_eq!(queue.stored_rows().expect("stored rows"), state);
    assert_eq!(
        queue.counts().expect("counts"),
        QueueCounts {
            proposals: 1,
            events: 1
        }
    );
    // A handle opened afterwards reads the same rows.
    let reopened = SqliteQueue::open_existing(&db, PROJECT).unwrap().unwrap();
    assert_eq!(reopened.stored_rows().unwrap(), state);
}

/// A `user_version` above this build's: `open_existing` refuses it as
/// `SchemaTooNew`.
#[test]
fn open_existing_refuses_a_newer_schema() {
    let scratch = Scratch::new("qs-newer");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    sqlite3(&db, "PRAGMA user_version = 3");
    match SqliteQueue::open_existing(&db, PROJECT) {
        Err(QueueError::SchemaTooNew { found: 3 }) => {}
        Err(other) => panic!("expected SchemaTooNew, got {other}"),
        Ok(_) => panic!("expected SchemaTooNew, got a handle"),
    }
}

/// `stored_rows`: every project's rows, an unreadable one kept as it is
/// (`get` refuses it), `proposals` by ID number (`PR-9999` before
/// `PR-10000`) and `events` by `seq`, whatever the insertion order; equal
/// rows inserted in another order read back equal. M: `ORDER BY rowid`;
/// rows filtered by project or by `list_readable`.
#[test]
fn stored_rows_are_every_projects_rows_raw_in_id_and_seq_order() {
    let scratch = Scratch::new("qs-rows");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let state = StoredQueue {
        proposals: vec![
            row("PR-10000", PROJECT, false),
            row("PR-0002", PROJECT, true),
            row("PR-9999", PROJECT, false),
            row("PR-0001", PROJECT, false),
        ],
        events: vec![event(3, PROJECT), event(1, PROJECT), event(2, PROJECT)],
    };
    assert_eq!(queue.restore(&state).expect("restore"), Restore::Restored);
    let read = queue.stored_rows().expect("stored rows");
    assert_eq!(ids(&read), ["PR-0001", "PR-0002", "PR-9999", "PR-10000"]);
    assert_eq!(seqs(&read), [1, 2, 3]);
    assert!(queue.get("PR-0002").is_err(), "`get` refuses the row");
    assert!(read.proposals.contains(&row("PR-0002", PROJECT, true)));

    // Another project's handle on the same DB adds its own row.
    let mut other = SqliteQueue::open(&db, "other").expect("open other");
    let created = other
        .create(&new_proposal("X-1", "/r/.git"), T0)
        .expect("create");
    let read = queue.stored_rows().expect("stored rows");
    assert_eq!(read.proposals.len(), 5, "every project's rows");
    assert!(
        read.proposals
            .iter()
            .any(|row| row.id() == Some(created.id.as_str()) && row.project() == Some("other"))
    );
    assert_eq!(read.events.len(), 4);
    assert_eq!(
        queue.counts().unwrap(),
        QueueCounts {
            proposals: 5,
            events: 4
        }
    );
    assert_eq!(
        other.stored_rows().unwrap(),
        read,
        "the same from either handle"
    );

    // The same rows inserted in the opposite order read back equal.
    let db2 = scratch.db("q2");
    let mut reversed = state.clone();
    reversed.proposals.reverse();
    reversed.events.reverse();
    let mut queue2 = SqliteQueue::open(&db2, PROJECT).expect("open");
    assert_eq!(queue2.restore(&reversed).unwrap(), Restore::Restored);
    let db3 = scratch.db("q3");
    let mut queue3 = SqliteQueue::open(&db3, PROJECT).expect("open");
    assert_eq!(queue3.restore(&state).unwrap(), Restore::Restored);
    assert_eq!(queue2.stored_rows().unwrap(), queue3.stored_rows().unwrap());
    assert_eq!(queue2.dump().unwrap(), queue3.dump().unwrap());
}

/// `restore` is one transaction: a row the insert refuses (a repeated `id`,
/// a repeated `seq`) leaves the queue empty; another project's row is
/// `Invalid` before anything is written; an occupied queue (another
/// project's rows count) is `Occupied` with its counts, nothing inserted.
/// M: commit per row; `INSERT OR IGNORE`; the emptiness check removed.
#[test]
fn restore_is_all_or_nothing_into_an_empty_queue_only() {
    let scratch = Scratch::new("qs-atomic");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let empty = queue.dump().expect("dump");

    let repeated_id = StoredQueue {
        proposals: vec![
            row("PR-0001", PROJECT, false),
            row("PR-0002", PROJECT, false),
            row("PR-0001", PROJECT, false),
        ],
        events: vec![event(1, PROJECT)],
    };
    assert!(queue.restore(&repeated_id).is_err(), "a repeated id fails");
    assert_eq!(queue.dump().unwrap(), empty, "nothing kept");
    assert!(queue.counts().unwrap().is_empty());

    let repeated_seq = StoredQueue {
        proposals: vec![row("PR-0001", PROJECT, false)],
        events: vec![event(1, PROJECT), event(2, PROJECT), event(1, PROJECT)],
    };
    assert!(
        queue.restore(&repeated_seq).is_err(),
        "a repeated seq fails"
    );
    assert_eq!(queue.dump().unwrap(), empty, "nothing kept");

    for foreign in [
        StoredQueue {
            proposals: vec![
                row("PR-0001", PROJECT, false),
                row("PR-0002", "other", false),
            ],
            events: Vec::new(),
        },
        StoredQueue {
            proposals: vec![row("PR-0001", PROJECT, false)],
            events: vec![event(1, PROJECT), event(2, "other")],
        },
    ] {
        match queue.restore(&foreign) {
            Err(QueueError::Invalid(message)) => {
                assert!(message.contains(PROJECT), "{message}");
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
        assert_eq!(queue.dump().unwrap(), empty, "nothing written");
    }

    let mut other = SqliteQueue::open(&db, "other").expect("open other");
    other
        .create(&new_proposal("X-1", "/r/.git"), T0)
        .expect("create");
    let occupied = queue.dump().unwrap();
    let state = StoredQueue {
        proposals: vec![
            row("PR-0001", PROJECT, false),
            row("PR-0002", PROJECT, false),
        ],
        events: vec![event(1, PROJECT), event(2, PROJECT)],
    };
    assert_eq!(
        queue.restore(&state).expect("restore"),
        Restore::Occupied(QueueCounts {
            proposals: 1,
            events: 1
        })
    );
    assert_eq!(queue.dump().unwrap(), occupied, "nothing inserted");
}

/// After a restore the next ID is the highest restored plus one (gaps
/// kept: `PR-0001`, `PR-0002`, `PR-0007` → `PR-0008`), the next `seq` the
/// highest plus one, and the restore logged no event. M: IDs renumbered
/// from `PR-0001`; an event of the restore's own.
#[test]
fn the_next_id_and_seq_follow_the_highest_restored() {
    let scratch = Scratch::new("qs-next");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let state = StoredQueue {
        proposals: vec![
            row("PR-0001", PROJECT, false),
            row("PR-0002", PROJECT, false),
            row("PR-0007", PROJECT, false),
        ],
        events: (1..=5).map(|seq| event(seq, PROJECT)).collect(),
    };
    assert_eq!(queue.restore(&state).expect("restore"), Restore::Restored);
    assert_eq!(queue.stored_rows().unwrap(), state, "rows as given");
    assert_eq!(
        queue.events().expect("events").len(),
        5,
        "no event of its own"
    );
    let created = queue
        .create(&new_proposal("X-1", "/r/.git"), T0)
        .expect("create");
    assert_eq!(created.id, "PR-0008");
    let events = queue.events().unwrap();
    assert_eq!(events.len(), 6);
    let last = events.last().unwrap();
    assert_eq!(last.seq, 6);
    assert_eq!(last.payload["id"], "PR-0008");
}

// ------------------------------------------- iteration 2: the git side

/// Scratch repositories: `main` (one commit on `main`) with the linked
/// worktrees `t1`, `t2`, `odd` (its path holds a space and a newline) and
/// `gone` (its directory deleted, never pruned); `bare.git`, a bare
/// repository of the same commit (pushed by path, no remote), with the
/// linked worktree `bwt`; `plain`, a directory no repository holds (the
/// scratch is the ceiling).
struct Repos {
    scratch: Scratch,
    git: Sandbox,
    main: PathBuf,
    t1: PathBuf,
    t2: PathBuf,
    odd: PathBuf,
    gone: PathBuf,
    bare: PathBuf,
    bwt: PathBuf,
    plain: PathBuf,
}

impl Repos {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let main = scratch.join("main");
        common::write(&main, "docs/a.md", "# A\n\nOne.\n");
        git.init(&main);
        let main = fs::canonicalize(&main).unwrap();
        git.add_all(&main);
        git.commit(&main, "first");
        let add = |from: &Path, name: &str, branch: &str| {
            let path = scratch.join(name);
            let text = path.to_str().unwrap();
            git.git(from, &["worktree", "add", "-q", "-b", branch, text, "main"]);
            path
        };
        let t1 = fs::canonicalize(add(&main, "t1", "t1")).unwrap();
        let t2 = fs::canonicalize(add(&main, "t2", "t2")).unwrap();
        let odd = fs::canonicalize(add(&main, "t 3\nnl", "t3")).unwrap();
        let gone = add(&main, "gone", "gone");
        fs::remove_dir_all(&gone).unwrap();
        let bare = scratch.join("bare.git");
        git.init_with(&bare, &["--bare"]);
        let bare = fs::canonicalize(&bare).unwrap();
        git.git(&main, &["push", "-q", bare.to_str().unwrap(), "main"]);
        let bwt = fs::canonicalize(add(&bare, "bwt", "b1")).unwrap();
        let plain = scratch.join("plain");
        fs::create_dir_all(&plain).unwrap();
        let plain = fs::canonicalize(&plain).unwrap();
        Self {
            scratch,
            git,
            main,
            t1,
            t2,
            odd,
            gone,
            bare,
            bwt,
            plain,
        }
    }

    /// The sandbox's variables plus `extra`.
    fn env(&self, extra: &[(&str, &OsStr)]) -> GitEnv {
        let mut env = GitEnv::new(self.scratch.path(), self.git.vars());
        for (name, value) in extra {
            env = env.with_var(*name, *value);
        }
        env
    }

    fn top(&self, dir: &Path, extra: &[(&str, &OsStr)]) -> Result<Option<PathBuf>, GitError> {
        WorktreeGit::new(dir, &self.env(extra))
            .expect("git runs")
            .top_if_repository()
    }

    fn listed(&self, dir: &Path) -> Vec<ListedWorktree> {
        WorktreeGit::new(dir, &self.env(&[]))
            .expect("git runs")
            .worktrees()
            .unwrap_or_else(|error| panic!("worktree list in {}: {error}", dir.display()))
    }
}

const GERMAN: [(&str, &str); 3] = [
    ("LC_ALL", "de_DE.UTF-8"),
    ("LANG", "de_DE.UTF-8"),
    ("LANGUAGE", "de"),
];

/// `Err(Failed)` with exit 128 and `words` in git's message.
fn failed_with(outcome: Result<Option<PathBuf>, GitError>, words: &str, context: &str) {
    match outcome {
        Err(GitError::Failed {
            code: Some(128),
            stderr,
            ..
        }) => assert!(stderr.contains(words), "{context}: {stderr}"),
        other => panic!("{context}: expected git's failure ({words}), got {other:?}"),
    }
}

/// `top_if_repository`: the canonical worktree top in a repository; `None`
/// only where git finds none, in any locale (`LC_ALL=C` for git's own
/// words; a translating git shows it matters); every other failure an
/// error. M: any exit 128 read as no repository; `LC_ALL=C` dropped.
#[test]
fn top_if_repository_is_none_only_where_git_finds_no_repository() {
    let repos = Repos::new("qs-top");
    for (dir, top) in [
        (&repos.main, &repos.main),
        (&repos.main.join("docs"), &repos.main),
        (&repos.t1, &repos.t1),
        (&repos.odd, &repos.odd),
        (&repos.bwt, &repos.bwt),
    ] {
        assert_eq!(
            repos.top(dir, &[]).expect("a repository"),
            Some(top.clone()),
            "{}",
            dir.display()
        );
    }
    assert_eq!(repos.top(&repos.plain, &[]).expect("no repository"), None);

    let german: Vec<(&str, &OsStr)> = GERMAN
        .iter()
        .map(|(name, value)| (*name, OsStr::new(value)))
        .collect();
    assert_eq!(
        repos.top(&repos.plain, &german).expect("no repository"),
        None,
        "the real git under a German locale"
    );
    let shim = repos.git.translating_git(&repos.scratch.join("shim"));
    let path = repos.git.path_with(&shim);
    let mut translated = german.clone();
    translated.push(("PATH", path.as_os_str()));
    let probe = repos
        .git
        .command(shim.join("git"), &repos.plain, &translated)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .unwrap();
    assert_eq!(probe.status.code(), Some(128));
    assert!(
        String::from_utf8_lossy(&probe.stderr).starts_with("fatal: Kein Git-Repository"),
        "the shim translates: {}",
        String::from_utf8_lossy(&probe.stderr)
    );
    assert_eq!(
        repos.top(&repos.plain, &translated).expect("no repository"),
        None,
        "a translating git: git's own words under LC_ALL=C"
    );
    assert_eq!(
        repos.top(&repos.main, &translated).expect("a repository"),
        Some(repos.main.clone())
    );

    // Errors, never `None`: git found something it would not read.
    let broken = repos.scratch.join("broken");
    common::write(
        &broken,
        ".git",
        format!("gitdir: {}\n", repos.scratch.join("nowhere").display()),
    );
    failed_with(
        repos.top(&broken, &[]),
        "not a git repository: ",
        "a broken .git file",
    );
    failed_with(
        repos.top(&broken, &translated),
        "not a git repository: ",
        "a broken .git file, translating git",
    );
    failed_with(
        repos.top(
            &repos.main,
            &[("GIT_TEST_ASSUME_DIFFERENT_OWNER", OsStr::new("1"))],
        ),
        "dubious ownership",
        "another owner",
    );
    failed_with(
        repos.top(&repos.main.join(".git"), &[]),
        "must be run in a work tree",
        "the git directory",
    );
    failed_with(
        repos.top(&repos.bare, &[]),
        "must be run in a work tree",
        "a bare repository",
    );
    failed_with(
        repos.top(&repos.scratch.join("missing"), &[]),
        "cannot change to",
        "a missing directory",
    );
}

/// `worktrees()`: the main worktree first (a bare repository itself,
/// `bare`), then every linked one in any order, the same list from each
/// of them; a deleted worktree git never pruned kept, as git prints it;
/// a path with a space and a newline whole. No repository: an error. M:
/// the bare flag never set; a worktree whose directory is gone skipped.
#[test]
fn worktrees_lists_the_main_one_first_the_bare_one_flagged_a_deleted_one_kept() {
    let repos = Repos::new("qs-list");
    let worktree = |path: &Path| ListedWorktree {
        path: path.to_path_buf(),
        bare: false,
    };
    let mut linked = vec![
        worktree(&repos.t1),
        worktree(&repos.t2),
        worktree(&repos.odd),
        worktree(&repos.gone),
    ];
    linked.sort_by(|a, b| a.path.cmp(&b.path));
    assert!(!repos.gone.exists());
    for from in [&repos.main, &repos.t1, &repos.odd, &repos.main.join("docs")] {
        let listed = repos.listed(from);
        assert_eq!(listed[0], worktree(&repos.main), "{}", from.display());
        let mut rest = listed[1..].to_vec();
        rest.sort_by(|a, b| a.path.cmp(&b.path));
        assert_eq!(rest, linked, "from {}", from.display());
    }
    let bare = vec![
        ListedWorktree {
            path: repos.bare.clone(),
            bare: true,
        },
        worktree(&repos.bwt),
    ];
    for from in [&repos.bwt, &repos.bare] {
        assert_eq!(repos.listed(from), bare, "from {}", from.display());
    }
    let outcome = WorktreeGit::new(&repos.plain, &repos.env(&[]))
        .expect("git runs")
        .worktrees();
    assert!(
        matches!(outcome, Err(GitError::Failed { .. })),
        "no repository: {outcome:?}"
    );
}
