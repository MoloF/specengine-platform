//! docs/features/lock-levels.md RD-2's **M4**
//! (`format_1_text_is_the_same_everywhere`), RD-4's
//! `lock_level_names_match_the_marker_grammar` and TE-1 ("Readers"): who
//! reads `spec.lock` how, over scratch repositories of `fixtures/code-a`
//! with a lock committed (every git process sandboxed, fixed dates, a
//! scratch `HOME`).
//!
//! - Every writer and code judge reads with the build's recipes (Q27): a
//!   format-1 lock (the format-1 golden,
//!   `fixtures/spec-lock/code-a.format-1.lock`), a format-2 one of another
//!   `code_recipe`, of another `norm_recipe`, or claiming `format = 3`
//!   exits 2 with the text of Q27's first failing check (one text, every
//!   caller alike: Q11), `git status --porcelain=v1 -z`, `HEAD`, the refs,
//!   the index and every file (the lock's bytes) as before. The callers:
//!   `spec verify` plain, `--changed`, `--staged`; `spec lock accept`,
//!   `--editorial`, a gone node's; `spec propose drift`; a drift approve;
//!   an editorial approve that would refresh an entry (Q29, format 1).
//!   MCP `report_drift`'s case is `mcp_drift.rs`'.
//! - The readers of the spec side alone read format 1 (Q10): the rule
//!   through `spec check --staged` (and `rev_rule.rs`
//!   `format_1_head_lock_is_read`, M3), `spec bump`, `spec review`'s
//!   vouching count, each giving what it gives over the format-2 golden
//!   (MCP `get_proposal`'s case is `mcp_drift.rs`').
//!
//! The variants are made from the format-2 golden
//! (`fixtures/spec-lock/code-a.lock`), never hand-written digests.

#![cfg(unix)]

mod common;
mod drift_common;
mod lock_common;
mod rev_common;
mod rev_rule_common;
mod verify_common;

use std::fs;

use common::proposal::edit;
use lock_common::{
    CODE_RECIPE_FORMAT_1, LockRepo, RepoState, Variant, format_1_text, golden_format_1_text,
};
use rev_common::{At, approve_at, proposed, review_at};
use serde_json::json;
use specengine_cli::{Globals, ShowRequest, show};
use specengine_code::Level;
use specengine_core::lock::LockLevel;
use verify_common::{Verify, golden, verify};

const LOCK: &str = "spec.lock";
const COUNTER_MD: &str = "docs/spec/counter.md";
const GOAL_MD: &str = "docs/records/GOAL-01.md";
const PIECE: &str = "PIECE-COUNTER";
/// A change of `PIECE-COUNTER`'s own text.
const MEANING: (&str, &str) = ("grows one step at a time", "grows a single step at a time");

// ------------------------------------------------------------------ RD-4

/// RD-4: core's own copy of the level names (`lock.rs`, which reads no
/// code crate) is the marker grammar's, `specengine-code`'s `Level::ALL`,
/// name for name and in its canonical order.
#[test]
fn lock_level_names_match_the_marker_grammar() {
    let lock: Vec<&str> = LockLevel::ALL.iter().map(|level| level.as_str()).collect();
    let grammar: Vec<&str> = Level::ALL.iter().map(|level| level.as_str()).collect();
    assert_eq!(lock, grammar);
    assert_eq!(lock, ["path", "sig", "body", "deps"]);
}

// ----------------------------------------------------------- the callers

/// code-a, the format-2 golden committed (`the lock`), then `variant`'s
/// bytes committed over it.
fn repo_with(label: &str, variant: Variant) -> LockRepo {
    let repo = golden(label);
    let text = variant.text(&repo.lock_text());
    repo.write(LOCK, &text);
    repo.commit_all(variant.label());
    repo
}

/// The library's view from the root.
fn at(repo: &LockRepo) -> At<'_> {
    At::new(&repo.root, &repo.home, repo.git_env(&repo.root))
}

/// `(span_hash, text)` of `reference` as `spec show` gives them.
fn shown(repo: &LockRepo, reference: &str) -> (String, String) {
    let outcome = show(
        &repo.env(&repo.root),
        &Globals::default(),
        &ShowRequest {
            reference: reference.to_owned(),
            links: false,
            archive: false,
        },
    )
    .unwrap_or_else(|error| panic!("show {reference}: {error}"));
    let node = outcome.nodes.into_iter().next().expect("one node");
    (node.span_hash, node.text)
}

/// Proposes `reference`'s span with `(from, to)` applied: its ID.
fn propose_edit(repo: &LockRepo, reference: &str, (from, to): (&str, &str)) -> String {
    let (hash, text) = shown(repo, reference);
    proposed(&at(repo), reference, &hash, &edit(&text, from, to))
}

/// A refusing caller: its name and its run over `repo`, giving the exit-2
/// message (a verify run: its `cannot:` causes, the first one).
struct Caller {
    name: &'static str,
    /// Prepares `repo` (holding the format-2 golden) before the variant is
    /// committed; `None`: nothing.
    before: Option<fn(&LockRepo)>,
    /// Prepares `repo` after the variant is committed.
    after: Option<fn(&LockRepo)>,
    run: fn(&LockRepo) -> String,
}

/// The first `cannot:` cause of a verify run that exits 2 with no row.
fn first_cause(run: &Verify, context: &str) -> String {
    assert_eq!(run.code(), 2, "{context}:\n{}", run.text.show());
    assert_eq!(run.verdict(), "cannot-verify", "{context}");
    assert!(run.rows().is_empty(), "{context}: no row judged");
    let causes = run.json["cannot"].as_array().expect("cannot");
    let first = causes
        .first()
        .and_then(|cause| cause.as_str())
        .unwrap_or_else(|| panic!("{context}: no cause"))
        .to_owned();
    run.has_line(&format!("cannot: {first}"));
    first
}

fn verify_plain(repo: &LockRepo) -> String {
    first_cause(&verify(repo, &[]), "verify")
}

fn verify_changed(repo: &LockRepo) -> String {
    first_cause(&verify(repo, &["--changed"]), "verify --changed")
}

/// `--staged` reads the index's lock: its cause names the index.
fn verify_staged(repo: &LockRepo) -> String {
    let cause = first_cause(&verify(repo, &["--staged"]), "verify --staged");
    cause
        .strip_suffix(" (in the index)")
        .unwrap_or_else(|| panic!("the index named: {cause}"))
        .to_owned()
}

fn accept(repo: &LockRepo) -> String {
    let attempt = repo.accept(PIECE, true);
    assert!(
        attempt.prompts.is_empty(),
        "accept asked: {:?}",
        attempt.prompts
    );
    attempt.cannot("accept")
}

fn accept_editorial(repo: &LockRepo) -> String {
    let attempt = repo.editorial(PIECE, true);
    assert!(attempt.prompts.is_empty(), "--editorial asked");
    attempt.cannot("accept --editorial")
}

/// `GOAL-01`'s document deleted and committed: a gone node with entries.
fn goal_gone(repo: &LockRepo) {
    fs::remove_file(repo.root.join(GOAL_MD)).expect("GOAL-01's document");
    repo.commit_all("GOAL-01 gone");
}

fn accept_gone(repo: &LockRepo) -> String {
    let attempt = repo.accept("GOAL-01", true);
    assert!(attempt.prompts.is_empty(), "a gone node's accept asked");
    attempt.cannot("a gone node's accept")
}

fn propose_drift(repo: &LockRepo) -> String {
    drift_common::raise(repo, &[]).cannot("propose drift")
}

/// `CAP` changed and committed over the format-2 golden, `LAW-BOUND`'s
/// drift raised (`PR-0001`).
fn drift_raised(repo: &LockRepo) {
    drift_common::cap_changed(repo);
    repo.commit_all("the cap");
    let raised = drift_common::raised(repo, &["LAW-BOUND"]);
    assert_eq!(raised.created_nodes(), ["LAW-BOUND"]);
}

fn drift_approve(repo: &LockRepo) -> String {
    let asked = drift_common::approve_yes(repo, "PR-0001");
    assert!(asked.questions.is_empty(), "a drift approve asked");
    asked.cannot("a drift approve")
}

const CALLERS: [Caller; 8] = [
    Caller {
        name: "spec verify",
        before: None,
        after: None,
        run: verify_plain,
    },
    Caller {
        name: "spec verify --changed",
        before: None,
        after: None,
        run: verify_changed,
    },
    Caller {
        name: "spec verify --staged",
        before: None,
        after: None,
        run: verify_staged,
    },
    Caller {
        name: "spec lock accept",
        before: None,
        after: None,
        run: accept,
    },
    Caller {
        name: "spec lock accept --editorial",
        before: None,
        after: None,
        run: accept_editorial,
    },
    Caller {
        name: "a gone node's accept",
        before: None,
        after: Some(goal_gone),
        run: accept_gone,
    },
    Caller {
        name: "spec propose drift",
        before: None,
        after: None,
        run: propose_drift,
    },
    Caller {
        name: "a drift approve",
        before: Some(drift_raised),
        after: None,
        run: drift_approve,
    },
];

/// Every caller over `variant`: `(caller, message)`, each run on a
/// repository of its own, its state unchanged by the run (status, `HEAD`,
/// refs, index, every file: the lock's bytes).
fn refusals(variant: Variant) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for (index, caller) in CALLERS.iter().enumerate() {
        let repo = golden(&format!("lf-{index}-{}", variant.label().replace(' ', "-")));
        if let Some(before) = caller.before {
            before(&repo);
        }
        let text = variant.text(&repo.lock_text());
        repo.write(LOCK, &text);
        repo.commit_all(variant.label());
        if let Some(after) = caller.after {
            after(&repo);
        }
        let before: RepoState = repo.state();
        let status = repo.porcelain();
        let message = (caller.run)(&repo);
        assert_eq!(repo.state(), before, "{}: nothing changed", caller.name);
        assert_eq!(repo.porcelain(), status, "{}: status", caller.name);
        assert_eq!(repo.lock_text(), text, "{}: the lock's bytes", caller.name);
        out.push((caller.name, message));
    }
    out
}

/// RD-2 **M4** (one caller without the step): a format-1 lock gives every
/// writer and code judge the one text, `spec.lock:2: format 1 ("<its
/// code_recipe>"), this build's is 2 ("<build's>"): run spec lock rebase
/// on a terminal`, exit 2, nothing changed. The text is the reader's,
/// whole; a caller may set it in its own frame as it does any lock error
/// (`spec propose drift`: `cannot judge the drift: … (at <commit>)`; a
/// drift approve: `` `PR` not applied (step 3): … ``), never reword it; an editorial approve that
/// would refresh an entry (step 3's lock, read without the build's
/// recipes) refuses with the same text before rendering (Q29).
#[test]
fn format_1_text_is_the_same_everywhere() {
    let want = Variant::Format1.message();
    assert!(
        want.ends_with(&format_1_text(CODE_RECIPE_FORMAT_1)),
        "{want}"
    );
    let mut failures = Vec::new();
    for (caller, message) in refusals(Variant::Format1) {
        if !message.contains(&want) {
            failures.push(format!("{caller}:\n  want {want}\n  got  {message}"));
        }
    }

    // The editorial approve of PIECE-COUNTER, whose four entries the
    // format-1 lock holds: exit 2 before the question, nothing written.
    let repo = repo_with("lf-editorial-refreshing", Variant::Format1);
    let id = propose_edit(&repo, PIECE, MEANING);
    let before = repo.state();
    let asked = approve_at(&at(&repo), &id, true, true);
    let message = asked.cannot("an editorial approve refreshing entries");
    if !message.contains(&format_1_text(CODE_RECIPE_FORMAT_1)) {
        failures.push(format!("an editorial approve: {message}"));
    }
    assert!(asked.questions.is_empty(), "refused before the question");
    assert_eq!(
        repo.state(),
        before,
        "an editorial approve: nothing written"
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// TE-1: every refusing caller × another `code_recipe`, another
/// `norm_recipe`, `format = 3` (format 1: [`format_1_text_is_the_same_everywhere`]):
/// exit 2 with the text of Q27's first failing check, nothing changed.
#[test]
fn every_refusing_caller_refuses_every_variant() {
    let mut failures = Vec::new();
    for variant in [Variant::OtherCode, Variant::OtherNorm, Variant::Format3] {
        let want = variant.message();
        for (caller, message) in refusals(variant) {
            if !message.contains(&want) {
                failures.push(format!(
                    "{} / {caller}:\n  want {want}\n  got  {message}",
                    variant.label()
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// TE-1 (Q29): an editorial approve that refreshes no entry passes over a
/// format-1 lock, the lock untouched: `LAW-STEP` holds no entry.
#[test]
fn editorial_approve_refreshing_nothing_passes_over_format_1() {
    let repo = repo_with("lf-editorial-nothing", Variant::Format1);
    let lock = repo.lock_text();
    let id = propose_edit(
        &repo,
        "LAW-STEP",
        ("Each step adds one.", "Each step adds exactly one."),
    );
    let asked = approve_at(&at(&repo), &id, true, true);
    asked.applied("an editorial approve refreshing nothing");
    assert_eq!(repo.lock_text(), lock, "the format-1 lock untouched");
    let head = repo.head();
    assert_eq!(repo.changed(&head), [COUNTER_MD], "the text alone");
}

// ----------------------------------------------- the spec side's readers

/// TE-1 (Q10): `spec bump` reads a format-1 lock: `LAW-BOUND` and
/// `PIECE-COUNTER` changed without a raise give the rows they give over
/// the format-2 golden (both hold the same four entries of each), exit
/// alike; never a cannot-check.
#[test]
fn bump_reads_format_1() {
    let edit = |repo: &LockRepo| {
        repo.replace(
            COUNTER_MD,
            rev_rule_common::BOUND_EDIT.0,
            rev_rule_common::BOUND_EDIT.1,
        );
        repo.replace(COUNTER_MD, MEANING.0, MEANING.1);
    };
    let two = golden("lf-bump-2");
    edit(&two);
    let one = repo_with("lf-bump-1", Variant::Format1);
    edit(&one);
    let over_two = rev_rule_common::bump(&two, &[]);
    let over_one = rev_rule_common::bump(&one, &[]);
    assert_eq!(over_one.code(), over_two.code(), "{}", over_one.show());
    assert_eq!(
        over_one.row_lines(),
        over_two.row_lines(),
        "{}",
        over_one.show()
    );
    assert_eq!(
        over_one.needs(),
        [
            ("PIECE-COUNTER".to_owned(), "required".to_owned()),
            ("LAW-BOUND".to_owned(), "required".to_owned())
        ],
        "{}",
        over_one.show()
    );
}

/// TE-1 (Q10): the rule through `spec check --staged` reads a format-1
/// lock in the index: a staged lock of format 1 (and `HEAD`'s) with
/// `LAW-BOUND` changed and staged without a raise: the finding, as over
/// the format-2 golden; never a cannot-check.
#[test]
fn check_staged_reads_a_staged_format_1_lock() {
    let two = golden("lf-check-2");
    let one = golden("lf-check-1");
    one.write(LOCK, golden_format_1_text());
    for repo in [&two, &one] {
        repo.replace(
            COUNTER_MD,
            rev_rule_common::BOUND_EDIT.0,
            rev_rule_common::BOUND_EDIT.1,
        );
        rev_rule_common::stage_all(repo);
    }
    let over_two = rev_rule_common::staged(&two);
    let over_one = rev_rule_common::staged(&one);
    assert_eq!(over_one.subjects(), ["LAW-BOUND"], "{}", over_one.show());
    assert_eq!(over_one.subjects(), over_two.subjects());
    assert_eq!(over_one.code(), over_two.code(), "{}", over_one.show());
    assert_eq!(over_one.finding("LAW-BOUND"), over_two.finding("LAW-BOUND"));
}

/// TE-1 (Q10): `spec review`'s vouching count reads a format-1 lock:
/// `PIECE-COUNTER`'s update counts its four entries, as over the format-2
/// golden.
#[test]
fn review_counts_format_1_entries() {
    let mut counts = Vec::new();
    for (label, variant) in [
        ("lf-review-2", None),
        ("lf-review-1", Some(Variant::Format1)),
    ] {
        let repo = match variant {
            Some(variant) => repo_with(label, variant),
            None => golden(label),
        };
        let id = propose_edit(&repo, PIECE, MEANING);
        let (text, value) = review_at(&at(&repo), &id);
        assert!(
            text.contains("\nrevisions: PIECE-COUNTER 3 -> 4 (4 lock entries)\n"),
            "{label}: {text}"
        );
        counts.push(value["revisions"].clone());
    }
    assert_eq!(counts[0], counts[1]);
    assert_eq!(
        counts[1],
        json!([rev_common::revision_entry(PIECE, 3, 4, Some(4))])
    );
}
