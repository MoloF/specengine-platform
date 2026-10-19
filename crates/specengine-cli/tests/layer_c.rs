//! docs/features/layer-c-resolve.md (11a-2) "Acceptance criteria": `spec
//! resolve`, the caller's side of protocol v1
//! (docs/canon/layer-c-protocol.md), against fake `specengine-ra`
//! executables (TE-1):
//!
//! - R1: the binary beside `spec` before `PATH`; none: the install line.
//! - R2: every ending but `done` and exit 0 keeps the snapshot; the canon's
//!   variants are not malformed; a protocol mismatch; each check; the cap.
//! - R3: the group killed (a grandchild ignoring `SIGTERM`), a forwarded
//!   signal.
//! - R4: nothing stored before `done`; a tree changed during the run; the
//!   lock.
//! - R6: the snapshot through `spec index --full`, an `exclude` change, a
//!   re-derived code index, no `[code]` (its freshness:
//!   `layer_c_state.rs`).
//! - R7: an index format change drops the snapshot (the tables recreated:
//!   `code_index.rs`).
//! - R8: the golden's pinned counts, text and JSON; the join on bytes and
//!   kinds, never lines or names; CRLF; a second project.
//! - R9: byte-identical rows across runs and roots.
//! - R14: the request; R15: stdin held to the last line, the budget over
//!   an unread request.
//! - TE-2 (ignored): the real binary, built, on a scratch ra-mini.
//!
//! R5 and R12 over a locked game repository: `layer_c_outputs.rs`; the
//! source scans of R5 and R10: `layer_c_scan.rs`.
//!
//! Every run is a copy of `spec` with a fake beside it, under a scratch
//! `HOME` (`layer_c_common`); the fixtures are only read.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod code_index_common;
mod common;
mod layer_c_common;

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::os::unix::process::ExitStatusExt as _;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};

use code_index_common::{count, execute, query};
use common::{copy_file, read_text, write};
use layer_c_common::{
    CONFIG, End, FILES, GOLDEN_TEXT, GRACE, INSTALL, Item, JSON_KEYS, KEPT, Lab, RA_BIN,
    REQUEST_KEYS, RUN_LIMIT, Reply, SLACK, STATES, Step, WORKSPACE_KEYS, assert_refused,
    assert_refused_before_spawn, assert_signal_ending, counts, crlf_offset, error_line,
    golden_head, golden_lines, golden_text, keys_of, last_line, mask_timing, refused_as,
    refused_line, reply_of, sorted, swap_in, top_keys, wait_until, write_executable,
};

/// A fake's plan, as a case holds it.
type Plan = &'static [(&'static str, &'static str)];

/// A fake's plan, owned.
type PlanEntries = Vec<(&'static str, &'static str)>;

/// The lab's config without its `[code]` table.
fn config_without_code() -> String {
    let head = CONFIG
        .split("\n[code]\n")
        .next()
        .expect("the config before [code]");
    format!("{head}\n")
}

/// [`CONFIG`] with its `[code] roots` replaced by `roots`.
fn config_with_roots(roots: &str) -> String {
    swap_in(
        CONFIG,
        "roots = [\"arena\", \"arena-derive\", \"tools\"]",
        &format!("roots = {roots}"),
    )
}

/// The request a start read, as JSON.
fn request_json(lab: &Lab, pid: i32) -> Value {
    let line = lab
        .fake
        .request_of(pid)
        .unwrap_or_else(|| panic!("the fake {pid} read no request"));
    serde_json::from_str(line.trim_end_matches('\n'))
        .unwrap_or_else(|error| panic!("the request is JSON ({error}): {line:?}"))
}

// ------------------------------------------------------------ the tools

/// TE-1's base: the reply builder renders the golden byte for byte, and the
/// timing mask leaves everything but a number before a unit.
#[test]
fn golden_parses_and_renders_byte_identical() {
    let golden = Reply::golden();
    assert_eq!(
        golden.render(),
        golden_text(),
        "the builder renders the golden as it is"
    );
    let paths: Vec<&str> = golden.files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(
        paths, FILES,
        "the golden answers the six files in request order"
    );
    assert_eq!(golden.item_count(), 29);
    let example = "workspace 1 .: discover 235 ms (toolchain 1.98.1, sysroot 26, degraded no), database 313 ms\njoined 29, unmatched 0, c_only 0; agrees 26, disagrees 0; collisions 1; stored fresh, 20.5 s, peak 0.5 GiB\n";
    assert_eq!(
        mask_timing(example),
        "workspace 1 .: discover <n> ms (toolchain 1.98.1, sysroot 26, degraded no), database <n> ms\njoined 29, unmatched 0, c_only 0; agrees 26, disagrees 0; collisions 1; stored fresh, <n> s, peak <n> GiB\n"
    );
}

// ------------------------------------------------------------------- R1

/// R1. M: `PATH` first. A fake beside the copy of `spec` and one on
/// `PATH` (after a directory named `specengine-ra` and a non-executable
/// file of that name): the one beside answers, the one on `PATH` never
/// starts; with none beside, the first executable regular file on `PATH`
/// answers the same request.
#[test]
fn layer_c_binary_beside_spec_wins_over_path() {
    let mut lab = Lab::ra_mini("lcr-beside");
    let on_path = lab.extra_fake("on-path");
    let dir_decoy = lab.base.join("decoy-dir");
    fs::create_dir_all(dir_decoy.join(RA_BIN)).expect("a directory named like the binary");
    let file_decoy = lab.base.join("decoy-file");
    fs::create_dir_all(&file_decoy).expect("the decoy's directory");
    fs::write(file_decoy.join(RA_BIN), "#!/bin/sh\nexit 0\n").expect("a decoy");
    fs::set_permissions(file_decoy.join(RA_BIN), fs::Permissions::from_mode(0o644))
        .expect("a decoy that is not executable");
    let on_path_dir = on_path
        .program
        .parent()
        .expect("the fake's directory")
        .to_path_buf();
    let mut path = vec![lab.path[0].clone(), dir_decoy, file_decoy, on_path_dir];
    path.extend(lab.path[1..].iter().cloned());
    lab.path = path;

    let run = lab.resolve();
    run.code(0);
    let beside = lab.fake.only();
    assert!(
        on_path.invocations().is_empty(),
        "the binary on PATH never starts while one is beside spec: {:?}",
        on_path.invocations()
    );
    let request = lab.fake.request_of(beside.pid).expect("the request beside");

    fs::remove_file(&lab.fake.program).expect("remove the binary beside spec");
    let run = lab.resolve();
    run.code(0);
    let found = on_path.only();
    assert_eq!(
        on_path.request_of(found.pid).as_deref(),
        Some(request.as_str()),
        "the binary on PATH gets the same request"
    );
}

/// R1. M: exit 0 without a binary. No `specengine-ra` beside the copy or on
/// `PATH`: exit 2, stdout empty, the install line naming the copy, text
/// and JSON.
#[test]
fn resolve_without_binary_is_exit_2_with_install_line() {
    let lab = Lab::ra_mini("lcr-missing");
    fs::remove_file(&lab.fake.program).expect("no binary beside spec");
    let reason = format!(
        "specengine-ra not found beside {} or on PATH; install it: {INSTALL}",
        lab.spec.display()
    );
    for args in [&["resolve"][..], &["--json", "resolve"][..]] {
        let run = lab.run(args);
        assert_refused_before_spawn(&run, &reason);
    }
}

/// Iteration 2 (review, minor): a relative `PATH` entry is taken against
/// `spec`'s current directory, and the program is made absolute before the
/// spawn (whose cwd is `D/tmp`). No binary beside the copy; `PATH` is
/// `bin-rel` (a directory of the cwd holding a fake), then the system's;
/// `spec --root <root> resolve` from that cwd: the fake answers, exit 0.
/// M: the relative program spawned as it is (`cannot be started`).
#[test]
fn relative_path_entry_finds_the_binary_from_the_cwd() {
    let mut lab = Lab::ra_mini("lcr-relative");
    fs::remove_file(&lab.fake.program).expect("no binary beside spec");
    let relative = lab.extra_fake("bin-rel");
    lab.path = vec![
        PathBuf::from("bin-rel"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ];
    let root = lab.root.to_str().expect("a UTF-8 root").to_owned();
    let run = lab
        .start_in(&lab.base, &lab.home, &["--root", &root, "resolve"])
        .wait(RUN_LIMIT)
        .run();
    run.code(0);
    assert!(run.stdout.contains("stored fresh"), "{}", run.show());
    let started = relative.only();
    assert_eq!(started.args, ["--protocol", "1"], "{started:?}");
}

/// Step 1 and the usage: `--timeout` outside 1..=3600 or not a whole
/// number, no `[code]`, no Rust file to resolve, a walk gap: exit 2,
/// stdout empty, the binary never started; the bounds themselves taken.
#[test]
fn resolve_refuses_before_the_spawn() {
    let lab = Lab::ra_mini("lcr-pre-spawn");
    for timeout in ["0", "3601", "-1", "x", "1.5"] {
        let run = lab.run(&["resolve", "--timeout", timeout]);
        run.code(2);
        assert_eq!(run.stdout, "", "--timeout {timeout}:\n{}", run.show());
    }

    write(&lab.root, "specengine.toml", config_without_code());
    let run = lab.resolve();
    run.code(2);
    assert_eq!(run.stdout, "", "{}", run.show());
    assert!(run.stderr.contains("[code]"), "no [code]:\n{}", run.show());

    write(
        &lab.root,
        "specengine.toml",
        config_with_roots("[\"data\"]"),
    );
    write(&lab.root, "data/tuning.ron", "(regen: 3)\n");
    let run = lab.resolve();
    assert_refused_before_spawn(&run, "no Rust file to resolve");

    write(&lab.root, "specengine.toml", CONFIG);
    let locked = lab.root.join("arena/src");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("chmod 000");
    let run = lab.resolve();
    fs::set_permissions(locked, fs::Permissions::from_mode(0o755)).expect("chmod back");
    run.code(2);
    assert_eq!(run.stdout, "", "{}", run.show());
    assert!(
        run.stderr.contains("the code walk is incomplete"),
        "a walk gap:\n{}",
        run.show()
    );
    assert!(
        lab.fake.invocations().is_empty(),
        "nothing before the spawn starts the binary: {:?}",
        lab.fake.invocations()
    );

    for (timeout, budget_ms) in [("3600", 3_600_000), ("1", 1000)] {
        lab.fake.clear();
        lab.run(&["resolve", "--timeout", timeout]).code(0);
        let started = lab.fake.only();
        assert_eq!(
            request_json(&lab, started.pid)["budget_ms"],
            budget_ms,
            "--timeout {timeout}"
        );
    }
}

// ------------------------------------------------------------------- R2

/// R2. M: `error`, `refused` or exit 3 taken as success. After a stored
/// golden, one case per row of "Output and exits" but the first (and a
/// last line with another exit, twice): exit 2, the row's reason, stdout
/// empty, the layer C dump byte-identical.
#[test]
fn every_ending_but_done_and_exit_0_keeps_the_snapshot() {
    let lab = Lab::ra_mini("lcr-endings");
    let stored = lab.store_golden();
    let head = golden_head(3);
    let cases: Vec<(&str, PlanEntries, String, String)> = vec![
        (
            "error in workspace 1",
            vec![("exit", "1")],
            format!("{head}{}\n", error_line(1, "database")),
            "load failed: database (workspace 1)".to_owned(),
        ),
        (
            "error under no manifest",
            vec![("exit", "1")],
            format!("{head}{}\n", error_line(0, "path")),
            "load failed: path (files under no manifest)".to_owned(),
        ),
        (
            "refused, protocol 1",
            vec![("exit", "2")],
            format!(
                "{}\n",
                refused_line(1, "files: arena/src/gone.rs is not a regular file")
            ),
            "specengine-ra refused the request: files: arena/src/gone.rs is not a regular file"
                .to_owned(),
        ),
        (
            "exit 3, no line",
            vec![("exit", "3")],
            String::new(),
            "specengine-ra ended (exit 3) before done".to_owned(),
        ),
        (
            "done, then exit 1",
            vec![("exit", "1")],
            golden_text(),
            "specengine-ra ended (exit 1) after done".to_owned(),
        ),
        (
            "error, then exit 0",
            vec![("exit", "0")],
            format!("{head}{}\n", error_line(1, "metadata")),
            "specengine-ra ended (exit 0) before done".to_owned(),
        ),
        (
            "refused, then exit 1",
            vec![("exit", "1")],
            format!("{}\n", refused_line(1, "budget_ms: not a positive integer")),
            "specengine-ra ended (exit 1) before done".to_owned(),
        ),
    ];
    for (name, plan, reply, reason) in &cases {
        for args in [&["resolve"][..], &["--json", "resolve"][..]] {
            lab.fake.clear();
            lab.fake.script(plan, reply);
            let run = lab.run(args);
            refused_as(name, &run, reason);
            lab.fake.only();
            assert_eq!(lab.layer_c(), stored, "{name}: the snapshot is kept");
        }
    }

    lab.fake.clear();
    lab.fake.script(&[("after", "kill")], &head);
    let run = lab.resolve();
    assert_signal_ending(&run, false);
    assert_eq!(lab.layer_c(), stored, "a signal: the snapshot is kept");

    write_executable(
        &lab.fake.program,
        b"#!/nonexistent/specengine-test-interpreter\n",
    );
    let run = lab.resolve();
    run.code(2);
    assert_eq!(run.stdout, "", "{}", run.show());
    let last = last_line(&run);
    assert!(
        last.starts_with("spec resolve: specengine-ra cannot be started: ") && last.ends_with(KEPT),
        "a binary that cannot start:\n{}",
        run.show()
    );
    assert_eq!(
        lab.layer_c(),
        stored,
        "a failed spawn: the snapshot is kept"
    );
    lab.fake.restore();
    assert!(lab.fake.survivors(GRACE).is_empty());
}

/// R2. M: a check stricter than the canon. Protocol v1's variants: a
/// workspace `dir` of `..` with a null `toolchain` and a null `peak_rss`
/// (stored; `toolchain unknown`, `peak unknown`; JSON nulls); workspaces
/// whose dirs are neither unique nor sorted; no workspace at all (every
/// file parsed alone); `error` N with no step N; `error` 0 right after
/// `hello` (`load failed`, never malformed).
#[test]
fn protocol_v1_variants_are_not_malformed() {
    let lab = Lab::ra_mini("lcr-variants");
    lab.index();

    let mut above = Reply::golden();
    above.steps[0].dir = "..".to_owned();
    above.steps[0].toolchain = None;
    above.peak_rss = None;
    lab.fake.script(&[], &above.render());
    let run = lab.resolve();
    run.code(0);
    let text = mask_timing(&run.stdout);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines.get(1).copied(),
        Some(
            "workspace 1 ..: discover <n> ms (toolchain unknown, sysroot 0, degraded no), database <n> ms"
        ),
        "{}",
        run.show()
    );
    assert!(
        lines
            .last()
            .is_some_and(|line| line.ends_with("peak unknown")),
        "{}",
        run.show()
    );
    assert!(
        lab.layer_c().contains("code_resolve\t"),
        "a `..` workspace is stored like any run"
    );
    lab.fake.clear();
    let run = lab.resolve_json();
    run.code(0);
    let doc = run.json();
    assert_eq!(doc["workspaces"][0]["dir"], "..", "{doc}");
    assert!(doc["workspaces"][0]["toolchain"].is_null(), "{doc}");
    assert!(doc["peak_rss"].is_null(), "{doc}");

    let mut several = Reply::golden();
    several.steps = vec![
        Step {
            degraded: true,
            toolchain: Some("1.98.1".to_owned()),
            ..Step::at(1, "arena")
        },
        Step {
            toolchain: None,
            ..Step::at(2, ".")
        },
        Step {
            toolchain: Some("1.97.0".to_owned()),
            ..Step::at(3, "arena")
        },
    ];
    lab.fake.clear();
    lab.fake.script(&[], &several.render());
    let run = lab.resolve_json();
    run.code(0);
    let doc = run.json();
    let dirs: Vec<&str> = doc["workspaces"]
        .as_array()
        .expect("workspaces")
        .iter()
        .map(|workspace| workspace["dir"].as_str().expect("a dir"))
        .collect();
    assert_eq!(dirs, ["arena", ".", "arena"], "dirs as given: {doc}");
    assert_eq!(doc["workspaces"][0]["metadata_degraded"], true, "{doc}");

    let mut alone = Reply::golden();
    alone.steps.clear();
    lab.fake.clear();
    lab.fake.script(&[], &alone.render());
    lab.resolve().code(0);

    let errors = [
        (
            "error 2 with no step 2",
            Reply {
                steps: vec![Step::at(1, ".")],
                end: End::Error {
                    workspace: 2,
                    category: "metadata".to_owned(),
                },
                ..Reply::golden()
            },
            "load failed: metadata (workspace 2)",
        ),
        (
            "error 0 right after hello",
            Reply {
                steps: Vec::new(),
                end: End::Error {
                    workspace: 0,
                    category: "path".to_owned(),
                },
                ..Reply::golden()
            },
            "load failed: path (files under no manifest)",
        ),
    ];
    let kept = lab.layer_c();
    for (name, reply, reason) in &errors {
        lab.fake.clear();
        lab.fake.script(&[("exit", "1")], &reply.render());
        let run = lab.resolve();
        refused_as(name, &run, reason);
        assert_eq!(lab.layer_c(), kept, "{name}: the snapshot is kept");
    }

    // Deviation 3 (accepted): an `error` after `file` lines ends the run as
    // any `error` does, the workspace bound kept.
    let mut after_files = golden_lines()[..6].to_vec();
    after_files.push(error_line(1, "database"));
    lab.fake.clear();
    lab.fake.script(&[("exit", "1")], &reply_of(&after_files));
    let run = lab.resolve();
    refused_as(
        "error after file lines",
        &run,
        "load failed: database (workspace 1)",
    );
    assert_eq!(lab.layer_c(), kept, "error after file lines: kept");

    // Deviation 5 (accepted): an item's `end` is not checked against its
    // `start`.
    let mut backwards = Reply::golden();
    backwards.item_mut("arena/src/lib.rs", 278).end = 100;
    lab.fake.clear();
    lab.fake.script(&[], &backwards.render());
    lab.resolve().code(0);
}

/// R2. M: no version check, or the keys checked before `protocol`. A
/// `hello` or `refused` whose `protocol` is not 1, with keys of another
/// set, missing or extra: exit 2, the mismatch reason, never `malformed`;
/// the escalation at once (the fake left hanging), nothing stored.
#[test]
fn protocol_mismatch_is_exit_2_and_stores_nothing() {
    let lab = Lab::ra_mini("lcr-mismatch");
    let stored = lab.store_golden();
    let rest: Vec<String> = golden_lines()[1..].to_vec();
    let with_hello = |hello: &str| {
        let mut lines = vec![hello.to_owned()];
        lines.extend(rest.iter().cloned());
        reply_of(&lines)
    };
    let hang: Plan = &[("after", "hang")];
    let refuse: Plan = &[("exit", "2")];
    let mut misplaced = golden_lines();
    misplaced.insert(
        3,
        r#"{"t":"hello","protocol":2,"ra_ap":"0.0.352","host":"masked"}"#.to_owned(),
    );
    let cases: [(&str, Plan, String, u64); 6] = [
        (
            "hello, protocol 2",
            hang,
            with_hello(r#"{"t":"hello","protocol":2,"ra_ap":"0.0.352","host":"masked"}"#),
            2,
        ),
        (
            "hello, protocol 2, another key set",
            hang,
            with_hello(
                r#"{"t":"hello","protocol":2,"ra_ap":"0.0.352","host":"masked","features":["moniker"]}"#,
            ),
            2,
        ),
        (
            "hello, protocol 0, keys missing",
            hang,
            with_hello(r#"{"t":"hello","protocol":0}"#),
            0,
        ),
        (
            "refused, protocol 2, an extra key",
            refuse,
            "{\"t\":\"refused\",\"protocol\":2,\"reason\":\"unknown protocol\",\"accepts\":[2]}\n"
                .to_owned(),
            2,
        ),
        (
            "refused, protocol 3, no reason",
            refuse,
            "{\"t\":\"refused\",\"protocol\":3}\n".to_owned(),
            3,
        ),
        (
            "a hello, protocol 2, after the steps (deviation 4)",
            hang,
            reply_of(&misplaced),
            2,
        ),
    ];
    for (name, plan, reply, protocol) in &cases {
        lab.fake.clear();
        lab.fake.script(plan, reply);
        let ended = lab.start(&["resolve", "--timeout", "120"]).wait(RUN_LIMIT);
        refused_as(
            name,
            &ended.run(),
            &format!(
                "specengine-ra speaks protocol {protocol}, spec needs 1: install both from one commit"
            ),
        );
        assert!(
            ended.elapsed < GRACE + SLACK,
            "{name}: the escalation starts at once, not at the budget: {:?}",
            ended.elapsed
        );
        assert!(
            lab.fake.survivors(GRACE).is_empty(),
            "{name}: the fake is gone"
        );
        assert_eq!(lab.layer_c(), stored, "{name}: nothing stored");
    }
}

/// R2. M: any one check off. Each case breaks one rule of "Checks" in the
/// golden (the rest left valid): exit 2, `malformed reply at line <n>` at
/// the line that breaks it, nothing stored.
#[test]
fn malformed_reply_is_exit_2_and_stores_nothing() {
    let lab = Lab::ra_mini("lcr-malformed");
    let stored = lab.store_golden();
    let g = golden_lines();
    assert_eq!(g.len(), 10, "hello, two steps, six files, done");
    let edit = |at: usize, from: &str, to: &str| -> Vec<String> {
        let mut lines = g.clone();
        lines[at] = swap_in(&lines[at], from, to);
        lines
    };
    let without = |at: usize| -> Vec<String> {
        let mut lines = g.clone();
        lines.remove(at);
        lines
    };
    let swapped = |a: usize, b: usize| -> Vec<String> {
        let mut lines = g.clone();
        lines.swap(a, b);
        lines
    };
    let replaced = |at: usize, line: &str| -> Vec<String> {
        let mut lines = g.clone();
        lines[at] = line.to_owned();
        lines
    };
    let inserted = |at: usize, line: &str| -> Vec<String> {
        let mut lines = g.clone();
        lines.insert(at, line.to_owned());
        lines
    };
    let mut cases: Vec<(&str, Vec<String>, &str, usize)> = vec![
        ("no hello first", g[1..].to_vec(), "0", 1),
        (
            "hello with an extra key",
            edit(
                0,
                "\"host\":\"masked\"}",
                "\"host\":\"masked\",\"extra\":1}",
            ),
            "0",
            1,
        ),
        (
            "hello without host",
            edit(0, ",\"host\":\"masked\"", ""),
            "0",
            1,
        ),
        (
            "a line that is not JSON",
            replaced(1, "{\"t\":\"step\","),
            "0",
            2,
        ),
        (
            "an unknown line type",
            inserted(1, "{\"t\":\"progress\",\"percent\":5}"),
            "0",
            2,
        ),
        (
            "workspaces not numbered from 1",
            {
                let mut lines = g.clone();
                for at in [1, 2] {
                    lines[at] = swap_in(&lines[at], "\"workspace\":1", "\"workspace\":2");
                }
                lines
            },
            "0",
            2,
        ),
        ("database before discover", swapped(1, 2), "0", 2),
        (
            "database of another dir",
            edit(2, "\"dir\":\".\"", "\"dir\":\"arena\""),
            "0",
            3,
        ),
        (
            "discover without metadata_degraded",
            edit(1, ",\"metadata_degraded\":false", ""),
            "0",
            2,
        ),
        (
            "toolchain a number",
            edit(1, "\"toolchain\":\"masked\"", "\"toolchain\":7"),
            "0",
            2,
        ),
        (
            "a step with an extra key",
            edit(2, "\"ms\":0}", "\"ms\":0,\"files\":6}"),
            "0",
            3,
        ),
        ("a lone discover before the files", without(2), "0", 3),
        ("files out of request order", swapped(3, 4), "0", 4),
        ("a requested file missing", without(8), "0", 9),
        (
            "a file not requested",
            edit(6, "\"arena/src/stray.rs\"", "\"arena/src/strays.rs\""),
            "0",
            7,
        ),
        (
            "a file line without in_crate",
            edit(6, "\"in_crate\":false,", ""),
            "0",
            7,
        ),
        (
            "an unknown kind",
            edit(3, "\"kind\":\"function\"", "\"kind\":\"closure\""),
            "0",
            4,
        ),
        (
            "an unknown state",
            edit(3, "\"state\":\"moniker\"", "\"state\":\"maybe\""),
            "0",
            4,
        ),
        (
            "a moniker on an unresolved item",
            edit(
                6,
                "\"moniker\":null",
                "\"moniker\":\"arena stray_helper().\"",
            ),
            "0",
            7,
        ),
        (
            "a moniker item without its moniker",
            edit(
                4,
                "\"moniker\":\"build_script_build main().\"",
                "\"moniker\":null",
            ),
            "0",
            5,
        ),
        (
            "an item with an extra key",
            edit(8, "\"moniker\":null}", "\"moniker\":null,\"line\":5}"),
            "0",
            9,
        ),
        (
            "done with a wrong item count",
            edit(9, "\"items\":29", "\"items\":28"),
            "0",
            10,
        ),
        (
            "done without not_loaded",
            edit(9, ",\"not_loaded\":1", ""),
            "0",
            10,
        ),
        (
            "done with peak_rss a string",
            edit(9, "\"peak_rss\":0", "\"peak_rss\":\"0\""),
            "0",
            10,
        ),
        (
            "a line after done",
            {
                let mut lines = g.clone();
                lines.push(g[9].clone());
                lines
            },
            "0",
            11,
        ),
        (
            "an error past the last step and one",
            vec![
                g[0].clone(),
                g[1].clone(),
                g[2].clone(),
                error_line(3, "metadata"),
            ],
            "1",
            4,
        ),
        (
            "a line after refused",
            vec![refused_line(1, "files: not byte-sorted"), g[0].clone()],
            "2",
            2,
        ),
        (
            "refused, protocol 1, an extra key",
            vec!["{\"t\":\"refused\",\"protocol\":1,\"reason\":\"x\",\"extra\":1}".to_owned()],
            "2",
            1,
        ),
        (
            "hello with protocol a string",
            edit(0, "\"protocol\":1", "\"protocol\":\"1\""),
            "0",
            1,
        ),
        (
            "hello with protocol a float",
            edit(0, "\"protocol\":1", "\"protocol\":1.0"),
            "0",
            1,
        ),
        (
            "an error of category build_scripts (never written)",
            vec![
                g[0].clone(),
                g[1].clone(),
                g[2].clone(),
                error_line(1, "build_scripts"),
            ],
            "1",
            4,
        ),
    ];
    let mut unordered = Reply::golden();
    unordered.file_mut("arena/src/lib.rs").items.swap(0, 1);
    cases.push(("items not by start", unordered.lines(), "0", 6));
    let mut doubled = Reply::golden();
    let items = &mut doubled.file_mut("arena/src/lib.rs").items;
    items[1].start = items[0].start;
    cases.push(("two items at one start", doubled.lines(), "0", 6));

    for (name, lines, exit, n) in &cases {
        lab.fake.clear();
        lab.fake.script(&[("exit", *exit)], &reply_of(lines));
        let run = lab.resolve();
        refused_as(name, &run, &format!("malformed reply at line {n}"));
        assert_eq!(lab.layer_c(), stored, "{name}: nothing stored");
    }
    assert!(lab.fake.survivors(GRACE).is_empty());
}

/// `REPLY_LINE_CAP`: the bytes of a reply line before its end.
const REPLY_LINE_CAP: usize = 8_388_608;

/// The golden with its `discover` line `REPLY_LINE_CAP + extra` bytes long
/// before its end (its `dir` padded; the `database` line stays shorter).
fn discover_line_of(extra: usize) -> String {
    let mut reply = Reply::golden();
    let base = reply.lines()[1].len();
    reply.steps[0].dir = "a".repeat(REPLY_LINE_CAP + extra + 1 - base);
    let lines = reply.lines();
    assert_eq!(lines[1].len(), REPLY_LINE_CAP + extra, "the padded line");
    assert!(
        lines[2].len() < REPLY_LINE_CAP,
        "the database line is shorter"
    );
    reply_of(&lines)
}

/// R2. M: no cap. The review's boundary: a line of exactly
/// `REPLY_LINE_CAP` bytes before its end is taken; one byte more is
/// `malformed reply at line 2`, valid JSON otherwise; a line that never
/// ends (9 MiB, no line end) is the same whether the fake then hangs or
/// exits 0 (the fault fires before an EOF could cut the line), before the
/// budget. A fault stores nothing; the line at the cap is taken, its
/// `dir` printed as given (the rows themselves hold no `dir`).
#[test]
fn oversized_reply_line_is_exit_2() {
    let lab = Lab::ra_mini("lcr-oversized");
    let stored = lab.store_golden();

    lab.fake.clear();
    lab.fake.script(&[], &discover_line_of(1));
    let run = lab.run(&["resolve", "--timeout", "120"]);
    refused_as("one byte over the cap", &run, "malformed reply at line 2");
    assert_eq!(lab.layer_c(), stored, "nothing stored");

    let flood = (9 * 1024 * 1024).to_string();
    for (name, after) in [
        ("an endless line, hanging", "hang"),
        ("an endless line, then exit 0", "exit"),
    ] {
        lab.fake.clear();
        lab.fake.script(
            &[
                ("cut", "1"),
                ("flood", flood.as_str()),
                ("after", after),
                ("exit", "0"),
            ],
            &golden_text(),
        );
        let ended = lab.start(&["resolve", "--timeout", "120"]).wait(RUN_LIMIT);
        refused_as(name, &ended.run(), "malformed reply at line 2");
        assert!(
            ended.elapsed < Duration::from_secs(60),
            "{name}: the cap ends it, not the budget: {:?}",
            ended.elapsed
        );
        assert!(
            lab.fake.survivors(GRACE).is_empty(),
            "{name}: the fake is gone"
        );
        assert_eq!(lab.layer_c(), stored, "{name}: nothing stored");
    }

    lab.fake.clear();
    lab.fake.script(&[], &discover_line_of(0));
    let run = lab.run(&["--json", "resolve", "--timeout", "120"]);
    run.code(0);
    let doc = run.json();
    assert_eq!(
        doc["workspaces"][0]["dir"].as_str().map(str::len),
        Some(REPLY_LINE_CAP + 1 - Reply::golden().lines()[1].len()),
        "a line of exactly the cap is taken, its `dir` as given"
    );
    assert_eq!(lab.layer_c(), stored, "the same rows: `dir` is not stored");
}

// ------------------------------------------------------------------- R3

/// R3. M: only the child killed. The fake ignores `SIGTERM`, forks a
/// grandchild that ignores it too, writes three lines and hangs;
/// `--timeout 2`: exit 2 `over the budget of 2 s` once the grace has
/// passed, and no process of the group survives `spec`.
#[test]
fn no_layer_c_grandchild_outlives_the_budget() {
    let lab = Lab::ra_mini("lcr-grandchild");
    lab.index();
    lab.fake.script(
        &[
            ("grandchild", "1"),
            ("term", "ignore"),
            ("cut", "3"),
            ("after", "hang"),
        ],
        &golden_text(),
    );
    let ended = lab.start(&["resolve", "--timeout", "2"]).wait(RUN_LIMIT);
    assert_eq!(
        lab.fake.grandchildren().len(),
        1,
        "the fake forked its grandchild"
    );
    let survivors = lab.fake.survivors(Duration::from_secs(3));
    assert!(survivors.is_empty(), "{survivors:?} outlived spec");
    assert_refused(&ended.run(), "over the budget of 2 s");
    let budget = Duration::from_secs(2);
    assert!(
        ended.elapsed >= budget && ended.elapsed < budget + GRACE + SLACK,
        "the budget, the grace, then SIGKILL: {:?}",
        ended.elapsed
    );
    assert_eq!(lab.layer_c(), "", "nothing stored");
}

/// R3, L6. M: the default `SIGINT` disposition. A run with the fake
/// hanging after three lines; `SIGINT` (the fake and a grandchild ignoring
/// `SIGTERM`), `SIGTERM`, `SIGHUP` to `spec`: `spec` exits 2 `interrupted`,
/// not dying of the signal; the group is gone; the snapshot is kept.
#[test]
fn interrupted_resolve_kills_the_group_and_keeps_the_snapshot() {
    let lab = Lab::ra_mini("lcr-interrupt");
    let stored = lab.store_golden();
    for (signal, name, grandchild) in [
        (libc::SIGINT, "SIGINT", true),
        (libc::SIGTERM, "SIGTERM", false),
        (libc::SIGHUP, "SIGHUP", false),
    ] {
        lab.fake.clear();
        let mut plan = vec![("cut", "3"), ("after", "hang")];
        if grandchild {
            plan.push(("grandchild", "1"));
            plan.push(("term", "ignore"));
        }
        lab.fake.script(&plan, &golden_text());
        let started = lab.start(&["resolve", "--timeout", "120"]);
        wait_until(
            &format!("{name}: the fake wrote its lines"),
            Duration::from_secs(60),
            || lab.fake.wrote(),
        );
        started.signal(signal);
        let ended = started.wait(RUN_LIMIT);
        assert_eq!(
            ended.status.signal(),
            None,
            "{name}: spec forwards the signal, it does not die of it:\n{}",
            ended.show()
        );
        refused_as(name, &ended.run(), "interrupted");
        assert!(
            ended.elapsed < Duration::from_secs(60),
            "{name}: ended by the signal, not the budget: {:?}",
            ended.elapsed
        );
        if grandchild {
            assert_eq!(lab.fake.grandchildren().len(), 1, "{name}: a grandchild");
        }
        let survivors = lab.fake.survivors(Duration::from_secs(3));
        assert!(survivors.is_empty(), "{name}: {survivors:?} outlived spec");
        assert_eq!(lab.layer_c(), stored, "{name}: the snapshot is kept");
    }
}

// ------------------------------------------------------------------- R4

/// R4. M: rows written while streaming. After a stored golden, a reply
/// with other monikers stops after three `file` lines: hanging past
/// `--timeout 2` (`over the budget of 2 s`), or killed mid-stream (`ended
/// (signal …) before done`): the layer C dump unchanged. The same reply,
/// complete and slow, does replace it (the control).
#[test]
fn budget_overrun_keeps_the_previous_snapshot() {
    let lab = Lab::ra_mini("lcr-overrun");
    let stored = lab.store_golden();
    let mut other = Reply::golden();
    for file in &mut other.files {
        file.in_crate = !file.in_crate;
        for item in &mut file.items {
            item.moniker = item
                .moniker
                .as_ref()
                .map(|moniker| format!("other {moniker}"));
        }
    }
    let partial = other.render();

    lab.fake.clear();
    lab.fake
        .script(&[("cut", "6"), ("after", "hang")], &partial);
    let ended = lab.start(&["resolve", "--timeout", "2"]).wait(RUN_LIMIT);
    assert_refused(&ended.run(), "over the budget of 2 s");
    assert_eq!(lab.layer_c(), stored, "an overrun keeps the snapshot");

    lab.fake.clear();
    lab.fake
        .script(&[("cut", "6"), ("after", "kill")], &partial);
    let run = lab.resolve();
    assert_signal_ending(&run, false);
    assert_eq!(
        lab.layer_c(),
        stored,
        "a crash mid-stream keeps the snapshot"
    );

    lab.fake.clear();
    lab.fake.script(&[("line_delay_ms", "50")], &partial);
    lab.resolve().code(0);
    assert_ne!(
        lab.layer_c(),
        stored,
        "the other reply, complete, replaces the snapshot"
    );
    assert!(lab.fake.survivors(GRACE).is_empty());
}

/// R4. M: no re-hash. The fake edits a requested file of the tree before
/// its `done`: exit 2 `the tree changed during the run`, nothing stored;
/// the edited tree, resolved again, stores.
#[test]
fn tree_change_during_run_stores_nothing() {
    let lab = Lab::ra_mini("lcr-tree-change");
    let stored = lab.store_golden();
    let edited = lab.root.join("arena/src/stray.rs");
    lab.fake.clear();
    lab.fake.script(
        &[("touch", edited.to_str().expect("a UTF-8 path"))],
        &golden_text(),
    );
    let run = lab.resolve();
    assert!(lab.fake.touched(), "the fake edited the tree before done");
    assert_refused(&run, "the tree changed during the run");
    assert_eq!(lab.layer_c(), stored, "nothing stored");

    let again = lab.store_golden();
    assert_ne!(
        again, stored,
        "the edited file's hash is the new snapshot's"
    );
}

/// R4. M: no lock. While one run waits at its fake's gate, a second
/// `spec resolve` of the project exits 2 `a layer C run of ra-mini is in
/// progress` and starts no binary; the first then stores; a third runs.
#[test]
fn second_resolve_is_refused_while_one_runs() {
    let lab = Lab::ra_mini("lcr-lock");
    lab.index();
    let gate = lab.base.join("gate");
    lab.fake.script(
        &[("gate", gate.to_str().expect("a UTF-8 path"))],
        &golden_text(),
    );
    let first = lab.start(&["resolve", "--timeout", "120"]);
    wait_until(
        "the first run's fake at its gate",
        Duration::from_secs(60),
        || lab.fake.started(),
    );
    let second = lab.resolve();
    assert_refused_before_spawn(&second, "a layer C run of ra-mini is in progress");
    assert_eq!(
        lab.fake.invocations().len(),
        1,
        "the second run started no binary"
    );
    fs::write(gate, "").expect("open the gate");
    let first = first.wait(RUN_LIMIT).run();
    first.code(0);
    assert!(first.stdout.contains("stored fresh"), "{}", first.show());
    lab.store_golden();
}

// ------------------------------------------------------------------- R6

/// R6 (A3). M: `--full` drops. `spec index --full`, an `exclude` change and
/// back, and a code index re-derived whole (`code_worktrees.code_fp` of an
/// older build, as a `CODE_INDEX_FORMAT` change leaves it; PHASE-B: the
/// fingerprint the refresh compares) each keep the layer C dump byte for
/// byte.
#[test]
fn full_reindex_keeps_the_snapshot() {
    let lab = Lab::ra_mini("lcr-full");
    let stored = lab.store_golden();
    lab.run(&["index", "--full"]).code(0);
    assert_eq!(
        lab.layer_c(),
        stored,
        "`spec index --full` keeps the snapshot"
    );

    write(
        &lab.root,
        "specengine.toml",
        format!("{CONFIG}exclude = [\"tools/**\"]\n"),
    );
    lab.index();
    assert_eq!(
        lab.layer_c(),
        stored,
        "an exclude change keeps the snapshot"
    );
    write(&lab.root, "specengine.toml", CONFIG);
    lab.index();
    assert_eq!(lab.layer_c(), stored, "the exclude gone again");

    execute(
        &lab.db(),
        "UPDATE code_worktrees SET code_fp = 'of-an-older-build'",
    );
    lab.index();
    assert_eq!(
        lab.layer_c(),
        stored,
        "a code index re-derived whole keeps the snapshot"
    );
}

/// R6 (A3). M: no cascade. `[code]` removed: `spec index` clears the code
/// rows (`clear_code`) and the snapshot with them; `[code]` back, the
/// snapshot stays gone.
#[test]
fn clear_code_drops_the_snapshot() {
    let lab = Lab::ra_mini("lcr-clear");
    lab.store_golden();
    write(&lab.root, "specengine.toml", config_without_code());
    lab.index();
    assert_eq!(lab.layer_c(), "", "no [code]: the snapshot is gone");
    write(&lab.root, "specengine.toml", CONFIG);
    lab.index();
    assert_eq!(
        lab.layer_c(),
        "",
        "the code index back, the snapshot stays gone"
    );
}

// ------------------------------------------------------------------- R7

/// R7 (A3). M: the snapshot tables outside `FORMAT_TABLES`. The stored
/// index format rewritten as an older build left it: `spec index` drops
/// the snapshot; a run stores one again.
#[test]
fn format_change_drops_the_snapshot() {
    let lab = Lab::ra_mini("lcr-format");
    lab.store_golden();
    execute(
        &lab.db(),
        "UPDATE index_meta SET value = '0' WHERE key = 'format'",
    );
    lab.index();
    assert_eq!(
        lab.layer_c(),
        "",
        "an INDEX_FORMAT change drops the snapshot"
    );
    lab.store_golden();
}

// ------------------------------------------------------------------- R8

/// R8. The golden replayed: the spec's four lines (timings masked), the
/// binary's stderr copied to `spec`'s; JSON with exactly the spec's keys
/// in its order and the pinned counts (joined 29, unmatched 0, c_only 0,
/// agrees 26, disagrees 0, collisions 1: `arena impl#[Health]` at 341 and
/// 453); the stored rows one per file and item. M: a `mod` row's own
/// `name/` compared (L10), or no `-` -> `_` (`build_script_build`).
#[test]
fn golden_replay_gives_the_pinned_counts() {
    let lab = Lab::ra_mini("lcr-golden");
    lab.fake
        .script(&[("stderr", "LCR-FAKE-STDERR-SENTINEL")], &golden_text());
    let run = lab.resolve();
    run.code(0);
    assert_eq!(mask_timing(&run.stdout), GOLDEN_TEXT, "{}", run.show());
    assert!(
        run.stderr.contains("LCR-FAKE-STDERR-SENTINEL\n"),
        "the binary's stderr is copied to spec's:\n{}",
        run.show()
    );

    lab.fake.clear();
    lab.fake.script(&[], &golden_text());
    let run = lab.resolve_json();
    run.code(0);
    assert_eq!(top_keys(&run.stdout), JSON_KEYS, "{}", run.stdout);
    let doc = run.json();
    assert_eq!(doc["protocol"], 1, "{doc}");
    assert_eq!(doc["ra_ap"], "0.0.352", "{doc}");
    assert_eq!(doc["host"], "masked", "{doc}");
    let workspaces = doc["workspaces"].as_array().expect("workspaces");
    assert_eq!(workspaces.len(), 1, "{doc}");
    let workspace = &workspaces[0];
    assert_eq!(keys_of(workspace), sorted(&WORKSPACE_KEYS), "{workspace}");
    assert_eq!(workspace["dir"], ".", "{workspace}");
    assert_eq!(workspace["toolchain"], "masked", "{workspace}");
    assert_eq!(workspace["sysroot_packages"], 0, "{workspace}");
    assert_eq!(workspace["metadata_degraded"], false, "{workspace}");
    for key in ["discover_ms", "database_ms"] {
        assert!(workspace[key].is_u64(), "{key}: {workspace}");
    }
    assert_eq!(keys_of(&doc["by_state"]), sorted(&STATES), "{doc}");
    for (state, want) in [
        ("moniker", 26),
        ("local", 0),
        ("none", 0),
        ("unresolved", 2),
        ("not_loaded", 1),
    ] {
        assert_eq!(doc["by_state"][state], want, "by_state.{state}: {doc}");
    }
    for (key, want) in [
        ("files", 6),
        ("in_crate", 4),
        ("items", 29),
        ("joined", 29),
        ("unmatched", 0),
        ("c_only", 0),
        ("agrees", 26),
        ("disagrees", 0),
        ("collisions", 1),
    ] {
        assert_eq!(doc[key], want, "{key}: {doc}");
    }
    assert!(doc["ms"].is_u64(), "{doc}");
    assert!(doc["peak_rss"].is_u64(), "{doc}");

    let db = lab.db();
    assert_eq!(count(&db, "code_resolve"), 1);
    assert_eq!(count(&db, "code_resolve_files"), 6);
    assert_eq!(count(&db, "code_monikers"), 29);
    let in_crate = query(
        &db,
        "SELECT path FROM code_resolve_files WHERE in_crate = 1 ORDER BY path",
    );
    assert_eq!(in_crate.len(), 4, "{in_crate:?}");
    let raw = query(
        &db,
        "SELECT moniker FROM code_monikers WHERE path = 'arena/src/lib.rs' AND start IN (341, 453)",
    );
    assert!(
        raw.iter()
            .all(|row| row["moniker"] == "arena impl#[Health]"),
        "the raw moniker is stored, no suffix: {raw:?}"
    );
}

/// The line appended to `arena/src/systems.rs` by
/// [`attributed_item_joins_its_row`]: two attributed items on one line.
const SHARED_LINE: &str = "#[inline] pub fn alpha() {} #[inline] pub fn beta() {}\n";

/// R8. M: the join by line. Two items, each behind an attribute, share one
/// line appended to `arena/src/systems.rs` (the golden's offsets
/// untouched); C rows at their exact bytes (after the attributes, as the
/// golden's `Health`, its attribute a line above): joined 31, unmatched 0,
/// c_only 0.
#[test]
fn attributed_item_joins_its_row() {
    let lab = Lab::ra_mini("lcr-attributed");
    let systems = "arena/src/systems.rs";
    let before = read_text(&lab.root, systems);
    write(&lab.root, systems, format!("{before}{SHARED_LINE}"));
    let base = u64::try_from(before.len()).expect("a length");
    let span = |item: &str| -> (u64, u64) {
        let at = SHARED_LINE.find(item).expect("the item on the line");
        let start = base + u64::try_from(at).expect("an offset");
        (start, start + u64::try_from(item.len()).expect("a length"))
    };
    let alpha = span("pub fn alpha() {}");
    let beta = span("pub fn beta() {}");
    lab.index();
    let rows = query(
        &lab.db(),
        "SELECT s.start_byte AS start, s.end_byte AS \"end\" FROM symbols s \
         JOIN code_files f ON f.file_id = s.file WHERE f.path = 'arena/src/systems.rs'",
    );
    let spans: Vec<(u64, u64)> = rows
        .iter()
        .map(|row| {
            (
                row["start"].as_u64().expect("start_byte"),
                row["end"].as_u64().expect("end_byte"),
            )
        })
        .collect();
    assert!(
        spans.contains(&alpha) && spans.contains(&beta),
        "layer A's rows of the shared line at {alpha:?} and {beta:?}: {spans:?}"
    );

    let mut reply = Reply::golden();
    let file = reply.file_mut(systems);
    for ((start, end), name) in [(alpha, "alpha"), (beta, "beta")] {
        file.items.push(Item {
            start,
            end,
            kind: "function".to_owned(),
            state: "moniker".to_owned(),
            moniker: Some(format!("arena systems/{name}().")),
        });
    }
    lab.fake.script(&[], &reply.render());
    let run = lab.resolve_json();
    run.code(0);
    let found = counts(&run);
    assert_eq!(
        (found["joined"], found["unmatched"], found["c_only"]),
        (31, 0, 0),
        "{}",
        run.show()
    );
}

/// R8. M: the join by name. The second `impl Health` (453) answered at
/// bytes no layer A row has (600..601) with the same raw moniker: by
/// offset it joins nothing (c_only 1) and A's `impl` stays unmatched,
/// joined 28; the raw moniker still collides (collisions 1).
#[test]
fn same_name_items_join_by_offset() {
    let lab = Lab::ra_mini("lcr-same-name");
    lab.index();
    let mut reply = Reply::golden();
    let items = &mut reply.file_mut("arena/src/lib.rs").items;
    let second = items
        .iter()
        .position(|item| item.start == 453)
        .expect("the second `impl Health`");
    let moniker = items[second].moniker.clone();
    assert_eq!(moniker.as_deref(), Some("arena impl#[Health]"));
    items.remove(second);
    let at = items
        .iter()
        .position(|item| item.start > 600)
        .expect("an item after byte 600");
    items.insert(
        at,
        Item {
            start: 600,
            end: 601,
            kind: "impl".to_owned(),
            state: "moniker".to_owned(),
            moniker,
        },
    );
    lab.fake.script(&[], &reply.render());
    let run = lab.resolve_json();
    run.code(0);
    let found = counts(&run);
    assert_eq!(
        (
            found["joined"],
            found["unmatched"],
            found["c_only"],
            found["collisions"]
        ),
        (28, 1, 1, 1),
        "{}",
        run.show()
    );
}

/// R8. M: `end` ignored; kinds ignored. The golden with `Health`'s `end`
/// one byte off, then with its `kind` one the kind map does not match
/// (`enum` for a `struct_item`): each joined 28, unmatched 1, c_only 1.
#[test]
fn boundary_mismatch_is_unmatched_and_c_only() {
    let lab = Lab::ra_mini("lcr-boundary");
    lab.index();
    let mut off_by_one = Reply::golden();
    off_by_one.item_mut("arena/src/lib.rs", 278).end = 340;
    let mut other_kind = Reply::golden();
    other_kind.item_mut("arena/src/lib.rs", 278).kind = "enum".to_owned();
    for (name, reply) in [
        ("an end one byte off", off_by_one),
        ("a kind the map does not match", other_kind),
    ] {
        lab.fake.clear();
        lab.fake.script(&[], &reply.render());
        let run = lab.resolve_json();
        run.code(0);
        let found = counts(&run);
        assert_eq!(
            (found["joined"], found["unmatched"], found["c_only"]),
            (28, 1, 1),
            "{name}:\n{}",
            run.show()
        );
    }
}

/// R8. M: offsets from normalised text. `arena/src/systems.rs` with CRLF
/// line ends, its items answered at their raw bytes (one more per line
/// end before them): joined 29, unmatched 0, c_only 0.
#[test]
fn crlf_file_joins_by_raw_bytes() {
    let lab = Lab::ra_mini("lcr-crlf");
    let systems = "arena/src/systems.rs";
    let lf = read_text(&lab.root, systems);
    assert!(!lf.contains('\r'), "the fixture is LF");
    write(&lab.root, systems, lf.replace('\n', "\r\n"));
    lab.index();
    let mut reply = Reply::golden();
    for item in &mut reply.file_mut(systems).items {
        item.start = crlf_offset(&lf, item.start);
        item.end = crlf_offset(&lf, item.end);
    }
    lab.fake.script(&[], &reply.render());
    let run = lab.resolve_json();
    run.code(0);
    let found = counts(&run);
    assert_eq!(
        (found["joined"], found["unmatched"], found["c_only"]),
        (29, 0, 0),
        "{}",
        run.show()
    );
}

/// ADR-0008: a project of another nature (`fixtures/code-index`, a
/// library of sheets: `cfg` twins, a broken item, a binary, a test, RON
/// data). The request names its walked UTF-8 `.rs` files in byte order;
/// a reply built from its own layer A rows joins every row.
#[test]
fn every_row_joins_on_a_second_project() {
    let lab = Lab::fixture("lcr-second", "code-index", "inkwell");
    lab.index();
    let files = lab.requested_files();
    assert!(files.len() >= 5, "{files:?}");
    let reply = Reply::from_index(&lab.db(), &files);
    let items = u64::try_from(reply.item_count()).expect("a count");
    assert!(items > 10, "{items} rows");
    lab.fake.script(&[], &reply.render());
    let run = lab.resolve_json();
    run.code(0);
    let started = lab.fake.only();
    assert_eq!(
        request_json(&lab, started.pid)["files"],
        json!(files),
        "the walked UTF-8 `.rs` files, byte order"
    );
    let found = counts(&run);
    assert_eq!(
        (
            found["joined"],
            found["unmatched"],
            found["c_only"],
            found["collisions"]
        ),
        (items, 0, 0, 0),
        "{}",
        run.show()
    );
}

// ------------------------------------------------------------------- R9

/// R9 (TE-4). M: hash-map order. ra-mini in two roots under two `HOME`s,
/// the second created in reverse order: the layer C dumps and the masked
/// texts are byte-identical; a second run in the first root too.
#[test]
fn resolve_rows_are_byte_identical_across_runs_and_roots() {
    let one = Lab::ra_mini("lcr-det-one");
    let two = Lab::ra_mini_reversed("lcr-det-two");
    let run_one = one.resolve();
    run_one.code(0);
    let rows_one = one.layer_c();
    assert!(rows_one.contains("code_monikers\t"), "{rows_one}");
    let run_two = two.resolve();
    run_two.code(0);
    assert_eq!(two.layer_c(), rows_one, "two roots, reversed creation");
    assert_eq!(
        mask_timing(&run_two.stdout),
        mask_timing(&run_one.stdout),
        "the text"
    );
    one.fake.clear();
    let again = one.resolve();
    again.code(0);
    assert_eq!(one.layer_c(), rows_one, "a second run");
    assert_eq!(mask_timing(&again.stdout), mask_timing(&run_one.stdout));
}

// ------------------------------------------------------------------ R14

/// R14. M: `files` re-sorted by `Path`; `root` or `<slug>.ra` not
/// canonicalised. `spec` run from a subdirectory of a symlink to the root,
/// with `HOME` a symlink too, a `.ron` and a non-UTF-8 `.rs` under a
/// walked root: argv exactly `--protocol 1`, the binary leading its own
/// group, cwd `D/tmp`; one request line with exactly the six keys, `root`
/// canonical, `files` the golden's six in byte order, `budget_ms` the
/// timeout's, `target_dir` and `tmp_dir` under the canonical `D`; the
/// environment `spec` had, unchanged. Without `--timeout`: 300 000.
#[test]
fn resolve_request_is_protocol_v1() {
    let lab = Lab::ra_mini("lcr-request");
    write(&lab.root, "arena/src/tuning.ron", "(regen: 3)\n");
    write(
        &lab.root,
        "arena/src/latin1.rs",
        b"// caf\xe9 au lait\nfn latin() {}\n",
    );
    let root_link = lab.base.join("root-link");
    symlink(&lab.root, &root_link).expect("a symlink to the root");
    let home_link = lab.base.join("home-link");
    symlink(&lab.home, &home_link).expect("a symlink to HOME");

    let ended = lab
        .start_in(
            &root_link.join("arena"),
            &home_link,
            &["resolve", "--timeout", "7"],
        )
        .wait(RUN_LIMIT);
    let exited = ended.run();
    let started = lab.fake.only();
    assert_eq!(started.args, ["--protocol", "1"], "{started:?}");
    assert_eq!(
        started.pgid, started.pid,
        "the binary leads a process group of its own"
    );
    let ra = fs::canonicalize(lab.ra_dir()).expect("<data dir>/ra-mini.ra exists");
    assert_eq!(PathBuf::from(&started.cwd), ra.join("tmp"), "cwd D/tmp");

    let line = lab
        .fake
        .request_of(started.pid)
        .expect("the request was read");
    assert!(
        line.ends_with('\n') && line.matches('\n').count() == 1,
        "one line: {line:?}"
    );
    let request = request_json(&lab, started.pid);
    assert_eq!(keys_of(&request), sorted(&REQUEST_KEYS), "{request}");
    assert_eq!(request["protocol"], 1, "{request}");
    assert_eq!(
        request["root"],
        lab.root.to_str().expect("a UTF-8 root"),
        "the canonical root"
    );
    assert_eq!(
        request["files"],
        json!(FILES),
        "byte order, .rs and UTF-8 only"
    );
    assert_eq!(request["budget_ms"], 7000, "{request}");
    assert_eq!(
        request["target_dir"],
        ra.join("target").to_str().expect("UTF-8"),
        "{request}"
    );
    assert_eq!(
        request["tmp_dir"],
        ra.join("tmp").to_str().expect("UTF-8"),
        "{request}"
    );

    let seen = lab.fake.env_of(started.pid);
    let had: BTreeMap<String, String> = lab
        .env_for(&home_link)
        .into_iter()
        .map(|(name, value)| {
            (
                name.into_string().expect("a UTF-8 name"),
                value.into_string().expect("a UTF-8 value"),
            )
        })
        .collect();
    assert_eq!(
        seen, had,
        "spec's environment, unchanged (no TMPDIR of its own)"
    );

    exited.code(0);

    lab.fake.clear();
    lab.resolve().code(0);
    let started = lab.fake.only();
    assert_eq!(
        request_json(&lab, started.pid)["budget_ms"],
        300_000,
        "the default budget"
    );
}

// ------------------------------------------------------------------ R15

/// R15. M: stdin closed after the request. A fake that reads the request,
/// then watches stdin for 1 s and ends as the binary does on EOF (exit 3,
/// no line): held, it replays the golden, exit 0, `fresh`. A fake that
/// waits for EOF after its last line (the binary's guard): `spec` closes
/// stdin then, and the run ends at once.
#[test]
fn stdin_is_held_until_the_last_line() {
    let lab = Lab::ra_mini("lcr-stdin");
    lab.fake.script(&[("hold_ms", "1000")], &golden_text());
    let run = lab.resolve();
    assert!(
        !lab.fake.ended_on_eof(),
        "stdin closed before the last line:\n{}",
        run.show()
    );
    run.code(0);
    assert!(run.stdout.contains("stored fresh"), "{}", run.show());

    lab.fake.clear();
    lab.fake.script(&[("after", "eof")], &golden_text());
    let ended = lab.ended(&["resolve"]);
    ended.run().code(0);
    assert_eq!(
        lab.fake.waited().as_deref(),
        Some("eof"),
        "stdin is closed once the last line is read"
    );
    assert!(ended.elapsed < SLACK, "closed at once: {:?}", ended.elapsed);

    let cases = [
        (
            "after error",
            format!("{}{}\n", golden_head(3), error_line(1, "database")),
            "1",
            "load failed: database (workspace 1)",
        ),
        (
            "after refused",
            format!("{}\n", refused_line(1, "tmp_dir: not created")),
            "2",
            "specengine-ra refused the request: tmp_dir: not created",
        ),
    ];
    for (name, reply, exit, reason) in &cases {
        lab.fake.clear();
        lab.fake.script(&[("after", "eof"), ("exit", *exit)], reply);
        let ended = lab.ended(&["resolve"]);
        refused_as(name, &ended.run(), reason);
        assert_eq!(
            lab.fake.waited().as_deref(),
            Some("eof"),
            "{name}: stdin is closed once the last line is read"
        );
        assert!(
            ended.elapsed < SLACK,
            "{name}: at once: {:?}",
            ended.elapsed
        );
    }
}

/// R15. M: the request written to its end before the timer starts. A
/// request over 128 KiB (a scratch tree of 800 long `.rs` names) to a
/// fake that never reads stdin, `--timeout 1`: exit 2 `over the budget of
/// 1 s` within 1 + 5 s plus slack, the request never read, no survivor.
#[test]
fn budget_covers_an_unread_request() {
    let lab = Lab::ra_mini("lcr-unread");
    let long = "a".repeat(200);
    for n in 0..800 {
        write(
            &lab.root,
            &format!("many/{long}_{n:03}.rs"),
            "pub fn f() {}\n",
        );
    }
    write(
        &lab.root,
        "specengine.toml",
        config_with_roots("[\"many\"]"),
    );
    lab.index();
    assert!(
        lab.requested_files()
            .iter()
            .map(|path| path.len() + 3)
            .sum::<usize>()
            > 128 * 1024,
        "the request is over 128 KiB"
    );
    lab.fake.script(&[("read", "none"), ("after", "hang")], "");
    let ended = lab.start(&["resolve", "--timeout", "1"]).wait(RUN_LIMIT);
    assert_refused(&ended.run(), "over the budget of 1 s");
    assert!(
        ended.elapsed < Duration::from_secs(1) + GRACE + SLACK,
        "the budget ran while the request was written: {:?}",
        ended.elapsed
    );
    let started = lab.fake.only();
    assert!(lab.fake.request_of(started.pid).is_none(), "never read");
    let survivors = lab.fake.survivors(Duration::from_secs(3));
    assert!(survivors.is_empty(), "{survivors:?} outlived spec");
}

// ----------------------------------------------------------------- TE-2

/// TE-2 (A6): the real `specengine-ra`, built beside the `spec` under test
/// (`cargo build -p specengine-ra`), copied beside a copy of `spec` on a
/// scratch ra-mini with the test's own toolchain environment, `--timeout
/// 180`: exit 0, `fresh`; the stored files and items equal the golden
/// replay's (the `code_resolve` row's stamps aside).
#[test]
#[ignore = "TE-2: the real specengine-ra on ra-mini, 21-50 s in debug; build it first"]
fn real_binary_stores_the_golden_rows() {
    let built = PathBuf::from(common::SPEC).with_file_name(RA_BIN);
    assert!(
        built.is_file(),
        "build it first: cargo build -p specengine-ra ({})",
        built.display()
    );
    let mut real = Lab::ra_mini("lcr-te2-real");
    fs::remove_file(&real.fake.program).expect("the fake out");
    copy_file(built, &real.fake.program);
    real.path = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    for name in ["CARGO_HOME", "RUSTUP_HOME"] {
        if let Some(value) = std::env::var_os(name) {
            real.vars.push((name.into(), value));
        }
    }
    let ended = real
        .start(&["resolve", "--timeout", "180"])
        .wait(Duration::from_secs(240));
    let run = ended.run();
    run.code(0);
    assert!(run.stdout.contains("stored fresh"), "{}", run.show());

    let replay = Lab::ra_mini("lcr-te2-replay");
    replay.store_golden();
    let pick = |dump: &str, table: &str| -> Vec<String> {
        dump.lines()
            .filter(|line| line.starts_with(&format!("{table}\t")))
            .map(str::to_owned)
            .collect()
    };
    let (real_rows, replayed) = (real.layer_c(), replay.layer_c());
    for table in ["code_resolve_files", "code_monikers"] {
        assert_eq!(
            pick(&real_rows, table),
            pick(&replayed, table),
            "{table}: the real binary's rows are the golden's"
        );
    }
    assert_eq!(pick(&real_rows, "code_resolve").len(), 1);
}
