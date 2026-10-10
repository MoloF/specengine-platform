//! docs/features/code-refs.md TE-1, TE-2 (CRLF), RD-4, RD-5: `spec refs`
//! over scratch copies of `fixtures/code-a` against its `expected.json`.
//! For every node the fixture names, the text is exactly the node's
//! header, its marker rows (by relation, path bytes, line, column), its
//! mention rows (by path, line, column) and the summary; the JSON holds
//! the same rows with every key, each derived from the text row as
//! "spec refs" defines it. The generated cases (`expected.json`
//! `generated`: the U+0420 look-alike, two holders of one ID, a RON
//! container past the depth cap) are written at test time; CRLF copies
//! are made at test time. The states `spec refs` lists under no node are
//! checked by the library (`crates/specengine-cli/src/refs/tests.rs`).
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

use common::{Run, Scratch, fixture, read_text, spec, write};

const RELATIONS: [&str; 4] = ["implements", "verifies", "configures", "assumes"];

/// The keys of a JSON marker, in the spec's order.
const MARKER_KEYS: [&str; 14] = [
    "relation",
    "path",
    "line",
    "column",
    "token",
    "id",
    "rev",
    "id_state",
    "fix",
    "rev_state",
    "levels",
    "levels_state",
    "note",
    "binding",
];

fn expected() -> Value {
    let text = std::fs::read_to_string(fixture("code-a").join("expected.json"))
        .expect("fixtures/code-a/expected.json");
    serde_json::from_str(&text).expect("expected.json is JSON")
}

/// A scratch copy of `fixtures/code-a` at `<scratch>/<dir>`, with the
/// generated files when asked.
fn code_a(scratch: &Scratch, dir: &str, generated: bool) -> std::path::PathBuf {
    let root = scratch.copy("code-a", dir);
    if generated {
        write_generated(&root, &expected());
    }
    root
}

fn write_generated(root: &Path, expected: &Value) {
    for file in expected["generated"]["files"].as_array().expect("files") {
        let mut text = String::new();
        for part in file["parts"].as_array().expect("parts") {
            let times = usize::try_from(part["times"].as_u64().expect("times")).expect("usize");
            text.push_str(&part["text"].as_str().expect("text").repeat(times));
        }
        write(root, file["path"].as_str().expect("path"), text);
    }
}

/// Every `.rs` and `.ron` file under `root`'s code roots, its LF made CRLF.
fn to_crlf(root: &Path) {
    for (path, bytes) in common::snapshot(root) {
        let Some(bytes) = bytes else { continue };
        if path.ends_with(".rs") || path.ends_with(".ron") {
            let text = String::from_utf8(bytes).expect("UTF-8 code");
            assert!(!text.contains('\r'), "{path} is LF to begin with");
            write(root, &path, text.replace('\n', "\r\n"));
        }
    }
}

// ------------------------------------------------------------ expectations

/// One expected node block.
struct Node {
    id: String,
    path: String,
    value: Value,
}

/// One expected marker or mention row under a node.
#[derive(Clone)]
struct Row {
    text: String,
    binding_line: Option<u64>,
}

struct Expectation {
    nodes: Vec<Node>,
    /// `(node id, node path)` → its marker rows, its mention rows.
    markers: BTreeMap<(String, String), Vec<Row>>,
    mentions: BTreeMap<(String, String), Vec<Row>>,
    code: Value,
}

fn rows_of(expectation: &mut Expectation, value: &Value, mention: bool) {
    for row in value.as_array().expect("rows") {
        let binding_line = row["binding_line"].as_u64();
        for id in row["listed_under"].as_array().expect("listed_under") {
            let id = id.as_str().expect("id");
            let holders: Vec<String> = expectation
                .nodes
                .iter()
                .filter(|node| node.id == id)
                .map(|node| node.path.clone())
                .collect();
            assert!(!holders.is_empty(), "expected.json lists {id} as no node");
            for path in holders {
                let text = match row["row"].as_str() {
                    Some(text) => text.to_owned(),
                    None => row["rows_by_holder"][&path]
                        .as_str()
                        .unwrap_or_else(|| panic!("no row for the holder {path}"))
                        .to_owned(),
                };
                let map = if mention {
                    &mut expectation.mentions
                } else {
                    &mut expectation.markers
                };
                map.entry((id.to_owned(), path))
                    .or_default()
                    .push(Row { text, binding_line });
            }
        }
    }
}

fn expectation(generated: bool) -> Expectation {
    let expected = expected();
    let mut node_values: Vec<Value> = expected["nodes"].as_array().expect("nodes").clone();
    if generated {
        node_values.extend(
            expected["generated"]["nodes"]
                .as_array()
                .expect("nodes")
                .clone(),
        );
    }
    let nodes = node_values
        .into_iter()
        .map(|value| Node {
            id: value["id"].as_str().expect("id").to_owned(),
            path: value["path"].as_str().expect("path").to_owned(),
            value,
        })
        .collect();
    let mut expectation = Expectation {
        nodes,
        markers: BTreeMap::new(),
        mentions: BTreeMap::new(),
        code: if generated {
            expected["generated"]["code"].clone()
        } else {
            expected["code"].clone()
        },
    };
    rows_of(&mut expectation, &expected["markers"], false);
    rows_of(&mut expectation, &expected["mentions"], true);
    if generated {
        rows_of(&mut expectation, &expected["generated"]["markers"], false);
        rows_of(&mut expectation, &expected["generated"]["mentions"], true);
    }
    expectation
}

fn fields(row: &str) -> Vec<&str> {
    row.split(" | ").collect()
}

/// `path:line:column` of a row.
fn location(field: &str) -> (String, u64, u64) {
    let mut parts = field.rsplitn(3, ':');
    let column = parts.next().expect("column").parse().expect("a column");
    let line = parts.next().expect("line").parse().expect("a line");
    (parts.next().expect("path").to_owned(), line, column)
}

fn marker_key(row: &Row) -> (usize, Vec<u8>, u64, u64) {
    let fields = fields(&row.text);
    let relation = RELATIONS
        .iter()
        .position(|relation| *relation == fields[0])
        .expect("a relation");
    let (path, line, column) = location(fields[1]);
    (relation, path.into_bytes(), line, column)
}

fn mention_key(row: &Row) -> (Vec<u8>, u64, u64) {
    let (path, line, column) = location(fields(&row.text)[1]);
    (path.into_bytes(), line, column)
}

/// The blocks of `id`, by path: (node, markers sorted, mentions sorted).
fn blocks(expectation: &Expectation, id: &str) -> Vec<(Value, Vec<Row>, Vec<Row>)> {
    let mut nodes: Vec<&Node> = expectation
        .nodes
        .iter()
        .filter(|node| node.id == id)
        .collect();
    nodes.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    nodes
        .into_iter()
        .map(|node| {
            let key = (node.id.clone(), node.path.clone());
            let mut markers = expectation.markers.get(&key).cloned().unwrap_or_default();
            markers.sort_by_key(marker_key);
            let mut mentions = expectation.mentions.get(&key).cloned().unwrap_or_default();
            mentions.sort_by_key(mention_key);
            (node.value.clone(), markers, mentions)
        })
        .collect()
}

/// The text `spec refs <id>` must print.
fn expected_text(expectation: &Expectation, id: &str) -> String {
    let blocks = blocks(expectation, id);
    let mut out = String::new();
    let (mut markers, mut mentions) = (0, 0);
    for (index, (node, marker_rows, mention_rows)) in blocks.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(node["header"].as_str().expect("header"));
        out.push('\n');
        for row in marker_rows.iter().chain(mention_rows) {
            out.push_str(&row.text);
            out.push('\n');
        }
        markers += marker_rows.len();
        mentions += mention_rows.len();
    }
    let code = &expectation.code;
    out.push_str(&format!(
        "refs: markers {markers}, mentions {mentions}; code files {} (.rs {}, .ron {}), skipped {}\n",
        code["files"], code["rs"], code["ron"], code["skipped"]
    ));
    out
}

/// A binding's JSON from its text form and the bound item's line.
fn binding_json(text: &str, line: Option<u64>) -> Value {
    let (kind, rest) = text.split_once(' ').unwrap_or((text, ""));
    let (rest, ambiguity) = match rest.rsplit_once(" ambiguous ") {
        Some((rest, reason)) => (rest, Some(reason)),
        None => (rest, None),
    };
    let mut value = json!({
        "kind": kind, "qpath": null, "item_kind": null, "line": null,
        "ambiguity": ambiguity, "category": null,
    });
    match kind {
        "item" => {
            let (qpath, item_kind) = rest.rsplit_once(' ').expect("qpath and kind");
            value["qpath"] = json!(qpath);
            value["item_kind"] = json!(item_kind);
            value["line"] = json!(line.expect("an item's line"));
        }
        "module" | "ron" => value["qpath"] = json!(rest),
        "unanchored" | "cannot_verify" => value["category"] = json!(rest),
        "file" => {}
        other => panic!("no binding word {other:?}"),
    }
    value
}

fn state_and_fix(field: &str) -> (&str, Option<&str>) {
    match field.split_once(' ') {
        Some((state, fix)) => (state, Some(fix)),
        None => (field, None),
    }
}

/// A marker's JSON from its text row ("spec refs": `id` is the token up to
/// its first `@`, `rev` the 1-9 digits after it ending the token).
fn marker_json(row: &Row) -> Value {
    let fields = fields(&row.text);
    let (path, line, column) = location(fields[1]);
    let token = fields[2];
    let (id, rev) = match token.split_once('@') {
        None => (token, None),
        Some((id, digits)) => {
            let ok = (1..=9).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit());
            (id, ok.then(|| digits.parse::<u64>().expect("digits")))
        }
    };
    let (id_state, fix) = state_and_fix(fields[3]);
    let (levels, levels_state) = match fields[5] {
        "default" => (json!(["sig", "body"]), "default"),
        invalid if invalid.starts_with("invalid:") => (Value::Null, &invalid["invalid:".len()..]),
        declared => (
            json!(
                declared
                    .trim_matches(['[', ']'])
                    .split(", ")
                    .collect::<Vec<_>>()
            ),
            "declared",
        ),
    };
    json!({
        "relation": fields[0], "path": path, "line": line, "column": column,
        "token": token, "id": id, "rev": rev, "id_state": id_state, "fix": fix,
        "rev_state": fields[4], "levels": levels, "levels_state": levels_state,
        "note": fields.get(7), "binding": binding_json(fields[6], row.binding_line),
    })
}

fn mention_json(row: &Row) -> Value {
    let fields = fields(&row.text);
    let (path, line, column) = location(fields[1]);
    let (id_state, fix) = state_and_fix(fields[3]);
    json!({
        "path": path, "line": line, "column": column, "written": fields[2],
        "id_state": id_state, "fix": fix,
        "binding": binding_json(fields[4], row.binding_line),
    })
}

/// The JSON `spec refs <reference>` must print for the blocks of `id`.
fn expected_json(expectation: &Expectation, reference: &str, id: &str) -> Value {
    let nodes: Vec<Value> = blocks(expectation, id)
        .into_iter()
        .map(|(node, markers, mentions)| {
            json!({
                "id": node["id"], "kind": node["kind"], "title": node["title"],
                "path": node["path"], "line": node["line"], "rev": node["rev"],
                "rev_written": node["rev_written"],
                "markers": markers.iter().map(marker_json).collect::<Vec<_>>(),
                "mentions": mentions.iter().map(mention_json).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "ref": reference, "reason": null, "notes": [], "nodes": nodes,
        "code": expectation.code, "omitted": {"markers": 0, "mentions": 0},
    })
}

/// The keys of the first JSON marker of `stdout`, in their order.
fn first_marker_keys(stdout: &str) -> Vec<String> {
    let start = stdout.find("\"markers\":[{").expect("a marker") + "\"markers\":[".len();
    let value: Value = serde_json::Deserializer::from_str(&stdout[start..])
        .into_iter::<Value>()
        .next()
        .expect("an object")
        .expect("JSON");
    let mut keys = Vec::new();
    let object = &stdout[start..];
    for key in value.as_object().expect("an object").keys() {
        let at = object.find(&format!("\"{key}\":")).expect("the key");
        keys.push((at, key.clone()));
    }
    keys.sort();
    keys.into_iter().map(|(_, key)| key).collect()
}

/// `spec refs <reference>` and `spec --json refs <reference>` in `root`.
fn refs(home: &Path, root: &Path, reference: &str) -> (Run, Run) {
    (
        spec(home, root, &["refs", reference]),
        spec(home, root, &["--json", "refs", reference]),
    )
}

/// Every node's block, text and JSON, against `expected.json`.
fn assert_every_node(root: &Path, home: &Path, expectation: &Expectation, ids: &[&str]) {
    let mut failures = Vec::new();
    for &id in ids {
        let (text, json_run) = refs(home, root, id);
        text.code(0);
        json_run.code(0);
        let want = expected_text(expectation, id);
        if text.stdout != want {
            failures.push(format!(
                "spec refs {id}\n--- got\n{}--- want\n{want}",
                text.stdout
            ));
        }
        let holders = blocks(expectation, id).len();
        if holders == 1 && !text.stderr.is_empty() {
            failures.push(format!("spec refs {id}: stderr {:?}", text.stderr));
        }
        let got = json_run.json();
        let want = expected_json(expectation, id, id);
        if got != want {
            failures.push(format!(
                "spec --json refs {id}\n--- got\n{}\n--- want\n{}",
                serde_json::to_string_pretty(&got).expect("JSON"),
                serde_json::to_string_pretty(&want).expect("JSON")
            ));
        }
        if json_run.stdout.contains("\"markers\":[{") {
            assert_eq!(first_marker_keys(&json_run.stdout), MARKER_KEYS, "{id}");
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

const BASE_IDS: [&str; 7] = [
    "PIECE-COUNTER",
    "LAW-BOUND",
    "LAW-STEP",
    "LAW-ODD",
    "GOAL-01",
    "PREM-001",
    "PASS-01",
];

// ------------------------------------------------------------------ TE-1

/// TE-1: every node of `fixtures/code-a` lists exactly `expected.json`'s
/// rows, text and JSON.
#[test]
fn code_a_refs_match_expected_json() {
    let scratch = Scratch::new("refs-base");
    let root = code_a(&scratch, "code-a", false);
    let home = scratch.home("h");
    assert_every_node(&root, &home, &expectation(false), &BASE_IDS);
}

/// TE-1, `generated`: the look-alike's row under its fix's node, the two
/// holders of `GOAL-03` (a `warning:`, a block each, each with its own
/// rev_state), the RON rows past the depth cap.
#[test]
fn code_a_generated_cases_match_expected_json() {
    let scratch = Scratch::new("refs-generated");
    let root = code_a(&scratch, "code-a", true);
    let home = scratch.home("h");
    let expectation = expectation(true);
    let mut ids = BASE_IDS.to_vec();
    ids.push("GOAL-03");
    assert_every_node(&root, &home, &expectation, &ids);
    let (text, _) = refs(&home, &root, "GOAL-03");
    assert_eq!(
        text.stderr,
        "warning: `GOAL-03` has 2 holders, all shown: docs/records/GOAL-03-twin.md, \
         docs/records/GOAL-03.md\n"
    );
}

/// TE-2 end to end: with every code file's line ends CRLF, every block is
/// byte for byte the LF one.
#[test]
fn code_a_refs_are_the_same_with_crlf_line_ends() {
    let scratch = Scratch::new("refs-crlf");
    let root = code_a(&scratch, "code-a", true);
    to_crlf(&root);
    let home = scratch.home("h");
    let mut ids = BASE_IDS.to_vec();
    ids.push("GOAL-03");
    assert_every_node(&root, &home, &expectation(true), &ids);
}

/// "spec refs": REF takes every `spec show` form — `slug/ID`, an
/// `aliases_from` ID, `ID#SECTION`, a root-relative `.md` path — and
/// `@rev` is ignored with show's `note:`.
#[test]
fn refs_takes_every_show_form_of_ref() {
    let scratch = Scratch::new("refs-forms");
    let root = code_a(&scratch, "code-a", false);
    let home = scratch.home("h");
    let expectation = expectation(false);
    for (reference, id) in [
        ("cap-check/PASS-01", "PASS-01"),
        ("AIM-01", "GOAL-01"),
        ("PIECE-COUNTER#LAW-BOUND", "LAW-BOUND"),
        ("docs/spec/counter.md", "PIECE-COUNTER"),
    ] {
        let (text, json_run) = refs(&home, &root, reference);
        text.code(0);
        assert_eq!(text.stdout, expected_text(&expectation, id), "{reference}");
        assert_eq!(
            json_run.json(),
            expected_json(&expectation, reference, id),
            "{reference}"
        );
    }
    let (text, json_run) = refs(&home, &root, "LAW-BOUND@2");
    text.code(0);
    assert_eq!(text.stdout, expected_text(&expectation, "LAW-BOUND"));
    assert_eq!(text.stderr_lines().len(), 1, "{}", text.show());
    assert!(text.stderr.starts_with("note: "), "{}", text.show());
    assert_eq!(json_run.json()["notes"].as_array().expect("notes").len(), 1);
}

/// "Exit": REF naming nothing is exit 1 with show's reason, JSON `nodes`
/// `[]`, `code` and `omitted` `null`; a look-alike REF is exit 2 naming
/// its Latin fix, stdout empty.
#[test]
fn refs_exits_1_for_nothing_and_2_for_a_look_alike_ref() {
    let scratch = Scratch::new("refs-exits");
    let root = code_a(&scratch, "code-a", false);
    let home = scratch.home("h");
    let (text, json_run) = refs(&home, &root, "GOAL-99");
    text.code(1);
    assert_eq!(text.stdout, "");
    assert!(text.stderr.starts_with("spec: "), "{}", text.show());
    json_run.code(1);
    let value = json_run.json();
    assert_eq!(value["nodes"], json!([]));
    assert_eq!(value["code"], Value::Null);
    assert_eq!(value["omitted"], Value::Null);
    assert!(value["reason"].is_string());

    let look_alike = "\u{0420}IECE-COUNTER";
    let (text, json_run) = refs(&home, &root, look_alike);
    text.code(2);
    json_run.code(2);
    assert_eq!(text.stdout, "");
    assert_eq!(json_run.stdout, "");
    assert!(text.stderr.contains("PIECE-COUNTER"), "{}", text.show());
}

// ------------------------------------------------------------------ RD-4

fn row_under<'r>(stdout: &'r str, location: &str) -> Vec<&'r str> {
    stdout
        .lines()
        .filter(|line| line.split(" | ").nth(1) == Some(location))
        .collect()
}

/// RD-4. M: the document's revision for a section turns `LAW-STEP@1`
/// (the section writes none: 1) into `spec_ahead` and `LAW-STEP@2` into
/// `current` (the document is at 3).
#[test]
fn section_marker_compares_the_section_rev() {
    let scratch = Scratch::new("refs-section");
    let root = code_a(&scratch, "code-a", false);
    let home = scratch.home("h");
    let (text, _) = refs(&home, &root, "LAW-STEP");
    text.code(0);
    assert!(
        text.stdout.starts_with(
            "LAW-STEP | law | Step | docs/spec/counter.md:17 | rev 1 (none written)\n"
        )
    );
    assert_eq!(
        row_under(&text.stdout, "crates/tally/src/counter.rs:23:8"),
        [
            "implements | crates/tally/src/counter.rs:23:8 | LAW-STEP@1 | resolved | current | default | item crates/tally::counter::Tally::step function_item"
        ]
    );
    assert_eq!(
        row_under(&text.stdout, "crates/tally/src/counter.rs:74:9"),
        [
            "implements | crates/tally/src/counter.rs:74:9 | LAW-STEP@2 | resolved | predated | default | module crates/tally::counter::inner"
        ]
    );
    let (bound, _) = refs(&home, &root, "LAW-BOUND");
    assert!(
        bound
            .stdout
            .contains(" | PIECE-COUNTER#LAW-BOUND@2 | resolved | current | ")
    );
}

/// RD-4. M: `>=` for `current` makes `LAW-BOUND@3` (the section is at 2)
/// `current`.
#[test]
fn newer_marker_is_predated() {
    let scratch = Scratch::new("refs-predated");
    let root = code_a(&scratch, "code-a", false);
    let home = scratch.home("h");
    let (text, json_run) = refs(&home, &root, "LAW-BOUND");
    text.code(0);
    assert_eq!(
        row_under(&text.stdout, "crates/tally/src/counter.rs:22:17"),
        [
            "implements | crates/tally/src/counter.rs:22:17 | LAW-BOUND@3 | resolved | predated | default | item crates/tally::counter::impl Tally impl_item ambiguous duplicate"
        ]
    );
    let value = json_run.json();
    let states: Vec<&str> = value["nodes"][0]["markers"]
        .as_array()
        .expect("markers")
        .iter()
        .map(|marker| marker["rev_state"].as_str().expect("rev_state"))
        .collect();
    assert!(
        states.contains(&"current") && states.contains(&"spec_ahead"),
        "{states:?}"
    );
}

/// RD-4. M: resolving a look-alike lists the U+0420 marker as `resolved`
/// with its `@3` `current`.
#[test]
fn homoglyph_marker_is_never_resolved() {
    let scratch = Scratch::new("refs-homoglyph");
    let root = code_a(&scratch, "code-a", true);
    let home = scratch.home("h");
    let (text, json_run) = refs(&home, &root, "PIECE-COUNTER");
    text.code(0);
    assert_eq!(
        row_under(&text.stdout, "crates/tally/src/look.rs:1:4"),
        [
            "implements | crates/tally/src/look.rs:1:4 | \u{0420}IECE-COUNTER@3 | homoglyph PIECE-COUNTER | rev_unknown | default | item crates/tally::look::look function_item"
        ]
    );
    let value = json_run.json();
    let look = value["nodes"][0]["markers"]
        .as_array()
        .expect("markers")
        .iter()
        .find(|marker| marker["path"] == "crates/tally/src/look.rs")
        .expect("the look-alike's row");
    assert_eq!(look["id_state"], "homoglyph");
    assert_eq!(look["fix"], "PIECE-COUNTER");
    assert_eq!(look["id"], "\u{0420}IECE-COUNTER");
    assert_eq!(look["rev"], 3);
    assert_eq!(look["rev_state"], "rev_unknown");
}

// ------------------------------------------------------------------ RD-5

/// RD-5. M: scanning string literals lists `counter.rs:11`'s
/// `"GOAL-01 // @implements LAW-STEP"` as a mention (and a marker).
#[test]
fn string_literal_ids_are_not_mentions() {
    let scratch = Scratch::new("refs-strings");
    let root = code_a(&scratch, "code-a", false);
    let home = scratch.home("h");
    let source = read_text(&root, "crates/tally/src/counter.rs");
    assert_eq!(
        source.lines().nth(10),
        Some("pub const LABEL: &str = \"GOAL-01 // @implements LAW-STEP\";")
    );
    for id in ["GOAL-01", "LAW-STEP"] {
        let (text, _) = refs(&home, &root, id);
        text.code(0);
        let on_line: Vec<&str> = text
            .stdout
            .lines()
            .filter(|line| line.contains("crates/tally/src/counter.rs:11:"))
            .collect();
        assert!(on_line.is_empty(), "{id}: {on_line:?}");
    }
}

/// RD-5. M: counting the marker's own token adds a mention at every
/// marker (`counter.rs:4:16` under GOAL-01); a mention in a marker's note
/// still counts (`counter.rs:4:28` under LAW-STEP).
#[test]
fn marker_id_is_not_a_mention() {
    let scratch = Scratch::new("refs-own-token");
    let root = code_a(&scratch, "code-a", false);
    let home = scratch.home("h");
    let (goal, _) = refs(&home, &root, "GOAL-01");
    goal.code(0);
    let mentions: Vec<&str> = goal
        .stdout
        .lines()
        .filter(|line| line.starts_with("mention | "))
        .collect();
    assert_eq!(
        mentions,
        [
            "mention | crates/tally/src/counter.rs:15:40 | AIM-01 | alias | item crates/tally::counter::Tally struct_item",
            "mention | crates/tally/src/counter.rs:91:26 | GOAL-01 | resolved | file",
            "mention | crates/tally/src/lib.rs:1:44 | GOAL-01 | resolved | module crates/tally",
            "mention | data/limits.ron:1:25 | GOAL-01 | resolved | ron data/limits.ron#root",
        ]
    );
    let (step, _) = refs(&home, &root, "LAW-STEP");
    assert!(
        step.stdout
            .contains("mention | crates/tally/src/counter.rs:4:28 | LAW-STEP | resolved | file\n"),
        "{}",
        step.show()
    );
}

/// RD-5. M: a doc comment's mention given to the file: `counter.rs:7`
/// (`/// ... see LAW-BOUND.` above `CAP`) and `:15` (`AIM-01` above the
/// derive of `Tally`) belong to their items.
#[test]
fn doc_comment_mention_belongs_to_its_item() {
    let scratch = Scratch::new("refs-doc-mention");
    let root = code_a(&scratch, "code-a", false);
    let home = scratch.home("h");
    let (bound, json_run) = refs(&home, &root, "LAW-BOUND");
    bound.code(0);
    assert!(bound.stdout.contains(
        "mention | crates/tally/src/counter.rs:7:28 | LAW-BOUND | resolved | item crates/tally::counter::CAP const_item\n"
    ));
    let mention = &json_run.json()["nodes"][0]["mentions"][0];
    assert_eq!(
        mention["binding"],
        json!({"kind": "item", "qpath": "crates/tally::counter::CAP", "item_kind": "const_item",
               "line": 8, "ambiguity": null, "category": null})
    );
    let (goal, _) = refs(&home, &root, "GOAL-01");
    assert!(goal.stdout.contains(
        "mention | crates/tally/src/counter.rs:15:40 | AIM-01 | alias | item crates/tally::counter::Tally struct_item\n"
    ));
}

// ------------------------------------------------ marker tokens (iteration 2)

/// Review iteration 2: a token ends at an `@` beginning a keyword.
/// `@implements @verifies GOAL-01` lists the `verifies` row under GOAL-01
/// (the empty first ID names nothing); `@implements LAW-STEP@@verifies
/// GOAL-01` lists `LAW-STEP@` (`bad_rev`, no note) under LAW-STEP and the
/// `verifies` row under GOAL-01. M: drop the keyword stop.
#[test]
fn a_keyword_after_an_empty_or_glued_token_is_a_row_of_its_own() {
    let scratch = Scratch::new("refs-glued");
    let root = code_a(&scratch, "code-a", false);
    write(
        &root,
        "crates/tally/src/twice.rs",
        "// @implements @verifies GOAL-01\npub fn both() {}\n\n// @implements LAW-STEP@@verifies GOAL-01\npub fn glued() {}\n",
    );
    let home = scratch.home("h");
    let (goal, _) = refs(&home, &root, "GOAL-01");
    goal.code(0);
    let rows: Vec<&str> = goal
        .stdout
        .lines()
        .filter(|line| line.contains("crates/tally/src/twice.rs"))
        .collect();
    assert_eq!(
        rows,
        [
            "verifies | crates/tally/src/twice.rs:1:16 | GOAL-01 | resolved | unversioned | default | item crates/tally::twice::both function_item",
            "verifies | crates/tally/src/twice.rs:4:25 | GOAL-01 | resolved | unversioned | default | item crates/tally::twice::glued function_item",
        ]
    );
    let (step, _) = refs(&home, &root, "LAW-STEP");
    let rows: Vec<&str> = step
        .stdout
        .lines()
        .filter(|line| line.contains("crates/tally/src/twice.rs"))
        .collect();
    assert_eq!(
        rows,
        [
            "implements | crates/tally/src/twice.rs:4:4 | LAW-STEP@ | resolved | bad_rev | default | item crates/tally::twice::glued function_item"
        ]
    );
}
