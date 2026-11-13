//! docs/features/usage-orphans.md (RM-0111) "Acceptance criteria", the core
//! half: `usage::tally` and `usage::usage_of` over a `SpecGraph` built by the
//! real parser from `fixtures/spec-a` (only read) and from invented corpora
//! held in memory.
//!
//! - UO1: `fixtures/spec-a`'s counts are the spec's table, node by node, a
//!   section's `orphan` `None`, the Tier 3 file absent; `tally` counts
//!   distinct sources of other live files, resolved rows only, strong and
//!   `mentions` apart. M: rows counted, not sources (MEC-STAMINA and R-12
//!   `mentioned_by` 7); same-file sources counted (`stamina-tuning.md` 2);
//!   a document without its sections' uses (MEC-STAMINA `used_by` 1).
//! - UO2: a generated or Tier 3 source counts for nothing. M: a generated
//!   source counted.
//! - UO3: the orphan rules on an invented corpus: one's own `parent:` is no
//!   link, a child (even under a section) keeps its holder, `abandoned` is
//!   never an orphan, a resolved link out is, a dangling, other-project or
//!   unchecked link is not one, nor a wiki title, nor a generated child.
//!   M: one's own `parent:` taken as a link; children ignored; `abandoned`
//!   ignored; links out ignored.
//! - UO8: `src/usage.rs` holds no string literal but `"abandoned"`; the
//!   answer is blind to input order. M: rows taken in input order.
//!
//! Every expected value is worked out from the spec and the files here,
//! never read back from a run of the code under test.

mod common;

use std::collections::BTreeMap;

use common::check::{Config, fixture_input};
use common::{fixture, repository_root};
use specengine_core::check::{CheckInput, Endpoint, NodeAt, SpecGraph, Standing};
use specengine_core::usage::{NodeUsage, UseRow, tally, usage_of};

/// `(used_by, mentioned_by, orphan)` of one node.
type Counts = (u32, u32, Option<bool>);

fn counts(usage: &NodeUsage) -> Counts {
    (usage.used_by, usage.mentioned_by, usage.orphan)
}

/// `usage_of` keyed by each node's name (ID, else path), every node of a
/// live file once. A node of a file that is not live is never judged
/// (`orphan` `None`), whatever counts it carries: the code serves those
/// nodes too (its `usage_of` doc: "every node of every file"), where
/// "Data" names live files only; the answer for live nodes is the same.
fn by_name(graph: &SpecGraph<'_>) -> BTreeMap<String, Counts> {
    let mut out = BTreeMap::new();
    for (at, usage) in usage_of(graph) {
        let name = graph.name(at);
        if graph.standing(at.file) != Standing::Live {
            assert_eq!(usage.orphan, None, "{name}: not live, not judged");
            continue;
        }
        assert!(
            out.insert(name.clone(), counts(&usage)).is_none(),
            "{name} named twice"
        );
    }
    out
}

/// Every node of every live file, in (path, position) order.
fn live_nodes(graph: &SpecGraph<'_>) -> Vec<NodeAt> {
    let mut out = Vec::new();
    for file in 0..graph.paths().len() {
        if graph.standing(file) != Standing::Live {
            continue;
        }
        for ord in 0..graph.nodes(file).len() {
            out.push(NodeAt { file, ord });
        }
    }
    out
}

// ------------------------------------------------------------------ UO1

/// The spec's table on `fixtures/spec-a` ("Data", "On `fixtures/spec-a`").
const SPEC_A: [(&str, Counts); 13] = [
    ("MEC-STAMINA", (2, 6, Some(false))),
    ("RULE-STAM-REGEN", (2, 3, None)),
    ("MEC-SPRINT", (1, 1, Some(false))),
    ("DOM-MOVEMENT", (0, 1, Some(false))),
    ("DOM-GAME", (0, 0, Some(false))),
    ("A-101", (2, 3, Some(false))),
    ("A-102", (0, 0, Some(true))),
    ("DEC-0023", (1, 1, Some(false))),
    ("Q-031", (1, 3, Some(false))),
    ("Q-032", (0, 0, Some(false))),
    ("R-12", (1, 6, Some(false))),
    ("TERM-exhausted", (1, 1, Some(false))),
    ("docs/features/stamina-tuning.md", (0, 1, Some(false))),
];

/// UO1: every row of the spec's table; every node of the 12 live files,
/// a section's `orphan` `None`, a document's `Some`; DEC-0007 (Tier 3) not
/// judged; A-102 the one orphan.
#[test]
fn uo1_spec_a_is_the_specs_table() {
    let (config, input) = fixture_input(&fixture("spec-a"));
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let usage = usage_of(&graph);
    let named = by_name(&graph);
    for (name, want) in SPEC_A {
        assert_eq!(named.get(name), Some(&want), "{name}");
    }

    let live = live_nodes(&graph);
    for at in &live {
        let node = usage
            .get(at)
            .unwrap_or_else(|| panic!("{}: a live node", graph.name(*at)));
        assert_eq!(
            node.orphan.is_some(),
            at.ord == 0,
            "{}: only documents are judged",
            graph.name(*at)
        );
    }
    let documents: Vec<NodeAt> = live.iter().copied().filter(|at| at.ord == 0).collect();
    assert_eq!(documents.len(), 12, "12 live documents");
    assert!(!named.contains_key("DEC-0007"), "Tier 3: {named:?}");
    let orphans: Vec<String> = usage
        .iter()
        .filter(|(_, node)| node.orphan == Some(true))
        .map(|(at, _)| graph.name(*at))
        .collect();
    assert_eq!(orphans, ["A-102"]);
}

fn row<'a>(source: &'a str, file: &'a str, weak: bool) -> UseRow<'a> {
    UseRow {
        source,
        file,
        weak,
        live: true,
        resolved: true,
    }
}

/// The rows of [`uo1_tally_counts_distinct_sources_of_other_live_files`],
/// seen from a node of `docs/spec/held.md`.
fn tally_rows() -> Vec<UseRow<'static>> {
    let own = "docs/spec/held.md";
    vec![
        // One source citing twice, strong and weak: once in each.
        row("MEC-ONE", "docs/spec/one.md", false),
        row("MEC-ONE", "docs/spec/one.md", false),
        row("MEC-ONE", "docs/spec/one.md", true),
        // Two sources of one file: two.
        row("RULE-TWO-A", "docs/spec/two.md", true),
        row("RULE-TWO-B", "docs/spec/two.md", true),
        row("RULE-TWO-B", "docs/spec/two.md", true),
        // The node's own file: none.
        row("RULE-HELD-PART", own, false),
        row("DOM-HELD", own, true),
        // A path-named source.
        row("docs/notes/loose.md", "docs/notes/loose.md", true),
        // Not live, not resolved: none.
        UseRow {
            live: false,
            ..row("DEC-0001", "docs/records/DEC-0001.md", false)
        },
        UseRow {
            live: false,
            ..row("docs/generated/map.md", "docs/generated/map.md", true)
        },
        UseRow {
            resolved: false,
            ..row("MEC-FAR", "docs/spec/far.md", false)
        },
        UseRow {
            resolved: false,
            ..row("MEC-FAR", "docs/spec/far.md", true)
        },
    ]
}

/// UO1, `tally`: distinct sources, not rows; a source of the node's own
/// file never; strong and `mentions` apart, one source in both; a row of a
/// file that is not live, or not resolved, never; the rows' order changes
/// nothing. M: rows counted, not sources; same-file sources counted.
#[test]
fn uo1_tally_counts_distinct_sources_of_other_live_files() {
    let own = "docs/spec/held.md";
    assert_eq!(tally(tally_rows(), own), (1, 4));
    assert_eq!(
        tally(tally_rows().into_iter().rev(), own),
        (1, 4),
        "blind to the rows' order"
    );
    assert_eq!(tally(Vec::new(), own), (0, 0));
    // The same rows seen from another node's file: its own rows now count.
    assert_eq!(tally(tally_rows(), "docs/spec/elsewhere.md"), (2, 5));
}

// ------------------------------------------------------------- the corpus

const TOML: &str = "\
[ids]
DOM  = { kind = \"domain\",   shape = \"name\" }
MEC  = { kind = \"mechanic\", shape = \"name\" }
RULE = { kind = \"rule\",     shape = \"name\" }
DEC  = { kind = \"decision\", width = 4 }
";

/// A canon document `id` with `extra` front-matter lines and `body`.
fn doc(id: &str, extra: &str, body: &str) -> String {
    format!("---\nid: {id}\nclass: canon\n{extra}---\n\n# {id}\n\n{body}")
}

/// The invented corpus of the orphan rules, `(path, text)` in path order.
fn corpus() -> Vec<(String, String)> {
    let mut files = vec![
        // A holder with a child, cited strong once and weak from three
        // sources (two sections of one file).
        ("docs/spec/top.md".to_owned(), doc("DOM-TOP", "", "The top.\n")),
        (
            "docs/spec/kid.md".to_owned(),
            doc("MEC-KID", "parent: DOM-TOP\n", "Only its own parent.\n"),
        ),
        (
            "docs/spec/out.md".to_owned(),
            doc(
                "MEC-OUT",
                "links:\n  depends_on: [DOM-TOP]\n",
                "Leans on DOM-TOP.\n",
            ),
        ),
        (
            "docs/spec/two.md".to_owned(),
            doc(
                "MEC-TWO",
                "",
                "## First {#RULE-TWO-A}\n\nCites DOM-TOP.\n\n## Second {#RULE-TWO-B}\n\nCites DOM-TOP again.\n",
            ),
        ),
        // A holder whose child hangs under one of its sections.
        (
            "docs/spec/held.md".to_owned(),
            doc("DOM-HELD", "", "## Part {#RULE-HELD-PART}\n\nA part.\n"),
        ),
        (
            "docs/spec/under.md".to_owned(),
            doc("MEC-UNDER", "parent: RULE-HELD-PART\n", "Under a part.\n"),
        ),
        // Set aside.
        (
            "docs/spec/old.md".to_owned(),
            doc("MEC-OLD", "status: abandoned\n", "Set aside.\n"),
        ),
        // Links within itself only.
        (
            "docs/spec/self.md".to_owned(),
            doc(
                "MEC-SELF",
                "",
                "See RULE-SELF-PART.\n\n## Part {#RULE-SELF-PART}\n\nSee MEC-SELF.\n",
            ),
        ),
        // A dangling link, another project's, a code file's.
        (
            "docs/spec/dangle.md".to_owned(),
            doc(
                "MEC-DANGLE",
                "links:\n  depends_on: [MEC-NOWHERE]\n",
                "Nothing here.\n",
            ),
        ),
        (
            "docs/spec/foreign.md".to_owned(),
            doc(
                "MEC-FOREIGN",
                "",
                "See other:MEC-ELSEWHERE and [the code](../../src/main.rs).\n",
            ),
        ),
        // A link out to a Tier 3 file's node.
        (
            "docs/spec/arch-link.md".to_owned(),
            doc(
                "MEC-ARCH",
                "links:\n  depends_on: [DEC-0001]\n",
                "Leans on a set-aside decision.\n",
            ),
        ),
        (
            "docs/records/DEC-0001.md".to_owned(),
            "---\nid: DEC-0001\nclass: decision\nstatus: rejected\ndate: 2026-01-01\n---\n\n# Rejected\n\nMEC-CITED and MEC-QUIET, both.\n"
                .to_owned(),
        ),
        // Cited only by a generated file and by a Tier 3 one.
        (
            "docs/spec/cited.md".to_owned(),
            doc("MEC-CITED", "", "Cited by no live file.\n"),
        ),
        (
            "docs/generated/map.md".to_owned(),
            "---\nclass: generated\nparent: MEC-GENPARENT\nlinks:\n  depends_on: [MEC-CITED]\n---\n\n# Map\n\nMEC-CITED and MEC-GENPARENT.\n"
                .to_owned(),
        ),
        // A child that is not live.
        (
            "docs/spec/genparent.md".to_owned(),
            doc("MEC-GENPARENT", "", "Its only child is generated.\n"),
        ),
        // A wiki title is no link.
        (
            "docs/spec/wiki.md".to_owned(),
            "---\nid: MEC-WIKI\nclass: canon\ntitle: The wiki page\n---\n\n# The wiki page\n\nA page.\n"
                .to_owned(),
        ),
        (
            "docs/spec/wiki-citer.md".to_owned(),
            doc("MEC-WIKICITER", "", "See [[The wiki page]].\n"),
        ),
        // Nested sections: a use of the inner one is a use of each holder.
        (
            "docs/spec/nest.md".to_owned(),
            doc(
                "MEC-NEST",
                "",
                "## Part {#RULE-NEST-A}\n\nA.\n\n### Sub {#RULE-NEST-B}\n\nB.\n",
            ),
        ),
        (
            "docs/spec/nester.md".to_owned(),
            doc(
                "MEC-NESTER",
                "links:\n  constrains: [RULE-NEST-B]\n",
                "See RULE-NEST-B and RULE-NEST-A.\n",
            ),
        ),
        // Mentioned only.
        (
            "docs/spec/quiet.md".to_owned(),
            doc("MEC-QUIET", "", "Quiet.\n"),
        ),
        (
            "docs/spec/talker.md".to_owned(),
            doc("MEC-TALKER", "", "MEC-QUIET is quiet.\n"),
        ),
        // A failed front-matter: live, judged by its parse.
        (
            "docs/spec/broken.md".to_owned(),
            "---\nid: [MEC-BROKEN\nclass: canon\n---\n\n# Broken\n\nNo link.\n".to_owned(),
        ),
    ];
    files.sort();
    files
}

fn graph_input(files: &[(String, String)]) -> (Config, CheckInput) {
    let config = Config::from_toml(TOML);
    let pairs: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let input = config.input(&pairs);
    (config, input)
}

/// The corpus' answer, by name; the failed front-matter's document by its
/// path, whatever name the graph gives it.
fn corpus_answer(files: &[(String, String)]) -> BTreeMap<String, Counts> {
    let (config, input) = graph_input(files);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let mut named = by_name(&graph);
    let broken = graph.file_of("docs/spec/broken.md").expect("the file");
    let at = graph.document(broken).expect("its document");
    let name = graph.name(at);
    let counts = named.remove(&name).expect("the broken file is live");
    named.insert("<broken>".to_owned(), counts);
    named
}

/// UO2, UO3: the orphan rules node by node. M: one's own `parent:` taken as
/// a link (MEC-KID, MEC-UNDER not orphans); children ignored (DOM-HELD an
/// orphan); `abandoned` ignored (MEC-OLD an orphan); links out ignored
/// (MEC-OUT, MEC-ARCH orphans); a generated source counted (MEC-CITED 1).
#[test]
fn uo3_the_orphan_rules_node_by_node() {
    let files = corpus();
    let named = corpus_answer(&files);
    let want: BTreeMap<String, Counts> = [
        // MEC-OUT strong, MEC-OUT, RULE-TWO-A, RULE-TWO-B weak; MEC-KID's
        // `parent:` none; a child.
        ("DOM-TOP", (1, 3, Some(false))),
        // Its own `parent:` is no link.
        ("MEC-KID", (0, 0, Some(true))),
        ("MEC-OUT", (0, 0, Some(false))),
        ("MEC-TWO", (0, 0, Some(false))),
        ("RULE-TWO-A", (0, 0, None)),
        ("RULE-TWO-B", (0, 0, None)),
        // A child under its section.
        ("DOM-HELD", (0, 0, Some(false))),
        ("RULE-HELD-PART", (0, 0, None)),
        ("MEC-UNDER", (0, 0, Some(true))),
        ("MEC-OLD", (0, 0, Some(false))),
        // Its own links land in its own file.
        ("MEC-SELF", (0, 0, Some(true))),
        ("RULE-SELF-PART", (0, 0, None)),
        ("MEC-DANGLE", (0, 0, Some(true))),
        ("MEC-FOREIGN", (0, 0, Some(true))),
        // A link out to a Tier 3 node is a link out.
        ("MEC-ARCH", (0, 0, Some(false))),
        // Its sources are generated and Tier 3.
        ("MEC-CITED", (0, 0, Some(true))),
        // Its only child is generated.
        ("MEC-GENPARENT", (0, 0, Some(true))),
        ("MEC-WIKI", (0, 0, Some(true))),
        ("MEC-WIKICITER", (0, 0, Some(true))),
        // MEC-NESTER cites RULE-NEST-B strong, B and A weak: one source.
        ("MEC-NEST", (1, 1, Some(false))),
        ("RULE-NEST-A", (1, 1, None)),
        ("RULE-NEST-B", (1, 1, None)),
        ("MEC-NESTER", (0, 0, Some(false))),
        // Mentioned is not an orphan.
        ("MEC-QUIET", (0, 1, Some(false))),
        ("MEC-TALKER", (0, 0, Some(false))),
        ("<broken>", (0, 0, Some(true))),
    ]
    .into_iter()
    .map(|(name, counts)| (name.to_owned(), counts))
    .collect();
    assert_eq!(named, want);
}

/// UO3, the premises: the corpus' links fare as the rules name them (a
/// test whose links all failed to resolve would pass the wrong way).
#[test]
fn uo3_the_corpus_links_fare_as_named() {
    let files = corpus();
    let (config, input) = graph_input(&files);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let find = |name: &str| {
        let mut found = Vec::new();
        for file in 0..graph.paths().len() {
            for ord in 0..graph.nodes(file).len() {
                let at = NodeAt { file, ord };
                if graph.name(at) == name {
                    found.push(at);
                }
            }
        }
        assert_eq!(found.len(), 1, "{name}");
        found[0]
    };
    let held = find("DOM-HELD");
    let part = find("RULE-HELD-PART");
    let under = find("MEC-UNDER");
    assert_eq!(
        graph.listed_under(under),
        Some(part),
        "a child of a section"
    );
    assert_eq!(graph.listed_under(part), Some(held));
    let genparent = find("MEC-GENPARENT");
    let map = graph.file_of("docs/generated/map.md").expect("the map");
    assert_eq!(graph.standing(map), Standing::Generated);
    assert_eq!(
        graph.listed_under(NodeAt { file: map, ord: 0 }),
        Some(genparent),
        "the generated file's `parent:` resolves"
    );
    let record = graph.file_of("docs/records/DEC-0001.md").expect("DEC-0001");
    assert_eq!(graph.standing(record), Standing::Tier3);
    let broken = graph.file_of("docs/spec/broken.md").expect("broken");
    assert_eq!(
        graph.standing(broken),
        Standing::Live,
        "a failed front-matter is live"
    );
    // MEC-CITED is cited, resolved, by both files that are not live.
    let cited = find("MEC-CITED");
    let sources: Vec<String> = graph
        .edges()
        .iter()
        .filter(
            |edge| matches!(edge.written_end(), Endpoint::Nodes(nodes) if nodes.contains(&cited)),
        )
        .map(|edge| graph.paths()[edge.file].to_owned())
        .collect();
    assert!(
        sources.iter().any(|path| path == "docs/generated/map.md")
            && sources
                .iter()
                .any(|path| path == "docs/records/DEC-0001.md"),
        "{sources:?}"
    );
}

/// UO8: the corpus in reverse order gives the same answer, and so does a
/// second build. M: rows taken in input order.
#[test]
fn uo8_the_answer_is_blind_to_input_order() {
    let files = corpus();
    let forward = corpus_answer(&files);
    let mut reversed = files.clone();
    reversed.reverse();
    assert_eq!(corpus_answer(&reversed), forward);
    assert_eq!(corpus_answer(&files), forward);

    let (config, input) = fixture_input(&fixture("spec-a"));
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    let first = by_name(&graph);
    let (config, mut input) = fixture_input(&fixture("spec-a"));
    input.files.reverse();
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    assert_eq!(by_name(&graph), first, "spec-a read in reverse");
}

// ------------------------------------------------------------------ UO8

/// Every string literal of Rust `source`, comments skipped: plain, raw
/// (`r"…"`, `r#"…"#`) and byte strings; char literals and lifetimes are
/// none.
fn string_literals(source: &str) -> Vec<String> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            continue;
        }
        if c == '\'' {
            // A char literal: `'x'`, `'\n'`, `'\''`; else a lifetime.
            if next == Some('\\') {
                let mut j = i + 2;
                while j < chars.len() && chars[j] != '\'' {
                    j += 1;
                }
                i = j + 1;
            } else if chars.get(i + 2) == Some(&'\'') {
                i += 3;
            } else {
                i += 1;
            }
            continue;
        }
        let word_before = i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
        if c == 'r' && !word_before && matches!(next, Some('"' | '#')) {
            let mut j = i + 1;
            let mut hashes = 0;
            while chars.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if chars.get(j) == Some(&'"') {
                let start = j + 1;
                let mut k = start;
                loop {
                    assert!(k < chars.len(), "an unclosed raw string");
                    if chars[k] == '"' && (0..hashes).all(|h| chars.get(k + 1 + h) == Some(&'#')) {
                        break;
                    }
                    k += 1;
                }
                out.push(chars[start..k].iter().collect());
                i = k + 1 + hashes;
                continue;
            }
        }
        if c == '"' {
            let start = i + 1;
            let mut k = start;
            while k < chars.len() && chars[k] != '"' {
                if chars[k] == '\\' {
                    k += 1;
                }
                k += 1;
            }
            out.push(chars[start..k].iter().collect());
            i = k + 1;
            continue;
        }
        i += 1;
    }
    out
}

/// UO8: `src/usage.rs` holds no string literal but `"abandoned"` (the core
/// reads no kind, link type or class name). M: a link type or class name
/// written there.
#[test]
fn uo8_usage_rs_holds_no_literal_but_abandoned() {
    let path = repository_root().join("crates/specengine-core/src/usage.rs");
    let source = std::fs::read_to_string(&path).expect("src/usage.rs");
    let literals = string_literals(&source);
    assert!(
        literals.iter().any(|text| text == "abandoned"),
        "the convention's word is read: {literals:?}"
    );
    let others: Vec<&String> = literals
        .iter()
        .filter(|text| *text != "abandoned")
        .collect();
    assert!(others.is_empty(), "{others:?}");

    // The scanner itself: comments skipped, char literals and lifetimes
    // none, raw and byte strings read.
    assert_eq!(
        string_literals(
            "// \"no\"\n/* \"no\" /* \"no\" */ */ fn f<'a>(x: &'a str) { let q = '\"'; let e = '\\''; \
             g(\"one\", r#\"two \"x\"\"#, b\"three\", \"fo\\\"ur\"); }"
        ),
        ["one", "two \"x\"", "three", "fo\\\"ur"]
    );
}
