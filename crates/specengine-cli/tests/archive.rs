//! AC-11 of docs/features/spec-cli.md (ADR-0022, "reachable by id"):
//! `spec search` drops Tier 3 files' nodes in the query, before the limit,
//! and counts them, unless `--archive`; `spec show` reaches a Tier 3 node
//! and marks it.

#![cfg(unix)]

mod common;

use common::{Scratch, header_without_span, index, spec, write};

/// `(fixture, a word found only in its Tier 3 record, that record's ID)`.
const CASES: [(&str, &str, &str); 2] = [
    ("spec-a", "replaced", "DEC-0007"),
    // "klony" (clones), only in ADR-0002 (superseded).
    (
        "spec-b",
        "\u{043a}\u{043b}\u{043e}\u{043d}\u{044b}",
        "ADR-0002",
    ),
];

#[test]
fn a_word_only_in_a_superseded_record_is_left_out_and_counted() {
    for (fixture, word, id) in CASES {
        let scratch = Scratch::new("archive");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        index(&home, &root);

        let run = spec(&home, &root, &["search", word]);
        run.code(0);
        assert_eq!(
            run.stdout, "hits 0 (limit 20); archived matches left out: 1 (--archive)\n",
            "{fixture}"
        );
        assert_eq!(run.stderr, "", "{fixture}");
        let json = spec(&home, &root, &["--json", "search", word]).json();
        assert_eq!(json["hits"], serde_json::json!([]), "{fixture}");
        assert_eq!(json["tier3_left_out"], 1, "{fixture}");
        assert_eq!(json["archive"], false, "{fixture}");

        let run = spec(&home, &root, &["search", word, "--archive"]);
        run.code(0);
        let lines: Vec<&str> = run.stdout.lines().collect();
        assert_eq!(lines.len(), 3, "{fixture}: one hit\n{}", run.show());
        assert!(
            lines[0].starts_with(&format!("{id} | decision | "))
                && lines[0].ends_with(":1 | archived"),
            "{fixture}: {}",
            run.show()
        );
        assert_eq!(lines[2], "hits 1 (limit 20); archive included", "{fixture}");
        let json = spec(&home, &root, &["--json", "search", word, "--archive"]).json();
        assert_eq!(json["hits"][0]["archived"], true, "{fixture}");
        assert_eq!(json["hits"][0]["id"], id, "{fixture}");
        assert_eq!(json["tier3_left_out"], 0, "{fixture}");
        assert_eq!(json["archive"], true, "{fixture}");

        let run = spec(&home, &root, &["show", id]);
        run.code(0);
        // The flag is the last field before the span hash
        // (docs/features/proposal-apply.md).
        assert!(
            header_without_span(run.stdout.lines().next().unwrap()).ends_with(" | archived"),
            "{fixture}: {}",
            run.show()
        );
        let json = spec(&home, &root, &["--json", "show", id]).json();
        assert_eq!(json["nodes"][0]["archived"], true, "{fixture}");
    }
}

/// Three archived nodes outrank three live ones for a term: `--limit 3`
/// gives the live three (the filter runs before the limit), and the
/// summary counts the three left out.
#[test]
fn the_archive_filter_runs_before_the_limit() {
    for (fixture, _, _) in CASES {
        let scratch = Scratch::new("archive-limit");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let term = "obsidianwick";
        let filler =
            "Plain words about something else entirely, repeated to dilute the match. ".repeat(20);
        for n in 1..=3 {
            write(
                &root,
                &format!("docs/records/old/old-{n}.md"),
                format!(
                    "---\nclass: decision\nstatus: rejected\n---\n\n# {term} {term} {term} {n}\n\n{term} {term} {term}.\n"
                ),
            );
            write(
                &root,
                &format!("docs/records/live/live-{n}.md"),
                format!(
                    "---\nclass: canon\n---\n\n# Live note {n}\n\n{filler}\nOnce: {term}.\n\n{filler}\n"
                ),
            );
        }
        index(&home, &root);

        // The precondition: with the archive, the three archived rank first.
        let json = spec(
            &home,
            &root,
            &["--json", "search", term, "--archive", "--limit", "3"],
        )
        .json();
        let paths: Vec<&str> = json["hits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|hit| hit["path"].as_str().unwrap())
            .collect();
        assert_eq!(
            paths,
            [
                "docs/records/old/old-1.md",
                "docs/records/old/old-2.md",
                "docs/records/old/old-3.md"
            ],
            "{fixture}: the archived outrank the live"
        );

        let run = spec(&home, &root, &["--json", "search", term, "--limit", "3"]);
        run.code(0);
        let json = run.json();
        let mut paths: Vec<&str> = json["hits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|hit| hit["path"].as_str().unwrap())
            .collect();
        paths.sort_unstable();
        assert_eq!(
            paths,
            [
                "docs/records/live/live-1.md",
                "docs/records/live/live-2.md",
                "docs/records/live/live-3.md"
            ],
            "{fixture}: --limit 3 gives the live three\n{}",
            run.show()
        );
        assert!(
            json["hits"]
                .as_array()
                .unwrap()
                .iter()
                .all(|hit| hit["archived"] == false)
        );
        assert_eq!(json["tier3_left_out"], 3, "{fixture}");
        let text = spec(&home, &root, &["search", term, "--limit", "3"]);
        assert!(
            text.stdout
                .ends_with("hits 3 (limit 3); archived matches left out: 3 (--archive)\n"),
            "{fixture}: {}",
            text.show()
        );
    }
}
