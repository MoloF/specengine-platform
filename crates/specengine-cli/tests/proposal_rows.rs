//! docs/features/proposal-apply.md, what the queue's commands do with
//! proposals and rows that must not be applied or shown as they are
//! (review of iteration 1, fixed in iteration 2), through the library and,
//! where the printed streams are the subject, the `spec` binary:
//!
//! - no-op edits: refused at creation ("no change") and at apply step 5
//!   ("already in place": no prompt, nothing written or committed);
//! - rows put in the queue by hand (`sqlite3`) that no build writes: a
//!   `base_commit` `--output=…`, branches with a line break or starting
//!   with `-`, a `base_commit` with a line break, an author run with a line
//!   break: `review`, `approve`, `reject` exit 2 naming the row and the
//!   column, `inbox` skips each with one note (text, stderr and JSON) and
//!   exits 0; no file is ever written by git;
//! - control characters of agent-written text and of paths escaped as
//!   `\u{xx}` in the consent questions, `review`, `inbox` and errors;
//! - the orphan rule: a proposal whose repository no longer exists is
//!   rejected from another repository of the project (open or approved),
//!   while `review` and `approve` exit 2 naming `spec reject`; an applied
//!   orphan is refused; an approved proposal whose repository exists is
//!   asked about by name (iteration 3);
//! - iteration 3: `inbox` counts another existing repository's proposals
//!   and names a gone repository's; a CRLF file's lines keep their CR while
//!   a lone CR and the bidirectional controls are escaped;
//! - iteration 4: the gone note names at most ten IDs and counts the rest;
//! - iteration 6: the gone texts say `spec reject` takes a proposal out
//!   "unless its commit is in history"; a range-like stored branch
//!   (`t1..main`) is a row that does not decode, never handed to git.

#![cfg(unix)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::proposal::{
    CASES, DECIDER, NOW, Pair, cannot, edit, json_of, printed, printed_inbox, refused,
};
use common::{read, replace};
use specengine_cli::{Exit, Message, Preview};

const TARGET: &str = "EDGE-SPRINT-EMPTY";
const PATH: &str = "docs/spec/movement/sprint.md";
const FROM: &str = "the sprint ends;";
const TO: &str = "the sprint ends at once;";

/// The control characters the tests plant: ESC (with an erase-line
/// sequence), BEL, and the C1 CSI.
const CONTROLS: [char; 3] = ['\u{1b}', '\u{7}', '\u{9b}'];

fn assert_escaped(text: &str, context: &str) {
    assert!(
        !text.chars().any(|c| CONTROLS.contains(&c)),
        "{context}: a raw control character: {text:?}"
    );
    for escaped in ["\\u{1b}[2K", "\\u{7}", "\\u{9b}"] {
        assert!(text.contains(escaped), "{context}: no {escaped}: {text:?}");
    }
}

/// Every path under `dir` whose name contains `needle`.
fn named(dir: &Path, needle: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name().to_string_lossy().contains(needle) {
            found.push(path.clone());
        }
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            found.extend(named(&path, needle));
        }
    }
    found
}

/// Files, refs, rows, event count.
type RawState = (Vec<(String, Option<Vec<u8>>)>, String, String, String);

/// Both worktrees' files, every ref, the stored rows and the event count,
/// read without decoding the queue (its bad rows would not decode).
fn raw_state(pair: &Pair) -> RawState {
    let files = [&pair.main, &pair.linked]
        .iter()
        .flat_map(|dir| common::snapshot(dir))
        .filter(|(path, _)| path != ".git" && !path.starts_with(".git/"))
        .collect();
    let refs = pair.git_text(
        &pair.main,
        &["for-each-ref", "--format=%(refname) %(objectname)"],
    );
    (
        files,
        refs,
        pair.sql("select * from proposals order by id"),
        pair.sql("select count(*) from events"),
    )
}

/// A proposal whose text is the span as it is: refused at creation, exit 1,
/// "no change", nothing stored, on both fixtures (review of iteration 1:
/// a no-op was stored and could never be applied). M: the creation no-op
/// check dropped.
#[test]
fn a_no_op_proposal_is_refused_at_creation() {
    for case in &CASES {
        let pair = Pair::of("pa-noop-create", case);
        let (hash, text) = pair.span(&pair.linked, case.target);
        let before = pair.state();
        let outcome = pair.propose(&pair.linked, case.target, &hash, &text);
        let reason = refused(&outcome, case.fixture);
        assert_eq!(
            reason,
            format!(
                "no change: `{}` with the new text of `{}` is byte for byte the file as read; \
                 nothing to propose",
                case.path, case.target
            ),
            "{}",
            case.fixture
        );
        assert_eq!(pair.state(), before, "{}", case.fixture);
        assert!(
            pair.proposals().is_empty(),
            "{}: nothing stored",
            case.fixture
        );
        assert!(pair.events().is_empty(), "{}: nothing logged", case.fixture);
    }
}

/// The same edit committed by hand on `t1` after the proposal: `review`
/// gives preview `unavailable` with a step-5 "already in place" note;
/// `approve` is refused at step 5 before the prompt, nothing written or
/// committed, `open`, one `apply_failed` (review of iteration 1, my
/// iteration-1 defect: it prompted, approved and failed at step 9 with an
/// empty reason). Both fixtures. M: the apply-time no-op check dropped.
#[test]
fn an_edit_already_in_place_is_refused_at_step_5_without_a_prompt() {
    for case in &CASES {
        let pair = Pair::of("pa-noop-apply", case);
        let id = pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
        replace(&pair.linked, case.path, case.first.0, case.first.1);
        pair.commit_all(&pair.linked, "the same edit by hand");

        let review = pair.review_ok(&pair.main, &id);
        assert_eq!(
            review.document.preview,
            Some(Preview::Unavailable),
            "{}: {review:?}",
            case.fixture
        );
        assert!(
            review
                .document
                .notes
                .iter()
                .any(|note| note.contains("(step 5)") && note.contains("already in place")),
            "{}: {:?}",
            case.fixture,
            review.document.notes
        );

        let before = pair.state();
        let (outcome, questions) =
            pair.approve_answer(&pair.main, &id, true, pair.git_env(&pair.main));
        let reason = refused(&outcome, case.fixture);
        assert!(reason.contains("(step 5)"), "{}: {reason}", case.fixture);
        assert!(
            reason.contains("already in place"),
            "{}: {reason}",
            case.fixture
        );
        assert!(
            questions.is_empty(),
            "{}: no prompt: {questions:?}",
            case.fixture
        );
        let after = pair.state();
        assert_eq!(
            (after.linked_files, after.refs, after.linked_status),
            (before.linked_files, before.refs, before.linked_status),
            "{}: nothing written or committed",
            case.fixture
        );
        assert_eq!(
            pair.proposal(&id).status.as_str(),
            "open",
            "{}",
            case.fixture
        );
        assert_eq!(
            pair.events_of(&id),
            [
                ("proposal.created".to_owned(), None),
                ("proposal.apply_failed".to_owned(), Some(5))
            ],
            "{}",
            case.fixture
        );
    }
}

/// Rows no build writes, put in by `sqlite3` (review of iteration 1: a
/// `base_commit` `--output=PWNED` made `spec review` run `git log
/// --output=…`; line breaks inject trailer lines): PR-0002 (`approved`, so
/// a review would look for its commit) `base_commit` `--output=<scratch>/
/// PWNED`; PR-0003 a branch with a line break and a `Proposal:` line;
/// PR-0004 the branch `-q`; PR-0005 a `base_commit` ending in a line
/// break; PR-0006 an author run with a line break. `review`, `approve`
/// (no prompt) and `reject` of each exit 2 naming the row and the column;
/// `inbox` lists PR-0001 and one note per bad row, by ID, in the library,
/// the binary's stderr (`note:`) and `--json` `notes`, exit 0; `--all`
/// the same notes; no file named `PWNED` anywhere. M: the stored-value
/// checks at read dropped.
#[test]
fn rows_no_build_writes_exit_2_and_inbox_skips_them_with_a_note() {
    let pair = Pair::new("pa-rows", "spec-a");
    for _ in 0..6 {
        pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    }
    let pwned = pair.scratch.join("PWNED");
    let base = pair.rev(&pair.linked, "HEAD");
    pair.sql(&format!(
        "update proposals set base_commit = '--output={}', status = 'approved', \
         decided_by = '{DECIDER}', decided_at = '{NOW}' where id = 'PR-0002'",
        pwned.display()
    ));
    pair.sql(
        "update proposals set branch = 't1' || char(10) || 'Proposal: PR-0009' \
         where id = 'PR-0003'",
    );
    pair.sql("update proposals set branch = '-q' where id = 'PR-0004'");
    pair.sql(&format!(
        "update proposals set base_commit = '{base}' || char(10) where id = 'PR-0005'"
    ));
    pair.sql(
        "update proposals set author = json_set(author, '$.run', \
         'r1' || char(10) || 'Proposal: PR-0009') where id = 'PR-0006'",
    );
    let bad = [
        ("PR-0002", "base_commit"),
        ("PR-0003", "branch"),
        ("PR-0004", "branch"),
        ("PR-0005", "base_commit"),
        ("PR-0006", "author"),
    ];
    let before = raw_state(&pair);

    for (id, column) in bad {
        let named = format!("proposal {id}: the stored `{column}` cannot be read");
        let message = cannot(&pair.review(&pair.main, id), id);
        assert!(message.contains(&named), "review {id}: {message}");
        let (outcome, questions) =
            pair.approve_answer(&pair.main, id, true, pair.git_env(&pair.main));
        let message = cannot(&outcome, id);
        assert!(message.contains(&named), "approve {id}: {message}");
        assert!(questions.is_empty(), "approve {id}: no prompt");
        let (outcome, questions) = pair.reject_answer(&pair.main, id, "Bad row.", true);
        let message = cannot(&outcome, id);
        assert!(message.contains(&named), "reject {id}: {message}");
        assert!(questions.is_empty(), "reject {id}: no prompt");
    }

    let notes: Vec<String> = bad
        .iter()
        .map(|(id, column)| format!("proposal {id}: the stored `{column}` cannot be read: "))
        .collect();
    let check_notes = |got: &[String], context: &str| {
        assert_eq!(got.len(), notes.len(), "{context}: {got:?}");
        for (note, want) in got.iter().zip(&notes) {
            assert!(note.starts_with(want), "{context}: {note:?} vs {want:?}");
            assert!(note.ends_with("; not listed"), "{context}: {note:?}");
            assert!(!note.contains('\n'), "{context}: one line: {note:?}");
        }
    };
    let outcome = pair.inbox(&pair.main, false).expect("inbox exits 0");
    let ids: Vec<&str> = outcome
        .proposals
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(ids, ["PR-0001"]);
    check_notes(&outcome.notes, "library inbox");
    let lines: Vec<String> = outcome.messages.iter().map(Message::line).collect();
    assert_eq!(
        lines,
        outcome
            .notes
            .iter()
            .map(|note| format!("note: {note}"))
            .collect::<Vec<_>>()
    );
    let (text, json) = printed_inbox(&outcome);
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.starts_with("PR-0001 | update | open | "), "{text}");
    let notes_json: Vec<String> = json_of(&json)["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .map(|note| note.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(notes_json, outcome.notes);
    let all = pair.inbox(&pair.main, true).expect("inbox --all exits 0");
    check_notes(&all.notes, "library inbox --all");

    let run = pair.spec_piped(&pair.main, &["inbox"], b"");
    run.code(0);
    assert_eq!(run.stdout, text, "{}", run.show());
    let stderr: Vec<String> = run
        .stderr_lines()
        .into_iter()
        .filter(|line| line.starts_with("note: proposal PR-"))
        .map(|line| line.trim_start_matches("note: ").to_owned())
        .collect();
    check_notes(&stderr, "spec inbox stderr");
    let run = pair.spec_piped(&pair.main, &["inbox", "--json"], b"");
    run.code(0);
    let notes_json: Vec<String> = run.json()["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .map(|note| note.as_str().unwrap().to_owned())
        .collect();
    check_notes(&notes_json, "spec inbox --json");
    let run = pair.spec_piped(&pair.main, &["inbox", "--all"], b"");
    run.code(0);
    let run = pair.spec_piped(&pair.main, &["review", "PR-0002"], b"");
    run.code(2);
    assert!(
        run.stderr
            .contains("proposal PR-0002: the stored `base_commit` cannot be read"),
        "{}",
        run.show()
    );

    assert_eq!(raw_state(&pair), before, "nothing changed");
    assert!(
        named(pair.scratch.path(), "PWNED").is_empty(),
        "no file written by git: {:?}",
        named(pair.scratch.path(), "PWNED")
    );
}

/// A range-like branch stored by hand (iteration 6, the reviewer's note:
/// `WorktreeGit::branch_commits_with_trailer` itself reads
/// `refs/heads/t1..main` as the range `main ^t1`; the queue's decode check
/// keeps such a name from git): PR-0001's branch `t1..main`, PR-0002's
/// `main..t1` (`t1` holding PR-0002's text committed by hand with its
/// trailer, which that range would find). `review`, `approve` and
/// `reject` of each exit 2 naming "proposal <ID>: the stored `branch`
/// cannot be read: \"<name>\" is no branch name", no prompt; `inbox` lists
/// PR-0003 and names each bad row once ("…; not listed"); nothing written
/// (files, refs, rows, events). M: `..` dropped from the branch-name check
/// at decode.
#[test]
fn a_range_like_stored_branch_is_unreadable_and_never_handed_to_git() {
    let pair = Pair::new("pa-rows-range", "spec-a");
    for _ in 0..3 {
        pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    }
    replace(&pair.linked, PATH, FROM, TO);
    pair.git.git(
        &pair.linked,
        &["commit", "-q", "-am", "by hand", "-m", "Proposal: PR-0002"],
    );
    pair.sql("update proposals set branch = 't1..main' where id = 'PR-0001'");
    pair.sql("update proposals set branch = 'main..t1' where id = 'PR-0002'");
    let bad = [("PR-0001", "t1..main"), ("PR-0002", "main..t1")];
    let named = |id: &str, name: &str| {
        format!("proposal {id}: the stored `branch` cannot be read: \"{name}\" is no branch name")
    };
    let before = raw_state(&pair);

    for (id, name) in bad {
        let named = named(id, name);
        let message = cannot(&pair.review(&pair.main, id), id);
        assert!(message.contains(&named), "review {id}: {message}");
        let (outcome, questions) =
            pair.approve_answer(&pair.main, id, true, pair.git_env(&pair.main));
        let message = cannot(&outcome, id);
        assert!(message.contains(&named), "approve {id}: {message}");
        assert!(questions.is_empty(), "approve {id}: no prompt");
        let (outcome, questions) = pair.reject_answer(&pair.main, id, "Bad row.", true);
        let message = cannot(&outcome, id);
        assert!(message.contains(&named), "reject {id}: {message}");
        assert!(questions.is_empty(), "reject {id}: no prompt");
    }

    let inbox = pair.inbox(&pair.main, false).expect("inbox exits 0");
    let ids: Vec<&str> = inbox
        .proposals
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(ids, ["PR-0003"]);
    let want: Vec<String> = bad
        .iter()
        .map(|(id, name)| format!("{}; not listed", named(id, name)))
        .collect();
    assert_eq!(inbox.notes, want);
    assert_eq!(raw_state(&pair), before, "nothing changed");
}

/// Control characters (ESC with an erase-line sequence, BEL, the C1 CSI)
/// in agent-written text (the rationale, the new text) and in a worktree
/// path are written `\u{xx}` in `review`'s text (fields and diff),
/// `inbox`'s line, both consent questions and an error naming the
/// worktree (review of iteration 1: `ESC[2K` reached the owner's terminal
/// raw). M: the escaping dropped.
#[test]
fn control_characters_are_escaped_for_the_owner() {
    let pair = Pair::new("pa-controls", "spec-a");
    let weird = pair.scratch.join("w\u{1b}[2K\u{7}\u{9b}x");
    pair.git.git(
        &pair.main,
        &["worktree", "add", "-q", "-b", "t9", weird.to_str().unwrap()],
    );
    let weird = fs::canonicalize(weird).unwrap();
    let (hash, text) = pair.span(&weird, TARGET);
    let new_text = edit(&text, FROM, "the sprint ends\u{1b}[2K\u{7}\u{9b};");
    let mut request = pair.request(&weird, TARGET, &hash, &new_text);
    request.rationale = "Say it.\u{1b}[2K\u{7}\u{9b}31m hidden\nSecond line.".to_owned();
    let outcome = pair.propose_with(&weird, request).expect("propose");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let id = outcome.document.id.clone().expect("an ID");
    assert_eq!(pair.proposal(&id).new_text, new_text, "stored as given");

    let review = pair.review_ok(&weird, &id);
    let (text, json) = printed(&review);
    assert_escaped(&text, "review text");
    let diff_line = text
        .lines()
        .find(|line| line.trim_start().starts_with("+At zero stamina"))
        .unwrap_or_else(|| panic!("a diff line: {text}"));
    assert_escaped(diff_line, "review diff");
    assert!(
        !json.chars().any(|c| c == '\u{1b}' || c == '\u{7}'),
        "review JSON: C0 escaped by JSON"
    );

    let inbox = pair.inbox(&weird, false).expect("inbox");
    assert_escaped(&printed_inbox(&inbox).0, "inbox text");

    let (outcome, questions) =
        pair.approve_answer(&pair.main, &id, false, pair.git_env(&pair.main));
    refused(&outcome, "declined approve");
    assert_eq!(questions.len(), 1);
    assert_escaped(&questions[0], "approve question");
    let (outcome, questions) = pair.reject_answer(&pair.main, &id, "No.", false);
    refused(&outcome, "declined reject");
    assert_eq!(questions.len(), 1);
    assert_escaped(&questions[0], "reject question");

    // An error naming a worktree path with control characters.
    pair.sql(&format!(
        "update proposals set worktree = '{}/gone' || char(27) || '[2K' || char(7) || \
         char(155) where id = '{id}'",
        pair.scratch.path().display()
    ));
    let message = cannot(&pair.approve(&pair.main, &id), "approve of a gone worktree");
    assert!(message.contains("gone\\u{1b}[2K\\u{7}"), "{message}");
    assert!(
        !message.chars().any(|c| CONTROLS.contains(&c)),
        "{message:?}"
    );
    let review = pair.review_ok(&pair.main, &id);
    let text = printed(&review).0;
    assert!(text.contains("gone\\u{1b}[2K\\u{7}\\u{9b}"), "{text}");
    assert!(!text.chars().any(|c| CONTROLS.contains(&c)), "{text:?}");
}

/// The orphan rule (review of iteration 1: a moved or deleted checkout
/// left its proposals in every command's way for ever). PR-0001 `open`,
/// PR-0002 `approved` (by SQL), PR-0003 applied, all raised in the first
/// repository; a second repository of the project (the same slug, so the
/// same queue). While the first exists: `reject` of PR-0002 there asks
/// `reject approved PR-0002 (<target> in <path> on <branch> in
/// <worktree>)? [y/N]` (iteration 3: an approved proposal without its
/// commit on the branch is rejectable) and, declined, exits 1 with nothing
/// changed; `reject` from the second exits 2 ("run the command there").
/// Once the first is deleted, from the second: `review` and `approve` exit
/// 2 naming `spec reject PR-0001 --reason …` (`review`'s text exact: "…
/// takes it out of the inbox unless its commit is in history", iteration
/// 6), no prompt; `reject` takes
/// PR-0001 and PR-0002 with a note that their repository no longer exists
/// (one `proposal.rejected` each); PR-0003 is refused naming its commit.
/// M: the orphan rule dropped.
#[test]
fn an_orphan_is_rejected_from_another_repository_of_the_project() {
    let pair = Pair::new("pa-orphan", "spec-a");
    let open = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let approved = pair.propose_edit(
        &pair.linked,
        TARGET,
        "is not a reference",
        "is never a reference",
    );
    let applied = pair.propose_edit(&pair.linked, TARGET, FROM, "the sprint stops;");
    pair.approve_ok(&pair.main, &applied);
    let commit = pair.rev(&pair.linked, "t1");
    pair.sql(&format!(
        "update proposals set status = 'approved', decided_by = '{DECIDER}', \
         decided_at = '{NOW}', updated_at = '{NOW}' where id = '{approved}'"
    ));

    let other = pair.scratch.copy("spec-a", "other");
    pair.git.init(&other);
    pair.commit_all(&other, "another clone of the project");

    // The first repository exists: an approved proposal is asked about
    // by name (declined here: nothing changes), and another repository's
    // proposal is not this one's.
    let events = pair.events().len();
    let before = pair.proposal(&approved);
    let (outcome, questions) = pair.reject_answer(&pair.main, &approved, "No.", false);
    let reason = refused(&outcome, "approved, its repository there, declined");
    assert!(
        reason.contains(&format!(
            "`{approved}` not rejected: the answer was not `y`"
        )),
        "{reason}"
    );
    assert!(reason.contains("nothing changed"), "{reason}");
    assert_eq!(
        questions,
        [format!(
            "reject approved {approved} ({TARGET} in {PATH} on t1 in {})? [y/N]",
            before.place.worktree
        )],
        "the prompt names an approved proposal"
    );
    assert_eq!(pair.proposal(&approved), before, "nothing written");
    assert_eq!(pair.events().len(), events, "no event");
    let (outcome, _) = pair.reject_answer(&other, &open, "No.", true);
    let message = cannot(&outcome, "another existing repository");
    assert!(message.contains("run the command there"), "{message}");
    let inbox = pair.inbox(&other, false).expect("inbox");
    assert!(inbox.proposals.is_empty());

    fs::remove_dir_all(&pair.linked).unwrap();
    fs::remove_dir_all(&pair.main).unwrap();

    let message = cannot(&pair.review(&other, &open), "review of an orphan");
    let place = pair.proposal(&open).place;
    assert_eq!(
        message,
        format!(
            "spec: `{open}` belongs to the repository {} (worktree {}), which no longer exists: \
             `spec reject {open} --reason …` takes it out of the inbox unless its commit is in \
             history",
            place.git_common_dir, place.worktree
        )
    );
    let (outcome, questions) = pair.approve_answer(&other, &open, true, pair.git_env(&other));
    let message = cannot(&outcome, "approve of an orphan");
    assert!(message.contains("no longer exists"), "{message}");
    assert!(questions.is_empty(), "no prompt");

    for id in [&open, &approved] {
        let (outcome, questions) = pair.reject_answer(&other, id, "Moved away.", true);
        let outcome = outcome.unwrap_or_else(|error| panic!("reject {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{id}: {outcome:?}");
        assert_eq!(questions.len(), 1, "{id}");
        assert!(
            outcome.messages.iter().any(|message| matches!(message,
                Message::Note(text) if text.contains("which no longer exists"))),
            "{id}: {:?}",
            outcome.messages
        );
        let stored = pair.proposal(id);
        assert_eq!(stored.status.as_str(), "rejected", "{id}");
        assert_eq!(stored.decision_note.as_deref(), Some("Moved away."), "{id}");
        assert_eq!(stored.decided_by.as_deref(), Some(DECIDER), "{id}");
        let rejected = pair
            .events_of(id)
            .iter()
            .filter(|(kind, _)| kind == "proposal.rejected")
            .count();
        assert_eq!(rejected, 1, "{id}: one rejected event");
    }
    let (outcome, questions) = pair.reject_answer(&other, &applied, "Moved away.", true);
    let reason = refused(&outcome, "an applied orphan");
    assert!(reason.contains(&commit), "names its commit: {reason}");
    assert!(questions.is_empty());
    assert_eq!(pair.proposal(&applied).status.as_str(), "applied");
    let inbox = pair.inbox(&other, false).expect("inbox");
    assert!(
        inbox.proposals.is_empty() && inbox.notes.is_empty(),
        "{inbox:?}"
    );
    assert_eq!(read(&other, PATH), read(&common::fixture("spec-a"), PATH));
}

/// The inbox's notes about other repositories of the project (review of
/// iteration 2, minor: an orphan could not be found from the inbox).
/// PR-0001 and PR-0002 raised in the first repository, PR-0003 in a second
/// one of the same project; the first moved away (its recorded common dir
/// gone). From the moved checkout: no proposal listed, one note counting
/// the existing repository's ("1 proposal(s) of another repository of the
/// project `<slug>` not listed") and one naming the gone repository's
/// ("2 proposal(s) of a repository that no longer exists not listed:
/// PR-0001, PR-0002; `spec reject <ID> --reason …` …"), alike in the
/// library, `spec inbox`'s stderr and `--json`. After PR-0001 is rejected
/// (an orphan), the note names PR-0002 only; `--all` names both (accepted
/// deviation: the gone note lists applied and rejected ones too). From the
/// second repository PR-0003 is listed and PR-0002 named gone. M: the
/// other-repository notes merged into one count.
#[test]
fn inbox_names_a_gone_repositorys_proposals_and_counts_the_existing_ones() {
    let pair = Pair::new("pa-inbox-gone", "spec-a");
    let first = pair.propose_edit(&pair.linked, TARGET, FROM, TO);
    let second = pair.propose_edit(
        &pair.linked,
        TARGET,
        "is not a reference",
        "is never a reference",
    );
    let other = pair.scratch.copy("spec-a", "other");
    pair.git.init(&other);
    pair.commit_all(&other, "another clone of the project");
    let third = pair.propose_edit(&other, TARGET, FROM, "the sprint ends now;");
    assert_eq!(
        [first.as_str(), second.as_str(), third.as_str()],
        ["PR-0001", "PR-0002", "PR-0003"]
    );
    let moved = pair.scratch.join("moved");
    fs::rename(&pair.main, &moved).unwrap();
    let moved = fs::canonicalize(moved).unwrap();

    let existing = format!(
        "1 proposal(s) of another repository of the project `{}` not listed: `spec inbox` \
         lists the current repository's",
        pair.slug
    );
    let gone = |ids: &str, count: usize| {
        format!(
            "{count} proposal(s) of a repository that no longer exists not listed: {ids}; \
             `spec reject <ID> --reason …` takes an open or approved one out of the inbox \
             unless its commit is in history"
        )
    };
    let inbox = pair.inbox(&moved, false).expect("inbox");
    assert!(inbox.proposals.is_empty(), "{inbox:?}");
    let want = [existing.clone(), gone("PR-0001, PR-0002", 2)];
    assert_eq!(inbox.notes, want);
    let json: Vec<String> = json_of(&printed_inbox(&inbox).1)["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .map(|note| note.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(json, want, "JSON");
    let run = pair.spec_piped(&moved, &["inbox"], b"");
    run.code(0);
    for note in &want {
        assert!(
            run.stderr.contains(&format!("note: {note}")),
            "{}",
            run.show()
        );
    }
    let run = pair.spec_piped(&moved, &["inbox", "--json"], b"");
    run.code(0);
    assert_eq!(
        run.json()["notes"],
        serde_json::json!(want),
        "{}",
        run.show()
    );

    let (outcome, _) = pair.reject_answer(&moved, &first, "Moved away.", true);
    let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let inbox = pair.inbox(&moved, false).expect("inbox");
    assert_eq!(inbox.notes, [existing.clone(), gone("PR-0002", 1)]);
    let all = pair.inbox(&moved, true).expect("inbox --all");
    assert_eq!(all.notes, [existing, gone("PR-0001, PR-0002", 2)]);

    let inbox = pair.inbox(&other, false).expect("inbox of the second");
    let listed: Vec<&str> = inbox
        .proposals
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(listed, ["PR-0003"]);
    assert_eq!(inbox.notes, [gone("PR-0002", 1)]);
}

/// The bidirectional controls, as rendered (iteration 3).
const BIDI: [char; 12] = [
    '\u{61c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}',
    '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
];

fn assert_no_raw_bidi(text: &str, context: &str) {
    assert!(
        !text.chars().any(|c| BIDI.contains(&c)),
        "{context}: a raw bidirectional control: {text:?}"
    );
}

/// Every `\r` of `text` is the CR of a CRLF pair.
fn only_crlf(text: &str) -> bool {
    text.match_indices('\r')
        .all(|(at, _)| text[at + 1..].starts_with('\n'))
}

/// Iteration 3's escaping (review of iteration 2, nit): a CRLF file's
/// diff lines keep their CR (no `\u{d}` on every line); a lone CR of the
/// rationale is `\u{d}`, its CRLF kept; the bidirectional marks,
/// embeddings, overrides and isolates (U+061C, U+200E, U+200F,
/// U+202A–U+202E, U+2066–U+2069) of the rationale and of a worktree path
/// are written `\u{xx}` in `review`, `inbox` and both consent questions;
/// the JSON carries the text as stored. M: the CRLF/bidi escaping reverted.
#[test]
fn crlf_is_kept_and_lone_cr_and_bidi_controls_are_escaped() {
    let pair = Pair::new("pa-escape-bidi", "spec-a");
    let text = common::read_text(&pair.linked, PATH).replace('\n', "\r\n");
    common::write(&pair.linked, PATH, &text);
    pair.commit_all(&pair.linked, "crlf");
    let weird = pair.scratch.join("w\u{202e}rtl\u{2066}x");
    pair.git.git(
        &pair.linked,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "t9",
            weird.to_str().unwrap(),
            "t1",
        ],
    );
    let weird = fs::canonicalize(weird).unwrap();
    let (hash, span) = pair.span(&weird, TARGET);
    let mut request = pair.request(&weird, TARGET, &hash, &edit(&span, FROM, TO));
    let rationale = "evil \u{202e}txet\u{202c} iso \u{2066}x\u{2069} marks \u{61c}\u{200e}\u{200f}\
                     \u{202a}\u{202b}\u{202d}\u{2067}\u{2068} cr\rX\r\nSecond line.";
    request.rationale = rationale.to_owned();
    let outcome = pair.propose_with(&weird, request).expect("propose");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let id = outcome.document.id.clone().expect("an ID");

    let review = pair.review_ok(&weird, &id);
    let (text, json) = printed(&review);
    assert_no_raw_bidi(&text, "review text");
    for escaped in [
        "\\u{202e}txet\\u{202c}",
        "\\u{2066}x\\u{2069}",
        "\\u{61c}\\u{200e}\\u{200f}\\u{202a}\\u{202b}\\u{202d}\\u{2067}\\u{2068}",
        // A field's lines are indented by two spaces.
        "cr\\u{d}X\r\n  Second line.",
    ] {
        assert!(text.contains(escaped), "review: no {escaped}: {text:?}");
    }
    assert!(only_crlf(&text), "a lone raw CR: {text:?}");
    assert_eq!(
        text.matches("\\u{d}").count(),
        text.matches("cr\\u{d}X").count(),
        "only the lone CR escaped: {text:?}"
    );
    let diff_line = text
        .split('\n')
        .find(|line| line.trim_start().starts_with("+At zero stamina"))
        .unwrap_or_else(|| panic!("a diff line: {text:?}"));
    assert!(
        diff_line.ends_with('\r') && !diff_line.contains("\\u{"),
        "the CRLF line as it is: {diff_line:?}"
    );
    assert_eq!(json_of(&json)["rationale"], rationale, "JSON as stored");

    let inbox = pair.inbox(&weird, false).expect("inbox");
    let line = printed_inbox(&inbox).0;
    assert_no_raw_bidi(&line, "inbox text");
    assert!(line.contains("\\u{202e}txet\\u{202c}"), "{line:?}");
    // One line per proposal: its line breaks (the lone CR too) are spaces.
    assert!(!line.contains('\r') && line.contains("cr X"), "{line:?}");
    assert!(
        !line.contains("Second line"),
        "the first line only: {line:?}"
    );

    let (outcome, questions) =
        pair.approve_answer(&pair.main, &id, false, pair.git_env(&pair.main));
    refused(&outcome, "declined approve");
    let (outcome, rejects) = pair.reject_answer(&pair.main, &id, "No.", false);
    refused(&outcome, "declined reject");
    for question in questions.iter().chain(&rejects) {
        assert_no_raw_bidi(question, "question");
        assert!(question.contains("w\\u{202e}rtl\\u{2066}x"), "{question:?}");
    }
    assert_eq!((questions.len(), rejects.len()), (1, 1));
}

/// Iteration 4 (review of iteration 3, nit: the gone note listed every
/// orphan ID). Twelve proposals raised in the first repository, which is
/// then moved away; from a second repository of the project the note names
/// ten and counts two: "12 proposal(s) of a repository that no longer
/// exists not listed: PR-0001, …, PR-0010 … and 2 more; `spec reject <ID>
/// --reason …` …", alike in the library, `spec inbox`'s stderr and
/// `--json`. One orphan rejected: eleven, "PR-0002, …, PR-0011 … and 1
/// more"; two: ten, all named, no count; `--all` still twelve. M: the
/// inbox cap removed.
#[test]
fn the_gone_note_names_ten_ids_and_counts_the_rest() {
    let pair = Pair::new("pa-inbox-cap", "spec-a");
    let ids: Vec<String> = (1..=12)
        .map(|n| pair.propose_edit(&pair.linked, TARGET, FROM, &format!("the sprint ends {n};")))
        .collect();
    let want_ids: Vec<String> = (1..=12).map(|n| format!("PR-{n:04}")).collect();
    assert_eq!(ids, want_ids);
    let other = pair.scratch.copy("spec-a", "other");
    pair.git.init(&other);
    pair.commit_all(&other, "another clone of the project");
    let moved = pair.scratch.join("moved");
    fs::rename(&pair.main, &moved).unwrap();

    let gone = |count: usize, named: &[String], more: Option<usize>| {
        let mut list = named.join(", ");
        if let Some(more) = more {
            list.push_str(&format!(" \u{2026} and {more} more"));
        }
        format!(
            "{count} proposal(s) of a repository that no longer exists not listed: {list}; \
             `spec reject <ID> --reason \u{2026}` takes an open or approved one out of the inbox \
             unless its commit is in history"
        )
    };
    let twelve = gone(12, &ids[..10], Some(2));
    let inbox = pair.inbox(&other, false).expect("inbox");
    assert!(inbox.proposals.is_empty(), "{inbox:?}");
    assert_eq!(inbox.notes, std::slice::from_ref(&twelve));
    let json: Vec<String> = json_of(&printed_inbox(&inbox).1)["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .map(|note| note.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(json, std::slice::from_ref(&twelve), "JSON");
    let run = pair.spec_piped(&other, &["inbox"], b"");
    run.code(0);
    assert!(
        run.stderr.contains(&format!("note: {twelve}")),
        "{}",
        run.show()
    );
    let run = pair.spec_piped(&other, &["inbox", "--json"], b"");
    run.code(0);
    assert_eq!(
        run.json()["notes"],
        serde_json::json!([twelve]),
        "{}",
        run.show()
    );

    for (id, want) in [
        (&ids[0], gone(11, &ids[1..11], Some(1))),
        (&ids[1], gone(10, &ids[2..12], None)),
    ] {
        let (outcome, _) = pair.reject_answer(&other, id, "Moved away.", true);
        let outcome = outcome.unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        let inbox = pair.inbox(&other, false).expect("inbox");
        assert_eq!(inbox.notes, [want]);
    }
    let all = pair.inbox(&other, true).expect("inbox --all");
    assert_eq!(all.notes, [twelve]);
}
