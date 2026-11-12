//! docs/features/direction-catalog.md (RM-0106) "Acceptance criteria", the
//! store's half of DC-4 and DC-8: `WorktreeGit::last_commits` on scratch
//! repositories built here, every commit with a named author and committer
//! and their dates, every git process in the sandbox of the CLI tests'
//! `common::git` (a logging `git` shim first on its `PATH` where the
//! commands are counted).
//!
//! - Each path's newest commit of `HEAD`'s first-parent line whose diff
//!   against its first parent names it, a root commit's files found there;
//!   paths absent at `HEAD` (never committed, deleted) in `absent`, never
//!   walked for; `walked` the commits read, the walk stopping once every
//!   present path is found. M: the oldest commit taken.
//! - `max` bounds the walk: the rest `unreached`. M: an unbounded walk.
//! - A merge is the change of a file its side branch changed; a side
//!   branch's edit and revert leave the first-parent line's own commit. M:
//!   `--first-parent` dropped.
//! - A shallow clone's boundary commit names no file: those it would are
//!   `unreached`.
//! - Three commands, one each, `GIT_NO_LAZY_FETCH=1`; an unborn `HEAD` or
//!   only absent paths: every path `absent`, no `log` run. M: a
//!   `file_log` per path.
//! - A walk stopped early over a long history returns at once, leaving no
//!   child behind (no zombie, no git still running, no stalled pipe).
//! - Paths with spaces and non-ASCII letters are read as written.

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use common::{Scratch, write};
use git::Sandbox;
use specengine_store::{GitEnv, LastCommits, LoggedCommit, WorktreeGit};

/// A person as git writes one.
#[derive(Clone, Copy)]
struct Ident {
    name: &'static str,
    email: &'static str,
}

const BOB: Ident = Ident {
    name: "Bob Hand",
    email: "bob@example.org",
};
const CLEO: Ident = Ident {
    name: "Cleo Commit",
    email: "cleo@example.org",
};
const SID: Ident = Ident {
    name: "Sid Side",
    email: "sid@example.org",
};
const MIA: Ident = Ident {
    name: "Mia Merge",
    email: "mia@example.org",
};

/// A scratch repository on `main`.
struct Repo {
    scratch: Scratch,
    git: Sandbox,
    root: PathBuf,
    messages: std::cell::Cell<usize>,
}

/// `YYYY-MM-DDTHH:MM:SSZ` as git's date.
fn git_date(utc: &str) -> String {
    format!("{}+0000", utc.trim_end_matches('Z'))
}

/// Seconds since 1970 of a 2026 March stamp `2026-03-DDT10:00:00Z`.
fn march(day: i64) -> i64 {
    // 2026-03-01T00:00:00Z.
    1_772_323_200 + (day - 1) * 86_400 + 10 * 3_600
}

impl Repo {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(&format!("lc-{label}"));
        let git = Sandbox::new(scratch.path());
        let root = scratch.join("repo");
        git.init(&root);
        let root = fs::canonicalize(&root).expect("canonical");
        Self {
            scratch,
            git,
            root,
            messages: std::cell::Cell::new(0),
        }
    }

    fn put(&self, relative: &str, text: &str) {
        write(&self.root, relative, text);
    }

    /// `git add -A`, then a commit by `author` committed by `committer` at
    /// `2026-03-<day>T10:00:00Z` (authored a day earlier): its OID.
    fn commit(&self, author: Ident, committer: Ident, day: i64, message: &str) -> String {
        self.git.add_all(&self.root);
        let n = self.messages.get();
        self.messages.set(n + 1);
        let file = self.scratch.join(&format!("message-{n}.txt"));
        fs::write(&file, message).expect("a message file");
        let stamp = self.stamp(author, committer, day);
        self.git
            .git_env(&self.root, &self.args(&file), &pairs(&stamp));
        self.head()
    }

    fn args<'a>(&self, file: &'a Path) -> Vec<&'a str> {
        vec![
            "commit",
            "-q",
            "--no-verify",
            "--allow-empty",
            "--cleanup=verbatim",
            "-F",
            file.to_str().expect("UTF-8"),
        ]
    }

    /// The identity variables of a commit by `author` committed by
    /// `committer` at `2026-03-<day>T10:00:00Z`, authored in February.
    fn stamp(&self, author: Ident, committer: Ident, day: i64) -> Vec<(&'static str, OsString)> {
        let authored = git_date(&format!("2026-02-{:02}T08:00:00Z", day.min(28)));
        let committed = git_date(&format!("2026-03-{day:02}T10:00:00Z"));
        vec![
            ("GIT_AUTHOR_NAME", author.name.into()),
            ("GIT_AUTHOR_EMAIL", author.email.into()),
            ("GIT_AUTHOR_DATE", authored.into()),
            ("GIT_COMMITTER_NAME", committer.name.into()),
            ("GIT_COMMITTER_EMAIL", committer.email.into()),
            ("GIT_COMMITTER_DATE", committed.into()),
        ]
    }

    fn head(&self) -> String {
        self.git.git_text(&self.root, &["rev-parse", "HEAD"])
    }

    /// The handle in `dir` with the sandbox's variables and `extra` over
    /// them.
    fn handle_in(&self, dir: &Path, extra: &[(&str, &OsStr)]) -> WorktreeGit {
        let mut vars: Vec<(OsString, OsString)> = self.git.vars();
        for (name, value) in extra {
            vars.retain(|(known, _)| known != OsStr::new(name));
            vars.push((OsString::from(name), value.to_os_string()));
        }
        WorktreeGit::new(dir, &GitEnv::new(dir, vars)).expect("a handle")
    }

    fn handle(&self) -> WorktreeGit {
        self.handle_in(&self.root, &[])
    }
}

/// `stamp`'s variables as `Sandbox::git_env` takes them.
fn pairs<'a>(stamp: &'a [(&'static str, OsString)]) -> Vec<(&'static str, &'a OsStr)> {
    stamp
        .iter()
        .map(|(name, value)| (*name, value.as_os_str()))
        .collect()
}

fn paths(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|path| (*path).to_owned()).collect()
}

/// The commit a path was found at.
fn found_at(answer: &LastCommits, path: &str) -> String {
    answer
        .found
        .get(path)
        .unwrap_or_else(|| panic!("{path} not found: {answer:?}"))
        .oid
        .clone()
}

/// Four commits: (0) the root adding `a.md`, `b.md`, `c.md`, `gone.md`;
/// (1) Bob on `a.md`, committed by Cleo; (2) an apply on `b.md`, deleting
/// `gone.md`; (3) Cleo on `x.txt` alone. Their OIDs.
fn four(repo: &Repo) -> [String; 4] {
    for name in ["a", "b", "c", "gone"] {
        repo.put(&format!("docs/{name}.md"), &format!("# {name}\n"));
    }
    let c0 = repo.commit(CLEO, CLEO, 1, "the root\n");
    repo.put("docs/a.md", "# a\n\nEdited.\n");
    let c1 = repo.commit(BOB, CLEO, 2, "Edit a\n");
    repo.put("docs/b.md", "# b\n\nApplied.\n");
    fs::remove_file(repo.root.join("docs/gone.md")).expect("removed");
    let c2 = repo.commit(
        CLEO,
        CLEO,
        3,
        "spec: apply PR-0001\n\nWhy.\n\nProposal: PR-0001\nDecided-by: Ann Owner \
         <ann@example.org>\n",
    );
    repo.put("x.txt", "x\n");
    let c3 = repo.commit(CLEO, CLEO, 4, "Edit x\n");
    [c0, c1, c2, c3]
}

/// DC-4: each path's newest commit; the root's files found at the root;
/// a deleted and a never-committed path `absent`; the walk read all four
/// (one file is the root's); each found commit carries its author, its
/// committer time, its raw message and no `change`. M: the oldest commit
/// taken.
#[test]
fn dc4_each_path_its_newest_commit() {
    let repo = Repo::new("newest");
    let [c0, c1, c2, _] = four(&repo);
    let answer = repo
        .handle()
        .last_commits(
            &paths(&[
                "docs/a.md",
                "docs/b.md",
                "docs/c.md",
                "docs/gone.md",
                "docs/never.md",
            ]),
            100,
        )
        .expect("read");
    assert_eq!(found_at(&answer, "docs/a.md"), c1);
    assert_eq!(found_at(&answer, "docs/b.md"), c2);
    assert_eq!(found_at(&answer, "docs/c.md"), c0);
    assert_eq!(answer.absent, paths(&["docs/gone.md", "docs/never.md"]));
    assert_eq!(answer.unreached, BTreeSet::new());
    assert_eq!(answer.walked, 4);
    assert_eq!((answer.capped, answer.shallow), (false, false));
    assert_eq!(
        answer.found["docs/a.md"],
        LoggedCommit {
            oid: c1.clone(),
            author_name: BOB.name.to_owned(),
            author_email: BOB.email.to_owned(),
            committed_at: march(2),
            message: "Edit a\n".to_owned(),
            change: None,
        }
    );
    assert!(
        answer.found["docs/b.md"]
            .message
            .ends_with("Decided-by: Ann Owner <ann@example.org>\n"),
        "the raw message: {:?}",
        answer.found["docs/b.md"].message
    );
}

/// DC-4: the walk stops once each present path is found: `b.md` alone
/// reads the two newest commits.
#[test]
fn dc4_the_walk_stops_once_every_path_is_found() {
    let repo = Repo::new("stops");
    let [_, _, c2, _] = four(&repo);
    let answer = repo
        .handle()
        .last_commits(&paths(&["docs/b.md"]), 100)
        .expect("read");
    assert_eq!(found_at(&answer, "docs/b.md"), c2);
    assert_eq!(answer.walked, 2);
}

/// DC-4 (the scan cap): `max` 2 reads the two newest commits: `b.md`
/// found, `a.md` and `c.md` unreached; `max` 3 finds `a.md` too; `max` 0
/// reads none.
#[test]
fn dc4_max_bounds_the_walk() {
    let repo = Repo::new("max");
    let [_, c1, c2, _] = four(&repo);
    let asked = paths(&["docs/a.md", "docs/b.md", "docs/c.md"]);
    let two = repo.handle().last_commits(&asked, 2).expect("read");
    assert_eq!(found_at(&two, "docs/b.md"), c2);
    assert_eq!(two.unreached, paths(&["docs/a.md", "docs/c.md"]));
    assert_eq!(two.walked, 2);
    assert!(two.capped && !two.shallow, "{two:?}");
    let three = repo.handle().last_commits(&asked, 3).expect("read");
    assert_eq!(found_at(&three, "docs/a.md"), c1);
    assert_eq!(three.unreached, paths(&["docs/c.md"]));
    assert_eq!(three.walked, 3);
    assert!(three.capped, "{three:?}");
    // A history of exactly `max` commits ends: the root's file found, not
    // capped.
    let four = repo.handle().last_commits(&asked, 4).expect("read");
    assert_eq!(four.unreached, BTreeSet::new(), "{four:?}");
    assert_eq!((four.walked, four.capped), (4, false), "{four:?}");
    let none = repo.handle().last_commits(&asked, 0).expect("read");
    assert!(none.found.is_empty(), "{none:?}");
    assert_eq!(none.unreached, asked);
    assert_eq!(none.walked, 0);
}

/// DC-4: `side` from the root edits `s.md`, then edits and reverts
/// `r.md`; `main` edits `r.md`, then merges `side` (authored by Mia,
/// committed by Cleo). `s.md`'s commit is the merge; `r.md`'s is `main`'s
/// own edit. M: `--first-parent` dropped.
#[test]
fn dc4_the_first_parent_line_and_its_merge() {
    let repo = Repo::new("merge");
    repo.put("s.md", "# s\n");
    repo.put("r.md", "# r\n");
    repo.commit(CLEO, CLEO, 1, "the root\n");
    repo.git.git(&repo.root, &["branch", "side"]);
    repo.put("r.md", "# r\n\nMain.\n");
    let main_edit = repo.commit(BOB, BOB, 2, "Main edits r\n");
    repo.git.git(&repo.root, &["checkout", "-q", "side"]);
    repo.put("s.md", "# s\n\nSide.\n");
    repo.commit(SID, SID, 3, "Side edits s\n");
    repo.put("r.md", "# r\n\nSide.\n");
    repo.commit(SID, SID, 4, "Side edits r\n");
    repo.put("r.md", "# r\n");
    let revert = repo.commit(SID, SID, 5, "Side reverts r\n");
    repo.git.git(&repo.root, &["checkout", "-q", "main"]);
    repo.git.git_env(
        &repo.root,
        &[
            "merge",
            "-q",
            "--no-ff",
            "--no-edit",
            "-m",
            "Merge side",
            "side",
        ],
        &pairs(&repo.stamp(MIA, CLEO, 6)),
    );
    let merge = repo.head();
    let answer = repo
        .handle()
        .last_commits(&paths(&["s.md", "r.md"]), 100)
        .expect("read");
    assert_eq!(found_at(&answer, "s.md"), merge, "the merge");
    assert_eq!(answer.found["s.md"].author_name, MIA.name);
    assert_eq!(answer.found["s.md"].committed_at, march(6));
    assert_eq!(
        found_at(&answer, "r.md"),
        main_edit,
        "main's own, not the side's revert {revert}"
    );
}

/// A `--depth 2` clone: the tip's file found; the boundary's files (the
/// boundary has no parent in the clone) `unreached`, never the boundary.
#[test]
fn dc4_a_shallow_boundary_names_no_file() {
    let repo = Repo::new("shallow");
    let [_, _, c2, c3] = four(&repo);
    repo.put("docs/a.md", "# a\n\nAgain.\n");
    let c4 = repo.commit(BOB, BOB, 5, "Edit a again\n");
    let clone = repo.scratch.join("clone");
    let url = format!("file://{}", repo.root.display());
    repo.git.git(
        repo.scratch.path(),
        &[
            "clone",
            "-q",
            "--depth",
            "2",
            &url,
            clone.to_str().expect("UTF-8"),
        ],
    );
    let clone = fs::canonicalize(clone).expect("canonical");
    let answer = repo
        .handle_in(&clone, &[])
        .last_commits(&paths(&["docs/a.md", "docs/b.md", "x.txt"]), 100)
        .expect("read");
    assert_eq!(found_at(&answer, "docs/a.md"), c4);
    assert_eq!(
        answer.unreached,
        paths(&["docs/b.md", "x.txt"]),
        "the boundary {c3} names nothing; b.md's {c2} is beyond it: {answer:?}"
    );
    assert!(answer.shallow && !answer.capped, "{answer:?}");
    // A full clone: no boundary.
    let full = repo.scratch.join("full");
    repo.git.git(
        repo.scratch.path(),
        &["clone", "-q", &url, full.to_str().expect("UTF-8")],
    );
    let full = fs::canonicalize(full).expect("canonical");
    let whole = repo
        .handle_in(&full, &[])
        .last_commits(&paths(&["docs/a.md", "docs/b.md", "x.txt"]), 100)
        .expect("read");
    assert_eq!(whole.unreached, BTreeSet::new(), "{whole:?}");
    assert_eq!(found_at(&whole, "docs/b.md"), c2);
    assert_eq!(found_at(&whole, "x.txt"), c3);
    assert!(!whole.shallow && !whole.capped, "{whole:?}");
}

// ------------------------------------------------------------- the shim

/// A `git` shim (in `<scratch>/<dir>`) logging `$GIT_NO_LAZY_FETCH|<args>`
/// and running the real git: its directory and its log.
fn shim(repo: &Repo, dir: &str) -> (PathBuf, PathBuf) {
    let bin = repo.scratch.join(dir);
    fs::create_dir_all(&bin).expect("the shim's directory");
    let log = bin.join("args.log");
    let script = format!(
        "#!/bin/sh\nprintf '%s|%s\\n' \"$GIT_NO_LAZY_FETCH\" \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
        log.display(),
        repo.git.git_program().display()
    );
    let path = bin.join("git");
    fs::write(&path, script).expect("the shim");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    (bin, log)
}

fn path_with(repo: &Repo, first: &Path) -> OsString {
    let mut dirs = vec![first.to_path_buf()];
    dirs.extend(std::env::split_paths(
        repo.git.var("PATH").expect("the sandbox's PATH"),
    ));
    std::env::join_paths(dirs).expect("a PATH")
}

/// The logged lines without `rev-parse` ones (the handle's own lookup).
fn commands(log: &Path) -> Vec<String> {
    fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.contains(" rev-parse ") && !line.contains("|rev-parse "))
        .map(str::to_owned)
        .collect()
}

/// DC-8: one call over three paths runs one presence read (`ls-tree -r
/// --name-only` since iteration 2), one walking `log --first-parent`, one
/// `log --no-walk`, each with
/// `GIT_NO_LAZY_FETCH=1`. M: a `file_log` per path.
#[test]
fn dc8_three_commands_one_each() {
    let repo = Repo::new("three");
    four(&repo);
    let (bin, log) = shim(&repo, "shim");
    let path = path_with(&repo, &bin);
    let handle = repo.handle_in(&repo.root, &[("PATH", path.as_os_str())]);
    let _ = fs::remove_file(&log);
    let answer = handle
        .last_commits(&paths(&["docs/a.md", "docs/b.md", "docs/c.md"]), 100)
        .expect("read");
    assert_eq!(answer.found.len(), 3);
    let lines = commands(&log);
    assert_eq!(lines.len(), 3, "{lines:#?}");
    // Iteration 2: presence from `HEAD`'s tree, one `ls-tree -r
    // --name-only`, where the spec names a `cat-file --batch-check` session.
    assert!(
        lines[0].contains(" ls-tree ") && lines[0].contains("--name-only"),
        "{lines:#?}"
    );
    assert!(
        lines[1].contains(" log ") && lines[1].contains("--first-parent"),
        "{lines:#?}"
    );
    assert!(
        lines[2].contains(" log ") && lines[2].contains("--no-walk"),
        "{lines:#?}"
    );
    for line in &lines {
        assert!(line.starts_with("1|"), "GIT_NO_LAZY_FETCH=1: {line}");
    }
}

/// DC-4: an unborn `HEAD`: every path `absent`, no `log`; on a born one,
/// only absent paths run no `log` either.
#[test]
fn dc4_unborn_head_and_absent_paths_run_no_log() {
    let repo = Repo::new("unborn");
    repo.put("docs/a.md", "# a\n");
    let (bin, log) = shim(&repo, "shim");
    let path = path_with(&repo, &bin);
    let handle = repo.handle_in(&repo.root, &[("PATH", path.as_os_str())]);
    let asked = paths(&["docs/a.md", "docs/b.md"]);
    let answer = handle.last_commits(&asked, 100).expect("read");
    assert_eq!(
        answer,
        LastCommits {
            absent: asked.clone(),
            ..LastCommits::default()
        }
    );
    let lines = commands(&log);
    assert!(
        lines.iter().all(|line| !line.contains(" log ")),
        "no log: {lines:#?}"
    );
    repo.commit(CLEO, CLEO, 1, "the root\n");
    let _ = fs::remove_file(&log);
    let answer = handle
        .last_commits(&paths(&["docs/never.md"]), 100)
        .expect("read");
    assert_eq!(answer.absent, paths(&["docs/never.md"]));
    assert_eq!(answer.walked, 0);
    let lines = commands(&log);
    assert!(
        lines.iter().all(|line| !line.contains(" log ")),
        "no log: {lines:#?}"
    );
}

/// Paths with a space and non-ASCII letters are asked and named as written.
#[test]
fn dc4_paths_as_written() {
    let repo = Repo::new("names");
    let spaced = "docs/the storm.md";
    let accented = "docs/caf\u{e9}-\u{f1}.md";
    repo.put(spaced, "# s\n");
    repo.put(accented, "# c\n");
    let root = repo.commit(CLEO, CLEO, 1, "the root\n");
    repo.put(spaced, "# s\n\nRain.\n");
    let edit = repo.commit(BOB, BOB, 2, "Rain\n");
    let answer = repo
        .handle()
        .last_commits(&paths(&[spaced, accented]), 100)
        .expect("read");
    assert_eq!(found_at(&answer, spaced), edit);
    assert_eq!(found_at(&answer, accented), root);
}

/// Every child process of this test process: `(pid, state, command)`.
fn children() -> Vec<(String, String, String)> {
    let output = Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid=,stat=,comm="])
        .output()
        .expect("ps runs");
    let me = std::process::id().to_string();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.to_owned();
            let ppid = fields.next()?;
            let stat = fields.next()?.to_owned();
            let comm = fields.collect::<Vec<_>>().join(" ");
            (ppid == me).then_some((pid, stat, comm))
        })
        .collect()
}

/// The risk of a killed walk: 6 000 commits on top of the root, each
/// naming three long file names (over a megabyte of walk output, far past
/// a pipe's buffer); a path changed at the tip is found after one commit,
/// the call returns within the watchdog, and no git child of this process
/// is left, running or a zombie (any child of state `Z` counts); then a
/// path only the root names walks the whole history and returns too.
#[test]
fn a_walk_stopped_early_leaves_no_child() {
    let repo = Repo::new("killed");
    repo.put("docs/root.md", "# root\n");
    let root = repo.commit(CLEO, CLEO, 1, "the root\n");
    let long = "a-rather-long-directory-name-for-the-walk/".repeat(2);
    let mut stream = String::new();
    for n in 0..6_000_u32 {
        let message = format!("tick {n}\n");
        stream.push_str(&format!(
            "commit refs/heads/main\ncommitter Tick Tock <tick@example.org> {} +0000\ndata {}\n{}",
            1_780_000_000 + u64::from(n),
            message.len(),
            message
        ));
        if n == 0 {
            stream.push_str(&format!("from {root}\n"));
        }
        for file in 0..3 {
            let data = format!("{n}\n");
            stream.push_str(&format!(
                "M 100644 inline {long}tick-file-number-{file}.txt\ndata {}\n{data}\n",
                data.len()
            ));
        }
    }
    repo.git
        .git_stdin(&repo.root, &["fast-import", "--quiet"], stream.as_bytes());
    repo.git
        .git(&repo.root, &["reset", "-q", "--hard", "refs/heads/main"]);
    let tip_path = format!("{long}tick-file-number-1.txt");
    let tip = repo.head();
    let handle = repo.handle();
    let (sender, receiver) = mpsc::channel();
    let asked = paths(&[tip_path.as_str()]);
    let worker = std::thread::spawn(move || {
        let started = Instant::now();
        let answer = handle.last_commits(&asked, 20_000);
        let _ = sender.send((answer, started.elapsed()));
    });
    let (answer, took) = receiver
        .recv_timeout(Duration::from_secs(60))
        .expect("the stopped walk returns: no stalled pipe");
    worker.join().expect("the worker");
    let answer = answer.expect("read");
    assert_eq!(found_at(&answer, &tip_path), tip);
    assert_eq!(answer.walked, 1);
    let left: Vec<_> = children()
        .into_iter()
        .filter(|(_, stat, comm)| stat.contains('Z') || comm.contains("git"))
        .collect();
    assert_eq!(left, Vec::new(), "no git child left (took {took:?})");

    let started = Instant::now();
    let whole = repo
        .handle()
        .last_commits(&paths(&["docs/root.md"]), 20_000)
        .expect("read");
    assert_eq!(found_at(&whole, "docs/root.md"), root);
    assert_eq!(whole.walked, 6_001);
    let left: Vec<_> = children()
        .into_iter()
        .filter(|(_, stat, comm)| stat.contains('Z') || comm.contains("git"))
        .collect();
    assert_eq!(
        left,
        Vec::new(),
        "no git child left after a whole walk ({:?})",
        started.elapsed()
    );
}
