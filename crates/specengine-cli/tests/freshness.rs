//! AC-08 of docs/features/spec-cli.md: `search` and `show` run `update`
//! first, so an edit or a deletion after `spec index` is seen at once.

#![cfg(unix)]

mod common;

use std::fs;

use common::graph::{link_lines, spec30, summary_line, tree_depths};
use common::{Scratch, index, replace, spec};

const SPRINT: &str = "docs/spec/movement/sprint.md";

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

/// docs/features/spec-cli-graph.md AC-17: `tree`, `graph` and `show
/// --links` run `update` first: a `parent:` edit, a new child document and
/// a new link made after `spec index` show at once. M: `update` skipped.
#[test]
fn graph_reads_see_edits_made_after_index() {
    let scratch = Scratch::new("fresh-graph");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    index(&home, &root);
    let tree = |args: &[&str]| {
        let run = spec(&home, &root, args);
        run.code(0);
        assert_eq!(run.stderr, "", "{}", run.show());
        run.stdout
    };
    assert!(tree(&["tree"]).contains("\n    MEC-SPRINT | "));
    // The same size, so only `update` can see it.
    replace(
        &root,
        "docs/spec/movement/sprint.md",
        "parent: DOM-MOVEMENT\nowner: owner\n",
        "parent: DOM-GAME\nowner: owner1234\n",
    );
    let after = tree(&["tree"]);
    assert!(
        after.contains("\n  MEC-SPRINT | mechanic | Sprint | docs/spec/movement/sprint.md:1\n"),
        "the edited parent:\n{after}"
    );
    let json = spec(&home, &root, &["--json", "tree", "MEC-SPRINT"]).json();
    assert_eq!(json["nodes"].as_array().unwrap().len(), 3, "{json}");
    // A new link, then gone again.
    replace(
        &root,
        "docs/spec/movement/stamina.md",
        "  depends_on: [MEC-SPRINT]\n",
        "  depends_on: [MEC-SPRINT, R-12]\n",
    );
    assert!(
        tree(&["show", "R-12", "--links"])
            .contains("  in depends_on MEC-STAMINA | docs/spec/movement/stamina.md:13\n")
    );
    assert!(
        tree(&["graph", "R-12", "--impact", "--type", "depends_on"]).contains("\n1 MEC-STAMINA | ")
    );
    fs::remove_file(root.join("docs/spec/movement/stamina.md")).unwrap();
    let run = spec(&home, &root, &["graph", "R-12", "--impact"]);
    run.code(0);
    assert!(!run.stdout.contains("MEC-STAMINA"), "{}", run.show());

    // spec-b: a child document written after `spec index`.
    let root = scratch.copy("spec-b", "copy-b");
    let home = scratch.home("b");
    index(&home, &root);
    common::write(
        &root,
        "docs/spec/pull.md",
        "---\nid: CMD-PULL\nclass: canon\nparent: MOD-CLI\n---\n\n# Pull\n",
    );
    let run = spec(&home, &root, &["tree"]);
    run.code(0);
    assert!(
        run.stdout
            .contains("\n  CMD-PULL | command | Pull | docs/spec/pull.md:1\nnodes 5, roots 1\n"),
        "{}",
        run.show()
    );
}

/// AC-17: a new child document written after a read (the index exists)
/// and in a project never indexed shows in `tree`, `show --links` and
/// `graph --impact` at once, no `spec index` run. M: `update` skipped.
#[test]
fn a_new_child_document_shows_without_spec_index() {
    let scratch = Scratch::new("fresh-child");
    let child = "---\nid: MEC-DASH\nclass: canon\nparent: MEC-STAMINA\nlinks:\n  depends_on: [MEC-STAMINA]\n---\n\n# Dash\n";
    for first_read in [true, false] {
        let home = scratch.home(if first_read { "read" } else { "never" });
        let root = scratch.copy("spec-a", if first_read { "read" } else { "never" });
        if first_read {
            // A read builds the index; the child comes after it.
            let run = spec30(&home, &root, &["tree"]);
            run.code(0);
            assert!(!run.stdout.contains("MEC-DASH"), "{}", run.show());
        }
        common::write(&root, "docs/spec/movement/dash.md", child);
        let run = spec30(&home, &root, &["tree", "MEC-STAMINA"]);
        run.code(0);
        assert_eq!(run.stderr, "", "{}", run.show());
        assert_eq!(
            tree_depths(&run.stdout),
            [
                (0, "MEC-STAMINA".to_owned()),
                (1, "RULE-STAM-REGEN".to_owned()),
                (1, "EDGE-STAM-ZERO".to_owned()),
                (1, "MEC-DASH".to_owned()),
            ],
            "{}",
            run.show()
        );
        assert!(
            run.stdout
                .contains("\n  MEC-DASH | mechanic | Dash | docs/spec/movement/dash.md:1\n"),
            "{}",
            run.show()
        );
        let run = spec30(&home, &root, &["tree"]);
        run.code(0);
        assert_eq!(
            summary_line(&run.stdout),
            "nodes 11, roots 1",
            "{}",
            run.show()
        );
        let run = spec30(&home, &root, &["show", "MEC-STAMINA", "--links"]);
        run.code(0);
        assert!(
            link_lines(&run.stdout)
                .contains(&"  in depends_on MEC-DASH | docs/spec/movement/dash.md:6"),
            "{}",
            run.show()
        );
        let run = spec30(&home, &root, &["graph", "MEC-STAMINA", "--impact"]);
        run.code(0);
        assert!(
            run.stdout
                .contains("\n1 MEC-DASH | mechanic | Dash | docs/spec/movement/dash.md:1\n"),
            "{}",
            run.show()
        );
    }
}

/// sprint.md with `parent: DOM-GAME` and four more blank lines before its
/// title: the same size as the fixture's, every byte offset after the
/// title the same, every line after it four lower.
fn sprint_moved(original: &str) -> String {
    let moved = original
        .replacen("parent: DOM-MOVEMENT\n", "parent: DOM-GAME\n", 1)
        .replacen("\n# Sprint\n", "\n\n\n\n\n# Sprint\n", 1);
    assert_eq!(moved.len(), original.len());
    moved
}

/// AC-17 and "Data" ("no size heuristic"): while sprint.md is swapped
/// (atomic rename) between two same-size versions, one with `parent:
/// DOM-MOVEMENT`, the other with `parent: DOM-GAME` and its sections four
/// lines lower at the same byte offsets, every `spec tree` prints exactly
/// one version's tree: never a parent from one version with lines from
/// the other (an edit landing between `update` and the re-read). The race
/// can only be provoked, not placed, so the mutation's red is a high
/// probability over the runs (iteration 1 mixed 71 of 200). M: the
/// re-parse removed, a same-size file kept with its indexed parse.
#[test]
fn a_same_size_edit_racing_the_read_is_never_mixed() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    let scratch = Scratch::new("fresh-race");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let path = root.join(SPRINT);
    let original = fs::read_to_string(&path).unwrap();
    let moved = sprint_moved(&original);
    let at_rest = spec30(&home, &root, &["tree"]);
    at_rest.code(0);
    fs::write(&path, &moved).unwrap();
    let moved_tree = spec30(&home, &root, &["tree"]);
    moved_tree.code(0);
    assert!(
        moved_tree
            .stdout
            .contains("\n  MEC-SPRINT | mechanic | Sprint | docs/spec/movement/sprint.md:1\n    RULE-SPRINT-COST | rule | Cost | docs/spec/movement/sprint.md:22\n"),
        "{}",
        moved_tree.show()
    );
    fs::write(&path, &original).unwrap();

    let stop = Arc::new(AtomicBool::new(false));
    let flipper = {
        let stop = Arc::clone(&stop);
        let (path, original, moved) = (path.clone(), original.clone(), moved.clone());
        let temp = path.with_extension("swap");
        std::thread::spawn(move || {
            let mut flips = 0usize;
            while !stop.load(Ordering::Relaxed) {
                let text = if flips.is_multiple_of(2) {
                    &moved
                } else {
                    &original
                };
                fs::write(&temp, text).unwrap();
                fs::rename(&temp, &path).unwrap();
                flips += 1;
                std::thread::sleep(Duration::from_micros(200));
            }
            flips
        })
    };
    let started = Instant::now();
    let (mut rest, mut moved_seen, mut mixed) = (0, 0, Vec::new());
    let mut runs = 0;
    while runs < 150 && started.elapsed() < Duration::from_secs(40) {
        let run = spec30(&home, &root, &["tree"]);
        runs += 1;
        if run.code == 0 && run.stdout == at_rest.stdout && run.stderr.is_empty() {
            rest += 1;
        } else if run.code == 0 && run.stdout == moved_tree.stdout && run.stderr.is_empty() {
            moved_seen += 1;
        } else {
            mixed.push(run.show());
        }
    }
    stop.store(true, Ordering::Relaxed);
    let flips = flipper.join().expect("the flipper");
    fs::write(&path, &original).unwrap();
    assert!(
        mixed.is_empty(),
        "{} of {runs} runs mixed the two versions ({flips} flips); first:\n{}",
        mixed.len(),
        mixed[0]
    );
    assert!(
        rest > 0 && moved_seen > 0,
        "the race was not exercised: {rest} at rest, {moved_seen} moved of {runs}"
    );
}
