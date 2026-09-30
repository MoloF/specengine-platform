//! AC-09 of docs/features/spec-cli.md: `spec show` prints a header per
//! node and the node's bytes (a section's span, a document's file) from a
//! parse of the very bytes it prints; its JSON has exactly the keys of
//! "Data". Every ID of both fixtures is checked against the parser (AC-17).

#![cfg(unix)]

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::{FIXTURES, Scratch, index, lines, md_files, read, read_text, spec, write};
use specengine_core::ProjectConfig;
use specengine_model::ParsedFile;

const STAMINA: &str = "docs/spec/movement/stamina.md";

fn parse(root: &std::path::Path, path: &str) -> ParsedFile {
    let config = ProjectConfig::from_toml(&read_text(root, "specengine.toml")).expect("config");
    specengine_core::parse(path, &read(root, path), &config.scheme)
}

fn tokens_of(parsed: &ParsedFile, id: &str) -> u32 {
    parsed
        .nodes
        .iter()
        .find(|node| node.id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("{id} in the parse"))
        .tokens_est
}

#[test]
fn a_section_prints_its_header_then_its_lines_byte_for_byte() {
    let scratch = Scratch::new("show-section");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    index(&home, &root);
    let run = spec(&home, &root, &["show", "RULE-STAM-REGEN"]);
    run.code(0);
    assert_eq!(run.stderr, "", "{}", run.show());
    let tokens = tokens_of(&parse(&root, STAMINA), "RULE-STAM-REGEN");
    let mut want = format!(
        "RULE-STAM-REGEN | rule | Regeneration | docs/spec/movement/stamina.md:21 | {tokens} tokens\n"
    )
    .into_bytes();
    want.extend(lines(&read(&root, STAMINA), 21, 23));
    assert_eq!(
        run.stdout.as_bytes(),
        want.as_slice(),
        "{}\nwant:\n{}",
        run.show(),
        String::from_utf8_lossy(&want)
    );
}

#[test]
fn a_document_and_a_path_print_the_whole_file() {
    let scratch = Scratch::new("show-document");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    index(&home, &root);

    let run = spec(&home, &root, &["show", "MEC-STAMINA"]);
    run.code(0);
    let tokens = tokens_of(&parse(&root, STAMINA), "MEC-STAMINA");
    let mut want = format!(
        "MEC-STAMINA | mechanic | Stamina | docs/spec/movement/stamina.md:1 | {tokens} tokens | status accepted\n"
    )
    .into_bytes();
    want.extend(read(&root, STAMINA));
    assert_eq!(run.stdout.as_bytes(), want.as_slice(), "{}", run.show());

    for path in ["docs/spec/game.md", STAMINA] {
        let run = spec(&home, &root, &["show", path]);
        run.code(0);
        let (header, body) = run.stdout.split_once('\n').unwrap();
        assert!(
            header.contains(&format!(" | {path}:1 | ")),
            "{path}: {header}"
        );
        assert_eq!(body.as_bytes(), read(&root, path).as_slice(), "{path}");
    }
    // The document by path and by ID print the same.
    let by_path = spec(&home, &root, &["show", STAMINA]);
    assert_eq!(by_path.stdout, run_stdout(&home, &root, "MEC-STAMINA"));
}

fn run_stdout(home: &std::path::Path, root: &std::path::Path, reference: &str) -> String {
    let run = spec(home, root, &["show", reference]);
    run.code(0);
    run.stdout
}

/// `--json`: `{ref, reason, notes, nodes}`, a node exactly the keys of
/// "Data", every key present, absent = `null`.
#[test]
fn show_json_has_exactly_the_data_keys() {
    let scratch = Scratch::new("show-json");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    index(&home, &root);
    let json = spec(&home, &root, &["--json", "show", "MEC-STAMINA"]).json();
    let top: BTreeSet<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        top,
        BTreeSet::from(["ref", "reason", "notes", "nodes"]),
        "{json}"
    );
    let node = &json["nodes"][0];
    let keys: BTreeSet<&str> = node
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from([
            "id",
            "kind",
            "title",
            "path",
            "line",
            "end_line",
            "status",
            "rev",
            "tokens_est",
            "archived",
            "utf8",
            "sections",
            "text",
            "truncated",
            "omitted",
        ]),
        "{json}"
    );
    assert_eq!(json["ref"], "MEC-STAMINA");
    assert!(json["reason"].is_null());
    assert_eq!(json["notes"], serde_json::json!([]));
    assert_eq!(node["id"], "MEC-STAMINA");
    assert_eq!(node["kind"], "mechanic");
    assert_eq!(node["title"], "Stamina");
    assert_eq!(node["path"], STAMINA);
    assert_eq!(node["line"], 1);
    assert_eq!(node["end_line"], 26);
    assert_eq!(node["status"], "accepted");
    assert!(node["rev"].is_null());
    assert_eq!(node["archived"], false);
    assert_eq!(node["utf8"], true);
    assert_eq!(
        node["sections"],
        serde_json::json!(["RULE-STAM-REGEN", "EDGE-STAM-ZERO"])
    );
    assert_eq!(node["text"], read_text(&root, STAMINA));
    assert_eq!(node["truncated"], false);
    assert!(node["omitted"].is_null());

    // A section: no status (a document's), no nested sections.
    let json = spec(&home, &root, &["--json", "show", "RULE-STAM-REGEN"]).json();
    let node = &json["nodes"][0];
    assert!(node["status"].is_null(), "{json}");
    assert_eq!(node["line"], 21);
    assert_eq!(node["end_line"], 23);
    assert_eq!(node["sections"], serde_json::json!([]));
    // `@rev` is a note, also in JSON.
    let json = spec(&home, &root, &["--json", "show", "R-12@3"]).json();
    assert_eq!(json["notes"].as_array().unwrap().len(), 1, "{json}");
}

/// The header's optional fields, in order: status, rev, archived, not UTF-8.
#[test]
fn the_header_flags_come_in_order() {
    let scratch = Scratch::new("show-flags");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    write(
        &root,
        "docs/records/DEC/DEC-0099.md",
        "---\nid: DEC-0099\nclass: decision\nstatus: rejected\nrev: 3\n---\n\n# Old one\n\nBody.\n",
    );
    write(
        &root,
        "docs/records/DEC/raw.md",
        b"# Raw \xff bytes\n".as_slice(),
    );
    index(&home, &root);
    let run = spec(&home, &root, &["show", "DEC-0099"]);
    run.code(0);
    let header = run.stdout.lines().next().unwrap();
    assert!(
        header.ends_with(" tokens | status rejected | rev 3 | archived"),
        "{header}"
    );
    let json = spec(&home, &root, &["--json", "show", "DEC-0099"]).json();
    assert_eq!(json["nodes"][0]["rev"], 3);
    assert_eq!(json["nodes"][0]["archived"], true);
    let run = spec(&home, &root, &["show", "docs/records/DEC/raw.md"]);
    run.code(0);
    let header = run.stdout.lines().next().unwrap();
    assert!(
        header.starts_with("docs/records/DEC/raw.md | - | - | docs/records/DEC/raw.md:1 | ")
            && header.ends_with(" tokens | not UTF-8"),
        "{header}"
    );
    assert_eq!(
        run.stdout.split_once('\n').unwrap().1,
        "# Raw \u{fffd} bytes\n"
    );
}

/// Every ID defined once in each fixture: `show --json ID` gives the
/// parser's node — path, line, end line, kind, title, tokens and the
/// span's bytes.
#[test]
fn every_id_of_both_fixtures_shows_its_span() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("show-every");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        index(&home, &root);
        let mut holders: BTreeMap<String, Vec<(String, usize)>> = BTreeMap::new();
        let mut parses = BTreeMap::new();
        for file in md_files(&root.join("docs")) {
            let path = format!("docs/{file}");
            let parsed = parse(&root, &path);
            for (ord, node) in parsed.nodes.iter().enumerate() {
                if let Some(id) = &node.id {
                    holders
                        .entry(id.clone())
                        .or_default()
                        .push((path.clone(), ord));
                }
            }
            parses.insert(path, parsed);
        }
        let mut checked = 0;
        for (id, places) in &holders {
            let [(path, ord)] = places.as_slice() else {
                continue;
            };
            let bytes = read(&root, path);
            let node = &parses[path].nodes[*ord];
            let run = spec(&home, &root, &["--json", "show", id]);
            run.code(0);
            let json = run.json();
            let shown = &json["nodes"][0];
            assert_eq!(
                json["nodes"].as_array().unwrap().len(),
                1,
                "{fixture}: {id}"
            );
            let line = |offset: usize| bytes[..offset].iter().filter(|&&b| b == b'\n').count() + 1;
            assert_eq!(shown["path"], path.as_str(), "{fixture}: {id}");
            assert_eq!(shown["line"], line(node.span.start), "{fixture}: {id}");
            assert_eq!(
                shown["end_line"],
                line(node.span.end.saturating_sub(1).max(node.span.start)),
                "{fixture}: {id}"
            );
            assert_eq!(
                shown["kind"],
                serde_json::json!(node.kind),
                "{fixture}: {id}"
            );
            assert_eq!(
                shown["title"],
                serde_json::json!(node.title),
                "{fixture}: {id}"
            );
            assert_eq!(shown["tokens_est"], node.tokens_est, "{fixture}: {id}");
            assert_eq!(
                shown["text"],
                String::from_utf8_lossy(&bytes[node.span.range()]).as_ref(),
                "{fixture}: {id}"
            );
            checked += 1;
        }
        assert!(checked > 10, "{fixture}: only {checked} IDs checked");
    }
}
