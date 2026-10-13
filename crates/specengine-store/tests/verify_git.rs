//! docs/features/spec-verify.md ("Base", the store's base reader),
//! implementation iteration 2: `VerifyGit::read_files` reads each object
//! once and moves its bytes out at the OID's last use, copying them for
//! the earlier uses. A commit whose paths share one blob gives every such
//! path its bytes, in any order, a second read of the same session too.
//! Every git process runs in the sandbox of the CLI tests' `common::git`.

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;

use common::{Scratch, write};
use git::Sandbox;
use specengine_store::{CommitFile, GitEnv, VerifyGit};

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
