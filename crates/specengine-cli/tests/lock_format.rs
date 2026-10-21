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
//!
//! docs/features/tested-evidence.md (8b-1) RD-2 and TE-5: every reader of
//! the lock reads format 3 (the golden's entries plus `[[tested]]` blocks)
//! and answers what it answers over the format-2 golden: each reader runs
//! over two repositories alike but for the blocks, its answer compared
//! with the commits' OIDs masked (`every_lock_reader_reads_format_3`);
//! format 4 is newer than this build's 3 (`format_4_is_newer_than_this_build`).

#![cfg(unix)]

mod common;
mod drift_common;
mod lock_common;
mod rev_common;
mod rev_rule_common;
mod tested_common;
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
            bindings: false,
            git: repo.git_env(&repo.root),
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
/// `norm_recipe`, `format = 3` without a block, `format = 4`
/// (docs/features/tested-evidence.md **Reader**; format 1:
/// [`format_1_text_is_the_same_everywhere`]): exit 2 with the text of
/// Q27's first failing check, nothing changed.
#[test]
fn every_refusing_caller_refuses_every_variant() {
    let mut failures = Vec::new();
    for variant in [
        Variant::OtherCode,
        Variant::OtherNorm,
        Variant::Format3,
        Variant::Format4,
    ] {
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

// ------------------------------------------- tested-evidence (8b-1) RD-2

/// The golden's format-3 twin: its entries, then blocks that vouch for
/// nothing (made-up `spec_hash` and `bound_hash`) of PIECE-COUNTER (at
/// its revision), of LAW-STEP (at another) and of a gone node, so no
/// count, vouching or verdict can change by them.
fn format_3_text(golden_text: &str, commit: &str) -> String {
    tested_common::with_blocks_added(
        golden_text,
        &[
            tested_common::made_up_block(PIECE, 3, '7', commit),
            tested_common::made_up_block("LAW-STEP", 9, '8', commit),
            tested_common::made_up_block("GONE-77", 1, '9', commit),
        ],
    )
}

/// `answer` with `repo`'s commit OIDs (every ref's history, newest first)
/// masked by their place, the root's path and the scratch's masked.
fn masked(repo: &LockRepo, answer: &str) -> String {
    let mut out = answer.replace(&repo.root.display().to_string(), "<root>");
    let oids = repo.git_text(&["rev-list", "--all", "--topo-order"]);
    for (at, oid) in oids.lines().enumerate() {
        out = out.replace(oid, &format!("<commit {at}>"));
    }
    out
}

/// A plain-verify answer: the exit, the text without its `tested | `
/// lines and the JSON without its `tested` key (8b-1's own output).
fn verify_answer(repo: &LockRepo, extra: &[&str]) -> String {
    let mut args = vec!["verify"];
    args.extend_from_slice(extra);
    let text = repo.spec_piped(&repo.root, &args);
    args.push("--json");
    let json = repo.spec_piped(&repo.root, &args);
    format!(
        "exit {} {}\n{}\n{}",
        text.code,
        json.code,
        tested_common::without_tested_lines(&text.stdout),
        tested_common::without_tested_key(&json.json())
    )
}

/// A binary read's answer: the exit, text and JSON stdout.
fn read_answer(repo: &LockRepo, args: &[&str]) -> String {
    let text = repo.spec_piped(&repo.root, args);
    let mut json_args = args.to_vec();
    json_args.push("--json");
    let json = repo.spec_piped(&repo.root, &json_args);
    format!(
        "exit {}\n{}\nexit {}\n{}",
        text.code, text.stdout, json.code, json.stdout
    )
}

/// One reader of RD-2's table: what it needs first, and its answer.
struct Reader {
    name: &'static str,
    prepare: fn(&LockRepo),
    run: fn(&LockRepo) -> String,
}

fn nothing(_: &LockRepo) {}

fn task_on_bound(repo: &LockRepo) {
    let id = verify_common::new_task(repo, &["LAW-BOUND"]);
    assert_eq!(id, "T-0001");
}

fn cap_edited(repo: &LockRepo) {
    drift_common::cap_changed(repo);
}

fn cap_staged(repo: &LockRepo) {
    drift_common::cap_changed(repo);
    repo.git(&["add", "-A"]);
}

fn cap_committed(repo: &LockRepo) {
    drift_common::cap_changed(repo);
    repo.commit_all("the cap");
}

fn bound_edited(repo: &LockRepo) {
    repo.replace(
        COUNTER_MD,
        rev_rule_common::BOUND_EDIT.0,
        rev_rule_common::BOUND_EDIT.1,
    );
}

fn bound_staged(repo: &LockRepo) {
    bound_edited(repo);
    rev_rule_common::stage_all(repo);
}

/// A merge of two branches whose `verified_at` of the first entry differs
/// (`2026-10-09` theirs, `2026-10-10` ours): `spec.lock` conflicts, the
/// blocks the same on both sides.
fn lock_conflict(repo: &LockRepo) {
    let text = repo.lock_text();
    let day = "verified_at = \"2026-10-08\"";
    assert!(text.contains(day), "the golden's day");
    repo.git(&["switch", "-q", "-c", "theirs"]);
    repo.write(LOCK, text.replacen(day, "verified_at = \"2026-10-09\"", 1));
    repo.commit_all("their day");
    repo.git(&["switch", "-q", "main"]);
    repo.write(LOCK, text.replacen(day, "verified_at = \"2026-10-10\"", 1));
    repo.commit_all("our day");
    let merge = repo
        .git
        .git_output(&repo.root, &["merge", "-q", "--no-edit", "theirs"], &[]);
    assert!(!merge.status.success(), "spec.lock conflicts");
}

const READERS: [Reader; 17] = [
    Reader {
        name: "spec verify",
        prepare: nothing,
        run: |repo| verify_answer(repo, &[]),
    },
    Reader {
        name: "spec verify --node",
        prepare: nothing,
        run: |repo| verify_answer(repo, &["--node", PIECE]),
    },
    Reader {
        name: "spec verify --task",
        prepare: task_on_bound,
        run: |repo| verify_answer(repo, &["--task", "T-0001"]),
    },
    Reader {
        name: "spec verify --changed (HEAD's lock the base)",
        prepare: cap_edited,
        run: |repo| verify_answer(repo, &["--changed"]),
    },
    Reader {
        name: "spec verify --staged (the index's lock, HEAD's the base)",
        prepare: cap_staged,
        run: |repo| verify_answer(repo, &["--staged"]),
    },
    Reader {
        name: "spec lock accept",
        prepare: nothing,
        run: |repo| {
            let attempt = repo.accept("LAW-BOUND", false);
            let outcome = attempt
                .outcome
                .as_ref()
                .unwrap_or_else(|error| panic!("accept refused: {error}"));
            let questions: Vec<&str> = attempt
                .prompts
                .iter()
                .map(|prompt| prompt.question.as_str())
                .collect();
            format!(
                "{:?} {}\n{}\n{questions:?}",
                outcome.exit(),
                outcome.unchanged,
                attempt.rows()
            )
        },
    },
    Reader {
        name: "spec lock regen",
        prepare: lock_conflict,
        run: |repo| {
            let attempt = repo.regen(true);
            if let Err(error) = &attempt.outcome {
                panic!("regen refused: {error}");
            }
            let all = attempt.rows();
            let rows: Vec<&str> = all
                .lines()
                .filter(|row| !row.ends_with(" | tested"))
                .collect();
            format!(
                "{}\n{}",
                rows.join("\n"),
                tested_common::without_blocks(&repo.lock_text())
            )
        },
    },
    Reader {
        name: "spec lock rebase (nothing to change)",
        prepare: nothing,
        run: |repo| {
            let attempt = repo.rebase(true);
            attempt.unchanged("rebase");
            "nothing to change".to_owned()
        },
    },
    Reader {
        name: "spec bump",
        prepare: bound_edited,
        run: |repo| {
            let bump = rev_rule_common::bump(repo, &[]);
            format!(
                "exit {}\n{}\n{}",
                bump.code(),
                bump.text.stdout,
                bump.json_run.stdout
            )
        },
    },
    Reader {
        name: "the rule (spec check --staged)",
        prepare: bound_staged,
        run: |repo| {
            let check = rev_rule_common::staged(repo);
            format!(
                "exit {}\n{:?}\n{}",
                check.code(),
                check.subjects(),
                check.finding("LAW-BOUND")
            )
        },
    },
    Reader {
        name: "the review's count",
        prepare: nothing,
        run: |repo| {
            let id = propose_edit(repo, PIECE, MEANING);
            let (text, value) = review_at(&at(repo), &id);
            format!("{text}\n{}", value["revisions"])
        },
    },
    Reader {
        name: "the editorial approve",
        prepare: nothing,
        run: |repo| {
            let id = propose_edit(repo, PIECE, MEANING);
            let asked = approve_at(&at(repo), &id, true, true);
            asked.applied("an editorial approve");
            let head = repo.head();
            format!(
                "{}\n{}",
                repo.message(&head),
                tested_common::without_blocks(&repo.lock_text())
            )
        },
    },
    Reader {
        name: "spec propose drift",
        prepare: cap_committed,
        run: |repo| {
            let raised = drift_common::raised(repo, &[]);
            format!("{}\n{:?}", raised.text(), raised.created_nodes())
        },
    },
    Reader {
        name: "spec show --bindings",
        prepare: nothing,
        run: |repo| read_answer(repo, &["show", "--bindings", PIECE]),
    },
    Reader {
        name: "spec bundle",
        prepare: nothing,
        run: |repo| read_answer(repo, &["bundle", PIECE]),
    },
    Reader {
        name: "spec task show",
        prepare: task_on_bound,
        run: |repo| read_answer(repo, &["task", "show", "T-0001"]),
    },
    Reader {
        name: "spec impact",
        prepare: nothing,
        run: |repo| read_answer(repo, &["impact", PIECE]),
    },
];

/// RD-2 **M2** (format 3 refused under `Some`) and TE-5 (M: a read
/// degrading to `sync -`): every reader of the lock reads format 3 and
/// answers over it what it answers over the format-2 golden, the commits
/// masked: verify plain, `--node`, `--task`, the `--changed` and
/// `--staged` base; accept (the question and rows); regen (its entries'
/// rows and lock, the blocks aside); rebase `nothing to change`; `spec
/// bump`, the rule, the review's count, the editorial approve (its commit
/// and entries); `spec propose drift`; `show --bindings`, `bundle`, `task
/// show`, `impact` byte for byte (never cause (6), never `sync -`).
#[test]
fn every_lock_reader_reads_format_3() {
    let mut failures = Vec::new();
    for (index, reader) in READERS.iter().enumerate() {
        let two = golden(&format!("lf3-{index}-two"));
        let three = golden(&format!("lf3-{index}-three"));
        let text = three.lock_text();
        let head = three.head();
        three.write(LOCK, format_3_text(&text, &head));
        three.commit_all("the blocks");
        two.write("notes.txt", "the blocks' place\n");
        two.commit_all("the blocks");
        three.write("notes.txt", "the blocks' place\n");
        three.commit_all("a note");
        two.write("notes2.txt", "a note\n");
        two.commit_all("a note");
        (reader.prepare)(&two);
        (reader.prepare)(&three);
        let over_two = masked(&two, &(reader.run)(&two));
        let over_three = masked(&three, &(reader.run)(&three));
        if over_two != over_three {
            failures.push(format!(
                "{}:\n--- over format 2\n{over_two}\n--- over format 3\n{over_three}",
                reader.name
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// RD-2 (M: format 4 not refused): a lock claiming `format = 4` is newer
/// than this build's 3 for every reader, the spec side's too: verify's
/// first cause, accept's and the review's refusal text, nothing changed.
#[test]
fn format_4_is_newer_than_this_build() {
    let want = Variant::Format4.message();
    assert_eq!(want, "spec.lock:2: format 4 is newer than this build's 3");
    let repo = repo_with("lf-format-4", Variant::Format4);
    let before = repo.state();
    assert_eq!(verify_plain(&repo), want, "verify");
    assert!(accept(&repo).contains(&want), "accept");
    assert_eq!(repo.state(), before, "nothing changed");
    bound_edited(&repo);
    let bump = rev_rule_common::bump(&repo, &[]);
    assert!(
        bump.text.stdout.contains(&want) || bump.text.stderr.contains(&want),
        "spec bump names it:\n{}",
        bump.show()
    );
}
