//! docs/features/auto-feed.md FD1 and FD2 (and the CLI half of FD3's
//! body): `spec feed` over `fixtures/auto-mini` with `rules/on.toml`
//! committed and a night scripted through the store's writers at injected
//! clocks (a pack started, a line bound, a task approved by rule, claimed,
//! reported and completed, its update applied by rule, the task landed;
//! another held at its land; a person's apply; an observed gate write; an
//! open question; a failed run; a done task not landed).
//!
//! - FD1: the text lines and JSON entries in `seq` order (an event recorded
//!   later with an earlier `at` stays last), `--since <seq>` and `--since
//!   <time>` cutting alike, `--task` keeping its own and its proposals',
//!   `--department general` its lane's; `by` only where the payload says.
//! - FD2: `--needs-me`'s five lists; a held task landed afterwards leaves
//!   `held`.
//!
//! Mutations: ordering by `at`; `by` copied from a manual apply; `held`
//! taken from any past `auto.held`.

#![cfg(unix)]

mod auto_common;
mod common;
mod task_common;

use auto_common::{Auto, COMMITTER};
use serde_json::{Value, json};
use specengine_core::proposal::Author;
use specengine_model::{RunOutcome, SnapshotPlace, TaskClaim, TaskCriterion};
use specengine_store::{
    AutoBind, AutoEvent, AutoHeld, AutoLanded, ByRule, Decision, GateObserved, NewProposal,
    NewTask, Place, ProposalKind, ProposalQueue as _, SnapshotEntry, SqliteQueue, TaskChange,
    TaskSnapshot, patch_hash, record_gate_observed,
};
use task_common::keys;

const ENTRY_KEYS: [&str; 17] = [
    "seq", "at", "type", "task", "proposal", "card", "line", "title", "status", "by", "outcome",
    "summary", "commit", "reason", "detail", "path", "lane",
];

/// The night's facts the assertions name.
struct Night {
    auto: Auto,
    pack: i64,
    rule: String,
    applied_commit: String,
    manual_commit: String,
    land_commit: String,
    /// `seq` of the landing, the hold and the gate's write.
    landed: i64,
    held: i64,
    observed: i64,
}

fn at(minute: u32, second: u32) -> String {
    format!("2026-11-16T01:{minute:02}:{second:02}Z")
}

fn new_task(repository: &str, title: &str, target: &str) -> NewTask {
    NewTask {
        git_common_dir: repository.to_owned(),
        title: Some(title.to_owned()),
        goal: None,
        targets: vec![target.to_owned()],
        author: Author::human(),
    }
}

fn change(queue: &mut SqliteQueue, id: &str, change: &TaskChange, now: &str) {
    let seen = queue.get_task(id).unwrap().unwrap().seen();
    queue
        .change_task(id, &seen, change, now)
        .unwrap_or_else(|e| panic!("{id}: {e}"));
}

fn plan() -> TaskChange {
    TaskChange::Plan {
        plan: "1. Do it.".to_owned(),
        criteria: vec![TaskCriterion {
            reference: None,
            text: Some("Holds.".to_owned()),
        }],
        affected_nodes: Vec::new(),
    }
}

fn snapshot(worktree: &str) -> TaskSnapshot {
    TaskSnapshot {
        at: at(2, 0),
        by: COMMITTER.to_owned(),
        place: SnapshotPlace {
            worktree: worktree.to_owned(),
            root_rel: String::new(),
            branch: "t1".to_owned(),
            commit: "1".repeat(40),
        },
        nodes: vec![SnapshotEntry {
            id: "R-02".to_owned(),
            path: "docs/spec/core.md".to_owned(),
            span_hash: "b3:02".to_owned(),
            text: "text".to_owned(),
        }],
    }
}

fn claim(worktree: &str, now: &str) -> TaskChange {
    TaskChange::Claim {
        claim: TaskClaim {
            at: now.to_owned(),
            role: "developer".to_owned(),
            worktree: worktree.to_owned(),
            branch: "t1".to_owned(),
        },
        author: Author::new(Some("developer".to_owned()), None, None).expect("a role"),
    }
}

fn report(outcome: RunOutcome, summary: &str) -> TaskChange {
    TaskChange::Report {
        outcome,
        summary: summary.to_owned(),
        changed_files: vec!["src/lib.rs".to_owned()],
    }
}

fn proposal(kind: ProposalKind, target: &str, repository: &str, worktree: &str) -> NewProposal {
    let base_hash = format!("b3:{}", "0".repeat(64));
    NewProposal {
        kind,
        target_id: target.to_owned(),
        target_path: "docs/spec/core.md".to_owned(),
        place: Place {
            git_common_dir: repository.to_owned(),
            worktree: worktree.to_owned(),
            root_rel: String::new(),
            branch: "t1".to_owned(),
            base_commit: "0".repeat(40),
        },
        patch_hash: patch_hash(target, &base_hash, "new"),
        base_hash: Some(base_hash),
        base_text: Some("base".to_owned()),
        new_text: "new".to_owned(),
        rationale: "Why.".to_owned(),
        author: Author::human(),
        diagnostics: Vec::new(),
        new_ids: Vec::new(),
        interpretation: None,
    }
}

fn owner() -> Decision {
    Decision {
        decided_by: COMMITTER.to_owned(),
        note: None,
        staged_at: None,
    }
}

/// The night, scripted through the store.
fn night(label: &str) -> Night {
    let auto = Auto::with_rule(label, "on.toml");
    // The database exists once a read made it.
    auto.spec(&["index"]).code(0);
    let repository = auto.repository();
    let worktree = auto.linked.to_str().unwrap().to_owned();
    let rule = auto.head_rule().hash;
    let mut queue = auto.store();
    let t1 = queue
        .create_task(
            &new_task(&repository, "Fix the door timer", "R-02"),
            &at(0, 1),
        )
        .unwrap()
        .id;
    change(&mut queue, &t1, &plan(), &at(0, 2));
    let t2 = queue
        .create_task(
            &new_task(&repository, "Sort the inventory", "R-01"),
            &at(0, 3),
        )
        .unwrap()
        .id;
    change(&mut queue, &t2, &plan(), &at(0, 4));
    drop(queue);
    let pack = auto.record_pack(
        &[&t1, &t2],
        &["Make the inventory sortable by weight"],
        None,
        &at(1, 0),
    );
    let mut queue = auto.store();
    let (t3, bound) = queue
        .create_task_bound(
            &new_task(&repository, "Weigh the items", "R-01"),
            Some(AutoBind { pack, line: 1 }),
            &at(1, 1),
        )
        .unwrap();
    assert!(matches!(bound, Some(Ok(_))), "{bound:?}");
    let t3 = t3.id;
    let seen = queue.get_task(&t1).unwrap().unwrap().seen();
    queue
        .approve_task_by_rule(&t1, &seen, &snapshot(&worktree), pack, &rule, &at(2, 0))
        .unwrap();
    change(&mut queue, &t1, &claim(&worktree, &at(2, 1)), &at(2, 1));
    let pr1 = queue
        .create_with_task(
            &proposal(ProposalKind::Update, "R-02", &repository, &worktree),
            Some(&t1),
            &at(2, 2),
        )
        .unwrap()
        .id;
    change(
        &mut queue,
        &t1,
        &report(
            RunOutcome::Completed,
            "the timer resets on close\nand nothing else changed",
        ),
        &at(2, 3),
    );
    change(&mut queue, &t1, &TaskChange::Complete, &at(2, 4));
    // Its update, applied by rule after the task is done.
    let applied_commit = "3f2a1c9d0b4e".to_owned() + &"1".repeat(28);
    queue.set_by_rule(Some(ByRule {
        rule: rule.clone(),
        pack: Some(pack),
    }));
    queue.approve(&pr1, &owner(), &at(5, 40)).unwrap();
    queue.applied(&pr1, &applied_commit, &at(5, 40)).unwrap();
    queue.set_by_rule(None);
    let land_commit = "9b1e77c2d0aa".to_owned() + &"2".repeat(28);
    let landed = queue
        .record_auto(
            &AutoEvent::Landed(AutoLanded {
                pack,
                task: t1.clone(),
                branch: "auto".to_owned(),
                commit: land_commit.clone(),
                code_commit: None,
                applied: vec![pr1.clone()],
                left: Vec::new(),
                rule: rule.clone(),
            }),
            &at(6, 2),
        )
        .unwrap();
    // T-0002: approved by rule, done, held at its land.
    let seen = queue.get_task(&t2).unwrap().unwrap().seen();
    queue
        .approve_task_by_rule(&t2, &seen, &snapshot(&worktree), pack, &rule, &at(10, 0))
        .unwrap();
    // A person's apply of an unbound update: no `by`.
    let pr2 = queue
        .create(
            &proposal(ProposalKind::Update, "R-01", &repository, &worktree),
            &at(20, 0),
        )
        .unwrap()
        .id;
    queue.approve(&pr2, &owner(), &at(20, 1)).unwrap();
    let manual_commit = "4".repeat(40);
    queue.applied(&pr2, &manual_commit, &at(20, 1)).unwrap();
    let held = queue
        .record_auto(
            &AutoEvent::Held(AutoHeld {
                pack: Some(pack),
                task: Some(t2.clone()),
                line: None,
                proposal: None,
                act: "land".to_owned(),
                reason: "check".to_owned(),
                detail: "spec check: 1 error(s); the first docs/spec/core.md:12: ref-dangling"
                    .to_owned(),
                rule: rule.clone(),
            }),
            &at(31, 10),
        )
        .unwrap();
    // An open question bound to T-0003.
    let _question = queue
        .create(
            &proposal(ProposalKind::Update, "R-01", &repository, &worktree),
            &at(32, 0),
        )
        .unwrap();
    drop(queue);
    // An observed gate write, recorded later with an earlier `at`.
    let observed_at = at(0, 30);
    assert!(
        record_gate_observed(
            &auto.db(),
            auto_common::SLUG,
            &GateObserved {
                path: "src/x.rs".to_owned(),
                zone: Some("code".to_owned()),
                worktree: worktree.clone(),
                branch: Some("auto-T-0004".to_owned()),
            },
            &observed_at,
            std::time::Instant::now() + std::time::Duration::from_secs(10),
        )
        .unwrap()
    );
    let observed = auto
        .raw_events()
        .iter()
        .find(|e| e.kind == "gate.observed")
        .expect("observed")
        .seq;
    let _ = t3;
    Night {
        auto,
        pack,
        rule,
        applied_commit,
        manual_commit,
        land_commit,
        landed,
        held,
        observed,
    }
}

fn feed(auto: &Auto, args: &[&str]) -> (Value, String) {
    let mut all = vec!["--json", "feed"];
    all.extend_from_slice(args);
    let run = auto.spec(&all);
    run.code(0);
    (run.json(), run.stdout)
}

fn entry(feed: &Value, seq: i64) -> &Value {
    feed["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["seq"] == seq)
        .unwrap_or_else(|| panic!("no entry {seq}: {feed}"))
}

fn seqs(feed: &Value) -> Vec<i64> {
    feed["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["seq"].as_i64().unwrap())
        .collect()
}

/// FD1. M: ordering by `at`; `by` copied from a manual apply.
#[test]
fn fd1_the_feed_lists_the_night_in_seq_order() {
    let night = night("fd1");
    let auto = &night.auto;
    let all = auto.raw_events();
    let (json, raw) = feed(auto, &["--since", "0"]);
    assert_eq!(
        keys(raw.trim_end()),
        [
            "auto", "after", "last_seq", "more", "entries", "needs_me", "notes"
        ]
    );
    assert_eq!(json["after"], 0);
    assert_eq!(json["more"], false);
    assert_eq!(json["needs_me"], Value::Null);
    assert_eq!(json["auto"]["state"], "on");
    assert_eq!(json["auto"]["rule"], night.rule.as_str());
    assert_eq!(
        seqs(&json),
        all.iter().map(|e| e.seq).collect::<Vec<_>>(),
        "every event, by seq (the gate's write last though its `at` is the earliest)"
    );
    assert_eq!(*seqs(&json).last().unwrap(), night.observed);
    assert_eq!(json["last_seq"], night.observed);
    // Each entry's keys in order.
    let first_entry = {
        let start = raw.find("\"entries\":[").unwrap() + "\"entries\":[".len();
        let mut depth = 0;
        let mut end = start;
        for (k, c) in raw[start..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + k + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        raw[start..end].to_owned()
    };
    assert_eq!(keys(&first_entry), ENTRY_KEYS);

    // The run, the delegated apply, the landing, the hold, the gate's write.
    let reported = all
        .iter()
        .find(|e| e.kind == "task.run_reported")
        .unwrap()
        .seq;
    let e = entry(&json, reported);
    assert_eq!(e["task"], "T-0001");
    assert_eq!(e["title"], "Fix the door timer");
    assert_eq!(e["outcome"], "completed");
    assert_eq!(
        e["summary"],
        "the timer resets on close\nand nothing else changed"
    );
    let applied: Vec<i64> = all
        .iter()
        .filter(|e| e.kind == "proposal.applied")
        .map(|e| e.seq)
        .collect();
    let by_rule = entry(&json, applied[0]);
    assert_eq!(by_rule["proposal"], "PR-0001");
    assert_eq!(by_rule["task"], "T-0001");
    assert_eq!(by_rule["title"], "R-02");
    assert_eq!(by_rule["by"], "rule");
    assert_eq!(by_rule["commit"], night.applied_commit.as_str());
    assert_eq!(by_rule["status"], "applied");
    let manual = entry(&json, applied[1]);
    assert_eq!(manual["by"], Value::Null, "a person's apply: {manual}");
    assert_eq!(manual["commit"], night.manual_commit.as_str());
    let approved: Vec<&Value> = all
        .iter()
        .filter(|e| e.kind == "task.approved")
        .map(|e| entry(&json, e.seq))
        .collect();
    assert!(approved.iter().all(|e| e["by"] == "rule"));
    let landed = entry(&json, night.landed);
    assert_eq!(landed["commit"], night.land_commit.as_str());
    assert_eq!(landed["task"], "T-0001");
    let held = entry(&json, night.held);
    assert_eq!(held["reason"], "check");
    assert_eq!(held["task"], "T-0002");
    let gate = entry(&json, night.observed);
    assert_eq!(gate["path"], "src/x.rs");
    assert_eq!(gate["type"], "gate.observed");
    let bound = all.iter().find(|e| e.kind == "auto.bound").unwrap().seq;
    assert_eq!(entry(&json, bound)["line"], format!("{}/1", night.pack));
    assert_eq!(entry(&json, bound)["task"], "T-0003");

    // The text lines.
    let text = auto.spec(&["feed", "--since", "0"]);
    text.code(0);
    let lines: Vec<&str> = text.stdout.lines().collect();
    let want = [
        format!(
            "{reported} | {} | task.run_reported | T-0001 | Fix the door timer | completed: the timer resets on close",
            at(2, 3)
        ),
        format!(
            "{} | {} | proposal.applied | PR-0001 | R-02 | as 3f2a1c9d0b4e by rule",
            applied[0],
            at(5, 40)
        ),
        format!(
            "{} | {} | proposal.applied | PR-0002 | R-01 | as 444444444444",
            applied[1],
            at(20, 1)
        ),
        format!(
            "{} | {} | auto.landed | T-0001 | Fix the door timer | on auto as 9b1e77c2d0aa",
            night.landed,
            at(6, 2)
        ),
        format!(
            "{} | {} | auto.held | T-0002 | Sort the inventory | check: spec check: 1 error(s); the first docs/spec/core.md:12: ref-dangling",
            night.held,
            at(31, 10)
        ),
        format!(
            "{} | {} | auto.started | - | - | 2 task(s), 1 line(s) on auto",
            night.pack,
            at(1, 0)
        ),
        format!(
            "{bound} | {} | auto.bound | T-0003 | Weigh the items | line {}/1",
            at(1, 1),
            night.pack
        ),
        format!(
            "{} | {} | gate.observed | - | - | src/x.rs (code)",
            night.observed,
            at(0, 30)
        ),
    ];
    for line in &want {
        assert!(
            lines.contains(&line.as_str()),
            "no line {line:?} in:\n{}",
            text.stdout
        );
    }
    let approved_line = format!(
        "{} | {} | task.approved | T-0001 | Fix the door timer | by rule",
        all.iter().find(|e| e.kind == "task.approved").unwrap().seq,
        at(2, 0)
    );
    assert!(lines.contains(&approved_line.as_str()), "{}", text.stdout);
    assert_eq!(
        lines.iter().filter(|l| !l.starts_with("note:")).count(),
        all.len(),
        "one line per entry"
    );

    // --since SEQ and --since TIME cut alike.
    let (by_seq, _) = feed(auto, &["--since", &(night.landed - 1).to_string()]);
    assert_eq!(by_seq["after"], night.landed - 1);
    assert!(seqs(&by_seq).iter().all(|seq| *seq >= night.landed));
    assert_eq!(seqs(&by_seq)[0], night.landed);
    let (by_time, _) = feed(auto, &["--since", &at(6, 2)]);
    assert_eq!(by_time["after"], Value::Null);
    let at_or_after: Vec<i64> = all
        .iter()
        .filter(|e| e.at.as_str() >= at(6, 2).as_str())
        .map(|e| e.seq)
        .collect();
    assert_eq!(seqs(&by_time), at_or_after);
    // --task: its own, its proposals', and (decided 2026-11-15) the
    // `auto.started` whose `tasks` list names it.
    let (task, _) = feed(auto, &["--since", "0", "--task", "T-0001"]);
    assert!(
        task["entries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["task"] == "T-0001"
                || (e["type"] == "auto.started" && e["seq"] == night.pack)),
        "{task}"
    );
    assert!(seqs(&task).contains(&applied[0]), "its proposal's apply");
    assert!(!seqs(&task).contains(&applied[1]));
    assert!(seqs(&task).contains(&night.pack), "the pack naming it");
    assert!(!seqs(&task).contains(&night.held), "T-0002's hold");
    // T-0003, bound to the pack's line, is named by no `tasks` list.
    let (bound_only, _) = feed(auto, &["--since", "0", "--task", "T-0003"]);
    assert!(!seqs(&bound_only).contains(&night.pack), "{bound_only}");
    // --department general: the lane's.
    let (general, _) = feed(auto, &["--since", "0", "--department", "general"]);
    assert!(
        general["entries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["lane"] == "general"),
        "{general}"
    );
    assert!(seqs(&general).contains(&night.landed));
    // A read: no event written.
    assert_eq!(auto.raw_events(), all);
    // A bad value: exit 2.
    auto.spec(&["feed", "--since", "-1"]).code(2);
    auto.spec(&["feed", "--since", "2026-11-16"]).code(2);
}

/// FD2. M: `held` taken from any past `auto.held`.
#[test]
fn fd2_needs_me() {
    let night = night("fd2");
    let auto = &night.auto;
    let repository = auto.repository();
    let worktree = auto.linked.to_str().unwrap().to_owned();
    // A failed run of T-0004, and T-0005 done, neither landed nor held.
    {
        let mut queue = auto.store();
        let t4 = queue
            .create_task(&new_task(&repository, "Fail once", "R-01"), &at(40, 0))
            .unwrap()
            .id;
        change(&mut queue, &t4, &plan(), &at(40, 1));
        let seen = queue.get_task(&t4).unwrap().unwrap().seen();
        queue
            .approve_task_by_rule(
                &t4,
                &seen,
                &snapshot(&worktree),
                night.pack,
                &night.rule,
                &at(40, 2),
            )
            .unwrap();
        change(&mut queue, &t4, &claim(&worktree, &at(40, 3)), &at(40, 3));
        change(
            &mut queue,
            &t4,
            &report(RunOutcome::Failed, "the build broke"),
            &at(40, 4),
        );
    }
    let (json, raw) = feed(auto, &["--since", "0", "--needs-me"]);
    let needs = &json["needs_me"];
    assert_eq!(
        keys(&serde_json::to_string(needs).unwrap()).len(),
        5,
        "{needs}"
    );
    let start = raw.find("\"needs_me\":").unwrap() + "\"needs_me\":".len();
    assert_eq!(
        keys(&{
            let mut depth = 0;
            let mut end = start;
            for (k, c) in raw[start..].char_indices() {
                match c {
                    '{' | '[' => depth += 1,
                    '}' | ']' => {
                        depth -= 1;
                        if depth == 0 {
                            end = start + k + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            raw[start..end].to_owned()
        }),
        ["held", "open", "failed_runs", "gate_observed", "unlanded"]
    );
    assert_eq!(
        needs["held"],
        json!([{
            "seq": night.held, "at": at(31, 10), "pack": night.pack, "task": "T-0002",
            "line": null, "proposal": null, "act": "land", "reason": "check",
            "detail": "spec check: 1 error(s); the first docs/spec/core.md:12: ref-dangling"
        }])
    );
    let open: Vec<&str> = needs["open"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    assert_eq!(open, ["PR-0003"], "{needs}");
    assert_eq!(needs["failed_runs"][0]["task"], "T-0004");
    assert_eq!(needs["failed_runs"][0]["outcome"], "failed");
    assert_eq!(needs["failed_runs"][0]["run"], 1);
    assert_eq!(needs["gate_observed"][0]["path"], "src/x.rs");
    assert_eq!(needs["gate_observed"][0]["zone"], "code");
    assert_eq!(needs["gate_observed"][0]["branch"], "auto-T-0004");
    assert_eq!(
        needs["unlanded"],
        json!([]),
        "T-0001 landed, T-0002 held: {needs}"
    );
    // `held` ignores `--since`: a window past the hold still lists it.
    let (late, _) = feed(
        auto,
        &["--since", &night.observed.to_string(), "--needs-me"],
    );
    assert_eq!(late["needs_me"]["held"], needs["held"], "{late}");

    // The held task landed afterwards by the owner leaves `held`; a done
    // pack task neither landed nor held is `unlanded`.
    {
        let mut queue = auto.store();
        queue
            .record_auto(
                &AutoEvent::Landed(AutoLanded {
                    pack: night.pack,
                    task: "T-0002".to_owned(),
                    branch: "auto".to_owned(),
                    commit: "5".repeat(40),
                    code_commit: None,
                    applied: Vec::new(),
                    left: Vec::new(),
                    rule: night.rule.clone(),
                }),
                &at(50, 0),
            )
            .unwrap();
        // T-0003 (the bound line's task) taken to done.
        change(&mut queue, "T-0003", &plan(), &at(51, 0));
        let seen = queue.get_task("T-0003").unwrap().unwrap().seen();
        queue
            .approve_task_by_rule(
                "T-0003",
                &seen,
                &snapshot(&worktree),
                night.pack,
                &night.rule,
                &at(51, 1),
            )
            .unwrap();
        change(
            &mut queue,
            "T-0003",
            &claim(&worktree, &at(51, 2)),
            &at(51, 2),
        );
        change(
            &mut queue,
            "T-0003",
            &report(RunOutcome::Completed, "weighed"),
            &at(51, 3),
        );
        change(&mut queue, "T-0003", &TaskChange::Complete, &at(51, 4));
    }
    let (json, _) = feed(auto, &["--since", "0", "--needs-me"]);
    assert_eq!(json["needs_me"]["held"], json!([]), "{json}");
    assert_eq!(
        json["needs_me"]["unlanded"],
        json!([{"task": "T-0003", "status": "done", "pack": night.pack}])
    );
    let text = auto.spec(&["feed", "--since", "0", "--needs-me"]);
    text.code(0);
    assert!(
        text.stdout.contains("needs me: held: none"),
        "{}",
        text.stdout
    );
    let _ = (night.applied_commit, night.manual_commit);
}

/// FD2, the orchestrator's decision of 2026-11-15 (lane B's `still_held`):
/// an item leaves `needs_me.held` once nothing waits on it. A hub's hold
/// of a proposal alone (`pack`, `task` null) is listed until its row is no
/// longer `open` (here approved by hand); a held task leaves once it is
/// cancelled. M: `held` ignoring the proposal's status.
#[test]
fn fd2_a_confirm_after_a_hold_leaves_held() {
    let night = night("fd2-confirm");
    let auto = &night.auto;
    let rule = night.rule.clone();
    {
        let mut queue = auto.store();
        queue
            .record_auto(
                &AutoEvent::Held(AutoHeld {
                    pack: None,
                    task: None,
                    line: None,
                    proposal: Some("PR-0003".to_owned()),
                    act: "apply".to_owned(),
                    reason: "excluded-zone".to_owned(),
                    detail: "`docs/narrative/intro.md` is in the excluded zone `narrative`"
                        .to_owned(),
                    rule,
                }),
                &at(33, 0),
            )
            .unwrap();
    }
    let held = |label: &str| -> Vec<Value> {
        let (json, _) = feed(auto, &["--since", "0", "--needs-me"]);
        json["needs_me"]["held"]
            .as_array()
            .unwrap_or_else(|| panic!("{label}: {json}"))
            .clone()
    };
    let listed = held("after the hold");
    assert_eq!(listed.len(), 2, "{listed:?}");
    let proposal = listed
        .iter()
        .find(|row| row["proposal"] == "PR-0003")
        .unwrap_or_else(|| panic!("{listed:?}"));
    assert_eq!(proposal["pack"], Value::Null);
    assert_eq!(proposal["task"], Value::Null);
    assert_eq!(proposal["act"], "apply");
    assert_eq!(proposal["reason"], "excluded-zone");
    assert!(listed.iter().any(|row| row["task"] == "T-0002"));

    // Confirmed by hand: the proposal leaves.
    {
        let mut queue = auto.store();
        queue.approve("PR-0003", &owner(), &at(40, 0)).unwrap();
        queue
            .applied("PR-0003", &"6".repeat(40), &at(40, 0))
            .unwrap();
    }
    let listed = held("after the confirm");
    assert!(
        listed.iter().all(|row| row["proposal"] != "PR-0003"),
        "{listed:?}"
    );
    assert_eq!(listed.len(), 1, "{listed:?}");

    // The held task cancelled: it leaves.
    {
        let mut queue = auto.store();
        change(&mut queue, "T-0002", &TaskChange::Cancel, &at(41, 0));
    }
    assert_eq!(held("after the cancel"), Vec::<Value>::new());
}
