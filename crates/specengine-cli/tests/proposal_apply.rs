//! docs/features/proposal-apply.md, the apply (`spec approve`) and
//! `spec reject` through the library: AC-07 to AC-17 and AC-19's event
//! half; AC-07 and AC-11 on both fixtures (AC-20).
//!
//! "Library approve": consent yes, the current directory the main worktree,
//! the proposal raised in the linked worktree on `t1` (`common::proposal`).
//! "Refused": nothing stored or written, no commit, status `open`.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use common::proposal::{
    CASES, DECIDER, LATER, Pair, State, cannot, edit, json_of, printed, refused, with_vars,
};
use common::{copy_dir, read, read_text, replace, write};
use specengine_cli::Exit;

/// `git log --format=%H <from>..<to>` in `dir`, newest first.
fn commits(pair: &Pair, dir: &std::path::Path, range: &str) -> Vec<String> {
    pair.git_text(dir, &["log", "--format=%H", range])
        .lines()
        .map(str::to_owned)
        .collect()
}

/// `git diff --name-only <from> <to>` in `dir`.
fn changed(pair: &Pair, dir: &std::path::Path, from: &str, to: &str) -> Vec<String> {
    pair.git_text(dir, &["diff", "--name-only", from, to])
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Appends `line` to `root/relative`.
fn append(root: &std::path::Path, relative: &str, line: &str) {
    let mut text = read_text(root, relative);
    text.push_str(line);
    write(root, relative, text);
}

/// `state` with the proposals left out (they change by design).
fn worktrees(state: &State) -> State {
    State {
        proposals: Vec::new(),
        ..state.clone()
    }
}

/// AC-07 (and AC-20 for both fixtures): library approve from the main
/// worktree applies on `t1` in the linked worktree: one commit, its subject
/// and four trailers, only the target path, the file the old bytes with
/// exactly the span replaced; the main worktree's `HEAD`, index and files
/// unchanged; `applied` with the SHA stored. The consent question names
/// the path, branch, worktree and preview.
/// M: work in the current directory's worktree.
#[test]
fn ac07_library_approve_commits_on_t1_in_the_linked_worktree() {
    for case in &CASES {
        let pair = Pair::of("pa-ac07", case);
        let old = read(&pair.linked, case.path);
        let (hash, text) = pair.span(&pair.linked, case.target);
        let new_text = edit(&text, case.first.0, case.first.1);
        let mut request = pair.request(&pair.linked, case.target, &hash, &new_text);
        request.rationale = "Say it precisely.\n\nA second paragraph.".to_owned();
        request.run = Some("run-7".to_owned());
        let outcome = pair.propose_with(&pair.linked, request).unwrap();
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        let base = pair.rev(&pair.linked, "HEAD");
        let before = pair.state();

        let (outcome, questions) =
            pair.approve_answer(&pair.main, "PR-0001", true, pair.git_env(&pair.main));
        let outcome = outcome.unwrap_or_else(|error| panic!("{}: {error}", case.fixture));
        assert_eq!(
            outcome.exit(),
            Exit::Answered,
            "{}: {outcome:?}",
            case.fixture
        );
        assert_eq!(
            questions,
            [format!(
                "apply PR-0001 to {} on t1 in {} (applies)? [y/N]",
                case.path,
                pair.linked.display()
            )],
            "{}",
            case.fixture
        );
        let head = pair.rev(&pair.linked, "t1");
        assert_eq!(
            printed(&outcome).0,
            format!("applied PR-0001 as {head} on t1\n"),
            "{}",
            case.fixture
        );
        assert_eq!(
            commits(&pair, &pair.main, &format!("{base}..t1")),
            std::slice::from_ref(&head)
        );
        assert_eq!(pair.rev(&pair.linked, &format!("{head}^")), base);
        assert_eq!(changed(&pair, &pair.main, &base, &head), [case.path]);
        let message = pair.git_text(&pair.main, &["log", "-1", "--format=%B", &head]);
        assert_eq!(
            message,
            format!(
                "spec: apply PR-0001\n\nSay it precisely.\n\nA second paragraph.\n\n\
                 Proposal: PR-0001\nDecided-by: {DECIDER}\n\
                 Proposed-by: agent role=spec-writer model=claude-opus-5-5 run=run-7\n\
                 Base-commit: {base}"
            ),
            "{}",
            case.fixture
        );
        // Exactly the span replaced.
        let span = text.as_bytes();
        let at = old
            .windows(span.len())
            .position(|window| window == span)
            .expect("the span in the old bytes");
        let mut want = old[..at].to_vec();
        want.extend_from_slice(new_text.as_bytes());
        want.extend_from_slice(&old[at + span.len()..]);
        assert_eq!(read(&pair.linked, case.path), want, "{}", case.fixture);
        let committed = pair
            .git
            .git(&pair.main, &["show", &format!("{head}:{}", case.path)]);
        assert_eq!(committed, want, "{}", case.fixture);

        let after = pair.state();
        assert_eq!(after.main_files, before.main_files, "{}", case.fixture);
        assert_eq!(after.main_head, before.main_head, "{}", case.fixture);
        assert_eq!(after.main_staged, before.main_staged, "{}", case.fixture);
        assert_eq!(after.main_status, "", "{}", case.fixture);
        assert_eq!(after.linked_status, "", "{}", case.fixture);
        let stored = pair.proposal("PR-0001");
        assert_eq!(stored.status.as_str(), "applied");
        assert_eq!(stored.applied_commit.as_deref(), Some(head.as_str()));
        assert_eq!(stored.decided_by.as_deref(), Some(DECIDER));
        assert_eq!(stored.decided_at.as_deref(), Some(LATER));
        assert_eq!(
            pair.events_of("PR-0001"),
            [
                ("proposal.created".to_owned(), None),
                ("proposal.approved".to_owned(), None),
                ("proposal.applied".to_owned(), None),
            ]
        );
        let applied = pair
            .events()
            .into_iter()
            .find(|event| event.event_type == "proposal.applied")
            .unwrap();
        assert_eq!(applied.payload["commit"], head.as_str());
        // The index follows: `show` gives the new span.
        assert_eq!(pair.span(&pair.linked, case.target).1, new_text);
    }
}

/// AC-08: an unrelated staged file and another modified one in the linked
/// worktree: the commit holds only the target; both stay as they were.
/// M: no `--only <path>`.
#[test]
fn ac08_unrelated_staged_and_modified_files_stay_out_of_the_commit() {
    let pair = Pair::new("pa-ac08", "spec-a");
    let id = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );
    append(&pair.linked, "docs/spec/game.md", "Staged line.\n");
    pair.git.git(&pair.linked, &["add", "docs/spec/game.md"]);
    append(&pair.linked, "docs/records/A/A-101.md", "Modified line.\n");
    let staged = read(&pair.linked, "docs/spec/game.md");
    let modified = read(&pair.linked, "docs/records/A/A-101.md");
    let cached = pair.git_text(&pair.linked, &["diff", "--cached", "--name-only"]);
    let unstaged = pair.git_text(&pair.linked, &["diff", "--name-only"]);
    let staged_oid = pair.git.staged_oid(&pair.linked, "docs/spec/game.md");
    let base = pair.rev(&pair.linked, "HEAD");

    pair.approve_ok(&pair.main, &id);
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(
        changed(&pair, &pair.main, &base, &head),
        ["docs/spec/movement/sprint.md"]
    );
    assert_eq!(read(&pair.linked, "docs/spec/game.md"), staged);
    assert_eq!(read(&pair.linked, "docs/records/A/A-101.md"), modified);
    assert_eq!(
        pair.git_text(&pair.linked, &["diff", "--cached", "--name-only"]),
        cached
    );
    assert_eq!(cached, "docs/spec/game.md");
    assert_eq!(
        pair.git_text(&pair.linked, &["diff", "--name-only"]),
        unstaged
    );
    assert_eq!(unstaged, "docs/records/A/A-101.md");
    assert_eq!(
        pair.git.staged_oid(&pair.linked, "docs/spec/game.md"),
        staged_oid
    );
    assert_eq!(pair.proposal(&id).status.as_str(), "applied");
}

/// AC-09: the target staged-modified, then unstaged-modified: exit 1,
/// refused, one `apply_failed` (step 3) each. M: dirty check dropped.
#[test]
fn ac09_a_dirty_target_is_refused_with_one_apply_failed() {
    let pair = Pair::new("pa-ac09", "spec-a");
    let path = "docs/spec/movement/sprint.md";
    let id = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );
    replace(
        &pair.linked,
        path,
        "## Tuning notes",
        "## Tuning notes (draft)",
    );
    pair.git.git(&pair.linked, &["add", path]);
    let before = pair.state();
    let reason = refused(&pair.approve(&pair.main, &id), "staged");
    assert!(reason.contains("(step 3)"), "{reason}");
    assert_eq!(pair.state(), before, "staged: refused");
    assert_eq!(
        pair.events_of(&id),
        [
            ("proposal.created".to_owned(), None),
            ("proposal.apply_failed".to_owned(), Some(3))
        ]
    );

    pair.git.git(&pair.linked, &["reset", "-q", "--", path]);
    let before = pair.state();
    assert!(before.linked_status.contains(path), "{before:?}");
    let reason = refused(&pair.approve(&pair.main, &id), "unstaged");
    assert!(reason.contains("(step 3)"), "{reason}");
    assert_eq!(pair.state(), before, "unstaged: refused");
    assert_eq!(pair.events_of(&id).len(), 3, "{:?}", pair.events_of(&id));
    assert_eq!(
        pair.events_of(&id)[2],
        ("proposal.apply_failed".to_owned(), Some(3))
    );
    assert_eq!(pair.proposal(&id).status.as_str(), "open");
}

/// Approving `id` from the main worktree exits 2, nothing changes, and the
/// events of `id` grow by `events` (`apply_failed` at step 2 each).
fn assert_cannot_apply(pair: &Pair, id: &str, label: &str, events: usize) -> String {
    let before = pair.state();
    let logged = pair.events_of(id).len();
    let (outcome, questions) = pair.approve_answer(&pair.main, id, true, pair.git_env(&pair.main));
    let message = cannot(&outcome, label);
    assert!(questions.is_empty(), "{label}: asked {questions:?}");
    assert_eq!(pair.state(), before, "{label}: refused");
    let now = pair.events_of(id);
    assert_eq!(now.len(), logged + events, "{label}: {now:?}");
    if events == 1 {
        assert_eq!(
            now.last().unwrap(),
            &("proposal.apply_failed".to_owned(), Some(2)),
            "{label}"
        );
    }
    assert_eq!(pair.proposal(id).status.as_str(), "open", "{label}");
    message
}

/// AC-10: exit 2, refused: the linked worktree on another branch; its
/// `HEAD` detached; a merge in progress; the worktree removed (each one
/// `apply_failed` at step 2); the proposal named from a second repository
/// with the same slug (no event). M: drop any one.
#[test]
fn ac10_an_unbound_place_exits_2() {
    let pair = Pair::new("pa-ac10", "spec-a");
    let id = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );

    pair.git.git(&pair.linked, &["checkout", "-q", "-b", "t2"]);
    let message = assert_cannot_apply(&pair, &id, "another branch", 1);
    assert!(
        message.contains("`t2`") && message.contains("`t1`"),
        "{message}"
    );
    pair.git.git(&pair.linked, &["checkout", "-q", "t1"]);

    pair.git.git(&pair.linked, &["checkout", "-q", "--detach"]);
    let message = assert_cannot_apply(&pair, &id, "detached HEAD", 1);
    assert!(message.contains("detached"), "{message}");
    pair.git.git(&pair.linked, &["checkout", "-q", "t1"]);

    // A merge stopped on a conflict in another file.
    pair.git
        .git(&pair.linked, &["checkout", "-q", "-b", "side"]);
    append(&pair.linked, "docs/spec/game.md", "Side line.\n");
    pair.commit_all(&pair.linked, "side");
    pair.git.git(&pair.linked, &["checkout", "-q", "t1"]);
    append(&pair.linked, "docs/spec/game.md", "Our line.\n");
    pair.commit_all(&pair.linked, "ours");
    let merge = pair
        .git
        .git_output(&pair.linked, &["merge", "-q", "--no-edit", "side"], &[]);
    assert!(!merge.status.success(), "the merge stops on a conflict");
    let message = assert_cannot_apply(&pair, &id, "a merge in progress", 1);
    assert!(message.contains("merge"), "{message}");
    pair.git.git(&pair.linked, &["merge", "--abort"]);
    // The control: the place is bound again (the edit above is elsewhere).
    let review = pair.review_ok(&pair.main, &id);
    assert_eq!(
        review.document.preview.map(|p| p.as_str()),
        Some("applies"),
        "{review:?}"
    );

    // A second repository with the same slug: no event.
    let other = pair.scratch.copy("spec-a", "other");
    pair.git.init(&other);
    pair.commit_all(&other, "the same project elsewhere");
    let before = pair.state();
    let events = pair.events().len();
    let env = pair.env(&other);
    let (outcome, questions) = {
        let mut questions = Vec::new();
        let mut consent = |question: &str| {
            questions.push(question.to_owned());
            true
        };
        let outcome = specengine_cli::approve(
            &env,
            &specengine_cli::Globals::default(),
            &specengine_cli::ApproveRequest {
                id: id.clone(),
                note: None,
                now: LATER.to_owned(),
                git: pair.git_env(&other),
            },
            &mut consent,
        );
        (outcome, questions)
    };
    let message = cannot(&outcome, "a second repository");
    assert!(message.contains("another repository"), "{message}");
    assert!(questions.is_empty());
    let review = specengine_cli::review(
        &env,
        &specengine_cli::Globals::default(),
        &specengine_cli::ReviewRequest {
            id: id.clone(),
            git: pair.git_env(&other),
        },
    );
    cannot(&review, "review from a second repository");
    let inbox = specengine_cli::inbox(
        &env,
        &specengine_cli::Globals::default(),
        &specengine_cli::InboxRequest {
            all: true,
            git: pair.git_env(&other),
        },
    )
    .unwrap();
    assert!(inbox.proposals.is_empty(), "{inbox:?}");
    assert_eq!(inbox.notes.len(), 1, "{inbox:?}");
    assert_eq!(pair.state(), before);
    assert_eq!(pair.events().len(), events, "no event");

    // The worktree removed (last: it cannot come back).
    pair.git.git(
        &pair.main,
        &[
            "worktree",
            "remove",
            "--force",
            pair.linked.to_str().unwrap(),
        ],
    );
    assert!(!pair.linked.exists());
    let message = assert_cannot_apply(&pair, &id, "the worktree removed", 1);
    assert!(
        message.contains(pair.linked.to_str().unwrap()) && message.contains("no longer exists"),
        "names the missing worktree: {message}"
    );
}

/// AC-11 (08 §3 AC-4b in part; AC-20 on both fixtures): two proposals on
/// one section, on different lines: the second rebases, both edits land,
/// two commits; a third overlapping the first: refused (exit 1) printing
/// the conflict, `preview: conflicts` first. M: overwrite; take the
/// proposal's side.
#[test]
fn ac11_a_later_proposal_rebases_or_is_refused_with_the_conflict() {
    for case in &CASES {
        let pair = Pair::of("pa-ac11", case);
        let first = pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
        let second = pair.propose_edit(&pair.linked, case.target, case.second.0, case.second.1);
        let third = pair.propose_edit(
            &pair.linked,
            case.target,
            case.overlapping.0,
            case.overlapping.1,
        );
        let base = pair.rev(&pair.linked, "HEAD");
        pair.approve_ok(&pair.main, &first);
        assert_eq!(
            pair.review_ok(&pair.main, &second)
                .document
                .preview
                .map(|p| p.as_str()),
            Some("rebases"),
            "{}",
            case.fixture
        );
        let review = pair.review_ok(&pair.main, &third);
        assert_eq!(
            review.document.preview.map(|p| p.as_str()),
            Some("conflicts")
        );
        assert!(
            review
                .document
                .conflict
                .as_deref()
                .unwrap()
                .contains("<<<<<<< current")
        );

        let (outcome, questions) =
            pair.approve_answer(&pair.main, &second, true, pair.git_env(&pair.main));
        let outcome = outcome.unwrap();
        assert_eq!(
            outcome.exit(),
            Exit::Answered,
            "{}: {outcome:?}",
            case.fixture
        );
        assert!(questions[0].ends_with(" (rebases)? [y/N]"), "{questions:?}");
        assert_eq!(commits(&pair, &pair.main, &format!("{base}..t1")).len(), 2);
        let (span_now, text_now) = pair.span(&pair.linked, case.target);
        assert!(
            text_now.contains(case.first.1) && text_now.contains(case.second.1),
            "{}: both edits: {text_now}",
            case.fixture
        );

        let before = pair.state();
        let outcome = pair.approve(&pair.main, &third);
        let reason = refused(&outcome, "overlapping");
        assert!(reason.contains("(step 5)"), "{reason}");
        let (stdout, json) = printed(outcome.as_ref().unwrap());
        assert!(
            stdout.contains("<<<<<<< current")
                && stdout.contains("=======")
                && stdout.contains(">>>>>>> proposed")
                && stdout.contains(case.first.1)
                && stdout.contains(case.overlapping.1),
            "{}: the conflict printed:\n{stdout}",
            case.fixture
        );
        assert_eq!(json_of(&json)["preview"], "conflicts");
        assert_eq!(pair.state(), before, "{}: refused", case.fixture);
        assert_eq!(pair.span(&pair.linked, case.target).0, span_now);
        assert_eq!(
            pair.events_of(&third).last().unwrap(),
            &("proposal.apply_failed".to_owned(), Some(5))
        );
    }
}

/// AC-12: an edit above the target, committed after the proposal was
/// made, survives; exactly the span is replaced. M: splice at stored
/// offsets.
#[test]
fn ac12_an_edit_above_the_target_survives() {
    let pair = Pair::new("pa-ac12", "spec-a");
    let path = "docs/spec/movement/sprint.md";
    let id = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );
    replace(
        &pair.linked,
        path,
        "Hold the sprint key to run;",
        "Hold the sprint key down, firmly and without letting go, to run;",
    );
    pair.commit_all(&pair.linked, "an edit above");
    let above = read_text(&pair.linked, path);
    pair.approve_ok(&pair.main, &id);
    assert_eq!(
        read_text(&pair.linked, path),
        edit(&above, "the sprint ends;", "the sprint ends at once;")
    );
}

/// AC-13: the target's directory a symlink to a copy outside the
/// worktree: exit 1, the outside file unchanged.
/// M: no symlink check.
#[test]
fn ac13_a_symlinked_directory_is_refused_and_the_outside_file_kept() {
    let pair = Pair::new("pa-ac13", "spec-a");
    let id = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );
    let outside = pair.scratch.join("outside/movement");
    copy_dir(&pair.linked.join("docs/spec/movement"), &outside);
    fs::remove_dir_all(pair.linked.join("docs/spec/movement")).unwrap();
    std::os::unix::fs::symlink(&outside, pair.linked.join("docs/spec/movement")).unwrap();
    let kept = fs::read(outside.join("sprint.md")).unwrap();
    let before = pair.state();
    let reason = refused(&pair.approve(&pair.main, &id), "a symlinked directory");
    // Refused by the walk's rule (no symlink component), not only by git
    // seeing the directory replaced.
    assert!(
        reason.contains("(step 3)") && reason.contains("symlink"),
        "{reason}"
    );
    assert_eq!(fs::read(outside.join("sprint.md")).unwrap(), kept);
    assert_eq!(pair.state(), before);
    assert_eq!(pair.proposal(&id).status.as_str(), "open");
}

/// AC-14: `GIT_DIR`, `GIT_WORK_TREE` and `GIT_INDEX_FILE` of the caller
/// point at another repository: the proposal still binds to the linked
/// worktree, and the commit lands only there; the other repository's
/// `HEAD`, index and files are untouched. M: pass them through.
#[test]
fn ac14_the_callers_git_variables_never_reach_the_apply() {
    let pair = Pair::new("pa-ac14", "spec-a");
    let other = pair.scratch.dir("other");
    write(&other, "notes.txt", "another repository\n");
    pair.git.init(&other);
    pair.commit_all(&other, "other");
    let other_head = pair.rev(&other, "HEAD");
    let other_index = fs::read(other.join(".git/index")).unwrap();
    let git_dir = other.join(".git");
    let index_file = other.join(".git/index");
    let vars = [
        ("GIT_DIR", git_dir.as_os_str()),
        ("GIT_WORK_TREE", other.as_os_str()),
        ("GIT_INDEX_FILE", index_file.as_os_str()),
    ];

    let (hash, text) = pair.span(&pair.linked, "EDGE-SPRINT-EMPTY");
    let mut request = pair.request(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        &hash,
        &edit(&text, "the sprint ends;", "the sprint ends at once;"),
    );
    request.git = with_vars(&request.git, &vars);
    let outcome = pair.propose_with(&pair.linked, request).unwrap();
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let stored = pair.proposal("PR-0001");
    assert_eq!(stored.place.worktree, pair.linked.to_str().unwrap());
    assert_eq!(stored.place.branch, "t1");

    let base = pair.rev(&pair.linked, "HEAD");
    let main_head = pair.rev(&pair.main, "HEAD");
    let git = with_vars(&pair.git_env(&pair.main), &vars);
    let (outcome, _) = pair.approve_answer(&pair.main, "PR-0001", true, git);
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(commits(&pair, &pair.main, &format!("{base}..t1")).len(), 1);
    assert_eq!(pair.rev(&pair.main, "HEAD"), main_head);
    assert_eq!(pair.rev(&other, "HEAD"), other_head);
    assert_eq!(pair.git_text(&other, &["rev-list", "--count", "HEAD"]), "1");
    assert_eq!(fs::read(other.join(".git/index")).unwrap(), other_index);
    assert_eq!(read_text(&other, "notes.txt"), "another repository\n");
    assert_eq!(pair.git_text(&other, &["status", "--porcelain"]), "");
}

/// Installs `body` as the repository's `pre-commit` hook (the common
/// dir's `hooks/`, shared by the linked worktree).
fn hook(pair: &Pair, body: &str) {
    let hooks = pair.main.join(".git/hooks");
    fs::create_dir_all(&hooks).unwrap();
    let path = hooks.join("pre-commit");
    fs::write(&path, body).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// AC-15: a `pre-commit` hook that exits 1: exit 1 (step 9), the bytes
/// restored, refused; the hook's marker exists. A hook that only records
/// it ran: applied, the marker exists. M: no restore; `--no-verify`.
#[test]
fn ac15_a_failing_pre_commit_hook_restores_the_bytes() {
    let pair = Pair::new("pa-ac15", "spec-a");
    let id = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );
    let marker = pair.scratch.join("hook-ran");
    hook(
        &pair,
        &format!(
            "#!/bin/sh\necho refused by the hook >&2\ntouch '{}'\nexit 1\n",
            marker.display()
        ),
    );
    let before = pair.state();
    let outcome = pair.approve(&pair.main, &id);
    let reason = refused(&outcome, "a failing hook");
    assert!(reason.contains("(step 9)"), "{reason}");
    assert!(marker.exists(), "the hook ran");
    assert_eq!(
        pair.state(),
        before,
        "refused: the bytes restored, status open"
    );
    assert_eq!(
        pair.events_of(&id).last().unwrap(),
        &("proposal.apply_failed".to_owned(), Some(9))
    );
    let stored = pair.proposal(&id);
    assert!(
        stored.decided_by.is_none() && stored.applied_commit.is_none(),
        "{stored:?}"
    );

    fs::remove_file(&marker).unwrap();
    hook(
        &pair,
        &format!("#!/bin/sh\ntouch '{}'\nexit 0\n", marker.display()),
    );
    pair.approve_ok(&pair.main, &id);
    assert!(marker.exists(), "hooks run on the apply commit");
    assert_eq!(pair.proposal(&id).status.as_str(), "applied");
}

/// AC-16: approving an applied proposal exits 1 naming its commit; set back
/// to `approved` by SQL (`applied_commit` cleared), its commit on the
/// branch: `applied` again, the same commit, no new commit, no prompt.
/// M: trailer lookup dropped.
#[test]
fn ac16_an_applied_or_completed_proposal_is_never_applied_twice() {
    let pair = Pair::new("pa-ac16", "spec-a");
    let id = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );
    pair.approve_ok(&pair.main, &id);
    let head = pair.rev(&pair.linked, "t1");
    let before = pair.state();
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let reason = refused(&outcome, "applied");
    assert!(reason.contains(&head), "names its commit: {reason}");
    assert!(questions.is_empty());
    assert_eq!(pair.state(), before);

    pair.sql("update proposals set status='approved', applied_commit=NULL");
    assert_eq!(pair.proposal(&id).status.as_str(), "approved");
    let review = pair.review_ok(&pair.main, &id);
    assert!(
        review
            .document
            .notes
            .iter()
            .any(|note| note.contains(&head)),
        "review notes the completing commit: {review:?}"
    );
    let events = pair.events().len();
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    let outcome = outcome.unwrap();
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(
        questions.is_empty(),
        "a completion asks nothing: {questions:?}"
    );
    assert_eq!(pair.rev(&pair.linked, "t1"), head, "no new commit");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "applied");
    assert_eq!(stored.applied_commit.as_deref(), Some(head.as_str()));
    assert_eq!(pair.events().len(), events + 1);
    assert_eq!(pair.events().last().unwrap().event_type, "proposal.applied");
    assert_eq!(
        State {
            proposals: Vec::new(),
            ..pair.state()
        },
        worktrees(&before)
    );
}

/// AC-17: `reject --reason x`: `rejected`, the reason in `review`, files
/// and commits unchanged, one `proposal.rejected`; `approve` then exits 1.
/// The prompt names the target, path, branch and worktree; a declined
/// answer and an empty reason change nothing. M: approve after reject.
#[test]
fn ac17_reject_records_the_reason_and_approve_then_refuses() {
    let pair = Pair::new("pa-ac17", "spec-a");
    let id = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );
    let before = pair.state();

    let (outcome, _) = pair.reject_answer(&pair.main, &id, "", true);
    cannot(&outcome, "an empty reason");
    let (outcome, questions) = pair.reject_answer(&pair.main, &id, "x", false);
    refused(&outcome, "declined");
    assert_eq!(
        questions,
        [format!(
            "reject {id} (EDGE-SPRINT-EMPTY in docs/spec/movement/sprint.md on t1 in {})? [y/N]",
            pair.linked.display()
        )]
    );
    assert_eq!(pair.state(), before, "declined: nothing changed");
    assert_eq!(pair.events().len(), 1, "declined: no event");

    let (outcome, _) = pair.reject_answer(&pair.main, &id, "x", true);
    let outcome = outcome.unwrap();
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(printed(&outcome).0, format!("rejected {id}\n"));
    assert_eq!(worktrees(&pair.state()), worktrees(&before));
    let review = pair.review_ok(&pair.main, &id);
    assert_eq!(review.document.status.as_deref(), Some("rejected"));
    assert_eq!(review.document.decision_note.as_deref(), Some("x"));
    assert_eq!(review.document.decided_by.as_deref(), Some(DECIDER));
    assert!(review.document.preview.is_none(), "no preview once decided");
    assert!(printed(&review).0.contains("\ndecision_note:\n  x\n"));
    let rejected: Vec<_> = pair
        .events()
        .into_iter()
        .filter(|event| event.event_type == "proposal.rejected")
        .collect();
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0].payload["reason"], "x");

    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
    refused(&outcome, "approve after reject");
    assert!(questions.is_empty());
    let (outcome, _) = pair.reject_answer(&pair.main, &id, "again", true);
    refused(&outcome, "reject twice");
    assert_eq!(worktrees(&pair.state()), worktrees(&before));
    assert_eq!(pair.proposal(&id).status.as_str(), "rejected");
}

/// A declined consent: exit 1, nothing changed, no event (owner's answer
/// 1); only `y` or `yes` consents in `main` (not testable here).
#[test]
fn a_declined_approve_changes_nothing_and_logs_nothing() {
    let pair = Pair::new("pa-decline", "spec-a");
    let id = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );
    let before = pair.state();
    let (outcome, questions) =
        pair.approve_answer(&pair.main, &id, false, pair.git_env(&pair.main));
    refused(&outcome, "declined");
    assert_eq!(questions.len(), 1);
    assert_eq!(pair.state(), before);
    assert_eq!(pair.events().len(), 1);
}

/// AC-19, the event half: one event per state change, `seq` rising by
/// one, each payload JSON with `id`: created ×3, approved and applied,
/// apply_failed (step 5), rejected. M: skip an event.
#[test]
fn ac19_one_event_per_state_change_seq_rising() {
    let pair = Pair::new("pa-ac19-events", "spec-a");
    let target = "EDGE-SPRINT-EMPTY";
    let a = pair.propose_edit(
        &pair.linked,
        target,
        "the sprint ends;",
        "the sprint ends at once;",
    );
    let b = pair.propose_edit(
        &pair.linked,
        target,
        "the sprint ends;",
        "the sprint stops;",
    );
    let c = pair.propose_edit(&pair.linked, "EDGE-STAM-ZERO", "immediately", "at once");
    pair.approve_ok(&pair.main, &a);
    refused(&pair.approve(&pair.main, &b), "overlap");
    let (outcome, _) = pair.reject_answer(&pair.main, &b, "Overlaps.", true);
    assert_eq!(outcome.unwrap().exit(), Exit::Answered);
    let events = pair.events();
    let seen: Vec<(i64, String, String)> = events
        .iter()
        .map(|event| {
            (
                event.seq,
                event.event_type.clone(),
                event.payload["id"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let want = [
        ("proposal.created", &a),
        ("proposal.created", &b),
        ("proposal.created", &c),
        ("proposal.approved", &a),
        ("proposal.applied", &a),
        ("proposal.apply_failed", &b),
        ("proposal.rejected", &b),
    ];
    assert_eq!(seen.len(), want.len(), "{seen:?}");
    for (index, ((seq, kind, id), (want_kind, want_id))) in seen.iter().zip(want).enumerate() {
        assert_eq!((kind.as_str(), id), (want_kind, want_id), "{seen:?}");
        if index > 0 {
            assert!(*seq > seen[index - 1].0, "seq rises: {seen:?}");
        }
    }
    let failed = &events[5];
    assert_eq!(failed.payload["step"], 5);
    assert!(failed.payload["reason"].as_str().unwrap().contains(target));
    assert!(events.iter().all(|event| event.project == "lantern-keep"));
    assert_eq!(events[0].at, common::proposal::NOW);
    assert_eq!(events[3].at, LATER);
}

/// "Apply" 7 and "Exit codes": no git identity in the recorded worktree:
/// exit 2 before the prompt, one `apply_failed` at step 7, nothing
/// written.
#[test]
fn no_git_identity_exits_2_before_the_prompt() {
    let pair = Pair::new("pa-ident", "spec-a");
    let id = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );
    pair.git
        .git(&pair.main, &["config", "user.useConfigOnly", "true"]);
    let git = pair
        .git_env(&pair.main)
        .without_var("GIT_COMMITTER_NAME")
        .without_var("GIT_COMMITTER_EMAIL")
        .without_var("GIT_AUTHOR_NAME")
        .without_var("GIT_AUTHOR_EMAIL")
        .with_var("EMAIL", "");
    let before = pair.state();
    let (outcome, questions) = pair.approve_answer(&pair.main, &id, true, git);
    let message = cannot(&outcome, "no identity");
    assert!(message.contains("(step 7)"), "{message}");
    assert!(
        questions.is_empty(),
        "asked before the prompt: {questions:?}"
    );
    assert_eq!(pair.state(), before);
    assert_eq!(
        pair.events_of(&id).last().unwrap(),
        &("proposal.apply_failed".to_owned(), Some(7))
    );
}

/// The commit message holds the rationale verbatim (`--cleanup=verbatim`:
/// a `#` line and trailing spaces kept); the file keeps its mode (temp
/// sibling + rename); no temp sibling is left.
#[test]
fn the_rationale_is_verbatim_and_the_mode_is_kept() {
    let pair = Pair::new("pa-verbatim", "spec-a");
    let path = "docs/spec/movement/sprint.md";
    fs::set_permissions(pair.linked.join(path), fs::Permissions::from_mode(0o755)).unwrap();
    pair.git
        .git(&pair.linked, &["update-index", "--chmod=+x", path]);
    pair.git.commit(&pair.linked, "executable");
    let (hash, text) = pair.span(&pair.linked, "EDGE-SPRINT-EMPTY");
    let mut request = pair.request(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        &hash,
        &edit(&text, "the sprint ends;", "the sprint ends at once;"),
    );
    request.rationale = "# Not a comment\nTrailing spaces   \n\n  indented".to_owned();
    pair.propose_with(&pair.linked, request).unwrap();
    pair.approve_ok(&pair.main, "PR-0001");
    let message = pair
        .git
        .git(&pair.main, &["log", "-1", "--format=%B", "t1"]);
    let message = String::from_utf8(message).unwrap();
    assert!(
        message.starts_with("spec: apply PR-0001\n\n# Not a comment\nTrailing spaces   \n\n  indented\n\nProposal: PR-0001\n"),
        "{message:?}"
    );
    assert_eq!(
        fs::metadata(pair.linked.join(path))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    assert!(
        pair.git_text(&pair.linked, &["ls-files", "-s", path])
            .starts_with("100755 "),
        "the committed mode"
    );
    let names: Vec<String> = fs::read_dir(pair.linked.join("docs/spec/movement"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().all(|name| !name.starts_with('.')), "{names:?}");
}

/// A7: `review` (with its preview: apply steps 2–6) writes nothing in
/// either worktree, `.git` included.
#[test]
fn review_writes_nothing_in_a_worktree() {
    let pair = Pair::new("pa-readonly", "spec-a");
    let id = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );
    pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "is not a reference",
        "is never a reference",
    );
    replace(
        &pair.linked,
        "docs/spec/movement/sprint.md",
        "the sprint ends;",
        "the sprint ends now;",
    );
    pair.commit_all(
        &pair.linked,
        "a change: the second rebases, the first conflicts",
    );
    let main = common::snapshot(&pair.main);
    let linked = common::snapshot(&pair.linked);
    let first = pair.review_ok(&pair.main, &id);
    assert_eq!(
        first.document.preview.map(|p| p.as_str()),
        Some("conflicts")
    );
    let second = pair.review_ok(&pair.linked, "PR-0002");
    assert_eq!(second.document.preview.map(|p| p.as_str()), Some("rebases"));
    pair.inbox(&pair.main, true).unwrap();
    assert!(
        common::snapshot(&pair.main) == main,
        "the main worktree and the git dir untouched"
    );
    assert!(
        common::snapshot(&pair.linked) == linked,
        "the linked worktree untouched"
    );
}

/// ADR-0032 binding with a project root below the worktree's top
/// (`root_rel` not empty): proposed in `t1/proj`, approved from
/// `main/proj`: the commit changes `proj/<target_path>` only, the prompt
/// names the top-relative path.
#[test]
fn a_root_below_the_top_applies_at_its_top_relative_path() {
    let scratch = common::Scratch::new("pa-subdir");
    let git = common::git::Sandbox::new(scratch.path());
    let main = scratch.dir("main");
    copy_dir(&common::fixture("spec-a"), &main.join("proj"));
    write(&main, "README.txt", "outside the root\n");
    git.init(&main);
    git.add_all(&main);
    git.commit(&main, "a project below the top");
    let linked = scratch.join("t1");
    git.git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "t1",
            linked.to_str().unwrap(),
        ],
    );
    let linked = fs::canonicalize(linked).unwrap();
    let home = scratch.home("h");
    let env = |cwd: &std::path::Path| specengine_cli::Env {
        cwd: cwd.to_path_buf(),
        home: Some(home.clone().into_os_string()),
        xdg_data_home: None,
    };
    let genv = |cwd: &std::path::Path| specengine_store::GitEnv::new(cwd, git.vars());
    let globals = specengine_cli::Globals::default();
    let shown = specengine_cli::show(
        &env(&linked.join("proj")),
        &globals,
        &specengine_cli::ShowRequest {
            reference: "EDGE-SPRINT-EMPTY".to_owned(),
            links: false,
            archive: false,
        },
    )
    .unwrap();
    let node = &shown.nodes[0];
    let new_text = edit(&node.text, "the sprint ends;", "the sprint ends at once;");
    let outcome = specengine_cli::propose(
        &env(&linked.join("proj")),
        &globals,
        &specengine_cli::ProposeRequest {
            target: "EDGE-SPRINT-EMPTY".to_owned(),
            base: node.span_hash.clone(),
            text: specengine_cli::ProposedText::Given(new_text.clone().into_bytes()),
            rationale: "Below the top.".to_owned(),
            author_role: None,
            author_model: None,
            run: None,
            now: common::proposal::NOW.to_owned(),
            git: genv(&linked.join("proj")),
        },
    )
    .unwrap();
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(outcome.document.worktree.as_deref(), linked.to_str());
    assert_eq!(
        outcome.document.target_path.as_deref(),
        Some("docs/spec/movement/sprint.md")
    );
    let base = git.git_text(&linked, &["rev-parse", "HEAD"]);
    let mut questions = Vec::new();
    let mut consent = |question: &str| {
        questions.push(question.to_owned());
        true
    };
    let outcome = specengine_cli::approve(
        &env(&main.join("proj")),
        &globals,
        &specengine_cli::ApproveRequest {
            id: "PR-0001".to_owned(),
            note: None,
            now: LATER.to_owned(),
            git: genv(&main.join("proj")),
        },
        &mut consent,
    )
    .unwrap();
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        questions,
        [format!(
            "apply PR-0001 to proj/docs/spec/movement/sprint.md on t1 in {} (applies)? [y/N]",
            linked.display()
        )]
    );
    let head = git.git_text(&linked, &["rev-parse", "t1"]);
    assert_eq!(
        git.git_text(&main, &["diff", "--name-only", &base, &head]),
        "proj/docs/spec/movement/sprint.md"
    );
    assert!(
        read_text(&linked, "proj/docs/spec/movement/sprint.md")
            .contains("the sprint ends at once;")
    );
    assert!(!read_text(&main, "proj/docs/spec/movement/sprint.md").contains("at once"));
}

/// "Reject": `decided_by` from the recorded worktree, else (it is gone)
/// the current repository's identity; a rejected proposal is never
/// previewed.
#[test]
fn reject_falls_back_to_the_current_repository_identity() {
    let pair = Pair::new("pa-reject-fallback", "spec-a");
    let id = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );
    pair.git.git(
        &pair.main,
        &[
            "worktree",
            "remove",
            "--force",
            pair.linked.to_str().unwrap(),
        ],
    );
    let (outcome, questions) = pair.reject_answer(&pair.main, &id, "Gone.", true);
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(questions.len(), 1);
    let stored = pair.proposal(&id);
    assert_eq!(stored.status.as_str(), "rejected");
    assert_eq!(stored.decided_by.as_deref(), Some(DECIDER));
    assert_eq!(stored.decision_note.as_deref(), Some("Gone."));
}

/// "Apply" 4: the target moved to another file after creation: refused
/// at step 4 (exit 1) naming the new holder, nothing written.
#[test]
fn a_target_moved_to_another_file_is_refused_at_step_4() {
    let pair = Pair::new("pa-moved", "spec-a");
    let id = pair.propose_edit(&pair.linked, "EDGE-STAM-ZERO", "immediately", "at once");
    let text = read_text(&pair.linked, "docs/spec/movement/stamina.md");
    let cut = text.find("## Depletion").unwrap();
    write(&pair.linked, "docs/spec/movement/stamina.md", &text[..cut]);
    write(
        &pair.linked,
        "docs/spec/movement/zero.md",
        format!("---\nclass: canon\n---\n\n# Zero\n\n{}", &text[cut..]),
    );
    pair.commit_all(&pair.linked, "moved");
    let before = pair.state();
    let reason = refused(&pair.approve(&pair.main, &id), "moved");
    assert!(
        reason.contains("(step 4)") && reason.contains("docs/spec/movement/zero.md"),
        "{reason}"
    );
    assert_eq!(pair.state(), before);
}

/// Paths reach git literally: a target file whose name holds a space,
/// brackets and `*` is committed alone.
#[test]
fn a_glob_shaped_file_name_is_committed_literally() {
    let pair = Pair::new("pa-literal", "spec-a");
    let odd = "docs/spec/odd name [x]*.md";
    write(
        &pair.linked,
        odd,
        "---\nclass: canon\n---\n\n# Odd\n\n## Part {#RULE-ODD-PART}\n\nText here.\n",
    );
    write(
        &pair.linked,
        "docs/spec/odd name x1.md",
        "# Plain\n\nA neighbour the glob would match.\n",
    );
    pair.commit_all(&pair.linked, "odd names");
    write(
        &pair.linked,
        "docs/spec/odd name x1.md",
        "# Plain\n\nModified, uncommitted.\n",
    );
    let id = pair.propose_edit(&pair.linked, "RULE-ODD-PART", "Text here.", "Text there.");
    let base = pair.rev(&pair.linked, "HEAD");
    pair.approve_ok(&pair.main, &id);
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(changed(&pair, &pair.main, &base, &head), [odd]);
    assert!(read_text(&pair.linked, odd).contains("Text there."));
    assert_eq!(
        read_text(&pair.linked, "docs/spec/odd name x1.md"),
        "# Plain\n\nModified, uncommitted.\n"
    );
}

/// A8: no line-ending normalisation: a CRLF file's span carries its CRs,
/// and the applied file keeps CRLF everywhere.
#[test]
fn crlf_files_keep_their_line_ends() {
    let pair = Pair::new("pa-crlf", "spec-a");
    let path = "docs/spec/movement/sprint.md";
    let text = read_text(&pair.linked, path).replace('\n', "\r\n");
    write(&pair.linked, path, &text);
    pair.commit_all(&pair.linked, "crlf");
    let (_, span) = pair.span(&pair.linked, "EDGE-SPRINT-EMPTY");
    assert!(span.contains("\r\n") && !span.ends_with('\n'), "{span:?}");
    let id = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );
    pair.approve_ok(&pair.main, &id);
    let after = read_text(&pair.linked, path);
    assert_eq!(
        after,
        text.replace("the sprint ends;", "the sprint ends at once;")
    );
}

/// spec-b: a feature-scoped criterion (stored `dry-run/CRIT-01`) applies;
/// `--note` is kept as the decision's note.
#[test]
fn a_feature_scoped_target_applies_with_the_note() {
    let pair = Pair::new("pa-scoped", "spec-b");
    let id = pair.propose_edit(
        &pair.linked,
        "dry-run/CRIT-01",
        "sync --dry-run",
        "sync --dry-run --all",
    );
    let mut consent = |_: &str| true;
    let outcome = specengine_cli::approve(
        &pair.env(&pair.main),
        &specengine_cli::Globals::default(),
        &specengine_cli::ApproveRequest {
            id: id.clone(),
            note: Some("Looks right.".to_owned()),
            now: LATER.to_owned(),
            git: pair.git_env(&pair.main),
        },
        &mut consent,
    )
    .unwrap();
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let stored = pair.proposal(&id);
    assert_eq!(stored.target_id, "dry-run/CRIT-01");
    assert_eq!(stored.decision_note.as_deref(), Some("Looks right."));
    assert!(read_text(&pair.linked, "docs/features/dry-run.md").contains("sync --dry-run --all"));
}

/// "CLI": `--config` other than `<root>/specengine.toml` exits 2 for the
/// queue's commands; the root's own file is accepted.
#[test]
fn only_the_roots_own_config_is_accepted() {
    let pair = Pair::new("pa-config", "spec-a");
    let (hash, span) = pair.span(&pair.linked, "EDGE-SPRINT-EMPTY");
    let request = pair.request(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        &hash,
        &edit(&span, "the sprint ends;", "the sprint ends at once;"),
    );
    let other = pair.scratch.join("other.toml");
    fs::write(&other, read_text(&pair.linked, "specengine.toml")).unwrap();
    let globals = specengine_cli::Globals {
        root: None,
        config: Some(other),
    };
    let outcome = specengine_cli::propose(&pair.env(&pair.linked), &globals, &request);
    let message = cannot(&outcome, "another config");
    assert!(message.contains("--config"), "{message}");
    let inbox = specengine_cli::inbox(
        &pair.env(&pair.linked),
        &globals,
        &specengine_cli::InboxRequest {
            all: true,
            git: pair.git_env(&pair.linked),
        },
    );
    cannot(&inbox, "inbox with another config");
    assert!(pair.proposals().is_empty());
    let globals = specengine_cli::Globals {
        root: None,
        config: Some("specengine.toml".into()),
    };
    let outcome = specengine_cli::propose(&pair.env(&pair.linked), &globals, &request).unwrap();
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
}
