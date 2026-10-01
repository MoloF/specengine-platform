//! AC-07 (the walk's cycles), AC-12, AC-13 and AC-14 of
//! docs/features/spec-cli-graph.md: `spec graph REF [--impact] [--type T]…
//! [--depth N] [--archive]` walks breadth-first from REF's holders over the
//! built-in link-type table (default: every strong type outgoing,
//! `mentions` not followed; `--impact`: the impact table's directions),
//! a visited set, nodes by (distance, path, position), edges by (type,
//! path, line, column); dangling, `project:` and unchecked edges listed,
//! never followed. Exits: 0 answered, 1 an unresolvable REF/ROOT, 2 usage,
//! look-alike or `project:` REF (no JSON). Scratch copies of spec-a and
//! spec-b under their own `HOME`; every run is killed after 30 s.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use common::graph::{distances, field, graph_edges, graph_nodes, keys, spec30, summary_line};
use common::{Scratch, replace, write};

const STAMINA: &str = "docs/spec/movement/stamina.md";
const SPRINT: &str = "docs/spec/movement/sprint.md";

fn at(pairs: &[(usize, &str)]) -> Vec<(usize, String)> {
    pairs
        .iter()
        .map(|(distance, name)| (*distance, (*name).to_owned()))
        .collect()
}

/// The default walk: every strong type outgoing, unbounded, exactly the
/// spec's line formats and orders; the JSON has exactly the keys of
/// "Data" and the same nodes and edges.
#[test]
fn the_default_walk_follows_strong_links_outgoing() {
    let scratch = Scratch::new("graph-default");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let run = spec30(&home, &root, &["graph", "MEC-STAMINA"]);
    run.code(0);
    assert_eq!(run.stderr, "", "{}", run.show());
    assert_eq!(
        run.stdout,
        "\
0 MEC-STAMINA | mechanic | Stamina | docs/spec/movement/stamina.md:1
1 A-101 | assumption | Walking does not reset the regeneration delay | docs/records/A/A-101.md:1
1 R-12 | requirement | Stamina regenerates only at rest | docs/records/R/R-12.md:1
1 TERM-exhausted | term | Exhausted | docs/records/TERM/TERM-exhausted.md:1
1 MEC-SPRINT | mechanic | Sprint | docs/spec/movement/sprint.md:1
2 RULE-STAM-REGEN | rule | Regeneration | docs/spec/movement/stamina.md:21
MEC-SPRINT --constrains--> RULE-STAM-REGEN | docs/spec/movement/sprint.md:10
MEC-SPRINT --depends_on--> MEC-STAMINA | docs/spec/movement/sprint.md:9
MEC-STAMINA --depends_on--> MEC-SPRINT | docs/spec/movement/stamina.md:13
MEC-STAMINA --derived_from--> R-12 | docs/spec/movement/stamina.md:12
MEC-STAMINA --derived_from--> A-101 | docs/spec/movement/stamina.md:12
MEC-STAMINA --uses_term--> TERM-exhausted | docs/spec/movement/stamina.md:14
nodes 6, edges 6
",
        "{}",
        run.show()
    );
    let json = spec30(&home, &root, &["--json", "graph", "MEC-STAMINA"]).json();
    assert_eq!(
        keys(&json),
        [
            "ref",
            "reason",
            "impact",
            "types",
            "depth",
            "archive",
            "notes",
            "left_out",
            "truncated",
            "nodes",
            "edges"
        ]
        .into(),
        "{json}"
    );
    assert_eq!(json["ref"], "MEC-STAMINA");
    assert!(
        json["reason"].is_null() && json["depth"].is_null(),
        "{json}"
    );
    assert_eq!(json["impact"], false);
    assert_eq!(json["archive"], false);
    assert_eq!(json["truncated"], false);
    assert_eq!(
        json["left_out"],
        serde_json::json!({"generated": 0, "tier3": 0})
    );
    // The followed types: every strong one outgoing, never `mentions`.
    let types = json["types"].as_array().unwrap();
    for t in [
        "depends_on",
        "derived_from",
        "verifies",
        "uses_term",
        "constrains",
        "supersedes",
        "revises",
        "amends",
        "answers",
        "working_answer",
        "canon",
        "adopts",
    ] {
        assert!(
            types.contains(&serde_json::json!({"type": t, "direction": "out"})),
            "{t} not followed outgoing: {json}"
        );
    }
    assert!(
        !field(&json["types"], "type").contains(&"mentions".to_owned()),
        "{json}"
    );
    for node in json["nodes"].as_array().unwrap() {
        assert_eq!(
            keys(node),
            [
                "id", "kind", "title", "path", "line", "distance", "archived"
            ]
            .into(),
            "{node}"
        );
    }
    for edge in json["edges"].as_array().unwrap() {
        assert_eq!(
            keys(edge),
            [
                "src", "type", "dst", "written", "path", "line", "state", "reason"
            ]
            .into(),
            "{edge}"
        );
        assert_eq!(edge["state"], "resolved", "{edge}");
        assert!(edge["reason"].is_null(), "{edge}");
    }
    let from_json: Vec<(usize, String)> = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| {
            (
                node["distance"].as_u64().unwrap() as usize,
                node["id"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(from_json, distances(&run.stdout));
    let edges: Vec<String> = json["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|edge| {
            format!(
                "{} --{}--> {} | {}:{}",
                edge["src"].as_str().unwrap(),
                edge["type"].as_str().unwrap(),
                edge["dst"].as_str().unwrap(),
                edge["path"].as_str().unwrap(),
                edge["line"]
            )
        })
        .collect();
    assert_eq!(edges, graph_edges(&run.stdout));

    // spec-b: its own prefixes, a legacy Cyrillic `derived_from` item.
    let root = scratch.copy("spec-b", "copy-b");
    let run = spec30(&home, &root, &["graph", "MOD-CLI"]);
    run.code(0);
    assert_eq!(
        distances(&run.stdout),
        at(&[(0, "MOD-CLI"), (1, "REQ-001"), (1, "REQ-002")]),
        "{}",
        run.show()
    );
    assert_eq!(
        graph_edges(&run.stdout),
        [
            "MOD-CLI --derived_from--> REQ-001 | docs/spec/cli.md:10",
            "MOD-CLI --derived_from--> REQ-002 | docs/spec/cli.md:10",
        ]
    );
    let json = spec30(&home, &root, &["--json", "graph", "MOD-CLI"]).json();
    assert_eq!(
        json["edges"][1]["written"], "\u{0422}\u{0420}\u{0411}-002",
        "{json}"
    );
}

/// AC-12: a `mentions` of Q-032 in MEC-STAMINA's body is not followed by
/// default; `--type mentions` follows it (Q-032 at 1). M: `mentions`
/// followed by default.
#[test]
fn mentions_are_followed_only_when_asked() {
    let scratch = Scratch::new("graph-mentions");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    replace(
        &root,
        STAMINA,
        "Stamina limits sprinting.",
        "Stamina limits sprinting (Q-032).",
    );
    let run = spec30(&home, &root, &["graph", "MEC-STAMINA"]);
    run.code(0);
    assert!(
        !run.stdout.contains("Q-032"),
        "mentions followed by default:\n{}",
        run.show()
    );
    let run = spec30(
        &home,
        &root,
        &["graph", "MEC-STAMINA", "--type", "mentions"],
    );
    run.code(0);
    assert_eq!(
        distances(&run.stdout),
        at(&[(0, "MEC-STAMINA"), (1, "Q-032"), (1, "R-12")]),
        "{}",
        run.show()
    );
    assert_eq!(
        graph_edges(&run.stdout),
        [
            "MEC-STAMINA --mentions--> Q-032 | docs/spec/movement/stamina.md:19",
            "RULE-STAM-REGEN --mentions--> R-12 | docs/spec/movement/stamina.md:22",
        ],
        "{}",
        run.show()
    );
    let json = spec30(
        &home,
        &root,
        &["--json", "graph", "MEC-STAMINA", "--type", "mentions"],
    )
    .json();
    assert_eq!(
        json["types"],
        serde_json::json!([{"type": "mentions", "direction": "out"}]),
        "{json}"
    );
    // spec-b: the same rule, its own prefixes.
    let root = scratch.copy("spec-b", "copy-b");
    let run = spec30(&home, &root, &["graph", "MOD-CLI#CMD-SYNC"]);
    run.code(0);
    assert_eq!(
        distances(&run.stdout),
        at(&[(0, "CMD-SYNC")]),
        "{}",
        run.show()
    );
    let run = spec30(
        &home,
        &root,
        &[
            "graph",
            "MOD-CLI#CMD-SYNC",
            "--type",
            "mentions",
            "--depth",
            "1",
        ],
    );
    run.code(0);
    assert_eq!(
        distances(&run.stdout),
        at(&[
            (0, "CMD-SYNC"),
            (1, "CRIT-01"),
            (1, "GLS-worktree"),
            (1, "QN-07")
        ]),
        "{}",
        run.show()
    );
}

/// AC-13: `--impact` follows `depends_on`, `derived_from`, `verifies`,
/// `uses_term` incoming and `constrains` outgoing. M: `derived_from`
/// flipped.
#[test]
fn impact_follows_the_impact_table() {
    let scratch = Scratch::new("graph-impact");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let run = spec30(&home, &root, &["graph", "R-12", "--impact"]);
    run.code(0);
    assert_eq!(
        distances(&run.stdout),
        at(&[
            (0, "R-12"),
            (1, "MEC-STAMINA"),
            (2, "MEC-SPRINT"),
            (3, "RULE-STAM-REGEN")
        ]),
        "{}",
        run.show()
    );
    let run = spec30(&home, &root, &["graph", "MEC-SPRINT", "--impact"]);
    run.code(0);
    assert_eq!(
        distances(&run.stdout),
        at(&[
            (0, "MEC-SPRINT"),
            (1, "MEC-STAMINA"),
            (1, "RULE-STAM-REGEN")
        ]),
        "{}",
        run.show()
    );
    // `uses_term` incoming.
    let run = spec30(&home, &root, &["graph", "TERM-exhausted", "--impact"]);
    run.code(0);
    assert_eq!(
        distances(&run.stdout)[..2],
        at(&[(0, "TERM-exhausted"), (1, "MEC-STAMINA")])[..],
        "{}",
        run.show()
    );
    let json = spec30(&home, &root, &["--json", "graph", "R-12", "--impact"]).json();
    assert_eq!(json["impact"], true);
    assert_eq!(
        json["types"],
        serde_json::json!([
            {"type": "depends_on", "direction": "in"},
            {"type": "derived_from", "direction": "in"},
            {"type": "verifies", "direction": "in"},
            {"type": "uses_term", "direction": "in"},
            {"type": "constrains", "direction": "out"},
        ]),
        "{json}"
    );
    // `verifies` incoming: a scratch criterion verifying R-12.
    write(
        &root,
        "docs/features/regen-check.md",
        "---\nclass: spec\nstatus: draft\nscope: [movement]\n---\n\n# Regen check\n\n### Verified {#AC-31}\n\nChecks it.\n",
    );
    write(
        &root,
        "docs/records/A/A-103.md",
        "---\nid: A-103\nclass: canon\nlinks:\n  verifies: [R-12]\n---\n\n# Verifier\n",
    );
    let run = spec30(&home, &root, &["graph", "R-12", "--impact", "--depth", "1"]);
    run.code(0);
    assert_eq!(
        distances(&run.stdout),
        at(&[(0, "R-12"), (1, "A-103"), (1, "MEC-STAMINA")]),
        "{}",
        run.show()
    );

    // spec-b: a legacy Cyrillic `derived_from` item, walked back.
    let root = scratch.copy("spec-b", "copy-b");
    for start in ["REQ-001", "REQ-002"] {
        let run = spec30(&home, &root, &["graph", start, "--impact"]);
        run.code(0);
        assert_eq!(
            distances(&run.stdout),
            at(&[(0, start), (1, "MOD-CLI")]),
            "{}",
            run.show()
        );
    }
}

/// `--type T` replaces the set: outgoing; under `--impact` in its table's
/// direction, a type outside the table incoming. A dangling, `project:` or
/// unchecked edge is listed with its state, never followed.
#[test]
fn type_directions_and_unfollowed_edges() {
    let scratch = Scratch::new("graph-types");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let run = spec30(
        &home,
        &root,
        &["graph", "MEC-STAMINA", "--type", "depends_on"],
    );
    run.code(0);
    assert_eq!(
        distances(&run.stdout),
        at(&[(0, "MEC-STAMINA"), (1, "MEC-SPRINT")])
    );
    // Under --impact, `derived_from` is incoming: R-12 reaches MEC-STAMINA,
    // MEC-STAMINA reaches nothing.
    let run = spec30(
        &home,
        &root,
        &["graph", "R-12", "--impact", "--type", "derived_from"],
    );
    run.code(0);
    assert_eq!(
        distances(&run.stdout),
        at(&[(0, "R-12"), (1, "MEC-STAMINA")])
    );
    let run = spec30(
        &home,
        &root,
        &["graph", "MEC-STAMINA", "--impact", "--type", "derived_from"],
    );
    run.code(0);
    assert_eq!(distances(&run.stdout), at(&[(0, "MEC-STAMINA")]));
    // A type outside the impact table is incoming under --impact.
    let run = spec30(
        &home,
        &root,
        &["graph", "DEC-0007", "--impact", "--type", "supersedes"],
    );
    run.code(0);
    assert_eq!(
        distances(&run.stdout),
        at(&[(0, "DEC-0007"), (1, "DEC-0023")]),
        "{}",
        run.show()
    );
    let json = spec30(
        &home,
        &root,
        &[
            "--json",
            "graph",
            "DEC-0007",
            "--impact",
            "--type",
            "supersedes",
        ],
    )
    .json();
    assert_eq!(
        json["types"],
        serde_json::json!([{"type": "supersedes", "direction": "in"}])
    );
    assert_eq!(json["nodes"][0]["archived"], true, "{json}");
    // Outgoing without --impact: DEC-0023 reaches the archived DEC-0007.
    let run = spec30(&home, &root, &["graph", "DEC-0023", "--type", "supersedes"]);
    run.code(0);
    assert_eq!(
        graph_nodes(&run.stdout),
        [
            "0 DEC-0023 | decision | Regeneration waits for rest | docs/records/DEC/DEC-0023.md:1",
            "1 DEC-0007 | decision | Stamina regenerates while walking | docs/records/DEC/DEC-0007.md:1 | archived",
        ],
        "{}",
        run.show()
    );

    // Dangling, another project's, unchecked: listed, not followed.
    replace(
        &root,
        STAMINA,
        "  depends_on: [MEC-SPRINT]\n",
        "  depends_on: [MEC-SPRINT, MEC-NOWHERE, shared:R-1]\n",
    );
    replace(
        &root,
        STAMINA,
        "Stamina limits sprinting.",
        "Stamina limits sprinting ([licence](../../../LICENSE), [gone](gone.md)).",
    );
    let run = spec30(&home, &root, &["graph", "MEC-STAMINA", "--depth", "1"]);
    run.code(0);
    let edges = graph_edges(&run.stdout);
    for line in [
        "MEC-STAMINA --depends_on--> MEC-NOWHERE | docs/spec/movement/stamina.md:13 | dangling: ",
        "MEC-STAMINA --depends_on--> shared:R-1 | docs/spec/movement/stamina.md:13 | skipped: another project",
    ] {
        assert!(
            edges.iter().any(|edge| edge.starts_with(line)),
            "{line}\n{}",
            run.show()
        );
    }
    assert_eq!(
        distances(&run.stdout).len(),
        5,
        "dangling/skipped edges reach no node\n{}",
        run.show()
    );
    let run = spec30(
        &home,
        &root,
        &["graph", "MEC-STAMINA", "--type", "mentions", "--depth", "1"],
    );
    run.code(0);
    let edges = graph_edges(&run.stdout);
    assert!(
        edges.contains(
            &"MEC-STAMINA --mentions--> ../../../LICENSE | docs/spec/movement/stamina.md:19 | unchecked"
        ),
        "{}",
        run.show()
    );
    assert!(
        edges.iter().any(|edge| edge.starts_with(
            "MEC-STAMINA --mentions--> gone.md | docs/spec/movement/stamina.md:19 | dangling: "
        )),
        "{}",
        run.show()
    );
    let json = spec30(
        &home,
        &root,
        &["--json", "graph", "MEC-STAMINA", "--depth", "1"],
    )
    .json();
    let states: Vec<(String, String)> = json["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|edge| {
            (
                edge["written"].as_str().unwrap().to_owned(),
                edge["state"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert!(
        states.contains(&("MEC-NOWHERE".into(), "dangling".into())),
        "{json}"
    );
    assert!(
        states.contains(&("shared:R-1".into(), "skipped".into())),
        "{json}"
    );
    let dangling = json["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|edge| edge["state"] == "dangling")
        .unwrap();
    assert!(
        dangling["dst"].is_null() || dangling["dst"] == "MEC-NOWHERE",
        "{dangling}"
    );
    assert!(dangling["reason"].is_string(), "{dangling}");
}

/// `--depth N`: nodes at distance N are not expanded; 0 is REF alone;
/// anything but an integer ≥ 0 is exit 2.
#[test]
fn depth_bounds_the_walk() {
    let scratch = Scratch::new("graph-depth");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let run = spec30(&home, &root, &["graph", "MEC-STAMINA", "--depth", "0"]);
    run.code(0);
    assert_eq!(
        run.stdout,
        "0 MEC-STAMINA | mechanic | Stamina | docs/spec/movement/stamina.md:1\nnodes 1, edges 0\n"
    );
    let run = spec30(&home, &root, &["graph", "MEC-STAMINA", "--depth", "1"]);
    run.code(0);
    assert_eq!(
        distances(&run.stdout),
        at(&[
            (0, "MEC-STAMINA"),
            (1, "A-101"),
            (1, "R-12"),
            (1, "TERM-exhausted"),
            (1, "MEC-SPRINT")
        ])
    );
    assert_eq!(summary_line(&run.stdout), "nodes 5, edges 4");
    let json = spec30(
        &home,
        &root,
        &["--json", "graph", "MEC-STAMINA", "--depth", "1"],
    )
    .json();
    assert_eq!(json["depth"], 1);
    for depth in ["x", "-1"] {
        for json in [false, true] {
            let mut args = vec!["graph", "MEC-STAMINA", "--depth", depth];
            if json {
                args.insert(0, "--json");
            }
            let run = spec30(&home, &root, &args);
            run.code(2);
            assert_eq!(run.stdout, "", "{}", run.show());
        }
    }
}

/// AC-07 for the walk: `depends_on` / `constrains` cycles (the fixture's
/// two-node one, a three-node one, a self-link) are walked once per node,
/// every edge listed, within 30 s. M: no visited set.
#[test]
fn link_cycles_are_walked_once_per_node() {
    let scratch = Scratch::new("graph-cycle");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    // The fixture: MEC-STAMINA ⇄ MEC-SPRINT.
    for args in [
        &["graph", "MEC-STAMINA"][..],
        &["graph", "MEC-SPRINT"],
        &["graph", "MEC-STAMINA", "--impact"],
        &["graph", "MEC-SPRINT", "--impact"],
        &["graph", "R-12", "--impact"],
    ] {
        let run = spec30(&home, &root, args);
        run.code(0);
        let names: Vec<String> = distances(&run.stdout).into_iter().map(|(_, n)| n).collect();
        let unique: std::collections::BTreeSet<&String> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "{args:?}\n{}", run.show());
        assert_eq!(
            names.iter().filter(|n| *n == "MEC-SPRINT").count(),
            1,
            "{args:?}\n{}",
            run.show()
        );
    }
    // A self-link and a three-node cycle through the movement domain.
    replace(
        &root,
        STAMINA,
        "  depends_on: [MEC-SPRINT]\n",
        "  depends_on: [MEC-SPRINT, MEC-STAMINA]\n",
    );
    replace(
        &root,
        SPRINT,
        "  constrains: [RULE-STAM-REGEN]\n",
        "  constrains: [RULE-STAM-REGEN, DOM-MOVEMENT]\n",
    );
    replace(
        &root,
        "docs/spec/movement/README.md",
        "parent: DOM-GAME\n",
        "parent: DOM-GAME\nlinks:\n  depends_on: [MEC-STAMINA]\n",
    );
    for args in [
        &["graph", "MEC-STAMINA"][..],
        &["graph", "DOM-MOVEMENT", "--impact"],
        &["graph", "MEC-SPRINT", "--impact"],
    ] {
        let run = spec30(&home, &root, args);
        run.code(0);
        let names: Vec<String> = distances(&run.stdout).into_iter().map(|(_, n)| n).collect();
        let unique: std::collections::BTreeSet<&String> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "{args:?}\n{}", run.show());
    }
    let run = spec30(&home, &root, &["graph", "MEC-STAMINA"]);
    let edges = graph_edges(&run.stdout);
    for edge in [
        "MEC-STAMINA --depends_on--> MEC-STAMINA | docs/spec/movement/stamina.md:13",
        "DOM-MOVEMENT --depends_on--> MEC-STAMINA | docs/spec/movement/README.md:7",
        "MEC-SPRINT --constrains--> DOM-MOVEMENT | docs/spec/movement/sprint.md:10",
    ] {
        assert!(edges.contains(&edge), "{edge}\n{}", run.show());
    }
    assert!(
        distances(&run.stdout).contains(&(2, "DOM-MOVEMENT".to_owned())),
        "{}",
        run.show()
    );
}

/// AC-14: dangling parents and links and cycles answer 0; only an
/// unresolvable REF/ROOT is 1 (JSON with the reason); a look-alike ID is 2
/// naming the Latin fix, `project:` is 2, usage is 2; no JSON for 2. M:
/// exit 1 on a dangling link.
#[test]
fn exits_of_the_graph_reads() {
    let scratch = Scratch::new("graph-exit");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    replace(&root, SPRINT, "parent: DOM-MOVEMENT", "parent: DOM-NOWHERE");
    replace(
        &root,
        STAMINA,
        "  depends_on: [MEC-SPRINT]\n",
        "  depends_on: [MEC-SPRINT, MEC-NOWHERE]\n",
    );
    replace(
        &root,
        STAMINA,
        "Stamina limits sprinting.",
        "Stamina limits sprinting (R-99, [gone](gone.md)).",
    );
    replace(
        &root,
        "docs/spec/game.md",
        "tier: 0\n",
        "tier: 0\nparent: DOM-GAME\n",
    );
    let answered: [&[&str]; 8] = [
        &["tree"],
        &["tree", "MEC-SPRINT"],
        &["tree", "DOM-GAME"],
        &["graph", "MEC-STAMINA"],
        &["graph", "MEC-STAMINA", "--type", "mentions"],
        &["graph", "MEC-NOWHERE-ISH", "--depth", "0"],
        &["show", "MEC-STAMINA", "--links"],
        &["show", "MEC-SPRINT", "--links"],
    ];
    for args in answered {
        let code = i32::from(args.contains(&"MEC-NOWHERE-ISH"));
        for json in [false, true] {
            let mut full = args.to_vec();
            if json {
                full.insert(0, "--json");
            }
            let run = spec30(&home, &root, &full);
            run.code(code);
            if json {
                let value = run.json();
                assert_eq!(value["reason"].is_null(), code == 0, "{full:?}: {value}");
            }
        }
    }
    let look_alike = "\u{041c}\u{0415}\u{0421}-STAMINA";
    for command in ["tree", "graph", "show"] {
        // Unresolvable: 1, one `spec:` line, JSON with the reason.
        let mut args = vec![command, "R-99"];
        if command == "show" {
            args.push("--links");
        }
        let run = spec30(&home, &root, &args);
        run.code(1);
        assert!(run.stderr.starts_with("spec: "), "{}", run.show());
        let mut json = vec!["--json"];
        json.extend(&args);
        let run = spec30(&home, &root, &json);
        run.code(1);
        assert!(run.json()["reason"].as_str().unwrap().contains("R-99"));
        // Look-alike, `project:`: 2, no JSON, the Latin fix named.
        for reference in [look_alike, "other:R-12"] {
            for json in [false, true] {
                let mut args = vec![command, reference];
                if command == "show" {
                    args.push("--links");
                }
                if json {
                    args.insert(0, "--json");
                }
                let run = spec30(&home, &root, &args);
                run.code(2);
                assert_eq!(run.stdout, "", "{}", run.show());
                assert!(run.stderr.starts_with("spec: "), "{}", run.show());
                if reference == look_alike {
                    assert!(run.stderr.contains("`MEC-STAMINA`"), "{}", run.show());
                }
            }
        }
    }
    // Usage: `--archive` without `--links`, `--bindings`, `--history`.
    for args in [
        &["show", "MEC-STAMINA", "--archive"][..],
        &["--json", "show", "MEC-STAMINA", "--archive"],
        &["show", "MEC-STAMINA", "--bindings"],
        &["show", "MEC-STAMINA", "--history"],
        &["tree", "--depth"],
        &["graph"],
    ] {
        let run = spec30(&home, &root, args);
        run.code(2);
        assert_eq!(run.stdout, "", "{}", run.show());
    }
    // spec-b: a look-alike of its own scheme names the Latin fix.
    let root = scratch.copy("spec-b", "copy-b");
    let run = spec30(&home, &root, &["--json", "graph", "R\u{0415}Q-002"]);
    run.code(2);
    assert_eq!(run.stdout, "");
    assert!(run.stderr.contains("`REQ-002`"), "{}", run.show());
    let run = spec30(
        &home,
        &root,
        &["--json", "graph", "\u{0422}\u{0420}\u{0411}-002"],
    );
    run.code(0);
    assert_eq!(run.json()["nodes"][0]["id"], "REQ-002");
}

/// AC-21 and "Endpoints": a `canon:` path without an anchor (the check:
/// `canon-form`) and one whose anchor names nothing (`canon-anchor`) land
/// on the document, are followed, state `resolved` with a `reason`; on
/// spec-a (DEC-0023 to MEC-STAMINA) and spec-b (ADR-0001 to MOD-CLI). M:
/// an anchor-less path `unchecked`.
#[test]
fn a_canon_path_without_an_anchor_lands_on_the_document() {
    let decision_a = "docs/records/DEC/DEC-0023.md";
    let decision_b = "docs/records/ADR/ADR-0001.md";
    let anchor_b = "docs/spec/cli.md#\u{043a}\u{043e}\u{043c}\u{0430}\u{043d}\u{0434}\u{0430}-sync";
    for (fixture, decision, id, written, document, line, title) in [
        (
            "spec-a",
            decision_a,
            "DEC-0023",
            "docs/spec/movement/stamina.md#regeneration",
            (
                "MEC-STAMINA",
                "docs/spec/movement/stamina.md",
                "mechanic | Stamina",
            ),
            6,
            "decision | Regeneration waits for rest",
        ),
        (
            "spec-b",
            decision_b,
            "ADR-0001",
            anchor_b,
            (
                "MOD-CLI",
                "docs/spec/cli.md",
                "module | \u{041a}\u{043e}\u{043c}\u{0430}\u{043d}\u{0434}\u{043d}\u{0430}\u{044f} \u{0441}\u{0442}\u{0440}\u{043e}\u{043a}\u{0430}",
            ),
            7,
            "decision | \u{0421}\u{0438}\u{043d}\u{0445}\u{0440}\u{043e}\u{043d}\u{0438}\u{0437}\u{0430}\u{0446}\u{0438}\u{044f} \u{0447}\u{0435}\u{0440}\u{0435}\u{0437} \u{0440}\u{0430}\u{0431}\u{043e}\u{0447}\u{0438}\u{0435} \u{043a}\u{043e}\u{043f}\u{0438}\u{0438}",
        ),
    ] {
        let (name, path, kind_title) = document;
        for (target, code) in [
            (path.to_owned(), "canon-form"),
            (format!("{path}#nowhere"), "canon-anchor"),
        ] {
            let context = format!("{fixture} canon: {target}");
            let scratch = Scratch::new("graph-canon");
            let home = scratch.home("h");
            let root = scratch.copy(fixture, "copy");
            replace(
                &root,
                decision,
                &format!("canon: {written}\n"),
                &format!("canon: {target}\n"),
            );
            let run = spec30(&home, &root, &["graph", id, "--type", "canon"]);
            run.code(0);
            assert_eq!(
                run.stdout,
                format!(
                    "0 {id} | {title} | {decision}:1\n\
                     1 {name} | {kind_title} | {path}:1\n\
                     {id} --canon--> {name} | {decision}:{line}\n\
                     nodes 2, edges 1\n"
                ),
                "{context}\n{}",
                run.show()
            );
            // The default walk follows it too: the document at 1.
            let run = spec30(&home, &root, &["graph", id]);
            run.code(0);
            assert!(
                distances(&run.stdout).contains(&(1, name.to_owned())),
                "{context}\n{}",
                run.show()
            );
            let json = spec30(&home, &root, &["--json", "graph", id, "--type", "canon"]).json();
            let edge = &json["edges"][0];
            assert_eq!(edge["dst"], name, "{context}: {edge}");
            assert_eq!(edge["written"], target.as_str(), "{context}: {edge}");
            assert_eq!(edge["state"], "resolved", "{context}: {edge}");
            assert!(
                edge["reason"]
                    .as_str()
                    .is_some_and(|reason| !reason.is_empty()),
                "{context}: the reason says why: {edge}"
            );
            // `show --links` on the document: incoming, no ` | at`.
            let json = spec30(&home, &root, &["--json", "show", name, "--links"]).json();
            let incoming = json["nodes"][0]["links"]["incoming"].as_array().unwrap();
            let link = incoming
                .iter()
                .find(|link| link["type"] == "canon" && link["path"] == decision)
                .unwrap_or_else(|| panic!("{context}: no canon link: {json}"));
            assert!(link["at"].is_null(), "{context}: {link}");
            assert_eq!(link["state"], "resolved", "{context}: {link}");
            assert!(link["reason"].is_string(), "{context}: {link}");
            // The check names it.
            let check = spec30(&home, &root, &["--json", "check"]).json();
            assert!(
                check["findings"].as_array().unwrap().iter().any(|finding| {
                    finding["code"] == code
                        && finding["path"] == decision
                        && finding["line"] == line
                }),
                "{context}: {code} at {decision}:{line}: {check}"
            );
        }
    }
}
