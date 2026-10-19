//! docs/features/verify-tests.md (8a-1) RD-3, RD-11 and TE-4: which
//! `@verifies` bindings `spec verify --tests` lists and runs ("Selection",
//! "Runnable", "Verifiable", "The node set"), over scratch repositories of
//! `fixtures/verify-tests` (P10) and the runtime additions each test makes
//! and commits (a parts document, files of every "Runnable" reason). The
//! fake runner passes every test unless a test plans otherwise; consent is
//! `y` through the library. Each test names the mutation that turns it red
//! (`M:`).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;
mod lock_common;
mod verify_tests_common;

use std::collections::BTreeSet;

use serde_json::{Value, json};
use specengine_cli::LockRow;
use verify_tests_common::{
    COOLING, COOLING_TEST, GLAZE, GLAZE_TEST, HOLD, HOLD_TEST, KILN_MD, Kiln, RAMP_STOPS,
    RAMP_STOPS_TEST, SCHEDULE_MODULE, TWIN_NOT_UNIX, TWIN_TEST, TWIN_UNIX, args_of, owned,
};

/// The runtime parts document: one section per edge node.
const PARTS: [(&str, &str); 10] = [
    ("Vent", "KILN-VENT"),
    ("Door", "KILN-DOOR"),
    ("Flue", "KILN-FLUE"),
    ("Shelf", "KILN-SHELF"),
    ("Post", "KILN-POST"),
    ("Wash", "KILN-WASH"),
    ("Lid", "KILN-LID"),
    ("Plug", "KILN-PLUG"),
    ("Deep", "KILN-DEEP"),
    ("Coil", "KILN-COIL"),
];

/// `docs/spec/parts.md` holding [`PARTS`] (written, not committed).
fn write_parts(kiln: &Kiln) {
    let mut text = String::from(
        "---\nclass: canon\nid: KILN-PARTS\nowner: owner\nreviewed: 2026-10-16\n---\n\n# Parts\n\nThe kiln's parts, each with a check of its own.\n",
    );
    for (title, id) in PARTS {
        text.push_str(&format!(
            "\n## {title} {{#{id}}}\n\nThe {} works through a whole firing.\n",
            title.to_lowercase()
        ));
    }
    kiln.repo.write("docs/spec/parts.md", text);
}

/// An error-free function whose item hash is null: its use runs nest one
/// past `MAX_USE_RUN_NESTING` (`nesting_too_deep`, docs/canon/code-hash.md;
/// `specengine-code` `tests/hash_depth.rs`), the walk binding it as a
/// plain item.
fn deep_source() -> String {
    let levels = specengine_code::hash::MAX_USE_RUN_NESTING;
    format!(
        "// @verifies KILN-DEEP\npub fn deep_check() {{\n{}use x; use y;\n{}}}\n",
        "#[a = {\n".repeat(levels),
        "0 }] use x; use y;\n".repeat(levels)
    )
}

/// The files of every "Runnable" reason but `control character` (TE-5's),
/// committed; `crates/kiln/src/scratch.rs` ignored, never committed.
fn add_edges(kiln: &Kiln) {
    write_parts(kiln);
    kiln.repo
        .write(".gitignore", "crates/kiln/src/scratch.rs\n");
    kiln.repo.write(
        "crates/kiln/src/edges.rs",
        "//! @verifies KILN-DOOR\n//! Parts a firing touches.\n\n/// The lid's weight.\n// @verifies KILN-LID\npub const LID_WEIGHT: u32 = 12;\n\n/// The kiln wash.\npub struct Wash;\n\nimpl Wash {\n    // @verifies KILN-WASH\n    pub fn check(&self) -> bool {\n        true\n    }\n}\n",
    );
    kiln.repo.write(
        "crates/kiln/data/kiln.ron",
        "// Kiln limits.\nKiln(\n    peak: 1200, // @verifies KILN-FLUE\n)\n",
    );
    kiln.repo.write(
        "crates/kiln/misc/loose.rs",
        "// @verifies KILN-SHELF\npub fn loose_check() {}\n",
    );
    kiln.repo.write(
        "crates/kiln/tests/common/mod.rs",
        "// @verifies KILN-POST\npub fn shared_check() {}\n",
    );
    kiln.repo.write(
        "crates/nameless/Cargo.toml",
        "[package]\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n",
    );
    kiln.repo.write(
        "crates/nameless/src/lib.rs",
        "// @verifies KILN-PLUG\npub fn nameless_check() {}\n",
    );
    kiln.repo.write("crates/kiln/src/deep.rs", deep_source());
    kiln.repo.commit_all("the parts and their checks");
    kiln.repo.write(
        "crates/kiln/src/scratch.rs",
        "// @verifies KILN-VENT\npub fn scratch_check() {}\n",
    );
}

/// The `{test}` (last argument) of every test invocation.
fn tests_run(kiln: &Kiln) -> Vec<String> {
    kiln.test_invocations()
        .into_iter()
        .map(|args| args.last().cloned().unwrap_or_default())
        .collect()
}

/// The symbols of the JSON rows, in order.
fn symbols(rows: &[Value]) -> Vec<String> {
    rows.iter()
        .map(|row| row["symbol"].as_str().expect("a symbol").to_owned())
        .collect()
}

/// The IDs of the JSON node rows, in order.
fn ids(nodes: &[Value]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| node["id"].as_str().expect("an ID").to_owned())
        .collect()
}

// -------------------------------------------------------------------- RD-3

/// RD-3 ("Runnable"): a module `@verifies` marker (`//!` in
/// `tests/schedule.rs`) is listed `not run | … | module`, never run, and
/// does not keep its node from `verified`. M: module run.
#[test]
fn module_verifies_marker_is_not_run() {
    let kiln = Kiln::new("vt-module");
    let run = kiln.run();
    run.code(1);
    let row = run.row(SCHEDULE_MODULE);
    assert_eq!(
        (&row["kind"], &row["outcome"], &row["reason"], &row["nodes"]),
        (
            &json!("module"),
            &json!("not_run"),
            &json!("module"),
            &json!(["KILN-HOLD"])
        ),
        "{row}"
    );
    run.has_line(
        "not run | crates/kiln/tests/schedule.rs | module crates/kiln::test:schedule | KILN-HOLD | module",
    );
    let ran: BTreeSet<String> = tests_run(&kiln).into_iter().collect();
    assert_eq!(
        ran,
        owned(&[TWIN_TEST, RAMP_STOPS_TEST, GLAZE_TEST, HOLD_TEST])
            .into_iter()
            .collect(),
        "only the four item tests run"
    );
    assert_eq!(run.node("KILN-HOLD")["verdict"], "verified");
}

/// RD-3 ("Runnable", "Verifiable"): the test of `KILN-COOL` (unverifiable:
/// its `@implements KILN-COOL@1` is `spec_ahead`, a refusal 5b-1 keeps) is
/// `not run | … | node not verifiable`, never started; the node reads `not
/// verifiable spec_ahead` before the question and after the run. A test
/// bound to it and to a verifiable node runs ("no node of the set it binds
/// is verifiable" is the rule). M: an unverifiable node's test run.
#[test]
fn tests_of_an_unverifiable_node_are_not_run() {
    let kiln = Kiln::new("vt-unverifiable");
    let run = kiln.run();
    run.code(1);
    let row = run.row(COOLING);
    assert_eq!(
        (&row["outcome"], &row["reason"]),
        (&json!("not_run"), &json!("node not verifiable")),
        "{row}"
    );
    assert_eq!(
        run.node("KILN-COOL"),
        json!({"id": "KILN-COOL", "verdict": "not_verifiable", "state": "spec_ahead"})
    );
    run.has_line("node KILN-COOL: not verifiable spec_ahead");
    assert!(
        run.listing()
            .contains(&"node KILN-COOL: not verifiable spec_ahead".to_owned()),
        "the listing names it before the question: {:?}",
        run.listing()
    );
    assert!(
        !tests_run(&kiln).contains(&COOLING_TEST.to_owned()),
        "{:?}",
        kiln.log()
    );

    kiln.repo.write(
        "crates/kiln/src/mixed.rs",
        "#[cfg(test)]\nmod tests {\n    // @verifies KILN-COOL@2\n    // @verifies KILN-RAMP\n    #[test]\n    fn cooling_meets_the_ramp() {}\n}\n",
    );
    kiln.repo
        .commit_all("a test of two nodes, one unverifiable");
    let run = kiln.run();
    let row = run.row("crates/kiln::mixed::tests::cooling_meets_the_ramp");
    assert_eq!(row["nodes"], json!(["KILN-COOL", "KILN-RAMP"]), "{row}");
    assert_eq!(row["outcome"], "passed", "{row}");
    assert_eq!(
        tests_run(&kiln)
            .iter()
            .filter(|test| *test == "mixed::tests::cooling_meets_the_ramp")
            .count(),
        1
    );
    assert!(!tests_run(&kiln).contains(&COOLING_TEST.to_owned()));
    assert_eq!(run.node("KILN-COOL")["verdict"], "not_verifiable");
}

/// RD-3 ("Selection", "Rules": equal argv run once): `hold_follows_the_ramp`
/// is bound to `KILN-HOLD` and `KILN-RAMP`: one row naming both, one run;
/// it fails, so both nodes read `failed` (`KILN-RAMP`'s other test passes).
/// M: run twice (once per node).
#[test]
fn a_test_of_two_nodes_runs_once() {
    let kiln = Kiln::new("vt-two-nodes");
    kiln.plan(&[(HOLD_TEST, "fail")]);
    let run = kiln.run();
    run.code(1);
    assert_eq!(
        tests_run(&kiln)
            .iter()
            .filter(|test| *test == HOLD_TEST)
            .count(),
        1,
        "{:?}",
        kiln.log()
    );
    let row = run.row(HOLD);
    assert_eq!(
        (&row["nodes"], &row["outcome"], &row["exit"], &row["signal"]),
        (
            &json!(["KILN-HOLD", "KILN-RAMP"]),
            &json!("failed"),
            &json!(101),
            &Value::Null
        ),
        "{row}"
    );
    run.has_line(
        "failed | crates/kiln/tests/schedule.rs | item crates/kiln::test:schedule::hold_follows_the_ramp | KILN-HOLD, KILN-RAMP | exit 101",
    );
    assert_eq!(run.node("KILN-HOLD")["verdict"], "failed");
    assert_eq!(run.node("KILN-RAMP")["verdict"], "failed");
    assert_eq!(run.row(RAMP_STOPS)["outcome"], "passed");
    assert_eq!(run.verdict(), "unverified");
}

/// RD-3 (P5), TE-4: the `cfg` twins `peak_is_capped #cfg(test,unix)` and
/// `#cfg(test,not(unix))` give one argv (`{test}` from `QPath` module
/// segments and name, never the discriminator): one run, both rows take
/// its outcome; no argument holds ` #`. One twin's marker is an alias
/// (`PYRO-01` -> `CONE-01`): both rows name `CONE-01`. M: ` #cfg(…)` left
/// in `{test}`.
#[test]
fn cfg_twin_runs_its_undiscriminated_name() {
    let kiln = Kiln::new("vt-twins");
    kiln.plan(&[(TWIN_TEST, "fail")]);
    let run = kiln.run();
    run.code(1);
    let calls = kiln.test_invocations();
    assert_eq!(
        calls
            .iter()
            .filter(|args| args.last().is_some_and(|last| last == TWIN_TEST))
            .count(),
        1,
        "{calls:?}"
    );
    assert!(
        kiln.log()
            .iter()
            .all(|call| call.args.iter().all(|arg| !arg.contains(" #"))),
        "a discriminator reached the runner: {:?}",
        kiln.log()
    );
    for twin in [TWIN_UNIX, TWIN_NOT_UNIX] {
        let row = run.row(twin);
        assert_eq!(
            (&row["nodes"], &row["outcome"], &row["exit"]),
            (&json!(["CONE-01"]), &json!("failed"), &json!(101)),
            "{row}"
        );
        assert_eq!(
            args_of(&row),
            owned(&["kiln-core", "lib", "kiln_core", TWIN_TEST]),
            "{row}"
        );
    }
    assert_eq!(run.node("CONE-01")["verdict"], "failed");
    assert!(
        run.question()
            .starts_with("run 4 bound tests of 4 nodes as "),
        "four distinct argv: {}",
        run.question()
    );
}

/// RD-3 (P4): a test in a file `.gitignore` names (the walk reads it, the
/// clean check does not see it) is `not run | … | not committed`, never
/// run; its node reads `not verifiable not committed` (accept's "tracked
/// and clean"), so the node's committed test is `node not verifiable`;
/// `hold_follows_the_ramp` still runs for `KILN-HOLD`. M: an ignored
/// file's test run.
#[test]
fn ignored_file_test_is_not_run() {
    let kiln = Kiln::new("vt-ignored");
    kiln.repo
        .write(".gitignore", "crates/kiln/src/scratch.rs\n");
    kiln.repo.commit_all("ignore the scratch file");
    kiln.repo.write(
        "crates/kiln/src/scratch.rs",
        "// @verifies KILN-RAMP\npub fn scratch_check() {}\n",
    );
    let run = kiln.run();
    run.code(1);
    let row = run.row("crates/kiln::scratch::scratch_check");
    assert_eq!(
        (&row["outcome"], &row["reason"], &row["nodes"]),
        (
            &json!("not_run"),
            &json!("not committed"),
            &json!(["KILN-RAMP"])
        ),
        "{row}"
    );
    assert_eq!(
        run.node("KILN-RAMP"),
        json!({"id": "KILN-RAMP", "verdict": "not_verifiable", "state": "not committed"})
    );
    assert_eq!(run.row(RAMP_STOPS)["reason"], "node not verifiable");
    let ran = tests_run(&kiln);
    assert!(
        !ran.iter()
            .any(|test| test.contains("scratch_check") || test == RAMP_STOPS_TEST),
        "{ran:?}"
    );
    assert!(ran.contains(&HOLD_TEST.to_owned()), "{ran:?}");
}

/// RD-3 (P7): a node is verifiable exactly when `spec lock accept` would
/// lock every binding of it: for every node reported, over the fixture and
/// the runtime parts, the library accept (consent `n`) shows no `not
/// locked` row when `--tests` calls the node verifiable, and exactly the
/// one whose state `--tests` names when not. `KILN-DEEP`'s test is an
/// error-free function whose item hash is null (`nesting_too_deep`): the
/// walk binds it as a plain item, only accept's hashing refuses it, so it
/// is `not verifiable cannot_verify nesting_too_deep`, never run. M: a
/// second predicate passing a null-hash item.
#[test]
fn verifiable_is_accepts_predicate() {
    let kiln = Kiln::new("vt-predicate");
    add_edges(&kiln);
    let run = kiln.run();
    let deep = run.node("KILN-DEEP");
    assert_eq!(
        deep,
        json!({"id": "KILN-DEEP", "verdict": "not_verifiable", "state": "cannot_verify nesting_too_deep"})
    );
    assert_eq!(
        run.row("crates/kiln::deep::deep_check")["reason"],
        "node not verifiable"
    );
    assert!(
        !tests_run(&kiln)
            .iter()
            .any(|test| test.contains("deep_check")),
        "{:?}",
        kiln.log()
    );

    let nodes = run.nodes();
    assert!(nodes.len() >= 12, "{nodes:#?}");
    for node in nodes {
        let id = node["id"].as_str().expect("an ID");
        let attempt = kiln.repo.accept(id, false);
        let outcome = attempt
            .outcome
            .as_ref()
            .unwrap_or_else(|error| panic!("accept {id}: exit 2: {error}"));
        let refused: Vec<&LockRow> = outcome
            .rows
            .iter()
            .filter(|row| row.change == "not locked")
            .collect();
        match node["verdict"].as_str() {
            Some("not_verifiable") => {
                let state = node["state"].as_str().expect("a state");
                if state == "not committed" {
                    assert!(
                        outcome
                            .refusal
                            .as_deref()
                            .is_some_and(|reason| reason.contains("crates/kiln/src/scratch.rs")),
                        "{id}: accept refuses the uncommitted marker file: {outcome:?}"
                    );
                } else {
                    assert_eq!(refused.len(), 1, "{id}: one refusal: {:#?}", outcome.rows);
                    assert_eq!(
                        refused[0].state.as_deref(),
                        Some(state),
                        "{id}: the predicate's own words: {:#?}",
                        outcome.rows
                    );
                }
            }
            _ => {
                assert!(
                    refused.is_empty() && !outcome.rows.is_empty(),
                    "{id}: --tests calls it verifiable; accept: {:#?}",
                    outcome.rows
                );
                assert_eq!(
                    attempt.prompts.len(),
                    1,
                    "{id}: accept would lock it: {outcome:?}"
                );
            }
        }
    }
    assert!(kiln.repo.lock().is_none(), "no lock written");
}

// -------------------------------------------------------------------- TE-4

/// TE-4 ("Runnable", "Order" step 7): a case of every reason but `control
/// character` (TE-5), each listed `not run | <file> | <kind> <symbol> |
/// <node> | <reason>` (the first reason that applies) and never started;
/// the nodes whose only bindings are lockable read `no runnable test`, the
/// unrooted one `not verifiable ambiguous unrooted`, the ignored one `not
/// verifiable not committed`. M: any reason dropped or reordered (the item
/// run, or another reason named).
#[test]
fn every_runnable_reason_is_named() {
    let kiln = Kiln::new("vt-reasons");
    add_edges(&kiln);
    let run = kiln.run();
    run.code(1);
    let cases: [(&str, &str, &str, &str, &str); 9] = [
        (
            "crates/kiln/src/scratch.rs",
            "item",
            "crates/kiln::scratch::scratch_check",
            "KILN-VENT",
            "not committed",
        ),
        (
            "crates/kiln/src/edges.rs",
            "module",
            "crates/kiln::edges",
            "KILN-DOOR",
            "module",
        ),
        (
            "crates/kiln/data/kiln.ron",
            "ron",
            "root.peak",
            "KILN-FLUE",
            "ron",
        ),
        (
            "crates/kiln/misc/loose.rs",
            "item",
            "crates/kiln::loose_check",
            "KILN-SHELF",
            "unrooted",
        ),
        (
            "crates/kiln/tests/common/mod.rs",
            "item",
            "crates/kiln::shared:tests/common::shared_check",
            "KILN-POST",
            "shared unit",
        ),
        (
            "crates/kiln/src/edges.rs",
            "item",
            "crates/kiln::edges::Wash::check",
            "KILN-WASH",
            "member",
        ),
        (
            "crates/kiln/src/edges.rs",
            "item",
            "crates/kiln::edges::LID_WEIGHT",
            "KILN-LID",
            "not a function",
        ),
        (
            "crates/nameless/src/lib.rs",
            "item",
            "crates/nameless::nameless_check",
            "KILN-PLUG",
            "no value {package}",
        ),
        (
            "crates/kiln/src/deep.rs",
            "item",
            "crates/kiln::deep::deep_check",
            "KILN-DEEP",
            "node not verifiable",
        ),
    ];
    for (file, kind, symbol, node, reason) in cases {
        let row = run.row(symbol);
        assert_eq!(
            (
                &row["file"],
                &row["kind"],
                &row["nodes"],
                &row["outcome"],
                &row["reason"]
            ),
            (
                &json!(file),
                &json!(kind),
                &json!([node]),
                &json!("not_run"),
                &json!(reason)
            ),
            "{symbol}: {row}"
        );
        run.has_line(&format!(
            "not run | {file} | {kind} {symbol} | {node} | {reason}"
        ));
    }
    for node in [
        "KILN-DOOR",
        "KILN-FLUE",
        "KILN-POST",
        "KILN-WASH",
        "KILN-LID",
        "KILN-PLUG",
    ] {
        assert_eq!(
            run.node(node),
            json!({"id": node, "verdict": "no_runnable_test", "state": null}),
            "{node}"
        );
        run.has_line(&format!("node {node}: no runnable test"));
    }
    assert_eq!(
        run.node("KILN-SHELF"),
        json!({"id": "KILN-SHELF", "verdict": "not_verifiable", "state": "ambiguous unrooted"})
    );
    assert_eq!(
        run.node("KILN-VENT"),
        json!({"id": "KILN-VENT", "verdict": "not_verifiable", "state": "not committed"})
    );
    let ran: BTreeSet<String> = tests_run(&kiln).into_iter().collect();
    assert_eq!(
        ran,
        owned(&[TWIN_TEST, RAMP_STOPS_TEST, GLAZE_TEST, HOLD_TEST])
            .into_iter()
            .collect(),
        "only the fixture's runnable tests start"
    );
    // The listing before the question gives the same reasons.
    let listing = run.listing();
    for (file, kind, symbol, node, reason) in cases {
        let line = format!("not run | {file} | {kind} {symbol} | {node} | {reason}");
        assert!(listing.contains(&line), "{line}\n{listing:#?}");
    }
}

// ------------------------------------------------------------------- RD-11

/// RD-11 (P6), TE-4: `--task T` reads the task's columns now, in any state:
/// a `draft` task (no snapshot) selects its target `KILN-RAMP` and the
/// tests bound to it (each row naming only its set nodes), `verified`,
/// exit 0; once planned, its criteria refs (the feature-scoped
/// `glaze-check/GLZ-01`; a free-text criterion selects nothing) and
/// affected nodes (`CONE-01`) join. M: the snapshot read (a `draft` task
/// selects nothing).
#[test]
fn task_set_reads_the_columns_now() {
    let kiln = Kiln::new("vt-task-columns");
    let task = kiln.new_task(&["KILN-RAMP"]);
    let run = kiln.run_task(&task);
    run.code(0);
    let doc = run.json();
    assert_eq!(
        (&doc["task"], &doc["verdict"]),
        (&json!(task), &json!("verified"))
    );
    assert_eq!(ids(&run.nodes()), owned(&["KILN-RAMP"]));
    assert_eq!(symbols(&run.rows()), owned(&[RAMP_STOPS, HOLD]));
    assert_eq!(run.row(HOLD)["nodes"], json!(["KILN-RAMP"]));
    assert_eq!(tests_run(&kiln), owned(&[RAMP_STOPS_TEST, HOLD_TEST]));
    assert!(
        run.summary()
            .starts_with(&format!("verify --tests --task {task}: verified: ")),
        "{}",
        run.summary()
    );

    kiln.plan_task(
        &task,
        &["glaze-check/GLZ-01", "The glaze survives a whole firing."],
        &["CONE-01"],
    );
    let run = kiln.run_task(&task);
    run.code(0);
    assert_eq!(
        ids(&run.nodes()),
        owned(&["CONE-01", "KILN-RAMP", "glaze-check/GLZ-01"])
    );
    assert_eq!(
        symbols(&run.rows()),
        owned(&[TWIN_NOT_UNIX, TWIN_UNIX, RAMP_STOPS, GLAZE, HOLD])
    );
    assert_eq!(run.row(GLAZE)["nodes"], json!(["glaze-check/GLZ-01"]));
    assert_eq!(
        tests_run(&kiln),
        owned(&[TWIN_TEST, RAMP_STOPS_TEST, GLAZE_TEST, HOLD_TEST])
    );
}

/// RD-11 (P2): `--task` adds every section a set node contains: a task on
/// the document `KILN-FIRING` selects `KILN-RAMP`, `KILN-HOLD`,
/// `KILN-COOL` and their tests (not the twins, not the glaze), and notes
/// the one node of it without a `@verifies` binding. M: containment
/// dropped.
#[test]
fn task_selection_includes_contained_sections() {
    let kiln = Kiln::new("vt-task-sections");
    let task = kiln.new_task(&["KILN-FIRING"]);
    let run = kiln.run_task(&task);
    run.code(1);
    assert_eq!(
        ids(&run.nodes()),
        owned(&["KILN-COOL", "KILN-HOLD", "KILN-RAMP"])
    );
    assert_eq!(
        symbols(&run.rows()),
        owned(&[COOLING, RAMP_STOPS, SCHEDULE_MODULE, HOLD])
    );
    assert_eq!(run.row(HOLD)["nodes"], json!(["KILN-HOLD", "KILN-RAMP"]));
    let note = format!("verify --tests: 1 nodes of {task} have no @verifies binding");
    assert!(run.notes().contains(&note), "{:?}", run.notes());
    assert!(
        run.stderr_lines().contains(&format!("note: {note}")),
        "{:?}",
        run.stderr_lines()
    );
    assert_eq!(tests_run(&kiln), owned(&[RAMP_STOPS_TEST, HOLD_TEST]));
}

/// RD-11 (P2, "A row in a set", marker half): a row follows `spec refs`'
/// listing: a homoglyph marker (`KILN-H\u{041E}LD`, Cyrillic O) is listed
/// under its fix's node `KILN-HOLD`, an alias marker (`PYRO-01`) under
/// `CONE-01`; the homoglyph makes `KILN-HOLD` `not verifiable homoglyph`,
/// so its tests are not run. M: a homoglyph marker dropped from the rows
/// (the lock-entry half: not reachable through `--tests`, uncovered here).
#[test]
fn node_set_rows_follow_refs_listing() {
    let kiln = Kiln::new("vt-rows-refs");
    kiln.repo.write(
        "crates/kiln/src/look.rs",
        "#[cfg(test)]\nmod tests {\n    // @verifies KILN-H\u{041E}LD\n    #[test]\n    fn look_alike_check() {}\n}\n",
    );
    kiln.repo.commit_all("a look-alike marker");
    let task = kiln.new_task(&["KILN-HOLD", "CONE-01"]);
    let run = kiln.run_task(&task);
    run.code(1);
    let look = run.row("crates/kiln::look::tests::look_alike_check");
    assert_eq!(
        (&look["nodes"], &look["outcome"], &look["reason"]),
        (
            &json!(["KILN-HOLD"]),
            &json!("not_run"),
            &json!("node not verifiable")
        ),
        "{look}"
    );
    assert_eq!(
        run.node("KILN-HOLD"),
        json!({"id": "KILN-HOLD", "verdict": "not_verifiable", "state": "homoglyph"})
    );
    for twin in [TWIN_UNIX, TWIN_NOT_UNIX] {
        let row = run.row(twin);
        assert_eq!(
            (&row["nodes"], &row["outcome"]),
            (&json!(["CONE-01"]), &json!("passed")),
            "{row}"
        );
    }
    assert_eq!(run.node("CONE-01")["verdict"], "verified");
    assert_eq!(
        symbols(&run.rows()),
        owned(&[
            "crates/kiln::look::tests::look_alike_check",
            TWIN_NOT_UNIX,
            TWIN_UNIX,
            SCHEDULE_MODULE,
            HOLD,
        ])
    );
    assert_eq!(tests_run(&kiln), owned(&[TWIN_TEST]));
}

/// RD-11 (P6, S13): `--task T` with `T` unknown, a look-alike of a task ID,
/// another repository's task or an ID of `[ids]` exits 2, stdout empty,
/// with the texts of `docs/canon/tasks.md` "Commands"; no question, nothing
/// run, no event.
/// M: an unknown or another repository's task at exit 1.
#[test]
fn verify_tests_task_unknown_is_exit_2() {
    let kiln = Kiln::new("vt-task-unknown");
    let refused = |task: &str, words: &str| {
        let run = kiln.run_task(task);
        let message = run.failed(task);
        assert!(message.contains(words), "{task}: {message}");
        assert!(run.prompts.is_empty(), "{task}: asked {:?}", run.prompts);
        kiln.assert_nothing_started(task);
    };
    refused("T-0099", "no task T-0099 in this repository");
    refused("\u{0422}-0001", "T-0001");

    let other = kiln.repo.scratch.join("other");
    common::copy_dir(&kiln.repo.root, &other);
    let other = std::fs::canonicalize(&other).expect("the other repository");
    let theirs = kiln.new_task_in(&other, &["KILN-RAMP"]);
    refused(&theirs, "belongs to another repository");

    let config = kiln.repo.read("specengine.toml");
    kiln.repo.write(
        "specengine.toml",
        common::with_ids_line(&config, "T = { kind = \"trial-run\", width = 4 }\n"),
    );
    kiln.repo.commit_all("a T prefix");
    let run = kiln.run_task("T-0001");
    run.failed("T in [ids]");
    assert!(run.prompts.is_empty());
    kiln.assert_nothing_started("T in [ids]");
    assert!(kiln.events().is_empty(), "no event: {:?}", kiln.events());
}

/// "The node set": a task ref naming nothing now (the section renamed and
/// committed after the task was made) is a note, `verify: <ref> names
/// nothing now`, and skipped; the rest of the set runs. M: the ref fails
/// the run, or is read from the snapshot.
#[test]
fn task_ref_naming_nothing_is_a_note() {
    let kiln = Kiln::new("vt-task-gone");
    let task = kiln.new_task(&["KILN-HOLD", "KILN-RAMP"]);
    kiln.repo.replace(KILN_MD, "{#KILN-HOLD}", "{#KILN-DWELL}");
    kiln.repo.commit_all("the hold is a dwell now");
    let run = kiln.run_task(&task);
    run.code(0);
    let note = "verify: KILN-HOLD names nothing now".to_owned();
    assert!(run.notes().contains(&note), "{:?}", run.notes());
    assert!(
        run.stderr_lines().contains(&format!("note: {note}")),
        "{:?}",
        run.stderr_lines()
    );
    assert_eq!(ids(&run.nodes()), owned(&["KILN-RAMP"]));
    assert_eq!(tests_run(&kiln), owned(&[RAMP_STOPS_TEST, HOLD_TEST]));
}
