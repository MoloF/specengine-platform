//! docs/features/workspaces.md (RM-0081, v4 step 1a) "Acceptance criteria",
//! the CLI's half: `spec workspace [--json]` on scratch copies of
//! `fixtures/spec-a` (a game's brief, no table), `fixtures/card-press` (a
//! print shop, the example tables) and `fixtures/studio-lamp` (a lighthouse
//! game, the gamedev preset), so one binary reads projects of different
//! nature (ADR-0008). Every expected value is worked out from the spec's
//! "Data" and "Notes, exactly", never read back from a run.
//!
//! - WS-1: spec-a's implicit document and text, byte for byte; the command
//!   reads the config alone: no index, nothing written. M1: `tasks` kept in
//!   `IMPLICIT_SECTIONS`.
//! - card-press's document and text of "Data", its `expected.json` text.
//! - WS-4 (a) to (g): exit 0, each note's text, `note:` lines equal to JSON
//!   `notes`, and `spec check --json`'s `counts` and `spec tree --json` equal to
//!   the same copy's with both tables removed. M7: a malformed table refused
//!   by `ProjectConfig`. M8, M9 as core's.
//! - WS-7: studio-lamp's text is its `expected.json`'s, with no note.
//! - WS-8: on card-press every command exits 0; `[[workspaces]]` makes each
//!   exit 2 at its line with the TOML reader's message. M18: `workspace` or
//!   `module` missing from `RawFile`.
//! - Exit 2 on usage, no project and a refused config; `--config FILE` names
//!   the file as given in every note.
//!
//! The reader's grammar is core's `tests/workspace_toml.rs`; the route,
//! `GET /api/projects` and a daemon's start are `specengine-http`'s
//! `tests/workspaces.rs`; the MCP server's `initialize` and `tools/list` are
//! `specengine-mcp`'s `tests/mcp_workspaces.rs`.

#![cfg(unix)]

mod common;
mod workspaces_common;

use std::fs;
use std::path::Path;

use common::proposal::Pair;
use common::{Run, Scratch, fixture, read_text, snapshot, spec, write};
use serde_json::{Value, json};
use specengine_core::card_from_toml;
use workspaces_common::{
    CONFIG, IMPLICIT_JSON, IMPLICIT_TEXT, PRESS_JSON, PRESS_TEXT, line_holding, misspelt,
    nth_line_holding, variants, without_tables,
};

/// The spec's list of section IDs inside the unknown-section note.
const SECTION_LIST: &str = "(inbox, tasks, board, roadmap, flow, tree, graph, health, questions)";

/// `spec workspace --json` in `root`: exit 0, one document.
fn document(home: &Path, root: &Path) -> Run {
    let run = spec(home, root, &["workspace", "--json"]);
    run.code(0);
    run
}

/// `spec workspace` in `root`: exit 0.
fn text(home: &Path, root: &Path) -> Run {
    let run = spec(home, root, &["workspace"]);
    run.code(0);
    run
}

/// The JSON `notes` of `run`.
fn notes_of(run: &Run) -> Vec<String> {
    run.json()["notes"]
        .as_array()
        .unwrap_or_else(|| panic!("no notes: {}", run.show()))
        .iter()
        .map(|note| note.as_str().expect("a note is a string").to_owned())
        .collect()
}

/// The `note:` lines `notes` print on stderr, each with its line end.
fn note_lines(notes: &[String]) -> String {
    notes.iter().map(|note| format!("note: {note}\n")).collect()
}

/// `<config>:<line>: <message>`.
fn note(line: usize, message: &str) -> String {
    format!("{CONFIG}:{line}: {message}")
}

/// The entries under `home`: nothing once a command took no index.
fn entries(home: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(home)
        .expect("HOME")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// WS-1: on spec-a, `spec workspace --json` prints exactly the implicit
/// document of "Data" and the text its two lines; stderr empty; a repeat the
/// same bytes. It reads the config alone: the fresh `HOME` holds nothing after
/// and no file under the root changed. M1.
#[test]
fn ws1_the_implicit_workspace_on_spec_a() {
    let scratch = Scratch::new("ws-implicit");
    let home = scratch.home("fresh");
    let root = scratch.copy("spec-a", "a");
    let before = snapshot(&root);

    let run = document(&home, &root);
    assert_eq!(run.stdout, format!("{IMPLICIT_JSON}\n"), "{}", run.show());
    assert_eq!(run.stderr, "", "{}", run.show());
    assert_eq!(document(&home, &root).stdout, run.stdout, "a repeat");

    let shown = text(&home, &root);
    assert_eq!(shown.stdout, IMPLICIT_TEXT, "{}", shown.show());
    assert_eq!(shown.stderr, "", "{}", shown.show());

    assert_eq!(
        entries(&home),
        Vec::<String>::new(),
        "no index, no data directory"
    );
    assert!(snapshot(&root) == before, "nothing written under the root");
}

/// "Data": card-press's document and text, byte for byte, the text its
/// `expected.json`'s `workspaces`; no note.
#[test]
fn the_press_document_and_text_are_the_spec_s() {
    let scratch = Scratch::new("ws-press");
    let home = scratch.home("h");
    let root = scratch.copy("card-press", "press");
    let run = document(&home, &root);
    assert_eq!(run.stdout, format!("{PRESS_JSON}\n"), "{}", run.show());
    assert_eq!(run.stderr, "", "{}", run.show());
    let shown = text(&home, &root);
    assert_eq!(shown.stdout, PRESS_TEXT, "{}", shown.show());
    assert_eq!(shown.stderr, "");
    let expected: Value =
        serde_json::from_str(&read_text(&fixture("card-press"), "expected.json")).expect("JSON");
    assert_eq!(expected["workspaces"], json!(PRESS_TEXT));
    assert_eq!(entries(&home), Vec::<String>::new(), "no index");
}

/// The notes WS-4 expects of each variant of `base`, worked out from "Notes,
/// exactly" (lines in that copy).
fn expected_notes(label: &str, text: &str) -> Vec<String> {
    match &label[..3] {
        "(a)" => vec![note(
            line_holding(text, "\"stage\""),
            &format!("workspace `newsroom`: `stage` is no section {SECTION_LIST}: left out"),
        )],
        "(b)" => vec![note(
            line_holding(text, "\"inks\""),
            "workspace `newsroom`: no `[[card.flow]]` is named `inks`: left out",
        )],
        "(c)" => vec![note(
            line_holding(text, "landing = \"inbox\""),
            "workspace `pressroom`: landing `inbox` is not one of its sections: it lands on the \
             Overview",
        )],
        "(d)" => vec![note(
            line_holding(text, "plates-room"),
            "workspace `pressroom`: no `[[module]]` is named `plates-room`: no default module",
        )],
        "(e)" => vec![note(
            line_holding(text, "News room"),
            "`News room` is not a name: lower-case ASCII letters, digits, `_` and `-`; the \
             implicit workspace is served",
        )],
        "(f)" => vec![
            note(
                line_holding(text, "module = \"weekly\""),
                "workspace `newsroom`: no `[[module]]` is named `weekly`: no default module",
            ),
            note(
                nth_line_holding(text, "[[module]]", 2),
                "`[[module]]` is required to hold `root`; no module is served",
            ),
        ],
        "(g)" => {
            let card = card_from_toml(text).expect_err("copy has no accept");
            vec![format!(
                "{}; the workspaces' flows are served unjudged",
                card.at(CONFIG)
            )]
        }
        other => panic!("no variant {other}"),
    }
}

/// What else WS-4 names of each variant's document.
fn assert_document(label: &str, doc: &Value) {
    let workspace = |index: usize, key: &str| doc["workspaces"][index][key].clone();
    match &label[..3] {
        "(a)" => assert_eq!(workspace(0, "sections"), json!(["board", "tree", "inbox"])),
        "(b)" => assert_eq!(workspace(0, "flows"), json!(["copy"])),
        "(c)" => assert_eq!(workspace(1, "landing"), json!("overview")),
        "(d)" => assert_eq!(workspace(1, "module"), Value::Null),
        "(e)" => {
            assert_eq!(doc["source"], "implicit", "{doc}");
            assert_eq!(workspace(0, "name"), json!("all"), "{doc}");
            assert_eq!(
                doc["modules"].as_array().map(Vec::len),
                Some(2),
                "M8: both modules still served: {doc}"
            );
        }
        "(f)" => {
            assert_eq!(doc["source"], "config", "{doc}");
            assert_eq!(doc["modules"], json!([]), "{doc}");
            assert_eq!(workspace(0, "module"), Value::Null, "{doc}");
        }
        "(g)" => {
            assert_eq!(workspace(0, "flows"), json!(["copy"]), "{doc}");
            assert_eq!(workspace(1, "flows"), json!(["plates"]), "{doc}");
        }
        other => panic!("no variant {other}"),
    }
    if &label[..3] != "(e)" {
        assert_eq!(doc["source"], "config", "{label}: {doc}");
        assert_eq!(
            doc["workspaces"].as_array().map(Vec::len),
            Some(2),
            "{label}: {doc}"
        );
    }
}

/// `spec check --json`'s `counts` and `spec tree --json`'s bytes in `root`.
fn reads(home: &Path, root: &Path) -> (Value, String) {
    let check = spec(home, root, &["check", "--json"]);
    assert!([0, 1].contains(&check.code), "{}", check.show());
    let tree = spec(home, root, &["tree", "--json"]);
    tree.code(0);
    (check.json()["counts"].clone(), tree.stdout)
}

/// WS-4, W1, W2: each of (a) to (g) on its own copy of card-press. `spec
/// workspace` exits 0 in both forms; its `note:` lines are its JSON `notes`,
/// which are the spec's texts at the offending value's line; the document
/// keeps the rest. `spec check --json`'s `counts` and `spec tree --json` are
/// those of the same copy with both tables removed. M7, M8, M9.
#[test]
fn ws4_a_bad_table_fails_nothing() {
    let base = workspaces_common::press_config();
    for (label, changed) in variants(&base) {
        let scratch = Scratch::new("ws-variant");
        let home = scratch.home("h");
        let root = scratch.copy("card-press", "press");
        write(&root, CONFIG, &changed);

        let run = document(&home, &root);
        let notes = notes_of(&run);
        assert_eq!(
            notes,
            expected_notes(label, &changed),
            "{label}: {}",
            run.show()
        );
        assert_eq!(run.stderr, note_lines(&notes), "{label}: the `note:` lines");
        assert_document(label, &run.json());
        let shown = text(&home, &root);
        assert_eq!(
            shown.stderr,
            note_lines(&notes),
            "{label}: the text's notes"
        );
        assert_eq!(
            document(&home, &root).stdout,
            run.stdout,
            "{label}: a repeat"
        );

        let with_tables = reads(&home, &root);
        write(&root, CONFIG, without_tables(&changed));
        let without = reads(&home, &root);
        assert_eq!(with_tables.0, without.0, "{label}: check's counts");
        assert!(
            with_tables.1 == without.1,
            "{label}: spec tree --json is the same bytes"
        );
    }
}

/// WS-7: studio-lamp's text is its `expected.json`'s `workspaces`, with no
/// note, and its JSON `source` reads `config`; its check holds no error and its
/// tree the five documents of "Fixtures".
#[test]
fn ws7_studio_lamp_text_is_its_expected_json() {
    let scratch = Scratch::new("ws-lamp");
    let home = scratch.home("h");
    let root = scratch.copy("studio-lamp", "lamp");
    let expected: Value =
        serde_json::from_str(&read_text(&fixture("studio-lamp"), "expected.json")).expect("JSON");
    let shown = text(&home, &root);
    assert_eq!(
        json!(shown.stdout),
        expected["workspaces"],
        "{}",
        shown.show()
    );
    assert_eq!(shown.stderr, "", "no note");
    let run = document(&home, &root);
    assert_eq!(run.json()["source"], "config");
    assert_eq!(notes_of(&run), Vec::<String>::new());
    assert_eq!(run.stderr, "");

    let check = spec(&home, &root, &["check", "--json"]);
    assert_eq!(
        check.json()["counts"]["errors"],
        json!(0),
        "{}",
        check.show()
    );
    let tree = spec(&home, &root, &["tree", "--json"]);
    tree.code(0);
    for id in [
        "CHP-SHORE",
        "SCN-ARRIVAL",
        "SCN-STORM",
        "ISL-LAMP",
        "ISL-COVE",
    ] {
        assert!(
            tree.stdout.contains(&format!("\"{id}\"")),
            "{id}: {}",
            tree.show()
        );
    }
}

/// The commands WS-8 names on a terminal, each with what it needs.
const COMMANDS: [&[&str]; 7] = [
    &["index"],
    &["check"],
    &["tree"],
    &["flow"],
    &["card", "list"],
    &["team"],
    &["workspace"],
];

/// WS-8: on card-press, its tables in place, `spec index`, `spec check` (no
/// error), `spec tree`, `spec flow`, `spec card list`, `spec team` and `spec
/// workspace` each exit 0. With the first `[[workspace]]` misspelt
/// `[[workspaces]]`, each exits 2 naming `specengine.toml:<line>: ` and the
/// TOML reader's unknown-field message. M18.
#[test]
fn ws8_every_command_loads_the_tables() {
    let pair = Pair::new("ws-commands", "card-press");
    for args in COMMANDS {
        let run = pair.spec_piped(&pair.main, args, b"");
        run.code(0);
    }
    let check = pair.spec_piped(&pair.main, &["check", "--json"], b"");
    check.code(0);
    assert_eq!(
        check.json()["counts"]["errors"],
        json!(0),
        "{}",
        check.show()
    );

    let text = misspelt(&read_text(&pair.main, CONFIG));
    write(&pair.main, CONFIG, &text);
    let line = line_holding(&text, "[[workspaces]]");
    let opening = format!("{CONFIG}:{line}: unknown field `workspaces`, expected one of ");
    for args in COMMANDS {
        let run = pair.spec_piped(&pair.main, args, b"");
        run.code(2);
        let said = format!("{}{}", run.stdout, run.stderr);
        if args == ["check"] {
            // A check reports cannot-check: the cause names the line.
            assert!(
                said.contains(&format!("{CONFIG}:{line}"))
                    && said.contains("unknown field `workspaces`"),
                "{args:?}: {}",
                run.show()
            );
        } else if args == ["team"] {
            // The orchestrator's decision of 2026-11-09: `spec team` keeps
            // the `spec: ` before the line it has had since hub-accounts
            // (AC15); WS-8 means the line and the message, not the prefix.
            assert!(
                run.stderr.starts_with(&format!("spec: {opening}")),
                "{args:?}: `spec: {opening}...`: {}",
                run.show()
            );
        } else {
            assert!(
                run.stderr.starts_with(&opening),
                "{args:?}: `{opening}...`: {}",
                run.show()
            );
        }
    }
}

/// "Streams and exits": exit 2 on usage, with no project and on a config
/// `ProjectConfig` refuses (a slug off the grammar), stdout empty; whatever
/// the tables say, exit 0.
#[test]
fn the_command_exits_2_on_usage_no_project_and_a_refused_config() {
    let scratch = Scratch::new("ws-exits");
    let home = scratch.home("h");
    let root = scratch.copy("card-press", "press");
    for args in [
        &["workspace", "extra"][..],
        &["workspace", "--bogus"],
        &["workspace", "--json", "--limit", "3"],
    ] {
        let run = spec(&home, &root, args);
        run.code(2);
        assert_eq!(run.stdout, "", "{args:?}");
    }

    let empty = scratch.dir("empty");
    let run = spec(&home, &empty, &["workspace", "--json"]);
    run.code(2);
    assert_eq!(run.stdout, "");
    assert!(run.stderr.contains("spec init"), "{}", run.show());

    let config = read_text(&root, CONFIG);
    let bad = config.replacen("slug = \"card-press\"", "slug = \"Card Press\"", 1);
    write(&root, CONFIG, &bad);
    let run = spec(&home, &root, &["workspace", "--json"]);
    run.code(2);
    assert_eq!(run.stdout, "");
    let line = line_holding(&bad, "slug = ");
    assert!(
        run.stderr.starts_with(&format!("{CONFIG}:{line}: ")),
        "{}",
        run.show()
    );
}

/// "Rules and edge cases", `--config FILE`: every note names the file as
/// given, in JSON and on stderr.
#[test]
fn config_flag_names_the_file_as_given() {
    let scratch = Scratch::new("ws-config");
    let home = scratch.home("h");
    let root = scratch.copy("card-press", "press");
    let (_, changed) = variants(&workspaces_common::press_config())
        .into_iter()
        .find(|(label, _)| label.starts_with("(b)"))
        .expect("(b)");
    write(&root, "conf/press.toml", &changed);
    let run = spec(
        &home,
        &root,
        &["--config", "conf/press.toml", "workspace", "--json"],
    );
    run.code(0);
    let want = format!(
        "conf/press.toml:{}: workspace `newsroom`: no `[[card.flow]]` is named `inks`: left out",
        line_holding(&changed, "\"inks\"")
    );
    assert_eq!(
        notes_of(&run),
        std::slice::from_ref(&want),
        "{}",
        run.show()
    );
    assert_eq!(run.stderr, format!("note: {want}\n"));
}
