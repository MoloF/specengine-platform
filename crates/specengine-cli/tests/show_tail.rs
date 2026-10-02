//! AC-22 of docs/features/mcp-read.md ("Data", CLI `show` cut): the tail of
//! a cut `spec show` names at most `SHOW_TAIL_NAMES` = 20 hidden sections
//! (source order) and 20 holders not shown (print order); a longer list
//! ends `, <k> more`, `k` the rest. JSON, cut node only: `sections` holds
//! the IDs whose heading line ends within its `text`; `omitted` is exactly
//! `{lines, sections, sections_more, holders, holders_more}`, the names by
//! the same rule at the JSON's cut, `*_more` the `k` (0 if none). A list of
//! at most 20 names keeps its bytes (no `, 0 more`).
//!
//! The JSON's cut counts the `text` values only, the text's also the header
//! line, so the two may hide different sections: spec-a's glossary (one
//! body line per section) is cut one heading later in JSON than in text,
//! spec-b's (two body lines) at the same heading, where the JSON names
//! exactly the tail's 20 and its `k`.
//!
//! M: the tail listing every section; `sections` uncut in JSON;
//! `sections_more` off by one.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;

use common::{Run, Scratch, index, spec, write};
use serde_json::{Value, json};
use specengine_cli::{OUTPUT_CAP_CHARS, SHOW_TAIL_NAMES};

const CAP: usize = 40_000;
const NAMES: usize = 20;

#[test]
fn the_tail_names_constant_is_20() {
    assert_eq!(SHOW_TAIL_NAMES, NAMES);
    assert_eq!(OUTPUT_CAP_CHARS, CAP);
}

/// A glossary of `count` sections `## … {#<prefix>-glossary-entry-NNNN}`,
/// each followed by `body` lines of `line`.
fn glossary(prefix: &str, count: usize, line: &str, body: usize) -> String {
    let mut text = String::from("---\nclass: canon\n---\n\n# Glossary\n\n");
    for n in 0..count {
        text.push_str(&format!(
            "## The meaning of glossary entry number {n} in plain words {{#{prefix}-glossary-entry-{n:04}}}\n\n"
        ));
        for k in 0..body {
            text.push_str(&format!("{line} {n}.{k}\n"));
        }
        text.push('\n');
    }
    text
}

/// The 1-based line and ID of each `{#ID}` heading of `text`.
fn heading_lines(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let start = line.find("{#")?;
            let id = line[start + 2..].strip_suffix('}')?;
            Some((index + 1, id.to_owned()))
        })
        .collect()
}

/// The IDs of `text`'s headings before line `first_hidden` and from it.
fn split_sections(text: &str, first_hidden: usize) -> (Vec<String>, Vec<String>) {
    let mut shown = Vec::new();
    let mut hidden = Vec::new();
    for (line, id) in heading_lines(text) {
        if line < first_hidden {
            shown.push(id);
        } else {
            hidden.push(id);
        }
    }
    (shown, hidden)
}

/// A tail list as Data writes it: `none`, the names, or the first 20 then
/// `, <k> more`.
fn tail_list(names: &[String]) -> String {
    if names.is_empty() {
        "none".to_owned()
    } else if names.len() <= NAMES {
        names.join(", ")
    } else {
        format!(
            "{}, {} more",
            names[..NAMES].join(", "),
            names.len() - NAMES
        )
    }
}

/// `(everything before the tail, the tail)`, asserting one tail line, last.
fn split_tail(stdout: &str) -> (&str, &str) {
    let body = stdout.strip_suffix('\n').expect("a line end");
    let (before, tail) = body.rsplit_once('\n').expect("a tail line");
    assert!(tail.starts_with("[truncated: "), "{tail}");
    assert_eq!(stdout.matches("[truncated: ").count(), 1, "one tail line");
    (&stdout[..before.len() + 1], tail)
}

/// The key set of a JSON object.
fn keys(value: &Value) -> BTreeSet<&str> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("not an object: {value}"))
        .keys()
        .map(String::as_str)
        .collect()
}

/// Asserts the cut node's `omitted` has exactly the five keys of Data.
fn assert_omitted_keys(node: &Value, context: &str) {
    assert_eq!(
        keys(&node["omitted"]),
        BTreeSet::from([
            "lines",
            "sections",
            "sections_more",
            "holders",
            "holders_more"
        ]),
        "{context}: the omitted key set"
    );
}

/// The `(first, more)` split of `names` the JSON gives.
fn first_and_more(names: &[String]) -> (Value, usize) {
    let first = &names[..names.len().min(NAMES)];
    (json!(first), names.len() - first.len())
}

/// The text shown of the one node `stdout` prints (header line dropped)
/// and its tail.
fn shown_text(run: &Run) -> (String, String) {
    let (before, tail) = split_tail(&run.stdout);
    let chars = before.chars().count();
    assert!(chars <= CAP, "{chars} characters before the tail");
    let (_header, shown) = before.split_once('\n').expect("a header line");
    (shown.to_owned(), tail.to_owned())
}

#[test]
fn ac22_eight_thousand_sections_name_twenty_then_count_the_rest() {
    let cyrillic = "\u{0421}\u{043b}\u{043e}\u{0432}\u{043e} \u{0433}\u{043b}\u{043e}\u{0441}\u{0441}\u{0430}\u{0440}\u{0438}\u{044f}";
    for (fixture, prefix, line, body, same_cut) in [
        ("spec-a", "TERM", "Entry explained", 1, false),
        ("spec-b", "GLS", cyrillic, 2, true),
    ] {
        let scratch = Scratch::new("show-tail");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let path = "docs/spec/glossary.md";
        let text = glossary(prefix, 8_000, line, body);
        write(&root, path, &text);
        index(&home, &root);
        let last = text.lines().count();

        // Text: the cut, then a tail of 20 names and the exact rest.
        let run = spec(&home, &root, &["show", path]);
        run.code(0);
        let (shown, tail) = shown_text(&run);
        assert!(
            text.starts_with(&shown) && shown.ends_with('\n'),
            "{fixture}"
        );
        let first_hidden = shown.matches('\n').count() + 1;
        let (printed, hidden) = split_sections(&text, first_hidden);
        assert_eq!(printed.len() + hidden.len(), 8_000, "{fixture}");
        assert!(hidden.len() > 7_000, "{fixture}: {} hidden", hidden.len());
        assert_eq!(
            tail,
            format!(
                "[truncated: {path} lines {first_hidden}-{last} not shown; sections not shown: {}; holders not shown: none]",
                tail_list(&hidden)
            ),
            "{fixture}"
        );
        assert!(
            tail.ends_with(&format!(
                ", {} more; holders not shown: none]",
                hidden.len() - NAMES
            )),
            "{fixture}: k exact"
        );
        let named = tail.matches(&format!("{prefix}-glossary-entry-")).count();
        assert_eq!(named, NAMES, "{fixture}: the tail names 20 sections");
        eprintln!(
            "AC-22 {fixture}: {} characters before the tail, tail {} characters, {} printed, {} hidden",
            run.stdout.chars().count() - tail.chars().count() - 1,
            tail.chars().count(),
            printed.len(),
            hidden.len()
        );

        // JSON: the cut node's sections are the headings its text holds;
        // `omitted` names the first 20 hidden at the JSON's cut and counts
        // the rest.
        let json_run = spec(&home, &root, &["--json", "show", path]);
        json_run.code(0);
        let json = json_run.json();
        let nodes = json["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1, "{fixture}");
        let node = &nodes[0];
        assert_eq!(node["truncated"], true, "{fixture}");
        let json_text = node["text"].as_str().unwrap();
        assert!(json_text.chars().count() <= CAP, "{fixture}");
        assert!(
            text.starts_with(json_text) && json_text.ends_with('\n'),
            "{fixture}"
        );
        let json_first_hidden = json_text.matches('\n').count() + 1;
        let (json_printed, json_hidden) = split_sections(&text, json_first_hidden);
        assert_eq!(
            node["sections"],
            json!(json_printed),
            "{fixture}: JSON sections are the headings its text holds"
        );
        assert_omitted_keys(node, fixture);
        let (first, more) = first_and_more(&json_hidden);
        assert_eq!(node["omitted"]["sections"], first, "{fixture}");
        assert_eq!(node["omitted"]["sections_more"], json!(more), "{fixture}");
        assert_eq!(
            node["omitted"]["lines"],
            json!([json_first_hidden, last]),
            "{fixture}"
        );
        assert_eq!(node["omitted"]["holders"], json!([]), "{fixture}");
        assert_eq!(node["omitted"]["holders_more"], json!(0), "{fixture}");
        if same_cut {
            // The cuts hide the same sections: the JSON names exactly the
            // tail's 20 and its `k`; `sections` are the printed headings.
            assert_eq!(json_hidden, hidden, "{fixture}: the same cut heading");
            assert_eq!(node["sections"], json!(printed), "{fixture}");
            assert_eq!(
                node["omitted"]["sections"],
                json!(hidden[..NAMES]),
                "{fixture}"
            );
            assert_eq!(
                node["omitted"]["sections_more"],
                json!(hidden.len() - NAMES),
                "{fixture}"
            );
        } else {
            assert_ne!(json_hidden, hidden, "{fixture}: the cuts differ here");
        }
        eprintln!(
            "AC-22 {fixture} JSON: {} characters, sections {}, sections_more {more}",
            json_run.stdout.chars().count(),
            json_printed.len()
        );

        // Byte-identical reruns.
        assert_eq!(spec(&home, &root, &["show", path]).stdout, run.stdout);
        assert_eq!(
            spec(&home, &root, &["--json", "show", path]).stdout,
            json_run.stdout
        );

        // With `--links` JSON takes the text's cut: `sections` are exactly
        // the printed headings, `omitted.sections` the tail's 20 and
        // `sections_more` its `k`.
        let linked = spec(&home, &root, &["show", path, "--links"]);
        linked.code(0);
        let (before, tail) = split_tail(&linked.stdout);
        assert!(before.chars().count() <= CAP, "{fixture} --links");
        let block = "  links 0 out, 0 in\n";
        let (_header, rest) = before.split_once('\n').expect("a header line");
        let shown = rest.strip_prefix(block).unwrap_or_else(|| {
            panic!(
                "{fixture}: the links block: {}",
                rest.chars().take(80).collect::<String>()
            )
        });
        assert!(
            text.starts_with(shown) && shown.ends_with('\n'),
            "{fixture}"
        );
        let first_hidden = shown.matches('\n').count() + 1;
        let (printed, hidden) = split_sections(&text, first_hidden);
        assert_eq!(
            tail,
            format!(
                "[truncated: {path} lines {first_hidden}-{last} not shown; sections not shown: {}; holders not shown: none; links not shown: 0]",
                tail_list(&hidden)
            ),
            "{fixture} --links"
        );
        let json = spec(&home, &root, &["--json", "show", path, "--links"]).json();
        let node = &json["nodes"][0];
        assert_eq!(
            node["text"],
            json!(shown),
            "{fixture}: JSON text as printed"
        );
        assert_eq!(node["sections"], json!(printed), "{fixture} --links");
        assert_omitted_keys(node, fixture);
        assert_eq!(
            node["omitted"]["sections"],
            json!(hidden[..NAMES]),
            "{fixture} --links: the tail's 20"
        );
        assert_eq!(
            node["omitted"]["sections_more"],
            json!(hidden.len() - NAMES),
            "{fixture} --links: the tail's k"
        );
    }
}

/// A document whose preamble (no sections) passes the cap, then `count`
/// sections `RULE-EDGE<count>-<n>`: every section is hidden.
fn preamble_then_sections(count: usize) -> String {
    let mut text = String::from("---\nclass: canon\n---\n\n# Preamble\n\n");
    for k in 0..900 {
        text.push_str(&format!(
            "A preamble line of filler text, long enough to pass the cap, {k}.\n"
        ));
    }
    for n in 1..=count {
        text.push_str(&format!(
            "\n## Rule {n} {{#RULE-EDGE{count}-{n}}}\n\nRule {n}.\n"
        ));
    }
    text
}

#[test]
fn ac22_twenty_hidden_sections_are_named_whole_twenty_one_end_one_more() {
    let scratch = Scratch::new("show-tail-edge");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    for count in [19, 20, 21, 22] {
        let path = format!("docs/spec/edge-{count}.md");
        write(&root, &path, preamble_then_sections(count));
    }
    index(&home, &root);
    for count in [19, 20, 21, 22] {
        let path = format!("docs/spec/edge-{count}.md");
        let text = preamble_then_sections(count);
        let ids: Vec<String> = (1..=count)
            .map(|n| format!("RULE-EDGE{count}-{n}"))
            .collect();
        let run = spec(&home, &root, &["show", &path]);
        run.code(0);
        let (shown, tail) = shown_text(&run);
        let first_hidden = shown.matches('\n').count() + 1;
        let (printed, hidden) = split_sections(&text, first_hidden);
        assert!(printed.is_empty(), "{count}: the cut is in the preamble");
        assert_eq!(hidden, ids, "{count}");
        let list = if count <= NAMES {
            ids.join(", ")
        } else {
            format!("{}, {} more", ids[..NAMES].join(", "), count - NAMES)
        };
        assert_eq!(
            tail,
            format!(
                "[truncated: {path} lines {first_hidden}-{} not shown; sections not shown: {list}; holders not shown: none]",
                text.lines().count()
            ),
            "{count}"
        );
        assert!(!tail.contains(" 0 more"), "{count}: no `, 0 more`");

        let json = spec(&home, &root, &["--json", "show", &path]).json();
        let node = &json["nodes"][0];
        assert_eq!(node["truncated"], true, "{count}");
        assert_eq!(node["sections"], json!([]), "{count}: no heading shown");
        assert_omitted_keys(node, &path);
        assert_eq!(
            node["omitted"]["sections"],
            json!(ids[..count.min(NAMES)]),
            "{count}"
        );
        assert_eq!(
            node["omitted"]["sections_more"],
            json!(count.saturating_sub(NAMES)),
            "{count}"
        );
        assert_eq!(node["omitted"]["holders_more"], json!(0), "{count}");
    }
}

/// `aa-big.md` holds `AC-50` far over the cap, then `holders` small
/// feature specs `h-NN.md` hold `AC-50` too, each mentioning `R-12`.
fn many_holders(root: &std::path::Path, holders: usize) -> Vec<String> {
    let mut big =
        String::from("---\nclass: spec\nstatus: draft\n---\n\n# Aa big\n\n### Big {#AC-50}\n\n");
    for k in 0..2000 {
        big.push_str(&format!(
            "A line of filler text for the big criterion, number {k}.\n"
        ));
    }
    write(root, "docs/features/aa-big.md", &big);
    (1..=holders)
        .map(|n| {
            write(
                root,
                &format!("docs/features/h-{n:02}.md"),
                format!(
                    "---\nclass: spec\nstatus: draft\n---\n\n# Holder {n}\n\n### Small {{#AC-50}}\n\nShort, see R-12.\n"
                ),
            );
            format!("docs/features/h-{n:02}.md:8")
        })
        .collect()
}

#[test]
fn ac22_more_than_twenty_holders_past_the_cut_name_twenty_then_count_the_rest() {
    for holders in [20, 21, 29] {
        let scratch = Scratch::new("show-tail-holders");
        let home = scratch.home("h");
        let root = scratch.copy("spec-a", "copy");
        let names = many_holders(&root, holders);
        index(&home, &root);
        let list = tail_list(&names);
        for links in [false, true] {
            let context = format!("{holders} holders, --links {links}");
            let mut args = vec!["show", "AC-50"];
            if links {
                args.push("--links");
            }
            let run = spec(&home, &root, &args);
            run.code(0);
            let (before, tail) = split_tail(&run.stdout);
            assert!(before.chars().count() <= CAP, "{context}");
            assert!(
                !before.contains("docs/features/h-"),
                "{context}: no small holder printed"
            );
            let links_part = if links {
                // Every link of the dropped holders: one mention each.
                format!("; links not shown: {holders}")
            } else {
                String::new()
            };
            assert!(
                tail.starts_with("[truncated: docs/features/aa-big.md lines ")
                    && tail.ends_with(&format!(
                        "; sections not shown: none; holders not shown: {list}{links_part}]"
                    )),
                "{context}: {tail}"
            );
            if holders > NAMES {
                assert!(
                    tail.contains(&format!(
                        "docs/features/h-20.md:8, {} more",
                        holders - NAMES
                    )),
                    "{context}: {tail}"
                );
            } else {
                assert!(!tail.contains(" more"), "{context}: {tail}");
            }

            let mut json_args = vec!["--json"];
            json_args.extend(&args);
            let json = spec(&home, &root, &json_args).json();
            let nodes = json["nodes"].as_array().unwrap();
            assert_eq!(
                nodes.len(),
                1,
                "{context}: the holders after the cut dropped"
            );
            let node = &nodes[0];
            assert_eq!(node["truncated"], true, "{context}");
            assert_omitted_keys(node, &context);
            let (first, more) = first_and_more(&names);
            assert_eq!(node["omitted"]["holders"], first, "{context}");
            assert_eq!(node["omitted"]["holders_more"], json!(more), "{context}");
            assert_eq!(node["omitted"]["sections"], json!([]), "{context}");
            assert_eq!(node["omitted"]["sections_more"], json!(0), "{context}");
            if links {
                assert_eq!(node["links"]["omitted"], json!(holders), "{context}");
            } else {
                assert!(node["links"].is_null(), "{context}");
            }
        }
    }
}
