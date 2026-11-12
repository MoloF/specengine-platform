//! docs/features/departments.md (RM-0096, v5 step 4, ADR-0076) "Acceptance
//! criteria", the CLI half of the Rust lane: the `spec` binary over scratch
//! copies of `fixtures/card-press` (a print shop: `copy` titled "Copy desk",
//! `plates` untitled) and `fixtures/studio-lamp` (a lighthouse game: the
//! gamedev preset, two rooted modules and the direction `scenes`), each in
//! its own scratch `HOME`; the fixtures are only read.
//!
//! - DP-1: `spec card list --json`'s `flows[0]` keys are set 88 in order,
//!   `title` "Copy desk", `plates` titled by its name; the text output
//!   byte-equal to the build before ([`GOLDEN`]). M: `title` serialized
//!   after `disciplines`.
//! - DP-3: `spec workspace` on studio-lamp, its text and JSON as "Data"
//!   (`disciplines` JSON only, the direction's `kinds` line); `spec tree
//!   --kind scene --json`, the read a direction uses, lists `SCN-ARRIVAL` and
//!   `SCN-STORM` at depth 1, byte-equal to the build before.
//! - DP-7: studio-lamp's flows are titled as the preset titles them.
//!
//! [`GOLDEN`] was recorded once from the binary before the slice (5bec07e,
//! the slice's spec commit, built before any `src/` change of it) by
//! `record_the_answers_before_departments` (`#[ignore]`, by hand). The core
//! half is core's `tests/departments.rs`; the twins are http
//! `workspaces.rs`, `cards.rs` and MCP `mcp_cards.rs`; the key sets
//! `daemon_keys.rs`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod task_common;
mod workspaces_common;

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use common::{Run, SPEC, Scratch, fixture, program_with, read_text, write};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use task_common::Ordered;
use workspaces_common::{LAMP_JSON, LAMP_TEXT};

/// The golden of the reads before the slice, beside this file.
const GOLDEN: &str = "departments_before.json";

/// The binary the golden was recorded with.
const RECORDED_WITH: &str = "spec at 5bec07e (departments' spec commit, before any src change \
                             of the slice)";

/// Set 88 of `fixtures/daemon-keys.json`, `CardFlow`, in order ("Data").
const CARD_FLOW_KEYS: [&str; 7] = [
    "name",
    "title",
    "disciplines",
    "columns",
    "moves",
    "accept",
    "return",
];

/// card-press's `title` line ("Fixtures"); the recorder takes it out, so the
/// binary before, which refuses the key, reads the copy.
const PRESS_TITLE: &str = "title = \"Copy desk\"\n";

/// The reads of the golden, in order: the fixture, the args, what the copy
/// holds before them (cards made through the binary).
const PRESS_SEEDS: [&[&str]; 2] = [
    &[
        "card",
        "new",
        "--title",
        "Lead story",
        "--discipline",
        "copy",
    ],
    &[
        "card",
        "new",
        "--title",
        "Plate",
        "--discipline",
        "plates",
        "--parent",
        "C-0001",
        "--reviewer",
        "ben",
    ],
];

/// The press reads compared as text.
const PRESS_READS: [&[&str]; 3] = [
    &["card", "list"],
    &["card", "list", "--flow", "copy"],
    &["card", "list", "--parent", "C-0001"],
];

/// The lamp reads: the direction's read, both forms.
const LAMP_READS: [&[&str]; 2] = [
    &["tree", "--kind", "scene", "--json"],
    &["tree", "--kind", "scene"],
];

/// One recorded read: its fixture, args, exit code, stdout and stderr.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Row {
    fixture: String,
    args: Vec<String>,
    code: i32,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Golden {
    recorded_with: String,
    procedure: String,
    rows: Vec<Row>,
}

fn golden_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(GOLDEN)
}

/// `program args` in `cwd` under `home`, nothing else set.
fn run_with(program: &OsStr, home: &Path, cwd: &Path, args: &[&str]) -> Run {
    program_with(program, cwd, args, &[("HOME", home.as_os_str())])
}

/// Every read of the golden through `program`, each fixture on a fresh copy
/// with a fresh `HOME`.
fn play(program: &OsStr) -> Vec<Row> {
    let scratch = Scratch::new("dp-play");
    let mut rows = Vec::new();

    let press = scratch.copy("card-press", "press");
    // The binary before refuses `title`; the copy it reads is the fixture
    // as committed before the slice.
    if program != OsStr::new(SPEC) {
        let config = read_text(&press, "specengine.toml");
        write(
            &press,
            "specengine.toml",
            config.replacen(PRESS_TITLE, "", 1),
        );
    }
    let home = scratch.home("press");
    for args in PRESS_SEEDS {
        run_with(program, &home, &press, args).code(0);
    }
    for args in PRESS_READS {
        let done = run_with(program, &home, &press, args);
        rows.push(row("card-press", args, &done));
    }

    let lamp = scratch.copy("studio-lamp", "lamp");
    let home = scratch.home("lamp");
    for args in LAMP_READS {
        let done = run_with(program, &home, &lamp, args);
        rows.push(row("studio-lamp", args, &done));
    }
    rows
}

fn row(fixture: &str, args: &[&str], done: &Run) -> Row {
    Row {
        fixture: fixture.to_owned(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        code: done.code,
        stdout: done.stdout.clone(),
        stderr: done.stderr.clone(),
    }
}

fn golden() -> Golden {
    let text = fs::read_to_string(golden_path())
        .unwrap_or_else(|error| panic!("{GOLDEN}: {error}; record it first"));
    serde_json::from_str(&text).expect("the golden is JSON")
}

/// `spec args` in `root` under `home`: exit 0, stderr empty.
fn ok(home: &Path, root: &Path, args: &[&str]) -> Run {
    let done = run_with(OsStr::new(SPEC), home, root, args);
    done.code(0);
    assert_eq!(done.stderr, "", "{args:?}: {}", done.show());
    done
}

// ---------------------------------------------------------------- DP-1

/// DP-1: on a copy of card-press, `spec card list --json`'s flows each hold
/// set 88's keys in order, `copy` titled "Copy desk", `plates` by its name;
/// `card-press/expected.json`'s flows are the same (with `return` null). M:
/// `title` serialized after `disciplines`.
#[test]
fn dp1_list_serves_each_flow_s_title_after_its_name() {
    let scratch = Scratch::new("dp1-list");
    let home = scratch.home("h");
    let press = scratch.copy("card-press", "press");
    let done = ok(&home, &press, &["--json", "card", "list"]);
    let ordered = Ordered::of(&done.stdout);
    let flows = ordered.get("flows");
    for index in 0..2 {
        assert_eq!(
            flows.at(index).keys(),
            CARD_FLOW_KEYS,
            "flow {index}: set 88 in order"
        );
    }
    let document = done.json();
    assert_eq!(document["flows"][0]["name"], "copy");
    assert_eq!(document["flows"][0]["title"], "Copy desk");
    assert_eq!(document["flows"][1]["name"], "plates");
    assert_eq!(
        document["flows"][1]["title"], "plates",
        "untitled: the name"
    );
    // The fixture's `expected.json` gains `title` ("Fixtures").
    let expected: Value =
        serde_json::from_str(&read_text(&fixture("card-press"), "expected.json")).expect("JSON");
    let mut want = expected["flows"].clone();
    for flow in want.as_array_mut().expect("flows") {
        flow["return"] = Value::Null;
    }
    assert_eq!(document["flows"], want, "expected.json's flows");
    // The fixture's own line, as "Fixtures" gives it.
    let config = read_text(&fixture("card-press"), "specengine.toml");
    assert!(
        config.contains(&format!("name = \"copy\"\n{PRESS_TITLE}")),
        "card-press titles `copy` after its name"
    );
    assert_eq!(config.matches(PRESS_TITLE).count(), 1, "`plates` untitled");
}

/// DP-1 and DP-3, "the text is byte-equal" and "the read a direction uses,
/// unchanged": every read of [`GOLDEN`], played now on the fixtures as they
/// stand (card-press titling `copy`, studio-lamp holding the direction), is
/// byte for byte what the binary before printed. The cards are made through
/// the binary first; each fixture on a fresh copy and `HOME`.
#[test]
fn dp1_dp3_the_text_and_the_direction_s_read_are_the_build_before() {
    let golden = golden();
    assert_eq!(golden.recorded_with, RECORDED_WITH);
    let now = play(OsStr::new(SPEC));
    assert_eq!(now.len(), golden.rows.len(), "the same reads");
    for (now, before) in now.iter().zip(&golden.rows) {
        assert_eq!((&now.fixture, &now.args), (&before.fixture, &before.args));
        assert_eq!(now, before, "{} {:?}", now.fixture, now.args);
    }
    // What the golden holds is what the spec says of it.
    let tree = golden
        .rows
        .iter()
        .find(|row| row.args == ["tree", "--kind", "scene", "--json"])
        .expect("the direction's read");
    let nodes: Vec<(String, i64)> =
        serde_json::from_str::<Value>(&tree.stdout).expect("JSON")["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .map(|node| {
                (
                    node["id"].as_str().expect("an id").to_owned(),
                    node["depth"].as_i64().expect("a depth"),
                )
            })
            .collect();
    assert_eq!(
        nodes,
        [("SCN-ARRIVAL".to_owned(), 1), ("SCN-STORM".to_owned(), 1)]
    );
    let list = &golden.rows[0];
    assert_eq!(list.args, ["card", "list"]);
    assert!(
        list.stdout.starts_with("flow copy | draft 1 | "),
        "the text names the flow, never its title: {:?}",
        list.stdout
    );
}

// ---------------------------------------------------------------- DP-3

/// DP-3: studio-lamp's `spec workspace` text is "Data"'s lines, byte for
/// byte, and its `expected.json`'s; its JSON is "Data"'s document, every key
/// in order: each workspace's `disciplines` after `flows`, each module's
/// `root` and `kinds`, `null` where the other holds. No note.
#[test]
fn dp3_studio_lamp_s_workspaces_are_the_spec_s() {
    let scratch = Scratch::new("dp3-lamp");
    let home = scratch.home("h");
    let lamp = scratch.copy("studio-lamp", "lamp");
    let text = ok(&home, &lamp, &["workspace"]);
    assert_eq!(text.stdout, LAMP_TEXT, "{}", text.show());
    let expected: Value =
        serde_json::from_str(&read_text(&fixture("studio-lamp"), "expected.json")).expect("JSON");
    assert_eq!(expected["workspaces"], json!(LAMP_TEXT));
    let document = ok(&home, &lamp, &["workspace", "--json"]);
    assert_eq!(
        document.stdout,
        format!("{LAMP_JSON}\n"),
        "{}",
        document.show()
    );
    let ordered = Ordered::of(&document.stdout);
    // `default` last since docs/features/hub-department.md HD1.
    assert_eq!(
        ordered.keys(),
        ["source", "notes", "workspaces", "modules", "default"]
    );
    assert_eq!(
        ordered.get("workspaces").at(0).keys(),
        [
            "name",
            "title",
            "landing",
            "sections",
            "flows",
            "disciplines",
            "module"
        ]
    );
    assert_eq!(
        ordered.get("modules").at(2).keys(),
        ["name", "title", "root", "kinds"]
    );
    // A repeat is the same bytes; the command reads the config alone.
    assert_eq!(
        ok(&home, &lamp, &["workspace", "--json"]).stdout,
        document.stdout
    );
}

/// DP-3: the read a direction uses is today's `spec tree --kind`: on
/// studio-lamp, `scene`'s two nodes in served order at depth 1 with their
/// parent, in both forms; `chapter`'s one root at depth 0. No module or
/// workspace changes a tree read.
#[test]
fn dp3_a_direction_reads_the_kind_s_nodes() {
    let scratch = Scratch::new("dp3-tree");
    let home = scratch.home("h");
    let lamp = scratch.copy("studio-lamp", "lamp");
    let tree = ok(&home, &lamp, &["tree", "--kind", "scene", "--json"]).json();
    let rows: Vec<(String, i64, String)> = tree["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| {
            (
                node["id"].as_str().expect("an id").to_owned(),
                node["depth"].as_i64().expect("a depth"),
                node["parent"].as_str().unwrap_or("-").to_owned(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("SCN-ARRIVAL".to_owned(), 1, "CHP-SHORE".to_owned()),
            ("SCN-STORM".to_owned(), 1, "CHP-SHORE".to_owned()),
        ]
    );
    assert_eq!(tree["kinds"], json!(["scene"]));
    // The direction's tables removed, the tree is the same bytes.
    let with = ok(&home, &lamp, &["tree", "--json"]).stdout;
    let config = read_text(&lamp, "specengine.toml");
    let at = config
        .find("\n[[module]]\nname = \"scenes\"")
        .expect("the direction's table");
    write(&lamp, "specengine.toml", &config[..=at]);
    let without = ok(&home, &lamp, &["tree", "--json"]).stdout;
    assert!(with == without, "a direction changes no tree read");
}

// ---------------------------------------------------------------- DP-7

/// DP-7: studio-lamp holds the preset verbatim, so its `spec card list
/// --json` titles are the preset's, read here with the TOML reader, in
/// order.
#[test]
fn dp7_studio_lamp_s_flows_are_titled_as_the_preset() {
    let preset = fs::read_to_string(common::repository_root().join("scripts/presets/gamedev.toml"))
        .expect("the preset");
    let table: toml::Table = preset.parse().expect("TOML");
    let want: Vec<(String, String)> = table["card"]["flow"]
        .as_array()
        .expect("flows")
        .iter()
        .map(|flow| {
            (
                flow["name"].as_str().expect("a name").to_owned(),
                flow["title"].as_str().expect("a title").to_owned(),
            )
        })
        .collect();
    // docs/features/department-split.md "Acceptance criteria", S9: six.
    assert_eq!(want.len(), 6);
    let scratch = Scratch::new("dp7-lamp");
    let home = scratch.home("h");
    let lamp = scratch.copy("studio-lamp", "lamp");
    let document = ok(&home, &lamp, &["--json", "card", "list"]).json();
    let served: Vec<(String, String)> = document["flows"]
        .as_array()
        .expect("flows")
        .iter()
        .map(|flow| {
            (
                flow["name"].as_str().expect("a name").to_owned(),
                flow["title"].as_str().expect("a title").to_owned(),
            )
        })
        .collect();
    assert_eq!(served, want);
    // The text names flows, never their titles.
    let text = ok(&home, &lamp, &["card", "list"]).stdout;
    for (name, title) in &want {
        assert!(text.contains(&format!("flow {name} | ")), "{text}");
        assert!(!text.contains(title.as_str()), "{title}: {text}");
    }
}

// ------------------------------------------------------------ the golden

/// Writes [`GOLDEN`] from the binary `SPECENGINE_BEFORE_SPEC` names
/// (5bec07e's `spec`, built before any `src/` change of departments). Run
/// once, by hand: `SPECENGINE_BEFORE_SPEC=<binary> cargo nextest run -p
/// specengine-cli --test departments record_the_answers_before
/// --run-ignored ignored-only`. Every read is played twice and must answer
/// the same both times. It refuses when the golden exists.
#[test]
#[ignore = "manual: records departments' golden from 5bec07e's binary, once"]
fn record_the_answers_before_departments() {
    let program = std::env::var_os("SPECENGINE_BEFORE_SPEC").expect("SPECENGINE_BEFORE_SPEC");
    let path = golden_path();
    assert!(
        !path.exists(),
        "{GOLDEN} exists: it is recorded once, never again"
    );
    let rows = play(&program);
    assert_eq!(
        rows,
        play(&program),
        "the binary before answers alike twice"
    );
    for row in &rows {
        assert_eq!(row.code, 0, "{row:?}");
        assert_eq!(row.stderr, "", "{row:?}");
    }
    let golden = Golden {
        recorded_with: RECORDED_WITH.to_owned(),
        procedure: "card-press: a fresh copy less its `title` line, a fresh HOME, two cards made \
                    (`copy`, then a `plates` child of it), then three `card list` reads; \
                    studio-lamp: a fresh copy, a fresh HOME, `tree --kind scene` with --json and \
                    without; each read's exit code, stdout and stderr"
            .to_owned(),
        rows,
    };
    let text = serde_json::to_string_pretty(&golden).expect("JSON");
    fs::write(&path, format!("{text}\n")).expect("write the golden");
}
