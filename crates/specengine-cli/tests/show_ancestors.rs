//! docs/features/flow-scenario.md (RM-0094) FS-8, "The served path": `spec
//! show REF --json` gives each node `ancestors`, the nodes
//! `SpecGraph::ancestors` returns, top first, each `{id, kind, title, path}`,
//! before `links`; `[]` for a root. Over `fixtures/flow-aside` (every
//! document against its `expected.json`), `fixtures/parent-ref` (a `parent:`
//! naming another project), `fixtures/code-a` (`--bindings`, a whole read)
//! and scratch corpora (a dangling `parent:`, a parent ID held twice, a
//! `parent:` cycle, an ID-less document's section), each in its own scratch
//! `HOME`; the fixtures are only read. The text is unchanged. M: nearest
//! first; the last holder; a cycle member twice; `ancestors` printed in the
//! text.
//!
//! FS-9 (the subset read gives the walk's list, and decodes the chain's files
//! alone) is `src/store_reads_tests.rs`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use common::{Run, Scratch, fixture, read_text, snapshot, spec, write};
use serde_json::{Value, json};
use specengine_core::ProjectConfig;
use specengine_core::check::{NodeAt, SpecGraph};
use specengine_store::{WorkingTree, check_input};

const ASIDE: &str = "flow-aside";

/// FS-8's table: a REF and its `ancestors` by ID, top first.
const TABLE: [(&str, &[&str]); 5] = [
    ("HUNCH-0007", &["ATL-POTTERY", "WBAY-KILN"]),
    ("POT-FIRE", &["ATL-POTTERY", "KRUN-FIRING"]),
    (
        "CONE-GLOST-SHELF",
        &["ATL-POTTERY", "WBAY-KILN", "CONE-GLOST"],
    ),
    (
        "docs/workshop/gallery/open-day.md",
        &["ATL-POTTERY", "WBAY-GALLERY"],
    ),
    ("ATL-POTTERY", &[]),
];

/// "Data", "`spec show` JSON": the section's whole list, as printed (with
/// docs/features/node-history.md NH4's `history` last, `null`).
const SHELF_ANCESTORS: &str = "\"ancestors\":[\
{\"id\":\"ATL-POTTERY\",\"kind\":\"atelier\",\"title\":\"The pottery\",\
\"path\":\"docs/workshop/atelier.md\"},\
{\"id\":\"WBAY-KILN\",\"kind\":\"workbay\",\"title\":\"Kiln bay\",\
\"path\":\"docs/workshop/bays/kiln.md\"},\
{\"id\":\"CONE-GLOST\",\"kind\":\"conerule\",\"title\":\"Glost cone\",\
\"path\":\"docs/workshop/cones/glost.md\"}],\"links\":null,\"bindings\":null,\"history\":null}";

/// A shown node's keys, in order: `ancestors` before `links`; 21 since
/// docs/features/node-history.md NH4 (`history` last).
const SHOWN_NODE_KEYS: usize = 21;

// ------------------------------------------------------------------ helpers

struct Reads {
    scratch: Scratch,
    home: PathBuf,
    cwd: PathBuf,
}

impl Reads {
    fn new(name: &str) -> Self {
        let scratch = Scratch::new(name);
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        Self { scratch, home, cwd }
    }

    /// `spec show <args> --root <root>`.
    fn show(&self, root: &Path, args: &[&str]) -> Run {
        let mut all = vec!["show"];
        all.extend_from_slice(args);
        all.extend_from_slice(&["--root", root.to_str().expect("UTF-8 root")]);
        spec(&self.home, &self.cwd, &all)
    }

    /// `spec show <reference> <flags> --json --root <root>`: exit 0, one
    /// node.
    fn shown(&self, root: &Path, reference: &str, flags: &[&str]) -> Value {
        let mut args = vec![reference];
        args.extend_from_slice(flags);
        args.push("--json");
        let run = self.show(root, &args);
        run.code(0);
        let document = run.json();
        let nodes = document["nodes"].as_array().expect("nodes");
        assert_eq!(nodes.len(), 1, "{reference}: one node: {document}");
        nodes[0].clone()
    }
}

/// The names of a node's `ancestors`: each ID, else its path; every entry
/// exactly `{id, kind, title, path}`.
fn chain(node: &Value) -> Vec<String> {
    node["ancestors"]
        .as_array()
        .unwrap_or_else(|| panic!("`ancestors` is no list: {node}"))
        .iter()
        .map(|entry| {
            let keys: BTreeSet<&str> = entry
                .as_object()
                .expect("an entry is an object")
                .keys()
                .map(String::as_str)
                .collect();
            assert_eq!(
                keys,
                BTreeSet::from(["id", "kind", "title", "path"]),
                "{entry}"
            );
            assert!(entry["path"].is_string(), "`path` is the file: {entry}");
            entry["id"]
                .as_str()
                .or_else(|| entry["path"].as_str())
                .expect("a name")
                .to_owned()
        })
        .collect()
}

fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// A scratch project at `<scratch>/<dir>` with `files` under it.
fn project(reads: &Reads, dir: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = reads.scratch.dir(dir);
    write(
        &root,
        "specengine.toml",
        format!(
            "[project]\nslug = \"{dir}\"\n\n[ids]\nRING = {{ kind = \"ring\", shape = \"name\" }}\n"
        ),
    );
    for (path, text) in files {
        write(&root, path, text);
    }
    root
}

/// A document of the scratch corpora.
fn ring(id: Option<&str>, title: &str, parent: Option<&str>, extra: &str) -> String {
    let id_line = id.map(|id| format!("id: {id}\n")).unwrap_or_default();
    let parent_line = parent
        .map(|parent| format!("parent: {parent}\n"))
        .unwrap_or_default();
    format!(
        "---\n{id_line}class: canon\ntitle: {title}\n{parent_line}owner: owner\n\
         reviewed: 2026-10-01\n---\n\n# {title}\n\nA ring of the chain.\n{extra}"
    )
}

/// The whole graph of `root`, by its config.
fn with_graph<T>(root: &Path, read: impl FnOnce(&SpecGraph<'_>) -> T) -> T {
    let text = read_text(root, "specengine.toml");
    let config = ProjectConfig::from_toml(&text).expect("the config");
    let tree = WorkingTree::new(root, &config.paths).expect("the working tree");
    let input = check_input(&tree, &config.scheme);
    let graph = SpecGraph::new(&input, &config.scheme, &config.paths);
    read(&graph)
}

/// The node of the whole graph whose name is `name`.
fn find(graph: &SpecGraph<'_>, name: &str) -> NodeAt {
    let mut found = Vec::new();
    for file in 0..graph.paths().len() {
        for ord in 0..graph.nodes(file).len() {
            let at = NodeAt { file, ord };
            if graph.node(at).and_then(|node| node.id.as_deref()) == Some(name) {
                found.push(at);
            }
        }
    }
    assert_eq!(found.len(), 1, "{name}: {found:?}");
    found[0]
}

// ------------------------------------------------------------------ flow-aside

/// FS-8's table on the fixture, plain and with `--links`; the section's list
/// as "Data" prints it, before `links`, in a node of 21 keys.
#[test]
fn fs8_the_table_on_the_fixture() {
    let reads = Reads::new("anc-table");
    let root = fixture(ASIDE);
    let before = snapshot(&root);
    for (reference, want) in TABLE {
        let plain = reads.shown(&root, reference, &[]);
        assert_eq!(chain(&plain), owned(want), "{reference}: {plain}");
        assert_eq!(
            plain.as_object().expect("a node").len(),
            SHOWN_NODE_KEYS,
            "{reference}: {plain}"
        );
        let linked = reads.shown(&root, reference, &["--links"]);
        assert_eq!(
            linked["ancestors"], plain["ancestors"],
            "{reference}: --links gives the same list"
        );
    }
    let run = reads.show(&root, &["CONE-GLOST-SHELF", "--json"]);
    run.code(0);
    assert!(
        run.stdout.contains(&format!(",{SHELF_ANCESTORS}]}}")),
        "the list as Data prints it, before `links`:\n{}",
        run.stdout
    );
    assert_eq!(snapshot(&root), before, "the fixture is only read");
}

/// FS-8: every document of the fixture against `expected.json`'s
/// `ancestors`, top first, by its ID or (ID-less) its path.
#[test]
fn fs8_every_document_of_the_fixture() {
    let reads = Reads::new("anc-every");
    let root = fixture(ASIDE);
    let expected: Value =
        serde_json::from_str(&read_text(&root, "expected.json")).expect("expected.json");
    let mut deep = 0;
    for (path, file) in expected["files"].as_object().expect("files") {
        let reference = file["id"].as_str().unwrap_or(path);
        let want: Vec<String> = file["ancestors"]
            .as_array()
            .expect("ancestors")
            .iter()
            .map(|name| name.as_str().expect("a name").to_owned())
            .collect();
        let shown = reads.shown(&root, reference, &[]);
        assert_eq!(chain(&shown), want, "{reference}");
        assert_eq!(shown["path"], json!(path), "{reference}");
        if want.len() >= 2 {
            deep += 1;
        }
    }
    assert!(deep > 140, "premise: the records stand two deep: {deep}");
}

/// FS-8 "`spec show`'s text is unchanged": no `ancestors` line, and no name
/// or title of the chain in a section's text, which holds neither.
#[test]
fn fs8_the_text_names_no_ancestor() {
    let reads = Reads::new("anc-text");
    let root = fixture(ASIDE);
    for flags in [&[][..], &["--links"][..]] {
        let mut args = vec!["CONE-GLOST-SHELF"];
        args.extend_from_slice(flags);
        let run = reads.show(&root, &args);
        run.code(0);
        for word in [
            "ancestors",
            "ATL-POTTERY",
            "The pottery",
            "WBAY-KILN",
            "Kiln bay",
            "Glost cone",
        ] {
            assert!(
                !run.stdout.contains(word),
                "{flags:?}: {word:?} in the text:\n{}",
                run.show()
            );
        }
    }
}

// ------------------------------------------------------------------ other fixtures

/// FS-8 on `parent-ref`: AUK-04's `parent:` names another project, so its
/// chain ends at once.
#[test]
fn fs8_a_parent_in_another_project_ends_the_chain() {
    let reads = Reads::new("anc-project");
    let root = fixture("parent-ref");
    let shown = reads.shown(&root, "AUK-04", &[]);
    assert_eq!(chain(&shown), Vec::<String>::new(), "{shown}");
}

/// FS-8 on `code-a`, where `--bindings` takes the whole read: each list is
/// plain `show`'s, a section's (under its document) among them.
#[test]
fn fs8_bindings_give_the_lists_of_plain_show() {
    let reads = Reads::new("anc-code");
    let root = fixture("code-a");
    let mut non_empty = 0;
    for reference in [
        "PIECE-COUNTER",
        "LAW-BOUND",
        "PIECE-LIMITS",
        "PREM-001",
        "GOAL-01",
        "cap-check/PASS-01",
        "docs/spec/counter.md",
    ] {
        let plain = reads.shown(&root, reference, &[]);
        let bound = reads.shown(&root, reference, &["--bindings"]);
        assert_eq!(
            bound["ancestors"], plain["ancestors"],
            "{reference}: the whole read's list is the subset's"
        );
        chain(&plain);
        if !plain["ancestors"].as_array().expect("a list").is_empty() {
            non_empty += 1;
        }
    }
    assert!(non_empty > 0, "premise: some chain is not empty");
}

// ------------------------------------------------------------------ scratch corpora

/// FS-8: a `parent:` dangling two levels up ends the chain at the node
/// holding it.
#[test]
fn fs8_a_dangling_parent_ends_the_chain_there() {
    let reads = Reads::new("anc-dangling");
    let root = project(
        &reads,
        "dangling",
        &[
            (
                "docs/spec/ring-top.md",
                &ring(Some("RING-TOP"), "The top", Some("RING-GONE"), ""),
            ),
            (
                "docs/spec/ring-mid.md",
                &ring(Some("RING-MID"), "The middle", Some("RING-TOP"), ""),
            ),
            (
                "docs/spec/ring-low.md",
                &ring(Some("RING-LOW"), "The bottom", Some("RING-MID"), ""),
            ),
        ],
    );
    let low = reads.shown(&root, "RING-LOW", &[]);
    assert_eq!(chain(&low), ["RING-TOP", "RING-MID"], "{low}");
    let top = reads.shown(&root, "RING-TOP", &[]);
    assert_eq!(chain(&top), Vec::<String>::new(), "{top}");
}

/// FS-8: a parent ID held by two files gives the first by (path, ord), and
/// the chain goes on from it. M: the last holder.
#[test]
fn fs8_a_parent_held_twice_is_its_first_holder() {
    let reads = Reads::new("anc-twins");
    let root = project(
        &reads,
        "twins",
        &[
            (
                "docs/spec/a-twin.md",
                &ring(Some("RING-TWIN"), "The first twin", Some("RING-ALPHA"), ""),
            ),
            (
                "docs/spec/b-twin.md",
                &ring(Some("RING-TWIN"), "The second twin", Some("RING-BETA"), ""),
            ),
            (
                "docs/spec/c-kid.md",
                &ring(Some("RING-KID"), "The kid", Some("RING-TWIN"), ""),
            ),
            (
                "docs/spec/d-alpha.md",
                &ring(Some("RING-ALPHA"), "Alpha", None, ""),
            ),
            (
                "docs/spec/e-beta.md",
                &ring(Some("RING-BETA"), "Beta", None, ""),
            ),
        ],
    );
    let kid = reads.shown(&root, "RING-KID", &[]);
    assert_eq!(chain(&kid), ["RING-ALPHA", "RING-TWIN"], "{kid}");
    assert_eq!(kid["ancestors"][1]["path"], "docs/spec/a-twin.md", "{kid}");
    assert_eq!(kid["ancestors"][1]["title"], "The first twin", "{kid}");
}

/// FS-8: in a cycle A to B with C under A, C's list is the whole graph's
/// `ancestors` reversed, each member once. M: a cycle member twice.
#[test]
fn fs8_a_cycle_lists_each_member_once() {
    let reads = Reads::new("anc-cycle");
    let root = project(
        &reads,
        "cycle",
        &[
            (
                "docs/spec/ring-a.md",
                &ring(Some("RING-A"), "Ring a", Some("RING-B"), ""),
            ),
            (
                "docs/spec/ring-b.md",
                &ring(Some("RING-B"), "Ring b", Some("RING-A"), ""),
            ),
            (
                "docs/spec/ring-c.md",
                &ring(Some("RING-C"), "Ring c", Some("RING-A"), ""),
            ),
        ],
    );
    let want: Vec<String> = with_graph(&root, |graph| {
        let mut names: Vec<String> = graph
            .ancestors(find(graph, "RING-C"))
            .into_iter()
            .map(|at| graph.name(at))
            .collect();
        names.reverse();
        names
    });
    assert!(!want.is_empty(), "premise: the whole graph's chain");
    let shown = reads.shown(&root, "RING-C", &[]);
    let got = chain(&shown);
    assert_eq!(got, want, "{shown}");
    let once: BTreeSet<&String> = got.iter().collect();
    assert_eq!(once.len(), got.len(), "each member once: {got:?}");
    for member in ["RING-A", "RING-B"] {
        let shown = reads.shown(&root, member, &[]);
        let got = chain(&shown);
        let once: BTreeSet<&String> = got.iter().collect();
        assert_eq!(once.len(), got.len(), "{member}: each member once: {got:?}");
        assert!(
            !got.contains(&member.to_owned()),
            "{member} is not its own ancestor"
        );
    }
}

/// FS-8 "`path` is in each entry because a section's enclosing document may
/// have no ID": a section of an ID-less document lists it with `id`, `kind`
/// `null` and its path, below the document's `parent:`.
#[test]
fn fs8_an_id_less_document_stands_in_the_chain_by_its_path() {
    let reads = Reads::new("anc-plain");
    let root = project(
        &reads,
        "plain",
        &[
            (
                "docs/spec/ring-top.md",
                &ring(Some("RING-TOP"), "The top", None, ""),
            ),
            (
                "docs/spec/plain.md",
                &ring(
                    None,
                    "A plain page",
                    Some("RING-TOP"),
                    "\n## Inner part {#RING-INNER}\n\nA part with an ID.\n",
                ),
            ),
        ],
    );
    let inner = reads.shown(&root, "RING-INNER", &[]);
    assert_eq!(chain(&inner), ["RING-TOP", "docs/spec/plain.md"], "{inner}");
    assert_eq!(
        inner["ancestors"][1],
        json!({"id": null, "kind": null, "title": "A plain page", "path": "docs/spec/plain.md"}),
        "{inner}"
    );
}
