//! docs/features/spec-verify.md ("Base", the store's base reader),
//! implementation iteration 2: `VerifyGit::read_files` reads each object
//! once and moves its bytes out at the OID's last use, copying them for
//! the earlier uses. A commit whose paths share one blob gives every such
//! path its bytes, in any order, a second read of the same session too.
//! docs/features/lock-rebase.md RD-7: `VerifyGit::read_blobs` reads
//! objects by OID alone (a blob's raw bytes, a missing object, an object
//! of another type), never by path or revision.
//! docs/features/impact-since.md "Command and guard" (the owner's changes
//! to `verify_rev`): `list_revision`, `spec impact --since`'s reader, reads
//! a range (its ends printed, exit 1) or a negation (`^<oid>` printed, exit
//! 0) as naming no commit (`None`), as an unknown name; `list_commit`
//! (`read_head`, `commit_config`) stays strict: the same answers are an
//! unreadable git, a run cause. A REV naming a commit lists it through
//! both; an unborn `HEAD` still reads as unborn, through both and through
//! the strict `HEAD` read `spec check --changed` runs.
//! Every git process runs in the sandbox of the CLI tests' `common::git`.

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::path::PathBuf;

use common::{Scratch, copy_dir, fixture, write};
use git::Sandbox;
use specengine_store::{CommitFile, GitEnv, ObjectBlob, VerifyGit, check_changed_with_notes};

const SAME: &[u8] = b"pub fn same() -> u32 {\n    1\n}\n";
const OTHER: &[u8] = b"pub fn other() -> u32 {\n    2\n}\n";

/// Three paths of one blob, one of another; `missing.rs` is no path of
/// the commit. M: the move without the copy for a shared OID.
#[test]
fn read_files_gives_every_path_of_a_shared_blob() {
    let scratch = Scratch::new("verify-git-shared");
    let git = Sandbox::new(scratch.path());
    let top = scratch.path().join("repo");
    git.init(&top);
    let top = fs::canonicalize(&top).expect("the repository");
    for path in ["a/one.rs", "b/two.rs", "c/three.rs"] {
        write(&top, path, SAME);
    }
    write(&top, "d/other.rs", OTHER);
    git.add_all(&top);
    git.commit(&top, "four files, two blobs");

    let env = GitEnv::new(&top, git.vars());
    let mut session = VerifyGit::open(&top, &env).expect("a git session");
    let listing = session
        .list_commit("HEAD")
        .expect("the listing")
        .expect("HEAD names a commit");
    let oid = |path: &str| listing.entry(path).expect("listed").oid;
    assert_eq!(oid("a/one.rs"), oid("b/two.rs"), "the scenario: one blob");
    assert_eq!(oid("a/one.rs"), oid("c/three.rs"));
    assert_ne!(oid("a/one.rs"), oid("d/other.rs"));

    let expected = |paths: &[&str]| {
        paths
            .iter()
            .map(|path| {
                let bytes = if path.starts_with('d') { OTHER } else { SAME };
                ((*path).to_owned(), CommitFile::Bytes(bytes.to_vec()))
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let files = session
        .read_files(
            &listing,
            [
                "c/three.rs",
                "a/one.rs",
                "missing.rs",
                "d/other.rs",
                "b/two.rs",
            ],
        )
        .expect("read");
    assert_eq!(
        files,
        expected(&["a/one.rs", "b/two.rs", "c/three.rs", "d/other.rs"])
    );
    // The same session again: the moved-out objects are read anew.
    let again = session
        .read_files(&listing, ["b/two.rs", "a/one.rs"])
        .expect("read again");
    assert_eq!(again, expected(&["a/one.rs", "b/two.rs"]));
    session.finish().expect("the session ends");
}

/// lock-rebase.md RD-7 (Q40, step 5): `read_blobs` gives each OID's
/// object by OID alone, one entry per distinct OID: a committed blob's
/// raw bytes (CRLF and `$Id$` kept: no filter under `core.autocrlf` or
/// `ident`), a blob no commit holds (`hash-object -w`), a commit's and a
/// tree's OID `Foreign`, an OID no object has `Missing`, and a revision or
/// a `<rev>:<path>` (no OID) `Missing`, never read as one; the session
/// ends cleanly, a second read of it too.
#[test]
fn read_blobs_reads_objects_by_oid_alone() {
    const RAW: &[u8] = b"// $Id$\r\npub fn raw() -> u32 {\r\n    3\r\n}\r\n";
    const LOOSE: &[u8] = b"pub fn loose() {}\n// \xff\n";
    let scratch = Scratch::new("verify-git-oids");
    let git = Sandbox::new(scratch.path());
    let top = scratch.path().join("repo");
    git.init(&top);
    let top = fs::canonicalize(&top).expect("the repository");
    write(&top, "a/one.rs", SAME);
    write(&top, "b/raw.rs", RAW);
    write(&top, ".gitattributes", b"*.rs -text\n");
    git.add_all(&top);
    git.commit(&top, "two blobs");
    write(&top, ".gitattributes", b"*.rs text ident\n");
    git.git(&top, &["config", "core.autocrlf", "true"]);

    let rev = |name: &str| git.git_text(&top, &["rev-parse", name]);
    let one = rev("HEAD:a/one.rs");
    let raw = rev("HEAD:b/raw.rs");
    let commit = rev("HEAD");
    let tree = rev("HEAD^{tree}");
    let loose = String::from_utf8(git.git_stdin(&top, &["hash-object", "-w", "--stdin"], LOOSE))
        .expect("an OID")
        .trim_end()
        .to_owned();
    let missing = "d".repeat(commit.len());
    assert_ne!(missing, commit);

    let env = GitEnv::new(&top, git.vars());
    let mut session = VerifyGit::open(&top, &env).expect("a git session");
    let asked = [
        raw.as_str(),
        "HEAD",
        one.as_str(),
        tree.as_str(),
        "HEAD:a/one.rs",
        missing.as_str(),
        commit.as_str(),
        loose.as_str(),
        one.as_str(),
    ];
    let read = session.read_blobs(asked).expect("read");
    let expected: std::collections::BTreeMap<String, ObjectBlob> = [
        (one.clone(), ObjectBlob::Bytes(SAME.to_vec())),
        (raw.clone(), ObjectBlob::Bytes(RAW.to_vec())),
        (loose.clone(), ObjectBlob::Bytes(LOOSE.to_vec())),
        (commit.clone(), ObjectBlob::Foreign),
        (tree.clone(), ObjectBlob::Foreign),
        (missing.clone(), ObjectBlob::Missing),
        ("HEAD".to_owned(), ObjectBlob::Missing),
        ("HEAD:a/one.rs".to_owned(), ObjectBlob::Missing),
    ]
    .into_iter()
    .collect();
    assert_eq!(read, expected);
    // The same session again: what was moved out is read anew.
    let again = session
        .read_blobs([one.as_str(), missing.as_str()])
        .expect("read again");
    assert_eq!(again.get(&one), Some(&ObjectBlob::Bytes(SAME.to_vec())));
    assert_eq!(again.get(&missing), Some(&ObjectBlob::Missing));
    session.finish().expect("the session ends");
}

/// lock-rebase.md RD-7: a SHA-256 repository's 64-hex OIDs read alike.
#[test]
fn read_blobs_reads_a_sha256_repository() {
    let scratch = Scratch::new("verify-git-sha256");
    let git = Sandbox::new(scratch.path());
    let top = scratch.path().join("repo");
    git.init_with(&top, &["--object-format=sha256"]);
    let top = fs::canonicalize(&top).expect("the repository");
    write(&top, "a/one.rs", SAME);
    git.add_all(&top);
    git.commit(&top, "one blob");
    let one = git.git_text(&top, &["rev-parse", "HEAD:a/one.rs"]);
    assert_eq!(one.len(), 64, "the scenario: SHA-256");
    let env = GitEnv::new(&top, git.vars());
    let mut session = VerifyGit::open(&top, &env).expect("a git session");
    let read = session.read_blobs([one.as_str()]).expect("read");
    assert_eq!(read.get(&one), Some(&ObjectBlob::Bytes(SAME.to_vec())));
    assert_eq!(read.len(), 1);
    session.finish().expect("the session ends");
}

/// A repository of two commits (so both ends of each range exist): its
/// scratch, sandbox, top, `HEAD`'s and `HEAD~1`'s OIDs. The premises: git
/// prints something for the two ranges (exit 1) and the negation (exit 0).
fn two_commits(label: &str) -> (Scratch, Sandbox, PathBuf, String, String) {
    let scratch = Scratch::new(label);
    let git = Sandbox::new(scratch.path());
    let top = scratch.path().join("repo");
    git.init(&top);
    let top = fs::canonicalize(&top).expect("the repository");
    write(&top, "a.rs", SAME);
    git.add_all(&top);
    git.commit(&top, "one");
    write(&top, "a.rs", OTHER);
    git.add_all(&top);
    git.commit(&top, "two");
    let head = git.git_text(&top, &["rev-parse", "HEAD"]);
    let parent = git.git_text(&top, &["rev-parse", "HEAD~1"]);
    for (spec, code) in [
        ("HEAD~1..HEAD^{commit}", 1),
        ("HEAD...HEAD~1^{commit}", 1),
        ("^HEAD^{commit}", 0),
    ] {
        let output = git.git_output(&top, &["rev-parse", "--verify", "-q", spec], &[]);
        assert_eq!(output.status.code(), Some(code), "premise: {spec}'s exit");
        assert!(!output.stdout.is_empty(), "premise: git prints for {spec}");
    }
    (scratch, git, top, head, parent)
}

/// The ranges and the negation [`two_commits`] makes git print for.
const NOT_ONE_COMMIT: [&str; 3] = ["HEAD~1..HEAD", "HEAD...HEAD~1", "^HEAD"];

/// docs/features/impact-since.md "Command and guard": over [`two_commits`],
/// `list_revision` (`spec impact --since`'s reader) gives `None` for
/// `HEAD~1..HEAD` and `HEAD...HEAD~1` (git prints their ends and exits 1),
/// `^HEAD` (git prints `^<oid>` and exits 0), a range past the root and an
/// unknown name; `HEAD~1` and `HEAD` list their commits (the label each
/// one's full OID); a REV starting with `-` is refused. M8: a range's exit
/// 1 with output read as unreadable (exit 2 in the CLI).
#[test]
fn list_revision_names_no_commit_for_a_range_or_a_negation() {
    let (_scratch, git, top, head, parent) = two_commits("verify-git-ranges");
    let env = GitEnv::new(&top, git.vars());
    let mut session = VerifyGit::open(&top, &env).expect("a git session");
    for rev in NOT_ONE_COMMIT.into_iter().chain(["HEAD~5..HEAD", "nope"]) {
        let listed = session
            .list_revision(rev)
            .unwrap_or_else(|cause| panic!("{rev}: no commit, not a failure: {cause:?}"));
        assert!(listed.is_none(), "{rev} names no commit");
    }
    for (rev, oid) in [("HEAD~1", &parent), ("HEAD", &head)] {
        let listing = session
            .list_revision(rev)
            .expect("the listing")
            .unwrap_or_else(|| panic!("{rev} names a commit"));
        assert_eq!(listing.label(), oid.as_str(), "{rev}: its commit");
        assert!(listing.entry("a.rs").is_some(), "{rev}: its tree");
    }
    assert!(
        session.list_revision("-x").is_err(),
        "a leading `-` refused"
    );
    session.finish().expect("the session ends");
}

/// The strict half (the owner's micro fix): over [`two_commits`],
/// `list_commit` (`read_head`, `commit_config`) reads the same ranges and
/// negation as git misbehaving: an error whose cause, at `.`, is "`git
/// rev-parse` gave output that could not be read" (a run cause, never
/// `None`); a range past the root (exit 1, nothing printed) and an unknown
/// name stay `None`; `HEAD~1` and `HEAD` list their commits. M:
/// `list_commit` reading as `list_revision` does.
#[test]
fn list_commit_reads_a_range_strictly() {
    let (_scratch, git, top, head, parent) = two_commits("verify-git-strict");
    let env = GitEnv::new(&top, git.vars());
    let mut session = VerifyGit::open(&top, &env).expect("a git session");
    for rev in NOT_ONE_COMMIT {
        let cause = match session.list_commit(rev) {
            Err(cause) => cause,
            Ok(listed) => panic!(
                "{rev}: strict, an unreadable answer, not {:?}",
                listed.map(|listing| listing.label().to_owned())
            ),
        };
        assert_eq!(cause.path, ".", "{rev}: the cause at `.`");
        assert_eq!(
            cause.message, "`git rev-parse` gave output that could not be read",
            "{rev}: the cause"
        );
    }
    for rev in ["HEAD~5..HEAD", "nope"] {
        let listed = session.list_commit(rev).expect("no commit, not a failure");
        assert!(listed.is_none(), "{rev} names no commit");
    }
    for (rev, oid) in [("HEAD~1", &parent), ("HEAD", &head)] {
        let listing = session
            .list_commit(rev)
            .expect("the listing")
            .unwrap_or_else(|| panic!("{rev} names a commit"));
        assert_eq!(listing.label(), oid.as_str(), "{rev}: its commit");
    }
    session.finish().expect("the session ends");
}

/// The same change keeps an unborn `HEAD` unborn: `list_commit("HEAD")`
/// and `list_revision("HEAD")` before the first commit are `None`, not a
/// failure; and `spec check
/// --changed`'s strict `HEAD` read (`check_changed_with_notes`) over a copy
/// of `fixtures/spec-a` never committed checks against an empty base, no
/// cause. M: an unborn `HEAD` read as unreadable.
#[test]
fn an_unborn_head_still_reads_as_unborn() {
    let scratch = Scratch::new("verify-git-unborn");
    let git = Sandbox::new(scratch.path());
    let top = scratch.path().join("repo");
    copy_dir(&fixture("spec-a"), &top);
    git.init(&top);
    let top = fs::canonicalize(&top).expect("the repository");
    let env = GitEnv::new(&top, git.vars());
    let mut session = VerifyGit::open(&top, &env).expect("a git session");
    let listed = session
        .list_commit("HEAD")
        .expect("an unborn HEAD is no failure");
    assert!(listed.is_none(), "an unborn HEAD names no commit");
    let listed = session
        .list_revision("HEAD")
        .expect("an unborn HEAD is no failure");
    assert!(listed.is_none(), "an unborn HEAD names no revision");
    session.finish().expect("the session ends");

    let checked = check_changed_with_notes(&top, None, None, &env, "2026-10-21");
    assert!(
        checked.report.cannot_check.is_empty(),
        "the strict HEAD read: unborn, an empty base: {:?}",
        checked.report.cannot_check
    );
}
