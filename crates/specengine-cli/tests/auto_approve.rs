//! docs/features/auto-land.md LD2 (`spec auto approve` moves only a pack's
//! covered task) and LD6 (no rule, no auto: `approve`, `land`, `hold` exit
//! 1 off, `next` answers `stop`), over scratch copies of
//! `fixtures/auto-mini`; a pack recorded through the store, the branch
//! `auto` at `HEAD`, each task approved in its own worktree on
//! `auto-<T>` (the runner's place).
//!
//! Mutation: membership skipped.

#![cfg(unix)]

mod auto_common;
mod common;

use auto_common::{Auto, COMMITTER};
use serde_json::{Value, json};

/// LD2. M: membership skipped.
#[test]
fn ld2_approve_moves_only_a_packs_covered_task() {
    let auto = Auto::with_rule("ld2", "on.toml");
    let covered = auto.planned(&["R-02"], "Fix the door timer", &[]);
    let draft = auto.make_task(&["R-01"], "Still a draft");
    let scene = auto.planned(&["R-02"], "Touch the scene", &["SC-001"]);
    let outside = auto.planned(&["R-01"], "In no pack", &[]);
    let pack = auto.pack(&[&covered, &draft, &scene], &[]);
    let rule = auto.head_rule().hash;

    // The covered review task: ready, the event, the snapshot.
    let before = auto.raw_events().len();
    let run = auto.auto_approve(&covered);
    run.code(0);
    assert_eq!(
        run.stdout,
        format!("approved {covered}: ready (by rule, pack {pack})\n")
    );
    assert_eq!(auto.status(&covered), "ready");
    let events = auto.raw_events();
    assert_eq!(events.len(), before + 1, "one event");
    assert_eq!(events.last().unwrap().kind, "task.approved");
    assert_eq!(
        events.last().unwrap().payload,
        format!("{{\"id\":\"{covered}\",\"by\":\"rule\",\"pack\":{pack},\"rule\":\"{rule}\"}}")
    );
    // The snapshot as `spec task approve` freezes it, from the worktree,
    // `by` its git identity (the stored row: the package shows no `by`).
    let package = auto.spec(&["--json", "task", "show", &covered]);
    package.code(0);
    let snapshot = &package.json()["spec_snapshot"];
    let wt = auto.task_worktree(&covered);
    assert_eq!(snapshot["place"]["worktree"], wt.to_str().unwrap());
    assert_eq!(snapshot["place"]["branch"], format!("auto-{covered}"));
    assert_eq!(snapshot["nodes"][0]["id"], "R-02");
    let stored: Value = serde_json::from_str(
        auto.sql(&format!(
            "SELECT snapshot FROM tasks WHERE id = '{covered}'"
        ))
        .trim(),
    )
    .expect("the stored snapshot");
    assert_eq!(stored["by"], COMMITTER, "{stored}");

    // --json.
    let json_auto = Auto::with_rule("ld2-json", "on.toml");
    let task = json_auto.planned(&["R-02"], "Fix", &[]);
    let pack2 = json_auto.pack(&[&task], &[]);
    let wt2 = json_auto.task_worktree(&task);
    let run = json_auto.run(
        &wt2,
        &[
            "--json",
            "auto",
            "approve",
            &task,
            "--root",
            wt2.to_str().unwrap(),
        ],
        &[],
    );
    run.code(0);
    assert_eq!(
        run.json(),
        json!({"id": task, "status": "ready", "by": "rule", "pack": pack2,
               "rule": json_auto.head_rule().hash, "reason": null, "detail": null, "notes": []})
    );

    // In no pack: exit 1, unchanged, no event.
    let events = auto.raw_events();
    let run = auto.auto_approve(&outside);
    run.code(1);
    assert!(
        run.stderr.contains(&format!(
            "{outside} held (not-in-pack): `{outside}` is in no live pack; nothing approved"
        )),
        "{}",
        run.show()
    );
    assert_eq!(auto.status(&outside), "review");
    assert_eq!(auto.raw_events(), events);

    // A pack task in `draft`: exit 1, no event (a caller's error).
    let run = auto.auto_approve(&draft);
    run.code(1);
    assert!(
        run.stderr.contains(&format!(
            "{draft} held (status): `{draft}` is draft: `approve` needs review; nothing approved"
        )),
        "{}",
        run.show()
    );
    assert_eq!(auto.raw_events(), events);
    assert_eq!(auto.status(&draft), "draft");

    // A pack task affecting SC-001 (in the excluded `narrative` zone): held,
    // one `auto.held`. The canon judges the zone before the kind, so the
    // reason is `excluded-zone` (the criterion's text says excluded-kind:
    // see the report).
    let run = auto.auto_approve(&scene);
    run.code(1);
    assert_eq!(auto.status(&scene), "review");
    let held = auto.raw_events();
    assert_eq!(held.len(), events.len() + 1, "one auto.held");
    let last = held.last().unwrap();
    assert_eq!(last.kind, "auto.held");
    let payload = last.json();
    assert_eq!(payload["act"], "approve");
    assert_eq!(payload["task"], scene.as_str());
    assert_eq!(payload["pack"], pack);
    assert!(
        payload["reason"] == "excluded-zone" || payload["reason"] == "excluded-kind",
        "{payload}"
    );
    assert!(
        run.stderr.contains(&format!(
            "{scene} held ({}): ",
            payload["reason"].as_str().unwrap()
        )),
        "{}",
        run.show()
    );
    // Keys in the canon's order.
    assert_eq!(
        last.payload,
        format!(
            "{{\"pack\":{pack},\"task\":\"{scene}\",\"line\":null,\"proposal\":null,\"act\":\"approve\",\"reason\":{},\"detail\":{},\"rule\":\"{rule}\"}}",
            payload["reason"], payload["detail"]
        )
    );
}

/// LD6: no table: `approve`, `land`, `hold` exit 1 `auto is off: no [auto]
/// table in specengine.toml@HEAD`, nothing written; `next` answers `stop`
/// `off`; `spec task approve` stays the owner's (RL1).
#[test]
fn ld6_no_rule_no_auto() {
    let auto = Auto::new("ld6");
    let task = auto.planned(&["R-02"], "No rule", &[]);
    auto.make_branch("auto");
    // A pack started under the fixture's `rules/on.toml`, since removed.
    let on = specengine_core::auto_toml::AutoRule::from_toml(
        auto_common::with_table(&auto.config(), &auto_common::rule_text("on.toml")).as_bytes(),
        "specengine.toml",
    )
    .rule
    .expect("on.toml reads");
    auto.record_pack_of(&on, &[&task], &[], None, &auto_common::utc(-60));
    let events = auto.raw_events();
    let refs = auto.refs();
    let off = "auto is off: no [auto] table in specengine.toml@HEAD";
    for args in [
        vec!["auto", "approve", task.as_str()],
        vec!["auto", "land", task.as_str()],
        vec!["auto", "hold", task.as_str(), "--reason", "denied-tool"],
    ] {
        let run = if args[1] == "approve" {
            auto.auto_approve(&task)
        } else {
            auto.spec(&args)
        };
        run.code(1);
        assert!(run.stderr.contains(off), "{args:?}: {}", run.show());
        assert_eq!(auto.raw_events(), events, "{args:?}: nothing written");
    }
    let next = auto.next(&[]);
    assert_eq!(next["action"], "stop");
    assert_eq!(next["reason"], "off");
    assert_eq!(next["why"], "no [auto] table in specengine.toml@HEAD");
    for key in [
        "pack", "item", "task", "line", "text", "title", "lane", "branch", "base",
    ] {
        assert_eq!(next[key], Value::Null, "{key}");
    }
    assert_eq!(
        auto.refs().get("refs/heads/auto"),
        refs.get("refs/heads/auto")
    );
    assert_eq!(auto.status(&task), "review");
}
