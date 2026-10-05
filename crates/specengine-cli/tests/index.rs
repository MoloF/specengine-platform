//! AC-07 of docs/features/spec-cli.md: `spec index` is the store's
//! `update` (`--full`: `rebuild`), one summary line and the `db` line; its
//! JSON is `project`, `db` and the `UpdateReport` fields; broken,
//! non-UTF-8 and unreadable files are counted, never fatal.

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use common::bundle::blake3_hex;
use common::{FIXTURES, Scratch, assert_plain, data_dir, md_files, spec, write};
use specengine_store::UpdateReport;

fn summary(
    slug: &str,
    walked: usize,
    parsed: usize,
    unchanged: usize,
    removed: usize,
    unreadable: usize,
) -> String {
    format!(
        "indexed {slug}: walked {walked}, parsed {parsed}, unchanged {unchanged}, removed {removed}, unreadable {unreadable}"
    )
}

#[test]
fn index_counts_then_reruns_then_reparses_one_then_rebuilds() {
    for (fixture, slug) in FIXTURES {
        let scratch = Scratch::new("index");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let files = md_files(&root.join("docs")).len();
        if fixture == "spec-a" {
            assert_eq!(files, 13, "spec-a has 13 documents");
        }
        let db = data_dir(&home).join(format!("{slug}.db"));
        let db_line = format!("db {}\n", db.display());

        let first = spec(&home, &root, &["index"]);
        first.code(0);
        assert_eq!(
            first.stdout,
            format!("{}\n{db_line}", summary(slug, files, files, 0, 0, 0)),
            "{fixture}"
        );
        assert_eq!(first.stderr, "", "{fixture}: {}", first.show());
        assert_plain(&first.stdout, fixture);

        let again = spec(&home, &root, &["index"]);
        again.code(0);
        assert_eq!(
            again.stdout,
            format!("{}\n{db_line}", summary(slug, files, 0, files, 0, 0)),
            "{fixture}"
        );

        let one = md_files(&root.join("docs"))[0].clone();
        let path = root.join("docs").join(&one);
        let mut bytes = fs::read(&path).unwrap();
        bytes.extend_from_slice(b"\nOne more line.\n");
        fs::write(&path, bytes).unwrap();
        let edited = spec(&home, &root, &["index"]);
        edited.code(0);
        assert_eq!(
            edited.stdout,
            format!("{}\n{db_line}", summary(slug, files, 1, files - 1, 0, 0)),
            "{fixture}: docs/{one} edited"
        );

        let full = spec(&home, &root, &["index", "--full"]);
        full.code(0);
        assert_eq!(
            full.stdout,
            format!(
                "{}, reparsed all\n{db_line}",
                summary(slug, files, files, 0, 0, 0)
            ),
            "{fixture}: --full"
        );
        let full_json = spec(&home, &root, &["--json", "index", "--full"]).json();
        assert_eq!(full_json["reparsed_all"], true, "{fixture}: {full_json}");
        assert_eq!(full_json["parsed"], files, "{fixture}: {full_json}");

        fs::remove_file(&path).unwrap();
        let removed = spec(&home, &root, &["index"]);
        removed.code(0);
        assert_eq!(
            removed.stdout,
            format!(
                "{}\n{db_line}",
                summary(slug, files - 1, 0, files - 1, 1, 0)
            ),
            "{fixture}: docs/{one} removed"
        );
    }
}

/// `--json`: exactly `project`, `db` and the `UpdateReport` keys, with the
/// same values as the text.
#[test]
fn index_json_is_project_db_and_the_update_report() {
    for (fixture, slug) in FIXTURES {
        let scratch = Scratch::new("index-json");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let json = spec(&home, &root, &["--json", "index"]).json();
        let report = serde_json::to_value(UpdateReport::default()).unwrap();
        let mut want: BTreeSet<String> = report.as_object().unwrap().keys().cloned().collect();
        want.insert("project".to_owned());
        want.insert("db".to_owned());
        let got: BTreeSet<String> = json
            .as_object()
            .expect("an object")
            .keys()
            .cloned()
            .collect();
        assert_eq!(got, want, "{fixture}: {json}");
        assert_eq!(json["project"], slug);
        assert_eq!(
            json["db"],
            data_dir(&home)
                .join(format!("{slug}.db"))
                .display()
                .to_string()
        );
        let files = md_files(&root.join("docs")).len();
        assert_eq!(json["walked"], files);
        assert_eq!(json["parsed"], files);
        assert_eq!(json["reparsed_all"], false);
        for key in ["unchanged", "removed", "unreadable", "skipped_names"] {
            assert_eq!(json[key], 0, "{fixture}: {key}");
        }
        assert!(json["missing_roots"].is_array() && json["unreadable_dirs"].is_array());
    }
}

/// An unclosed front-matter, a non-UTF-8 file and an unreadable file:
/// exit 0, each counted; `show` of the broken ones still answers.
#[test]
fn broken_files_are_counted_never_fatal() {
    for (fixture, slug) in FIXTURES {
        let scratch = Scratch::new("index-broken");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let files = md_files(&root.join("docs")).len();
        write(
            &root,
            "docs/records/broken/unclosed.md",
            "---\nclass: canon\n\n# Never closed\n\nText.\n",
        );
        write(
            &root,
            "docs/records/broken/bytes.md",
            b"# Bad \xff\xfe bytes\n\nMore.\n".as_slice(),
        );
        write(&root, "docs/records/broken/locked.md", "# Locked\n");
        let locked = root.join("docs/records/broken/locked.md");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

        let run = spec(&home, &root, &["index"]);
        run.code(0);
        assert_eq!(
            run.stdout.lines().next(),
            Some(summary(slug, files + 3, files + 2, 0, 0, 1).as_str()),
            "{fixture}\n{}",
            run.show()
        );
        let json = spec(&home, &root, &["--json", "index", "--full"]).json();
        assert_eq!(json["walked"], files + 3, "{fixture}: {json}");
        assert_eq!(json["unreadable"], 1, "{fixture}: {json}");

        let unclosed = spec(&home, &root, &["show", "docs/records/broken/unclosed.md"]);
        unclosed.code(0);
        assert!(
            unclosed.stdout.contains("Never closed"),
            "{fixture}: {}",
            unclosed.show()
        );
        let bytes = spec(&home, &root, &["show", "docs/records/broken/bytes.md"]);
        bytes.code(0);
        let header = bytes.stdout.lines().next().unwrap();
        // docs/features/proposal-apply.md: the header ends with the span
        // hash of the raw bytes (the whole file), after the flags.
        let hash = format!(
            "b3:{}",
            blake3_hex(b"# Bad \xff\xfe bytes\n\nMore.\n".as_slice())
        );
        assert!(
            header.ends_with(&format!(" | not UTF-8 | span {hash}")),
            "{fixture}: {}",
            bytes.show()
        );
        assert!(
            bytes.stdout.contains("# Bad \u{fffd}\u{fffd} bytes"),
            "{}",
            bytes.show()
        );
        let json = spec(
            &home,
            &root,
            &["--json", "show", "docs/records/broken/bytes.md"],
        )
        .json();
        assert_eq!(json["nodes"][0]["utf8"], false, "{json}");

        // Searches still answer with the broken files in the index.
        spec(&home, &root, &["search", "more"]).code(0);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).unwrap();
    }
}

/// A `[paths]` root that is missing is a `warning:` when `[paths]` is
/// written, silent otherwise; never exit 2.
#[test]
fn missing_roots_warn_only_when_paths_are_written() {
    let scratch = Scratch::new("index-missing");
    let home = scratch.home("h");
    // spec-a writes no [paths]: its default roots include a missing one.
    let a = scratch.copy("spec-a", "a");
    let run = spec(&home, &a, &["index"]);
    run.code(0);
    assert_eq!(run.stderr, "", "{}", run.show());
    // spec-b writes [paths]: a missing root is a warning.
    let b = scratch.copy("spec-b", "b");
    let text = fs::read_to_string(b.join("specengine.toml")).unwrap();
    let text = text.replacen("roots = [\"docs\"]", "roots = [\"docs\", \"gone\"]", 1);
    fs::write(b.join("specengine.toml"), text).unwrap();
    for args in [&["index"][..], &["search", "dry-run"], &["show", "REQ-001"]] {
        let run = spec(&home, &b, args);
        run.code(0);
        let lines = run.stderr_lines();
        assert_eq!(lines.len(), 1, "{args:?}: {}", run.show());
        assert!(
            lines[0].starts_with("warning: ") && lines[0].contains("gone"),
            "{args:?}: {}",
            run.show()
        );
        assert!(
            !run.stdout.contains("warning"),
            "{args:?}: warnings stay off stdout"
        );
    }
}
