//! docs/features/auto-land.md LD1 (`spec auto start` asks once, on a
//! terminal) and LD6's `start` (no rule, no auto), over scratch copies of
//! `fixtures/auto-mini` with `rules/on.toml` committed; the terminal is a
//! pseudo-terminal (`script`).
//!
//! Mutation: accepting a pipe.

#![cfg(unix)]

mod auto_common;
mod common;

use std::fs;

use auto_common::{Auto, COMMITTER};
use serde_json::{Value, json};

const PIPE_REFUSAL: &str = "spec: `spec auto start` asks the owner for consent on a terminal, and stdin is not one (a pipe, a script or an agent's shell): run it in a terminal; nothing changed\n";

/// LD1: a pipe is refused before anything; `n` changes nothing; `y`
/// records `auto.started` with the pinned payload and makes the branch at
/// `HEAD`. M: accepting a pipe.
#[test]
fn ld1_start_asks_once_on_a_terminal() {
    let auto = Auto::with_rule("ld1", "on.toml");
    let review = auto.planned(&["R-02"], "Add the save slot list", &[]);
    let draft = auto.make_task(&["SC-001"], "Fix the door timer");
    let lines = auto.scratch.join("lines.txt");
    fs::write(
        &lines,
        "# the night's lines\n\n  Make the inventory sortable by weight  \n",
    )
    .unwrap();
    let lines = lines.to_str().unwrap().to_owned();
    let args = [
        "auto",
        "start",
        review.as_str(),
        draft.as_str(),
        "--lines",
        lines.as_str(),
        "--until",
        "2099-11-16T07:00:00Z",
    ];
    let refs = auto.refs();
    let events = auto.raw_events();

    // A pipe: exit 2, the message, nothing read or written.
    let piped = auto.piped(&auto.main, &args, b"y\n");
    piped.code(2);
    assert_eq!(piped.stdout, "");
    assert_eq!(piped.stderr, PIPE_REFUSAL);
    assert_eq!(auto.raw_events(), events);
    assert_eq!(auto.refs(), refs, "no branch");

    // `n`: exit 1, nothing.
    let no = auto.pty(&auto.main, &args, Some("n"), &[]);
    no.code(1);
    assert!(no.asked, "{}", no.output);
    let rule = auto.head_rule();
    let head = auto.rev(&auto.main, "HEAD");
    for want in [
        format!(
            "auto pack: 2 task(s), 1 line(s); rule {}; on auto (new at {}); until 2099-11-16T07:00:00Z",
            rule.hash,
            &head[..12]
        ),
        format!("  {review} | review | Add the save slot list | apply"),
        format!(
            "  {draft} | draft | Fix the door timer | hold (excluded-zone): `docs/narrative/intro.md` is in the excluded zone `narrative`"
        ),
        "  line 1 | Make the inventory sortable by weight".to_owned(),
        "note: [gate] mode is observe: an agent's uncovered write passes; spec feed --needs-me lists them".to_owned(),
        "start this pack: its tasks approved and landed on `auto` by the rule, with no further key? [y/N]".to_owned(),
        "not started: the answer was not `y`; nothing changed".to_owned(),
    ] {
        assert!(no.output.contains(&want), "no {want:?} in:\n{}", no.output);
    }
    assert_eq!(auto.raw_events(), events);
    assert_eq!(auto.refs(), refs);

    // `y`: the event and the branch.
    let yes = auto.pty(&auto.main, &args, Some("y"), &[]);
    yes.code(0);
    let started = auto.events_of_kind("auto.started");
    assert_eq!(started.len(), 1);
    let pack = started[0].seq;
    assert!(
        yes.output.contains(&format!(
            "started pack {pack}: 2 task(s), 1 line(s) on auto"
        )),
        "{}",
        yes.output
    );
    let frozen: Value = serde_json::from_str(&rule.canonical).unwrap();
    assert_eq!(
        started[0].payload,
        format!(
            "{{\"repository\":{},\"tasks\":[\"{review}\",\"{draft}\"],\"lines\":[\"Make the inventory sortable by weight\"],\"until\":\"2099-11-16T07:00:00Z\",\"branch\":\"auto\",\"branch_at\":\"{head}\",\"rule\":\"{}\",\"frozen\":{},\"by\":\"{COMMITTER}\"}}",
            serde_json::to_string(&auto.repository()).unwrap(),
            rule.hash,
            rule.canonical
        ),
        "the pinned payload"
    );
    assert_eq!(started[0].json()["frozen"], frozen);
    assert_eq!(
        auto.refs().get("refs/heads/auto"),
        Some(&head),
        "refs/heads/auto made at HEAD"
    );
    assert_eq!(auto.raw_events().len(), events.len() + 1, "one event");
}

/// LD1's `--json` and the refusals: a done task, a task of a live pack,
/// eleven items under `max_tasks = 10`, a bad `--until`, no task.
#[test]
fn ld1_start_refuses_before_asking() {
    let auto = Auto::with_rule("ld1-refuse", "on.toml");
    let first = auto.planned(&["R-02"], "First", &[]);
    let json = auto.pty(
        &auto.main,
        &["--json", "auto", "start", &first],
        Some("y"),
        &[],
    );
    json.code(0);
    let pack = auto.events_of_kind("auto.started")[0].seq;
    let document = json
        .output
        .lines()
        .find(|line| line.starts_with('{'))
        .unwrap_or_else(|| panic!("no JSON in:\n{}", json.output));
    let document: Value = serde_json::from_str(document).unwrap();
    assert_eq!(document["pack"], pack);
    assert_eq!(document["tasks"], json!([first]));
    assert_eq!(document["lines"], json!([]));
    assert_eq!(document["branch"], "auto");
    assert_eq!(document["until"], Value::Null);
    assert_eq!(document["rule"], auto.head_rule().hash.as_str());

    let refused = |args: &[&str], want: &str| {
        let events = auto.raw_events();
        let mut all = vec!["auto", "start"];
        all.extend_from_slice(args);
        let run = auto.pty(&auto.main, &all, Some("y"), &[]);
        run.code(1);
        assert!(!run.asked, "{args:?}: asked:\n{}", run.output);
        assert!(
            run.output.contains(want),
            "{args:?}: no {want:?} in:\n{}",
            run.output
        );
        assert_eq!(auto.raw_events(), events, "{args:?}: nothing written");
    };
    // A task of a live pack.
    refused(
        &[&first],
        &format!("{first} is in live pack {pack}; nothing started"),
    );
    // Eleven items.
    let mut many = Vec::new();
    for k in 0..11 {
        many.push(auto.make_task(&["R-01"], &format!("Item {k}")));
    }
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    refused(
        &many,
        "the pack holds 11 items; the rule's max_tasks is 10; nothing started",
    );
    // A done task.
    let done_auto = Auto::with_rule("ld1-done", "on.toml");
    let done = done_auto.planned(&["R-02"], "Done already", &[]);
    let done_pack = done_auto.pack(&[&done], &[]);
    done_auto.to_done(&done, &[]);
    let _ = done_pack;
    let fresh = done_auto.raw_events();
    let run = done_auto.pty(&done_auto.main, &["auto", "start", &done], Some("y"), &[]);
    run.code(1);
    assert!(
        run.output.contains(&format!(
            "{done} is done: a pack takes draft, review, changes_requested or ready; nothing started"
        )),
        "{}",
        run.output
    );
    assert_eq!(done_auto.raw_events(), fresh);
    // Exit 2: no task nor --lines; a past --until.
    let other = auto.make_task(&["R-01"], "Other");
    let events = auto.raw_events();
    let none = auto.pty(&auto.main, &["auto", "start"], Some("y"), &[]);
    none.code(2);
    let past = auto.pty(
        &auto.main,
        &["auto", "start", &other, "--until", "2020-01-01T00:00:00Z"],
        Some("y"),
        &[],
    );
    past.code(2);
    assert!(past.output.contains("--until: "), "{}", past.output);
    assert_eq!(auto.raw_events(), events);
}

/// LD6's `start`: no table, exit 1 `auto is off: …`, nothing written.
#[test]
fn ld6_start_with_no_rule_is_off() {
    let auto = Auto::new("ld6-start");
    let task = auto.planned(&["R-02"], "No rule", &[]);
    let refs = auto.refs();
    let events = auto.raw_events();
    let run = auto.pty(&auto.main, &["auto", "start", &task], Some("y"), &[]);
    run.code(1);
    assert!(!run.asked);
    assert!(
        run.output
            .contains("auto is off: no [auto] table in specengine.toml@HEAD"),
        "{}",
        run.output
    );
    assert_eq!(auto.raw_events(), events);
    assert_eq!(auto.refs(), refs);
}

/// Lane A's decision: the listing names the rule's notes (an uncommitted
/// config: `specengine.toml differs from HEAD: …`).
#[test]
fn ld1_start_lists_the_rules_notes() {
    let auto = Auto::with_rule("ld1-notes", "on.toml");
    let task = auto.planned(&["R-02"], "Noted", &[]);
    auto.write_config(&format!("{}# an edit not committed\n", auto.config()));
    let run = auto.pty(&auto.main, &["auto", "start", &task], Some("n"), &[]);
    run.code(1);
    assert!(
        run.output.contains(
            "note: specengine.toml differs from HEAD: auto reads HEAD's; commit it to change the rule"
        ),
        "{}",
        run.output
    );
}
