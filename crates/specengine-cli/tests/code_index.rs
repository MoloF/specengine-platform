//! docs/features/symbol-index.md (slice 6a-2): TE-1 (`fixtures/code-index`
//! and its `expected.json`: every stored `symbols` row, the suffix rows,
//! signatures, level states, the `spec index` code line, `spec refs` from
//! the index), RD-1 (the tables with the schema in every DB), RD-14 (a bad
//! `[code]` still writes the md index), RD-15 (the store's per-file names
//! for 6b), TE-6 (spec-c: no code line), and the edge data the store keeps
//! leniently while reads exit 2.
//!
//! Real data: the `spec` binary over scratch copies of the fixtures, each
//! index in a scratch `HOME`; the stored rows read by column name through
//! the system's `sqlite3` (`code_index_common`). The stored levels and
//! signatures are compared with `specengine-code`'s own `item_levels` and
//! `item_signatures` over the same bytes, on code-index, code-a and
//! verify-ecs (an ECS-style game crate: ADR-0008's second project).
//!
//! docs/features/bevy-roles.md RD-3: `code_roles` is one of the code
//! tables (`CODE_TABLES`), so the format change gives every one of them a
//! stale column and wants it gone (`FORMAT_TABLES`).
//!
//! Built names the spec leaves open: `SqliteIndex::stored_names` (RD-15)
//! and the JSON keys of `spec index`'s `code` object (the line's words).
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod code_index_common;
mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use specengine_code::{
    LevelState, NamedFile, RustParser, bind_tree, item_levels, item_signatures, layout_targets,
    name_files,
};

use code_index_common::{
    CODE_A, CODE_TABLES, ECS, FIXTURE, SLUG, code_files, code_line, code_lines, copy, count,
    counts_of, db, dump, execute, expected, fresh_dump, fresh_dump_with_exit, index, md_lines,
    query, refs_rows, suffixes, symbols, table_names, with_code_table,
};
use common::{Scratch, read_text, spec, write};

/// `b3:` and 64 lower-case hex digits.
fn is_digest(value: &Value) -> bool {
    value.as_str().is_some_and(|text| {
        text.strip_prefix("b3:").is_some_and(|hex| {
            hex.len() == 64
                && hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
    })
}

/// The level-digest columns (5b's key names, Q11).
const DIGESTS: [&str; 5] = [
    "sig_hash",
    "body_hash",
    "deps_hash",
    "skeleton_hash",
    "quote_hash",
];

/// The 1-based line holding byte `at` of `bytes`.
fn line_at(bytes: &[u8], at: usize) -> usize {
    bytes[..at].iter().filter(|&&b| b == b'\n').count() + 1
}

// ------------------------------------------------------------------ TE-1

/// TE-1. M: a discriminator, a suffix, a signature or a level state of
/// the fixture changed. Every `symbols` row of `fixtures/code-index` as
/// `expected.json` gives it, in (path, ord) order; `layer` `a` on every row
/// (ADR-0049); digests `b3:` + 64 hex and `body_bytes` when hashed, all
/// NULL when `cannot_verify`; `start_byte`..`end_byte` the item node, on
/// its lines; the suffix rows of the named qpaths by depth, each row's
/// deepest suffix its qpath, depths 1..=n; the code line of `spec index`
/// between the md line and the `db` line.
#[test]
fn code_index_rows_match_expected_json() {
    let scratch = Scratch::new("ci-rows");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    let expected = expected();
    let run = index(&home, &root);
    let lines: Vec<&str> = run.stdout.lines().collect();
    assert_eq!(lines.len(), 3, "{}", run.show());
    assert!(
        lines[0].starts_with("indexed inkwell: walked "),
        "{}",
        run.show()
    );
    assert_eq!(lines[1], expected["index_code_line"].as_str().unwrap());
    assert!(lines[2].starts_with("db "), "{}", run.show());
    assert_eq!(run.stderr, "", "{}", run.show());
    let db = db(&home, SLUG);

    let files = code_files(&db);
    let want_files = expected["code_files"].as_array().unwrap();
    assert_eq!(files.len(), want_files.len(), "{files:#?}");
    for (row, want) in files.iter().zip(want_files) {
        assert_eq!(row["path"], want["path"]);
        if want["package"].is_string() {
            assert_eq!(row["package"], want["package"], "{}", want["path"]);
            assert_eq!(row["module"], want["module"], "{}", want["path"]);
        }
        assert_eq!(row["utf8"], 1, "{}", want["path"]);
        assert!(row["read_error"].is_null(), "{}", want["path"]);
        assert!(
            row["blake3"].as_str().is_some_and(|hash| !hash.is_empty()),
            "{}",
            want["path"]
        );
        let bytes = fs::read(root.join(want["path"].as_str().unwrap())).unwrap();
        assert_eq!(row["size"], bytes.len(), "{}", want["path"]);
    }
    let packages: Vec<Value> = query(&db, "SELECT dir FROM code_packages ORDER BY dir")
        .into_iter()
        .map(|row| row["dir"].clone())
        .collect();
    assert_eq!(Value::Array(packages), expected["packages"]);

    let rows = symbols(&db);
    let want_rows = expected["symbols"].as_array().unwrap();
    assert_eq!(rows.len(), want_rows.len(), "symbols");
    for (row, want) in rows.iter().zip(want_rows) {
        let at = format!("{}#{}", want["path"], want["ord"]);
        for key in [
            "path",
            "ord",
            "item_kind",
            "name",
            "owner",
            "qpath",
            "discriminator",
            "parent_ord",
            "start_line",
            "end_line",
            "signature",
            "level_state",
            "twin_files",
        ] {
            assert_eq!(row[key], want[key], "{at}: {key}");
        }
        let categories: Value = serde_json::from_str(
            row["level_categories"]
                .as_str()
                .unwrap_or_else(|| panic!("{at}: level_categories is JSON text")),
        )
        .expect("level_categories JSON");
        assert_eq!(categories, want["level_categories"], "{at}");
        assert_eq!(row["layer"], "a", "{at}");
        if want["level_state"] == "hashed" {
            for key in DIGESTS {
                assert!(is_digest(&row[key]), "{at}: {key} {}", row[key]);
            }
            assert!(row["body_bytes"].is_u64(), "{at}: body_bytes");
        } else {
            for key in DIGESTS.iter().chain(&["body_bytes"]) {
                assert!(row[*key].is_null(), "{at}: {key} NULL when cannot_verify");
            }
        }
        let bytes = fs::read(root.join(want["path"].as_str().unwrap())).unwrap();
        let start = usize::try_from(row["start_byte"].as_u64().expect("start_byte")).unwrap();
        let end = usize::try_from(row["end_byte"].as_u64().expect("end_byte")).unwrap();
        assert!(start < end && end <= bytes.len(), "{at}: {start}..{end}");
        assert_eq!(
            line_at(&bytes, start),
            want["start_line"],
            "{at}: start_byte"
        );
        assert_eq!(line_at(&bytes, end - 1), want["end_line"], "{at}: end_byte");
        assert!(
            !bytes[start..end].starts_with(b"#[") && !bytes[start..end].starts_with(b"//"),
            "{at}: attributes and comments are not the item's"
        );
        if let Some(signature) = row["signature"].as_str() {
            assert!(
                signature.len() <= 512,
                "{at}: within SIGNATURE_CAP_BYTES: {}",
                signature.len()
            );
        }
    }
    let wide = rows
        .iter()
        .find(|row| row["qpath"] == "crates/inkwell::page::wide")
        .expect("wide");
    let wide = wide["signature"].as_str().unwrap();
    assert_eq!(wide.len(), 512, "a cut signature fills the cap");
    assert!(wide.ends_with('\u{2026}'), "{wide}");

    let suffix_rows = suffixes(&db);
    assert_eq!(suffix_rows.len(), expected["suffix_rows"], "suffix rows");
    let mut by_symbol: BTreeMap<(String, u64), Vec<(u64, String)>> = BTreeMap::new();
    for (path, ord, depth, suffix) in suffix_rows {
        by_symbol
            .entry((path, ord))
            .or_default()
            .push((depth, suffix));
    }
    for row in &rows {
        let key = (
            row["path"].as_str().unwrap().to_owned(),
            row["ord"].as_u64().unwrap(),
        );
        let depths = &by_symbol[&key];
        let numbers: Vec<u64> = depths.iter().map(|(depth, _)| *depth).collect();
        assert_eq!(
            numbers,
            (1..=depths.len() as u64).collect::<Vec<_>>(),
            "{key:?}: depths 1..=n"
        );
        assert_eq!(
            depths.last().unwrap().1,
            row["qpath"].as_str().unwrap(),
            "{key:?}: the deepest suffix is the qpath"
        );
    }
    for (qpath, want) in expected["suffixes"].as_object().unwrap() {
        let row = rows
            .iter()
            .find(|row| row["qpath"] == qpath.as_str())
            .unwrap_or_else(|| panic!("no symbol {qpath}"));
        let key = (
            row["path"].as_str().unwrap().to_owned(),
            row["ord"].as_u64().unwrap(),
        );
        let got: Vec<&str> = by_symbol[&key].iter().map(|(_, s)| s.as_str()).collect();
        let want: Vec<&str> = want
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect();
        assert_eq!(got, want, "{qpath}: segments, never a `::` split");
    }

    assert_eq!(count(&db, "code_markers"), expected["markers"]);
    assert_eq!(count(&db, "code_mentions"), expected["mentions"]);
    assert_eq!(count(&db, "code_worktrees"), 1, "one code_fp row");
}

/// TE-1, RD-8: `spec refs` from the index prints `expected.json`'s rows for
/// every REF it names (a cfg twin, an ordinal twin, an inline mod's twin
/// module, a trait impl's method, a cross-file duplicate, a broken item,
/// RON paths, mentions); text and JSON agree on the code count.
#[test]
fn refs_from_the_index_match_expected_json() {
    let scratch = Scratch::new("ci-refs");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    let expected = expected();
    for (reference, want) in expected["refs"].as_object().unwrap() {
        let run = spec(&home, &root, &["refs", reference]);
        run.code(0);
        let want: Vec<String> = want
            .as_array()
            .unwrap()
            .iter()
            .map(|line| line.as_str().unwrap().to_owned())
            .collect();
        assert_eq!(refs_rows(&run.stdout), want, "{reference}\n{}", run.show());
        assert_eq!(run.stderr, "", "{reference}: {}", run.show());
        let json = spec(&home, &root, &["--json", "refs", reference]).json();
        assert_eq!(
            json["code"],
            json!({"files": 10, "rs": 9, "ron": 1, "skipped": 0}),
            "{reference}"
        );
    }
}

/// The stored levels and signatures of every Rust file equal
/// `specengine-code`'s `item_levels` and `item_signatures` over the same
/// bytes, item for item (`ord` = the analysis index), on code-index,
/// code-a and verify-ecs: the analyzer stores what the library computes,
/// for every item, bound or not (Q11). M: levels of bound items only; a
/// signature from another index.
#[test]
fn stored_levels_and_signatures_equal_the_code_library() {
    for (fixture, dir) in [
        (FIXTURE, "code-index"),
        (CODE_A, "code-a"),
        (ECS, "verify-ecs"),
    ] {
        let scratch = Scratch::new("ci-levels");
        let home = scratch.home("h");
        let root = copy(&scratch, fixture, dir);
        let slug = slug_of(&root);
        index(&home, &root);
        let db = db(&home, &slug);
        let rows = symbols(&db);
        let mut by_path: BTreeMap<String, Vec<&BTreeMap<String, Value>>> = BTreeMap::new();
        for row in &rows {
            by_path
                .entry(row["path"].as_str().unwrap().to_owned())
                .or_default()
                .push(row);
        }
        let mut parser = RustParser::new().expect("the Rust grammar");
        let rust: Vec<String> = code_files(&db)
            .into_iter()
            .filter(|row| row["path"].as_str().unwrap().ends_with(".rs") && row["utf8"] == 1)
            .map(|row| row["path"].as_str().unwrap().to_owned())
            .collect();
        assert!(rust.len() >= 2, "{fixture}: {rust:?}");
        let mut compared = 0;
        for path in rust {
            let source = read_text(&root, &path);
            let tree = parser.parse(&source).expect("a tree");
            let binding = bind_tree(&tree, &source);
            let indexes: Vec<usize> = (0..binding.analysis.items.len()).collect();
            let levels = item_levels(&tree, &source, &indexes);
            let signatures = item_signatures(&tree, &source, &indexes);
            let stored = by_path.remove(&path).unwrap_or_default();
            assert_eq!(
                stored.len(),
                indexes.len(),
                "{fixture}: {path}: one row per item"
            );
            for (index, row) in stored.iter().enumerate() {
                let at = format!("{fixture}: {path}#{index}");
                assert_eq!(row["ord"], index, "{at}");
                assert_eq!(
                    row["signature"].as_str(),
                    signatures[index].as_deref(),
                    "{at}: signature"
                );
                match levels[index].as_ref().expect("a level state") {
                    LevelState::Hashed(hashes) => {
                        assert_eq!(row["level_state"], "hashed", "{at}");
                        for (key, digest) in [
                            ("sig_hash", hashes.sig),
                            ("body_hash", hashes.body),
                            ("deps_hash", hashes.deps),
                            ("skeleton_hash", hashes.skeleton),
                            ("quote_hash", hashes.quote),
                        ] {
                            assert_eq!(row[key], format!("b3:{}", digest.to_hex()), "{at}: {key}");
                        }
                        assert_eq!(row["body_bytes"], hashes.body_bytes, "{at}");
                    }
                    LevelState::CannotVerify { categories } => {
                        assert_eq!(row["level_state"], "cannot_verify", "{at}");
                        let names: Vec<&str> = categories
                            .iter()
                            .map(|category| category.as_str())
                            .collect();
                        let stored: Value =
                            serde_json::from_str(row["level_categories"].as_str().unwrap())
                                .unwrap();
                        assert_eq!(stored, json!(names), "{at}");
                    }
                }
                compared += 1;
            }
        }
        assert!(
            by_path.is_empty(),
            "{fixture}: rows of no Rust file: {by_path:?}"
        );
        assert!(compared >= 10, "{fixture}: {compared} items compared");
    }
}

/// The `[project] slug` of a fixture copy.
fn slug_of(root: &Path) -> String {
    let table: toml::Table = read_text(root, "specengine.toml").parse().unwrap();
    table["project"]["slug"].as_str().unwrap().to_owned()
}

// ------------------------------------------------------------------ RD-1

/// RD-1, TE-6. M: created lazily. spec-a (no `[code]`): `spec index` exits
/// 0 with its two lines, JSON `code` `null`, and the database holds every
/// code table, empty (no `code_worktrees` row: no code indexed).
#[test]
fn index_without_code_creates_code_tables() {
    let scratch = Scratch::new("ci-lazy");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "spec-a");
    let run = spec(&home, &root, &["index"]);
    run.code(0);
    assert!(code_line(&run).is_none(), "{}", run.show());
    assert_eq!(run.stdout.lines().count(), 2, "{}", run.show());
    let db = db(&home, "lantern-keep");
    let tables = table_names(&db);
    for table in CODE_TABLES {
        assert!(tables.contains(table), "{table} missing: {tables:?}");
        assert_eq!(count(&db, table), 0, "{table}");
    }
    let json = spec(&home, &root, &["--json", "index"]).json();
    assert!(json["code"].is_null(), "{json}");
}

/// TE-6: spec-c, a project of another genre with no `[code]`: `index`
/// exits 0, no code line, JSON `code` `null` (`refs` exits 2 naming
/// `[code]`: `code_refs_walk.rs` `refs_on_spec_c_exits_2_naming_the_code_table`).
#[test]
fn index_on_spec_c_prints_no_code_line() {
    let scratch = Scratch::new("ci-spec-c");
    let home = scratch.home("h");
    let root = scratch.copy("spec-c", "spec-c");
    let run = spec(&home, &root, &["index"]);
    run.code(0);
    assert!(code_line(&run).is_none(), "{}", run.show());
    assert!(
        !run.stdout.lines().any(|line| line.starts_with("code ")),
        "{}",
        run.show()
    );
    let json = spec(&home, &root, &["--json", "index"]).json();
    assert!(json["code"].is_null(), "{json}");
    let run = spec(&home, &root, &["refs", "WISH-001"]);
    run.code(2);
    assert!(run.stderr.contains("[code]"), "{}", run.show());
}

/// RD-1, TE-3; bevy-roles RD-3. M: a code table absent from
/// `FORMAT_TABLES` (`code_roles` too). An index format change (the stored
/// stamp rewritten as an older build left it, every code table given a
/// stale column of that build): `spec index` recreates every index table,
/// the code ones too (each stale column gone), re-derives the code rows
/// and ends equal to a fresh index; the queue's proposal, events, task and
/// stage stay byte for byte; `--full` keeps them too.
#[test]
fn format_change_recreates_code_tables_and_keeps_queue_rows() {
    use specengine_core::proposal::Author;
    use specengine_store::{
        Decision, NewProposal, NewTask, Place, ProposalKind, ProposalQueue as _, SqliteQueue,
        Stage, patch_hash,
    };

    let scratch = Scratch::new("ci-format");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    index(&home, &root);
    let db = db(&home, SLUG);
    let mut queue = SqliteQueue::open(&db, SLUG).expect("the queue opens");
    let place = Place {
        git_common_dir: "/r/.git".to_owned(),
        worktree: "/r".to_owned(),
        root_rel: String::new(),
        branch: "main".to_owned(),
        base_commit: "1".repeat(40),
    };
    for target in ["QUILL-01", "QUILL-02"] {
        queue
            .create(
                &NewProposal {
                    kind: ProposalKind::Update,
                    target_id: target.to_owned(),
                    target_path: format!("docs/records/{target}.md"),
                    place: place.clone(),
                    base_hash: Some("b3:00".to_owned()),
                    base_text: Some("old".to_owned()),
                    new_text: "new".to_owned(),
                    patch_hash: patch_hash(target, "b3:00", "new"),
                    rationale: "why".to_owned(),
                    author: Author::human(),
                    diagnostics: Vec::new(),
                    new_ids: Vec::new(),
                    interpretation: None,
                },
                "2026-10-16T09:00:00Z",
            )
            .expect("create");
    }
    queue
        .approve(
            "PR-0001",
            &Decision {
                decided_by: "Ann <a@example.org>".to_owned(),
                note: None,
                staged_at: None,
            },
            "2026-10-16T09:01:00Z",
        )
        .expect("approve");
    let seen = queue.get("PR-0002").unwrap().unwrap().seen();
    queue
        .stage_from(
            "PR-0002",
            &seen,
            &Stage::Reject {
                reason: "Not now.".to_owned(),
            },
            "2026-10-16T09:02:00Z",
        )
        .expect("a reject stages");
    queue
        .create_task(
            &NewTask {
                git_common_dir: "/r/.git".to_owned(),
                title: Some("Print the marks".to_owned()),
                goal: Some("Pages print their marks".to_owned()),
                targets: vec!["QUILL-01".to_owned()],
                author: Author::human(),
            },
            "2026-10-16T09:03:00Z",
        )
        .expect("a task");
    let queue_dump = queue.dump().expect("queue dump");
    drop(queue);

    let stale: Vec<String> = CODE_TABLES
        .iter()
        .map(|table| {
            format!("ALTER TABLE {table} ADD COLUMN stale_of_an_older_build INTEGER DEFAULT 7;")
        })
        .collect();
    execute(
        &db,
        &format!(
            "{} UPDATE index_meta SET value = '0' WHERE key = 'format';",
            stale.join(" ")
        ),
    );
    let run = spec(&home, &root, &["index"]);
    run.code(0);
    let counts = counts_of(&run);
    assert_eq!(counts["parsed"], counts["walked"], "{}", run.show());
    for table in CODE_TABLES {
        let columns = query(
            &db,
            &format!(
                "SELECT name FROM pragma_table_xinfo('{table}') WHERE name = 'stale_of_an_older_build'"
            ),
        );
        assert!(columns.is_empty(), "{table} was not recreated");
    }
    assert_eq!(
        dump(&home, &root, SLUG),
        fresh_dump(&scratch, &root, SLUG),
        "after the format change"
    );
    let queue = SqliteQueue::open(&db, SLUG).expect("reopened");
    assert_eq!(queue.dump().expect("queue dump"), queue_dump, "the queue");
    drop(queue);

    let run = spec(&home, &root, &["index", "--full"]);
    run.code(0);
    let counts = counts_of(&run);
    assert_eq!(counts.get("rederived_all"), Some(&1), "{}", run.show());
    assert_eq!(counts["parsed"], counts["walked"], "{}", run.show());
    let queue = SqliteQueue::open(&db, SLUG).expect("reopened");
    assert_eq!(
        queue.dump().expect("queue dump"),
        queue_dump,
        "after --full"
    );
}

// ----------------------------------------------------------------- RD-14

/// RD-14 (Q15). M: the md index held back. A `[code]` with an unknown key:
/// `spec index` writes the md index and prints its line, then exits 2 with
/// one stderr line naming `specengine.toml:<line>:` and the key; no code
/// row changes; the md rows are a fresh index's.
#[test]
fn bad_code_table_still_writes_the_md_index() {
    let scratch = Scratch::new("ci-bad-code");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    index(&home, &root);
    let before = code_lines(&dump(&home, &root, SLUG));
    let config = read_text(&root, "specengine.toml");
    let bad = config.replacen(
        "roots = [\"crates\", \"data\"]\n",
        "roots = [\"crates\", \"data\"]\ncolour = 1\n",
        1,
    );
    assert_ne!(bad, config);
    write(&root, "specengine.toml", &bad);
    let line = bad
        .lines()
        .position(|text| text.starts_with("colour"))
        .unwrap()
        + 1;
    write(
        &root,
        "docs/records/QUILL-02.md",
        read_text(&root, "docs/records/QUILL-02.md").replace("never hashed", "not hashed"),
    );
    write(&root, "crates/inkwell/src/main.rs", "fn main() {}\n");

    let run = spec(&home, &root, &["index"]);
    run.code(2);
    assert!(
        run.stdout.starts_with("indexed inkwell: walked "),
        "the md index is printed: {}",
        run.show()
    );
    assert!(code_line(&run).is_none(), "{}", run.show());
    let lines = run.stderr_lines();
    assert_eq!(lines.len(), 1, "{}", run.show());
    let cause = lines[0].strip_prefix("spec: ").unwrap_or(lines[0]);
    let prefix = format!("specengine.toml:{line}: ");
    assert!(
        cause.starts_with(&prefix) && cause.contains("colour"),
        "want {prefix:?} naming the key: {}",
        run.show()
    );
    let have = dump(&home, &root, SLUG);
    assert_eq!(code_lines(&have), before, "no code row changes");
    let fresh = fresh_dump_with_exit(&scratch, &root, SLUG, 2);
    assert_eq!(md_lines(&have), md_lines(&fresh), "the md rows are fresh");
    assert!(
        have.contains("not hashed"),
        "the md edit is indexed despite the bad [code]"
    );
}

// ------------------------------------------------- lenient store, edges

/// "spec refs and spec index": the store is lenient (an unreadable file:
/// a `read_error` row; a non-UTF-8 file: `utf8` 0 and a `note:`), `index`
/// exits 0; reads exit 2 on any gap. An empty code tree counts zero.
#[test]
fn index_is_lenient_where_reads_exit_2() {
    let scratch = Scratch::new("ci-lenient");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    write(
        &root,
        "crates/inkwell/src/latin.rs",
        b"// caf\xe9 TENET-ORDER\npub fn latin() {}\n".as_slice(),
    );
    let locked = root.join("crates/inkwell/src/page.rs");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let run = spec(&home, &root, &["index"]);
    run.code(0);
    let counts = counts_of(&run);
    assert_eq!(counts["unreadable"], 1, "{}", run.show());
    assert_eq!(counts["skipped"], 1, "{}", run.show());
    assert!(
        run.stderr
            .lines()
            .any(|line| line.starts_with("note: ") && line.contains("crates/inkwell/src/latin.rs")),
        "{}",
        run.show()
    );
    let db = db(&home, SLUG);
    let rows: BTreeMap<String, BTreeMap<String, Value>> = code_files(&db)
        .into_iter()
        .map(|row| (row["path"].as_str().unwrap().to_owned(), row))
        .collect();
    assert!(
        rows["crates/inkwell/src/page.rs"]["read_error"].is_string(),
        "{:?}",
        rows["crates/inkwell/src/page.rs"]
    );
    assert_eq!(rows["crates/inkwell/src/latin.rs"]["utf8"], 0);
    let refs = spec(&home, &root, &["refs", "TENET-WIDTH"]);
    refs.code(2);
    assert_eq!(refs.stdout, "", "{}", refs.show());
    assert!(
        refs.stderr.starts_with(
            "spec: the code walk is incomplete: `crates/inkwell/src/page.rs` cannot be read"
        ),
        "{}",
        refs.show()
    );
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).unwrap();

    // An empty code tree: every root missing.
    with_code_table(&root, "[code]\nroots = [\"absent\"]\n");
    let run = spec(&home, &root, &["index"]);
    run.code(0);
    let counts = counts_of(&run);
    for key in ["walked", "parsed", "symbols", "markers", "mentions"] {
        assert_eq!(counts[key], 0, "{key}: {}", run.show());
    }
    assert_eq!(count(&db, "symbols"), 0);
    let refs = spec(&home, &root, &["refs", "TENET-WIDTH"]);
    refs.code(0);
    assert!(
        refs.stdout
            .ends_with("refs: markers 0, mentions 0; code files 0 (.rs 0, .ron 0), skipped 0\n"),
        "{}",
        refs.show()
    );
}

/// "Cache key and cross-file pass": no `[code]`, the worktree's code rows
/// go (the code line and JSON `code` with it); a `[code]` again re-derives
/// them, equal to a fresh index.
#[test]
fn removing_the_code_table_drops_the_code_rows() {
    let scratch = Scratch::new("ci-no-code");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    index(&home, &root);
    let config = read_text(&root, "specengine.toml");
    with_code_table(&root, "");
    let run = index(&home, &root);
    assert!(code_line(&run).is_none(), "{}", run.show());
    let db = db(&home, SLUG);
    for table in CODE_TABLES {
        assert_eq!(count(&db, table), 0, "{table}");
    }
    assert_eq!(dump(&home, &root, SLUG), fresh_dump(&scratch, &root, SLUG));
    write(&root, "specengine.toml", &config);
    let run = index(&home, &root);
    assert_eq!(counts_of(&run)["symbols"], 31, "{}", run.show());
    assert_eq!(dump(&home, &root, SLUG), fresh_dump(&scratch, &root, SLUG));
}

/// The JSON `code` object holds the text line's counts under the same
/// words, `rederived_all` a boolean.
#[test]
fn index_json_code_is_the_code_line() {
    let scratch = Scratch::new("ci-json");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    let json = spec(&home, &root, &["--json", "index", "--full"]).json();
    let code = json["code"].as_object().expect("a code object");
    let text = spec(&home, &root, &["index", "--full"]);
    let counts = counts_of(&text);
    for (word, value) in &counts {
        if word == "rederived_all" {
            assert_eq!(code["rederived_all"], true, "{json}");
        } else {
            assert_eq!(code[word.as_str()], *value, "{word}: {json}");
        }
    }
    assert_eq!(
        counts.get("rederived_all"),
        Some(&1),
        "--full re-derives all"
    );
}

// ----------------------------------------------------------------- RD-15

/// A file's module qpath and its items' (qpath, ambiguity).
type FileNames = (String, Vec<(String, Option<String>)>);

/// The names `name_files` gives code-index's Rust files, from the bytes on
/// disk: one package, `crates/inkwell`, named `inkwell-core`, its layout
/// targets.
fn library_names(root: &Path, paths: &[String]) -> BTreeMap<String, FileNames> {
    let package = Path::new("crates/inkwell");
    let mut parser = RustParser::new().expect("the Rust grammar");
    let mut analyses = Vec::new();
    for path in paths {
        let source = read_text(root, path);
        let tree = parser.parse(&source).expect("a tree");
        analyses.push(bind_tree(&tree, &source).analysis);
    }
    let relative: Vec<PathBuf> = paths
        .iter()
        .map(|path| Path::new(path).strip_prefix(package).unwrap().to_path_buf())
        .collect();
    let table = layout_targets("inkwell-core", relative.iter().map(PathBuf::as_path));
    let tables = BTreeMap::from([(package.to_path_buf(), table)]);
    let files: Vec<NamedFile<'_>> = paths
        .iter()
        .zip(&analyses)
        .map(|(path, analysis)| NamedFile {
            path: Path::new(path),
            package: Some(package),
            analysis,
        })
        .collect();
    let names = name_files(&files, &tables);
    paths
        .iter()
        .enumerate()
        .map(|(at, path)| {
            (
                path.clone(),
                (
                    names.modules[at].qpath.clone(),
                    names.items[at]
                        .iter()
                        .map(|name| {
                            (
                                name.qpath.clone(),
                                name.ambiguity
                                    .map(|ambiguity| ambiguity.as_str().to_owned()),
                            )
                        })
                        .collect(),
                ),
            )
        })
        .collect()
}

/// RD-15 (Q17). M: names from a tree walk. The store answers per file its
/// module qpath and each item's qpath and ambiguity, equal to
/// `name_files` over the indexed bytes; a file edited on disk since keeps
/// its stored names until the next refresh (the read walks no tree).
#[test]
fn stored_names_equal_the_walks_for_one_file() {
    let scratch = Scratch::new("ci-names");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    index(&home, &root);
    let paths: Vec<String> = code_files(&db(&home, SLUG))
        .into_iter()
        .map(|row| row["path"].as_str().unwrap().to_owned())
        .filter(|path| path.ends_with(".rs"))
        .collect();
    assert_eq!(paths.len(), 9);
    let want = library_names(&root, &paths);
    replace_once(
        &root,
        "crates/inkwell/src/twins.rs",
        "// @implements TENET-WIDTH@2\nimpl Sheet {\n    pub fn height(&self) -> u32 {\n        24\n    }\n}\n",
        "",
    );
    let store = specengine_store::SqliteIndex::open(db(&home, SLUG), SLUG, &root).expect("open");
    for path in &paths {
        // The store's per-file names (RD-15, Q17).
        let stored = store
            .stored_names(path)
            .expect("read the names")
            .unwrap_or_else(|| panic!("{path}: stored"));
        let got = (
            stored.module.qpath.clone(),
            stored
                .items
                .iter()
                .map(|name| (name.qpath.clone(), name.ambiguity.clone()))
                .collect::<Vec<_>>(),
        );
        assert_eq!(&got, &want[path], "{path}");
    }
    let twins = &want["crates/inkwell/src/twins.rs"].1;
    assert!(
        twins
            .iter()
            .any(|(qpath, _)| qpath == "crates/inkwell::twins::impl Sheet #2"),
        "the oracle names the twin the disk no longer holds: {twins:?}"
    );
    let joined = &want["crates/inkwell/src/split.rs"].1;
    assert_eq!(
        joined,
        &vec![(
            "crates/inkwell::split::joined".to_owned(),
            Some("duplicate".to_owned())
        )]
    );
}

fn replace_once(root: &Path, relative: &str, from: &str, to: &str) {
    let text = read_text(root, relative);
    assert_eq!(text.matches(from).count(), 1, "{relative}: {from:?}");
    write(root, relative, text.replacen(from, to, 1));
}

/// Every table name the dump prints is one `sqlite_master` holds: the
/// dump covers each code table (`dump.rs` maps code keys to `path`,
/// `(path, ord)`) and shows no surrogate key or rowid. code-index holds no
/// `#[path]` declaration and no framework role (`code_roles` by path:
/// `bevy_roles.rs`, store `format.rs`).
#[test]
fn the_dump_covers_every_code_table_by_path() {
    let scratch = Scratch::new("ci-dump");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    index(&home, &root);
    let text = dump(&home, &root, SLUG);
    let tables: BTreeSet<&str> = text
        .lines()
        .map(|line| line.split('\t').next().unwrap())
        .collect();
    for table in CODE_TABLES {
        if table == "code_decls" || table == "code_roles" {
            continue;
        }
        assert!(tables.contains(table), "{table} not dumped");
    }
    for line in code_lines(&text).lines() {
        if line.starts_with("symbols\t") || line.starts_with("symbol_suffixes\t") {
            assert!(
                line.contains("\"crates/inkwell/"),
                "a per-file row names its path: {line}"
            );
        }
    }
}
