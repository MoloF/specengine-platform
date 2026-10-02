//! AC-02 … AC-07 of docs/features/spec-cli-graph.md: `spec tree [ROOT]
//! [--depth N] [--kind K]… [--archive]` lists containment only (`parent:`,
//! section nesting), pre-order, a node's nested sections before its child
//! documents; default roots are the live documents under `[paths] spec`
//! with no resolving parent; dangling parents, several holders and
//! `parent:` cycles are answered (exit 0), never hung on. Scratch copies of
//! spec-a and spec-b (and of this repository's documents) under their own
//! `HOME`; every run that could loop is killed after 30 s.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use common::graph::{field, item_lines, keys, repository_copy, spec30, summary_line, tree_depths};
use common::{Scratch, replace, spec, write};
use serde_json::Value;

/// `spec tree` on `fixtures/spec-a`: the spec's "Data" example.
const EXAMPLE: &str = "\
DOM-GAME | domain | Lantern Keep | docs/spec/game.md:1 | status accepted
  RULE-CORE-LOOP | rule | Core loop | docs/spec/game.md:21
  DOM-MOVEMENT | domain | Movement | docs/spec/movement/README.md:1 | status accepted
    RULE-MOVE-SPEEDS | rule | Speeds | docs/spec/movement/README.md:16
    MEC-SPRINT | mechanic | Sprint | docs/spec/movement/sprint.md:1
      RULE-SPRINT-COST | rule | Cost | docs/spec/movement/sprint.md:18
        EDGE-SPRINT-EMPTY | edge-case | Empty tank | docs/spec/movement/sprint.md:22
    MEC-STAMINA | mechanic | Stamina | docs/spec/movement/stamina.md:1 | status accepted
      RULE-STAM-REGEN | rule | Regeneration | docs/spec/movement/stamina.md:21
      EDGE-STAM-ZERO | edge-case | Depletion | docs/spec/movement/stamina.md:25
nodes 10, roots 1
";

const SPRINT: &str = "docs/spec/movement/sprint.md";
const STAMINA: &str = "docs/spec/movement/stamina.md";
const MOVEMENT: &str = "docs/spec/movement/README.md";

/// `(id, depth, parent, mark)` of every JSON tree node.
fn json_rows(json: &Value) -> Vec<(String, u64, Option<String>, Option<String>)> {
    json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| {
            (
                node["id"]
                    .as_str()
                    .or(node["path"].as_str())
                    .unwrap()
                    .to_owned(),
                node["depth"].as_u64().unwrap(),
                node["parent"].as_str().map(str::to_owned),
                node["mark"].as_str().map(str::to_owned),
            )
        })
        .collect()
}

/// The JSON node list agrees with the text lines: same names, same depths,
/// and each node's `parent` is the nearest earlier node one level up.
fn assert_json_matches_text(json: &Value, stdout: &str, context: &str) {
    let rows = json_rows(json);
    let text: Vec<(usize, String)> = tree_depths(stdout);
    assert_eq!(
        rows.iter()
            .map(|(name, depth, _, _)| (*depth as usize, name.clone()))
            .collect::<Vec<_>>(),
        text,
        "{context}: JSON nodes vs text lines"
    );
    for (index, (name, depth, parent, _)) in rows.iter().enumerate() {
        let above = rows[..index]
            .iter()
            .rev()
            .find(|(_, other, _, _)| *other + 1 == *depth)
            .map(|(other, _, _, _)| other.clone());
        if *depth == 0 {
            assert!(parent.is_none(), "{context}: root {name} has a parent");
        } else {
            assert_eq!(parent, &above, "{context}: parent of {name}");
        }
    }
}

/// AC-02: spec-a's whole tree is exactly the example, from the root and
/// from below it; the JSON holds the same nodes with exactly the keys of
/// "Data". M: a section's parent taken as its document; every parentless
/// document a root.
#[test]
fn the_spec_a_tree_is_exactly_the_example() {
    let scratch = Scratch::new("tree-example");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    for cwd in [root.clone(), root.join("docs/spec/movement")] {
        let run = spec30(&home, &cwd, &["tree"]);
        run.code(0);
        assert_eq!(run.stdout, EXAMPLE, "{}", run.show());
        assert_eq!(run.stderr, "", "{}", run.show());
    }
    let run = spec30(&home, &root, &["--json", "tree"]);
    run.code(0);
    let json = run.json();
    assert_eq!(
        keys(&json),
        [
            "ref",
            "reason",
            "notes",
            "depth",
            "kinds",
            "archive",
            "left_out",
            "truncated",
            "nodes"
        ]
        .into(),
        "{json}"
    );
    assert!(json["ref"].is_null() && json["reason"].is_null(), "{json}");
    assert!(json["depth"].is_null(), "{json}");
    assert_eq!(json["kinds"], serde_json::json!([]));
    assert_eq!(json["archive"], false);
    assert_eq!(json["truncated"], false);
    assert_eq!(json["notes"], serde_json::json!([]));
    assert_eq!(
        json["left_out"],
        serde_json::json!({"generated": 0, "tier3": 0})
    );
    for node in json["nodes"].as_array().unwrap() {
        assert_eq!(
            keys(node),
            [
                "id",
                "kind",
                "title",
                "path",
                "line",
                "depth",
                "parent",
                "mark",
                "status",
                "rev",
                "tokens_est",
                "archived"
            ]
            .into(),
            "{node}"
        );
        assert_eq!(node["archived"], false, "{node}");
        assert!(node["mark"].is_null(), "{node}");
    }
    assert_json_matches_text(&json, EXAMPLE, "spec-a");
    let speeds = &json["nodes"][3];
    assert_eq!(speeds["id"], "RULE-MOVE-SPEEDS");
    assert_eq!(speeds["rev"], 2);
    assert_eq!(speeds["line"], 16);
    assert_eq!(speeds["path"], MOVEMENT);
    assert!(speeds["status"].is_null());
    assert_eq!(json["nodes"][0]["status"], "accepted");
    assert!(json["nodes"][0]["tokens_est"].as_u64().unwrap() > 0);
}

/// AC-03: spec-b (Russian CLI tooling, `roots`, other prefixes and kinds)
/// gives MOD-CLI 0, CMD-SYNC 1, FLAG-DRY-RUN 2, CMD-STATUS 1; this
/// repository (no document under `[paths] spec`) gives `nodes 0, roots 0`,
/// exit 0 and the note. M: a root rule by kind, prefix or path literal.
#[test]
fn spec_b_and_this_repository_find_their_own_roots() {
    let scratch = Scratch::new("tree-genre");
    let home = scratch.home("h");
    let root = scratch.copy("spec-b", "copy");
    let run = spec30(&home, &root, &["tree"]);
    run.code(0);
    assert_eq!(run.stderr, "", "{}", run.show());
    assert_eq!(
        tree_depths(&run.stdout),
        [
            (0, "MOD-CLI".to_owned()),
            (1, "CMD-SYNC".to_owned()),
            (2, "FLAG-DRY-RUN".to_owned()),
            (1, "CMD-STATUS".to_owned()),
        ],
        "{}",
        run.show()
    );
    assert_eq!(
        item_lines(&run.stdout)[0],
        "MOD-CLI | module | \u{041a}\u{043e}\u{043c}\u{0430}\u{043d}\u{0434}\u{043d}\u{0430}\u{044f} \u{0441}\u{0442}\u{0440}\u{043e}\u{043a}\u{0430} | docs/spec/cli.md:1 | status accepted"
    );
    assert_eq!(summary_line(&run.stdout), "nodes 4, roots 1");
    let json = spec30(&home, &root, &["--json", "tree"]).json();
    assert_json_matches_text(&json, &run.stdout, "spec-b");

    // This repository: a scratch copy of its walked documents with a slug.
    let repository = repository_copy(&scratch, "repository");
    let home = scratch.home("repository");
    let run = spec30(&home, &repository, &["tree"]);
    run.code(0);
    assert_eq!(run.stdout, "nodes 0, roots 0\n", "{}", run.show());
    assert_eq!(
        run.stderr,
        "note: no document under [paths] spec \"docs/spec\"; give a ROOT\n",
        "{}",
        run.show()
    );
    let json = spec30(&home, &repository, &["--json", "tree"]).json();
    assert_eq!(json["nodes"], serde_json::json!([]), "{json}");
    assert_eq!(
        json["notes"],
        serde_json::json!(["no document under [paths] spec \"docs/spec\"; give a ROOT"]),
        "{json}"
    );
    // A ROOT reaches anything there.
    let run = spec30(&home, &repository, &["tree", "docs/canon/architecture.md"]);
    run.code(0);
    assert!(
        item_lines(&run.stdout)[0].starts_with("docs/canon/architecture.md | - | "),
        "{}",
        run.show()
    );
}

/// AC-04: ROOT as an ID, a section and a path; `--depth` counts from ROOT;
/// an unknown ROOT is exit 1 with a JSON `reason`; a depth that is no
/// integer ≥ 0 is exit 2 without JSON. M: depth from the corpus root.
#[test]
fn a_root_and_a_depth_bound_the_tree() {
    let scratch = Scratch::new("tree-root");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let run = spec30(&home, &root, &["tree", "DOM-MOVEMENT", "--depth", "1"]);
    run.code(0);
    assert_eq!(
        run.stdout,
        "\
DOM-MOVEMENT | domain | Movement | docs/spec/movement/README.md:1 | status accepted
  RULE-MOVE-SPEEDS | rule | Speeds | docs/spec/movement/README.md:16
  MEC-SPRINT | mechanic | Sprint | docs/spec/movement/sprint.md:1
  MEC-STAMINA | mechanic | Stamina | docs/spec/movement/stamina.md:1 | status accepted
nodes 4, roots 1
",
        "{}",
        run.show()
    );
    let json = spec30(
        &home,
        &root,
        &["--json", "tree", "DOM-MOVEMENT", "--depth", "1"],
    )
    .json();
    assert_eq!(json["ref"], "DOM-MOVEMENT");
    assert_eq!(json["depth"], 1);
    assert_json_matches_text(&json, &run.stdout, "--depth 1");

    let run = spec30(&home, &root, &["tree", "DOM-MOVEMENT", "--depth", "0"]);
    run.code(0);
    assert_eq!(
        tree_depths(&run.stdout),
        [(0, "DOM-MOVEMENT".to_owned())],
        "{}",
        run.show()
    );

    let run = spec30(&home, &root, &["tree", "MEC-SPRINT#RULE-SPRINT-COST"]);
    run.code(0);
    assert_eq!(
        run.stdout,
        "\
RULE-SPRINT-COST | rule | Cost | docs/spec/movement/sprint.md:18
  EDGE-SPRINT-EMPTY | edge-case | Empty tank | docs/spec/movement/sprint.md:22
nodes 2, roots 1
",
        "{}",
        run.show()
    );

    let feature = "docs/features/stamina-tuning.md";
    let run = spec30(&home, &root, &["tree", feature]);
    run.code(0);
    assert_eq!(
        tree_depths(&run.stdout),
        [(0, feature.to_owned()), (1, "AC-07".to_owned())],
        "{}",
        run.show()
    );
    assert!(
        item_lines(&run.stdout)[0]
            .starts_with("docs/features/stamina-tuning.md | - | Stamina tuning | docs/features/stamina-tuning.md:1"),
        "{}",
        run.show()
    );
    let json = spec30(&home, &root, &["--json", "tree", feature]).json();
    assert!(json["nodes"][0]["id"].is_null(), "{json}");
    assert_eq!(json["nodes"][1]["parent"], feature, "{json}");

    // Not found: exit 1, one `spec:` line, JSON with the reason.
    let run = spec30(&home, &root, &["tree", "MEC-NOPE"]);
    run.code(1);
    assert_eq!(run.stdout, "", "{}", run.show());
    assert!(run.stderr.starts_with("spec: "), "{}", run.show());
    let run = spec30(&home, &root, &["--json", "tree", "MEC-NOPE"]);
    run.code(1);
    let json = run.json();
    assert_eq!(json["ref"], "MEC-NOPE");
    assert!(
        json["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("MEC-NOPE")),
        "{json}"
    );
    assert_eq!(json["nodes"], serde_json::json!([]));

    for depth in ["x", "-1", "1.5"] {
        for json in [false, true] {
            let mut args = vec!["tree", "DOM-MOVEMENT", "--depth", depth];
            if json {
                args.insert(0, "--json");
            }
            let run = spec30(&home, &root, &args);
            run.code(2);
            assert_eq!(run.stdout, "", "{}", run.show());
            assert!(run.stderr.starts_with("spec: "), "{}", run.show());
        }
    }
}

/// AC-05: `--kind` filters the lines after the walk: exactly the unfiltered
/// tree's lines of that kind, in its order, with its depths and parents.
/// The kind is a free string (it lives only in this test). M: filtering
/// before descending.
#[test]
fn kind_filters_lines_after_the_walk() {
    for (fixture, kinds) in [
        ("spec-a", vec!["edge-case"]),
        ("spec-a", vec!["edge-case", "domain"]),
        ("spec-b", vec!["flag"]),
        ("spec-b", vec!["command", "flag"]),
    ] {
        let scratch = Scratch::new("tree-kind");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let all = spec30(&home, &root, &["tree"]);
        all.code(0);
        let all_json = spec30(&home, &root, &["--json", "tree"]).json();
        let mut args = vec!["tree"];
        for kind in &kinds {
            args.extend(["--kind", kind]);
        }
        let filtered = spec30(&home, &root, &args);
        filtered.code(0);
        let want: Vec<&str> = item_lines(&all.stdout)
            .into_iter()
            .filter(|line| {
                kinds
                    .iter()
                    .any(|kind| line.contains(&format!(" | {kind} | ")))
            })
            .collect();
        assert!(!want.is_empty(), "{fixture} {kinds:?}");
        assert_eq!(
            item_lines(&filtered.stdout),
            want,
            "{fixture} {kinds:?}\n{}",
            filtered.show()
        );
        // The summary counts the lines listed; `roots` only those at depth
        // 0 among them, not the walk's unlisted roots.
        let roots = want.iter().filter(|line| !line.starts_with(' ')).count();
        assert_eq!(
            summary_line(&filtered.stdout),
            format!("nodes {}, roots {roots}", want.len()),
            "{fixture} {kinds:?}"
        );
        let mut json_args = vec!["--json"];
        json_args.extend(&args);
        let json = spec30(&home, &root, &json_args).json();
        assert_eq!(json["kinds"], serde_json::json!(kinds), "{json}");
        let want: Vec<&Value> = all_json["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|node| kinds.iter().any(|kind| node["kind"] == *kind))
            .collect();
        let got: Vec<&Value> = json["nodes"].as_array().unwrap().iter().collect();
        assert_eq!(got, want, "{fixture} {kinds:?}: JSON nodes");
    }
    // AC-05 as written: spec-a's two edge cases, both below the root.
    let scratch = Scratch::new("tree-kind-none");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let run = spec30(&home, &root, &["tree", "--kind", "edge-case"]);
    run.code(0);
    assert_eq!(
        run.stdout,
        "        EDGE-SPRINT-EMPTY | edge-case | Empty tank | docs/spec/movement/sprint.md:22\n      \
         EDGE-STAM-ZERO | edge-case | Depletion | docs/spec/movement/stamina.md:25\n\
         nodes 2, roots 0\n",
        "{}",
        run.show()
    );
    let run = spec30(&home, &root, &["tree", "--kind", "domain"]);
    run.code(0);
    assert_eq!(
        summary_line(&run.stdout),
        "nodes 2, roots 1",
        "{}",
        run.show()
    );
    // Under a ROOT: the ROOT is not listed, so no root is counted.
    let run = spec30(
        &home,
        &root,
        &["tree", "DOM-MOVEMENT", "--kind", "mechanic"],
    );
    run.code(0);
    assert_eq!(
        summary_line(&run.stdout),
        "nodes 2, roots 0",
        "{}",
        run.show()
    );
    // A kind no node has: nothing listed, still exit 0.
    let run = spec30(&home, &root, &["tree", "--kind", "lantern-wick"]);
    run.code(0);
    assert!(item_lines(&run.stdout).is_empty(), "{}", run.show());
    assert_eq!(summary_line(&run.stdout), "nodes 0, roots 0");
}

/// AC-06: a `parent:` naming an `aliases:` entry (spec-a) or a legacy
/// Cyrillic `aliases_from` form (spec-b) still hangs the document under
/// its node; a dangling `parent:` lists the document as a root marked
/// with it, exit 0, and `spec check` keeps `ref-dangling` there. M:
/// `parent_id` joined to `nodes.id` as text.
#[test]
fn parents_resolve_as_the_check_resolves_them() {
    let scratch = Scratch::new("tree-parent");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    replace(
        &root,
        MOVEMENT,
        "tier: 1\n",
        "tier: 1\naliases: [DOM-MOTION]\n",
    );
    replace(&root, SPRINT, "parent: DOM-MOVEMENT", "parent: DOM-MOTION");
    let run = spec30(&home, &root, &["tree"]);
    run.code(0);
    assert_eq!(
        run.stdout,
        EXAMPLE.replace("README.md:16", "README.md:17"),
        "{}",
        run.show()
    );
    assert_eq!(run.stderr, "", "{}", run.show());
    let json = spec30(&home, &root, &["--json", "tree"]).json();
    let sprint = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "MEC-SPRINT")
        .unwrap();
    assert_eq!(sprint["parent"], "DOM-MOVEMENT", "{json}");
    assert!(sprint["mark"].is_null(), "{json}");

    // `#SECTION`: under that section, its subtree with it.
    replace(
        &root,
        SPRINT,
        "parent: DOM-MOTION",
        "parent: DOM-MOVEMENT#RULE-MOVE-SPEEDS",
    );
    let run = spec30(&home, &root, &["tree"]);
    run.code(0);
    assert_eq!(run.stderr, "", "{}", run.show());
    assert_eq!(
        tree_depths(&run.stdout),
        [
            (0, "DOM-GAME".to_owned()),
            (1, "RULE-CORE-LOOP".to_owned()),
            (1, "DOM-MOVEMENT".to_owned()),
            (2, "RULE-MOVE-SPEEDS".to_owned()),
            (3, "MEC-SPRINT".to_owned()),
            (4, "RULE-SPRINT-COST".to_owned()),
            (5, "EDGE-SPRINT-EMPTY".to_owned()),
            (2, "MEC-STAMINA".to_owned()),
            (3, "RULE-STAM-REGEN".to_owned()),
            (3, "EDGE-STAM-ZERO".to_owned()),
        ],
        "{}",
        run.show()
    );
    assert!(
        item_lines(&run.stdout)
            .contains(&"      MEC-SPRINT | mechanic | Sprint | docs/spec/movement/sprint.md:1"),
        "{}",
        run.show()
    );
    assert_eq!(summary_line(&run.stdout), "nodes 10, roots 1");
    let json = spec30(&home, &root, &["--json", "tree"]).json();
    assert_json_matches_text(&json, &run.stdout, "#SECTION parent");
    let sprint = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "MEC-SPRINT")
        .unwrap();
    assert_eq!(sprint["parent"], "RULE-MOVE-SPEEDS", "{json}");
    assert!(sprint["mark"].is_null(), "{json}");
    // From the section itself as ROOT.
    let run = spec30(&home, &root, &["tree", "DOM-MOVEMENT#RULE-MOVE-SPEEDS"]);
    run.code(0);
    assert_eq!(
        tree_depths(&run.stdout),
        [
            (0, "RULE-MOVE-SPEEDS".to_owned()),
            (1, "MEC-SPRINT".to_owned()),
            (2, "RULE-SPRINT-COST".to_owned()),
            (3, "EDGE-SPRINT-EMPTY".to_owned()),
        ],
        "{}",
        run.show()
    );

    // `project:`-qualified: an unmarked root, no warning, no finding.
    replace(
        &root,
        SPRINT,
        "parent: DOM-MOVEMENT#RULE-MOVE-SPEEDS",
        "parent: other:DOM-MOVEMENT",
    );
    let run = spec30(&home, &root, &["tree"]);
    run.code(0);
    assert_eq!(run.stderr, "", "{}", run.show());
    assert_eq!(
        tree_depths(&run.stdout),
        [
            (0, "DOM-GAME".to_owned()),
            (1, "RULE-CORE-LOOP".to_owned()),
            (1, "DOM-MOVEMENT".to_owned()),
            (2, "RULE-MOVE-SPEEDS".to_owned()),
            (2, "MEC-STAMINA".to_owned()),
            (3, "RULE-STAM-REGEN".to_owned()),
            (3, "EDGE-STAM-ZERO".to_owned()),
            (0, "MEC-SPRINT".to_owned()),
            (1, "RULE-SPRINT-COST".to_owned()),
            (2, "EDGE-SPRINT-EMPTY".to_owned()),
        ],
        "{}",
        run.show()
    );
    assert!(
        item_lines(&run.stdout)
            .contains(&"MEC-SPRINT | mechanic | Sprint | docs/spec/movement/sprint.md:1"),
        "unmarked\n{}",
        run.show()
    );
    assert_eq!(summary_line(&run.stdout), "nodes 10, roots 2");
    let json = spec30(&home, &root, &["--json", "tree"]).json();
    let sprint = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "MEC-SPRINT")
        .unwrap();
    assert!(sprint["parent"].is_null(), "{json}");
    assert!(sprint["mark"].is_null(), "{json}");
    let check = spec(&home, &root, &["--json", "check"]).json();
    assert!(
        !check["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["path"] == SPRINT && finding["line"] == 5),
        "{check}"
    );
    replace(
        &root,
        SPRINT,
        "parent: other:DOM-MOVEMENT",
        "parent: DOM-MOTION",
    );

    // Dangling.
    replace(&root, SPRINT, "parent: DOM-MOTION", "parent: DOM-NOWHERE");
    let run = spec30(&home, &root, &["tree"]);
    run.code(0);
    let lines = item_lines(&run.stdout);
    assert!(
        lines.contains(
            &"MEC-SPRINT | mechanic | Sprint | docs/spec/movement/sprint.md:1 | parent DOM-NOWHERE dangling"
        ),
        "{}",
        run.show()
    );
    assert_eq!(
        tree_depths(&run.stdout),
        [
            (0, "DOM-GAME".to_owned()),
            (1, "RULE-CORE-LOOP".to_owned()),
            (1, "DOM-MOVEMENT".to_owned()),
            (2, "RULE-MOVE-SPEEDS".to_owned()),
            (2, "MEC-STAMINA".to_owned()),
            (3, "RULE-STAM-REGEN".to_owned()),
            (3, "EDGE-STAM-ZERO".to_owned()),
            (0, "MEC-SPRINT".to_owned()),
            (1, "RULE-SPRINT-COST".to_owned()),
            (2, "EDGE-SPRINT-EMPTY".to_owned()),
        ],
        "{}",
        run.show()
    );
    assert_eq!(summary_line(&run.stdout), "nodes 10, roots 2");
    let json = spec30(&home, &root, &["--json", "tree"]).json();
    let sprint = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "MEC-SPRINT")
        .unwrap();
    assert_eq!(sprint["mark"], "dangling-parent", "{json}");
    assert!(sprint["parent"].is_null(), "{json}");
    let check = spec(&home, &root, &["--json", "check"]).json();
    let dangling: Vec<(String, u64)> = check["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["code"] == "ref-dangling")
        .map(|finding| {
            (
                finding["path"].as_str().unwrap().to_owned(),
                finding["line"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(dangling, [(SPRINT.to_owned(), 5)], "{check}");

    // spec-b: a legacy Cyrillic `aliases_from` form in `parent:`.
    let root = scratch.copy("spec-b", "copy-b");
    let home = scratch.home("b");
    write(
        &root,
        "docs/spec/pull.md",
        "---\nid: CMD-PULL\nclass: canon\nparent: \u{0422}\u{0420}\u{0411}-001\n---\n\n# Pull\n",
    );
    let run = spec30(&home, &root, &["tree", "REQ-001"]);
    run.code(0);
    assert_eq!(
        tree_depths(&run.stdout),
        [(0, "REQ-001".to_owned()), (1, "CMD-PULL".to_owned())],
        "{}",
        run.show()
    );
    // Not a default root: it hangs under REQ-001.
    let run = spec30(&home, &root, &["tree"]);
    run.code(0);
    assert_eq!(
        summary_line(&run.stdout),
        "nodes 4, roots 1",
        "{}",
        run.show()
    );
}

/// The rules: a parent ID with several holders lists the node once, under
/// the first in (path, position), with one `warning:` per such node.
#[test]
fn several_parent_holders_list_the_node_once_under_the_first() {
    let scratch = Scratch::new("tree-holders");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    write(
        &root,
        "docs/spec/aa-movement.md",
        "---\nid: DOM-MOVEMENT\nclass: canon\nparent: DOM-GAME\n---\n\n# Movement again\n",
    );
    let run = spec30(&home, &root, &["tree"]);
    run.code(0);
    assert_eq!(
        tree_depths(&run.stdout),
        [
            (0, "DOM-GAME".to_owned()),
            (1, "RULE-CORE-LOOP".to_owned()),
            (1, "DOM-MOVEMENT".to_owned()),
            (2, "MEC-SPRINT".to_owned()),
            (3, "RULE-SPRINT-COST".to_owned()),
            (4, "EDGE-SPRINT-EMPTY".to_owned()),
            (2, "MEC-STAMINA".to_owned()),
            (3, "RULE-STAM-REGEN".to_owned()),
            (3, "EDGE-STAM-ZERO".to_owned()),
            (1, "DOM-MOVEMENT".to_owned()),
            (2, "RULE-MOVE-SPEEDS".to_owned()),
        ],
        "{}",
        run.show()
    );
    let lines = item_lines(&run.stdout);
    assert!(
        lines[2].ends_with("| docs/spec/aa-movement.md:1"),
        "{}",
        run.show()
    );
    let warnings: Vec<&str> = run.stderr_lines();
    assert_eq!(warnings.len(), 2, "{}", run.show());
    for (warning, child) in warnings.iter().zip(["MEC-SPRINT", "MEC-STAMINA"]) {
        assert!(
            warning.starts_with("warning: ")
                && warning.contains(child)
                && warning.contains("docs/spec/aa-movement.md:1")
                && warning.contains("docs/spec/movement/README.md:1"),
            "{warning}"
        );
    }
}

/// AC-07: a two-node `parent:` cycle and a self-parent (spec-a), a
/// self-parent (spec-b): each node listed once, the cycle broken at its
/// first member in (path, position) as a root marked `parent cycle`, one
/// `warning:` per cycle naming its members, exit 0, the check one
/// `parent-cycle` warning per cycle (spec-check-process AC-15);
/// `spec graph MEC-STAMINA` lists MEC-SPRINT once. Every run is killed
/// after 30 s. M: no visited set.
#[test]
fn parent_cycles_are_broken_and_listed_once() {
    let scratch = Scratch::new("tree-cycle");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let before = spec30(&home, &root, &["--json", "check"]).json();
    replace(&root, SPRINT, "parent: DOM-MOVEMENT", "parent: MEC-STAMINA");
    replace(&root, STAMINA, "parent: DOM-MOVEMENT", "parent: MEC-SPRINT");
    replace(
        &root,
        "docs/spec/game.md",
        "tier: 0\n",
        "tier: 0\nparent: DOM-GAME\n",
    );
    for json in [false, true] {
        let mut args = vec!["tree"];
        if json {
            args.insert(0, "--json");
        }
        let run = spec30(&home, &root, &args);
        run.code(0);
        let warnings = run.stderr_lines();
        assert_eq!(warnings.len(), 2, "{}", run.show());
        assert!(
            warnings[0].starts_with("warning: ")
                && warnings[0].contains("DOM-GAME")
                && !warnings[0].contains("MEC-"),
            "{}",
            run.show()
        );
        assert!(
            warnings[1].starts_with("warning: ") && warnings[1].contains("MEC-SPRINT, MEC-STAMINA"),
            "{}",
            run.show()
        );
        if json {
            let rows = json_rows(&run.json());
            let marked: Vec<(&str, Option<&str>)> = rows
                .iter()
                .filter(|row| row.3.is_some())
                .map(|row| (row.0.as_str(), row.3.as_deref()))
                .collect();
            assert_eq!(
                marked,
                [
                    ("DOM-GAME", Some("parent-cycle")),
                    ("MEC-SPRINT", Some("parent-cycle"))
                ]
            );
            continue;
        }
        assert_eq!(
            run.stdout,
            "\
DOM-GAME | domain | Lantern Keep | docs/spec/game.md:1 | status accepted | parent cycle
  RULE-CORE-LOOP | rule | Core loop | docs/spec/game.md:22
  DOM-MOVEMENT | domain | Movement | docs/spec/movement/README.md:1 | status accepted
    RULE-MOVE-SPEEDS | rule | Speeds | docs/spec/movement/README.md:16
MEC-SPRINT | mechanic | Sprint | docs/spec/movement/sprint.md:1 | parent cycle
  RULE-SPRINT-COST | rule | Cost | docs/spec/movement/sprint.md:18
    EDGE-SPRINT-EMPTY | edge-case | Empty tank | docs/spec/movement/sprint.md:22
  MEC-STAMINA | mechanic | Stamina | docs/spec/movement/stamina.md:1 | status accepted
    RULE-STAM-REGEN | rule | Regeneration | docs/spec/movement/stamina.md:21
    EDGE-STAM-ZERO | edge-case | Depletion | docs/spec/movement/stamina.md:25
nodes 10, roots 2
",
            "{}",
            run.show()
        );
    }
    // A ROOT inside the cycle: each node once, too.
    for start in ["MEC-STAMINA", "MEC-SPRINT", "DOM-GAME"] {
        let run = spec30(&home, &root, &["tree", start]);
        run.code(0);
        let names: Vec<String> = tree_depths(&run.stdout)
            .into_iter()
            .map(|(_, n)| n)
            .collect();
        let unique: std::collections::BTreeSet<&String> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "{start}: {}", run.show());
    }
    // The `depends_on` cycle of the fixture, walked: MEC-SPRINT once.
    for args in [
        &["graph", "MEC-STAMINA"][..],
        &["graph", "MEC-SPRINT", "--impact"],
        &["--json", "graph", "MEC-STAMINA"],
    ] {
        let run = spec30(&home, &root, args);
        run.code(0);
        if args[0] == "--json" {
            let json = run.json();
            let ids = field(&json["nodes"], "id");
            assert_eq!(
                ids.iter().filter(|id| *id == "MEC-SPRINT").count(),
                1,
                "{json}"
            );
        } else {
            let sprint = common::graph::graph_nodes(&run.stdout)
                .into_iter()
                .filter(|line| line.contains(" MEC-SPRINT | "))
                .count();
            assert_eq!(sprint, 1, "{args:?}\n{}", run.show());
        }
    }
    // The check reports each of the two cycles once as a `parent-cycle`
    // warning (docs/features/spec-check-process.md AC-15 reverses Q6 of
    // spec-cli-graph), its members those of the tree's warning; nothing
    // else changes.
    let after = spec30(&home, &root, &["--json", "check"]).json();
    let codes = |json: &Value| {
        let mut codes = field(&json["findings"], "code");
        codes.sort();
        codes
    };
    let mut want = codes(&before);
    want.extend(["parent-cycle".to_owned(), "parent-cycle".to_owned()]);
    want.sort();
    assert_eq!(codes(&after), want, "{after}");
    let cycles: Vec<(String, String, String)> = after["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|f| f["code"] == "parent-cycle")
        .map(|f| {
            (
                f["severity"].as_str().unwrap_or_default().to_owned(),
                format!("{}:{}", f["path"].as_str().unwrap_or_default(), f["line"]),
                f["subject"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        cycles,
        [
            (
                "warning".to_owned(),
                "docs/spec/game.md:5".to_owned(),
                "DOM-GAME".to_owned()
            ),
            (
                "warning".to_owned(),
                format!("{SPRINT}:5"),
                "MEC-SPRINT, MEC-STAMINA".to_owned()
            ),
        ],
        "{after}"
    );

    // spec-b: a self-parent.
    let root = scratch.copy("spec-b", "copy-b");
    let home = scratch.home("b");
    replace(
        &root,
        "docs/spec/cli.md",
        "tier: 1\n",
        "tier: 1\nparent: MOD-CLI\n",
    );
    let run = spec30(&home, &root, &["tree"]);
    run.code(0);
    assert_eq!(run.stderr_lines().len(), 1, "{}", run.show());
    assert!(run.stderr.contains("MOD-CLI"), "{}", run.show());
    assert!(
        item_lines(&run.stdout)[0].ends_with(" | status accepted | parent cycle"),
        "{}",
        run.show()
    );
    assert_eq!(summary_line(&run.stdout), "nodes 4, roots 1");
}
