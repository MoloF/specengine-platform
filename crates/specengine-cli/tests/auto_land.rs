//! docs/features/auto-land.md LD3 (commit, apply, check, merge), LD4
//! (never `main`, never a push), LD5 (the lock untouched), LD7 (one land
//! at a time; a conflict aborts clean), LD8 (off mid-run), LD11's land (a
//! held `done` task landed by the owner) and LD12 (a checked-out branch is
//! never moved), over scratch copies of `fixtures/auto-mini` with
//! `rules/on.toml` and a header-only `spec.lock` committed, a bare
//! repository as `origin`; a pack recorded through the store, each task
//! taken to `done` in its worktree on `auto-<T>` from `auto` through the
//! binary (`spec auto approve`, `spec task claim`, `report`, `complete`).
//!
//! Mutations: a held proposal applied; a red check merged; a config change
//! committed; a `git push` after the merge; a lock accept inside land; no
//! lock (and no `update-ref` compare); the checked-out check removed.

#![cfg(unix)]

mod auto_common;
mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use auto_common::{Auto, COMMITTER};
use serde_json::{Value, json};

/// A `spec.lock` of no entry: `spec check` never reads it.
const LOCK: &str = "# spec.lock: written only by SpecEngine (spec lock, spec approve, spec verify --tests); do not edit\nformat = 2\nnorm_recipe = \"specengine-norm/v1\"\ncode_recipe = \"specengine-hash/v3 tree-sitter-rust/0.24.2 abi/15\"\n";

/// The repository of a land test: the rule, the lock, `origin`.
fn landing(label: &str) -> (Auto, PathBuf) {
    let auto = Auto::with_rule(label, "on.toml");
    common::write(&auto.main, "spec.lock", LOCK);
    auto.commit_all(&auto.main, "the lock");
    let origin = auto.scratch.join("origin.git");
    auto.git.init_with(&origin, &["--bare"]);
    auto.git.git(
        &auto.main,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    auto.git.git(&auto.main, &["push", "-q", "origin", "main"]);
    (auto, origin)
}

fn origin_refs(auto: &Auto, origin: &Path) -> String {
    auto.git_text(
        origin,
        &["for-each-ref", "--format=%(refname) %(objectname)"],
    )
}

/// Every branch's `spec.lock` blob and the main worktree's bytes.
fn locks(auto: &Auto) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (name, _) in auto.refs() {
        if let Some(branch) = name.strip_prefix("refs/heads/") {
            let blob = auto.git.git_output(
                &auto.main,
                &["rev-parse", &format!("{branch}:spec.lock")],
                &[],
            );
            out.insert(
                branch.to_owned(),
                String::from_utf8_lossy(&blob.stdout).trim().to_owned(),
            );
        }
    }
    out.insert(
        "worktree".to_owned(),
        fs::read_to_string(auto.main.join("spec.lock")).unwrap(),
    );
    out
}

fn worktree_list(auto: &Auto) -> String {
    auto.git_text(&auto.main, &["worktree", "list", "--porcelain"])
}

fn land_lock(auto: &Auto) -> PathBuf {
    auto.data().join("auto-mini.auto-land")
}

/// LD3, LD4, LD5. M: a held proposal applied; a `git push` after the
/// merge; a lock accept inside land.
#[test]
fn ld3_land_commits_applies_checks_and_merges() {
    let (auto, origin) = landing("ld3");
    let task = auto.planned(&["R-02"], "Fix the door timer", &[]);
    let pack = auto.pack(&[&task], &[]);
    let rule = auto.head_rule().hash;
    auto.auto_approve(&task).code(0);
    let wt = auto.claim_in(&task);
    common::replace(
        &wt,
        "src/lib.rs",
        "DOOR_SECONDS: u32 = 5",
        "DOOR_SECONDS: u32 = 6",
    );
    let bound = ["--task", task.as_str()];
    let covered = auto.propose_update(&wt, "R-02", "five seconds", "six seconds", &bound);
    let narrative = auto.propose_update(&wt, "SC-001", "at dusk", "at dawn", &bound);
    auto.finish(
        &task,
        "completed",
        "The timer resets on close.\nSix seconds now.",
    );
    assert_eq!(auto.status(&task), "done");

    let main_before = auto.rev(&auto.main, "main");
    let auto_before = auto.rev(&auto.main, "auto");
    let origin_before = origin_refs(&auto, &origin);
    let remotes_before: Vec<(String, String)> = auto
        .refs()
        .into_iter()
        .filter(|(name, _)| name.starts_with("refs/remotes/"))
        .collect();
    let locks_before = locks(&auto);
    let claim_tip_before = auto.rev(&auto.main, &format!("auto-{task}"));

    let run = auto.land(&task);
    run.code(0);
    let merge = auto.rev(&auto.main, "auto");
    assert_eq!(
        run.stdout,
        format!(
            "landed {task} on auto as {} (applied {covered}; left {narrative})\n",
            &merge[..12]
        ),
        "{}",
        run.show()
    );

    // The code commit on the task's branch, then the apply.
    let code_commit = auto.git_text(
        &auto.main,
        &[
            "log",
            "--format=%H",
            "--grep",
            &format!("^auto: {task} "),
            &format!("auto-{task}"),
        ],
    );
    assert_eq!(code_commit.lines().count(), 1, "one code commit");
    assert_eq!(
        auto.message(&auto.main, &code_commit).trim_end(),
        format!(
            "auto: {task} Fix the door timer\n\nThe timer resets on close.\nSix seconds now.\n\nTask: {task}\nRun: 1\nPack: {pack}\nDelegated-by: auto {rule}"
        ),
        "the code commit's message"
    );
    assert_eq!(
        auto.git_text(
            &auto.main,
            &["log", "-1", "--format=%an <%ae>|%cn <%ce>", &code_commit]
        ),
        format!("Scratch Author <author@example.invalid>|{COMMITTER}"),
        "the worktree's git identity"
    );
    let applied = auto.proposal(&covered);
    assert_eq!(applied.status.as_str(), "applied");
    let apply_commit = applied.applied_commit.clone().expect("applied");
    let message = auto.message(&auto.main, &apply_commit);
    let trailers: Vec<&str> = message.trim_end().lines().rev().take(1).collect();
    assert_eq!(
        trailers,
        [format!("Delegated-by: auto {rule}").as_str()],
        "{message}"
    );
    for trailer in ["Proposal: ", "Decided-by: ", "Base-commit: "] {
        assert!(message.contains(trailer), "{message}");
    }
    assert!(
        message.find("Base-commit: ").unwrap() < message.find("Delegated-by: ").unwrap(),
        "Delegated-by last: {message}"
    );
    // The narrative one left open, held once.
    assert_eq!(auto.proposal(&narrative).status.as_str(), "open");
    let held: Vec<Value> = auto
        .events_of_kind("auto.held")
        .iter()
        .map(auto_common::RawEvent::json)
        .collect();
    assert_eq!(
        held,
        [
            json!({"pack": pack, "task": task, "line": null, "proposal": narrative,
                 "act": "apply", "reason": "excluded-zone",
                 "detail": "`docs/narrative/intro.md` is in the excluded zone `narrative`",
                 "rule": rule})
        ]
    );
    // The applied events carry the rule and the pack.
    let applied_event = auto
        .events_of_kind("proposal.applied")
        .pop()
        .expect("applied");
    assert_eq!(
        applied_event.json(),
        json!({"id": covered, "commit": apply_commit, "by": "rule", "rule": rule, "pack": pack})
    );

    // The merge: --no-ff, two parents, its message.
    let parents = auto.git_text(&auto.main, &["log", "-1", "--format=%P", &merge]);
    let parents: Vec<&str> = parents.split(' ').collect();
    let claim_tip = auto.rev(&auto.main, &format!("auto-{task}"));
    assert_eq!(parents, [auto_before.as_str(), claim_tip.as_str()]);
    assert_eq!(
        auto.message(&auto.main, &merge).trim_end(),
        format!(
            "auto: land {task} into auto\n\nTask: {task}\nPack: {pack}\nDelegated-by: auto {rule}"
        )
    );
    // Commits stay on the task's branch: the code commit, then the apply.
    let on_branch = auto.git_text(
        &auto.main,
        &["rev-list", &format!("{claim_tip_before}..auto-{task}")],
    );
    assert_eq!(
        on_branch.lines().collect::<Vec<_>>(),
        [apply_commit.as_str(), code_commit.as_str()]
    );
    // The landing event.
    let landed: Vec<Value> = auto
        .events_of_kind("auto.landed")
        .iter()
        .map(auto_common::RawEvent::json)
        .collect();
    assert_eq!(
        landed,
        [
            json!({"pack": pack, "task": task, "branch": "auto", "commit": merge,
                 "code_commit": code_commit, "applied": [covered], "left": [narrative],
                 "rule": rule})
        ]
    );
    assert_eq!(
        auto.git_text(
            &auto.main,
            &[
                "log",
                "--format=%H",
                "--grep",
                "^Delegated-by: auto",
                "auto"
            ]
        )
        .lines()
        .count(),
        3,
        "the merge, the code commit and the apply"
    );

    // LD4: main and origin untouched, no remote ref moved.
    assert_eq!(auto.rev(&auto.main, "main"), main_before);
    assert_eq!(origin_refs(&auto, &origin), origin_before);
    let remotes_after: Vec<(String, String)> = auto
        .refs()
        .into_iter()
        .filter(|(name, _)| name.starts_with("refs/remotes/"))
        .collect();
    assert_eq!(remotes_after, remotes_before);
    // LD5: the lock's bytes.
    let locks_after = locks(&auto);
    for (branch, blob) in &locks_before {
        assert_eq!(locks_after.get(branch), Some(blob), "spec.lock on {branch}");
    }
    assert_eq!(locks_after["auto"], locks_before["main"]);
    assert_eq!(
        locks_after[&format!("auto-{task}")],
        locks_before["main"],
        "the task's branch"
    );
    // No scratch worktree, no land lock left.
    assert!(
        !worktree_list(&auto).contains("land-"),
        "{}",
        worktree_list(&auto)
    );
    assert!(!land_lock(&auto).exists());
    assert_eq!(auto.status(&task), "done");

    // Landed already: exit 1, no event.
    let events = auto.raw_events();
    let again = auto.land(&task);
    again.code(1);
    assert!(
        again.stderr.contains(&format!(
            "{task} is landed: {} on auto; nothing changed",
            &merge[..12]
        )),
        "{}",
        again.show()
    );
    assert_eq!(auto.raw_events(), events);
}

/// LD3's holds: a merged tree with a dangling link is held `check`, `auto`
/// unmoved; a changed `specengine.toml` is held `config-target`, nothing
/// committed. M: a red check merged; a config change committed.
#[test]
fn ld3_a_red_check_or_a_config_change_holds() {
    let (auto, _) = landing("ld3-holds");
    let red = auto.planned(&["R-02"], "Break a link", &[]);
    let config = auto.planned(&["R-01"], "Touch the config", &[]);
    let pack = auto.pack(&[&red, &config], &[]);
    let rule = auto.head_rule().hash;
    auto.to_done(
        &red,
        &[(
            "docs/spec/core.md",
            "reviewed: 2026-11-15\n---",
            "reviewed: 2026-11-15\nlinks:\n  depends_on: [R-77]\n---",
        )],
    );
    let before = auto.rev(&auto.main, "auto");
    let run = auto.land(&red);
    run.code(1);
    assert!(
        run.stderr.contains(&format!(
            "{red} held (check): spec check: 1 error(s); the first docs/spec/core.md:7: ref-dangling; auto unmoved"
        )),
        "{}",
        run.show()
    );
    assert_eq!(auto.rev(&auto.main, "auto"), before, "auto unmoved");
    let held = auto.events_of_kind("auto.held").pop().expect("held").json();
    assert_eq!(held["act"], "land");
    assert_eq!(held["reason"], "check");
    assert_eq!(held["task"], red.as_str());
    assert_eq!(held["pack"], pack);
    assert_eq!(held["rule"], rule.as_str());
    assert!(!worktree_list(&auto).contains("land-"));

    // The config: held, nothing committed, the worktree as it was.
    let wt = auto.to_done(
        &config,
        &[("specengine.toml", "[gate]\n", "# touched\n[gate]\n")],
    );
    let tip = auto.rev(&auto.main, &format!("auto-{config}"));
    let status = auto.git_text(&wt, &["status", "--porcelain"]);
    let run = auto.land(&config);
    run.code(1);
    assert!(
        run.stderr.contains(&format!(
            "{config} held (config-target): `specengine.toml` is a project config or the lock: never delegated; auto unmoved"
        )),
        "{}",
        run.show()
    );
    assert_eq!(
        auto.rev(&auto.main, &format!("auto-{config}")),
        tip,
        "nothing committed"
    );
    assert_eq!(auto.git_text(&wt, &["status", "--porcelain"]), status);
    assert_eq!(auto.rev(&auto.main, "auto"), before);
    // --json of a hold.
    let run = auto.spec(&["--json", "auto", "land", &config]);
    run.code(1);
    let doc = run.json();
    assert_eq!(doc["landed"], false);
    assert_eq!(doc["reason"], "config-target");
}

/// LD7: the lock file present: exit 1 naming it, nothing written; two
/// tasks editing one line of `src/lib.rs` land in turn, the second held
/// `merge-conflict`, `auto` at the first's merge, no scratch worktree, no
/// merge in progress. M: no lock.
#[test]
fn ld7_one_land_at_a_time_and_a_conflict_aborts_clean() {
    let (auto, _) = landing("ld7");
    let first = auto.planned(&["R-02"], "Six seconds", &[]);
    let second = auto.planned(&["R-01"], "Seven seconds", &[]);
    auto.pack(&[&first, &second], &[]);
    auto.to_done(
        &first,
        &[(
            "src/lib.rs",
            "DOOR_SECONDS: u32 = 5",
            "DOOR_SECONDS: u32 = 6",
        )],
    );
    auto.to_done(
        &second,
        &[(
            "src/lib.rs",
            "DOOR_SECONDS: u32 = 5",
            "DOOR_SECONDS: u32 = 7",
        )],
    );

    // Another land's lock.
    let lock = land_lock(&auto);
    fs::create_dir_all(lock.parent().unwrap()).unwrap();
    // A live pid (this test's): another land still running.
    let held_by = format!("{} 2026-11-16T01:00:00Z", std::process::id());
    fs::write(&lock, &held_by).unwrap();
    let events = auto.raw_events();
    let refs = auto.refs();
    let run = auto.land(&first);
    run.code(1);
    assert!(
        run.stderr.contains(&format!(
            "another land holds {} since {held_by}; if none runs, remove it; nothing changed",
            lock.display()
        )),
        "{}",
        run.show()
    );
    assert_eq!(auto.raw_events(), events);
    assert_eq!(auto.refs(), refs);
    assert_eq!(fs::read_to_string(&lock).unwrap(), held_by);
    fs::remove_file(&lock).unwrap();

    auto.land(&first).code(0);
    let landed = auto.rev(&auto.main, "auto");
    let run = auto.land(&second);
    run.code(1);
    assert!(
        run.stderr.contains(&format!(
            "{second} held (merge-conflict): 1 path(s) conflict: src/lib.rs; auto unmoved"
        )),
        "{}",
        run.show()
    );
    assert_eq!(
        auto.rev(&auto.main, "auto"),
        landed,
        "auto at the first's merge"
    );
    assert!(
        !worktree_list(&auto).contains("land-"),
        "{}",
        worktree_list(&auto)
    );
    assert!(!lock.exists());
    // No merge in progress anywhere.
    for wt in [auto.main.clone(), auto.task_worktree(&second)] {
        let git_dir = auto.git_text(&wt, &["rev-parse", "--git-dir"]);
        let git_dir = if Path::new(&git_dir).is_absolute() {
            PathBuf::from(git_dir)
        } else {
            wt.join(git_dir)
        };
        assert!(!git_dir.join("MERGE_HEAD").exists(), "{}", wt.display());
    }
    let held = auto.events_of_kind("auto.held").pop().unwrap().json();
    assert_eq!(held["reason"], "merge-conflict");
    assert_eq!(held["act"], "land");
}

/// LD8: the table removed on the main worktree between approve and land:
/// exit 1 off, nothing written, the task `done`, its claim kept; a
/// narrowing commit: held `uncovered`. M: judging the frozen rule alone.
#[test]
fn ld8_off_mid_run() {
    let (auto, _) = landing("ld8");
    let task = auto.planned(&["R-02"], "Fix the door timer", &[]);
    auto.pack(&[&task], &[]);
    auto.to_done(
        &task,
        &[(
            "src/lib.rs",
            "DOOR_SECONDS: u32 = 5",
            "DOOR_SECONDS: u32 = 6",
        )],
    );
    let config = auto.config();
    auto.commit_without_auto("auto off");
    let events = auto.raw_events();
    let refs = auto.refs();
    let run = auto.land(&task);
    run.code(1);
    assert!(
        run.stderr
            .contains("auto is off: no [auto] table in specengine.toml@HEAD"),
        "{}",
        run.show()
    );
    assert_eq!(auto.raw_events(), events);
    assert_eq!(auto.refs(), refs);
    assert_eq!(auto.status(&task), "done");
    let package = auto.spec(&["--json", "task", "show", &task]);
    assert!(package.json()["claim"].is_object(), "the claim kept");

    // The rule back, narrowed: no `code`.
    auto.write_config(&config.replacen(
        "zones         = [\"code\", \"tests\", \"docs\"]",
        "zones         = [\"tests\", \"docs\"]",
        1,
    ));
    auto.commit_all(&auto.main, "narrow");
    let run = auto.land(&task);
    run.code(1);
    assert!(
        run.stderr.contains(&format!(
            "{task} held (uncovered): `src/lib.rs` is in no zone the rule covers (docs, tests) (the rule at HEAD); auto unmoved"
        )),
        "{}",
        run.show()
    );
}

/// LD11's land: a held `done` task, landed by the owner's `spec auto land`.
/// LD12: `auto` checked out in a second worktree: held
/// `branch-checked-out`, unmoved. M: the check removed.
#[test]
fn ld11_ld12_a_held_task_lands_and_a_checked_out_branch_never_moves() {
    let (auto, _) = landing("ld12");
    let task = auto.planned(&["R-02"], "Fix the door timer", &[]);
    auto.pack(&[&task], &[]);
    auto.to_done(
        &task,
        &[(
            "src/lib.rs",
            "DOOR_SECONDS: u32 = 5",
            "DOOR_SECONDS: u32 = 6",
        )],
    );
    let held = auto.spec(&[
        "auto",
        "hold",
        &task,
        "--reason",
        "denied-tool",
        "--note",
        "Bash",
    ]);
    held.code(0);
    assert_eq!(held.stdout, format!("held {task} (denied-tool): Bash\n"));

    // LD12 first: checked out elsewhere.
    let elsewhere = auto.scratch.join("checked-out");
    auto.git.git(
        &auto.main,
        &["worktree", "add", "-q", elsewhere.to_str().unwrap(), "auto"],
    );
    let elsewhere = fs::canonicalize(elsewhere).unwrap();
    let before = auto.rev(&auto.main, "auto");
    let run = auto.land(&task);
    run.code(1);
    assert!(
        run.stderr.contains(&format!(
            "{task} held (branch-checked-out): `auto` is checked out in {}; auto unmoved",
            elsewhere.display()
        )),
        "{}",
        run.show()
    );
    assert_eq!(auto.rev(&auto.main, "auto"), before);
    assert!(!worktree_list(&auto).contains("land-"));
    auto.git.git(
        &auto.main,
        &["worktree", "remove", "--force", elsewhere.to_str().unwrap()],
    );

    // The owner's land of the held task lands it.
    let run = auto.spec(&["--json", "auto", "land", &task]);
    run.code(0);
    let doc = run.json();
    assert_eq!(doc["landed"], true);
    assert_eq!(doc["id"], task.as_str());
    assert_eq!(doc["branch"], "auto");
    assert_eq!(doc["commit"], auto.rev(&auto.main, "auto").as_str());
    assert_eq!(auto.events_of_kind("auto.landed").len(), 1);
}

/// LD3 (a), the reviewer's row: a `.spec-debt.toml` entry past its
/// `expires` on the integration tip makes `spec check` red on the merge:
/// the land holds `check`, `auto` unmoved.
#[test]
fn ld3_expired_debt_holds_check() {
    let (auto, _) = landing("ld3-debt");
    common::replace(
        &auto.main,
        "docs/spec/core.md",
        "reviewed: 2026-11-15\n---",
        "reviewed: 2026-11-15\nlinks:\n  depends_on: [R-77]\n---",
    );
    common::write(
        &auto.main,
        ".spec-debt.toml",
        "[[debt]]\ncode    = \"ref-dangling\"\npath    = \"docs/spec/core.md\"\nsubject = \"R-77\"\nreason  = \"later\"\nexpires = \"2020-01-01\"\n",
    );
    auto.commit_all(&auto.main, "a debt long expired");
    let check = auto.spec(&["check"]);
    check.code(1);
    assert!(check.stdout.contains("1 expired"), "{}", check.show());
    let task = auto.planned(&["R-02"], "Six seconds", &[]);
    auto.pack(&[&task], &[]);
    auto.to_done(
        &task,
        &[(
            "src/lib.rs",
            "DOOR_SECONDS: u32 = 5",
            "DOOR_SECONDS: u32 = 6",
        )],
    );
    let before = auto.rev(&auto.main, "auto");
    let run = auto.land(&task);
    run.code(1);
    assert!(
        run.stderr
            .contains(&format!("{task} held (check): spec check: ")),
        "{}",
        run.show()
    );
    assert!(run.stderr.contains("ref-dangling"), "{}", run.show());
    assert_eq!(auto.rev(&auto.main, "auto"), before);
    assert_eq!(
        auto.events_of_kind("auto.held").pop().unwrap().json()["reason"],
        "check"
    );
}

/// LD3 (b), the reviewer's row: a task with nothing of its own (no commit,
/// no change) holds `place`, "nothing to land"; a claim tip already merged
/// into `auto` is recorded as a recovery, `auto` unmoved, no fresh merge.
#[test]
fn ld3_nothing_new_and_an_already_merged_tip() {
    let (auto, _) = landing("ld3-nothing");
    let empty = auto.planned(&["R-02"], "Nothing done", &[]);
    let merged = auto.planned(&["R-04"], "Merged by hand", &[]);
    auto.pack(&[&empty, &merged], &[]);
    auto.to_done(&empty, &[]);
    let before = auto.rev(&auto.main, "auto");
    let run = auto.land(&empty);
    run.code(1);
    assert!(
        run.stderr.contains(&format!("{empty} held (place): "))
            && run.stderr.contains("nothing to land"),
        "{}",
        run.show()
    );
    assert_eq!(auto.rev(&auto.main, "auto"), before);

    // The claim's work committed and merged into `auto` as the land's own
    // merge (its message and trailers), as a land killed after its merge
    // and before its event left it.
    let rule = auto.head_rule().hash;
    let pack = auto.events_of_kind("auto.started")[0].seq;
    let wt = auto.to_done(
        &merged,
        &[(
            "src/lib.rs",
            "DOOR_SECONDS: u32 = 5",
            "DOOR_SECONDS: u32 = 6",
        )],
    );
    auto.commit_all(&wt, "the work");
    let side = auto.scratch.join("side");
    auto.git.git(
        &auto.main,
        &["worktree", "add", "-q", side.to_str().unwrap(), "auto"],
    );
    auto.git.git(
        &side,
        &[
            "merge",
            "-q",
            "--no-ff",
            "-m",
            &format!(
                "auto: land {merged} into auto\n\nTask: {merged}\nPack: {pack}\nDelegated-by: auto {rule}"
            ),
            &format!("auto-{merged}"),
        ],
    );
    auto.git.git(
        &auto.main,
        &["worktree", "remove", "--force", side.to_str().unwrap()],
    );
    let tip = auto.rev(&auto.main, "auto");
    let run = auto.spec(&["--json", "auto", "land", &merged]);
    run.code(0);
    assert_eq!(
        auto.rev(&auto.main, "auto"),
        tip,
        "no fresh merge: {}",
        run.show()
    );
    assert!(
        run.json()["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|note| note
                == &format!(
                    "recovered: {} merged {merged} into auto before, unrecorded; recorded now",
                    &tip[..12]
                )),
        "{}",
        run.show()
    );
    let landed = auto.events_of_kind("auto.landed");
    assert_eq!(landed.len(), 1);
    assert_eq!(landed[0].json()["task"], merged.as_str());
    assert_eq!(landed[0].json()["commit"], tip.as_str());
}

/// LD3 (c), the reviewer's row: a nested project (`root_rel` `proj`): a
/// changed path outside `proj/` holds `uncovered`.
#[test]
fn ld3_a_path_outside_the_root_is_uncovered() {
    let auto = Auto::with_rule("ld3-nested", "on.toml");
    // The project moved into `proj/`, a file beside it at the top.
    let proj = auto.main.join("proj");
    fs::create_dir_all(&proj).unwrap();
    for name in [
        "specengine.toml",
        "docs",
        "notes",
        "src",
        "tests",
        "rules",
        "expected.json",
    ] {
        auto.git
            .git(&auto.main, &["mv", name, &format!("proj/{name}")]);
    }
    common::write(&auto.main, "README.md", "The top.\n");
    auto.commit_all(&auto.main, "nested");
    let proj = fs::canonicalize(proj).unwrap();
    let root = proj.to_str().unwrap().to_owned();
    let ok = |cwd: &Path, args: &[&str]| {
        let run = auto.run(cwd, args, &[]);
        run.code(0);
        run
    };
    let task = ok(
        &proj,
        &["--json", "task", "new", "--nodes", "R-02", "--title", "Top"],
    )
    .json()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    auto.piped(
        &proj,
        &["task", "plan", &task, "--plan-file", "-"],
        b"1. Do.\n",
    )
    .code(0);
    auto.make_branch("auto");
    let config = auto.git_text(&auto.main, &["show", "HEAD:proj/specengine.toml"]);
    let rule =
        specengine_core::auto_toml::AutoRule::from_toml(config.as_bytes(), "proj/specengine.toml")
            .rule
            .expect("the rule");
    auto.record_pack_of(&rule, &[&task], &[], None, &auto_common::utc(-60));
    let wt = auto.task_worktree(&task);
    let wt_root = wt.join("proj");
    ok(
        &wt_root,
        &[
            "auto",
            "approve",
            &task,
            "--root",
            wt_root.to_str().unwrap(),
        ],
    );
    ok(
        &wt_root,
        &[
            "task",
            "claim",
            &task,
            "--role",
            "developer",
            "--worktree",
            wt.to_str().unwrap(),
        ],
    );
    common::replace(&wt, "README.md", "The top.", "The very top.");
    ok(
        &wt_root,
        &[
            "task",
            "report",
            &task,
            "--outcome",
            "completed",
            "--summary",
            "Top.",
            "--changed",
            "README.md",
        ],
    );
    ok(&wt_root, &["task", "complete", &task]);
    let run = auto.run(&proj, &["auto", "land", &task, "--root", &root], &[]);
    run.code(1);
    assert!(
        run.stderr.contains(&format!(
            "{task} held (uncovered): `../README.md` is in no zone the rule covers"
        )),
        "{}",
        run.show()
    );
    // A project config outside the root stays `config-target`.
    common::write(&wt, "extra/specengine.toml", "[project]\nslug = \"x\"\n");
    let run = auto.run(&proj, &["auto", "land", &task, "--root", &root], &[]);
    run.code(1);
    assert!(
        run.stderr.contains(&format!(
            "{task} held (config-target): `../extra/specengine.toml` is a project config or the \
             lock: never delegated"
        )),
        "{}",
        run.show()
    );
}

/// LD3 (d), the reviewer's row: a changed spec document in a covered zone
/// that now declares an ID of an excluded kind holds `excluded-kind`.
#[test]
fn ld3_a_changed_document_declaring_an_excluded_kind_holds() {
    let (auto, _) = landing("ld3-kind");
    let task = auto.planned(&["R-02"], "A scene in the spec", &[]);
    auto.pack(&[&task], &[]);
    auto.to_done(
        &task,
        &[(
            "docs/spec/core.md",
            "The timer starts again when the door opens.\n",
            "The timer starts again when the door opens.\n\n### A scene {#SC-002}\n\nThe door creaks.\n",
        )],
    );
    let before = auto.rev(&auto.main, "auto");
    let run = auto.land(&task);
    run.code(1);
    assert!(
        run.stderr.contains(&format!(
            "{task} held (excluded-kind): `SC-002` in `docs/spec/core.md` is a scenario, an \
             excluded kind; auto unmoved"
        )),
        "{}",
        run.show()
    );
    assert_eq!(auto.rev(&auto.main, "auto"), before);
}

/// LD7, the reviewer's row and lane A's decision: SIGINT, SIGTERM and
/// SIGHUP are held from step 3; a land sent SIGTERM while its merge's
/// `pre-merge-commit` hook runs exits 2 `interrupted: …`, records nothing,
/// and leaves no lock and no `land-*` scratch worktree; the next land goes
/// through.
#[test]
fn ld7_a_land_killed_mid_way_leaves_nothing_stuck() {
    use std::io::Read as _;
    use std::os::unix::fs::PermissionsExt as _;
    use std::process::{Command, Stdio};
    let (auto, _) = landing("ld7-kill");
    let task = auto.planned(&["R-02"], "Killed", &[]);
    auto.pack(&[&task], &[]);
    auto.to_done(
        &task,
        &[(
            "src/lib.rs",
            "DOOR_SECONDS: u32 = 5",
            "DOOR_SECONDS: u32 = 6",
        )],
    );
    let marker = auto.scratch.join("hook-entered");
    let release = auto.scratch.join("hook-release");
    let hook = auto.main.join(".git/hooks/pre-merge-commit");
    fs::create_dir_all(hook.parent().unwrap()).unwrap();
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\n: > '{}'\ni=0\nwhile [ ! -e '{}' ] && [ \"$i\" -lt 600 ]; do sleep 0.1; i=$((i + 1)); done\nexit 0\n",
            marker.display(),
            release.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let before = auto.rev(&auto.main, "auto");
    let events = auto.raw_events();
    let mut child = Command::new(common::SPEC)
        .env_clear()
        .envs(auto.git.vars())
        .env("HOME", &auto.home)
        .current_dir(&auto.main)
        .args(["auto", "land", &task])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the land");
    let started = std::time::Instant::now();
    while !marker.exists() {
        assert!(
            started.elapsed().as_secs() < 60,
            "the merge's hook never ran"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        land_lock(&auto).exists(),
        "the lock is held during the merge"
    );
    Command::new("/bin/kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    // The signal is held: the hook ends, then the land stops.
    std::thread::sleep(std::time::Duration::from_millis(300));
    fs::write(&release, "").unwrap();
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    let status = child.wait().unwrap();
    fs::remove_file(&hook).unwrap();
    assert_eq!(status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("interrupted: "), "{stderr}");
    assert_eq!(auto.raw_events(), events, "nothing recorded");
    assert_eq!(auto.rev(&auto.main, "auto"), before, "auto unmoved");
    assert!(!land_lock(&auto).exists(), "the lock removed");
    assert!(
        !worktree_list(&auto).contains("land-"),
        "{}",
        worktree_list(&auto)
    );
    // The next land goes through and leaves nothing.
    let run = auto.land(&task);
    run.code(0);
    assert!(!land_lock(&auto).exists(), "the lock is gone");
    assert!(
        !worktree_list(&auto).contains("land-"),
        "{}",
        worktree_list(&auto)
    );
    assert_eq!(auto.events_of_kind("auto.landed").len(), 1);
}

/// Lane A's decision: a lock whose pid is gone is removed and taken; a
/// stale `land-<pid>` scratch worktree is pruned under the lock.
#[test]
fn ld7_a_dead_lock_and_a_stale_scratch_worktree_are_taken() {
    let (auto, _) = landing("ld7-stale");
    let task = auto.planned(&["R-02"], "After a crash", &[]);
    auto.pack(&[&task], &[]);
    auto.to_done(
        &task,
        &[(
            "src/lib.rs",
            "DOOR_SECONDS: u32 = 5",
            "DOOR_SECONDS: u32 = 6",
        )],
    );
    let lock = land_lock(&auto);
    fs::create_dir_all(lock.parent().unwrap()).unwrap();
    // A pid no process has (the largest a pid_t takes).
    fs::write(&lock, "2147483646 2026-11-16T01:00:00Z").unwrap();
    let stale = auto
        .data()
        .join("auto")
        .join("auto-mini")
        .join("land-2147483646");
    fs::create_dir_all(stale.parent().unwrap()).unwrap();
    auto.git.git(
        &auto.main,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            stale.to_str().unwrap(),
            "auto",
        ],
    );
    assert!(worktree_list(&auto).contains("land-2147483646"));
    auto.land(&task).code(0);
    assert!(!lock.exists());
    assert!(
        !worktree_list(&auto).contains("land-"),
        "{}",
        worktree_list(&auto)
    );
}

/// Lane A's decision: `left` lists every open row bound to the task,
/// whichever worktree raised it (here a question raised in the main
/// worktree).
#[test]
fn ld3_left_lists_every_open_bound_row() {
    let (auto, _) = landing("ld3-left");
    let task = auto.planned(&["R-02"], "With a question", &[]);
    auto.pack(&[&task], &[]);
    let asked = auto.spec(&[
        "--json",
        "propose",
        "question",
        "R-02",
        "--text",
        "Six seconds?",
        "--working-answer",
        "Yes.",
        "--price-of-other",
        "A test edit.",
        "--task",
        &task,
    ]);
    asked.code(0);
    let question = asked.json()["id"].as_str().unwrap().to_owned();
    auto.to_done(
        &task,
        &[(
            "src/lib.rs",
            "DOOR_SECONDS: u32 = 5",
            "DOOR_SECONDS: u32 = 6",
        )],
    );
    let run = auto.spec(&["--json", "auto", "land", &task]);
    run.code(0);
    assert_eq!(run.json()["left"], json!([question]), "{}", run.show());
}

/// Lane A's decision on step 9: new debt alone (an error in live debt the
/// base's baseline lacks) holds `check`, `spec check: <verdict>: debt the
/// base's baseline lacks`.
#[test]
fn ld3_new_debt_alone_holds_check() {
    let auto = Auto::new("ld3-newdebt");
    // `.spec-debt.toml` a covered zone of its own.
    let config = auto.config().replacen(
        "narrative = [\"docs/narrative/**\"]\n",
        "narrative = [\"docs/narrative/**\"]\nmeta      = [\".spec-debt.toml\"]\n",
        1,
    );
    auto.write_config(&auto_common::with_table(
        &config,
        &auto_common::rule_text("on.toml").replacen(
            "[\"code\", \"tests\", \"docs\"]",
            "[\"code\", \"tests\", \"docs\", \"meta\"]",
            1,
        ),
    ));
    auto.commit_all(&auto.main, "the rule and a meta zone");
    let task = auto.planned(&["R-02"], "A debt", &[]);
    auto.pack(&[&task], &[]);
    let wt = auto.to_done(
        &task,
        &[(
            "docs/spec/core.md",
            "reviewed: 2026-11-15\n---",
            "reviewed: 2026-11-15\nlinks:\n  depends_on: [R-77]\n---",
        )],
    );
    common::write(
        &wt,
        ".spec-debt.toml",
        "[[debt]]\ncode    = \"ref-dangling\"\npath    = \"docs/spec/core.md\"\nsubject = \"R-77\"\nreason  = \"later\"\nexpires = \"2099-01-01\"\n",
    );
    let before = auto.rev(&auto.main, "auto");
    let run = auto.spec(&["--json", "auto", "land", &task]);
    run.code(1);
    let doc = run.json();
    assert_eq!(doc["reason"], "check", "{}", run.show());
    let detail = doc["detail"].as_str().unwrap();
    assert!(
        detail.starts_with("spec check: ") && detail.ends_with(": debt the base's baseline lacks"),
        "{detail}"
    );
    assert_eq!(auto.rev(&auto.main, "auto"), before);
}

/// The round-2 review's rows: the excluded kinds a land judges are the
/// union of the IDs declared at the merge base and after the change: a
/// covered document declaring an `SC-` ID deleted, or its `SC-` section
/// removed, holds `excluded-kind`.
#[test]
fn ld3_removing_an_excluded_kind_holds_too() {
    let (auto, _) = landing("ld3-removed");
    common::write(
        &auto.main,
        "docs/spec/scenes.md",
        "---\nclass: canon\ntitle: Scenes\nowner: owner\nreviewed: 2026-11-15\n---\n\n# Scenes\n\nScenes kept beside the spec.\n\n## The gate {#SC-005}\n\nThe gate stands open.\n",
    );
    common::replace(
        &auto.main,
        "docs/spec/core.md",
        "The timer starts again when the door opens.\n",
        "The timer starts again when the door opens.\n\n### A creak {#SC-006}\n\nThe door creaks.\n",
    );
    auto.commit_all(&auto.main, "two scenes in covered files");
    let deleting = auto.planned(&["R-02"], "Delete the scenes file", &[]);
    let removing = auto.planned(&["R-04"], "Remove the creak", &[]);
    auto.pack(&[&deleting, &removing], &[]);
    let wt = auto.to_done(&deleting, &[]);
    fs::remove_file(wt.join("docs/spec/scenes.md")).unwrap();
    let run = auto.land(&deleting);
    run.code(1);
    assert!(
        run.stderr.contains(&format!(
            "{deleting} held (excluded-kind): `SC-005` in `docs/spec/scenes.md` is a scenario, \
             an excluded kind; auto unmoved"
        )),
        "{}",
        run.show()
    );
    auto.to_done(
        &removing,
        &[(
            "docs/spec/core.md",
            "\n### A creak {#SC-006}\n\nThe door creaks.\n",
            "",
        )],
    );
    let run = auto.land(&removing);
    run.code(1);
    assert!(
        run.stderr.contains(&format!(
            "{removing} held (excluded-kind): `SC-006` in `docs/spec/core.md` is a scenario, \
             an excluded kind; auto unmoved"
        )),
        "{}",
        run.show()
    );
}

/// The round-2 review's row: two lands racing to take over one stale lock
/// (a dead pid): only one runs at a time; the other refuses naming the
/// lock, or runs after; never a `branch-moved` from the race.
#[test]
fn ld7_two_lands_racing_for_a_stale_lock() {
    use std::process::{Command, Stdio};
    let (auto, _) = landing("ld7-race");
    let first = auto.planned(&["R-02"], "First", &[]);
    let second = auto.planned(&["R-04"], "Second", &[]);
    auto.pack(&[&first, &second], &[]);
    auto.to_done(
        &first,
        &[(
            "src/lib.rs",
            "DOOR_SECONDS: u32 = 5",
            "DOOR_SECONDS: u32 = 6",
        )],
    );
    auto.to_done(&second, &[("tests/a.rs", "door_open(4)", "door_open(3)")]);
    let lock = land_lock(&auto);
    fs::create_dir_all(lock.parent().unwrap()).unwrap();
    fs::write(&lock, "2147483646 2026-11-16T01:00:00Z").unwrap();
    let spawn = |task: &str| {
        Command::new(common::SPEC)
            .env_clear()
            .envs(auto.git.vars())
            .env("HOME", &auto.home)
            .current_dir(&auto.main)
            .args(["--json", "auto", "land", task])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("a land")
    };
    let children = [spawn(&first), spawn(&second)];
    let outputs: Vec<std::process::Output> = children
        .into_iter()
        .map(|child| child.wait_with_output().unwrap())
        .collect();
    let mut landed = 0;
    for output in &outputs {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        match output.status.code() {
            Some(0) => landed += 1,
            Some(1) => assert!(
                stderr.contains("another land holds") || stdout.contains("another land holds"),
                "{stdout}{stderr}"
            ),
            other => panic!("{other:?}: {stdout}{stderr}"),
        }
        assert!(!stdout.contains("branch-moved"), "{stdout}");
    }
    assert!(landed >= 1, "one of the two ran");
    assert_eq!(auto.events_of_kind("auto.landed").len(), landed);
    assert!(
        auto.events_of_kind("auto.held")
            .iter()
            .all(|held| held.json()["reason"] != "branch-moved")
    );
    assert!(!lock.exists());
    assert!(!worktree_list(&auto).contains("land-"));
}

/// The round-2 review's row: a step-7 apply the run refuses (here a
/// `commit-msg` hook refusing `spec: apply` commits) never stops the land
/// and leaves one note naming its row and reason; the row stays in `left`.
#[test]
fn ld3_a_refused_apply_leaves_a_note_and_lands() {
    use std::os::unix::fs::PermissionsExt as _;
    let (auto, _) = landing("ld3-refused");
    let task = auto.planned(&["R-02"], "With a refused apply", &[]);
    auto.pack(&[&task], &[]);
    auto.auto_approve(&task).code(0);
    let wt = auto.claim_in(&task);
    common::replace(
        &wt,
        "src/lib.rs",
        "DOOR_SECONDS: u32 = 5",
        "DOOR_SECONDS: u32 = 6",
    );
    let id = auto.propose_update(
        &wt,
        "R-02",
        "five seconds",
        "six seconds",
        &["--task", &task],
    );
    auto.finish(&task, "completed", "Six.");
    let hook = auto.main.join(".git/hooks/commit-msg");
    fs::create_dir_all(hook.parent().unwrap()).unwrap();
    fs::write(
        &hook,
        "#!/bin/sh\nif grep -q '^spec: apply' \"$1\"; then echo 'no applies here' >&2; exit 1; fi\nexit 0\n",
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let run = auto.spec(&["--json", "auto", "land", &task]);
    fs::remove_file(&hook).unwrap();
    run.code(0);
    let doc = run.json();
    assert_eq!(doc["landed"], true, "{}", run.show());
    assert_eq!(doc["applied"], json!([]));
    assert_eq!(doc["left"], json!([id]));
    let notes: Vec<&str> = doc["notes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .filter(|note| note.contains(&id))
        .collect();
    assert_eq!(notes.len(), 1, "one note for {id}: {doc}");
    assert_eq!(auto.proposal(&id).status.as_str(), "open");
}
