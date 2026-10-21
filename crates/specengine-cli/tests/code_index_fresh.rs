//! docs/features/symbol-index.md "Description and interactions",
//! Freshness: RD-8 (`refs` refreshes the code index before it reads), RD-9
//! (`show`, `search`, `graph`, `verify`, `verify --staged`,
//! `propose drift` and `lock accept` never refresh it: they keep their
//! fresh walk or a commit's blobs), RD-10 and TE-5 (only the data
//! directory is written: `index` and `refs` leave a git worktree's status
//! and every code file's size, mtime and BLAKE3 as they were), TE-4
//! (`refs` byte-identical from two roots, another cwd, a reverse creation
//! order and a CRLF copy). docs/features/show-bindings.md RD-4: `show`
//! without `--bindings` still never refreshes the code index (RD-9's
//! test), and `show --bindings` refreshes it before it reads
//! (`show_bindings_refresh_the_code_index`).
//!
//! Over scratch copies of `fixtures/code-index` and `fixtures/code-a`
//! (git repositories made at test time where git is the subject); the code
//! tables compared through the store's canonical dump.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod code_index_common;
mod common;
mod lock_common;

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use code_index_common::{
    CODE_A, CODE_A_SLUG, FIXTURE, SLUG, code_lines, copy, dump, expected, index, to_crlf,
};
use common::bundle::blake3_hex;
use common::git::Sandbox;
use common::{Run, Scratch, spec, write};
use lock_common::LockRepo;

/// The REFs of code-a every determinism test reads (`code_refs_walk.rs`'s).
const CODE_A_REFS: [&str; 8] = [
    "PIECE-COUNTER",
    "LAW-BOUND",
    "LAW-STEP",
    "LAW-ODD",
    "GOAL-01",
    "PREM-001",
    "cap-check/PASS-01",
    "GOAL-99",
];

/// The REFs of code-index: `expected.json`'s and the root node's.
fn code_index_refs() -> Vec<String> {
    let mut refs: Vec<String> = expected()["refs"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    refs.push("SLAB-SHEET".to_owned());
    refs.push("SLAB-INK".to_owned());
    refs
}

fn same(a: &Run, b: &Run, context: &str) {
    assert_eq!(
        (a.code, &a.stdout, &a.stderr),
        (b.code, &b.stdout, &b.stderr),
        "{context}"
    );
}

// ------------------------------------------------------------------ RD-8

/// A named edit of a root.
type Edit = (&'static str, fn(&Path));

/// RD-8. M: unrefreshed rows. After code edits that no `spec index`
/// follows (a marker added, an item moved down, a file removed, a twin
/// inserted), `spec refs` in the indexed `HOME` prints what it prints from
/// a fresh `HOME`, text and JSON: `refs` refreshes the code index first.
#[test]
fn refs_after_edit_equals_a_fresh_walk() {
    let scratch = Scratch::new("cf-refs-edit");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    let references = code_index_refs();
    for reference in &references {
        spec(&home, &root, &["refs", reference]);
    }
    let edits: [Edit; 4] = [
        ("a marker added", |root| {
            let path = "crates/inkwell/src/broken.rs";
            let text = fs::read_to_string(root.join(path)).unwrap();
            write(
                root,
                path,
                text.replace("pub fn intact()", "// @implements TENET-WIDTH@2\npub fn later() -> u32 {\n    0\n}\n\npub fn intact()"),
            );
        }),
        ("an item moved down", |root| {
            let path = "crates/inkwell/src/page.rs";
            let text = fs::read_to_string(root.join(path)).unwrap();
            write(root, path, format!("\n\n{text}"));
        }),
        ("a file removed", |root| {
            fs::remove_file(root.join("crates/inkwell/src/split/mod.rs")).unwrap();
        }),
        ("a twin inserted", |root| {
            let path = "crates/inkwell/src/twins.rs";
            let text = fs::read_to_string(root.join(path)).unwrap();
            write(
                root,
                path,
                format!(
                    "{text}\nimpl Sheet {{\n    pub fn depth(&self) -> u32 {{\n        1\n    }}\n}}\n"
                ),
            );
        }),
    ];
    for (what, edit) in edits {
        edit(&root);
        let fresh = scratch.home(&format!("fresh-{}", what.replace(' ', "-")));
        for reference in &references {
            for json in [false, true] {
                let mut args: Vec<&str> = if json { vec!["--json"] } else { Vec::new() };
                args.extend(["refs", reference.as_str()]);
                let have = spec(&home, &root, &args);
                let want = spec(&fresh, &root, &args);
                same(&have, &want, &format!("{what}: {args:?}"));
            }
        }
    }
}

// ------------------------------------------------------------------ RD-9

/// RD-9. M: `show`, `search` or `graph` refreshes code. After a code edit
/// the code tables of the index stay as the last `spec index` left them
/// through `show`, `search` and `graph` (text and JSON); `spec index` then
/// picks the edit up.
#[test]
fn show_search_graph_never_refresh_code() {
    let scratch = Scratch::new("cf-no-refresh");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    index(&home, &root);
    let before = code_lines(&dump(&home, &root, SLUG));
    let path = "crates/inkwell/src/page.rs";
    let text = fs::read_to_string(root.join(path)).unwrap();
    write(&root, path, format!("{text}\npub fn added() {{}}\n"));
    for args in [
        &["show", "QUILL-01"][..],
        &["--json", "show", "QUILL-01"],
        &["show", "SLAB-SHEET"],
        &["search", "sheet"],
        &["--json", "search", "marks"],
        &["graph", "SLAB-SHEET"],
        &["--json", "graph", "QUILL-01"],
    ] {
        spec(&home, &root, args).code(0);
        assert_eq!(
            code_lines(&dump(&home, &root, SLUG)),
            before,
            "{args:?} refreshed the code index"
        );
    }
    index(&home, &root);
    assert_ne!(
        code_lines(&dump(&home, &root, SLUG)),
        before,
        "spec index sees the edit"
    );
}

/// RD-9. M: `verify`, `verify --staged`, `propose drift` or `lock accept`
/// refreshes the code index. A git repository of code-index with
/// QUILL-01 accepted and indexed, then a committed edit that drifts it:
/// each of the four answers (from its own walk or the commit's blobs)
/// and leaves the code tables as the last `spec index` left them.
#[test]
fn verify_leaves_the_code_index_untouched() {
    let repo = LockRepo::of("cf-verify", FIXTURE, "repo", &[]);
    repo.accept_ok("QUILL-01");
    repo.spec_piped(&repo.root, &["index"]).code(0);
    let before = code_lines(&dump(&repo.home, &repo.root, SLUG));
    repo.replace(
        "crates/inkwell/src/page.rs",
        "self.marks.len())",
        "self.marks.len() + 1)",
    );
    repo.commit_all("the edit");
    let commands: [&[&str]; 5] = [
        &["verify"],
        &["--json", "verify"],
        &["verify", "--staged"],
        &["--json", "verify", "--staged"],
        &["propose", "drift", "--json"],
    ];
    for args in commands {
        let run = repo.spec_piped(&repo.root, args);
        // An answer, whatever its verdict (`verify` exits 2 on the
        // fixture's `cannot_verify` row): never a refusal with nothing
        // read.
        assert!(
            !run.stdout.is_empty() && [0, 1, 2].contains(&run.code),
            "{args:?} must answer: {}",
            run.show()
        );
        assert_eq!(
            code_lines(&dump(&repo.home, &repo.root, SLUG)),
            before,
            "{args:?} refreshed the code index"
        );
    }
    repo.accept_ok("QUILL-01");
    assert_eq!(
        code_lines(&dump(&repo.home, &repo.root, SLUG)),
        before,
        "lock accept refreshed the code index"
    );
    repo.spec_piped(&repo.root, &["index"]).code(0);
    assert_ne!(
        code_lines(&dump(&repo.home, &repo.root, SLUG)),
        before,
        "spec index sees the edit"
    );
}

/// docs/features/show-bindings.md RD-4. M4: `show` without `--bindings`
/// refreshes the code index (above, RD-9); here, M: `show --bindings`
/// reading the index the last `spec index` left. A git repository of
/// code-a with the golden lock, indexed; then a new test file with a
/// marker of LAW-BOUND and no `spec index`: `show LAW-BOUND --bindings`
/// (text and JSON) lists the new file's `unlocked` row, located as its
/// item, and leaves the index equal to a fresh rebuild; a marker edited
/// away again drops the row, still without `spec index`.
#[test]
fn show_bindings_refresh_the_code_index() {
    let repo = LockRepo::of("cf-show-bindings", CODE_A, "repo", &[]);
    repo.write(
        "spec.lock",
        fs::read(common::fixture("spec-lock").join("code-a.lock")).expect("the golden lock"),
    );
    repo.commit_all("the lock");
    repo.spec_piped(&repo.root, &["index"]).code(0);
    let indexed = code_lines(&dump(&repo.home, &repo.root, CODE_A_SLUG));
    let fresh = "crates/tally/tests/fresh.rs";
    repo.write(
        fresh,
        "// @verifies LAW-BOUND@2\n#[test]\nfn fresh_bound() {}\n",
    );
    let rows_of = |args: &[&str]| -> Vec<serde_json::Value> {
        let run = repo.spec_piped(&repo.root, args);
        run.code(0);
        run.json()["nodes"][0]["bindings"]["rows"]
            .as_array()
            .unwrap_or_else(|| panic!("{args:?}: bindings rows\n{}", run.show()))
            .clone()
    };
    let rows = rows_of(&["show", "LAW-BOUND", "--bindings", "--json"]);
    let new_row = rows
        .iter()
        .find(|row| row["file"] == fresh)
        .unwrap_or_else(|| panic!("the new file's row without spec index: {rows:#?}"));
    assert_eq!(new_row["sync"], "unlocked", "{new_row}");
    assert_eq!(new_row["relation"], "verifies");
    assert_eq!(
        new_row["line"], 3,
        "located as its stored item (the attribute aside): {new_row}"
    );
    assert_eq!(new_row["end_line"], 3, "{new_row}");
    let text = repo.spec_piped(&repo.root, &["show", "LAW-BOUND", "--bindings"]);
    text.code(0);
    assert!(
        text.stdout.lines().any(|line| line
            .starts_with(&format!("  LAW-BOUND | verifies | {fresh}:3-3 | "))
            && line.ends_with(" | sync unlocked")),
        "the text lists it too\n{}",
        text.show()
    );
    assert_ne!(
        code_lines(&dump(&repo.home, &repo.root, CODE_A_SLUG)),
        indexed,
        "the read refreshed the code index"
    );
    code_index_common::assert_equals_fresh(
        &repo.scratch,
        &repo.home,
        &repo.root,
        CODE_A_SLUG,
        "after show --bindings",
    );
    repo.write(fresh, "#[test]\nfn fresh_bound() {}\n");
    let rows = rows_of(&["show", "LAW-BOUND", "--bindings", "--json"]);
    assert!(
        rows.iter().all(|row| row["file"] != fresh),
        "the marker gone, the row gone, still without spec index: {rows:#?}"
    );
}

/// docs/features/bundle-bindings.md RD-5 (freshness): with `[code]`,
/// `bundle` and `task show` refresh the code index once (as `show
/// --bindings`): a git repository of code-a with the golden lock, indexed;
/// a new test file bound to LAW-BOUND and no `spec index`: the bundle's
/// layer 9 lists its `unlocked` row and the index equals a fresh rebuild;
/// another new file, and a task's package on LAW-BOUND lists its row too.
/// Without `[code]` neither reads code: after a code edit the code tables
/// stay as they were. M: the bundle reading the index the last `spec
/// index` left; the code refreshed without `[code]`.
#[test]
fn bundle_and_task_show_refresh_the_code_index_only_with_code() {
    let repo = LockRepo::of("cf-bundle-bindings", CODE_A, "repo", &[]);
    repo.write(
        "spec.lock",
        fs::read(common::fixture("spec-lock").join("code-a.lock")).expect("the golden lock"),
    );
    repo.commit_all("the lock");
    repo.spec_piped(&repo.root, &["index"]).code(0);
    let made = repo.spec_piped(
        &repo.root,
        &[
            "task",
            "new",
            "--nodes",
            "LAW-BOUND",
            "--title",
            "Bound",
            "--goal",
            "Keep the cap.",
        ],
    );
    made.code(0);
    let indexed = code_lines(&dump(&repo.home, &repo.root, CODE_A_SLUG));
    let fresh = "crates/tally/tests/fresh.rs";
    repo.write(
        fresh,
        "// @verifies LAW-BOUND@2\n#[test]\nfn fresh_bound() {}\n",
    );
    let bundle = repo.spec_piped(
        &repo.root,
        &["bundle", "LAW-BOUND", "--budget", "10000", "--json"],
    );
    bundle.code(0);
    let tests = bundle.json()["layers"]["tests"]
        .as_array()
        .expect("layer 9")
        .clone();
    let row = tests
        .iter()
        .find(|row| row["file"] == fresh)
        .unwrap_or_else(|| panic!("the new file's row without spec index: {tests:#?}"));
    assert_eq!(row["sync"], "unlocked", "{row}");
    assert_ne!(
        code_lines(&dump(&repo.home, &repo.root, CODE_A_SLUG)),
        indexed,
        "the bundle refreshed the code index"
    );
    code_index_common::assert_equals_fresh(
        &repo.scratch,
        &repo.home,
        &repo.root,
        CODE_A_SLUG,
        "after bundle",
    );
    let again = "crates/tally/tests/again.rs";
    repo.write(
        again,
        "// @verifies LAW-BOUND@2\n#[test]\nfn bound_again() {}\n",
    );
    let package = repo.spec_piped(&repo.root, &["task", "show", "T-0001", "--json"]);
    package.code(0);
    let rows = package.json()["bindings"]
        .as_array()
        .expect("bindings")
        .clone();
    assert!(
        rows.iter()
            .any(|row| row["file"] == again && row["sync"] == "unlocked"),
        "the package lists the newer file's row without spec index: {rows:#?}"
    );
    code_index_common::assert_equals_fresh(
        &repo.scratch,
        &repo.home,
        &repo.root,
        CODE_A_SLUG,
        "after task show",
    );

    // Without `[code]`: never read.
    let config = repo.read("specengine.toml");
    let start = config.find("\n[code]\n").expect("a [code] table");
    repo.write("specengine.toml", &config[..=start]);
    let before = code_lines(&dump(&repo.home, &repo.root, CODE_A_SLUG));
    repo.write(
        "crates/tally/tests/third.rs",
        "// @verifies LAW-BOUND@2\n#[test]\nfn third() {}\n",
    );
    for args in [
        &["bundle", "LAW-BOUND", "--budget", "10000"][..],
        &["bundle", "LAW-BOUND", "--json"],
        &["task", "show", "T-0001"],
        &["task", "show", "T-0001", "--json"],
    ] {
        let run = repo.spec_piped(&repo.root, args);
        run.code(0);
        assert!(
            !run.stderr.contains("bindings"),
            "{args:?}: no note\n{}",
            run.show()
        );
        assert_eq!(
            code_lines(&dump(&repo.home, &repo.root, CODE_A_SLUG)),
            before,
            "{args:?} read code without [code]"
        );
    }
}

// ------------------------------------------------------------ RD-10, TE-5

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

/// RD-10, TE-5. M: a cache under the root. In a git repository of
/// code-index and of code-a, `index` (incremental and `--full`, text and
/// JSON) and `refs` (answers, exit 1) leave `git status --porcelain`
/// empty (ignored files too), every code file's (path, size, mtime,
/// BLAKE3) and the whole worktree unchanged; the database lives under the
/// scratch `HOME`.
#[test]
fn index_leaves_the_worktree_untouched() {
    for (fixture, slug, references) in [
        (FIXTURE, SLUG, code_index_refs()),
        (
            CODE_A,
            CODE_A_SLUG,
            CODE_A_REFS.iter().map(|r| (*r).to_owned()).collect(),
        ),
    ] {
        let scratch = Scratch::new("cf-read-only");
        let sandbox = Sandbox::new(scratch.path());
        let repo = copy(&scratch, fixture, "repo");
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
        assert_eq!(status(), "", "{fixture}: a clean repository to begin with");
        let code_before = code_files(&repo);
        let tree_before = worktree(&repo);
        let home = scratch.home("h");
        for args in [
            &["index"][..],
            &["index"],
            &["--json", "index"],
            &["index", "--full"],
        ] {
            spec(&home, &repo, args).code(0);
        }
        for reference in &references {
            spec(&home, &repo, &["refs", reference]);
            spec(&home, &repo, &["--json", "refs", reference]);
        }
        spec(&home, &repo, &["refs", "\u{0420}IECE-COUNTER"]);
        assert_eq!(
            status(),
            "",
            "{fixture}: index or refs wrote under the root"
        );
        assert_eq!(code_files(&repo), code_before, "{fixture}");
        assert_eq!(worktree(&repo), tree_before, "{fixture}");
        assert!(
            code_index_common::db(&home, slug).is_file(),
            "{fixture}: the index is in the data directory"
        );
    }
}

// ------------------------------------------------------------------ TE-4

/// Copies `from` to `to` creating every entry in reverse name order.
fn copy_reversed(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("target");
    let mut entries: Vec<_> = fs::read_dir(from)
        .expect("directory")
        .map(|entry| entry.expect("entry"))
        .collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    for entry in entries {
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_reversed(&entry.path(), &target);
        } else if entry.file_name() != "expected.json" {
            fs::write(&target, fs::read(entry.path()).expect("read")).expect("write");
        }
    }
}

/// Every `.rs` and `.ron` file under `root` (`.git` aside), root-relative.
fn code_paths_under(root: &Path) -> Vec<String> {
    common::snapshot(root)
        .into_iter()
        .filter(|(path, bytes)| {
            bytes.is_some()
                && !path.starts_with(".git")
                && (path.ends_with(".rs") || path.ends_with(".ron"))
        })
        .map(|(path, _)| path)
        .collect()
}

/// TE-4. M: rowid order, a stored absolute path, CR kept. `refs` over
/// code-index and code-a: a copy, a second run, a copy made in reverse
/// creation order under another name read with `--root` from another cwd,
/// and a copy with every code file CRLF, each with its own `HOME`: stdout,
/// stderr and exit byte for byte alike, text and JSON.
#[test]
fn refs_is_byte_identical_across_roots_order_and_crlf() {
    for (fixture, references) in [
        (FIXTURE, code_index_refs()),
        (
            CODE_A,
            CODE_A_REFS
                .iter()
                .map(|r| (*r).to_owned())
                .collect::<Vec<_>>(),
        ),
    ] {
        let scratch = Scratch::new("cf-determinism");
        let first = copy(&scratch, fixture, "one/copy");
        let second: PathBuf = scratch.dir("two/elsewhere-else");
        copy_reversed(&common::fixture(fixture), &second);
        let crlf = copy(&scratch, fixture, "three/crlf");
        for path in code_paths_under(&crlf) {
            to_crlf(&crlf, &path);
        }
        let cwd = scratch.dir("somewhere");
        let (home_a, home_b, home_c) = (scratch.home("a"), scratch.home("b"), scratch.home("c"));
        let second_text = second.to_str().expect("UTF-8 scratch");
        for reference in &references {
            for json in [false, true] {
                let mut args: Vec<&str> = if json { vec!["--json"] } else { Vec::new() };
                args.extend(["refs", reference.as_str()]);
                let a = spec(&home_a, &first, &args);
                let again = spec(&home_a, &first, &args);
                let mut rooted = vec!["--root", second_text];
                rooted.extend(&args);
                let b = spec(&home_b, &cwd, &rooted);
                let c = spec(&home_c, &crlf, &args);
                same(&a, &again, &format!("{fixture}: again {args:?}"));
                same(&a, &b, &format!("{fixture}: another root {args:?}"));
                same(&a, &c, &format!("{fixture}: CRLF {args:?}"));
            }
        }
    }
}
