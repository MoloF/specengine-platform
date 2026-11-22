//! docs/features/auto-rule.md RL12 and the store half of RL1 and RL11
//! (docs/canon/auto-mode.md "Events"): `SqliteQueue`'s `auto.*` events
//! through the public API, the stored payloads read raw with the system's
//! `sqlite3` (the key order is the subject).
//!
//! - RL12: `record_auto` of each type stores its payload with keys in the
//!   canon's order and `null`s written, `seq` rising, `at` the injected
//!   clock; `auto_events` returns them by `seq`, decoded as recorded, the
//!   repository's alone; `stored_rows` then `restore` into a fresh queue
//!   keeps them byte for byte; `PRAGMA user_version` stays 14.
//! - `approve_task_by_rule`: the task `ready` and `task.approved {id, by,
//!   pack, rule}` in one transaction; a plain approve stays `{id}`.
//! - By rule: `proposal.approved` and `.applied` add `by`, `rule` and
//!   (locally) `pack` after today's keys; a handle with none writes today's
//!   bytes; `rule_applies_on` counts a day's delegated applies only.

#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{Scratch, blake3_hex};
use specengine_core::auto_toml::CanonicalRule;
use specengine_core::proposal::Author;
use specengine_model::{SnapshotPlace, TaskCriterion, TaskStatus};
use specengine_store::{
    AUTO_EVENTS, AutoBound, AutoEvent, AutoHeld, AutoLanded, AutoStarted, ByRule, Decision,
    NewProposal, NewTask, Place, ProposalKind, ProposalQueue as _, QUEUE_SCHEMA_VERSION,
    SnapshotEntry, SqliteQueue, TaskChange, TaskSnapshot, patch_hash,
};

const T0: &str = "2026-11-16T00:00:00Z";
const T1: &str = "2026-11-16T00:00:01Z";
const T2: &str = "2026-11-16T23:59:59Z";
const NEXT_DAY: &str = "2026-11-17T00:00:00Z";
const PROJECT: &str = "auto-mini";
const COMMON: &str = "/r/.git";
const OTHER: &str = "/elsewhere/.git";
const RULE: &str = "b3:1111111111111111111111111111111111111111111111111111111111111111";

fn sqlite3(db: &Path, sql: &str) -> String {
    let client = ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
        .expect("a sqlite3 client");
    let output = Command::new(client)
        .arg(db)
        .arg(sql)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// `(type, payload, at)` of every event, raw, by `seq`.
fn raw_events(db: &Path) -> Vec<(String, String, String)> {
    sqlite3(
        db,
        "SELECT type || char(9) || payload || char(9) || at FROM events ORDER BY seq",
    )
    .lines()
    .map(|line| {
        let mut parts = line.split('\t');
        (
            parts.next().unwrap().to_owned(),
            parts.next().unwrap().to_owned(),
            parts.next().unwrap().to_owned(),
        )
    })
    .collect()
}

fn frozen() -> CanonicalRule {
    serde_json::from_str(
        "{\"format\":1,\"mode\":\"both\",\"zones\":{\"code\":[\"src/**\"]},\"exclude_zones\":{},\"exclude_kinds\":{},\"branch\":\"auto\",\"max_tasks\":10,\"max_applies\":20}",
    )
    .expect("a canonical form")
}

fn started(repository: &str, until: Option<&str>) -> AutoEvent {
    AutoEvent::Started(AutoStarted {
        repository: repository.to_owned(),
        tasks: vec!["T-0041".to_owned(), "T-0042".to_owned()],
        lines: vec!["Make the inventory sortable".to_owned()],
        until: until.map(str::to_owned),
        branch: "auto".to_owned(),
        branch_at: "3".repeat(40),
        rule: RULE.to_owned(),
        frozen: frozen(),
        by: "Ann Owner <ann@example.org>".to_owned(),
    })
}

/// RL12. M: a key out of order or a `null` skipped; a schema step added.
#[test]
fn rl12_each_auto_event_round_trips_in_the_canons_key_order() {
    assert_eq!(
        AUTO_EVENTS,
        ["auto.started", "auto.bound", "auto.held", "auto.landed"]
    );
    assert_eq!(
        QUEUE_SCHEMA_VERSION, 15,
        "comments (RM-0136) added step 15; auto-mode added none"
    );
    let scratch = Scratch::new("auto-events");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let events = [
        started(COMMON, None),
        AutoEvent::Bound(AutoBound {
            pack: 1,
            line: 1,
            task: "T-0050".to_owned(),
        }),
        AutoEvent::Held(AutoHeld {
            pack: Some(1),
            task: Some("T-0042".to_owned()),
            line: None,
            proposal: None,
            act: "land".to_owned(),
            reason: "check".to_owned(),
            detail: "spec check: 1 error(s); the first docs/spec/core.md:12: ref-dangling"
                .to_owned(),
            rule: RULE.to_owned(),
        }),
        AutoEvent::Held(AutoHeld {
            pack: Some(1),
            task: None,
            line: Some(1),
            proposal: None,
            act: "run".to_owned(),
            reason: "not-planned".to_owned(),
            detail: "not-planned".to_owned(),
            rule: RULE.to_owned(),
        }),
        AutoEvent::Landed(AutoLanded {
            pack: 1,
            task: "T-0041".to_owned(),
            branch: "auto".to_owned(),
            commit: "9".repeat(40),
            code_commit: None,
            applied: vec!["PR-0007".to_owned()],
            left: Vec::new(),
            rule: RULE.to_owned(),
        }),
    ];
    let mut seqs = Vec::new();
    for (k, event) in events.iter().enumerate() {
        let at = format!("2026-11-16T00:00:0{k}Z");
        seqs.push(queue.record_auto(event, &at).expect("recorded"));
    }
    assert!(seqs.windows(2).all(|w| w[0] < w[1]), "{seqs:?}");
    assert_eq!(seqs[0], 1, "the pack is its auto.started's seq");

    let raw = raw_events(&db);
    let want = [
        (
            "auto.started",
            format!(
                "{{\"repository\":\"/r/.git\",\"tasks\":[\"T-0041\",\"T-0042\"],\"lines\":[\"Make the inventory sortable\"],\"until\":null,\"branch\":\"auto\",\"branch_at\":\"{}\",\"rule\":\"{RULE}\",\"frozen\":{},\"by\":\"Ann Owner <ann@example.org>\"}}",
                "3".repeat(40),
                frozen().to_text()
            ),
        ),
        (
            "auto.bound",
            "{\"pack\":1,\"line\":1,\"task\":\"T-0050\"}".to_owned(),
        ),
        (
            "auto.held",
            format!(
                "{{\"pack\":1,\"task\":\"T-0042\",\"line\":null,\"proposal\":null,\"act\":\"land\",\"reason\":\"check\",\"detail\":\"spec check: 1 error(s); the first docs/spec/core.md:12: ref-dangling\",\"rule\":\"{RULE}\"}}"
            ),
        ),
        (
            "auto.held",
            format!(
                "{{\"pack\":1,\"task\":null,\"line\":1,\"proposal\":null,\"act\":\"run\",\"reason\":\"not-planned\",\"detail\":\"not-planned\",\"rule\":\"{RULE}\"}}"
            ),
        ),
        (
            "auto.landed",
            format!(
                "{{\"pack\":1,\"task\":\"T-0041\",\"branch\":\"auto\",\"commit\":\"{}\",\"code_commit\":null,\"applied\":[\"PR-0007\"],\"left\":[],\"rule\":\"{RULE}\"}}",
                "9".repeat(40)
            ),
        ),
    ];
    assert_eq!(raw.len(), want.len());
    for (k, ((kind, payload, at), (want_kind, want_payload))) in raw.iter().zip(&want).enumerate() {
        assert_eq!(kind, want_kind);
        assert_eq!(payload, want_payload, "{kind}");
        assert_eq!(at, &format!("2026-11-16T00:00:0{k}Z"));
    }

    // Read back by seq, decoded as recorded.
    let stored = queue.auto_events(COMMON).expect("auto events");
    assert_eq!(
        stored.iter().map(|e| e.seq).collect::<Vec<_>>(),
        seqs,
        "by seq"
    );
    for (stored, event) in stored.iter().zip(&events) {
        assert_eq!(&stored.event, event);
    }
    // Another repository's pack and what names it are not this one's.
    let other = queue
        .record_auto(&started(OTHER, Some("2026-11-17T07:00:00Z")), T2)
        .expect("another repository's pack");
    queue
        .record_auto(
            &AutoEvent::Bound(AutoBound {
                pack: other,
                line: 1,
                task: "T-0060".to_owned(),
            }),
            T2,
        )
        .unwrap();
    assert_eq!(queue.auto_events(COMMON).unwrap().len(), 5);
    let theirs = queue.auto_events(OTHER).unwrap();
    assert_eq!(theirs.len(), 2);
    assert_eq!(theirs[0].seq, other);

    // The dump keeps them as any event: restored into a fresh queue, the
    // events table is the same bytes; no schema step.
    let rows = queue.stored_rows().expect("rows");
    let fresh = scratch.db("restored");
    let mut restored = SqliteQueue::open(&fresh, PROJECT).expect("open");
    restored.restore(&rows).expect("restored");
    assert_eq!(raw_events(&fresh), raw_events(&db));
    assert_eq!(
        restored.auto_events(COMMON).unwrap(),
        queue.auto_events(COMMON).unwrap()
    );
    for path in [&db, &fresh] {
        assert_eq!(sqlite3(path, "PRAGMA user_version"), "15");
        assert_eq!(queue.schema_version().unwrap(), 15);
    }
}

fn new_task() -> NewTask {
    NewTask {
        git_common_dir: COMMON.to_owned(),
        title: Some("Fix the door timer".to_owned()),
        goal: None,
        targets: vec!["R-02".to_owned()],
        author: Author::human(),
    }
}

fn snapshot() -> TaskSnapshot {
    TaskSnapshot {
        at: T1.to_owned(),
        by: "Ann Owner <ann@example.org>".to_owned(),
        place: SnapshotPlace {
            worktree: "/r".to_owned(),
            root_rel: String::new(),
            branch: "auto-T-0001".to_owned(),
            commit: "1".repeat(40),
        },
        nodes: vec![SnapshotEntry {
            id: "R-02".to_owned(),
            path: "docs/spec/core.md".to_owned(),
            span_hash: "b3:02".to_owned(),
            text: "text of R-02".to_owned(),
        }],
    }
}

fn planned(queue: &mut SqliteQueue) -> String {
    let id = queue.create_task(&new_task(), T0).expect("created").id;
    let seen = queue.get_task(&id).unwrap().unwrap().seen();
    queue
        .change_task(
            &id,
            &seen,
            &TaskChange::Plan {
                plan: "1. Change it.".to_owned(),
                criteria: vec![TaskCriterion {
                    reference: None,
                    text: Some("Holds.".to_owned()),
                }],
                affected_nodes: Vec::new(),
            },
            T0,
        )
        .expect("planned");
    id
}

/// The task's approve by rule: one transaction, the pinned payload; the
/// plain approve's payload stays `{id}` (RL1). M: `by` always written.
#[test]
fn a_task_approved_by_rule_logs_its_keys_and_a_plain_one_does_not() {
    let scratch = Scratch::new("auto-task");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = planned(&mut queue);
    let seen = queue.get_task(&id).unwrap().unwrap().seen();
    let task = queue
        .approve_task_by_rule(&id, &seen, &snapshot(), 812, RULE, T1)
        .expect("approved by rule");
    assert_eq!(task.status, TaskStatus::Ready);
    let plain = planned(&mut queue);
    let seen = queue.get_task(&plain).unwrap().unwrap().seen();
    queue
        .change_task(
            &plain,
            &seen,
            &TaskChange::Approve {
                snapshot: snapshot(),
            },
            T1,
        )
        .expect("approved");
    let approved: Vec<String> = raw_events(&db)
        .into_iter()
        .filter(|(kind, _, _)| kind == "task.approved")
        .map(|(_, payload, _)| payload)
        .collect();
    assert_eq!(
        approved,
        [
            format!("{{\"id\":\"{id}\",\"by\":\"rule\",\"pack\":812,\"rule\":\"{RULE}\"}}"),
            format!("{{\"id\":\"{plain}\"}}"),
        ]
    );
    // A stale `seen` (the revision before the approve): refused, nothing
    // written.
    let before = queue.dump().unwrap();
    let mut stale = queue.get_task(&id).unwrap().unwrap().seen();
    stale.revision -= 1;
    let again = queue.approve_task_by_rule(&id, &stale, &snapshot(), 812, RULE, T2);
    assert!(again.is_err(), "{again:?}");
    assert_eq!(queue.dump().unwrap(), before);
}

fn proposal(target: &str) -> NewProposal {
    let base_hash = format!("b3:{}", blake3_hex(b"base"));
    NewProposal {
        kind: ProposalKind::Update,
        target_id: target.to_owned(),
        target_path: "docs/spec/core.md".to_owned(),
        place: Place {
            git_common_dir: COMMON.to_owned(),
            worktree: "/r".to_owned(),
            root_rel: String::new(),
            branch: "main".to_owned(),
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
        decided_by: "Ann Owner <ann@example.org>".to_owned(),
        note: None,
        staged_at: None,
    }
}

/// By rule: `proposal.approved` and `.applied` add `"by":"rule"`, `rule`
/// and `pack` after today's keys; `None` gives today's bytes; a hub's
/// `ByRule` has no `pack`. `rule_applies_on` counts only by-rule applies
/// on its date. M: the keys before today's; `by` always written; counting
/// every apply.
#[test]
fn by_rule_events_and_the_days_count() {
    let scratch = Scratch::new("auto-by-rule");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let commit = "c".repeat(40);
    let mut ids = Vec::new();
    for (k, target) in ["R-01", "R-02", "R-03", "R-04"].iter().enumerate() {
        let by = match k {
            0 => None,
            1 => Some(ByRule {
                rule: RULE.to_owned(),
                pack: Some(812),
            }),
            _ => Some(ByRule {
                rule: RULE.to_owned(),
                pack: None,
            }),
        };
        let at = [T0, T1, T2, NEXT_DAY][k];
        let id = queue.create(&proposal(target), at).expect("created").id;
        queue.set_by_rule(by.clone());
        assert_eq!(queue.by_rule(), by.as_ref());
        queue.approve(&id, &owner(), at).expect("approved");
        queue.applied(&id, &commit, at).expect("applied");
        queue.set_by_rule(None);
        ids.push(id);
    }
    let decided: Vec<(String, String)> = raw_events(&db)
        .into_iter()
        .filter(|(kind, _, _)| kind == "proposal.approved" || kind == "proposal.applied")
        .map(|(kind, payload, _)| (kind, payload))
        .collect();
    let applied = |id: &str, tail: &str| {
        (
            "proposal.applied".to_owned(),
            format!("{{\"commit\":\"{commit}\",\"id\":\"{id}\"{tail}}}"),
        )
    };
    let local = format!(",\"by\":\"rule\",\"rule\":\"{RULE}\",\"pack\":812");
    let hub = format!(",\"by\":\"rule\",\"rule\":\"{RULE}\"");
    // Today's bytes (0.46.1's) for the plain row; a by-rule row is them plus
    // the keys, after every one of today's.
    let plain_approved = decided[0].1.clone();
    assert!(
        plain_approved.starts_with('{')
            && plain_approved.contains(&format!("\"id\":\"{}\"", ids[0])),
        "{plain_approved}"
    );
    assert!(
        !plain_approved.contains("\"by\"") && !plain_approved.contains("\"rule\""),
        "a plain approve: {plain_approved}"
    );
    assert_eq!(decided[1], applied(&ids[0], ""), "today's bytes");
    let with_tail = |plain: &str, id: &str, tail: &str| {
        plain.replace(&ids[0], id).trim_end_matches('}').to_owned() + tail + "}"
    };
    let approved = |k: usize, tail: &str| {
        (
            "proposal.approved".to_owned(),
            with_tail(&plain_approved, &ids[k], tail),
        )
    };
    assert_eq!(decided[2], approved(1, &local));
    assert_eq!(decided[3], applied(&ids[1], &local));
    assert_eq!(decided[4], approved(2, &hub));
    assert_eq!(decided[5], applied(&ids[2], &hub));
    assert_eq!(decided[6], approved(3, &hub));
    assert_eq!(decided[7], applied(&ids[3], &hub));
    // Today's by-rule applies: R-02's and R-03's; R-01's is a person's,
    // R-04's the next day's.
    assert_eq!(queue.rule_applies_on("2026-11-16").unwrap(), 2);
    assert_eq!(queue.rule_applies_on("2026-11-17").unwrap(), 1);
    assert_eq!(queue.rule_applies_on("2026-11-15").unwrap(), 0);
}
