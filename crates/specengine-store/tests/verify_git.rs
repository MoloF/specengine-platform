//! docs/features/spec-verify.md ("Base", the store's base reader),
//! implementation iteration 2: `VerifyGit::read_files` reads each object
//! once and moves its bytes out at the OID's last use, copying them for
//! the earlier uses. A commit whose paths share one blob gives every such
//! path its bytes, in any order, a second read of the same session too.
//! docs/features/lock-rebase.md RD-7: `VerifyGit::read_blobs` reads
//! objects by OID alone (a blob's raw bytes, a missing object, an object
//! of another type), never by path or revision.
//! Every git process runs in the sandbox of the CLI tests' `common::git`.

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;

use common::{Scratch, write};
use git::Sandbox;
use specengine_store::{CommitFile, GitEnv, ObjectBlob, VerifyGit};

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
