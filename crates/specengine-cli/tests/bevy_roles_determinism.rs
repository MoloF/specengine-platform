//! docs/features/bevy-roles.md TE-3, the `index` half (the `symbols` half
//! is `bevy-roles-reads.md`'s), and TE-4 ("Rules and edge cases":
//! determinism over two roots, cwd, reverse creation and CRLF, nothing
//! following rowid; the data directory the only thing written).
//!
//! Over scratch copies of `fixtures/code-bevy`, each indexed in a `HOME` of
//! its own: `spec index` text (the `db <path>` line masked: one data
//! directory per `HOME`) and JSON (without `db`) byte for byte alike; a
//! copy indexed, then two files re-derived (their rows written after every
//! other file's) gives the same `roles` line and `code.roles`. TE-4 in a
//! scratch git repository: every read leaves `git status` empty and the
//! code files' (path, size, mtime, BLAKE3) kept.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod bevy_roles_common;
mod code_index_common;
mod common;

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::MetadataExt as _;
use std::path::Path;

use serde_json::Value;

use bevy_roles_common::{
    SLUG, code_paths_under, copy, copy_reversed, expected, mask_db, roles_line_of_run, without_db,
};
use code_index_common::{db, to_crlf};
use common::bundle::blake3_hex;
use common::git::Sandbox;
use common::{Run, Scratch, fixture, spec, write};

/// `spec index` and `spec --json index` in `cwd` with `extra` before the
/// command, exit 0: the masked text and the JSON without `db`.
fn index_both(home: &Path, cwd: &Path, extra: &[&str]) -> (String, Value) {
    let mut args: Vec<&str> = extra.to_vec();
    args.push("index");
    let text = spec(home, cwd, &args);
    text.code(0);
    let mut args: Vec<&str> = vec!["--json"];
    args.extend_from_slice(extra);
    args.push("index");
    let json = spec(home, cwd, &args);
    json.code(0);
    (mask_db(&text.stdout), without_db(json.json()))
}

/// TE-3. M: rowid order (also a stored absolute path, CR kept). The same
/// copy twice; a copy made in reverse creation order under another name,
/// indexed with `--root` from another cwd; the copy indexed from a
/// subdirectory; a copy whose code files are all CRLF: `spec index` text
/// and JSON byte for byte alike, the `roles` line `expected.json`'s. A
/// copy indexed, then `lib.rs` and `herd.rs` re-derived: the same `roles`
/// line and `code.roles`.
#[test]
fn index_roles_are_byte_identical_across_roots_and_order() {
    let want = expected();
    let scratch = Scratch::new("br-determinism");
    let first = copy(&scratch, "one/copy");
    let second = scratch.dir("two/elsewhere-else");
    copy_reversed(&fixture("code-bevy"), &second);
    let crlf = copy(&scratch, "three/crlf");
    for path in code_paths_under(&crlf) {
        to_crlf(&crlf, &path);
    }
    let cwd = scratch.dir("somewhere");
    let second_text = second.to_str().expect("a UTF-8 scratch").to_owned();
    let base = index_both(&scratch.home("a"), &first, &[]);
    assert!(
        base.0
            .lines()
            .any(|line| line == want["index_roles_line"].as_str().unwrap()),
        "{}",
        base.0
    );
    assert_eq!(base.1["code"]["roles"], want["code_roles"], "{}", base.1);
    let variants = [
        (
            "the same copy again",
            index_both(&scratch.home("b"), &first, &[]),
        ),
        (
            "reverse creation, --root from another cwd",
            index_both(&scratch.home("c"), &cwd, &["--root", second_text.as_str()]),
        ),
        (
            "from a subdirectory",
            index_both(&scratch.home("d"), &first.join("crates/farm/src"), &[]),
        ),
        ("CRLF", index_both(&scratch.home("e"), &crlf, &[])),
    ];
    for (label, (text, json)) in variants {
        assert_eq!(text, base.0, "{label}: text");
        assert_eq!(json, base.1, "{label}: JSON");
    }

    let later = copy(&scratch, "four/later");
    let home = scratch.home("later");
    spec(&home, &later, &["index"]).code(0);
    for path in ["crates/farm/src/lib.rs", "crates/farm/src/herd.rs"] {
        let mut bytes = fs::read(later.join(path)).expect("read");
        bytes.extend_from_slice(b"// appended\n");
        fs::write(later.join(path), bytes).expect("write");
    }
    let run = spec(&home, &later, &["index"]);
    run.code(0);
    assert_eq!(roles_line_of_run(&run), want["index_roles_line"]);
    let json = spec(&home, &later, &["--json", "index"]).json();
    assert_eq!(json["code"]["roles"], want["code_roles"], "{json}");
}

/// `(path, size, mtime ns, BLAKE3)` of every code file and manifest under
/// `root`, `.git` left out.
fn code_files(root: &Path) -> Vec<(String, u64, i128, String)> {
    let mut out = Vec::new();
    for (path, bytes) in common::snapshot(root) {
        let Some(bytes) = bytes else { continue };
        if path.starts_with(".git") {
            continue;
        }
        if path.ends_with(".rs") || path.ends_with(".ron") || path.ends_with("Cargo.toml") {
            let metadata = fs::symlink_metadata(root.join(&path)).expect("metadata");
            let mtime =
                i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec());
            out.push((path, metadata.len(), mtime, blake3_hex(&bytes)));
        }
    }
    out
}

fn worktree(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    common::snapshot(root)
        .into_iter()
        .filter(|(path, _)| path != ".git" && !path.starts_with(".git/"))
        .collect()
}

fn ran(run: &Run, codes: &[i32]) {
    assert!(codes.contains(&run.code), "{:?}:\n{}", run.args, run.show());
}

/// TE-4. M: a cache file under the root. In a git repository of code-bevy,
/// `spec index` (text, JSON, `--full`), `refs`, `symbols`, `unmapped` and
/// an index refused for an unknown framework (`--config` outside the root)
/// leave `git status --porcelain --ignored` empty, every code file's
/// (path, size, mtime, BLAKE3) and the whole worktree unchanged; the
/// index is in the data directory.
#[test]
fn roles_write_only_the_data_directory() {
    let scratch = Scratch::new("br-read-only");
    let sandbox = Sandbox::new(scratch.path());
    let repo = copy(&scratch, "repo");
    sandbox.init(&repo);
    sandbox.add_all(&repo);
    sandbox.commit(&repo, "the fixture");
    let status = || {
        sandbox.git_text(
            &repo,
            &[
                "--no-optional-locks",
                "status",
                "--porcelain",
                "--untracked-files=all",
                "--ignored",
            ],
        )
    };
    assert_eq!(status(), "", "a clean repository to begin with");
    let code_before = code_files(&repo);
    assert!(code_before.len() >= 11, "{code_before:?}");
    let tree_before = worktree(&repo);
    let home = scratch.home("h");
    for args in [
        &["index"][..],
        &["--json", "index"],
        &["index", "--full"],
        &["index"],
    ] {
        ran(&spec(&home, &repo, args), &[0]);
    }
    for args in [
        &["refs", "ALM-GROWTH"][..],
        &["--json", "refs", "CHORE-01"],
        &["symbols", "tick"],
        &["--json", "symbols", "--file", "crates/farm/src/lib.rs"],
        &["unmapped"],
        &["--json", "unmapped", "crates/farm/src"],
    ] {
        ran(&spec(&home, &repo, args), &[0, 1]);
    }
    let configs = scratch.dir("configs");
    let config = fs::read_to_string(repo.join("specengine.toml")).expect("config");
    write(
        &configs,
        "loom.toml",
        config.replace("frameworks = [\"bevy\"]", "frameworks = [\"loom\"]"),
    );
    let loom = configs.join("loom.toml");
    let root = repo.to_str().expect("a UTF-8 root");
    let loom = loom.to_str().expect("a UTF-8 path");
    ran(
        &spec(&home, &repo, &["--root", root, "--config", loom, "index"]),
        &[2],
    );
    assert_eq!(status(), "", "a read wrote under the root");
    assert_eq!(code_files(&repo), code_before);
    assert_eq!(worktree(&repo), tree_before);
    assert!(
        db(&home, SLUG).is_file(),
        "the index is in the data directory"
    );
}
