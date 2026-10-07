//! docs/features/proposal-kinds.md through the CLI library: the kind
//! `create`, a new spec file (file form) or new `{#ID}` sections in a
//! node's span (section form), proposed with nothing written and applied by
//! `spec approve` as one commit where it was raised. AC-01 to AC-09 and
//! AC-11 to AC-14 (AC-10: `specengine-mcp/tests/mcp_create.rs`; AC-05's
//! store half: `specengine-store/tests/queue_kinds.rs`; AC-11's scan half:
//! `proposal_genre.rs`; AC-15: `plugin_skills.rs`, `plugin_files.rs`;
//! AC-17: `mcp_decision.rs`, `mcp_path.rs`), AC-16's source half, and the
//! review's iteration-2 rules: a section create already in place is step
//! 5's refusal before step 4's ID check (n1), a create lost in the race
//! names the holder and its state (n2), the project's index and its shards
//! refused at propose and at apply step 3 (n4).
//!
//! Setup (the AC's): scratch git repositories of `fixtures/spec-a` and
//! `fixtures/spec-b`, committed, a linked worktree on `t1`; a scratch
//! `HOME`; the clock `2026-10-06T12:00:00Z`; the sandbox's git identity;
//! library calls, consent yes unless said; proposals raised in the linked
//! worktree, approved from the main one. T13 = "Data"'s `r13.md`: spec-a's
//! `R-12.md` with `id: R-13` and its own H1. "Refused": the exit named,
//! nothing stored, written or reserved.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;

use common::check::{quoted, set_paths_key};
use common::proposal::{
    DECIDER, Pair, State, cannot, edit, json_of, printed, printed_inbox, refused,
};
use common::{RUN_TIMEOUT, read, read_text, write};
use serde_json::{Value, json};
use specengine_cli::{
    ApproveFlags, CliError, CreateRequest, Exit, Globals, ProposalOutcome, ProposedText,
    RejectRequest, propose_create, reject,
};
use specengine_core::proposal::Author;
use specengine_store::{
    Decision, EVENT_APPLIED, EVENT_APPLY_FAILED, EVENT_APPROVED, EVENT_CREATED, NewProposal, Place,
    ProposalKind, ProposalQueue as _, ProposalStatus, patch_hash,
};

/// The clock of every propose (AC Setup).
const CLOCK: &str = "2026-10-06T12:00:00Z";
/// The clock of every approve and reject.
const DECIDED: &str = "2026-10-06T12:30:00Z";
/// "Data"'s new file.
const R13: &str = "docs/records/R/R-13.md";
/// "Data"'s rationale.
const RATIONALE: &str = "sprint needs it";
const STAMINA: &str = "docs/spec/movement/stamina.md";
/// AC-06's new section.
const REST: &str =
    "### Rest delay {#EDGE-STAM-REST}\n- Regeneration waits 1.5 s after the last sprint.";

// ---------------------------------------------------------------- helpers

/// T13: spec-a's `R-12.md` with `id: R-13` and its own H1.
fn t13() -> String {
    let r12 = read_text(&common::fixture("spec-a"), "docs/records/R/R-12.md");
    let text = edit(&r12, "id: R-12\n", "id: R-13\n");
    edit(
        &text,
        "# Stamina regenerates only at rest\n",
        "# Sprint keeps a stamina reserve\n",
    )
}

/// A spec-a record whose `id:` (line 2) is written `id`.
fn record(id: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\nstatus: accepted\nowner: owner\nreviewed: 2026-09-20\n---\n\n\
         # A new record\n\nA body.\n"
    )
}

/// A spec-b record whose `id:` (line 2) is written `id`, Russian prose.
fn record_b(id: &str, kind: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\nkind: {kind}\nowner: owner\nreviewed: 2026-09-20\n---\n\n\
         # \u{0421}\u{043b}\u{0438}\u{044f}\u{043d}\u{0438}\u{0435}\n\n\
         \u{0412}\u{0435}\u{0442}\u{043a}\u{0430} \u{0441}\u{043b}\u{0438}\u{0432}\u{0430}\u{0435}\u{0442}\u{0441}\u{044f}.\n"
    )
}

/// `spec propose create TARGET [--base B] --text-file - --rationale R
/// --author-role writer` at [`CLOCK`], from `cwd`.
fn request(pair: &Pair, cwd: &Path, target: &str, base: Option<&str>, text: &str) -> CreateRequest {
    CreateRequest {
        target: target.to_owned(),
        base: base.map(str::to_owned),
        text: ProposedText::Given(text.as_bytes().to_vec()),
        rationale: RATIONALE.to_owned(),
        author_role: Some("writer".to_owned()),
        author_model: None,
        run: None,
        now: CLOCK.to_owned(),
        git: pair.git_env(cwd),
    }
}

fn create(
    pair: &Pair,
    cwd: &Path,
    target: &str,
    base: Option<&str>,
    text: &str,
) -> Result<ProposalOutcome, CliError> {
    propose_create(
        &pair.env(cwd),
        &Globals::default(),
        &request(pair, cwd, target, base, text),
    )
}

/// A create that must be stored: its ID.
fn created(pair: &Pair, cwd: &Path, target: &str, base: Option<&str>, text: &str) -> String {
    let outcome = create(pair, cwd, target, base, text)
        .unwrap_or_else(|error| panic!("propose create {target}: {error}"));
    assert_eq!(
        outcome.exit(),
        Exit::Answered,
        "propose create {target}: {outcome:?}"
    );
    outcome.document.id.clone().expect("an ID")
}

/// Library approve (no decision flag) at [`DECIDED`], consent `answer`.
fn approve(
    pair: &Pair,
    cwd: &Path,
    id: &str,
    answer: bool,
) -> (Result<ProposalOutcome, CliError>, Vec<String>) {
    pair.decide_at(cwd, id, &ApproveFlags::default(), None, answer, DECIDED)
}

/// An approve that must apply: the outcome and its one question.
fn approve_ok(pair: &Pair, cwd: &Path, id: &str) -> (ProposalOutcome, String) {
    let (outcome, questions) = approve(pair, cwd, id, true);
    let outcome = outcome.unwrap_or_else(|error| panic!("approve {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "approve {id}: {outcome:?}");
    assert_eq!(questions.len(), 1, "approve {id}: {questions:?}");
    (outcome, questions.into_iter().next().expect("one question"))
}

/// Library reject at [`DECIDED`], consent yes.
fn reject_at(pair: &Pair, cwd: &Path, id: &str, reason: &str) -> Result<ProposalOutcome, CliError> {
    let mut consent = |_: &str| true;
    reject(
        &pair.env(cwd),
        &Globals::default(),
        &RejectRequest {
            id: id.to_owned(),
            reason: reason.to_owned(),
            now: DECIDED.to_owned(),
            git: pair.git_env(cwd),
        },
        &mut consent,
    )
}

/// The owner's decision, as the store records `approved` at step 7.
fn owner() -> Decision {
    Decision {
        decided_by: DECIDER.to_owned(),
        note: None,
    }
}

/// `git diff-tree --name-status` of `commit`: its lines.
fn name_status(pair: &Pair, commit: &str) -> Vec<String> {
    pair.git_text(
        &pair.main,
        &["diff-tree", "-r", "--no-commit-id", "--name-status", commit],
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

/// The blob of `path` at `commit`, its bytes.
fn blob(pair: &Pair, commit: &str, path: &str) -> Vec<u8> {
    pair.git.git(
        &pair.main,
        &["cat-file", "blob", &format!("{commit}:{path}")],
    )
}

/// `(id, by, status)` of every live create's new ID; none without a queue.
fn reservations(pair: &Pair) -> Vec<(String, String, String)> {
    if !pair.db().exists() {
        return Vec::new();
    }
    pair.queue()
        .reserved()
        .expect("reserved")
        .into_iter()
        .map(|held| (held.id, held.by, held.status.as_str().to_owned()))
        .collect()
}

/// The review document of `id` as `spec review --json` prints it.
fn review_json(pair: &Pair, id: &str) -> Value {
    json_of(&printed(&pair.review_ok(&pair.main, id)).1)
}

/// A refusal of a propose: exit 1, its reason; nothing stored, written or
/// reserved (the state and the reservations as `before`).
fn refused_create(
    pair: &Pair,
    cwd: &Path,
    target: &str,
    base: Option<&str>,
    text: &str,
    context: &str,
) -> String {
    let before = (pair.state(), reservations(pair));
    let outcome = create(pair, cwd, target, base, text);
    let reason = refused(&outcome, context);
    assert!(
        outcome
            .as_ref()
            .is_ok_and(|outcome| outcome.document.id.is_none()),
        "{context}: {outcome:?}"
    );
    assert_eq!(
        (pair.state(), reservations(pair)),
        before,
        "{context}: nothing stored, written or reserved"
    );
    reason
}

/// Like [`refused_create`] for an exit 2: its message.
fn cannot_create(
    pair: &Pair,
    cwd: &Path,
    target: &str,
    base: Option<&str>,
    text: &str,
    context: &str,
) -> String {
    let before = (pair.state(), reservations(pair));
    let message = cannot(&create(pair, cwd, target, base, text), context);
    assert_eq!(
        (pair.state(), reservations(pair)),
        before,
        "{context}: nothing stored, written or reserved"
    );
    message
}

/// Everything of a [`State`] but the queue's rows.
fn files_and_git(state: &State) -> State {
    State {
        proposals: Vec::new(),
        ..state.clone()
    }
}

/// `(span_hash, the span with AC-06's section appended)` of
/// `RULE-STAM-REGEN` in `cwd`.
fn regen_with_rest(pair: &Pair, cwd: &Path) -> (String, String) {
    let (hash, span) = pair.span(cwd, "RULE-STAM-REGEN");
    (hash, format!("{span}\n\n{REST}"))
}

/// `(type, step)` pairs, typed.
fn events(pairs: &[(&str, Option<u64>)]) -> Vec<(String, Option<u64>)> {
    pairs
        .iter()
        .map(|(kind, step)| ((*kind).to_owned(), *step))
        .collect()
}

// ------------------------------------------------------------------ AC-01

/// AC-01: spec-a `propose create docs/records/R/R-13.md` T13 stores
/// `PR-0001` (`PR-0001`, `introduced: 0`); its review: `kind` `create`,
/// `target_id` `R-13`, `target_path` the path, `target_ids` `["R-13"]`,
/// `base_hash` and `base_text` `null`, `new_text` T13 byte for byte; the
/// row's `patch_hash` `b3(target_id LF LF new_text)`, `R-13` reserved by
/// it; both worktrees, every ref and git's index as they were, `git status
/// --porcelain` empty. M: writing at propose.
#[test]
fn ac01_a_new_file_is_proposed_and_nothing_is_written() {
    let pair = Pair::new("pk-ac01", "spec-a");
    let t13 = t13();
    let before = pair.state();
    let outcome = create(&pair, &pair.linked, R13, None, &t13).expect("propose create");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let (text, json) = printed(&outcome);
    assert_eq!(text, "PR-0001\nintroduced: 0\n");
    assert_eq!(json_of(&json)["id"], json!("PR-0001"));
    assert_eq!(
        files_and_git(&pair.state()),
        files_and_git(&before),
        "nothing written"
    );
    assert_eq!(pair.porcelain(&pair.linked), "");
    assert_eq!(pair.porcelain(&pair.main), "");

    let review = review_json(&pair, "PR-0001");
    assert_eq!(review["kind"], json!("create"));
    assert_eq!(review["status"], json!("open"));
    assert_eq!(review["target_id"], json!("R-13"));
    assert_eq!(review["target_path"], json!(R13));
    assert_eq!(review["target_ids"], json!(["R-13"]));
    assert_eq!(review["base_hash"], Value::Null);
    assert_eq!(review["base_text"], Value::Null);
    assert_eq!(review["new_text"], json!(t13));
    assert_eq!(review["preview"], json!("applies"));
    assert_eq!(review["branch"], json!("t1"));
    assert_eq!(
        review["patch_hash"],
        json!(patch_hash("R-13", "", &t13)),
        "b3(target_id LF LF new_text)"
    );
    let stored = pair.proposal("PR-0001");
    assert_eq!(stored.kind, ProposalKind::Create);
    assert!(stored.new_file());
    assert_eq!(stored.new_ids, ["R-13"]);
    assert_eq!(stored.target_ids(), ["R-13"]);
    assert_eq!(stored.new_text, t13);
    assert_eq!(
        pair.sql(
            "SELECT kind, target_ids, base_hash IS NULL, base_text IS NULL, \
             severity IS NULL AND summary IS NULL AND linked IS NULL AND record_id IS NULL \
             FROM proposals;"
        ),
        "create|[\"R-13\"]|1|1|1\n"
    );
    assert_eq!(
        reservations(&pair),
        [("R-13".to_owned(), "PR-0001".to_owned(), "open".to_owned())]
    );
    assert_eq!(pair.events_of("PR-0001"), events(&[(EVENT_CREATED, None)]));
}

// ------------------------------------------------------------------ AC-02

/// AC-02: a file modified and another staged in the linked worktree;
/// `approve` (asking `apply PR-0001 to <path> on t1 in <worktree> (new
/// file)? [y/N]`): one commit on `t1`, one parent, exactly `A
/// docs/records/R/R-13.md`, its blob T13, the message `spec: apply
/// PR-0001`, the rationale, four trailers; both files as before (the
/// modified one unstaged, the staged one staged, their bytes); `spec show
/// R-13` resolves; `applied`, three events, `R-13` no longer reserved. M:
/// staging the whole tree.
#[test]
fn ac02_approve_commits_exactly_the_new_file() {
    let pair = Pair::new("pk-ac02", "spec-a");
    let t13 = t13();
    let id = created(&pair, &pair.linked, R13, None, &t13);
    let game = format!(
        "{}\nA local edit.\n",
        read_text(&pair.linked, "docs/spec/game.md")
    );
    write(&pair.linked, "docs/spec/game.md", &game);
    let sprint = format!(
        "{}\nA staged edit.\n",
        read_text(&pair.linked, "docs/spec/movement/sprint.md")
    );
    write(&pair.linked, "docs/spec/movement/sprint.md", &sprint);
    pair.git
        .git(&pair.linked, &["add", "--", "docs/spec/movement/sprint.md"]);
    let status = pair.porcelain(&pair.linked);
    assert_eq!(
        status,
        " M docs/spec/game.md\nM  docs/spec/movement/sprint.md\n"
    );
    let staged = pair.git_text(&pair.linked, &["diff", "--cached", "--name-status"]);
    let base = pair.rev(&pair.linked, "HEAD");
    let base_commit = pair.proposal(&id).place.base_commit;

    let (outcome, question) = approve_ok(&pair, &pair.main, &id);
    assert_eq!(
        question,
        format!(
            "apply PR-0001 to {R13} on t1 in {} (new file)? [y/N]",
            pair.linked.display()
        )
    );
    let head = pair.rev(&pair.linked, "t1");
    assert_ne!(head, base);
    assert_eq!(
        pair.git_text(&pair.main, &["rev-list", "--parents", "-n", "1", &head]),
        format!("{head} {base}"),
        "one commit, one parent"
    );
    assert_eq!(name_status(&pair, &head), [format!("A\t{R13}")]);
    assert_eq!(blob(&pair, &head, R13), t13.as_bytes(), "the blob is T13");
    assert_eq!(
        pair.git_text(&pair.main, &["log", "-1", "--format=%B", &head]),
        format!(
            "spec: apply PR-0001\n\n{RATIONALE}\n\nProposal: PR-0001\nDecided-by: {DECIDER}\n\
             Proposed-by: agent role=writer model=unknown run=unknown\nBase-commit: {base_commit}"
        )
    );
    // The other files: as before, never staged or committed.
    assert_eq!(pair.porcelain(&pair.linked), status);
    assert_eq!(
        pair.git_text(&pair.linked, &["diff", "--cached", "--name-status"]),
        staged
    );
    assert_eq!(read_text(&pair.linked, "docs/spec/game.md"), game);
    assert_eq!(
        read_text(&pair.linked, "docs/spec/movement/sprint.md"),
        sprint
    );
    assert_eq!(read(&pair.linked, R13), t13.as_bytes());
    let shown = pair.node(&pair.linked, "R-13");
    assert_eq!(
        (shown.id.as_deref(), shown.path.as_str()),
        (Some("R-13"), R13)
    );

    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Applied);
    assert_eq!(stored.applied_commit.as_deref(), Some(head.as_str()));
    assert_eq!(stored.decided_by.as_deref(), Some(DECIDER));
    assert_eq!(
        pair.events_of(&id),
        events(&[
            (EVENT_CREATED, None),
            (EVENT_APPROVED, None),
            (EVENT_APPLIED, None)
        ])
    );
    assert!(
        reservations(&pair).is_empty(),
        "an applied create holds nothing"
    );
    let (text, _) = printed(&outcome);
    assert!(
        text.starts_with(&format!("applied PR-0001 as {head} on t1")),
        "{text}"
    );
}

// ------------------------------------------------------------------ AC-03

/// AC-03, propose: an indexed file without `--base`, a dangling symlink,
/// an index-only path: exit 1 `exists`, nothing stored or reserved. M:
/// rename over the path.
#[test]
fn ac03_a_path_holding_anything_is_refused_at_propose() {
    let pair = Pair::new("pk-ac03p", "spec-a");
    let t13 = t13();
    let exists = |path: &str| {
        format!(
            "`{path}` exists: a create never replaces a file; to add ID sections to it, name its \
             span_hash with --base"
        )
    };
    let path = "docs/records/R/R-12.md";
    assert_eq!(
        refused_create(&pair, &pair.linked, path, None, &t13, "R-12.md"),
        exists(path)
    );
    let path = "docs/records/R/R-14.md";
    symlink("missing.md", pair.linked.join(path)).expect("a dangling symlink");
    assert_eq!(
        refused_create(
            &pair,
            &pair.linked,
            path,
            None,
            &record("R-14"),
            "a dangling symlink"
        ),
        exists(path)
    );
    assert!(
        fs::symlink_metadata(pair.linked.join(path))
            .expect("still there")
            .file_type()
            .is_symlink()
    );
    let path = "docs/records/R/R-15.md";
    write(&pair.linked, path, "staged\n");
    pair.git.git(&pair.linked, &["add", "--", path]);
    fs::remove_file(pair.linked.join(path)).expect("removed from the worktree");
    assert_eq!(
        pair.git_text(&pair.linked, &["ls-files", "--", path]),
        path,
        "in git's index only"
    );
    assert_eq!(
        refused_create(
            &pair,
            &pair.linked,
            path,
            None,
            &record("R-15"),
            "index-only"
        ),
        exists(path)
    );
    assert!(pair.proposals().is_empty());
}

/// AC-03, apply: after AC-01, X at the path (untracked): approve exit 1
/// before any question, `apply_failed` step 4, X intact, no commit, still
/// `open`; the new ID defined meanwhile in another committed file: step 4
/// names it; X written by the consent callback (after steps 2–6): exit 1
/// at step 8, X intact, no commit. M: step 4 not re-checked; rename over
/// the path.
#[test]
fn ac03_a_file_at_the_path_stops_the_apply_at_step_4_or_8() {
    let pair = Pair::new("pk-ac03a", "spec-a");
    let t13 = t13();
    let id = created(&pair, &pair.linked, R13, None, &t13);
    let head = pair.rev(&pair.linked, "t1");
    write(&pair.linked, R13, "X\n");
    let (outcome, questions) = approve(&pair, &pair.main, &id, true);
    let reason = refused(&outcome, "X at the path");
    assert!(
        reason.starts_with(&format!(
            "`{id}` not applied (step 4): `{R13}` exists: a create never replaces a file"
        )) || reason.contains(&format!("`{R13}` exists: a create never replaces a file")),
        "{reason}"
    );
    assert!(questions.is_empty(), "{questions:?}");
    assert_eq!(read(&pair.linked, R13), b"X\n", "X intact");
    assert_eq!(pair.rev(&pair.linked, "t1"), head, "no commit");
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Open);
    assert_eq!(
        pair.events_of(&id),
        events(&[(EVENT_CREATED, None), (EVENT_APPLY_FAILED, Some(4))])
    );
    fs::remove_file(pair.linked.join(R13)).expect("X removed");

    // The new ID defined meanwhile by another file, committed.
    write(&pair.linked, "docs/records/R/R-13-x.md", record("R-13"));
    pair.commit_all(&pair.linked, "R-13 by hand");
    let moved = pair.rev(&pair.linked, "t1");
    let (outcome, questions) = approve(&pair, &pair.main, &id, true);
    let reason = refused(&outcome, "the ID taken meanwhile");
    assert!(
        reason.contains(
            "`R-13` is defined in `docs/records/R/R-13-x.md` (`id: R-13`); nothing changed"
        ),
        "{reason}"
    );
    assert!(questions.is_empty());
    assert_eq!(pair.rev(&pair.linked, "t1"), moved, "no commit");
    assert_eq!(
        pair.events_of(&id).last(),
        Some(&(EVENT_APPLY_FAILED.to_owned(), Some(4)))
    );
    pair.git.git(
        &pair.linked,
        &["rm", "-q", "--", "docs/records/R/R-13-x.md"],
    );
    pair.git.commit(&pair.linked, "R-13 by hand removed");
    let tip = pair.rev(&pair.linked, "t1");

    // X written by the consent callback: steps 2–6 passed, the file appears.
    let at = pair.linked.join(R13);
    let mut asked = Vec::new();
    let mut consent = |question: &str| {
        asked.push(question.to_owned());
        fs::write(&at, "Y\n").expect("X by the callback");
        true
    };
    let outcome = pair.decide_with(
        &pair.main,
        &id,
        &ApproveFlags::default(),
        None,
        DECIDED,
        &mut consent,
    );
    let reason = refused(&outcome, "X by the callback");
    assert!(reason.contains(&format!("`{R13}` exists")), "{reason}");
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(read(&pair.linked, R13), b"Y\n", "X intact");
    assert_eq!(pair.rev(&pair.linked, "t1"), tip, "no commit");
    assert_eq!(
        pair.events_of(&id).last(),
        Some(&(EVENT_APPLY_FAILED.to_owned(), Some(8)))
    );
    assert_ne!(pair.proposal(&id).status, ProposalStatus::Applied);
}

// ------------------------------------------------------------------ AC-04

/// AC-04: new files, each refused (exit 1) naming the holder or the form:
/// `id: R-12` its file and the next free `R-13`; `TERM-tired` the
/// document holding it as an alias, `TERM-exhausted`; `QST-033` `Q-033`;
/// `R-7` and `R-007` `R-07`; a `{#RULE-STAM-REGEN}` section its file; a
/// `{#QST-034}` section `Q-034`; an ID twice; the `[decision_records]`
/// prefix aside (AC-09). A look-alike `id: A-103` (U+0410 for `A`): exit
/// 2 naming `A-103`; the same as a `{#…}` section. U+0420 for `R` reads as
/// `P-13`, no prefix of the project: stored, `id-not-in-scheme` introduced,
/// named by its path, nothing reserved. M: `id:` checked, `{#…}` not.
#[test]
fn ac04_new_ids_are_checked_in_text_order_against_the_index() {
    let pair = Pair::new("pk-ac04", "spec-a");
    let cases: [(&str, String, &str); 8] = [
        (
            "docs/records/R/R-50.md",
            record("R-12"),
            "`R-12` is defined in `docs/records/R/R-12.md` (`id: R-12`); the next free is \
             `R-13`; nothing stored",
        ),
        (
            "docs/records/TERM/TERM-tired.md",
            record("TERM-tired"),
            "`TERM-tired` is an alias in `docs/records/TERM/TERM-exhausted.md` \
             (`id: TERM-exhausted`); nothing stored",
        ),
        (
            "docs/records/Q/Q-033.md",
            record("QST-033"),
            "`QST-033` (line 2) is written with a legacy `aliases_from` prefix: a new ID is \
             written with its canonical prefix, `Q-033`; nothing stored",
        ),
        (
            "docs/records/R/R-7.md",
            record("R-7"),
            "`R-7` (line 2): its prefix's numbers are written as `R-07`; nothing stored",
        ),
        (
            "docs/records/R/R-007.md",
            record("R-007"),
            "`R-007` (line 2): its prefix's numbers are written as `R-07`; nothing stored",
        ),
        (
            "docs/spec/movement/rest.md",
            "# Rest\n\n## Regeneration again {#RULE-STAM-REGEN}\n\nA body.\n".to_owned(),
            "`RULE-STAM-REGEN` is defined in `docs/spec/movement/stamina.md` \
             (`id: MEC-STAMINA`); nothing stored",
        ),
        (
            "docs/spec/movement/rest.md",
            "# Rest\n\n## Asked {#QST-034}\n\nA body.\n".to_owned(),
            "`QST-034` (line 3) is written with a legacy `aliases_from` prefix: a new ID is \
             written with its canonical prefix, `Q-034`; nothing stored",
        ),
        (
            "docs/spec/movement/rest.md",
            "# Rest\n\n## One {#RULE-REST-ONE}\n\nA body.\n\n## Two {#RULE-REST-ONE}\n\nMore.\n"
                .to_owned(),
            "`RULE-REST-ONE` is defined twice in the text (lines 3 and 7); nothing stored",
        ),
    ];
    for (path, text, want) in &cases {
        let reason = refused_create(&pair, &pair.linked, path, None, text, path);
        assert_eq!(reason, *want, "{path}: {text}");
    }
    // A look-alike: exit 2 naming the Latin form, as `id:` and as `{#…}`.
    let look_alike = "\u{0410}-103";
    let message = cannot_create(
        &pair,
        &pair.linked,
        "docs/records/A/A-103.md",
        None,
        &record(look_alike),
        "U+0410 in id:",
    );
    assert_eq!(
        message,
        format!(
            "spec: line 2: `{look_alike}` mixes scripts or uses look-alike letters; IDs are \
             Latin only: write `A-103`; nothing stored"
        )
    );
    let message = cannot_create(
        &pair,
        &pair.linked,
        "docs/spec/movement/rest.md",
        None,
        &format!("# Rest\n\n## Assumed {{#{look_alike}}}\n\nA body.\n"),
        "U+0410 in {#}",
    );
    assert!(
        message.starts_with("spec: line 3: ") && message.contains("write `A-103`"),
        "{message}"
    );
    assert!(pair.proposals().is_empty());

    // U+0420 (Er) for `R`: a `P` to the grammar, no prefix: a finding.
    let er = "\u{0420}-13";
    let id = created(
        &pair,
        &pair.linked,
        "docs/records/R/P-13.md",
        None,
        &record(er),
    );
    let stored = pair.proposal(&id);
    assert_eq!(
        stored.target_id, "docs/records/R/P-13.md",
        "named by its path"
    );
    assert!(stored.new_ids.is_empty(), "{:?}", stored.new_ids);
    assert!(
        stored
            .diagnostics
            .iter()
            .any(|finding| finding.code == "id-not-in-scheme" && finding.subject == er),
        "{:?}",
        stored.diagnostics
    );
    assert!(reservations(&pair).is_empty());
}

// ------------------------------------------------------------------ AC-05

/// AC-05, the CLI half: `PR-0001` (T13) open, then `approved`: `id: R-13`
/// at `R-13-b.md` exit 1 naming `PR-0001`, its state and `R-14`; after
/// `reject PR-0001` it stores (`PR-0002`), holding `R-13`. M: the
/// reservation not read.
#[test]
fn ac05_a_live_create_reserves_its_new_ids() {
    let pair = Pair::new("pk-ac05", "spec-a");
    let id = created(&pair, &pair.linked, R13, None, &t13());
    let other = "docs/records/R/R-13-b.md";
    for status in ["open", "approved"] {
        if status == "approved" {
            pair.queue()
                .approve(&id, &owner(), DECIDED)
                .expect("approved as step 7 leaves it");
        }
        let reason = refused_create(&pair, &pair.main, other, None, &record("R-13"), status);
        assert_eq!(
            reason,
            format!(
                "`R-13` is reserved by `PR-0001` ({status}), a live create; the next free is \
                 `R-14`; nothing stored"
            )
        );
    }
    let outcome = reject_at(&pair, &pair.main, &id, "Not now.").expect("reject");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(
        reservations(&pair).is_empty(),
        "a rejected create frees its IDs"
    );
    let second = created(&pair, &pair.main, other, None, &record("R-13"));
    assert_eq!(second, "PR-0002");
    assert_eq!(
        reservations(&pair),
        [("R-13".to_owned(), "PR-0002".to_owned(), "open".to_owned())]
    );
}

// ------------------------------------------------------------------ AC-06

/// AC-06: the section form on `RULE-STAM-REGEN` (its span + `### Rest
/// delay {#EDGE-STAM-REST}`) stores `target_ids` `["RULE-STAM-REGEN",
/// "EDGE-STAM-REST"]` against the span's hash; approve: exactly `M
/// docs/spec/movement/stamina.md`, `EDGE-STAM-REST` a section of
/// `RULE-STAM-REGEN`. M: update's rule relaxed.
#[test]
fn ac06_new_sections_are_added_below_the_target() {
    let pair = Pair::new("pk-ac06", "spec-a");
    let (hash, text) = regen_with_rest(&pair, &pair.linked);
    let id = created(&pair, &pair.linked, "RULE-STAM-REGEN", Some(&hash), &text);
    let review = review_json(&pair, &id);
    assert_eq!(review["kind"], json!("create"));
    assert_eq!(review["target_id"], json!("RULE-STAM-REGEN"));
    assert_eq!(review["target_path"], json!(STAMINA));
    assert_eq!(
        review["target_ids"],
        json!(["RULE-STAM-REGEN", "EDGE-STAM-REST"])
    );
    assert_eq!(review["base_hash"], json!(hash));
    assert_eq!(review["new_text"], json!(text));
    assert_eq!(review["preview"], json!("applies"));
    assert!(!pair.proposal(&id).new_file());
    assert_eq!(
        reservations(&pair),
        [("EDGE-STAM-REST".to_owned(), id.clone(), "open".to_owned())]
    );
    let base = pair.rev(&pair.linked, "HEAD");
    approve_ok(&pair, &pair.main, &id);
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(
        pair.git_text(&pair.main, &["rev-parse", &format!("{head}^")]),
        base
    );
    assert_eq!(name_status(&pair, &head), [format!("M\t{STAMINA}")]);
    let regen = pair.node(&pair.linked, "RULE-STAM-REGEN");
    assert!(
        regen
            .sections
            .iter()
            .any(|section| section.id == "EDGE-STAM-REST"),
        "{:?}",
        regen.sections
    );
    let rest = pair.node(&pair.linked, "EDGE-STAM-REST");
    assert_eq!(rest.path, STAMINA);
    assert!(regen.text.contains(REST), "{}", regen.text);
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Applied);
}

/// AC-06's refusals (exit 1, nothing stored): the new heading at the
/// target's level; no new ID; on `MEC-STAMINA`, `{#EDGE-STAM-ZERO}`
/// dropped or `## Regeneration` made `###`; AC-06's text by `propose
/// update` (its rule unchanged).
#[test]
fn ac06_a_section_create_keeps_every_id_and_level() {
    let pair = Pair::new("pk-ac06r", "spec-a");
    let (hash, span) = pair.span(&pair.linked, "RULE-STAM-REGEN");
    let level2 = format!("{span}\n\n## Rest delay {{#EDGE-STAM-REST}}\n- Waits.");
    let reason = refused_create(
        &pair,
        &pair.linked,
        "RULE-STAM-REGEN",
        Some(&hash),
        &level2,
        "a heading at the target's level",
    );
    assert!(
        reason.starts_with("`RULE-STAM-REGEN`: the edit does not stay inside the node"),
        "{reason}"
    );
    let plain = format!("{span}\n\n### Rest delay\n- Waits.");
    assert_eq!(
        refused_create(
            &pair,
            &pair.linked,
            "RULE-STAM-REGEN",
            Some(&hash),
            &plain,
            "no new ID"
        ),
        "no new ID: the text adds no `{#ID}` section; `spec propose update` replaces a span"
    );
    let (mec_hash, mec) = pair.span(&pair.linked, "MEC-STAMINA");
    let added = "\n\n## Rest {#EDGE-STAM-REST}\nA rest.\n";
    let dropped = format!("{}{added}", edit(&mec, " {#EDGE-STAM-ZERO}", ""));
    assert_eq!(
        refused_create(
            &pair,
            &pair.linked,
            "MEC-STAMINA",
            Some(&mec_hash),
            &dropped,
            "an ID dropped"
        ),
        "`MEC-STAMINA`: EDGE-STAM-ZERO (level 2) is not in the text: a create keeps every \
         `{#ID}` heading of the span and its level"
    );
    let deeper = format!(
        "{}{added}",
        edit(&mec, "## Regeneration", "### Regeneration")
    );
    assert_eq!(
        refused_create(
            &pair,
            &pair.linked,
            "MEC-STAMINA",
            Some(&mec_hash),
            &deeper,
            "a level changed"
        ),
        "`MEC-STAMINA`: `RULE-STAM-REGEN` was level 2, would be level 3: a create keeps every \
         `{#ID}` heading of the span and its level"
    );
    // The same text by `propose update`: its rule unchanged.
    let (_, text) = regen_with_rest(&pair, &pair.linked);
    let before = pair.state();
    let outcome = pair.propose(&pair.linked, "RULE-STAM-REGEN", &hash, &text);
    let reason = refused(&outcome, "propose update adding a section");
    assert!(
        reason.contains("the edit changes the file's structure"),
        "{reason}"
    );
    assert_eq!(pair.state(), before);
    assert!(pair.proposals().is_empty());
}

// ------------------------------------------------------------------ AC-07

/// AC-07: after AC-06's propose, a commit `Base rate 10` → `12`: `preview`
/// `rebases`, approve writes both changes. Instead a line added after the
/// `Exhausted` bullet: `conflicts`, approve exit 1, the conflict in its
/// answer, the file untouched, `apply_failed` step 5. M: `new_text` over
/// the current span.
#[test]
fn ac07_a_moved_span_rebases_or_conflicts() {
    let pair = Pair::new("pk-ac07", "spec-a");
    let (hash, text) = regen_with_rest(&pair, &pair.linked);
    let id = created(&pair, &pair.linked, "RULE-STAM-REGEN", Some(&hash), &text);
    let file = read_text(&pair.linked, STAMINA);
    write(
        &pair.linked,
        STAMINA,
        edit(&file, "Base rate 10 units/s", "Base rate 12 units/s"),
    );
    pair.commit_all(&pair.linked, "Base rate 12");
    assert_eq!(review_json(&pair, &id)["preview"], json!("rebases"));
    approve_ok(&pair, &pair.main, &id);
    let after = read_text(&pair.linked, STAMINA);
    assert!(after.contains("Base rate 12 units/s"), "{after}");
    assert!(after.contains(REST), "{after}");
    assert!(!after.contains("Base rate 10"), "{after}");

    let pair = Pair::new("pk-ac07c", "spec-a");
    let (hash, text) = regen_with_rest(&pair, &pair.linked);
    let id = created(&pair, &pair.linked, "RULE-STAM-REGEN", Some(&hash), &text);
    let file = read_text(&pair.linked, STAMINA);
    let walking = edit(
        &file,
        "rate \u{00d7} 0.5.\n",
        "rate \u{00d7} 0.5.\n- While walking, rate \u{00d7} 0.8.\n",
    );
    write(&pair.linked, STAMINA, &walking);
    pair.commit_all(&pair.linked, "Walking");
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(review_json(&pair, &id)["preview"], json!("conflicts"));
    let (outcome, questions) = approve(&pair, &pair.main, &id, true);
    let outcome = outcome.expect("a refusal, not an error");
    assert_eq!(outcome.exit(), Exit::NotFound, "{outcome:?}");
    let (stdout, _) = printed(&outcome);
    assert!(
        stdout.contains("<<<<<<< current\n- While walking, rate \u{00d7} 0.8.\n=======")
            && stdout.contains(">>>>>>> proposed"),
        "the conflict on stdout: {stdout}"
    );
    assert!(questions.is_empty());
    assert_eq!(read_text(&pair.linked, STAMINA), walking, "file untouched");
    assert_eq!(pair.rev(&pair.linked, "t1"), head);
    assert_eq!(pair.porcelain(&pair.linked), "");
    assert_eq!(
        pair.events_of(&id),
        events(&[(EVENT_CREATED, None), (EVENT_APPLY_FAILED, Some(5))])
    );
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Open);
}

// ------------------------------------------------------------------ AC-08

/// AC-08: T13 with `links: {derived_from: [R-99]}`: `introduced: 1`,
/// `ref-dangling` naming `R-99`, stored; approve applies it. M: findings
/// refusing.
#[test]
fn ac08_findings_are_stored_never_a_refusal() {
    let pair = Pair::new("pk-ac08", "spec-a");
    let text = edit(
        &t13(),
        "reviewed: 2026-09-20\n---\n",
        "reviewed: 2026-09-20\nlinks: {derived_from: [R-99]}\n---\n",
    );
    let outcome = create(&pair, &pair.linked, R13, None, &text).expect("propose create");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let (stdout, _) = printed(&outcome);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[..2], ["PR-0001", "introduced: 1"], "{stdout}");
    assert_eq!(lines.len(), 3, "{stdout}");
    assert!(
        lines[2].contains("ref-dangling") && lines[2].contains("R-99"),
        "{stdout}"
    );
    let stored = pair.proposal("PR-0001");
    assert_eq!(stored.diagnostics.len(), 1, "{:?}", stored.diagnostics);
    assert_eq!(stored.diagnostics[0].code, "ref-dangling");
    assert_eq!(stored.diagnostics[0].subject, "R-99");
    let (outcome, question) = approve_ok(&pair, &pair.main, "PR-0001");
    assert_eq!(
        question,
        format!(
            "apply PR-0001 to {R13} on t1 in {} (new file)? [y/N]",
            pair.linked.display()
        )
    );
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(name_status(&pair, &head), [format!("A\t{R13}")]);
    assert_eq!(blob(&pair, &head, R13), text.as_bytes());
    assert_eq!(outcome.document.status.as_deref(), Some("applied"));
}

// ------------------------------------------------------------------ AC-09

/// AC-09: exit 1, nothing stored: T13 `class: generated`;
/// `docs/generated/R-13.md`, `templates/R-13.md` (outside the walk),
/// `docs/records/R/R-13.txt`, `../R-13.md`; a section on `R-12`
/// (`immutable_text`); `id: DEC-0024` naming `spec approve`. M: the
/// `[decision_records]` check dropped.
#[test]
fn ac09_what_only_a_generator_or_spec_approve_writes_is_refused() {
    let pair = Pair::new("pk-ac09", "spec-a");
    let t13 = t13();
    let generated = edit(&t13, "class: canon\n", "class: generated\n");
    assert_eq!(
        refused_create(
            &pair,
            &pair.linked,
            R13,
            None,
            &generated,
            "class: generated"
        ),
        format!(
            "the text of `{R13}` is a `class: generated` document: only its registered \
             generator writes one, never a proposal; nothing stored"
        )
    );
    let outside = |path: &str| {
        format!(
            "`{path}` is no file the walk would list (outside the `[paths]` roots, excluded, or \
             a `.`-named component): a new spec file is a document of the corpus; nothing stored"
        )
    };
    for path in ["docs/generated/R-13.md", "templates/R-13.md"] {
        assert_eq!(
            refused_create(&pair, &pair.linked, path, None, &t13, path),
            outside(path)
        );
    }
    let reason = refused_create(
        &pair,
        &pair.linked,
        "docs/records/R/R-13.txt",
        None,
        &t13,
        ".txt",
    );
    assert!(
        reason.starts_with("`docs/records/R/R-13.txt` is no reference"),
        "{reason}"
    );
    assert_eq!(
        refused_create(&pair, &pair.linked, "../R-13.md", None, &t13, "../"),
        "`../R-13.md` is no clean root-relative path (no leading `/`, no `.`, `..` or empty \
         component): a new file is named by its path from the project root; nothing stored"
    );
    let (hash, span) = pair.span(&pair.linked, "R-12");
    let note = format!("{span}\n## Note {{#RULE-R12-NOTE}}\nA note.\n");
    assert_eq!(
        refused_create(&pair, &pair.linked, "R-12", Some(&hash), &note, "immutable"),
        "`R-12`: `R-12` has the prefix `R`, whose text is immutable (`immutable_text` in \
         `[ids]`): it is never updated"
    );
    assert_eq!(
        refused_create(
            &pair,
            &pair.linked,
            "docs/records/DEC/DEC-0024.md",
            None,
            &record("DEC-0024"),
            "DEC-0024"
        ),
        "`DEC-0024`: these records are made by `spec approve` of a `question` or \
         `discrepancy`; nothing stored"
    );
    assert!(pair.proposals().is_empty());
}

/// AC-09 where `[paths] generated` lies inside the walk (spec-b's root is
/// `docs`): the generated directory is named.
#[test]
fn ac09_the_generated_directory_inside_the_walk_is_named() {
    let pair = Pair::new("pk-ac09b", "spec-b");
    let path = "docs/generated/REQ-003.md";
    let reason = refused_create(
        &pair,
        &pair.linked,
        path,
        None,
        &record_b("REQ-003", "requirement"),
        "generated",
    );
    assert_eq!(
        reason,
        format!(
            "`{path}` lies under `[paths] generated` (`docs/generated`): only a registered \
             generator writes there, never a proposal; nothing stored"
        )
    );
}

// ------------------------------------------------------------------ AC-11

/// AC-11, the run half: spec-b, the same functions: the Cyrillic alias
/// prefix + `-003` exit 1 naming `REQ-003`; `id: REQ-003` stores and
/// applies as `A`; `GLS-merge` stores; `GLS-worktree` exit 1 naming its
/// file. M: a prefix literal outside core.
#[test]
fn ac11_spec_b_creates_with_its_own_scheme() {
    let pair = Pair::new("pk-ac11", "spec-b");
    let alias = "\u{0422}\u{0420}\u{0411}-003";
    assert_eq!(
        refused_create(
            &pair,
            &pair.linked,
            "docs/records/REQ/REQ-003.md",
            None,
            &record_b(alias, "requirement"),
            "the alias prefix"
        ),
        format!(
            "`{alias}` (line 2) is written with a legacy `aliases_from` prefix: a new ID is \
             written with its canonical prefix, `REQ-003`; nothing stored"
        )
    );
    let path = "docs/records/REQ/REQ-003.md";
    let text = record_b("REQ-003", "requirement");
    let id = created(&pair, &pair.linked, path, None, &text);
    assert_eq!(pair.proposal(&id).target_id, "REQ-003");
    approve_ok(&pair, &pair.main, &id);
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(name_status(&pair, &head), [format!("A\t{path}")]);
    assert_eq!(blob(&pair, &head, path), text.as_bytes());
    let merge = created(
        &pair,
        &pair.linked,
        "docs/records/GLS/GLS-merge.md",
        None,
        &record_b("GLS-merge", "glossary-entry"),
    );
    assert_eq!(pair.proposal(&merge).target_ids(), ["GLS-merge"]);
    assert_eq!(
        refused_create(
            &pair,
            &pair.linked,
            "docs/records/GLS/GLS-worktree-2.md",
            None,
            &record_b("GLS-worktree", "glossary-entry"),
            "GLS-worktree"
        ),
        "`GLS-worktree` is defined in `docs/records/GLS/GLS-worktree.md` \
         (`id: GLS-worktree`); nothing stored"
    );
}

// ------------------------------------------------------------------ AC-12

/// Commits `bytes` at `path` in the linked worktree by hand with the
/// trailer `Proposal: <id>`: the commit.
fn commit_by_hand(pair: &Pair, id: &str, path: &str, bytes: &[u8]) -> String {
    write(&pair.linked, path, bytes);
    pair.git.git(&pair.linked, &["add", "--", path]);
    pair.git.git(
        &pair.linked,
        &[
            "commit",
            "-q",
            "--no-verify",
            "-m",
            "Add it by hand",
            "-m",
            &format!("Proposal: {id}"),
        ],
    );
    pair.rev(&pair.linked, "t1")
}

/// AC-12: the file form `approved` at step 7 (the store's state), then
/// committed by hand (`new_text`, `Proposal: PR-0001`): approve →
/// `applied`, that commit, no commit or question of its own. M: completion
/// ignoring the blob.
#[test]
fn ac12_a_new_file_committed_by_hand_completes_it() {
    let pair = Pair::new("pk-ac12", "spec-a");
    let t13 = t13();
    let id = created(&pair, &pair.linked, R13, None, &t13);
    pair.queue()
        .approve(&id, &owner(), DECIDED)
        .expect("approved");
    let hand = commit_by_hand(&pair, &id, R13, t13.as_bytes());
    let (outcome, questions) = approve(&pair, &pair.main, &id, true);
    let outcome = outcome.expect("approve");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(questions.is_empty(), "{questions:?}");
    assert_eq!(pair.rev(&pair.linked, "t1"), hand, "no commit of its own");
    let stored = pair.proposal(&id);
    assert_eq!(stored.status, ProposalStatus::Applied);
    assert_eq!(stored.applied_commit.as_deref(), Some(hand.as_str()));
}

/// AC-12: committed by hand with other bytes: approve exit 1 `does not
/// carry the proposal's text`, no commit; reject refused while that commit
/// is on the branch. M: completion ignoring the blob.
#[test]
fn ac12_other_bytes_by_hand_complete_nothing() {
    let pair = Pair::new("pk-ac12o", "spec-a");
    let t13 = t13();
    let id = created(&pair, &pair.linked, R13, None, &t13);
    pair.queue()
        .approve(&id, &owner(), DECIDED)
        .expect("approved");
    let other = format!("{t13}One more line.\n");
    let hand = commit_by_hand(&pair, &id, R13, other.as_bytes());
    let (outcome, questions) = approve(&pair, &pair.main, &id, true);
    let reason = refused(&outcome, "other bytes");
    assert!(
        reason.contains("does not carry the proposal's text"),
        "{reason}"
    );
    assert!(questions.is_empty());
    assert_eq!(pair.rev(&pair.linked, "t1"), hand);
    assert_eq!(read(&pair.linked, R13), other.as_bytes());
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Approved);
    let before = pair.state();
    let outcome = reject_at(&pair, &pair.main, &id, "Changed my mind.");
    let reason = refused(&outcome, "reject with the commit on the branch");
    assert!(
        reason.contains(&hand) || reason.contains("Proposal"),
        "{reason}"
    );
    assert_eq!(pair.state(), before);
}

/// AC-12: a run killed between steps 8 and 9 (`approved`, the file holding
/// `new_text`, an intent-to-add entry): approve exit 1 at step 4 naming
/// the two ways out; the file and the entry as they were.
#[test]
fn ac12_a_killed_run_names_its_two_ways_out() {
    let pair = Pair::new("pk-ac12k", "spec-a");
    let t13 = t13();
    let id = created(&pair, &pair.linked, R13, None, &t13);
    pair.queue()
        .approve(&id, &owner(), DECIDED)
        .expect("approved");
    write(&pair.linked, R13, &t13);
    pair.git.git(&pair.linked, &["add", "-N", "--", R13]);
    let head = pair.rev(&pair.linked, "t1");
    let status = pair.porcelain(&pair.linked);
    let (outcome, questions) = approve(&pair, &pair.main, &id, true);
    let reason = refused(&outcome, "a killed run");
    assert!(questions.is_empty());
    for part in [
        format!("`{R13}` exists"),
        format!("it holds `{id}`'s text as an interrupted apply left it"),
        format!("two ways out in {}", pair.linked.display()),
        format!("git commit --only --trailer 'Proposal: {id}'"),
        format!("git rm --cached -- {R13}"),
        format!("`spec approve {id}` completes it"),
    ] {
        assert!(reason.contains(&part), "{part}: {reason}");
    }
    assert_eq!(
        pair.events_of(&id).last(),
        Some(&(EVENT_APPLY_FAILED.to_owned(), Some(4)))
    );
    assert_eq!(read(&pair.linked, R13), t13.as_bytes());
    assert_eq!(pair.porcelain(&pair.linked), status);
    assert_eq!(pair.rev(&pair.linked, "t1"), head);
    // The first way out: the commit by hand, then approve completes it.
    pair.git.git(
        &pair.linked,
        &[
            "commit",
            "-q",
            "--no-verify",
            "--only",
            "--trailer",
            &format!("Proposal: {id}"),
            "-m",
            &format!("spec: apply {id}"),
            "--",
            R13,
        ],
    );
    let hand = pair.rev(&pair.linked, "t1");
    let (outcome, questions) = approve(&pair, &pair.main, &id, true);
    assert_eq!(outcome.expect("approve").exit(), Exit::Answered);
    assert!(questions.is_empty());
    assert_eq!(
        pair.proposal(&id).applied_commit.as_deref(),
        Some(hand.as_str())
    );
}

// ------------------------------------------------------------------ AC-13

/// AC-13: `approve PR-0001` with `--option 0`, `--answer a`, `--canon
/// R-12`: exit 2, no event, nothing changed; `propose create
/// docs/records/R/R-13.md --base b3:<any>` exit 1 `drop --base`; `propose
/// create RULE-STAM-REGEN` without `--base` exit 1 `exists`. M: a decision
/// flag accepted.
#[test]
fn ac13_decision_flags_and_a_misplaced_base_are_refused() {
    let pair = Pair::new("pk-ac13", "spec-a");
    let t13 = t13();
    let id = created(&pair, &pair.linked, R13, None, &t13);
    let before = pair.state();
    for (flags, name) in [
        (
            ApproveFlags {
                option: Some(0),
                ..ApproveFlags::default()
            },
            "--option",
        ),
        (
            ApproveFlags {
                answer: Some("a".to_owned()),
                ..ApproveFlags::default()
            },
            "--answer",
        ),
        (
            ApproveFlags {
                canon: Some("R-12".to_owned()),
                ..ApproveFlags::default()
            },
            "--canon",
        ),
    ] {
        let (outcome, questions) = pair.decide_at(&pair.main, &id, &flags, None, true, DECIDED);
        let message = cannot(&outcome, name);
        assert_eq!(
            message,
            format!(
                "spec: `{name}` decides a question or a discrepancy; `{id}` is a create: `spec \
                 approve {id}` applies it as proposed; nothing changed"
            )
        );
        assert!(questions.is_empty());
        assert_eq!(pair.state(), before, "{name}");
        assert_eq!(pair.events_of(&id), events(&[(EVENT_CREATED, None)]));
    }
    let base = format!("b3:{}", "0".repeat(64));
    assert_eq!(
        refused_create(
            &pair,
            &pair.linked,
            "docs/records/R/R-14.md",
            Some(&base),
            &record("R-14"),
            "--base on a new file"
        ),
        "nothing at `docs/records/R/R-14.md`: a new file is written against no base; drop --base"
    );
    let (_, text) = regen_with_rest(&pair, &pair.linked);
    assert_eq!(
        refused_create(
            &pair,
            &pair.linked,
            "RULE-STAM-REGEN",
            None,
            &text,
            "a node without --base"
        ),
        "`RULE-STAM-REGEN` exists: a create never replaces a file; to add ID sections to it, \
         name its span_hash with --base"
    );
}

// ------------------------------------------------------------------ AC-14

/// The `Env` of `cwd` with `home` as `HOME`.
fn env_at(home: &Path, cwd: &Path) -> specengine_cli::Env {
    specengine_cli::Env {
        cwd: cwd.to_path_buf(),
        home: Some(home.as_os_str().to_owned()),
        xdg_data_home: None,
    }
}

/// `spec export state --out <out>` of the queue under `home`: its bytes.
fn export(pair: &Pair, home: &Path, out: &Path) -> Vec<u8> {
    specengine_cli::export_state(
        &env_at(home, &pair.main),
        &Globals::default(),
        &specengine_cli::ExportStateRequest {
            out: Some(out.to_path_buf()),
            now: DECIDED.to_owned(),
            git: pair.git_env(&pair.main),
        },
    )
    .unwrap_or_else(|error| panic!("export state: {error}"));
    fs::read(out).expect("the dump")
}

/// The queue's `dump()` under `home`.
fn dump_at(pair: &Pair, home: &Path) -> String {
    specengine_store::SqliteQueue::open(
        common::data_dir(home).join(format!("{}.db", pair.slug)),
        &pair.slug,
    )
    .expect("open")
    .dump()
    .expect("dump")
}

/// AC-14: `review PR-0001`'s `diff` against an empty base (`--- base
/// <path>`, `+++ proposed <path>`, `@@ -0,0 +1,<n> @@`, every other line
/// `+`), `preview` `applies`, a file at the path → `unavailable`; `inbox`
/// `PR-0001 | create | open | R-13 | …`; `export state` (`queue_schema` 3,
/// 40 columns), `import-state` fresh, the re-export byte-identical,
/// `user_version` 3. M: a column or a schema bump.
#[test]
fn ac14_review_inbox_and_backup_carry_a_create() {
    let pair = Pair::new("pk-ac14", "spec-a");
    let t13 = t13();
    let id = created(&pair, &pair.linked, R13, None, &t13);
    let (hash, text) = regen_with_rest(&pair, &pair.linked);
    let section = created(&pair, &pair.linked, "RULE-STAM-REGEN", Some(&hash), &text);
    let review = review_json(&pair, &id);
    let diff = review["diff"].as_str().expect("a diff");
    let lines: Vec<&str> = diff.lines().collect();
    let n = t13.lines().count();
    assert_eq!(
        lines[..3],
        [
            format!("--- base {R13}").as_str(),
            format!("+++ proposed {R13}").as_str(),
            format!("@@ -0,0 +1,{n} @@").as_str(),
        ],
        "{diff}"
    );
    assert_eq!(lines.len(), n + 3, "{diff}");
    let added: Vec<&str> = lines[3..]
        .iter()
        .map(|line| line.strip_prefix('+').expect("every line added"))
        .collect();
    assert_eq!(added, t13.lines().collect::<Vec<_>>());
    assert_eq!(review["preview"], json!("applies"));
    let (review_text, _) = printed(&pair.review_ok(&pair.main, &id));
    assert!(
        review_text.contains("\nbase_hash: -\nbase_text: -\n"),
        "{review_text}"
    );
    write(&pair.linked, R13, "X\n");
    let blocked = review_json(&pair, &id);
    assert_eq!(blocked["preview"], json!("unavailable"));
    assert!(blocked.to_string().contains("step 4"), "{blocked}");
    fs::remove_file(pair.linked.join(R13)).expect("removed");

    let (inbox, inbox_json) = printed_inbox(&pair.inbox(&pair.main, false).expect("inbox"));
    assert_eq!(
        inbox,
        format!(
            "PR-0001 | create | open | R-13 | t1 | {CLOCK} | {RATIONALE}\n\
             PR-0002 | create | open | RULE-STAM-REGEN | t1 | {CLOCK} | {RATIONALE}\n"
        )
    );
    let listed = json_of(&inbox_json);
    assert_eq!(listed["proposals"][0]["target_ids"], json!(["R-13"]));
    assert_eq!(
        listed["proposals"][1]["target_ids"],
        json!(["RULE-STAM-REGEN", "EDGE-STAM-REST"])
    );
    assert_eq!(section, "PR-0002");

    let dumps = pair.scratch.dir("dumps");
    let bytes = export(&pair, &pair.home, &dumps.join("q.jsonl"));
    let lines: Vec<String> = String::from_utf8(bytes.clone())
        .expect("UTF-8")
        .lines()
        .map(str::to_owned)
        .collect();
    assert!(
        lines[0].starts_with(
            "{\"format\":1,\"queue_schema\":3,\"project\":\"lantern-keep\",\"proposals\":2,"
        ),
        "{}",
        lines[0]
    );
    let columns = specengine_store::PROPOSAL_COLUMNS;
    assert_eq!(columns.len(), 40);
    for line in &lines[1..3] {
        let row: Value = serde_json::from_str(line).expect("a row");
        let row = row["proposals"].as_object().expect("a proposals row");
        assert_eq!(row.len(), 40, "{line}");
        assert_eq!(row["kind"], json!("create"), "{line}");
    }
    let first: Value = serde_json::from_str(&lines[1]).unwrap();
    assert_eq!(first["proposals"]["base_hash"], Value::Null);
    assert_eq!(first["proposals"]["target_ids"], json!("[\"R-13\"]"));
    let fresh = pair.scratch.home("fresh");
    let mut consent = |_: &str| true;
    let outcome = specengine_cli::import_state(
        &env_at(&fresh, &pair.main),
        &Globals::default(),
        &specengine_cli::ImportStateRequest {
            file: dumps.join("q.jsonl"),
        },
        &mut consent,
    )
    .expect("import");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(dump_at(&pair, &fresh), dump_at(&pair, &pair.home));
    assert_eq!(export(&pair, &fresh, &dumps.join("again.jsonl")), bytes);
    assert_eq!(pair.sql("PRAGMA user_version;"), "3\n");
    assert_eq!(
        pair.sql("SELECT count(*) FROM pragma_table_info('proposals');"),
        "40\n"
    );
    let restored = specengine_store::SqliteQueue::open(
        common::data_dir(&fresh).join("lantern-keep.db"),
        "lantern-keep",
    )
    .unwrap();
    assert_eq!(restored.get(&id).unwrap().unwrap(), pair.proposal(&id));
    assert_eq!(
        restored.reserved().unwrap(),
        pair.queue().reserved().unwrap()
    );
}

// ------------------------------------------------------------------ AC-16

/// The comment text of a Rust source: every `//` comment's body (after
/// `//`, `///` or `//!`), trimmed, joined by single spaces.
fn comment_text(source: &str) -> String {
    let mut out = Vec::new();
    for line in source.lines() {
        if let Some(at) = line.find("//") {
            let body = line[at..].trim_start_matches('/');
            let body = body.strip_prefix('!').unwrap_or(body).trim();
            if !body.is_empty() {
                out.push(body.to_owned());
            }
        }
    }
    out.join(" ")
}

/// The quoted headings after each occurrence of `lead` in `text`: `"A"`,
/// then any `, "B"`.
fn cited_headings(text: &str, lead: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(lead) {
        rest = &rest[at + lead.len()..];
        let mut tail = rest;
        while let Some(quoted) = tail.strip_prefix('"') {
            let Some(end) = quoted.find('"') else { break };
            found.push(quoted[..end].to_owned());
            tail = &quoted[end + 1..];
            match tail.strip_prefix(", ") {
                Some(next) if next.starts_with('"') => tail = next,
                _ => break,
            }
        }
    }
    found
}

/// AC-16, the sources' half: no comment of `crates/*/src` cites
/// `decision-apply`; every citation of the canon `decision-record` (its
/// path in backticks then quoted headings, or the CLI's form: the word
/// canon, its name in backticks, a comma, quoted headings) names headings
/// the canon has. M: a heading the canon lacks.
#[test]
fn ac16_the_source_comments_cite_the_decision_record_canon() {
    let root = common::repository_root();
    let canon = read_text(&root, "docs/canon/decision-record.md");
    let headings: Vec<&str> = canon
        .lines()
        .filter_map(|line| line.strip_prefix("## "))
        .collect();
    assert!(headings.contains(&"Steps") && headings.contains(&"Queue and documents"));
    let mut sources = Vec::new();
    for entry in fs::read_dir(root.join("crates")).expect("crates") {
        let src = entry.expect("an entry").path().join("src");
        let mut stack = vec![src];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries {
                let path = entry.expect("an entry").path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    sources.push(path);
                }
            }
        }
    }
    sources.sort();
    assert!(sources.len() > 50, "{} sources", sources.len());
    let mut stale = Vec::new();
    let mut cited = 0;
    let mut unknown = Vec::new();
    for path in &sources {
        let text = fs::read_to_string(path).expect("a source");
        let comments = comment_text(&text);
        if comments.contains("decision-apply") {
            stale.push(path.display().to_string());
        }
        // Assembled, so that this file holds no citation of its own.
        let path_form = format!("`{}` ", ["docs", "canon", "decision-record.md"].join("/"));
        for lead in [path_form.as_str(), "canon `decision-record`, "] {
            for heading in cited_headings(&comments, lead) {
                cited += 1;
                if !headings.contains(&heading.as_str()) {
                    unknown.push(format!("{}: {heading:?}", path.display()));
                }
            }
        }
    }
    assert!(
        stale.is_empty(),
        "comments citing decision-apply: {stale:?}"
    );
    assert!(cited >= 15, "{cited} citations found");
    assert!(unknown.is_empty(), "headings the canon lacks: {unknown:?}");
    // The scan sees what it must refuse.
    assert_eq!(
        cited_headings(
            "x canon `decision-record`, \"Flags\", \"Nope\") y",
            "canon `decision-record`, "
        ),
        ["Flags", "Nope"]
    );
}

// --------------------------------------------- iteration 2: n1, n2, n4

/// n1: a section create whose text is already in place (applied by hand,
/// no trailer): approve exit 1 at step 5 naming it in place, never step
/// 4's "defined in" its own file; a new ID defined in another file
/// meanwhile: step 4 names that file. M: step 4's ID check before step
/// 5's "in place".
#[test]
fn n1_a_section_create_in_place_is_step_5_before_the_id_check() {
    let pair = Pair::new("pk-n1", "spec-a");
    let (hash, text) = regen_with_rest(&pair, &pair.linked);
    let id = created(&pair, &pair.linked, "RULE-STAM-REGEN", Some(&hash), &text);
    let file = read_text(&pair.linked, STAMINA);
    let (_, span) = pair.span(&pair.linked, "RULE-STAM-REGEN");
    write(&pair.linked, STAMINA, edit(&file, &span, &text));
    pair.commit_all(&pair.linked, "Rest delay by hand");
    let head = pair.rev(&pair.linked, "t1");
    let (outcome, questions) = approve(&pair, &pair.main, &id, true);
    let reason = refused(&outcome, "in place");
    assert!(
        reason.contains(&format!(
            "`RULE-STAM-REGEN`: the proposal's text is already in place in `{STAMINA}`; \
             nothing to write or commit"
        )),
        "{reason}"
    );
    assert!(!reason.contains("is defined in"), "{reason}");
    assert!(questions.is_empty());
    assert_eq!(pair.rev(&pair.linked, "t1"), head);
    assert_eq!(
        pair.events_of(&id).last(),
        Some(&(EVENT_APPLY_FAILED.to_owned(), Some(5)))
    );

    let pair = Pair::new("pk-n1b", "spec-a");
    let (hash, text) = regen_with_rest(&pair, &pair.linked);
    let id = created(&pair, &pair.linked, "RULE-STAM-REGEN", Some(&hash), &text);
    let sprint = read_text(&pair.linked, "docs/spec/movement/sprint.md");
    write(
        &pair.linked,
        "docs/spec/movement/sprint.md",
        format!("{sprint}\n## Rest {{#EDGE-STAM-REST}}\nTaken here.\n"),
    );
    pair.commit_all(&pair.linked, "EDGE-STAM-REST in sprint");
    let (outcome, _) = approve(&pair, &pair.main, &id, true);
    let reason = refused(&outcome, "defined elsewhere");
    assert!(
        reason.contains(
            "`EDGE-STAM-REST` is defined in `docs/spec/movement/sprint.md` (`id: MEC-SPRINT`)"
        ),
        "{reason}"
    );
    assert_eq!(
        pair.events_of(&id).last(),
        Some(&(EVENT_APPLY_FAILED.to_owned(), Some(4)))
    );
}

/// `mkfifo path`.
fn mkfifo(path: &Path) {
    let status = Command::new("/usr/bin/mkfifo")
        .arg(path)
        .status()
        .expect("mkfifo runs");
    assert!(status.success(), "mkfifo {}", path.display());
}

/// A store-level create of `R-13` at another path of the linked worktree:
/// the competing live create of the race.
fn competing(pair: &Pair) -> NewProposal {
    let text = record("R-13");
    NewProposal {
        kind: ProposalKind::Create,
        target_id: "R-13".to_owned(),
        target_path: "docs/records/R/R-13-c.md".to_owned(),
        place: Place {
            git_common_dir: pair.main.join(".git").display().to_string(),
            worktree: pair.linked.display().to_string(),
            root_rel: String::new(),
            branch: "t1".to_owned(),
            base_commit: pair.rev(&pair.linked, "HEAD"),
        },
        base_hash: None,
        base_text: None,
        patch_hash: patch_hash("R-13", "", &text),
        new_text: text,
        rationale: "The other one.".to_owned(),
        author: Author::human(),
        diagnostics: Vec::new(),
        new_ids: vec!["R-13".to_owned()],
    }
}

/// n2: a create that loses the race (another live create takes its new ID
/// after step 4 read the reservations, before step 7's insert) is step
/// 4's refusal: the holder with its state (`open`, `approved`) and the
/// next free ID; nothing of its own stored. The hook, deterministic and
/// with no product change: the root's `.spec-debt.toml` is a FIFO, which
/// only step 5's validation opens; while the propose waits on it the
/// competing create is stored, then the FIFO is closed empty.
#[test]
fn n2_a_create_lost_in_the_race_names_the_holder_and_its_state() {
    for status in ["open", "approved"] {
        let pair = Pair::new(&format!("pk-n2-{status}"), "spec-a");
        let fifo: PathBuf = pair.linked.join(".spec-debt.toml");
        mkfifo(&fifo);
        let env = pair.env(&pair.linked);
        let request = request(&pair, &pair.linked, R13, None, &t13());
        let (done, finished) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = done.send(propose_create(&env, &Globals::default(), &request));
        });
        let (opened, at_step_5) = mpsc::channel();
        let (go, release) = mpsc::channel::<()>();
        let writer_path = fifo.clone();
        let writer = std::thread::spawn(move || {
            let file = fs::OpenOptions::new()
                .write(true)
                .open(&writer_path)
                .expect("the FIFO opens for writing");
            let _ = opened.send(());
            let _ = release.recv();
            drop(file);
        });
        if at_step_5.recv_timeout(RUN_TIMEOUT).is_err() {
            // Pair the waiting writer, then fail.
            let _ = fs::File::open(&fifo);
            let _ = go.send(());
            let _ = writer.join();
            panic!("{status}: the propose never opened the baseline (step 5)");
        }
        let holder = pair
            .queue()
            .create(&competing(&pair), CLOCK)
            .expect("the competing create");
        assert_eq!(holder.id, "PR-0001");
        if status == "approved" {
            pair.queue()
                .approve("PR-0001", &owner(), DECIDED)
                .expect("approved");
        }
        go.send(()).expect("release the writer");
        writer.join().expect("the writer");
        let Ok(outcome) = finished.recv_timeout(RUN_TIMEOUT) else {
            // A second read of the baseline waits on the FIFO: pair it from
            // a helper (the test process ends with the panic), then fail.
            let unblock = fifo.clone();
            std::thread::spawn(move || fs::OpenOptions::new().write(true).open(unblock));
            panic!("{status}: the propose did not finish (a second read of the baseline?)");
        };
        let reason = refused(&outcome, status);
        assert_eq!(
            reason,
            format!(
                "`R-13` is reserved by `PR-0001` ({status}), a live create; the next free is \
                 `R-14`; nothing stored"
            )
        );
        let ids: Vec<String> = pair.proposals().into_iter().map(|row| row.id).collect();
        assert_eq!(ids, ["PR-0001"], "{status}: nothing of its own stored");
        assert_eq!(
            reservations(&pair),
            [("R-13".to_owned(), "PR-0001".to_owned(), status.to_owned())]
        );
    }
}

/// spec-b's config with `[paths] index = "docs/index.md"` and an `index =
/// true` generator writing it, an archive shard and a live shard.
fn sharded_config(base: &str) -> String {
    let mut text = set_paths_key(base, "index", &quoted("docs/index.md"));
    text.push_str(
        "\n[[generators]]\ncommand = \"gen-index\"\nwrites  = [\"docs/index.md\", \
         \"docs/index-archive.md\", \"docs/spec/index-records.md\"]\nindex   = true\n\
         shards  = [\n  { path = \"docs/index-archive.md\", tier3 = true },\n  \
         { path = \"docs/spec/index-records.md\", claims = [\"docs/records/**\"] },\n]\n",
    );
    text
}

/// n4: `[paths] index` and each shard of the index generator (live,
/// archive) are refused at propose (exit 1, nothing stored); a create
/// stored before the config named its path is refused at apply step 3
/// (`apply_failed` step 3, nothing changed). M: the index's files not
/// refused.
#[test]
fn n4_the_index_and_its_shards_are_never_created_by_a_proposal() {
    let pair = Pair::new("pk-n4", "spec-b");
    let page = "---\nclass: generated\n---\n\n# Index\n";
    let plain = "# Index\n\nA page.\n";
    // Stored before the config names it.
    let early = created(
        &pair,
        &pair.linked,
        "docs/spec/index-records.md",
        None,
        plain,
    );
    let config = read_text(&pair.linked, "specengine.toml");
    write(&pair.linked, "specengine.toml", sharded_config(&config));
    pair.commit_all(&pair.linked, "The index and its shards");
    for (path, what) in [
        ("docs/index.md", "the project's index (`[paths] index`)"),
        ("docs/index-archive.md", "the index's archive shard"),
        ("docs/spec/index-records.md", "a shard of the index"),
    ] {
        for text in [plain, page] {
            let reason = refused_create(&pair, &pair.linked, path, None, text, path);
            assert_eq!(
                reason,
                format!(
                    "`{path}` is {what}: only `spec export index` writes it, never a proposal; \
                     nothing stored"
                ),
                "{path}"
            );
        }
    }
    let head = pair.rev(&pair.linked, "t1");
    let (outcome, questions) = approve(&pair, &pair.main, &early, true);
    let reason = refused(&outcome, "apply step 3");
    assert!(
        reason.contains(
            "`docs/spec/index-records.md` is a shard of the index: only `spec export index` \
             writes it, never a proposal; nothing changed"
        ),
        "{reason}"
    );
    assert!(questions.is_empty());
    assert_eq!(pair.rev(&pair.linked, "t1"), head);
    assert!(!pair.linked.join("docs/spec/index-records.md").exists());
    assert_eq!(
        pair.events_of(&early),
        events(&[(EVENT_CREATED, None), (EVENT_APPLY_FAILED, Some(3))])
    );
}
