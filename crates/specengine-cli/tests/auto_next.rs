//! docs/features/auto-land.md LD10 (`spec auto next` is one function of
//! the state) and LD11's `next` and `hold` (holds finish an item), over
//! scratch copies of `fixtures/auto-mini` with `rules/on.toml` and a
//! `maps` workspace (a module rooted at R-01) committed; packs recorded
//! through the store, states moved through the binary.
//!
//! Mutations: the overlap ignored; `next` returning held items.

#![cfg(unix)]

mod auto_common;
mod common;

use auto_common::Auto;
use serde_json::Value;

const MAPS: &str = "[[workspace]]\nname   = \"maps\"\nmodule = \"maps\"\n\n[[module]]\nname = \"maps\"\nroot = \"R-01\"\n";

const NEXT_KEYS: [&str; 12] = [
    "action", "reason", "why", "pack", "item", "task", "line", "text", "title", "lane", "branch",
    "base",
];

fn repo(label: &str) -> Auto {
    let auto = Auto::with_rule(label, "on.toml");
    auto.commit_table(MAPS, "a maps workspace");
    auto
}

fn act(next: &Value) -> (String, String) {
    (
        next["action"].as_str().unwrap_or_default().to_owned(),
        next["item"].as_str().unwrap_or_default().to_owned(),
    )
}

/// LD10: plan, approve, implement, land, done, in turn; the document's
/// keys; calling it twice writes nothing.
#[test]
fn ld10_next_walks_a_task_through_its_acts() {
    let auto = repo("ld10");
    let task = auto.make_task(&["R-02"], "Fix the door timer");
    let pack = auto.pack(&[&task], &[]);

    let next = auto.next(&[]);
    let raw = auto.spec(&["auto", "next"]);
    assert_eq!(
        task_common_keys(&raw.stdout),
        NEXT_KEYS,
        "every key, in order"
    );
    let json = auto.spec(&["--json", "auto", "next"]);
    assert_eq!(json.stdout, raw.stdout, "--json changes nothing");
    assert_eq!(act(&next), ("plan".into(), task.clone()));
    assert_eq!(next["pack"], pack);
    assert_eq!(next["task"], task.as_str());
    assert_eq!(next["title"], "Fix the door timer");
    assert_eq!(next["lane"], "general");
    assert_eq!(next["branch"], Value::Null);
    assert_eq!(next["base"], Value::Null);
    let events = auto.raw_events();
    assert_eq!(auto.next(&[]), next, "twice: the same");
    assert_eq!(auto.raw_events(), events, "a read");

    auto.plan_task(&task, &[]);
    let next = auto.next(&[]);
    assert_eq!(act(&next), ("approve".into(), task.clone()));
    assert_eq!(next["branch"], format!("auto-{task}"));
    assert_eq!(next["base"], "auto");
    auto.auto_approve(&task).code(0);
    let next = auto.next(&[]);
    assert_eq!(act(&next), ("implement".into(), task.clone()));
    assert_eq!(next["branch"], format!("auto-{task}"));
    // Busy: skipped, and running: wait.
    assert_eq!(act(&auto.next(&["--busy", &task])).0, "wait");
    // No slot: implement waits.
    assert_eq!(act(&auto.next(&["--slots", "0"])).0, "wait");
    auto.claim_in(&task);
    assert_eq!(act(&auto.next(&[])).0, "wait", "in progress: running");
    common::replace(
        &auto.task_worktree(&task),
        "src/lib.rs",
        "DOOR_SECONDS: u32 = 5",
        "DOOR_SECONDS: u32 = 6",
    );
    auto.finish(&task, "completed", "Six.");
    let next = auto.next(&["--slots", "0"]);
    assert_eq!(
        act(&next),
        ("land".into(), task.clone()),
        "a land takes no slot"
    );
    auto.land(&task).code(0);
    let next = auto.next(&[]);
    assert_eq!(next["action"], "done");
    for key in &NEXT_KEYS[1..] {
        assert_eq!(next[*key], Value::Null, "{key}");
    }

    // The stop file: stop.
    auto.spec(&["auto", "stop"]).code(0);
    let next = auto.next(&[]);
    assert_eq!(next["action"], "stop");
    assert_eq!(next["reason"], "stopped");
    assert_eq!(
        next["why"],
        format!(
            "the stop file {} exists: spec auto resume removes it",
            auto.stop_file().display()
        )
    );
}

/// The keys of the JSON object on `stdout`, in order.
fn task_common_keys(stdout: &str) -> Vec<String> {
    let value: serde_json::Map<String, Value> =
        serde_json::from_str(stdout.trim_end()).expect("one JSON object");
    // serde_json's map sorts; read the order from the text instead.
    let mut keys: Vec<(usize, String)> = value
        .keys()
        .map(|key| {
            (
                stdout
                    .find(&format!("\"{key}\":"))
                    .unwrap_or_else(|| panic!("{key} in {stdout}")),
                key.clone(),
            )
        })
        .collect();
    keys.sort();
    keys.into_iter().map(|(_, key)| key).collect()
}

/// LD10: a `done` task before a `review` one; two overlapping `ready`
/// tasks one at a time; `--skip-lane maps`; an empty pack `done`. M: the
/// overlap ignored.
#[test]
fn ld10_order_overlap_lanes_and_slots() {
    let auto = repo("ld10-order");
    let review = auto.planned(&["R-02"], "In review", &[]);
    let done = auto.planned(&["R-02"], "Done first", &[]);
    let ready_a = auto.planned(&["R-02"], "Ready A", &[]);
    let ready_b = auto.planned(&["R-02"], "Ready B", &[]);
    let maps = auto.make_task(&["R-01"], "A maps task");
    let pack = auto.pack(&[&review, &done, &ready_a, &ready_b, &maps], &[]);
    auto.to_done(&done, &[("tests/a.rs", "door_open(4)", "door_open(3)")]);
    auto.auto_approve(&ready_a).code(0);
    auto.auto_approve(&ready_b).code(0);

    // A done task first, whatever its order in the pack.
    assert_eq!(act(&auto.next(&[])), ("land".into(), done.clone()));
    auto.land(&done).code(0);
    // Then the review one's approve (overlapping nothing running).
    assert_eq!(act(&auto.next(&[])), ("approve".into(), review.clone()));
    // ready_a busy: the review overlaps it (R-02): passed over; ready_b
    // overlaps it too; maps' plan is next.
    let next = auto.next(&["--busy", &ready_a]);
    assert_eq!(act(&next), ("plan".into(), maps.clone()), "{next}");
    assert_eq!(next["lane"], "maps");
    // Skipping the maps lane as well: wait (passed over only for overlap).
    let next = auto.next(&["--busy", &ready_a, "--skip-lane", "maps"]);
    assert_eq!(next["action"], "wait");
    // ready_a claimed: running; ready_b overlaps it, never alongside.
    auto.claim_in(&ready_a);
    let next = auto.next(&["--skip-lane", "maps"]);
    assert_ne!(act(&next), ("implement".into(), ready_b.clone()), "{next}");
    assert_ne!(
        act(&next).0,
        "approve",
        "the review overlaps a running task: {next}"
    );
    // ready_a held: finished, no longer running; ready_b may go.
    auto.spec(&["auto", "hold", &ready_a, "--reason", "agent-error"])
        .code(0);
    let next = auto.next(&["--skip-lane", "maps"]);
    assert!(
        act(&next) == ("approve".into(), review.clone())
            || act(&next) == ("implement".into(), ready_b.clone()),
        "{next}\n{}",
        auto.spec(&["auto", "show"]).stdout
    );
    // --slots 0: only land or wait.
    let next = auto.next(&["--slots", "0"]);
    assert!(
        ["land", "wait"].contains(&next["action"].as_str().unwrap()),
        "{next}"
    );
    // The held item is never returned again (LD11).
    for args in [vec![], vec!["--skip-lane", "maps"], vec!["--slots", "4"]] {
        assert_ne!(act(&auto.next(&args)).1, ready_a, "{args:?}");
    }
    let _ = pack;

    // An empty pack: done.
    let empty = repo("ld10-empty");
    empty.pack(&[], &["A line held at once"]);
    let line = format!("{}/1", empty.events_of_kind("auto.started")[0].seq);
    empty
        .spec(&["auto", "hold", &line, "--reason", "not-planned"])
        .code(0);
    assert_eq!(empty.next(&[])["action"], "done");
    let none = repo("ld10-none");
    assert_eq!(none.next(&[])["action"], "done");
}

/// LD11: `hold` records `auto.held` `act` `run` with the note, `next`
/// never returns the item; a bad reason exits 2; a task in no pack exits
/// 1. M: `next` returning held items.
#[test]
fn ld11_holds_finish_an_item() {
    let auto = repo("ld11");
    let task = auto.planned(&["R-02"], "Fix the door timer", &[]);
    let other = auto.planned(&["R-01"], "Elsewhere", &[]);
    let pack = auto.pack(&[&task], &["Make the inventory sortable by weight"]);
    let rule = auto.head_rule().hash;
    let run = auto.spec(&[
        "auto",
        "hold",
        &task,
        "--reason",
        "denied-tool",
        "--note",
        "Bash",
    ]);
    run.code(0);
    assert_eq!(run.stdout, format!("held {task} (denied-tool): Bash\n"));
    let held = auto.events_of_kind("auto.held").pop().unwrap();
    assert_eq!(
        held.payload,
        format!(
            "{{\"pack\":{pack},\"task\":\"{task}\",\"line\":null,\"proposal\":null,\"act\":\"run\",\"reason\":\"denied-tool\",\"detail\":\"Bash\",\"rule\":\"{rule}\"}}"
        )
    );
    // The default note is the reason.
    let line = format!("{pack}/1");
    auto.spec(&["auto", "hold", &line, "--reason", "not-planned"])
        .code(0);
    let held = auto.events_of_kind("auto.held").pop().unwrap().json();
    assert_eq!(held["detail"], "not-planned");
    assert_eq!(held["task"], Value::Null);
    assert_eq!(held["line"], 1);
    for _ in 0..2 {
        let next = auto.next(&[]);
        assert_ne!(act(&next).1, task);
        assert_ne!(act(&next).1, line);
    }
    assert_eq!(auto.next(&[])["action"], "done");
    // In a pack still live (another item unfinished), a held `review` task
    // is never offered its approve.
    let held_review = auto.planned(&["R-04"], "Held in review", &[]);
    let draft = auto.make_task(&["R-05"], "Still a draft");
    auto.record_pack(&[&held_review, &draft], &[], None, &auto_common::utc(-30));
    auto.spec(&["auto", "hold", &held_review, "--reason", "agent-error"])
        .code(0);
    let next = auto.next(&[]);
    assert_eq!(act(&next), ("plan".into(), draft.clone()), "{next}");
    // Exit 2: a reason not the runner's; a note too long or with a control.
    let events = auto.raw_events();
    for args in [
        vec!["auto", "hold", task.as_str(), "--reason", "check"],
        vec!["auto", "hold", task.as_str(), "--reason", "bored"],
    ] {
        auto.spec(&args).code(2);
    }
    let long = "x".repeat(1025);
    auto.spec(&[
        "auto",
        "hold",
        &task,
        "--reason",
        "agent-error",
        "--note",
        &long,
    ])
    .code(2);
    auto.spec(&[
        "auto",
        "hold",
        &task,
        "--reason",
        "agent-error",
        "--note",
        "a\u{7}b",
    ])
    .code(2);
    // In no pack: exit 1, no event.
    auto.spec(&["auto", "hold", &other, "--reason", "agent-error"])
        .code(1);
    assert_eq!(auto.raw_events(), events);
}

/// The reviewer's rows on `next`: a `--busy` line alone gives `wait`, not
/// `done`; a pack whose frozen branch is not `HEAD`'s is skipped (`stop`
/// when it is the only one), a later pack on the new branch served.
#[test]
fn ld10_a_busy_line_waits_and_a_pack_of_another_branch_is_skipped() {
    let auto = repo("ld10-line");
    let pack = auto.pack(&[], &["Weigh the items"]);
    let line = format!("{pack}/1");
    let next = auto.next(&[]);
    assert_eq!(act(&next), ("plan-line".into(), line.clone()), "{next}");
    assert_eq!(auto.next(&["--busy", &line])["action"], "wait");

    let auto = repo("ld10-branch");
    let old = auto.make_task(&["R-02"], "Under the old branch");
    auto.pack(&[&old], &[]);
    let config = auto.config();
    auto.write_config(&config.replacen(
        "exclude_kinds = [\"scenario\"]\n",
        "exclude_kinds = [\"scenario\"]\nbranch        = \"night\"\n",
        1,
    ));
    assert!(auto.config().contains("branch        = \"night\""));
    auto.commit_all(&auto.main, "a new branch");
    let next = auto.next(&[]);
    assert_eq!(
        next["action"], "stop",
        "the only pack is another branch's: {next}"
    );
    assert_eq!(next["reason"], "off", "the skipped pack's reason: {next}");
    assert!(
        next["why"]
            .as_str()
            .is_some_and(|why| why.contains("branch changed since pack")),
        "{next}"
    );
    let new = auto.make_task(&["R-04"], "Under the new branch");
    auto.make_branch("night");
    auto.record_pack(&[&new], &[], None, &auto_common::utc(-30));
    let next = auto.next(&[]);
    assert_eq!(act(&next), ("plan".into(), new.clone()), "{next}");
    assert_eq!(next["base"], Value::Null);
}

/// The `until` decision: membership in a pack ignores `until`; past it an
/// approve holds `until` (one `auto.held`), and a land still runs.
#[test]
fn ld10_until_holds_an_approve_and_lets_a_land_run() {
    let auto = repo("ld10-until");
    let landing = auto.planned(&["R-02"], "Lands after the end", &[]);
    let late = auto.planned(&["R-04"], "Approved after the end", &[]);
    let draft = auto.make_task(&["R-05"], "Planned after the end");
    auto.make_branch("auto");
    let until = auto_common::utc(4);
    let pack = auto.record_pack(
        &[&landing, &late, &draft],
        &[],
        Some(&until),
        &auto_common::utc(-60),
    );
    auto.auto_approve(&landing).code(0);
    let wt = auto.claim_in(&landing);
    common::replace(
        &wt,
        "src/lib.rs",
        "DOOR_SECONDS: u32 = 5",
        "DOOR_SECONDS: u32 = 6",
    );
    // Past the end.
    std::thread::sleep(std::time::Duration::from_secs(5));
    auto.finish(&landing, "completed", "Six.");
    let held_before = auto.events_of_kind("auto.held").len();
    let run = auto.auto_approve(&late);
    run.code(1);
    assert!(
        run.stderr.contains(&format!(
            "{late} held (until): pack {pack} ended at {until}; nothing approved"
        )),
        "{}",
        run.show()
    );
    let held = auto.events_of_kind("auto.held");
    assert_eq!(held.len(), held_before + 1);
    assert_eq!(held.last().unwrap().json()["reason"], "until");
    assert_eq!(auto.status(&late), "review");
    // `show` marks the plan `hold | until`; `next` offers only the land.
    let show = auto.json(&["auto", "show"], 0);
    let item = show["packs"][0]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["item"] == draft.as_str())
        .unwrap_or_else(|| panic!("{show}"))
        .clone();
    assert_eq!(
        (
            item["act"].as_str(),
            item["verdict"].as_str(),
            item["reason"].as_str()
        ),
        (Some("plan"), Some("hold"), Some("until")),
        "{item}"
    );
    assert_eq!(act(&auto.next(&[])), ("land".into(), landing.clone()));
    auto.land(&landing).code(0);
    assert_eq!(auto.events_of_kind("auto.landed").len(), 1);
    let next = auto.next(&[]);
    assert_ne!(act(&next).1, draft, "no plan past the end: {next}");
}
