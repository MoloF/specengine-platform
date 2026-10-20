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
    // 3 000 open questions on MEC-STAMINA with titles of control
    // characters: the links block and the bundle at the ceiling.
    let root = scratch.copy("spec-a", "questions");
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
                 refs: [MEC-STAMINA]\nowner: owner\nreviewed: 2026-09-20\n---\n\n# {}{n}\n\nx\n",
                c.repeat(3)
            ),
        );
    }
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
        // A few lines each: the name of two items, a root's rows, one
        // binding.
        ("find_symbols", json!({"query": wide_name(7), "limit": 200})),
        ("unmapped", json!({})),
        (
            "check_binding",
            json!({"node": "LANE-WIDE", "symbol": format!("pkg/w::bound::{}", wide_name(7))}),
        ),
    ];
    // The first seven pass the CLI's cut.
    let cut = 7;
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
