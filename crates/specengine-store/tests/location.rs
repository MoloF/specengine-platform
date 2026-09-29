//! AC-04 of docs/features/spec-index.md (ADR-0003): a DB inside the worktree
//! it indexes — directly, through `..`, through a symlinked directory, as an
//! existing DB file symlinked from outside, or with the worktree named
//! through a symlink — is refused with `DbInsideWorktree` before anything is
//! created; a missing DB directory gives `DbDirMissing`; the fixtures stay
//! untouched (`git status -- fixtures/` and their bytes unchanged).

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use common::{Corpus, Scratch, fixture_bytes, fixtures_git_status};
use specengine_store::{SqliteIndex, StoreError};

/// Every entry under `dir`, relative, as `lstat` sees it (symlinks not
/// followed).
fn entries(dir: &Path) -> BTreeSet<PathBuf> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeSet<PathBuf>) {
        for entry in fs::read_dir(dir).expect("readable").flatten() {
            let path = entry.path();
            out.insert(path.strip_prefix(root).unwrap().to_path_buf());
            if entry.file_type().unwrap().is_dir() {
                walk(root, &path, out);
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(dir, dir, &mut out);
    out
}

fn assert_inside(db: &Path, root: &Path, scratch: &Scratch, context: &str) {
    let before = entries(scratch.path());
    let result = SqliteIndex::open(db, "demo", root);
    match result {
        Err(StoreError::DbInsideWorktree { .. }) => {}
        Err(other) => panic!("{context}: expected DbInsideWorktree, got {other:?}"),
        Ok(_) => panic!("{context}: a DB inside the worktree was opened"),
    }
    assert_eq!(
        entries(scratch.path()),
        before,
        "{context}: the refusal created or removed something"
    );
}

#[test]
fn a_db_inside_the_worktree_is_refused_before_anything_is_created() {
    let before_status = fixtures_git_status();
    let before_bytes = fixture_bytes("spec-a");
    let scratch = Scratch::new("location");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let root = corpus.root.clone();
    fs::create_dir_all(scratch.join("outside")).unwrap();

    // Directly in the root, and in a subdirectory.
    assert_inside(&root.join("index.db"), &root, &scratch, "direct");
    assert_inside(
        &root.join("docs/spec/index.db"),
        &root,
        &scratch,
        "subdirectory",
    );
    // Through `..`: the text starts outside, the directory resolves inside.
    assert_inside(
        &scratch.join("outside/../wt/index.db"),
        &root,
        &scratch,
        "through ..",
    );
    // Through `..` from inside a subdirectory back into the root.
    assert_inside(
        &root.join("docs/spec/../index.db"),
        &root,
        &scratch,
        "through .. inside the worktree",
    );
    // Through a symlinked directory outside that points into the worktree.
    symlink(root.join("docs"), scratch.join("outside/docs-link")).unwrap();
    assert_inside(
        &scratch.join("outside/docs-link/index.db"),
        &root,
        &scratch,
        "through a symlinked directory",
    );
    // An existing DB file outside that is a symlink to a file inside.
    fs::write(root.join("docs/stolen.db"), b"").unwrap();
    symlink(
        root.join("docs/stolen.db"),
        scratch.join("outside/stolen.db"),
    )
    .unwrap();
    assert_inside(
        &scratch.join("outside/stolen.db"),
        &root,
        &scratch,
        "an existing DB file symlinked into the worktree",
    );
    fs::remove_file(root.join("docs/stolen.db")).unwrap();
    // The worktree named through a symlink; the DB given by its real path.
    symlink(&root, scratch.join("wt-link")).unwrap();
    assert_inside(
        &root.join("index.db"),
        &scratch.join("wt-link"),
        &scratch,
        "the worktree named through a symlink",
    );

    // No `index.db` anywhere in the worktree.
    let leaked: Vec<PathBuf> = entries(&root)
        .into_iter()
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().contains("index.db"))
        })
        .collect();
    assert!(
        leaked.is_empty(),
        "DB files inside the worktree: {leaked:?}"
    );

    // A missing DB directory: `DbDirMissing`, nothing created.
    let before = entries(scratch.path());
    match SqliteIndex::open(scratch.join("missing/index.db"), "demo", &root) {
        Err(StoreError::DbDirMissing { .. }) => {}
        Err(other) => panic!("missing directory: expected DbDirMissing, got {other:?}"),
        Ok(_) => panic!("missing directory: the DB was opened"),
    }
    assert_eq!(
        entries(scratch.path()),
        before,
        "missing directory: something was created"
    );

    // Control: outside the worktree the DB opens (so the refusals above are
    // not a refusal of everything).
    let outside = scratch.join("outside/index.db");
    SqliteIndex::open(&outside, "demo", &root).expect("a DB outside the worktree opens");
    assert!(outside.is_file(), "the DB outside was created");

    // The fixtures are untouched: `git status` is what it was (empty on a
    // committed tree), and no byte of the copied fixture changed (a second
    // write to an already modified file would not show in `git status`).
    assert_eq!(
        fixtures_git_status(),
        before_status,
        "git status -- fixtures/ changed"
    );
    assert_eq!(
        fixture_bytes("spec-a"),
        before_bytes,
        "fixtures/spec-a changed"
    );
}
