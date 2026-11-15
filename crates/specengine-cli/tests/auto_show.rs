//! docs/features/auto-rule.md, the CLI's `spec auto show`, `stop` and
//! `resume` over scratch copies of `fixtures/auto-mini` (the binary, the
//! sandbox's git, a scratch `HOME`): RL2 (the rows a scripted queue shows),
//! RL3 (`HEAD`, not the tree), RL4 (a team), RL5 (malformed, never failing),
//! RL9 (stop and resume), RL10 (the config accepts the table), and the off
//! state with no table (RL1's `show`).
//!
//! The rule's hash is the core's reading of the committed config
//! (`tests/auto_rule.rs` pins its bytes); a pack is recorded through the
//! store (no terminal), as `spec auto start` records one.

#![cfg(unix)]

mod auto_common;
mod common;

use std::ffi::OsStr;
use std::fs;

use auto_common::{Auto, expected, rule_text, utc, with_table};
use serde_json::{Value, json};
use specengine_store::{ProposalQueue as _, Stage};

/// The rule's keys of `auto`, `null` when off.
const RULE_KEYS: [&str; 8] = [
    "rule",
    "mode",
    "zones",
    "exclude_zones",
    "exclude_kinds",
    "branch",
    "max_tasks",
    "max_applies",
];

/// `auto`'s keys, in order.
const AUTO_KEYS: [&str; 17] = [
    "state",
    "reason",
    "why",
    "surface",
    "rule",
    "mode",
    "zones",
    "exclude_zones",
    "exclude_kinds",
    "branch",
    "max_tasks",
    "max_applies",
    "logins",
    "stop_file",
    "stopped",
    "applies_today",
    "gate",
];

fn keys(value: &Value) -> Vec<String> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("an object: {value}"))
        .keys()
        .cloned()
        .collect()
}

fn sorted(list: &[&str]) -> Vec<String> {
    let mut list: Vec<String> = list.iter().map(|k| (*k).to_owned()).collect();
    list.sort();
    list
}

/// RL1's `show`: no table, `off`, `why` `no [auto] table in
/// specengine.toml@HEAD`, the rule's keys `null`. M: a missing table read
/// as `mode = "both"`.
#[test]
fn rl1_no_table_shows_off() {
    let auto = Auto::new("as-off");
    let show = auto.json(&["auto", "show"], 0);
    assert_eq!(
        keys(&show),
        sorted(&["auto", "packs", "proposals", "notes"])
    );
    let head = &show["auto"];
    assert_eq!(keys(head), sorted(&AUTO_KEYS));
    assert_eq!(head["state"], "off");
    assert_eq!(head["reason"], "off");
    assert_eq!(head["why"], "no [auto] table in specengine.toml@HEAD");
    assert_eq!(head["surface"], "local");
    for key in RULE_KEYS {
        assert_eq!(head[key], Value::Null, "{key}");
    }
    assert_eq!(head["stopped"], false);
    assert_eq!(head["applies_today"], Value::Null);
    assert_eq!(head["gate"], "observe");
    assert_eq!(
        head["stop_file"],
        auto.stop_file().to_str().unwrap(),
        "the data directory's stop file"
    );
    assert_eq!(show["packs"], json!([]));
    assert_eq!(show["proposals"], json!([]));
    let text = auto.spec(&["auto", "show"]);
    text.code(0);
    assert_eq!(
        text.stdout.lines().next(),
        Some("auto: off (off): no [auto] table in specengine.toml@HEAD"),
        "{}",
        text.show()
    );
    assert!(!auto.stop_file().exists());
    assert!(auto.auto_events().is_empty(), "show writes nothing");
}

/// The rule's head, on: the hash the core reads, the keys, the gate's note.
#[test]
fn the_head_of_an_on_rule() {
    let auto = Auto::with_rule("as-on", "on.toml");
    let rule = auto.head_rule();
    let show = auto.json(&["auto", "show"], 0);
    let head = &show["auto"];
    assert_eq!(head["state"], "on");
    assert_eq!(head["reason"], Value::Null);
    assert_eq!(head["why"], Value::Null);
    assert_eq!(head["rule"], rule.hash.as_str());
    assert_eq!(head["mode"], "both");
    assert_eq!(head["zones"], json!(["code", "docs", "tests"]));
    assert_eq!(head["exclude_zones"], json!(["narrative"]));
    assert_eq!(head["exclude_kinds"], json!(["scenario"]));
    assert_eq!(head["branch"], "auto");
    assert_eq!(head["max_tasks"], 10);
    assert_eq!(head["max_applies"], 20);
    assert_eq!(head["logins"], 0);
    assert_eq!(head["gate"], "observe");
    assert_eq!(
        show["notes"],
        json!([
            "[gate] mode is observe: an agent's uncovered write passes; spec feed --needs-me lists them"
        ])
    );
    let text = auto.spec(&["auto", "show"]);
    text.code(0);
    let lines: Vec<&str> = text.stdout.lines().collect();
    let stop = auto.stop_file();
    assert_eq!(
        lines[..4],
        [
            format!("auto: on (both), rule {}", rule.hash).as_str(),
            "  zones: code, docs, tests; excluded zones: narrative; excluded kinds: scenario; branch: auto; max_tasks 10; max_applies 20",
            "team: none",
            format!("stop file: {} absent", stop.display()).as_str(),
        ],
        "{}",
        text.show()
    );
    assert!(
        lines.contains(
            &"note: [gate] mode is observe: an agent's uncovered write passes; spec feed --needs-me lists them"
        ),
        "{}",
        text.show()
    );
}

/// One proposal row of `show --json`'s `proposals`, by ID.
fn row<'a>(show: &'a Value, id: &str) -> &'a Value {
    show["proposals"]
        .as_array()
        .expect("proposals")
        .iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("no row {id}: {show}"))
}

fn verdict(show: &Value, id: &str) -> (String, String, String) {
    let row = row(show, id);
    (
        row["verdict"].as_str().unwrap_or_default().to_owned(),
        row["reason"].as_str().unwrap_or_default().to_owned(),
        row["detail"].as_str().unwrap_or_default().to_owned(),
    )
}

/// RL2 through `show`: a scripted queue on `rules/on.toml`, each row its
/// reason and detail; `--surface hub` `author`. M: any one check removed.
#[test]
fn rl2_show_lists_each_rows_reason() {
    let auto = Auto::with_rule("as-rl2", "on.toml");
    let main = auto.main.clone();
    let task = auto.make_task(&["R-02"], "Fix the door timer");
    let pack = auto.record_pack(&[&task], &[], None, &utc(-60));
    let bound = ["--task", task.as_str()];

    // apply: an update of R-02 bound to the pack's task.
    let applies = auto.propose_update(&main, "R-02", "five seconds", "six seconds", &bound);
    // proposal-kind: a question.
    let asked = auto.spec(&[
        "--json",
        "propose",
        "question",
        "R-02",
        "--text",
        "Is six seconds right?",
        "--working-answer",
        "Six.",
        "--price-of-other",
        "A test edit.",
        "--task",
        &task,
    ]);
    asked.code(0);
    let question = asked.json()["id"].as_str().expect("an ID").to_owned();
    // staged: an update of R-01 staged as a reject.
    let staged = auto.propose_update(&main, "R-01", "three save slots", "four save slots", &bound);
    {
        let mut queue = auto.store();
        let seen = queue.get(&staged).unwrap().unwrap().seen();
        queue
            .stage_from(
                &staged,
                &seen,
                &Stage::Reject {
                    reason: "Not now.".to_owned(),
                },
                &utc(-30),
            )
            .expect("staged");
    }
    // excluded-zone: SC-001 in the narrative.
    let narrative = auto.propose_update(&main, "SC-001", "at dusk", "at dawn", &bound);
    // excluded-kind: a new SC section in the covered spec file.
    let base = auto.span_hash(&main, "R-02");
    let text = format!(
        "{}\n\n### A scene {{#SC-002}}\n\nThe door creaks.\n",
        auto.span_text(&main, "R-02").trim_end()
    );
    let created = auto.piped(
        &main,
        &[
            "--json",
            "propose",
            "create",
            "R-02",
            "--base",
            &base,
            "--text-file",
            "-",
            "--rationale",
            "A scene.",
            "--task",
            &task,
        ],
        text.as_bytes(),
    );
    created.code(0);
    let scene = created.json()["id"].as_str().expect("an ID").to_owned();
    // uncovered: R-03 in notes/, no zone.
    let uncovered = auto.propose_update(
        &main,
        "R-03",
        "colour of the hour",
        "colour of the night",
        &bound,
    );
    // findings: a dangling mention stored at creation.
    let dangling = auto.propose_update(
        &main,
        "R-01",
        "the time of its last save.",
        "the time of its last save (see R-99).",
        &bound,
    );
    // not-in-pack: bound to no task.
    let loose = auto.propose_update(&main, "R-02", "starts again", "starts over", &[]);

    let show = auto.json(&["auto", "show"], 0);
    let local = |id: &str| verdict(&show, id);
    assert_eq!(
        local(&applies),
        ("apply".into(), String::new(), String::new())
    );
    assert_eq!(
        local(&question),
        (
            "hold".into(),
            "proposal-kind".into(),
            "a question waits for a person".into()
        )
    );
    let (v, reason, detail) = local(&staged);
    assert_eq!((v.as_str(), reason.as_str()), ("hold", "staged"));
    assert!(
        detail.starts_with(&format!("`{staged}` has a reject staged at "))
            && detail.ends_with(": a person is deciding"),
        "{detail}"
    );
    assert_eq!(
        local(&narrative),
        (
            "hold".into(),
            "excluded-zone".into(),
            "`docs/narrative/intro.md` is in the excluded zone `narrative`".into()
        )
    );
    assert_eq!(
        local(&scene),
        (
            "hold".into(),
            "excluded-kind".into(),
            "`SC-002` is a scenario, an excluded kind".into()
        )
    );
    assert_eq!(
        local(&uncovered),
        (
            "hold".into(),
            "uncovered".into(),
            "`notes/ideas.md` is in no zone the rule covers (code, docs, tests)".into()
        )
    );
    let (v, reason, detail) = local(&dangling);
    assert_eq!(
        (v.as_str(), reason.as_str()),
        ("hold", "findings"),
        "{detail}"
    );
    assert!(
        detail.contains("finding(s) stored at creation, the first ")
            && detail.contains(" at docs/spec/core.md:"),
        "{detail}"
    );
    assert_eq!(
        local(&loose),
        (
            "hold".into(),
            "not-in-pack".into(),
            format!("`{loose}` is bound to no live pack's task")
        )
    );
    assert_eq!(row(&show, &applies)["task_id"], task.as_str());
    assert_eq!(row(&show, &applies)["kind"], "update");
    assert_eq!(row(&show, &loose)["task_id"], Value::Null);
    // Rows by ID number.
    let ids: Vec<&str> = show["proposals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    let mut by_number = ids.clone();
    by_number.sort_by_key(|id| id[3..].parse::<u64>().unwrap());
    assert_eq!(ids, by_number);
    // The pack's item.
    let packs = show["packs"].as_array().expect("packs");
    assert_eq!(packs.len(), 1, "{show}");
    assert_eq!(packs[0]["pack"], pack);
    assert_eq!(packs[0]["items"][0]["item"], task.as_str());
    assert_eq!(packs[0]["items"][0]["status"], "draft");
    assert_eq!(packs[0]["items"][0]["act"], "plan");

    // The text form of a row.
    let text = auto.spec(&["auto", "show"]);
    text.code(0);
    assert!(
        text.stdout
            .lines()
            .any(|line| line == format!("  {question} | question | {task} | hold | proposal-kind: a question waits for a person")),
        "{}",
        text.show()
    );
    assert!(
        text.stdout
            .lines()
            .any(|line| line == format!("  {applies} | update | {task} | apply | -")),
        "{}",
        text.show()
    );
    assert!(text.stdout.lines().any(|line| line == "proposals:"));

    // rebases: R-02's span changed and committed after the update was raised.
    // The heading changes, two lines apart from the committed edit: a
    // clean rebase.
    let rebased = auto.propose_update(&main, "R-02", "## Door timer", "## The door timer", &bound);
    common::replace(
        &main,
        "docs/spec/core.md",
        "The timer starts again when the door opens.",
        "The timer starts again when the door opens wide.",
    );
    // Committed (an uncommitted file is no preview at all: `gone`).
    auto.commit_all(&main, "R-02 moves on");
    let show = auto.json(&["auto", "show"], 0);
    assert_eq!(
        verdict(&show, &rebased),
        (
            "hold".into(),
            "rebases".into(),
            "its span changed since its base: it would rebase".into()
        )
    );

    // --surface hub: no [team] login is the switch; one owner, `author`.
    let hub = auto.json(&["auto", "show", "--surface", "hub"], 0);
    assert_eq!(hub["auto"]["state"], "off");
    assert_eq!(hub["auto"]["reason"], "team");
    assert_eq!(
        hub["auto"]["why"],
        "[team] names no login: a hub delegates only to its sole owner"
    );
    assert!(
        auto.auto_events().iter().all(|e| e.kind == "auto.started"),
        "show logs nothing"
    );
}

/// RL2 on a hub: `author` and `cap` (`max_applies = 1`, one delegated
/// apply today).
#[test]
fn rl2_show_on_a_hub_author_and_cap() {
    let auto = Auto::new("as-hub");
    auto.commit_table(
        "[auto]\nmode        = \"hub\"\nzones       = [\"docs\"]\nexclude_zones = [\"narrative\"]\nmax_applies = 1\n",
        "a hub rule",
    );
    auto.commit_team(&[("ann", "owner")]);
    let main = auto.main.clone();
    // A person's update: `author`.
    let by_person = auto.propose_update(&main, "R-02", "five seconds", "six seconds", &[]);
    // An agent's update raised as `ann` would be over /mcp: through the
    // store, with the login.
    let hub = auto.json(&["auto", "show", "--surface", "hub"], 0);
    assert_eq!(hub["auto"]["state"], "on", "{hub}");
    assert_eq!(hub["auto"]["applies_today"], 0);
    assert_eq!(hub["auto"]["logins"], 1);
    assert_eq!(
        verdict(&hub, &by_person),
        (
            "hold".into(),
            "author".into(),
            format!("`{by_person}` was not raised over /mcp by `ann`")
        )
    );
    // One delegated apply today: through the store's by-rule handle.
    {
        let mut queue = auto.store();
        let rule = auto.head_rule();
        queue.set_by_rule(Some(specengine_store::ByRule {
            rule: rule.hash.clone(),
            pack: None,
        }));
        let now = utc(0);
        let decision = specengine_store::Decision {
            decided_by: "ann <ann@example.org>".to_owned(),
            note: None,
            staged_at: None,
        };
        queue
            .approve(&by_person, &decision, &now)
            .expect("approved");
        queue
            .applied(&by_person, &"c".repeat(40), &now)
            .expect("applied");
    }
    let hub = auto.json(&["auto", "show", "--surface", "hub"], 0);
    assert_eq!(hub["auto"]["applies_today"], 1, "{hub}");
    // The local surface: a hub rule is off on a terminal.
    let local = auto.json(&["auto", "show"], 0);
    assert_eq!(local["auto"]["state"], "off");
    assert_eq!(
        local["auto"]["why"],
        "[auto] mode is hub: a terminal acts only under local or both"
    );
    assert_eq!(local["auto"]["applies_today"], Value::Null);
}

/// RL3: `HEAD`, not the tree. M: reading the working tree; reading the
/// current worktree's `HEAD`.
#[test]
fn rl3_head_not_the_tree() {
    let auto = Auto::with_rule("as-rl3", "on.toml");
    let main = auto.main.clone();
    let task = auto.make_task(&["R-02"], "Fix the door timer");
    auto.record_pack(&[&task], &[], None, &utc(-60));
    let id = auto.propose_update(
        &main,
        "R-01",
        "three save slots",
        "four save slots",
        &["--task", &task],
    );
    let before = auto.json(&["auto", "show"], 0);
    assert_eq!(verdict(&before, &id).0, "apply", "{before}");
    let rule = before["auto"]["rule"].clone();

    // The working tree narrows the rule: nothing changes but a note.
    let config = auto.config();
    let narrowed = config.replacen(
        "zones         = [\"code\", \"tests\", \"docs\"]",
        "zones         = [\"code\", \"tests\"]",
        1,
    );
    assert_ne!(narrowed, config);
    auto.write_config(&narrowed);
    let dirty = auto.json(&["auto", "show"], 0);
    assert_eq!(verdict(&dirty, &id).0, "apply");
    assert_eq!(dirty["auto"]["rule"], rule);
    assert!(
        dirty["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|note| note
                == "specengine.toml differs from HEAD: auto reads HEAD's; commit it to change the rule"),
        "{dirty}"
    );
    assert!(
        !before["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|note| note.as_str().unwrap().contains("differs from HEAD")),
        "{before}"
    );

    // From the linked worktree on `t1` (its HEAD has no table): the main
    // worktree's HEAD decides.
    let linked = auto.run(&auto.linked, &["--json", "auto", "show"], &[]);
    linked.code(0);
    assert_eq!(linked.json()["auto"]["state"], "on", "{}", linked.show());
    assert_eq!(linked.json()["auto"]["rule"], rule);

    // Committed: the docs proposal is uncovered.
    auto.commit_all(&main, "narrow the rule");
    let after = auto.json(&["auto", "show"], 0);
    let (v, reason, detail) = verdict(&after, &id);
    assert_eq!(
        (v.as_str(), reason.as_str()),
        ("hold", "uncovered"),
        "{after}"
    );
    assert_eq!(
        detail,
        "`docs/spec/core.md` is in no zone the rule covers (code, tests) (the rule at HEAD)"
    );
}

/// RL4: one login, on; two, `team` and every item held `team`; a hub's
/// sole lead. M: `> 2`.
#[test]
fn rl4_a_team_turns_it_off() {
    let auto = Auto::with_rule("as-rl4", "on.toml");
    auto.commit_team(&[("ann", "owner")]);
    let one = auto.json(&["auto", "show"], 0);
    assert_eq!(one["auto"]["state"], "on");
    assert_eq!(one["auto"]["logins"], 1);
    let task = auto.make_task(&["R-02"], "Fix the door timer");
    auto.record_pack(&[&task], &[], None, &utc(-60));
    let id = auto.propose_update(&auto.main, "R-02", "five", "six", &["--task", &task]);

    let auto2 = Auto::with_rule("as-rl4-two", "on.toml");
    auto2.commit_team(&[("ann", "owner"), ("mira", "lead")]);
    let task2 = auto2.make_task(&["R-02"], "Fix the door timer");
    auto2.record_pack(&[&task2], &[], None, &utc(-60));
    let id2 = auto2.propose_update(&auto2.main, "R-02", "five", "six", &["--task", &task2]);
    let two = auto2.json(&["auto", "show"], 0);
    assert_eq!(two["auto"]["state"], "off");
    assert_eq!(two["auto"]["reason"], "team");
    assert_eq!(
        two["auto"]["why"],
        "[team] names 2 logins: auto stops at two"
    );
    assert_eq!(
        verdict(&two, &id2),
        (
            "hold".into(),
            "team".into(),
            "[team] names 2 logins: auto stops at two".into()
        )
    );
    let item = &two["packs"][0]["items"][0];
    assert_eq!(item["verdict"], "hold", "{two}");
    assert_eq!(item["reason"], "team");
    let text = auto2.spec(&["auto", "show"]);
    assert_eq!(
        text.stdout.lines().next(),
        Some("auto: off (team): [team] names 2 logins: auto stops at two")
    );
    assert!(text.stdout.contains("team: 2 login(s)"), "{}", text.show());

    // A hub's sole lead.
    let auto3 = Auto::with_rule("as-rl4-lead", "on.toml");
    auto3.commit_team(&[("mira", "lead")]);
    let lead = auto3.json(&["auto", "show", "--surface", "hub"], 0);
    assert_eq!(lead["auto"]["reason"], "team");
    assert_eq!(
        lead["auto"]["why"],
        "the sole [team] login `mira` is a lead: a hub delegates only to the owner"
    );
    let _ = id;
}

/// RL5: each `rules/bad-*.toml` gives `off` and exactly one note at its
/// pinned line; `show R-01`, `check` and `tree` exit as without the table.
/// M: the reader's error propagated (exit 2).
#[test]
fn rl5_a_malformed_table_never_fails() {
    let pinned = expected();
    let pinned = pinned["bad_rules"].as_object().expect("bad_rules").clone();
    let auto = Auto::new("as-rl5");
    let base = auto.config();
    let base_lines = base.trim_end().lines().count();
    let mut failures = Vec::new();
    for (name, want) in &pinned {
        let table = rule_text(name);
        // The same file without its `[auto]` part: what follows it.
        let rest = table
            .find("\n[zones")
            .map_or(String::new(), |at| table[at + 1..].to_owned());
        auto.write_config(&with_table(&base, &rest));
        auto.commit_all(&auto.main, &format!("without {name}"));
        let without: Vec<(i32, String)> = [vec!["show", "R-01"], vec!["check"], vec!["tree"]]
            .iter()
            .map(|args| {
                let run = auto.spec(args);
                (run.code, run.stdout)
            })
            .collect();
        auto.write_config(&with_table(&base, &table));
        auto.commit_all(&auto.main, name);
        let show = auto.spec(&["--json", "auto", "show"]);
        if show.code != 0 {
            failures.push(format!("{name}: {}", show.show()));
            continue;
        }
        let show = show.json();
        let line = base_lines + 1 + want["line"].as_u64().unwrap() as usize;
        let notes: Vec<&str> = show["notes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .filter(|note| note.starts_with("specengine.toml@HEAD:"))
            .collect();
        let head = format!("specengine.toml@HEAD:{line}: ");
        let ok = show["auto"]["state"] == "off"
            && show["auto"]["reason"] == "off"
            && notes.len() == 1
            && notes[0].starts_with(&head)
            && notes[0].ends_with("; auto is off")
            && want["problem"]
                .as_str()
                .is_none_or(|problem| notes[0].starts_with(&format!("{head}{problem}")));
        if !ok {
            failures.push(format!("{name}: {show}"));
        }
        for (args, (code, stdout)) in [vec!["show", "R-01"], vec!["check"], vec!["tree"]]
            .iter()
            .zip(&without)
        {
            let run = auto.spec(args);
            if run.code != *code || (args[0] != "check" && run.stdout != *stdout) {
                failures.push(format!("{name}: spec {args:?}: {}", run.show()));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// RL10: `check` and `show R-01` exit 0 with `rules/on.toml` committed.
/// M: `auto` left out of `RawFile` (exit 2 naming the key).
#[test]
fn rl10_the_config_accepts_the_table() {
    let auto = Auto::with_rule("as-rl10", "on.toml");
    auto.spec(&["check"]).code(0);
    auto.spec(&["show", "R-01"]).code(0);
    auto.spec(&["tree"]).code(0);
}

/// RL9: stop makes the file, show says `stopped`; again unchanged; resume
/// on a pipe exits 2 and keeps it; on a pty `n` keeps it, `y` removes it.
/// M: `resume` without the terminal check.
#[test]
fn rl9_stop_and_resume() {
    let auto = Auto::with_rule("as-rl9", "on.toml");
    let stop = auto.stop_file();
    let path = stop.to_str().unwrap().to_owned();
    let run = auto.spec(&["auto", "stop"]);
    run.code(0);
    assert_eq!(run.stdout, format!("stopped: {path}\n"));
    let first = fs::read_to_string(&stop).expect("the stop file");
    assert!(
        first.starts_with("stopped ")
            && first.ends_with(&format!(" by {}\n", auto_common::COMMITTER)),
        "{first:?}"
    );
    assert_eq!(first.lines().count(), 1);
    let show = auto.json(&["auto", "show"], 0);
    assert_eq!(show["auto"]["state"], "off");
    assert_eq!(show["auto"]["reason"], "stopped");
    assert_eq!(
        show["auto"]["why"],
        format!("the stop file {path} exists: spec auto resume removes it")
    );
    assert_eq!(show["auto"]["stopped"], true);

    // Again: exit 0, unchanged.
    let again = auto.spec(&["auto", "stop"]);
    again.code(0);
    assert_eq!(
        again.stdout,
        format!("stopped: {path} (since {})\n", first.trim_end())
    );
    assert_eq!(fs::read_to_string(&stop).unwrap(), first);
    let json = auto.json(&["auto", "stop"], 0);
    assert_eq!(json["path"], path.as_str());
    assert_eq!(json["stopped"], true);

    // resume on a pipe: exit 2 before anything is read, the file kept.
    let piped = auto.piped(&auto.main, &["auto", "resume"], b"y\n");
    piped.code(2);
    assert!(piped.stderr.contains("terminal"), "{}", piped.show());
    assert!(stop.exists());
    // On a terminal: `n` keeps it, exit 1; `y` removes it.
    let no = auto.pty(&auto.main, &["auto", "resume"], Some("n"), &[]);
    no.code(1);
    assert!(no.asked);
    assert!(
        no.output.contains(&format!(
            "resume auto mode on this machine, removing {path} ({})? [y/N]",
            first.trim_end()
        )),
        "{}",
        no.output
    );
    assert!(
        no.output
            .contains("not resumed: the answer was not `y`; nothing changed"),
        "{}",
        no.output
    );
    assert!(stop.exists());
    let yes = auto.pty(&auto.main, &["auto", "resume"], Some("y"), &[]);
    yes.code(0);
    assert!(
        yes.output.contains(&format!("resumed: {path} removed")),
        "{}",
        yes.output
    );
    assert!(!stop.exists());
    // Absent: exit 1.
    let absent = auto.pty(&auto.main, &["auto", "resume"], None, &[]);
    absent.code(1);
    assert!(!absent.asked);
    assert!(
        absent
            .output
            .contains(&format!("not stopped: {path} is absent; nothing changed")),
        "{}",
        absent.output
    );
    let show = auto.json(&["auto", "show"], 0);
    assert_eq!(show["auto"]["state"], "on");
}

/// `spec auto stop` needs no project: only `HOME`'s data directory.
#[test]
fn stop_needs_no_project() {
    let auto = Auto::new("as-noproject");
    let elsewhere = auto.scratch.dir("elsewhere");
    let run = auto.run(&elsewhere, &["auto", "stop"], &[]);
    run.code(0);
    assert!(auto.stop_file().exists());
    let _ = OsStr::new("");
}
