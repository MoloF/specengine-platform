//! AC-15 of docs/features/mcp-read.md: corpora driving each read tool to
//! the CLI's 40 000-character cut, plus files of control characters (each
//! escaped to six characters in JSON): the text before the tail, notes and
//! warnings aside, is at most 40 000 characters (a bundle: its body), and
//! the text plus the serialized `structuredContent` is at most
//! `specengine_mcp::MAX_RESULT_CHARS`, counted in characters and in UTF-16
//! units (R1). Every answer is also checked for parity with the CLI library.
//! Each measurement is printed (`--no-capture`) for the spec's record.
//!
//! M: the declared value below the measurement.
//!
//! RD-6 and TE-1 (`mcp_size`) of docs/features/code-tools.md (6b-1): a
//! code corpus of 12 000 items in two files and 1 200 one-item files drives
//! `find_symbols`, `refs` and `unmapped` past the cut; each answer is the
//! CLI library's, carries the CLI's tail, and its text plus serialized
//! `structuredContent` stays within `MAX_RESULT_CHARS` (uncut, `unmapped`'s
//! JSON alone passes 1 000 000 characters). The maxima are printed for the
//! spec's "Implementation". M: `unmapped` JSON uncut.
//!
//! docs/features/bevy-roles-reads.md TE-8 ("`mcp_size` maxima on code-bevy
//! recorded"): `find_symbols` over a scratch `fixtures/code-bevy` (the
//! opt-in on, roles in every answer) in every form and over every code
//! file, each answer the CLI library's and within the caps; the maximum is
//! printed for the spec's "Implementation"
//! (`find_symbols_over_code_bevy_records_its_maxima`).
//!
//! docs/features/check-binding.md TE-2 ("Size"): the same corpus is
//! committed in git, and `check_binding {node: "LANE-WIDE"}` (its 3 000
//! bound items, no lock: 3 000 `unlocked` rows, `observed`) joins
//! `code_tools_cut_stay_under_the_declared_cap`: past the CLI's cut, its
//! JSON holding only the printed rows (`omitted.rows` the rest) and its
//! whole `delta`; one QPATH narrows it to one row, uncut. Its maximum is
//! printed with the others.
//!
//! docs/features/show-bindings.md TE-5: `get_node {id: "LANE-WIDE", with:
//! ["bindings"]}` (and with `"links"` too) joins the same test: its 3 000
//! rows pass the CLI's cut, the tail ends `; bindings not shown: <k>`, the
//! JSON's `bindings.rows` the printed ones and `omitted` k; its maximum is
//! printed with the others (`get_node` re-measured with rows).
//!
//! docs/features/bundle-bindings.md TE-5: over the same corpus (3 000
//! rows of `LANE-WIDE`) and 64 more bound items with 9 000-character names
//! sorting first, `get_context_bundle` at u32::MAX (rows past the ceiling,
//! the body within 40 000 characters, the JSON its placed rows) and at the
//! default budget, and `get_task` on a task of `LANE-WIDE` (its 64 long
//! rows past `PACKAGE_BUDGET`, rows leaving from the last), each the CLI
//! library's and within `MAX_RESULT_CHARS`; the maxima printed
//! (`bundle_and_task_with_rows_stay_under_the_declared_cap`).
//!
//! docs/features/impact.md TE-6: `get_impact {id: "LANE-WIDE"}` and
//! `{symbol: <a bound item's name>}` join
//! `code_tools_cut_stay_under_the_declared_cap`: the 3 000 rows past the
//! CLI's cut, the JSON the printed rows, `truncated.symbols` the rest, the
//! tail last; the maximum printed with the others.
//! docs/features/impact-since.md TE-5: `get_impact {since: "HEAD"}` over
//! the same corpus with 3 000 untracked items joins it (the delta cut).
//!
//! docs/features/flow-read.md AC-07: `get_flow` over 4 000 short steps (JSON
//! several times the text, every edge past the cut) and over titles of
//! control characters: each answer the CLI library's, the text before the
//! tail within the cut, the sum within `MAX_RESULT_CHARS`; the maximum
//! printed (`flow_cut_stays_under_the_declared_cap`).
//!
//! docs/features/doc-modal.md (RM-0095) DM-1 and DM-3: a link's end and a
//! gap's member now carry their titles in the JSON alone, uncut, as corpus
//! content: on titles of control characters (six-fold in JSON) the links of
//! `MEC-STAMINA` and the flow ring pass `MAX_RESULT_CHARS`, the residue
//! docs/canon/mcp-read.md "Size" lists as long corpus titles. The cap tests
//! keep the bounded claim on the same corpora with ordinary titles of the
//! same length (control characters stay in the questions' bodies); the
//! residue is pinned to its measured size on the control titles
//! (`doc_modal_control_title_residue_is_its_measured_size`), so a later
//! change shows.
//!
//! docs/features/card-context.md (RM-0108) CC11: `get_card {context: true,
//! history: true}` on CC3 (b)'s subtask (a 4 096 B goal, 32 criteria of
//! 1 024 B, 64 targets, its parent's goal 4 096 B) and on CC3 (a)'s card
//! (three long targets, 25 criteria of 1 000 B) holding 64 items, each a
//! decision with a 1 024 B text, a 2 048 B working answer and a 2 048 B
//! `why`: each the CLI library's, the text before notes and tail within
//! 40 000 characters, text plus JSON within `MAX_RESULT_CHARS`; the maximum
//! pinned ([`CARD_CONTEXT_MAXIMUM`]) and printed for "Implementation"
//! (`card_context_stays_under_the_declared_cap`). docs/features/upstream-signal.md
//! US-10, the size: CC3 (a)'s card is a subtask of CC3 (b)'s parent, whose
//! goal, targets and criteria are then edited twenty times as a 128-byte
//! name (`CARD_PERSON_MAX`), so its `upstream` lists 16 changes, `more` 4;
//! the maximum re-pinned, within 500 000.
//!
//! docs/features/node-history.md NH14, the size: `get_node {id: "R-12",
//! with: ["links", "bindings", "history"]}` on NH10's two nodes (64 apply
//! commits each with a 300-character rationale, under a 20 000-character
//! text; and under a 470-character `Decided-by:`, the history past the
//! cut): each the CLI library's, within the cut and `MAX_RESULT_CHARS`;
//! the maximum pinned ([`NODE_HISTORY_MAXIMUM`]) and printed for
//! "Implementation" (`node_history_stays_under_the_declared_cap`).
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::path::Path;

use common::read::{Era, Session, checked_call, cli_env, content_text, output_schemas};
use common::*;
use serde_json::{Value, json};
use specengine_cli::{Globals, OUTPUT_CAP_CHARS};
use specengine_mcp::MAX_RESULT_CHARS;

/// Every C0 control character a Markdown line can hold (no tab, line feed
/// or carriage return).
fn controls() -> String {
    (1u8..32)
        .filter(|byte| ![9, 10, 13].contains(byte))
        .map(char::from)
        .collect()
}

/// Characters and UTF-16 units of the text plus the serialized
/// `structuredContent`.
fn sizes(result: &Value) -> (usize, usize, usize, usize) {
    let text = content_text(result);
    let structured = result
        .get("structuredContent")
        .map(|value| serde_json::to_string(value).expect("serialize"))
        .unwrap_or_default();
    let chars = text.chars().count() + structured.chars().count();
    let units = text.encode_utf16().count() + structured.encode_utf16().count();
    (
        text.chars().count(),
        structured.chars().count(),
        chars,
        units,
    )
}

/// The text without its note and warning lines and its tail line.
fn before_tail(text: &str) -> String {
    text.split_inclusive('\n')
        .filter(|line| {
            !line.starts_with("note: ")
                && !line.starts_with("warning: ")
                && !line.starts_with("[truncated: ")
        })
        .collect()
}

/// Runs `calls` against a project and checks every answer's size; returns
/// the largest sum seen.
fn measure(label: &str, root: &Path, home: &Path, calls: &[(&str, Value)]) -> usize {
    let env = cli_env(root, Some(home));
    let globals = Globals::default();
    let mut session = Session::open(Era::Legacy, &[], Some(root), Home::At(home));
    let schemas = output_schemas(&session.tools());
    let mut largest = 0;
    for (tool, args) in calls {
        let result = checked_call(&mut session, &schemas, tool, args.clone(), &env, &globals);
        assert_eq!(result["isError"], json!(false), "{label} {tool} {args}");
        let (text, structured, chars, units) = sizes(&result);
        eprintln!(
            "AC-15 {label} {tool} {args}: text {text}, structuredContent {structured}, \
             sum {chars} chars, {units} UTF-16 units"
        );
        let shown = if *tool == "get_context_bundle" {
            result["structuredContent"]["body"]
                .as_str()
                .expect("body")
                .chars()
                .count()
        } else {
            before_tail(content_text(&result)).chars().count()
        };
        assert!(
            shown <= OUTPUT_CAP_CHARS,
            "{label} {tool} {args}: {shown} characters before the tail"
        );
        assert!(
            chars as u64 <= MAX_RESULT_CHARS && units as u64 <= MAX_RESULT_CHARS,
            "{label} {tool} {args}: text + structuredContent = {chars} characters \
             ({units} UTF-16 units), over MAX_RESULT_CHARS = {MAX_RESULT_CHARS}"
        );
        largest = largest.max(chars).max(units);
    }
    let done = session.finish();
    assert!(done.status.success(), "{label}: {}", done.stderr);
    largest
}

#[test]
fn ac15_trees_and_searches_cut_stay_under_the_declared_cap() {
    let scratch = Scratch::new("size-tree");
    let home = scratch.home("h");
    // Short lines: JSON several times the text.
    let short = scratch.dir("short");
    write(&short, "specengine.toml", "[project]\nslug = \"short\"\n");
    for n in 0..4_000 {
        write(&short, &format!("docs/spec/{n:04}.md"), "x\n");
    }
    let mut largest = measure("short-lines", &short, &home, &[("get_tree", json!({}))]);
    // Titles of control characters.
    let control = scratch.dir("control");
    write(
        &control,
        "specengine.toml",
        "[project]\nslug = \"control\"\n",
    );
    let c = controls();
    for n in 0..300 {
        write(
            &control,
            &format!("docs/spec/t/{n:03}.md"),
            format!("# {} zzq\n\nzzq {}\n", c.repeat(7), c.repeat(7)),
        );
    }
    largest = largest.max(measure(
        "control-titles",
        &control,
        &home,
        &[
            ("get_tree", json!({})),
            ("search", json!({"query": "zzq", "limit": 200})),
        ],
    ));
    eprintln!("AC-15 tree/search maximum: {largest}");
}

/// docs/features/flow-read.md AC-07: the read that lists a node, a line and
/// an edge per document stays within the cut and the declared cap, with
/// rows of 50 items printed above them. Parity here compares the library
/// with itself: CLI `flow_read.rs` is what holds JSON to the printed items.
#[test]
fn flow_cut_stays_under_the_declared_cap() {
    let scratch = Scratch::new("size-flow");
    let home = scratch.home("h");
    // Short lines: JSON several times the text; `[flow]` makes every step
    // an item of two rows, 50 printed.
    let short = scratch.dir("short");
    write(
        &short,
        "specengine.toml",
        "[project]\nslug = \"short\"\n\n[ids]\nS = { kind = \"pace\", width = 4 }\n\n\
         [[flow.step]]\nkinds = [\"pace\"]\n\n[[flow.start]]\npaths = [\"docs/spec/s-none.md\"]\n\n\
         [[flow.end]]\npaths = [\"docs/spec/s-none.md\"]\n",
    );
    for n in 1..=4_000 {
        let links = if n % 2 == 0 {
            format!("links:\n  follows: [S-{:04}]\n", n - 1)
        } else {
            String::new()
        };
        write(
            &short,
            &format!("docs/spec/s-{n:04}.md"),
            format!("---\nid: S-{n:04}\n{links}---\n\n# x\n"),
        );
    }
    let mut largest = measure(
        "flow-short-lines",
        &short,
        &home,
        &[
            ("get_flow", json!({})),
            ("get_flow", json!({"depth": 0})),
            ("get_flow", json!({"root": "S-0001"})),
        ],
    );
    // A ring of 300 long titles, ordinary since doc-modal (DM-3: the
    // cycle row titles its 300 members too; the control titles' residue is
    // `doc_modal_control_title_residue_is_its_measured_size`).
    let ring = scratch.dir("ring");
    flow_ring(&ring, &ordinary(controls().chars().count() * 7));
    largest = largest.max(measure(
        "flow-ring-titles",
        &ring,
        &home,
        &[("get_flow", json!({}))],
    ));
    eprintln!("flow-read get_flow maximum: {largest}");
}

/// `n` ordinary characters: a title as long as one of control characters,
/// with no escape in JSON.
fn ordinary(n: usize) -> String {
    "w".repeat(n)
}

/// The flow ring of `flow_cut_stays_under_the_declared_cap`: 300 steps,
/// each following the one before, the first the last, every title `title`
/// then ` zzq`.
fn flow_ring(root: &Path, title: &str) {
    write(
        root,
        "specengine.toml",
        "[project]\nslug = \"control\"\n\n[ids]\nS = { kind = \"pace\", width = 4 }\n",
    );
    for n in 1..=300 {
        let to = if n == 1 { 300 } else { n - 1 };
        write(
            root,
            &format!("docs/spec/s-{n:04}.md"),
            format!(
                "---\nid: S-{n:04}\nlinks:\n  follows: [S-{to:04}]\n---\n\n# {title} zzq\n\nzzq\n"
            ),
        );
    }
}

/// The questions of `ac15_nodes_and_bundles_cut_stay_under_the_declared_cap`:
/// a copy of spec-a with 3 000 open questions on MEC-STAMINA, each titled
/// `title` then its number, its body `body`.
fn questions(scratch: &Scratch, dir: &str, title: &str, body: &str) -> std::path::PathBuf {
    let root = scratch.copy("spec-a", dir);
    replace(
        &root,
        "specengine.toml",
        "TERM = { kind = \"term\",        shape = \"name\" }\n",
        "TERM = { kind = \"term\",        shape = \"name\" }\nOQ   = { kind = \"question\",    width = 4 }\n",
    );
    for n in 0..3_000 {
        write(
            &root,
            &format!("docs/records/OQ/OQ-{n:04}.md"),
            format!(
                "---\nid: OQ-{n:04}\nclass: canon\nstatus: open\nworking_answer: A-101\n\
                 refs: [MEC-STAMINA]\nowner: owner\nreviewed: 2026-09-20\n---\n\n# {title}{n}\n\n\
                 {body}\n"
            ),
        );
    }
    root
}

/// The sizes of one call, the CLI library's answer within the CLI's cut, by
/// `measure`'s rules but the cap: `(characters, UTF-16 units)` of the text
/// plus the serialized `structuredContent`.
fn residue(root: &Path, home: &Path, tool: &str, args: Value) -> (usize, usize) {
    let env = cli_env(root, Some(home));
    let globals = Globals::default();
    let mut session = Session::open(Era::Legacy, &[], Some(root), Home::At(home));
    let schemas = output_schemas(&session.tools());
    let result = checked_call(&mut session, &schemas, tool, args.clone(), &env, &globals);
    assert_eq!(result["isError"], json!(false), "{tool} {args}");
    let (text, structured, chars, units) = sizes(&result);
    eprintln!(
        "doc-modal residue {tool} {args}: text {text}, structuredContent {structured}, sum \
         {chars} chars, {units} UTF-16 units"
    );
    assert!(
        before_tail(content_text(&result)).chars().count() <= OUTPUT_CAP_CHARS,
        "{tool} {args}: the text within the CLI's cut"
    );
    let done = session.finish();
    assert!(done.status.success(), "{}", done.stderr);
    (chars, units)
}

/// docs/features/doc-modal.md DM-1 and DM-3, the residue (docs/canon/mcp-read.md
/// "Size", long corpus titles): on the control-character titles the cap
/// tests had before the slice, `get_node {id: "MEC-STAMINA", with:
/// ["links"]}` over 3 000 questions and `get_flow {}` over the ring of 300
/// are each their measured size (the 2026-11-10 measurement), above
/// `MAX_RESULT_CHARS`: the titles of link ends and gap members are corpus
/// content, uncut. A change of either size shows here.
#[test]
fn doc_modal_control_title_residue_is_its_measured_size() {
    let scratch = Scratch::new("size-residue");
    let home = scratch.home("h");
    let c = controls();
    let root = questions(&scratch, "questions", &c.repeat(3), "x");
    let links = residue(
        &root,
        &home,
        "get_node",
        json!({"id": "MEC-STAMINA", "with": ["links"]}),
    );
    let ring = scratch.dir("ring");
    flow_ring(&ring, &c.repeat(7));
    let flow = residue(&ring, &home, "get_flow", json!({}));
    // docs/features/node-history.md: each node's `"history":null` (15
    // characters) since node-history.
    assert_eq!(links, (549_473, 549_473), "get_node with links");
    assert_eq!(flow, (576_111, 576_111), "get_flow");
    for (chars, _) in [links, flow] {
        assert!(
            chars as u64 > MAX_RESULT_CHARS,
            "the residue is past the cap"
        );
    }
}

#[test]
fn ac15_nodes_and_bundles_cut_stay_under_the_declared_cap() {
    let scratch = Scratch::new("size-node");
    let home = scratch.home("h");
    let c = controls();
    // A document of control characters.
    let control = scratch.dir("control");
    write(
        &control,
        "specengine.toml",
        "[project]\nslug = \"control\"\n",
    );
    write(
        &control,
        "docs/spec/ctrl.md",
        format!(
            "---\nclass: canon\n---\n\n# T{}\n\n{}",
            c.repeat(5),
            format!("{}\n", c.repeat(3)).repeat(3_000)
        ),
    );
    let mut largest = measure(
        "control-document",
        &control,
        &home,
        &[("get_node", json!({"id": "docs/spec/ctrl.md"}))],
    );
    // 3 000 open questions on MEC-STAMINA: the links block and the bundle
    // at the ceiling. Since doc-modal (DM-1) each link carries its end's
    // title, so the titles are ordinary, as long as the control ones were,
    // and the control characters sit in each body; the control titles'
    // residue is `doc_modal_control_title_residue_is_its_measured_size`.
    let root = questions(
        &scratch,
        "questions",
        &ordinary(c.chars().count() * 3),
        &c.repeat(3),
    );
    largest = largest.max(measure(
        "questions",
        &root,
        &home,
        &[
            ("get_node", json!({"id": "MEC-STAMINA", "with": ["links"]})),
            (
                "get_context_bundle",
                json!({"node_ids": ["MEC-STAMINA"], "budget": 1_000_000}),
            ),
            (
                "get_context_bundle",
                json!({"node_ids": ["MEC-STAMINA"], "budget": 12_000}),
            ),
        ],
    ));
    eprintln!("AC-15 node/bundle maximum: {largest}");
}

/// A glossary-like document of 8 000 ID sections (iteration 1: 702 503
/// characters, the tail naming every hidden section, JSON every section):
/// the cut text stays within 40 000 characters, the tail names 20 hidden
/// sections then `, <k> more`, the JSON `sections` only the printed ones and
/// `omitted.sections` 20 (Data, CLI `show` cut); plain and with links.
/// M: the `show` tail unbounded.
#[test]
fn ac15_a_document_of_many_sections_stays_under_the_declared_cap() {
    let scratch = Scratch::new("size-sections");
    let home = scratch.home("h");
    let root = scratch.dir("glossary");
    write(
        &root,
        "specengine.toml",
        "[project]\nslug = \"glossary\"\n\n[ids]\nTERM = { kind = \"term\", shape = \"name\" }\n",
    );
    let mut text = String::from("---\nclass: canon\n---\n\n# Glossary\n\n");
    for n in 0..8_000 {
        text.push_str(&format!(
            "## The meaning of glossary entry number {n} in plain words {{#TERM-glossary-entry-{n:04}}}\n\n\
             Entry {n} explained.\n\n"
        ));
    }
    write(&root, "docs/spec/glossary.md", text);
    let largest = measure(
        "many-sections",
        &root,
        &home,
        &[
            ("get_tree", json!({})),
            ("get_node", json!({"id": "docs/spec/glossary.md"})),
            (
                "get_node",
                json!({"id": "docs/spec/glossary.md", "with": ["links"]}),
            ),
        ],
    );
    eprintln!("AC-15 many-sections maximum: {largest}");
    let mut session = Session::open(Era::Legacy, &[], Some(&root), Home::At(&home));
    for args in [
        json!({"id": "docs/spec/glossary.md"}),
        json!({"id": "docs/spec/glossary.md", "with": ["links"]}),
    ] {
        let reply = session.call("get_node", args.clone());
        let result = result(&reply);
        let text = content_text(result);
        let tail = text
            .lines()
            .find(|line| line.starts_with("[truncated: "))
            .expect("a tail");
        assert_eq!(
            tail.matches("TERM-glossary-entry-").count(),
            20,
            "{args}: the tail names 20 sections"
        );
        assert!(
            tail.contains(" more; holders not shown: none"),
            "{args}: {}",
            clip(tail)
        );
        let node = &result["structuredContent"]["nodes"][0];
        let sections = node["sections"].as_array().unwrap();
        assert!(
            !sections.is_empty() && sections.len() < 1_000,
            "{args}: {} sections in JSON",
            sections.len()
        );
        assert_eq!(node["omitted"]["sections"].as_array().unwrap().len(), 20);
        assert_eq!(
            sections.len() + 20 + node["omitted"]["sections_more"].as_u64().unwrap() as usize,
            8_000,
            "{args}: printed + named + counted"
        );
    }
    assert!(session.finish().status.success());
}

// ------------------------------------------------------- code-tools RD-6

/// One long item name of the size corpus (`n` < 100 000).
fn wide_name(n: usize) -> String {
    format!("item_with_a_rather_long_name_for_the_size_corpus_{n:05}")
}

/// A code corpus past every cut: `pkg/w/src/bound.rs` with 3 000 items,
/// each bound to `LANE-WIDE` by a marker; `pkg/w/src/loose.rs` with 9 000
/// unbound items; `pkg/w/src/many/` with 1 200 one-item files. Its own
/// prefix and kind (no fixture's), one node.
fn wide_code(root: &Path) {
    write(
        root,
        "specengine.toml",
        "[project]\nslug = \"wide-code\"\n\n[ids]\nLANE = { kind = \"lane\", shape = \"name\" }\n\n\
         [code]\nroots = [\"pkg\"]\n",
    );
    write(
        root,
        "docs/spec/lanes.md",
        "---\nclass: canon\nid: LANE-WIDE\nowner: owner\nreviewed: 2026-01-01\n---\n\n# Wide\n\n\
         One lane bound by many items.\n",
    );
    write(
        root,
        "pkg/w/Cargo.toml",
        "[package]\nname = \"w\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n",
    );
    write(
        root,
        "pkg/w/src/lib.rs",
        "pub mod bound;\npub mod loose;\npub mod many;\n",
    );
    let mut bound = String::new();
    for n in 0..3_000 {
        bound.push_str(&format!(
            "// @implements LANE-WIDE@1\npub fn {}() -> u32 {{\n    {n}\n}}\n",
            wide_name(n)
        ));
    }
    write(root, "pkg/w/src/bound.rs", bound);
    let mut loose = String::new();
    for n in 3_000..12_000 {
        loose.push_str(&format!(
            "pub fn {}() -> u32 {{\n    {n}\n}}\n",
            wide_name(n)
        ));
    }
    write(root, "pkg/w/src/loose.rs", loose);
    let mut modules = String::new();
    for n in 0..1_200 {
        modules.push_str(&format!("pub mod m{n:04};\n"));
        write(
            root,
            &format!("pkg/w/src/many/m{n:04}.rs"),
            format!("pub fn {}() -> u32 {{\n    {n}\n}}\n", wide_name(n)),
        );
    }
    write(root, "pkg/w/src/many/mod.rs", modules);
}

/// RD-6: every code tool's answer over [`wide_code`] is the CLI library's
/// (parity and the output schema), is cut by the CLI (its text ends in a
/// `[truncated: …]` tail and holds at most `OUTPUT_CAP_CHARS` plus that
/// tail and the summary line: reading L14 of the phase A report), and its
/// text plus serialized `structuredContent` is within `MAX_RESULT_CHARS`
/// in characters and UTF-16 units. The maxima are printed.
#[test]
fn code_tools_cut_stay_under_the_declared_cap() {
    let scratch = Scratch::new("size-code");
    let home = scratch.home("h");
    let root = scratch.dir("wide");
    wide_code(&root);
    // docs/features/check-binding.md TE-2: committed, so `check_binding`
    // judges a worktree (no git: a run cause, no row).
    let sandbox = git::Sandbox::new(scratch.path());
    sandbox.init(&root);
    sandbox.add_all(&root);
    sandbox.commit(&root, "the wide corpus");
    let env = cli_env(&root, Some(&home));
    let globals = Globals::default();
    let calls = [
        ("find_symbols", json!({"node": "LANE-WIDE"})),
        ("find_symbols", json!({"file": "pkg/w/src/loose.rs"})),
        ("find_symbols", json!({"file": "pkg/w/src/bound.rs"})),
        ("refs", json!({"id": "LANE-WIDE"})),
        ("unmapped", json!({"path": "pkg/w/src/loose.rs"})),
        ("unmapped", json!({"path": "pkg/w/src/many"})),
        ("check_binding", json!({"node": "LANE-WIDE"})),
        ("get_node", json!({"id": "LANE-WIDE", "with": ["bindings"]})),
        (
            "get_node",
            json!({"id": "LANE-WIDE", "with": ["links", "bindings"]}),
        ),
        // docs/features/impact.md TE-6: the radius of LANE-WIDE (its 3 000
        // rows past the cut), by its ID and by a bound item's name (the
        // item and its `many/` twin: the bound one seeds LANE-WIDE).
        ("get_impact", json!({"id": "LANE-WIDE"})),
        ("get_impact", json!({"symbol": wide_name(7)})),
        // A few lines each: the name of two items, a root's rows, one
        // binding.
        ("find_symbols", json!({"query": wide_name(7), "limit": 200})),
        ("unmapped", json!({})),
        (
            "check_binding",
            json!({"node": "LANE-WIDE", "symbol": format!("pkg/w::bound::{}", wide_name(7))}),
        ),
    ];
    // The first eleven pass the CLI's cut.
    let cut = 11;
    let mut session = Session::open(Era::Legacy, &[], Some(&root), Home::At(&home));
    let schemas = output_schemas(&session.tools());
    let mut largest: Vec<(String, usize)> = Vec::new();
    for (index, (tool, args)) in calls.iter().enumerate() {
        let result = checked_call(&mut session, &schemas, tool, args.clone(), &env, &globals);
        assert_eq!(result["isError"], json!(false), "{tool} {args}");
        let (text, structured, chars, units) = sizes(&result);
        eprintln!(
            "code-tools RD-6 {tool} {args}: text {text}, structuredContent {structured}, \
             sum {chars} chars, {units} UTF-16 units"
        );
        let shown = content_text(&result);
        if index < cut {
            assert!(
                shown.lines().any(|line| line.starts_with("[truncated: ")),
                "{tool} {args}: the corpus does not reach the CLI's cut: {}",
                clip(shown)
            );
        } else {
            assert!(
                !shown.contains("[truncated: "),
                "{tool} {args}: uncut: {}",
                clip(shown)
            );
        }
        if *tool == "check_binding" {
            // The document holds the printed rows, `omitted.rows` the rest.
            let document = &result["structuredContent"];
            assert_eq!(document["verdict"], json!("observed"), "{args}");
            let rows = document["counts"]["rows"].as_u64().expect("counts.rows");
            let printed = document["rows"].as_array().expect("rows").len() as u64;
            let omitted = document["omitted"]["rows"].as_u64().expect("omitted.rows");
            assert_eq!(printed + omitted, rows, "{args}: printed and omitted rows");
            let want = if index < cut { 3_000 } else { 1 };
            assert_eq!(rows, want, "{args}");
            assert_eq!(omitted > 0, index < cut, "{args}: omitted {omitted}");
        }
        if *tool == "get_impact" {
            // docs/features/impact.md "Cut": the printed rows in the JSON,
            // `truncated` the rest, the tail last.
            let document = &result["structuredContent"];
            let printed = document["symbols"].as_array().expect("symbols").len() as u64;
            let hidden = document["truncated"]["symbols"]
                .as_u64()
                .expect("truncated.symbols");
            assert_eq!(printed + hidden, 3_000, "{args}: printed and not shown");
            assert!(hidden > 0, "{args}: cut in the rows");
            assert!(
                shown
                    .lines()
                    .last()
                    .is_some_and(|line| line.starts_with("[truncated: ")),
                "{args}: the tail last"
            );
        }
        if *tool == "get_node" {
            // docs/features/show-bindings.md "spec show --bindings": the
            // printed rows, `omitted` the rest, the tail's count.
            let bindings = &result["structuredContent"]["nodes"][0]["bindings"];
            let printed = bindings["rows"].as_array().expect("rows").len() as u64;
            let omitted = bindings["omitted"].as_u64().expect("omitted");
            assert_eq!(printed + omitted, 3_000, "{args}: printed and omitted rows");
            assert!(printed > 0 && omitted > 0, "{args}: cut in the rows");
            let tail = shown
                .lines()
                .find(|line| line.starts_with("[truncated: "))
                .expect("a tail");
            assert!(
                tail.ends_with(&format!("; bindings not shown: {omitted}]")),
                "{args}: {tail}"
            );
        }
        assert!(
            before_tail(shown).chars().count() <= OUTPUT_CAP_CHARS + 1_000,
            "{tool} {args}: {} characters before the tail",
            before_tail(shown).chars().count()
        );
        assert!(
            chars as u64 <= MAX_RESULT_CHARS && units as u64 <= MAX_RESULT_CHARS,
            "{tool} {args}: text + structuredContent = {chars} characters ({units} UTF-16 \
             units), over MAX_RESULT_CHARS = {MAX_RESULT_CHARS}"
        );
        match largest.iter_mut().find(|(name, _)| name == tool) {
            Some((_, most)) => *most = (*most).max(chars).max(units),
            None => largest.push(((*tool).to_owned(), chars.max(units))),
        }
    }
    // docs/features/impact-since.md TE-5: `get_impact {since: "HEAD"}` with
    // 3 000 untracked items on disk: the delta cut last, the JSON its
    // printed items, `truncated.delta` the rest, the tail last; within
    // `MAX_RESULT_CHARS`, its size printed with the others.
    let mut untracked = String::new();
    for n in 0..3_000 {
        untracked.push_str(&format!(
            "pub fn an_untracked_item_with_a_long_name_to_fill_a_delta_line_{n:05}() {{}}\n"
        ));
    }
    write(&root, "pkg/w/src/since_many.rs", untracked);
    let args = json!({"since": "HEAD"});
    let result = checked_call(
        &mut session,
        &schemas,
        "get_impact",
        args.clone(),
        &env,
        &globals,
    );
    assert_eq!(result["isError"], json!(false), "{args}");
    let (text, structured, chars, units) = sizes(&result);
    eprintln!(
        "impact-since TE-5 get_impact {args}: text {text}, structuredContent {structured}, \
         sum {chars} chars, {units} UTF-16 units"
    );
    let shown = content_text(&result);
    assert!(
        shown
            .lines()
            .last()
            .is_some_and(|line| line.starts_with("[truncated: ")),
        "{args}: the tail last"
    );
    let document = &result["structuredContent"];
    let printed = document["delta"]["added"]
        .as_array()
        .expect("delta.added")
        .len() as u64;
    let hidden = document["truncated"]["delta"]
        .as_u64()
        .expect("truncated.delta");
    assert_eq!(printed + hidden, 3_000, "{args}: printed and not shown");
    assert!(printed > 0 && hidden > 0, "{args}: cut in the delta");
    assert!(
        before_tail(shown).chars().count() <= OUTPUT_CAP_CHARS + 1_000,
        "{args}: {} characters before the tail",
        before_tail(shown).chars().count()
    );
    assert!(
        chars as u64 <= MAX_RESULT_CHARS && units as u64 <= MAX_RESULT_CHARS,
        "{args}: {chars} characters ({units} UTF-16 units) over MAX_RESULT_CHARS"
    );
    eprintln!(
        "impact-since TE-5 get_impact since maximum: {}",
        chars.max(units)
    );
    let done = session.finish();
    assert!(done.status.success(), "{}", done.stderr);
    assert_eq!(done.stderr, "", "stderr");
    for (tool, most) in &largest {
        eprintln!("code-tools RD-6 {tool} maximum: {most}");
    }
}

/// docs/features/bevy-roles-reads.md TE-8, the recorded size: `find_symbols`
/// over a scratch `fixtures/code-bevy` (the opt-in on): QUERY, `--node`,
/// `--at` and `--file` of every code file, each answer the CLI library's
/// and its schema's, within `OUTPUT_CAP_CHARS` before any tail and
/// `MAX_RESULT_CHARS` in all (characters and UTF-16 units); the maximum is
/// printed for the spec's "Implementation".
#[test]
fn find_symbols_over_code_bevy_records_its_maxima() {
    let scratch = Scratch::new("size-bevy");
    let home = scratch.home("h");
    let root = scratch.copy("code-bevy", "code-bevy");
    let mut calls: Vec<(&str, Value)> = vec![
        ("find_symbols", json!({"query": "tick"})),
        ("find_symbols", json!({"query": "run"})),
        ("find_symbols", json!({"query": "graze"})),
        ("find_symbols", json!({"node": "ALM-GROWTH"})),
        ("find_symbols", json!({"node": "CHORE-01"})),
        ("find_symbols", json!({"at": "crates/farm/src/lib.rs:30"})),
    ];
    for path in [
        "assets/farm.ron",
        "crates/farm-app/src/main.rs",
        "crates/farm/src/clock.rs",
        "crates/farm/src/crops.rs",
        "crates/farm/src/fields.rs",
        "crates/farm/src/herd.rs",
        "crates/farm/src/lib.rs",
        "crates/farm/src/news.rs",
        "crates/farm/src/weather.rs",
    ] {
        calls.push(("find_symbols", json!({"file": path})));
    }
    let largest = measure("code-bevy", &root, &home, &calls);
    eprintln!("bevy-roles-reads TE-8 find_symbols maximum on code-bevy: {largest}");
    assert!(largest > 0, "measured");
}

/// docs/features/bundle-bindings.md TE-5 (`mcp_size` maxima with rows):
/// [`wide_code`] committed, plus `pkg/w/src/a_long.rs` with 64 items bound
/// to `LANE-WIDE` whose names are 9 000 characters (their rows sort first)
/// and a draft task on `LANE-WIDE`: `get_context_bundle` at u32::MAX and at
/// the default budget, and `get_task`, each the CLI library's (parity and
/// the output schema), within `OUTPUT_CAP_CHARS` before the tail (a
/// bundle: its body) and `MAX_RESULT_CHARS` in all (characters and UTF-16
/// units); the bundle places rows and leaves the rest out, the package
/// keeps fewer than 64 rows (the budget) and names the rest in one note.
/// The maxima are printed.
#[test]
fn bundle_and_task_with_rows_stay_under_the_declared_cap() {
    let scratch = Scratch::new("size-rows");
    let home = scratch.home("h");
    let root = scratch.dir("wide");
    wide_code(&root);
    let lib = read_text(&root, "pkg/w/src/lib.rs");
    write(&root, "pkg/w/src/lib.rs", format!("pub mod a_long;\n{lib}"));
    let mut long = String::new();
    for n in 0..64 {
        let stem = format!("long_{n:02}_");
        long.push_str(&format!(
            "// @implements LANE-WIDE@1\npub fn {stem}{}() -> u32 {{\n    {n}\n}}\n",
            "l".repeat(9_000 - stem.len())
        ));
    }
    write(&root, "pkg/w/src/a_long.rs", long);
    let sandbox = git::Sandbox::new(scratch.path());
    sandbox.init(&root);
    sandbox.add_all(&root);
    sandbox.commit(&root, "the wide corpus");
    let env = cli_env(&root, Some(&home));
    let made = specengine_cli::task_new(
        &env,
        &Globals::default(),
        &specengine_cli::TaskNewRequest {
            nodes: vec!["LANE-WIDE".to_owned()],
            title: Some("Widen the lane".to_owned()),
            goal: Some("Read the code bound to the lane.".to_owned()),
            author_role: None,
            author_model: None,
            run: None,
            now: "2026-10-21T09:00:00Z".to_owned(),
            git: specengine_cli::GitEnv::new(&root, sandbox.vars()),
        },
    )
    .expect("task new");
    let id = made.id.expect("an ID");
    let largest = measure(
        "bundle-bindings TE-5",
        &root,
        &home,
        &[
            (
                "get_context_bundle",
                json!({"node_ids": ["LANE-WIDE"], "budget": 4294967295u64}),
            ),
            ("get_context_bundle", json!({"node_ids": ["LANE-WIDE"]})),
            ("get_task", json!({"task_id": id})),
        ],
    );
    eprintln!("bundle-bindings TE-5 maximum: {largest}");
    let globals = Globals::default();
    let mut session = Session::open(Era::Legacy, &[], Some(&root), Home::At(&home));
    let schemas = output_schemas(&session.tools());
    let bundle = checked_call(
        &mut session,
        &schemas,
        "get_context_bundle",
        json!({"node_ids": ["LANE-WIDE"], "budget": 4294967295u64}),
        &env,
        &globals,
    );
    let document = &bundle["structuredContent"];
    let placed = document["layers"]["bindings"]
        .as_array()
        .expect("rows")
        .len();
    let left = document["tail"].as_array().expect("tail").len()
        + document["more"].as_u64().expect("more") as usize;
    assert!(
        placed > 0 && left > 0,
        "rows past the ceiling: {placed} placed, {left} left"
    );
    assert_eq!(placed + left, 3_064, "each row once");
    let task = session.call("get_task", json!({"task_id": id}));
    let package = &result(&task)["structuredContent"];
    let rows = package["bindings"].as_array().expect("bindings").len();
    assert!(rows > 0 && rows < 64, "the budget keeps {rows} long rows");
    let notes: Vec<&str> = package["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .filter_map(Value::as_str)
        .filter(|note| note.starts_with("bindings: "))
        .collect();
    assert_eq!(
        notes,
        [format!(
            "bindings: {} more row(s) left out; spec show LANE-WIDE --bindings lists them",
            3_064 - rows
        )
        .as_str()],
        "one note for the cap and the budget"
    );
    let done = session.finish();
    assert!(done.status.success(), "{}", done.stderr);
}

// ------------------------------------------------- card-context (RM-0108)

/// docs/features/card-context.md CC11: the largest `get_card {context:
/// true, history: true}` answer measured, text plus `structuredContent`, in
/// characters (UTF-16 units alike: the corpus is ASCII). 424 208 until
/// docs/features/upstream-signal.md US-10 made CC3 (a)'s card a subtask
/// whose `upstream` lists 16 edits by a 128-byte name (and its context
/// the parent's 4 096-byte goal); 432 982 until
/// docs/features/plain-questions.md PQ10 gave each of the answer's 64 items
/// `,"gist":null,"matters":null` (27 characters each, + 1 728).
const CARD_CONTEXT_MAXIMUM: usize = 434_710;

/// The clock of every write of the card scenes.
const CARD_CLOCK: &str = "2026-11-12T09:00:00Z";

/// About `chars` characters of prose marked `mark`, no two lines alike.
fn watch_prose(mark: &str, chars: usize) -> String {
    let mut out = String::new();
    let mut n = 0;
    while out.len() < chars {
        n += 1;
        out.push_str(&format!(
            "The {mark} keeper trims wick number {n} before the tide turns, and the lamp burns \
             steady through watch {n}.\n"
        ));
        if n % 6 == 0 {
            out.push('\n');
        }
    }
    out
}

/// A distinct ASCII text of exactly `bytes` bytes for item `n`: words drawn
/// from a small vocabulary by `n`, so no two items read alike.
fn distinct_text(n: usize, bytes: usize, end: &str) -> String {
    const WORDS: [&str; 16] = [
        "amber", "beacon", "cinder", "drift", "ember", "fathom", "gale", "harbour", "inlet",
        "jetty", "keel", "lantern", "mooring", "nimbus", "oar", "pennant",
    ];
    let mut seed = (n as u64)
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    let mut out = format!("Item {n}:");
    while out.len() + end.len() + 8 < bytes {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        out.push(' ');
        out.push_str(WORDS[(seed >> 60) as usize]);
    }
    while out.len() + end.len() < bytes {
        out.push('.');
    }
    out.push_str(end);
    out
}

/// `n` ASCII criteria of exactly `bytes` bytes each.
fn long_criteria(n: usize, bytes: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            let mut line = format!("Criterion {i}: the lamp stays lit");
            while line.len() < bytes {
                line.push_str(" and steady");
            }
            line.truncate(bytes);
            line
        })
        .collect()
}

/// A card through the library at [`CARD_CLOCK`]: its ID.
fn card_at(
    env: &specengine_cli::Env,
    title: &str,
    goal: &str,
    targets: &[String],
    criteria: Vec<String>,
    parent: Option<&str>,
) -> String {
    let made = specengine_cli::card_new(
        env,
        &Globals::default(),
        &specengine_cli::CardNewRequest {
            title: title.to_owned(),
            goal: Some(goal.to_owned()),
            discipline: None,
            targets: targets.to_vec(),
            criteria,
            assignee: None,
            reviewer: parent.map(|_| "Ana Ruiz".to_owned()),
            mandate: None,
            estimate: None,
            due: None,
            parent: parent.map(str::to_owned),
            corrects: None,
            actor: None,
            now: CARD_CLOCK.to_owned(),
        },
    )
    .unwrap_or_else(|error| panic!("card new {title}: {error}"));
    made.id
        .clone()
        .unwrap_or_else(|| panic!("card new {title}: {made:?}"))
}

/// CC11: the two cards of "Acceptance criteria" read with `context: true,
/// history: true`, each within the cut and `MAX_RESULT_CHARS`; their
/// maximum is [`CARD_CONTEXT_MAXIMUM`]. M: the declared value below the
/// measurement.
#[test]
fn card_context_stays_under_the_declared_cap() {
    let scratch = Scratch::new("size-card-context");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "a");
    for name in ["LONG-A", "LONG-B", "LONG-C"] {
        write(
            &root,
            &format!("docs/spec/long/{}.md", name.to_lowercase()),
            format!(
                "---\nid: MEC-{name}\nkind: mechanic\nclass: canon\ntier: 2\ntitle: Long \
                 {name}\nparent: DOM-MOVEMENT\nstatus: accepted\nowner: owner\nreviewed: \
                 2026-09-20\n---\n\n# Long {name}\n\n{}",
                watch_prose(name, 16_000)
            ),
        );
    }
    let mut fill = "---\nid: MEC-FILL\nkind: mechanic\nclass: canon\ntier: 2\ntitle: Fill\n\
                    parent: DOM-MOVEMENT\nstatus: accepted\nowner: owner\nreviewed: 2026-09-20\n\
                    ---\n\n# Fill\n\nThe lamp room's rules.\n"
        .to_owned();
    let mut rules = Vec::new();
    for n in 1..=64 {
        fill.push_str(&format!(
            "\n## Fill {n} {{#RULE-FILL-{n:02}}}\n\nWick {n} is trimmed at dusk.\n"
        ));
        rules.push(format!("RULE-FILL-{n:02}"));
    }
    write(&root, "docs/spec/fill.md", fill);
    let sandbox = git::Sandbox::new(scratch.path());
    sandbox.init(&root);
    sandbox.add_all(&root);
    sandbox.commit(&root, "spec-a with long documents");
    let env = cli_env(&root, Some(&home));
    let git = specengine_cli::GitEnv::new(&root, sandbox.vars());

    // CC3 (b): the subtask with 64 targets under a parent with a 4 096 B goal.
    let parent = card_at(
        &env,
        "The lamp room",
        &"g".repeat(4096),
        &["MEC-SPRINT".to_owned()],
        Vec::new(),
        None,
    );
    let subtask = card_at(
        &env,
        "Every wick",
        &"w".repeat(4096),
        &rules,
        long_criteria(32, 1024),
        Some(&parent),
    );
    // CC3 (a) with 64 items.
    let long: Vec<String> = ["MEC-LONG-A", "MEC-LONG-B", "MEC-LONG-C"]
        .iter()
        .map(|id| (*id).to_owned())
        .collect();
    // docs/features/upstream-signal.md US-10: the long watch is a subtask of
    // the lamp room, so what changes above it is listed.
    let watch = card_at(
        &env,
        "The long watch",
        "Every keeper reads the three watches.",
        &long,
        long_criteria(25, 1000),
        Some(&parent),
    );
    for n in 0..64 {
        let mut request = specengine_cli::DecisionRequest {
            input: specengine_cli::DecisionInput {
                node_ids: vec!["MEC-LONG-A".to_owned()],
                text: distinct_text(n, 1024, "?"),
                working_answer: distinct_text(n + 1000, 2048, "."),
                price_of_other: "A watch rewritten.".to_owned(),
                why: Some(distinct_text(n + 2000, 2048, ".")),
                severity: None,
                proposed_patch: None,
                distinct_from: None,
            },
            card: Some(watch.clone()),
            author_role: Some("developer".to_owned()),
            author_model: Some("claude-opus-5-5".to_owned()),
            run: None,
            login: None,
            now: CARD_CLOCK.to_owned(),
            git: git.clone(),
        };
        let raise = |request: &specengine_cli::DecisionRequest| {
            specengine_cli::propose_decision(&env, &Globals::default(), request)
                .unwrap_or_else(|error| panic!("decision {n}: {error}"))
        };
        let first = raise(&request);
        let done = if first.document.created {
            first
        } else {
            let names: Vec<String> = first
                .document
                .hits
                .iter()
                .map(|hit| hit.name().to_owned())
                .collect();
            request.input.distinct_from = Some(names);
            raise(&request)
        };
        assert!(done.document.created, "decision {n}: {done:?}");
    }
    // docs/features/upstream-signal.md US-10: twenty edits of the parent's
    // goal, targets and criteria as the longest name a card takes (128
    // bytes): both subtasks list the newest 16, `more` 4.
    let editor = format!("Keeper {}", "w".repeat(121));
    assert_eq!(editor.len(), 128);
    for n in 0..20 {
        let goal = if n % 2 == 0 { "h" } else { "g" };
        let edited = specengine_cli::card_edit(
            &env,
            &Globals::default(),
            &specengine_cli::CardEditRequest {
                id: parent.clone(),
                title: None,
                goal: Some(goal.repeat(4096)),
                targets: if n % 2 == 0 {
                    vec!["MEC-SPRINT".to_owned(), "MEC-LONG-A".to_owned()]
                } else {
                    vec!["MEC-SPRINT".to_owned()]
                },
                criteria: vec![format!("The lamp is lit by watch {n}.")],
                estimate: None,
                due: None,
                parent: None,
                corrects: None,
                evidence: Vec::new(),
                tasks: Vec::new(),
                mandate: None,
                reason: None,
                revision: None,
                actor: Some(editor.clone()),
                now: CARD_CLOCK.to_owned(),
                tasks_here: true,
            },
        )
        .unwrap_or_else(|error| panic!("edit {n}: {error}"));
        assert!(edited.id.is_some(), "edit {n}: {edited:?}");
    }
    let largest = measure(
        "card-context CC11",
        &root,
        &home,
        &[
            (
                "get_card",
                json!({"card_id": subtask, "context": true, "history": true}),
            ),
            (
                "get_card",
                json!({"card_id": watch, "context": true, "history": true}),
            ),
        ],
    );
    eprintln!("card-context CC11 get_card maximum: {largest}");
    assert!(largest as u64 <= MAX_RESULT_CHARS);
    // docs/features/upstream-signal.md US-10: within 500 000, each card's
    // `upstream` the 16 newest of the twenty edits, `more` 4.
    assert!(largest <= 500_000, "{largest}");
    for id in [&subtask, &watch] {
        let shown = specengine_cli::card_show(
            &env,
            &Globals::default(),
            &specengine_cli::CardShowRequest {
                id: id.clone(),
                history: false,
                marks: true,
                upstream: true,
                context: false,
                budget: None,
                git: git.clone(),
                tasks_here: true,
            },
        )
        .expect("card show");
        let upstream = shown
            .upstream
            .unwrap_or_else(|| panic!("{id}: an upstream"));
        assert_eq!((upstream.changes.len(), upstream.more), (16, 4), "{id}");
        assert!(
            upstream
                .changes
                .iter()
                .all(|change| change.by.as_deref() == Some(editor.as_str())
                    && change.fields == ["goal", "targets", "criteria"]),
            "{id}: {upstream:?}"
        );
    }
    assert_eq!(
        largest, CARD_CONTEXT_MAXIMUM,
        "the pinned maximum (\"Implementation\")"
    );
}

// ------------------------------------------------- node-history (RM-0035)

/// docs/features/node-history.md NH14: the largest `get_node` answer with
/// all three `with` values on NH10's nodes, text plus `structuredContent`,
/// in characters (UTF-16 units alike: the corpus is ASCII).
const NODE_HISTORY_MAXIMUM: usize = 110_216;

/// 2026-01-01T00:00:00Z.
const EPOCH_2026: i64 = 1_767_225_600;

/// A fast-import stream: R-12's text `body` created by hand, then 63 apply
/// commits of a 300-character rationale each, decided by `decider`, each
/// changing R-12's last line.
fn history_stream(body: &str, decider: &str) -> Vec<u8> {
    let config = "[project]\nslug = \"quill-rules\"\n\n[paths]\nroots = [\"docs\"]\n\n[ids]\n\
                  R = { kind = \"requirement\", width = 2 }\n";
    let rules = |n: usize| {
        format!(
            "# Rules\n\nThe house rules.\n\n## Window {{#R-12}}\n\n{body}\nEdit {n}.\n\n\
             ## Door {{#R-13}}\n\nThe door shuts at dusk.\n"
        )
    };
    let mut out = Vec::new();
    for n in 0..64_usize {
        let message = if n == 0 {
            "Add the rules\n".to_owned()
        } else {
            let mut rationale = format!("Step {n:02}:");
            while rationale.len() < 300 {
                rationale.push_str(" tighten");
            }
            rationale.truncate(300);
            format!(
                "spec: apply PR-{n:04}\n\n{rationale}\n\nProposal: PR-{n:04}\nDecided-by: \
                 {decider}\nProposed-by: agent role=spec-writer model=m run=r\nBase-commit: \
                 {}\n",
                "0".repeat(40)
            )
        };
        let time = EPOCH_2026 + 60 * i64::try_from(n).expect("small");
        out.extend_from_slice(
            format!(
                "commit refs/heads/main\nmark :{}\nauthor Bob Hand <bob@example.org> {time} \
                 +0000\ncommitter Dana Desk <dana@example.org> {time} +0000\ndata {}\n{message}\n",
                n + 1,
                message.len()
            )
            .as_bytes(),
        );
        if n > 0 {
            out.extend_from_slice(format!("from :{n}\n").as_bytes());
        }
        let mut files = vec![("docs/rules.md".to_owned(), rules(n))];
        if n == 0 {
            files.push(("specengine.toml".to_owned(), config.to_owned()));
        }
        for (path, content) in files {
            out.extend_from_slice(
                format!(
                    "M 100644 inline {path}\ndata {}\n{content}\n",
                    content.len()
                )
                .as_bytes(),
            );
        }
        out.push(b'\n');
    }
    out
}

/// NH14: NH10's two nodes read with `links`, `bindings` and `history`,
/// each within the cut and `MAX_RESULT_CHARS`; their maximum is
/// [`NODE_HISTORY_MAXIMUM`]. M: the declared value below the measurement.
#[test]
fn node_history_stays_under_the_declared_cap() {
    let scratch = Scratch::new("size-node-history");
    let home = scratch.home("h");
    let sandbox = git::Sandbox::new(scratch.path());
    let mut body = String::new();
    let mut n = 0;
    while body.len() < 20_000 {
        n += 1;
        body.push_str(&format!(
            "Line {n:05} of the window's long text, kept as written.\n"
        ));
    }
    let decider = format!(
        "{} <ann@example.org>",
        "Ann Owner of the Long Name".repeat(18)
    );
    let mut largest = 0;
    for (dir, decided_by) in [
        ("text-cut", "Ann Owner <ann@example.org>"),
        ("history-cut", decider.as_str()),
    ] {
        let root = scratch.dir(dir);
        sandbox.init(&root);
        sandbox.git_stdin(
            &root,
            &["fast-import", "--quiet"],
            &history_stream(body.trim_end(), decided_by),
        );
        sandbox.git(&root, &["reset", "-q", "--hard", "main"]);
        let size = measure(
            &format!("node-history NH14 {dir}"),
            &root,
            &home.join(dir),
            &[(
                "get_node",
                json!({"id": "R-12", "with": ["links", "bindings", "history"]}),
            )],
        );
        largest = largest.max(size);
    }
    eprintln!("node-history NH14 get_node maximum: {largest}");
    assert!(largest as u64 <= MAX_RESULT_CHARS);
    assert_eq!(
        largest, NODE_HISTORY_MAXIMUM,
        "the pinned maximum (\"Implementation\")"
    );
}
