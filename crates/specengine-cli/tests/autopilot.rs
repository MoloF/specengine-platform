//! docs/features/auto-runner.md RN1-RN8: `plugin/specengine/scripts/
//! autopilot.sh` run under `sh` (and `dash` and `bash` where the criterion
//! says each shell) with a `claude` shim first on `PATH` (the bench of
//! `auto_common/runner.rs`), `jq` required, over scratch copies of
//! `fixtures/auto-mini` with `rules/on.toml` committed and a pack recorded
//! through the store (no terminal), the branch `auto` at `HEAD`.
//!
//! Not covered: RN0 (the owner's live run against Claude Code).
//!
//! Mutations: `Command::new("claude")` in the CLI; `--busy` not passed;
//! `--skip-lane` not passed for a busy lane; the pause counted as a
//! failure; the stop answer ignored; `--dangerously-skip-permissions` added
//! to the argv; classifying by the exit code; the script left out of the
//! plugin's set (`specengine-mcp` `tests/plugin_files.rs`).

#![cfg(unix)]

mod auto_common;
mod common;
#[path = "auto_common/runner.rs"]
mod runner;

use std::fs;
use std::path::Path;
use std::time::Duration;

use auto_common::{Auto, utc};
use common::repository_root;
use runner::{Bench, finish, script, shells};
use serde_json::Value;

const LIMIT: Duration = Duration::from_secs(150);

fn bench(label: &str) -> Bench {
    Bench::new(Auto::with_rule(label, "on.toml"))
}

/// The runner's own arguments of every test: the root, a one-second poll,
/// a two-second pause.
fn args<'a>(bench: &'a Bench, extra: &[&'a str]) -> Vec<&'a str> {
    let mut all = vec![
        "--root",
        bench.main.to_str().unwrap(),
        "--poll",
        "1",
        "--pause",
        "2",
    ];
    all.extend_from_slice(extra);
    all
}

fn held_of(bench: &Bench, item: &str) -> Vec<Value> {
    bench
        .events_of_kind("auto.held")
        .iter()
        .map(auto_common::RawEvent::json)
        .filter(|held| held["task"] == item)
        .collect()
}

fn landed(bench: &Bench) -> Vec<String> {
    bench
        .events_of_kind("auto.landed")
        .iter()
        .map(|event| event.json()["task"].as_str().unwrap().to_owned())
        .collect()
}

/// Rust source with `//` and `/* */` comments blanked (string literals
/// kept), as `tests/door.rs` scans.
fn without_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes: Vec<char> = text.chars().collect();
    let (mut i, mut in_string) = (0, false);
    while i < bytes.len() {
        let c = bytes[i];
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < bytes.len() {
                out.push(bytes[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
        } else if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
        } else if c == '/' && bytes.get(i + 1) == Some(&'/') {
            while i < bytes.len() && bytes[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && bytes.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == '*' && bytes[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// Each `Command::new(...)` of `code` naming `claude` (any case).
fn claude_commands(code: &str) -> Vec<String> {
    code.match_indices("Command::new(")
        .filter_map(|(at, _)| {
            let call: String = code[at..].chars().take(120).collect();
            let call = call.split(')').next().unwrap_or_default().to_owned();
            call.to_ascii_lowercase().contains("claude").then_some(call)
        })
        .collect()
}

/// RN1: no `Command::new` in any crate's `src` names `claude`; no crate
/// depends on a Claude SDK. M: `Command::new("claude")` in the CLI's
/// `auto_next.rs`.
#[test]
fn rn1_no_agent_inside_specengine() {
    assert_eq!(
        claude_commands(&without_comments(
            "let a = Command::new(\"claude\"); // Command::new(\"claude\")\n"
        ))
        .len(),
        1,
        "the scan sees one, the comment's aside"
    );
    assert!(claude_commands(&without_comments("// Command::new(\"claude\")\n")).is_empty());
    let mut offenders = Vec::new();
    let mut scanned = 0;
    let crates = repository_root().join("crates");
    let mut stack = vec![crates.clone()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            let rel = path
                .strip_prefix(&crates)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            if path.is_dir() {
                let name = entry.file_name();
                if name != "tests" && name != "target" && name != "benches" {
                    stack.push(path);
                }
            } else if rel.contains("/src/") && rel.ends_with(".rs") {
                scanned += 1;
                let text = fs::read_to_string(&path).unwrap();
                for call in claude_commands(&without_comments(&text)) {
                    offenders.push(format!("{rel}: {call}"));
                }
            } else if rel.ends_with("Cargo.toml") {
                let manifest: toml::Table = fs::read_to_string(&path).unwrap().parse().unwrap();
                for table in ["dependencies", "dev-dependencies", "build-dependencies"] {
                    if let Some(deps) = manifest.get(table).and_then(toml::Value::as_table) {
                        for name in deps.keys() {
                            let lower = name.to_ascii_lowercase();
                            if lower.contains("claude") || lower.contains("anthropic") {
                                offenders.push(format!("{rel}: depends on {name}"));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(scanned > 100, "{scanned} sources scanned");
    let workspace: toml::Table = fs::read_to_string(repository_root().join("Cargo.toml"))
        .unwrap()
        .parse()
        .unwrap();
    if let Some(deps) = workspace
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(toml::Value::as_table)
    {
        for name in deps.keys() {
            let lower = name.to_ascii_lowercase();
            assert!(
                !lower.contains("claude") && !lower.contains("anthropic"),
                "the workspace depends on {name}"
            );
        }
    }
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

/// RN2: three tasks, two overlapping on R-02, `--lanes 2`: one worktree
/// per task on `auto-<T>` from `auto`, at most two legs alive, the
/// overlapping pair's implement legs never together, every worktree
/// removed after its land. M: `--busy` not passed.
#[test]
fn rn2_one_worktree_per_task_never_two_overlapping_legs() {
    let bench = bench("rn2");
    let a = bench.make_task(&["R-02"], "Six seconds");
    let b = bench.make_task(&["R-02"], "A creaking door");
    let c = bench.make_task(&["R-04"], "The hall");
    bench.pack(&[&a, &b, &c], &[]);
    let start = bench.rev(&bench.main, "auto");
    let run = bench.run("/bin/sh", &args(&bench, &["--lanes", "2"]), LIMIT);
    assert_eq!(run.code, 0, "{}", run.show());
    assert!(run.stderr.contains("autopilot: done"), "{}", run.show());
    let mut want = vec![a.clone(), b.clone(), c.clone()];
    let mut got = landed(&bench);
    want.sort();
    got.sort();
    assert_eq!(got, want, "{}", run.show());
    let worktrees = bench.worktrees();
    for task in [&a, &b, &c] {
        let implement: Vec<_> = bench
            .calls()
            .into_iter()
            .filter(|call| {
                call.argv.contains(&format!("/specengine:feature {task}"))
                    && call.cwd != bench.main.to_str().unwrap()
            })
            .collect();
        assert_eq!(implement.len(), 1, "{task}: one implement leg");
        assert_eq!(
            Path::new(&implement[0].cwd),
            fs::canonicalize(&worktrees).unwrap().join(task),
            "{task}: its worktree"
        );
        let branch = format!("auto-{task}");
        assert!(bench.refs().contains_key(&format!("refs/heads/{branch}")));
        bench.git.git(
            &bench.main,
            &["merge-base", "--is-ancestor", &start, &branch],
        );
        assert!(
            !worktrees.join(task).exists(),
            "{task}: removed after its land"
        );
    }
    assert!(
        !bench
            .git_text(&bench.main, &["worktree", "list", "--porcelain"])
            .contains(".auto/T-"),
        "no task worktree left"
    );
    let (most, _) = bench.concurrency(|_| true);
    assert!(most <= 2, "{most} legs at once");
    let (_, pairs) = bench.concurrency(|mark| mark.leg == "implement");
    let pair = if a < b {
        (a.clone(), b.clone())
    } else {
        (b.clone(), a.clone())
    };
    assert!(
        !pairs.contains(&pair),
        "{pair:?} implemented together: {pairs:?}"
    );
    bench.assert_permissions();
}

const DEPARTMENTS: &str = "[[workspace]]\nname   = \"core\"\nmodule = \"core\"\n\n[[workspace]]\nname   = \"rooms\"\nmodule = \"rooms\"\n\n[[module]]\nname = \"core\"\nroot = \"R-02\"\n\n[[module]]\nname = \"rooms\"\nroot = \"R-04\"\n";

/// RN3: two modules and `--by-department`: each lane one leg at a time, a
/// task under neither on `general`, nothing refused. M: `--skip-lane` not
/// passed for a busy lane.
#[test]
fn rn3_lanes_by_department() {
    let bench = bench("rn3");
    bench.commit_table(DEPARTMENTS, "two departments");
    let core_a = bench.make_task(&["R-02"], "Core one");
    let core_b = bench.make_task(&["R-02"], "Core two");
    let rooms = bench.make_task(&["R-04"], "Rooms");
    let general = bench.make_task(&["R-05"], "Neither");
    bench.pack(&[&core_a, &core_b, &rooms, &general], &[]);
    let lanes = [
        (core_a.clone(), "core"),
        (core_b.clone(), "core"),
        (rooms.clone(), "rooms"),
        (general.clone(), "general"),
    ];
    let lane = |task: &str| lanes.iter().find(|(t, _)| t == task).unwrap().1;
    // `next` names each task's lane.
    let next = bench.next(&[]);
    assert_eq!(next["lane"], "core");
    let run = bench.run(
        "/bin/sh",
        &args(&bench, &["--lanes", "4", "--by-department"]),
        LIMIT,
    );
    assert_eq!(run.code, 0, "{}", run.show());
    assert!(run.stderr.contains("autopilot: done"), "{}", run.show());
    assert_eq!(landed(&bench).len(), 4, "nothing refused: {}", run.show());
    let (_, pairs) = bench.concurrency(|_| true);
    for (x, y) in &pairs {
        assert_ne!(
            lane(x),
            lane(y),
            "{x} and {y} ran together on lane {}",
            lane(x)
        );
    }
    bench.assert_permissions();
}

/// RN4: `spec auto stop` during a held leg: no leg starts after the next
/// `next`, the held one finishes, exit 0 `stopped (stopped)`. M: the stop
/// answer ignored.
#[test]
fn rn4_stop_during_a_held_leg() {
    let bench = bench("rn4-stop");
    let a = bench.make_task(&["R-02"], "Held");
    let b = bench.make_task(&["R-04"], "Never started");
    bench.pack(&[&a, &b], &[]);
    bench.scenario(&[(a.as_str(), "1", "ok", "yes", 0, true)]);
    let child = bench.spawn("/bin/sh", &args(&bench, &["--lanes", "1"]), None);
    let waited = std::time::Instant::now();
    while !fs::read_to_string(bench.log.join("events"))
        .unwrap_or_default()
        .contains("held ")
    {
        assert!(waited.elapsed() < Duration::from_secs(60), "no leg held");
        std::thread::sleep(Duration::from_millis(50));
    }
    bench.spec(&["auto", "stop"]).code(0);
    std::thread::sleep(Duration::from_secs(2));
    bench.release();
    let run = finish(child, "/bin/sh", &["rn4-stop"], LIMIT);
    assert_eq!(run.code, 0, "{}", run.show());
    assert!(
        run.stderr
            .contains("autopilot: stopped (stopped): the stop file "),
        "{}",
        run.show()
    );
    let calls = bench.calls();
    assert_eq!(calls.len(), 1, "no leg after the stop: {calls:?}");
    assert_eq!(bench.status(&a), "review", "the held leg finished its work");
    assert_eq!(bench.status(&b), "draft");
    bench.assert_permissions();
}

/// RN4: `--until` two seconds ahead: no leg starts at or after it; exit 0
/// `until`.
#[test]
fn rn4_until() {
    let bench = bench("rn4-until");
    let a = bench.make_task(&["R-02"], "First");
    let b = bench.make_task(&["R-04"], "Second");
    let c = bench.make_task(&["R-05"], "Third");
    bench.pack(&[&a, &b, &c], &[]);
    let until = utc(2);
    let until_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        + 2;
    let run = bench.run(
        "/bin/sh",
        &args(&bench, &["--lanes", "1", "--until", &until]),
        LIMIT,
    );
    assert_eq!(run.code, 0, "{}", run.show());
    assert!(
        run.stderr.contains(&format!("autopilot: until {until}")),
        "{}",
        run.show()
    );
    for call in bench.calls() {
        assert!(
            call.at < until_epoch,
            "a leg at {} past {until_epoch}",
            call.at
        );
    }
    assert!(landed(&bench).is_empty());
}

/// RN4: a usage limit (`rate_limit_event` rejected, its `resetsAt` 58 s
/// ago, so the pause, `resetsAt` + 60 s, ends 2 s on): no hold, no
/// failure counted, the same leg runs again; a failure before it is the
/// only one. M: the pause counted as a failure (two in a row: exit 1).
#[test]
fn rn4_a_usage_limit_pauses_and_runs_the_leg_again() {
    let bench = bench("rn4-limit");
    let failing = bench.make_task(&["R-04"], "Fails once");
    let limited = bench.make_task(&["R-02"], "Waits out a limit");
    bench.pack(&[&failing, &limited], &[]);
    bench.scenario(&[
        (failing.as_str(), "*", "execution", "no", 0, false),
        (limited.as_str(), "1", "limited", "no", 0, false),
    ]);
    let run = bench.run("/bin/sh", &args(&bench, &["--lanes", "1"]), LIMIT);
    assert_eq!(run.code, 0, "{}", run.show());
    assert!(run.stderr.contains("autopilot: done"), "{}", run.show());
    assert!(run.stderr.contains(" paused "), "{}", run.show());
    assert!(
        held_of(&bench, &limited).is_empty(),
        "no hold for the limit"
    );
    assert_eq!(held_of(&bench, &failing).len(), 1);
    assert_eq!(landed(&bench), std::slice::from_ref(&limited));
    let plans = bench
        .marks()
        .into_iter()
        .filter(|mark| mark.start && mark.subject == limited && mark.leg == "plan")
        .count();
    assert_eq!(plans, 2, "the same leg ran again");
}

/// RN4: two `agent-error` legs in a row: exit 1.
#[test]
fn rn4_two_failures_in_a_row_end_the_run() {
    let bench = bench("rn4-fail");
    let a = bench.make_task(&["R-02"], "One");
    let b = bench.make_task(&["R-04"], "Two");
    let c = bench.make_task(&["R-05"], "Three");
    bench.pack(&[&a, &b, &c], &[]);
    bench.scenario(&[
        (a.as_str(), "*", "execution", "no", 1, false),
        (b.as_str(), "*", "execution", "no", 1, false),
    ]);
    let run = bench.run("/bin/sh", &args(&bench, &["--lanes", "1"]), LIMIT);
    assert_eq!(run.code, 1, "{}", run.show());
    assert!(
        run.stderr
            .contains("autopilot: two failures in a row on lane "),
        "{}",
        run.show()
    );
    assert_eq!(bench.calls().len(), 2, "nothing after the second");
    assert_eq!(held_of(&bench, &a)[0]["reason"], "agent-error");
    assert_eq!(held_of(&bench, &b)[0]["reason"], "agent-error");
}

/// RN5: the script's text holds none of the three forbidden words, and
/// passes no free `claude` argument. M: `--dangerously-skip-permissions`
/// added to the argv.
#[test]
fn rn5_permissions_are_never_skipped() {
    let text = fs::read_to_string(script()).unwrap();
    let code: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "--dangerously-skip-permissions",
        "--allow-dangerously-skip-permissions",
        "bypassPermissions",
    ] {
        assert!(
            !code.contains(forbidden),
            "{forbidden} in the script's code"
        );
    }
    assert!(code.contains("--permission-prompts none"));
    // A run with a denial: the argv and the hold.
    let bench = bench("rn5");
    let task = bench.make_task(&["R-02"], "Wants a shell");
    bench.pack(&[&task], &[]);
    bench.scenario(&[(task.as_str(), "1", "denied", "yes", 0, false)]);
    let run = bench.run(
        "/bin/sh",
        &args(
            &bench,
            &["--lanes", "1", "--accept-edits", "--model", "opus"],
        ),
        LIMIT,
    );
    assert_eq!(run.code, 0, "{}", run.show());
    let held = held_of(&bench, &task);
    assert_eq!(held.len(), 1);
    assert_eq!(held[0]["reason"], "denied-tool");
    assert_eq!(held[0]["detail"], "denied: Bash");
    assert_eq!(held[0]["act"], "run");
    bench.assert_permissions();
    let call = &bench.calls()[0];
    assert!(
        call.argv
            .windows(2)
            .any(|w| w == ["--permission-mode", "acceptEdits"])
    );
}

/// RN6: three tasks and one line, end to end: each planned, approved by
/// rule, implemented and landed; `auto` holds four `--no-ff` merges; `next`
/// answers `done`; exit 0 `autopilot: done`; four merges and four code
/// commits carry `Delegated-by: auto`.
fn night(shell: &str, label: &str) {
    let bench = bench(label);
    let a = bench.make_task(&["R-02"], "Six seconds");
    let b = bench.make_task(&["R-04"], "The hall");
    let c = bench.make_task(&["R-05"], "The cellar");
    let line = "Make the inventory sortable by weight";
    let pack = bench.pack(&[&a, &b, &c], &[line]);
    let start = bench.rev(&bench.main, "auto");
    let run = bench.run(shell, &args(&bench, &["--lanes", "2"]), LIMIT);
    assert_eq!(run.code, 0, "{shell}: {}", run.show());
    assert!(run.stderr.contains("autopilot: done"), "{}", run.show());
    let bound = bench.events_of_kind("auto.bound");
    assert_eq!(bound.len(), 1, "the line bound");
    let line_task = bound[0].json()["task"].as_str().unwrap().to_owned();
    let mut got = landed(&bench);
    got.sort();
    let mut want = vec![a, b, c, line_task.clone()];
    want.sort();
    assert_eq!(got, want);
    let approved: Vec<Value> = bench
        .events_of_kind("task.approved")
        .iter()
        .map(auto_common::RawEvent::json)
        .collect();
    assert_eq!(approved.len(), 4);
    assert!(
        approved
            .iter()
            .all(|e| e["by"] == "rule" && e["pack"] == pack)
    );
    let merges = bench.git_text(
        &bench.main,
        &[
            "rev-list",
            "--merges",
            "--first-parent",
            &format!("{start}..auto"),
        ],
    );
    assert_eq!(merges.lines().count(), 4, "four --no-ff merges");
    let delegated = bench.git_text(
        &bench.main,
        &[
            "log",
            "--format=%H %P",
            "--grep",
            "^Delegated-by: auto",
            "auto",
        ],
    );
    let merges_delegated = delegated
        .lines()
        .filter(|line| line.split(' ').count() == 3)
        .count();
    assert_eq!(merges_delegated, 4, "{delegated}");
    assert_eq!(
        delegated.lines().count() - merges_delegated,
        4,
        "four code commits"
    );
    assert_eq!(bench.next(&[])["action"], "done");
    // The line's leg ran with its variable; the others without it.
    for call in bench.calls() {
        let line_leg = call.argv.contains(&format!("/specengine:feature {line}"));
        assert_eq!(
            call.env["SPECENGINE_AUTO_LINE"],
            if line_leg {
                format!("{pack}/1")
            } else {
                "<unset>".to_owned()
            },
            "{call:?}"
        );
        assert_eq!(call.env["CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS"], "3600000");
        assert_eq!(
            call.argv[2..],
            [
                "--output-format",
                "stream-json",
                "--verbose",
                "--permission-prompts",
                "none",
                "--max-turns",
                "200"
            ],
            "the pinned argv: {call:?}"
        );
        assert_eq!(call.argv[0], "-p");
        assert!(call.argv[1].starts_with("/specengine:feature "));
    }
    // The log file holds one line per act.
    let log = fs::read_to_string(bench.worktrees().join(".log/autopilot.log")).unwrap();
    assert!(
        log.lines()
            .any(|l| l.contains(&format!(" {line_task} land landed "))),
        "{log}"
    );
    bench.assert_permissions();
}

#[test]
fn rn6_a_night_end_to_end_sh() {
    night("/bin/sh", "rn6-sh");
}

#[test]
fn rn6_a_night_end_to_end_other_shells() {
    for shell in shells().into_iter().skip(1) {
        let label = format!(
            "rn6-{}",
            Path::new(&shell).file_name().unwrap().to_str().unwrap()
        );
        night(&shell, &label);
    }
}

/// One RN7 row: a one-task pack whose leg `attempt` gives `stream` with
/// `queue` work and exit `code`: the run's result and the task's holds.
fn row(
    label: &str,
    attempt: &str,
    stream: &str,
    queue: &str,
    code: i32,
) -> (Bench, String, crate::common::Run) {
    let bench = bench(label);
    let task = bench.make_task(&["R-02"], "A row");
    bench.pack(&[&task], &[]);
    bench.scenario(&[(task.as_str(), attempt, stream, queue, code, false)]);
    let run = bench.run("/bin/sh", &args(&bench, &["--lanes", "1"]), LIMIT);
    assert_eq!(run.code, 0, "{label}: {}", run.show());
    bench.assert_permissions();
    (bench, task, run)
}

fn one_hold(bench: &Bench, task: &str) -> (String, String) {
    let held = held_of(bench, task);
    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!(held[0]["act"], "run");
    (
        held[0]["reason"].as_str().unwrap().to_owned(),
        held[0]["detail"].as_str().unwrap().to_owned(),
    )
}

/// RN7: no init server: `no-mcp`. M: classifying by the exit code.
#[test]
fn rn7_no_mcp_server() {
    let (bench, task, _) = row("rn7-nomcp", "1", "nomcp", "yes", 0);
    assert_eq!(
        one_hold(&bench, &task),
        (
            "no-mcp".into(),
            "the specengine MCP server was not connected".into()
        )
    );
    let (bench, task, _) = row("rn7-noinit", "1", "noinit", "yes", 0);
    assert_eq!(one_hold(&bench, &task).0, "no-mcp");
}

#[test]
fn rn7_max_turns_and_budget() {
    let (bench, task, _) = row("rn7-turns", "1", "maxturns", "no", 1);
    assert_eq!(one_hold(&bench, &task).0, "max-turns");
    let (bench, task, _) = row("rn7-budget", "1", "maxbudget", "no", 1);
    assert_eq!(one_hold(&bench, &task).0, "max-budget");
}

#[test]
fn rn7_agent_errors() {
    let (bench, task, _) = row("rn7-exec", "1", "execution", "no", 1);
    assert_eq!(
        one_hold(&bench, &task),
        (
            "agent-error".into(),
            "error_during_execution: api_error: the API failed".into()
        )
    );
    let (bench, task, _) = row("rn7-noresult", "1", "noresult", "no", 0);
    assert_eq!(
        one_hold(&bench, &task),
        ("agent-error".into(), "no result; claude exited 0".into())
    );
}

/// RN7: success with the task left `draft`: `not-planned`; left
/// `in_progress`: `not-done`.
#[test]
fn rn7_judged_by_the_queue() {
    let (bench, task, _) = row("rn7-notplanned", "1", "ok", "no", 0);
    assert_eq!(
        one_hold(&bench, &task),
        (
            "not-planned".into(),
            format!("{task} is draft after the plan leg")
        )
    );
    let (bench, task, _) = row("rn7-notdone", "2", "ok", "claim", 0);
    let (reason, detail) = one_hold(&bench, &task);
    assert_eq!(reason, "not-done");
    assert!(detail.starts_with("in_progress; run 1"), "{detail}");
}

/// RN7: a success with exit code 1 is classified by its stream: the task
/// goes on and lands.
#[test]
fn rn7_a_success_exiting_1_goes_on() {
    let (bench, task, run) = row("rn7-exit1", "*", "ok", "yes", 1);
    assert!(held_of(&bench, &task).is_empty(), "{}", run.show());
    assert_eq!(landed(&bench), [task]);
}

/// RN8: `sh -n` passes under each shell; `--dry-run` prints the pinned
/// argv and runs nothing; a bad flag or a missing tool exits 2, nothing
/// run.
#[test]
fn rn8_the_script_parses_and_dry_runs() {
    for shell in shells() {
        let status = std::process::Command::new(&shell)
            .arg("-n")
            .arg(script())
            .status()
            .unwrap();
        assert!(status.success(), "{shell} -n");
    }
    let bench = bench("rn8");
    let task = bench.make_task(&["R-02"], "Dry");
    bench.pack(&[&task], &[]);
    let events = bench.raw_events();
    let root = bench.main.to_str().unwrap().to_owned();
    let run = bench.run(
        "/bin/sh",
        &[
            "--root",
            &root,
            "--dry-run",
            "--max-budget-usd",
            "2.5",
            "--model",
            "opus",
        ],
        LIMIT,
    );
    assert_eq!(run.code, 0, "{}", run.show());
    assert_eq!(
        run.stdout,
        format!(
            "next: plan {task} (lane general)\ncwd: {root}\nenv: CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=3600000\nargv: claude -p '/specengine:feature {task}' --output-format stream-json --verbose --permission-prompts none --max-turns 200 --max-budget-usd 2.5 --model opus\n"
        )
    );
    assert!(bench.calls().is_empty(), "nothing run");
    assert_eq!(bench.raw_events(), events);
    assert!(!bench.worktrees().exists(), "no folder made");
    // A bad flag; a missing tool.
    let bad = bench.run("/bin/sh", &["--root", &root, "--lanes", "0"], LIMIT);
    assert_eq!(bad.code, 2);
    assert_eq!(
        bad.stderr,
        "autopilot: --lanes: 0 is out of range: 1 to 16\n"
    );
    let path = bench
        .path()
        .replacen(&format!("{}:", bench.bin.display()), "", 1);
    let child = bench.spawn("/bin/sh", &["--root", &root], Some(&path));
    let missing = finish(child, "/bin/sh", &["no claude"], LIMIT);
    assert_eq!(missing.code, 2);
    assert_eq!(missing.stderr, "autopilot: spec is required on PATH\n");
    assert!(bench.calls().is_empty());
}

/// The reviewer's runner rows: a note opening with `-` reaches `spec auto
/// hold` intact (an `is_error` result with no `subtype`: `-: -: boom`).
#[test]
fn rn7_a_note_opening_with_a_dash_reaches_the_hold_intact() {
    let (bench, task, _) = row("rn7-dash", "1", "dasherror", "no", 1);
    assert_eq!(
        one_hold(&bench, &task),
        ("agent-error".into(), "-: -: boom".into())
    );
}

/// The reviewer's runner rows: a leg whose working directory cannot be
/// entered holds `agent-error`, not `no-mcp`, and no `claude` runs.
#[test]
fn rn7_a_missing_working_directory_holds_agent_error() {
    use std::os::unix::fs::PermissionsExt as _;
    let bench = bench("rn7-nodir");
    let task = bench.planned(&["R-02"], "Nowhere to work", &[]);
    bench.pack(&[&task], &[]);
    bench.auto_approve(&task).code(0);
    // The runner's worktree folder for the task: there, but not enterable.
    let dir = bench.worktrees().join(&task);
    fs::create_dir_all(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o000)).unwrap();
    let run = bench.run("/bin/sh", &args(&bench, &["--lanes", "1"]), LIMIT);
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(run.code, 0, "{}", run.show());
    let (reason, _) = one_hold(&bench, &task);
    assert_eq!(reason, "agent-error", "{}", run.show());
    assert!(bench.calls().is_empty(), "no claude ran");
}

/// The reviewer's runner rows: a land refused "another land holds" is
/// retried a bounded number of times, then the run ends (exit 1), no hold.
#[test]
fn rn4_a_held_lock_is_retried_a_bounded_number_of_times() {
    let bench = bench("rn4-lock");
    let task = bench.planned(&["R-02"], "Waits for the lock", &[]);
    bench.pack(&[&task], &[]);
    bench.to_done(
        &task,
        &[(
            "src/lib.rs",
            "DOOR_SECONDS: u32 = 5",
            "DOOR_SECONDS: u32 = 6",
        )],
    );
    let lock = bench.data().join("auto-mini.auto-land");
    // A live pid (this test's): another land still running.
    fs::write(
        &lock,
        format!("{} 2026-11-16T01:00:00Z", std::process::id()),
    )
    .unwrap();
    let run = bench.run("/bin/sh", &args(&bench, &["--lanes", "1"]), LIMIT);
    assert_eq!(run.code, 1, "{}", run.show());
    let tries = run
        .stderr
        .lines()
        .filter(|line| line.contains(" land skipped another land holds"))
        .count();
    assert!((2..=20).contains(&tries), "{tries} tries: {}", run.show());
    assert!(run.stderr.contains("found the lock held"), "{}", run.show());
    assert!(held_of(&bench, &task).is_empty(), "no hold for a lock");
    assert!(landed(&bench).is_empty());
    assert!(
        lock.exists(),
        "the runner never removes another land's lock"
    );
}
