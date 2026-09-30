//! AC-08 of docs/features/spec-cli.md: `search` and `show` run `update`
//! first, so an edit or a deletion after `spec index` is seen at once.

#![cfg(unix)]

mod common;

use std::fs;

use common::{Scratch, index, replace, spec};

/// `(fixture, document, its ID, a line of its own text, a record to
/// delete, its ID)`.
const CASES: [(&str, &str, &str, &str, &str, &str); 2] = [
    (
        "spec-a",
        "docs/spec/movement/stamina.md",
        "MEC-STAMINA",
        "Stamina limits sprinting.",
        "docs/records/R/R-12.md",
        "R-12",
    ),
    (
        "spec-b",
        "docs/records/REQ/REQ-001.md",
        "REQ-001",
        "\u{041a}\u{043e}\u{043c}\u{0430}\u{043d}\u{0434}\u{0430} ",
        "docs/records/REQ/REQ-002.md",
        "REQ-002",
    ),
];

#[test]
fn a_word_added_after_index_is_found_by_search_in_its_node() {
    for (fixture, document, id, anchor, _, _) in CASES {
        let scratch = Scratch::new("fresh-search");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        index(&home, &root);
        let word = "quillfeatherous";
        let before = spec(&home, &root, &["search", word]);
        before.code(0);
        assert!(
            before.stdout.starts_with("hits 0 "),
            "{fixture}: {}",
            before.show()
        );

        // Inside the document's own text, not in a later section.
        replace(&root, document, anchor, &format!("{word} {anchor}"));
        let run = spec(&home, &root, &["--json", "search", word]);
        run.code(0);
        let json = run.json();
        let ids: Vec<&str> = json["hits"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|hit| hit["id"].as_str())
            .collect();
        assert!(
            ids.contains(&id),
            "{fixture}: {id} among {ids:?}\n{}",
            run.show()
        );
    }
}

#[test]
fn a_record_deleted_after_index_is_not_found_by_show() {
    for (fixture, _, _, _, record, record_id) in CASES {
        let scratch = Scratch::new("fresh-show");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        index(&home, &root);
        spec(&home, &root, &["show", record_id]).code(0);
        fs::remove_file(root.join(record)).unwrap();
        let run = spec(&home, &root, &["show", record_id]);
        run.code(1);
        assert_eq!(run.stdout, "", "{fixture}");
        assert!(
            run.stderr.starts_with("spec: "),
            "{fixture}: {}",
            run.show()
        );
        let json = spec(&home, &root, &["--json", "show", record_id]);
        json.code(1);
        let json = json.json();
        assert_eq!(json["nodes"], serde_json::json!([]), "{fixture}");
        assert!(json["reason"].is_string(), "{fixture}: {json}");
        // By path, too.
        spec(&home, &root, &["show", record]).code(1);
    }
}

/// `show` prints the bytes on disk now, not those of the last `index`.
#[test]
fn show_prints_an_edit_made_after_index() {
    let scratch = Scratch::new("fresh-edit");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    index(&home, &root);
    replace(
        &root,
        "docs/spec/movement/stamina.md",
        "- While `Exhausted`, rate × 0.5.",
        "- While `Exhausted`, rate × 0.25.",
    );
    let run = spec(&home, &root, &["show", "RULE-STAM-REGEN"]);
    run.code(0);
    assert!(run.stdout.contains("rate × 0.25."), "{}", run.show());
    // A new ID added after index resolves.
    replace(
        &root,
        "docs/spec/movement/stamina.md",
        "## Depletion {#EDGE-STAM-ZERO}",
        "## Recovery {#RULE-STAM-RECOVER}\n- New.\n\n## Depletion {#EDGE-STAM-ZERO}",
    );
    let added = spec(&home, &root, &["show", "RULE-STAM-RECOVER"]);
    added.code(0);
    assert!(
        added
            .stdout
            .starts_with("RULE-STAM-RECOVER | rule | Recovery | "),
        "{}",
        added.show()
    );
}
