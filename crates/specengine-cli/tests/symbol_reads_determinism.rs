//! docs/features/symbol-reads.md RD-4 and TE-2 (its "Rules and edge
//! cases": output byte-identical from two roots, cwds, creation orders and
//! CRLF copies, no order following rowid), and its read-only rule (only the
//! data directory is written, through 6a-2's refresh).
//!
//! Over scratch copies of `fixtures/code-index` and of
//! `fixtures/verify-ecs` (an ECS-style game crate: ADR-0008's second
//! project), each read with its own `HOME`.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod symbol_reads_common;

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::MetadataExt as _;
use std::path::Path;

use common::git::Sandbox;
use common::{Run, Scratch, spec};
use symbol_reads_common::{
    ECS, FIXTURE, code_paths_under, copy, copy_reversed, reads, stdout_of, to_crlf,
};

/// The reads compared over code-index: every form, a twin pair, a
/// cross-file pair, a module and a RON target, each `unmapped` shape.
const INKWELL_READS: [&[&str]; 11] = [
    &["symbols", "main"],
    &["symbols", "run"],
    &["symbols", "impl Sheet #2"],
    &["symbols", "separator"],
    &["symbols", "--node", "TENET-WIDTH"],
    &["symbols", "--node", "SLAB-SHEET"],
    &["symbols", "--file", "crates/inkwell/src/twins.rs"],
    &["symbols", "--at", "crates/inkwell/src/lib.rs:19"],
    &["unmapped"],
    &["unmapped", "crates/inkwell/src"],
    &["unmapped", "crates/inkwell/src/twins.rs"],
];

/// The reads compared over verify-ecs.
const ECS_READS: [&[&str]; 6] = [
    &["symbols", "regen_system"],
    &["symbols", "--node", "PERK-STAMINA"],
    &["symbols", "--file", "crates/game/src/stamina.rs"],
    &["symbols", "--at", "crates/game/src/lib.rs:18"],
    &["unmapped"],
    &["unmapped", "crates/game/src"],
];

fn same(a: &Run, b: &Run, context: &str) {
    assert_eq!(
        (a.code, &a.stdout, &a.stderr),
        (b.code, &b.stdout, &b.stderr),
        "{context}"
    );
}

/// docs/features/layer-c-reads.md TE-2 over this file's two corpora (no
/// layer C snapshot is ever stored here): a `spec symbols` text holds no
/// `  c ` line and no `layer c:` line; a JSON document's `layer_c` and each
/// target's `c_state`, `moniker`, `agrees` are `null`.
fn assert_absent_layer_c(run: &Run, json: bool, context: &str) {
    if json {
        let value = run.json();
        assert_eq!(
            value.get("layer_c"),
            Some(&serde_json::Value::Null),
            "{context}: `layer_c` while absent"
        );
        for target in value["symbols"].as_array().expect("symbols") {
            for key in ["c_state", "moniker", "agrees"] {
                assert_eq!(
                    target.get(key),
                    Some(&serde_json::Value::Null),
                    "{context}: `{key}` while absent: {target}"
                );
            }
        }
    } else {
        assert!(
            !run.stdout
                .lines()
                .any(|line| line.starts_with("  c ") || line.starts_with("layer c:")),
            "{context}: a layer C line while no snapshot is stored:\n{}",
            run.stdout
        );
    }
}

/// `text` appended to `root/relative`: the file is re-derived, its rows
/// written again after every other file's.
fn append(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    let mut bytes = fs::read(&path).expect("read");
    bytes.extend_from_slice(text.as_bytes());
    fs::write(&path, bytes).expect("write");
}

/// RD-4, TE-2. M: rowid order (also a stored absolute path, CR kept). For
/// code-index and verify-ecs, every read of the lists above, text and
/// JSON, each with its own `HOME`: a copy; the same again; a copy made in
/// reverse creation order under another name, read with `--root` from
/// another cwd; the copy read from a subdirectory (PATH stays
/// root-relative); a copy whose code files are all CRLF; and a copy
/// indexed, then two of its files re-derived (a comment line appended:
/// their rows now follow the others' in rowid) — stdout, stderr and exit
/// byte for byte alike. docs/features/layer-c-reads.md TE-2 (M: a `c` line
/// while absent): no `symbols` read shows a layer C line or value
/// ([`assert_absent_layer_c`]).
#[test]
fn symbols_and_unmapped_are_byte_identical_across_roots_and_order() {
    for (fixture, reads_of, subdirectory, rederived) in [
        (
            FIXTURE,
            &INKWELL_READS[..],
            "crates/inkwell",
            [
                "crates/inkwell/src/bin/stamp.rs",
                "crates/inkwell/src/twins.rs",
            ],
        ),
        (
            ECS,
            &ECS_READS[..],
            "crates/game/src",
            ["crates/game/src/lib.rs", "assets/data/movement.ron"],
        ),
    ] {
        let scratch = Scratch::new("sr-determinism");
        let first = copy(&scratch, fixture, "one/copy");
        let second = scratch.dir("two/elsewhere-else");
        copy_reversed(&common::fixture(fixture), &second);
        let crlf = copy(&scratch, fixture, "three/crlf");
        for path in code_paths_under(&crlf) {
            to_crlf(&crlf, &path);
        }
        let later = copy(&scratch, fixture, "four/later");
        let home_later = scratch.home("later");
        spec(&home_later, &later, &["index"]).code(0);
        for path in rederived {
            append(&later, path, "// appended\n");
        }
        let cwd = scratch.dir("somewhere");
        let (home_a, home_b, home_c, home_d) = (
            scratch.home("a"),
            scratch.home("b"),
            scratch.home("c"),
            scratch.home("d"),
        );
        let second_text = second.to_str().expect("a UTF-8 scratch");
        for read in reads_of {
            for json in [false, true] {
                let mut args: Vec<&str> = if json { vec!["--json"] } else { Vec::new() };
                args.extend(read.iter());
                let context = format!("{fixture}: {args:?}");
                let a = spec(&home_a, &first, &args);
                a.code(0);
                assert!(!a.stdout.is_empty(), "{context}");
                if read[0] == "symbols" {
                    assert_absent_layer_c(&a, json, &context);
                }
                same(
                    &a,
                    &spec(&home_a, &first, &args),
                    &format!("{context} again"),
                );
                let mut rooted = vec!["--root", second_text];
                rooted.extend(&args);
                same(
                    &a,
                    &spec(&home_b, &cwd, &rooted),
                    &format!("{context} another root, cwd and creation order"),
                );
                same(
                    &a,
                    &spec(&home_d, &first.join(subdirectory), &args),
                    &format!("{context} from {subdirectory}"),
                );
                same(&a, &spec(&home_c, &crlf, &args), &format!("{context} CRLF"));
                same(
                    &a,
                    &spec(&home_later, &later, &args),
                    &format!("{context} re-derived files inserted last"),
                );
            }
        }
    }
}

/// RD-4 against the goldens: a fresh index answers TE-1's multi-row
/// goldens, so an order that equals the walk's by chance is not enough.
#[test]
fn reads_order_is_the_golden_order_after_an_edit() {
    let scratch = Scratch::new("sr-golden-order");
    let root = copy(&scratch, FIXTURE, "code-index");
    let home = scratch.home("h");
    spec(&home, &root, &["index"]).code(0);
    append(&root, "crates/inkwell/src/main.rs", "// appended\n");
    append(&root, "crates/inkwell/src/split.rs", "// appended\n");
    let all = reads();
    for list in ["symbols", "unmapped"] {
        for entry in all["reads"][list].as_array().expect("a list") {
            let args: Vec<String> = entry["args"]
                .as_array()
                .expect("args")
                .iter()
                .map(|arg| arg.as_str().expect("an argument").to_owned())
                .collect();
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            let run = spec(&home, &root, &args);
            run.code(0);
            assert_eq!(run.stdout, stdout_of(entry), "{args:?}");
        }
    }
}

/// `(size, mtime ns)` of every entry under `root`, `.git` aside.
fn stats(root: &Path) -> BTreeMap<String, (u64, i128)> {
    common::snapshot(root)
        .into_keys()
        .filter(|path| !path.starts_with(".git"))
        .map(|path| {
            let metadata = fs::symlink_metadata(root.join(&path)).expect("metadata");
            let mtime =
                i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec());
            (path, (metadata.len(), mtime))
        })
        .collect()
}

/// "Rules and edge cases": only the data directory is written. In a git
/// repository of code-index, every read (answers, exit 1, exit 2), text and
/// JSON, leaves `git status --porcelain --ignored` empty and every entry's
/// bytes, size and mtime as they were; the database is under the scratch
/// `HOME`.
#[test]
fn symbols_and_unmapped_leave_the_worktree_untouched() {
    let scratch = Scratch::new("sr-read-only");
    let root = copy(&scratch, FIXTURE, "code-index");
    let git = Sandbox::new(scratch.path());
    git.init(&root);
    git.add_all(&root);
    git.commit(&root, "fixture");
    let home = scratch.home("h");
    let worktree = |root: &Path| {
        let mut entries = common::snapshot(root);
        entries.retain(|path, _| !path.starts_with(".git"));
        entries
    };
    let bytes_before = worktree(&root);
    let stats_before = stats(&root);
    let mut commands: Vec<Vec<&str>> = INKWELL_READS.iter().map(|read| read.to_vec()).collect();
    commands.push(vec!["symbols", "--node", "QUILL-99"]);
    commands.push(vec!["symbols", "--file", "docs/spec/ink.md"]);
    commands.push(vec!["unmapped", "docs"]);
    for read in &commands {
        for json in [false, true] {
            let mut args: Vec<&str> = if json { vec!["--json"] } else { Vec::new() };
            args.extend(read.iter());
            let run = spec(&home, &root, &args);
            assert!([0, 1, 2].contains(&run.code), "{}", run.show());
        }
    }
    assert_eq!(
        git.git_text(&root, &["status", "--porcelain", "--ignored"]),
        ""
    );
    assert!(worktree(&root) == bytes_before, "the worktree's bytes");
    assert!(stats(&root) == stats_before, "sizes and mtimes");
    let data = common::data_dir(&home);
    assert!(
        data.join("inkwell.db").is_file(),
        "the index lives in the data directory: {}",
        data.display()
    );
}
