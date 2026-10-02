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
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

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
