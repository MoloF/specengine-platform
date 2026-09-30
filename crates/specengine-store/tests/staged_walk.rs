//! AC-05 of docs/features/spec-cli-staged.md: `GitIndex`'s walk, store
//! level. An index built by `git update-index --index-info -z` with no
//! file on disk: symlinks (`120000`) and gitlinks (`160000`) under a root
//! skipped, `100755` listed; a written root holding only a gitlink, only a
//! symlink or nothing is missing, a root that is a gitlink is missing, a
//! root naming a `.md` blob is listed; a non-UTF-8 `.md` name and a
//! non-UTF-8 directory of two files are one `name-skipped` counting 2, a
//! non-UTF-8 dot-name is not counted. And for every entry APFS can hold,
//! the listing equals `WorkingTree`'s over the same tree on disk. Every
//! git process runs in the sandbox of the CLI tests' `common::git`.

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::Path;

use common::{Scratch, write};
use git::Sandbox;
use specengine_core::Paths;
use specengine_store::{GitEnv, GitIndex, Listing, Source, WorkingTree};

/// `git hash-object -w` of `bytes` (written to a file beside the
/// repository): the blob's OID.
fn blob(git: &Sandbox, top: &Path, scratch: &Scratch, bytes: &[u8]) -> String {
    let file = scratch.join(&format!("blob-{}", blake3::hash(bytes).to_hex()));
    fs::write(&file, bytes).unwrap();
    let oid = git.git_text(top, &["hash-object", "-w", file.to_str().unwrap()]);
    fs::remove_file(&file).unwrap();
    oid
}

fn paths(toml: &str) -> Paths {
    Paths::from_toml(toml).expect("a valid [paths]")
}

fn env(git: &Sandbox, top: &Path) -> GitEnv {
    GitEnv::new(top, git.vars())
}

#[test]
fn the_index_walk_skips_links_counts_non_utf8_once_and_finds_missing_roots() {
    let scratch = Scratch::new("staged-walk");
    let git = Sandbox::new(scratch.path());
    let top = scratch.path().join("repo");
    git.init(&top);
    let top = fs::canonicalize(&top).unwrap();

    let doc = blob(&git, &top, &scratch, b"# A document\n");
    let other = blob(&git, &top, &scratch, b"# Another document\n");
    let target = blob(&git, &top, &scratch, b"a.md");
    let commit = "0123456789abcdef0123456789abcdef01234567";
    let entries: Vec<(&str, &str, Vec<u8>)> = vec![
        ("100644", &doc, b"docs/a.md".to_vec()),
        ("100755", &other, b"docs/exec.md".to_vec()),
        ("120000", &target, b"docs/link.md".to_vec()),
        ("160000", commit, b"docs/sub".to_vec()),
        ("100644", &doc, b"docs/.hidden/x.md".to_vec()),
        ("100644", &doc, b"docs/\xff.md".to_vec()),
        ("100644", &doc, b"docs/\xfe-dir/one.md".to_vec()),
        ("100644", &other, b"docs/\xfe-dir/two.md".to_vec()),
        ("100644", &doc, b"docs/.\xfd.md".to_vec()),
        ("100644", &doc, b"docs/\xfc-notes.txt".to_vec()),
        ("100644", &doc, b"docs/notes.txt".to_vec()),
        ("120000", &target, b"only-link/l.md".to_vec()),
        ("160000", commit, b"only-gitlink/module".to_vec()),
        ("160000", commit, b"gitlink-root".to_vec()),
        ("100644", &doc, b"single.md".to_vec()),
        ("100644", &doc, b"outside/o.md".to_vec()),
    ];
    let mut input = Vec::new();
    for (mode, oid, path) in &entries {
        input.extend_from_slice(format!("{mode} {oid}\t").as_bytes());
        input.extend_from_slice(path);
        input.push(0);
    }
    git.git_stdin(&top, &["update-index", "-z", "--index-info"], &input);
    let staged = git.git(&top, &["ls-files", "-s", "-z"]);
    assert_eq!(
        staged
            .split(|&byte| byte == 0)
            .filter(|record| !record.is_empty())
            .count(),
        entries.len(),
        "every entry is staged"
    );
    let on_disk: Vec<_> = fs::read_dir(&top)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name())
        .collect();
    assert_eq!(on_disk, [".git"], "no file on disk");

    let config = paths(
        "[paths]\nroots = [\"docs\", \"only-link\", \"only-gitlink\", \"gitlink-root\", \"empty-root\", \"single.md\"]\n",
    );
    let index = GitIndex::open(&top, &config, &env(&git, &top)).expect("the index opens");
    let listing = index.list().unwrap();
    assert_eq!(
        listing,
        Listing {
            paths: vec![
                "docs/a.md".to_owned(),
                "docs/exec.md".to_owned(),
                "single.md".to_owned(),
            ],
            missing_roots: vec![
                "only-link".to_owned(),
                "only-gitlink".to_owned(),
                "gitlink-root".to_owned(),
                "empty-root".to_owned(),
            ],
            skipped_names: 2,
            unreadable_dirs: vec![],
        }
    );
    for path in &listing.paths {
        assert!(index.probe(path), "{path} probed");
    }
    for path in [
        "docs/link.md",
        "docs/.hidden/x.md",
        "outside/o.md",
        "docs/sub",
    ] {
        assert!(!index.probe(path), "{path} is not listed");
    }
    assert_eq!(index.read("docs/a.md").unwrap(), b"# A document\n");
    assert_eq!(index.read("docs/exec.md").unwrap(), b"# Another document\n");
    assert!(index.read("docs/link.md").is_err());
    assert_eq!(index.root(), top.as_path());
}

/// The same tree on disk and in the index (`git add -A`): the listings are
/// equal for every entry APFS can hold (no non-UTF-8 name, no submodule).
/// A root holding only a symlink is left out: the spec's AC-05 names it
/// missing in the index, while `WorkingTree` sees its directory (the
/// developer's divergence 1); the first test pins the spec's side.
#[test]
fn the_index_walk_equals_the_working_tree_walk() {
    let scratch = Scratch::new("staged-walk-parity");
    let git = Sandbox::new(scratch.path());
    let top = scratch.path().join("repo");
    git.init(&top);
    let top = fs::canonicalize(&top).unwrap();
    for path in [
        "docs/a.md",
        "docs/nested/deep/b.md",
        "docs/.hidden/x.md",
        "docs/.dot.md",
        "docs/dir.md/inner.md",
        "docs/notes.txt",
        "docs/UPPER.MD",
        "docs/excluded-1.md",
        "docs/keep/excluded-2.md",
        "docs/caf\u{e9}.md",
        "docs/tab\there.md",
        "single.md",
        "other/c.md",
        "outside/o.md",
    ] {
        write(&top, path, format!("# {path}\n"));
    }
    write(&top, "docs/exec.md", "# exec\n");
    fs::set_permissions(top.join("docs/exec.md"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::create_dir_all(top.join("only-link")).unwrap();
    symlink("../docs/a.md", top.join("docs/link.md")).unwrap();
    symlink("../docs/a.md", top.join("only-link/l.md")).unwrap();
    symlink("../outside", top.join("docs/linked-dir")).unwrap();
    git.add_all(&top);

    for toml in [
        "[paths]\nroots = [\"docs\", \"single.md\", \"missing\", \"other/c.md\"]\nexclude = [\"docs/excluded-*.md\", \"**/excluded-2.md\"]\n",
        "[paths]\nroots = [\"docs/nested\", \"docs\"]\n",
        "[paths]\nroots = [\"docs/.hidden\", \"docs/dir.md\", \"docs/linked-dir\"]\n",
        "",
    ] {
        let config = paths(toml);
        let tree = WorkingTree::new(&top, &config).unwrap().list().unwrap();
        let index = GitIndex::open(&top, &config, &env(&git, &top))
            .expect("the index opens")
            .list()
            .unwrap();
        assert_eq!(index, tree, "{toml}");
    }
}
