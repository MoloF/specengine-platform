//! docs/features/rev-rule.md RD-1: the rule `rev-not-raised` (its
//! `docs/features/rev-rule.md` "Data" and "Rules and edge cases"), judged
//! end to end: `spec check --staged` over scratch repositories of
//! `fixtures/code-a` with its golden lock `fixtures/spec-lock/code-a.lock`
//! committed (`verify_common::golden`), the edits staged. Every mutation
//! RD-1 names turns its test red here as in the pure function: the rule is
//! read through the binary, never a mock.
//!
//! The rule's pure half is core's `check::rev_rule` (`changed_nodes`,
//! `rule_lock`, `judge`); `model_and_core_sources_do_no_file_io` (eval
//! `build_graph.rs`) scans it with every model and core source. The later
//! of 4b-2 and 8b writes `tested_block_vouches_like_an_entry`.

#![cfg(unix)]

mod common;
mod lock_common;
mod rev_rule_common;
mod tested_common;
mod verify_common;

use rev_rule_common::{
    BOUND, BOUND_EDIT, BOUND_LINE, CAP_MD, CODE, COUNTER_MD, GOAL, GOAL_EDIT, GOAL_LINE, GOAL_MD,
    LIMITS_MD, LOCK, MAX_REV, PASS, PASS_EDIT, PASS_LINE, PIECE, PIECE_EDIT, PIECE_LINE, REGEN,
    REGEN_EDIT, STAMINA_DOC, STAMINA_EDIT, STAMINA_MD, STEP_EDIT, assert_finding, assert_no_raise,
    bound_at_max, bump, bump_line, bump_line_alone, changed, check_mode, close_goal, close_stamina,
    entries_of, fake_hash, lock_hash, message, no_raise_shape, refresh, stage_all, staged,
    unwritable_raises, with_field, with_spec_hash,
};
use serde_json::json;
use verify_common::{ecs, golden, golden_format_1};

/// No finding at all: exit 0, no rule finding.
fn assert_silent(check: &rev_rule_common::Check, context: &str) {
    assert_eq!(
        check.subjects(),
        Vec::<String>::new(),
        "{context}:\n{}",
        check.show()
    );
    assert_eq!(check.code(), 0, "{context}:\n{}", check.show());
}

// ------------------------------------------------------------------ RD-1

/// RD-1: nodes no `HEAD` lock entry names (`LAW-STEP`, `LAW-SOURCE`,
/// `PIECE-LIMITS`) changed without a raise: no finding, exit 0; the same
/// tree with the locked `LAW-BOUND` changed too: that node alone.
/// M: every ID judged.
#[test]
fn unlocked_node_change_is_no_finding() {
    let repo = golden("rr1-unlocked");
    repo.replace(COUNTER_MD, STEP_EDIT.0, STEP_EDIT.1);
    repo.replace(
        LIMITS_MD,
        "The cap is read from the data file.",
        "The cap is read from its data file.",
    );
    repo.replace(
        LIMITS_MD,
        "Where the cap of PIECE-COUNTER comes from.",
        "Where the cap of PIECE-COUNTER is read from.",
    );
    stage_all(&repo);
    assert_silent(&staged(&repo), "unlocked nodes changed");

    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    stage_all(&repo);
    let check = staged(&repo);
    assert_eq!(check.subjects(), [BOUND], "{}", check.show());
    assert_eq!(check.code(), 1, "{}", check.show());
}

/// RD-1: `LAW-BOUND` changed at rev 2 and each of its four vouching
/// entries given the new `norm_hash` in the staged lock (an editorial
/// refresh by hand): no finding; unrefreshed, the finding.
/// M: the evidence (the checked lock) ignored.
#[test]
fn refreshed_entries_pass_at_the_same_revision() {
    let repo = golden("rr1-refreshed");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    stage_all(&repo);
    let unrefreshed = staged(&repo);
    assert_eq!(unrefreshed.subjects(), [BOUND], "{}", unrefreshed.show());

    let old = lock_hash(&repo.lock_text(), BOUND);
    let new = refresh(&repo, BOUND, None);
    assert_eq!(repo.lock_text().matches(&new).count(), 4, "four entries");
    assert!(!repo.lock_text().contains(&old));
    stage_all(&repo);
    assert_silent(&staged(&repo), "every vouching entry refreshed");
}

/// RD-1: `GOAL-01` (a document, revision 1 unwritten, two vouching
/// entries) changed with only one of its two entries refreshed, either
/// one: the finding, "The rule `rev-not-raised`" in full (its `id:` line,
/// the subject, the message naming both ways out, the inserted `rev: 2`).
/// `LAW-BOUND` with three of four refreshed: the same, its raised heading
/// the fix. M: one of two refreshed passes. `GOAL-01`'s two entries are
/// the format-1 golden's (format 2 locks one: its `[path, path]` binding
/// is `bad_levels`, docs/features/lock-levels.md); the rule reads both.
#[test]
fn partial_refresh_is_a_finding() {
    for only in [0, 1] {
        let repo = golden_format_1(&format!("rr1-partial-{only}"));
        repo.replace(GOAL_MD, GOAL_EDIT.0, GOAL_EDIT.1);
        refresh(&repo, GOAL, Some(only));
        stage_all(&repo);
        let check = staged(&repo);
        assert_eq!(check.subjects(), [GOAL], "entry {only}:\n{}", check.show());
        assert_eq!(check.code(), 1, "{}", check.show());
        assert_finding(&check, GOAL_MD, GOAL_LINE, GOAL, true, 1, 2, "rev: 2");
    }

    let repo = golden("rr1-partial-bound");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    let new = repo.norm_hash(BOUND);
    let mut text = repo.lock_text();
    for only in 0..3 {
        text = with_spec_hash(&text, BOUND, Some(only), &new);
    }
    repo.write(LOCK, text);
    stage_all(&repo);
    let check = staged(&repo);
    assert_eq!(check.subjects(), [BOUND], "{}", check.show());
    assert_finding(
        &check,
        COUNTER_MD,
        BOUND_LINE,
        BOUND,
        false,
        2,
        3,
        "## Bound {#LAW-BOUND rev=3}",
    );
}

/// RD-1: a meaning change with the revision lowered (`LAW-BOUND` `rev=2`
/// -> `rev=1`, `PIECE-COUNTER` `rev: 3` -> `rev: 2`) is no raise: a
/// finding for each, naming `HEAD`'s revision and `HEAD`'s plus one;
/// raised (`rev: 4`), `PIECE-COUNTER` passes. M: a lower revision taken as
/// a raise.
#[test]
fn lowered_revision_is_a_finding() {
    let repo = golden("rr1-lowered");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.replace(COUNTER_MD, "{#LAW-BOUND rev=2}", "{#LAW-BOUND rev=1}");
    repo.replace(COUNTER_MD, PIECE_EDIT.0, PIECE_EDIT.1);
    repo.replace(COUNTER_MD, "\nrev: 3\n", "\nrev: 2\n");
    stage_all(&repo);
    let check = staged(&repo);
    let mut subjects = check.subjects();
    subjects.sort();
    assert_eq!(subjects, [BOUND, PIECE], "{}", check.show());
    assert_eq!(check.code(), 1);
    assert_eq!(
        check.finding(BOUND)["message"].as_str(),
        Some(message(BOUND, false, 2, 3).as_str())
    );
    assert_eq!(
        check.finding(PIECE)["message"].as_str(),
        Some(message(PIECE, true, 3, 4).as_str())
    );
    assert_eq!(check.finding(PIECE)["line"], PIECE_LINE);

    repo.replace(COUNTER_MD, "\nrev: 2\n", "\nrev: 4\n");
    stage_all(&repo);
    let check = staged(&repo);
    assert_eq!(check.subjects(), [BOUND], "{}", check.show());
}

/// RD-1: headings renamed, their IDs and revisions kept (`LAW-BOUND`'s and
/// `PASS-01`'s sections, `GOAL-01`'s and `PIECE-COUNTER`'s first H1, all
/// locked): the meaning hash never covers a heading, so no finding, and
/// `spec bump --staged` lists no node. M: the heading hashed.
#[test]
fn heading_rename_is_no_finding() {
    let repo = golden("rr1-heading");
    repo.replace(
        COUNTER_MD,
        "## Bound {#LAW-BOUND rev=2}",
        "## Upper bound {#LAW-BOUND rev=2}",
    );
    repo.replace(COUNTER_MD, "\n# Counter\n", "\n# The counter\n");
    repo.replace(
        CAP_MD,
        "### Stepping past the cap keeps the cap {#PASS-01}",
        "### Stepping beyond the cap keeps the cap {#PASS-01}",
    );
    repo.replace(
        GOAL_MD,
        "# The count stops at the cap",
        "# The count halts at the cap",
    );
    stage_all(&repo);
    assert_silent(&staged(&repo), "headings renamed");
    let rows = bump(&repo, &["--staged"]);
    assert_eq!(
        rows.json,
        json!({"rows": []}),
        "a renamed heading changes no node: {}",
        rows.show()
    );
}

/// RD-1: `LAW-BOUND`'s section moved, unchanged, from `counter.md` to the
/// end of `limits.md`: judged by its canonical ID, so no finding; moved
/// and changed, the finding at its new place (`limits.md:20`).
/// M: nodes matched by path.
#[test]
fn moved_section_is_no_finding() {
    let section = "## Bound {#LAW-BOUND rev=2}\n\nThe count never exceeds the cap.\n\n";
    let repo = golden("rr1-moved");
    repo.replace(COUNTER_MD, section, "");
    repo.append(LIMITS_MD, &format!("\n{}", section.trim_end_matches('\n')));
    repo.append(LIMITS_MD, "\n");
    assert!(
        repo.read(LIMITS_MD)
            .ends_with("\n\n## Bound {#LAW-BOUND rev=2}\n\nThe count never exceeds the cap.\n")
    );
    stage_all(&repo);
    assert_silent(&staged(&repo), "a section moved unchanged");

    repo.replace(LIMITS_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    stage_all(&repo);
    let check = staged(&repo);
    assert_eq!(check.subjects(), [BOUND], "{}", check.show());
    assert_finding(
        &check,
        LIMITS_MD,
        20,
        BOUND,
        false,
        2,
        3,
        "## Bound {#LAW-BOUND rev=3}",
    );
}

/// RD-1: `HEAD`'s lock holds `LAW-BOUND`'s entries at rev 2 with another
/// `spec_hash` (an earlier unreviewed change), or `GOAL-01`'s at another
/// revision: none vouches, the node is out of scope, its change no
/// finding. One of the four left vouching: in scope, the finding.
/// M: a non-vouching entry taken as vouching.
#[test]
fn pre_existing_lock_drift_is_out_of_scope() {
    let repo = golden("rr1-drift-hash");
    let tampered = with_spec_hash(&repo.lock_text(), BOUND, None, &fake_hash());
    repo.write(LOCK, tampered);
    repo.commit_all("an earlier unreviewed change left the entries");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    stage_all(&repo);
    assert_silent(&staged(&repo), "no entry vouches (spec_hash)");

    let repo = golden("rr1-drift-rev");
    let tampered = with_field(&repo.lock_text(), GOAL, None, "rev", "2");
    repo.write(LOCK, tampered);
    repo.commit_all("entries at another revision");
    repo.replace(GOAL_MD, GOAL_EDIT.0, GOAL_EDIT.1);
    stage_all(&repo);
    assert_silent(&staged(&repo), "no entry vouches (rev)");

    let repo = golden("rr1-drift-one-left");
    let mut text = repo.lock_text();
    for only in 0..3 {
        text = with_spec_hash(&text, BOUND, Some(only), &fake_hash());
    }
    repo.write(LOCK, text);
    repo.commit_all("three entries no longer vouch");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    stage_all(&repo);
    let check = staged(&repo);
    assert_eq!(check.subjects(), [BOUND], "{}", check.show());
    assert_eq!(check.code(), 1);
}

/// RD-1: the feature-scoped `PASS-01` of `docs/features/cap-check.md` is
/// locked as `cap-check/PASS-01`: its change is judged under that key
/// (the subject, the message), its raised heading the fix; its entry
/// refreshed under that key, no finding. M: a bare-ID key. Only the
/// format-1 golden holds its entry (its one marker `[Sig]` is
/// `bad_levels` under format 2: docs/features/lock-levels.md
/// "Fixtures and the golden"); the rule reads format 1 (Q10).
#[test]
fn feature_scoped_node_is_judged_by_its_lock_key() {
    let repo = golden_format_1("rr1-feature");
    repo.replace(CAP_MD, PASS_EDIT.0, PASS_EDIT.1);
    stage_all(&repo);
    let check = staged(&repo);
    assert_eq!(check.subjects(), [PASS], "{}", check.show());
    assert_finding(
        &check,
        CAP_MD,
        PASS_LINE,
        PASS,
        false,
        1,
        2,
        "### Stepping past the cap keeps the cap {#PASS-01 rev=2}",
    );

    refresh(&repo, PASS, None);
    stage_all(&repo);
    assert_silent(&staged(&repo), "cap-check/PASS-01 refreshed");
}

// ------------------------------------------------ lock-levels.md RD-2

/// docs/features/lock-levels.md RD-2 **M3** (`parse_lock(None)` refusing
/// format 1; 4b-2's carry-over, Q18): `HEAD` holds the format-1 golden
/// (`fixtures/spec-lock/code-a.format-1.lock`), a project not yet
/// rebased; `LAW-BOUND` and the feature-scoped `cap-check/PASS-01` changed
/// and staged without a raise are each found `rev-not-raised` under the
/// entries format 1 holds, exit 1; never a cannot-check. Refreshed by
/// hand in the staged format-1 lock: silent.
#[test]
fn format_1_head_lock_is_read() {
    let repo = golden_format_1("rr-format-1-head");
    assert!(
        repo.lock_text().contains("\nformat = 1\n"),
        "the premise: HEAD's lock is format 1"
    );
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.replace(CAP_MD, PASS_EDIT.0, PASS_EDIT.1);
    stage_all(&repo);
    let check = staged(&repo);
    assert_eq!(
        check.causes(),
        Vec::<String>::new(),
        "no cannot-check:\n{}",
        check.show()
    );
    assert_eq!(check.subjects(), [PASS, BOUND], "{}", check.show());
    assert_eq!(check.code(), 1, "{}", check.show());
    assert_finding(
        &check,
        COUNTER_MD,
        BOUND_LINE,
        BOUND,
        false,
        2,
        3,
        "## Bound {#LAW-BOUND rev=3}",
    );
    assert_finding(
        &check,
        CAP_MD,
        PASS_LINE,
        PASS,
        false,
        1,
        2,
        "### Stepping past the cap keeps the cap {#PASS-01 rev=2}",
    );

    refresh(&repo, BOUND, None);
    refresh(&repo, PASS, None);
    stage_all(&repo);
    assert_silent(&staged(&repo), "format-1 entries refreshed");
}

// --------------------------------------------- no raise (iteration 2, W14)

/// W14 (iteration 2, "Raising"): `GOAL-01` made a document of the closed
/// class `decision`, whose contract admits no `rev` (an apply refuses its
/// raise, `RaiseRefusal::ClosedClass`), changed without a raise under its
/// two vouching entries: the finding at its `id:` line with no `fix`, its
/// message naming only the `--editorial` way out (neither "raise it" nor
/// "the largest revision"), exit 1 under the default `enforce`; alike
/// under `--staged` and `--changed`, text and JSON. `LAW-BOUND` (a section
/// of an open class's document) changed with it keeps its raise.
/// M: the closed class not consulted (a raise and a `fix` advised).
#[test]
fn closed_class_document_finding_names_only_editorial() {
    let repo = golden("w14-closed");
    close_goal(&repo);
    assert_silent(&staged(&repo), "GOAL-01 a closed decision, committed");
    repo.replace(GOAL_MD, GOAL_EDIT.0, GOAL_EDIT.1);
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    stage_all(&repo);
    for (flag, check) in [("--staged", staged(&repo)), ("--changed", changed(&repo))] {
        let mut subjects = check.subjects();
        subjects.sort();
        assert_eq!(subjects, [GOAL, BOUND], "{flag}:\n{}", check.show());
        assert_eq!(check.code(), 1, "{flag}:\n{}", check.show());
        assert_no_raise(&check, GOAL_MD, Some(GOAL_LINE), GOAL, 1);
        assert_finding(
            &check,
            COUNTER_MD,
            BOUND_LINE,
            BOUND,
            false,
            2,
            3,
            "## Bound {#LAW-BOUND rev=3}",
        );
    }
}

/// W14 on the second genre (verify-ecs, an ECS game crate): `stamina.md`
/// made a closed `decision` (`close_stamina`). `PERK-STAMINA` changed:
/// the finding with no `fix`, only `--editorial`; `PERK-REGEN`, a section
/// of that same document (its `rev=N` is a heading attribute, no
/// front-matter key), changed too: its raise kept, `fix` its raised
/// heading, both ways out. `spec bump --staged`: `PERK-STAMINA` `rev 1`
/// alone and `-`, `to` and `write` null; `PERK-REGEN` `rev 1 -> 2` and its
/// line. M: the closed class judged for every node of the document (the
/// section refused its raise).
#[test]
fn closed_class_section_keeps_its_raise() {
    let repo = ecs("w14-closed-ecs");
    close_stamina(&repo);
    assert_silent(&staged(&repo), "stamina.md a closed decision, committed");
    repo.replace(STAMINA_MD, STAMINA_EDIT.0, STAMINA_EDIT.1);
    repo.replace(STAMINA_MD, REGEN_EDIT.0, REGEN_EDIT.1);
    stage_all(&repo);
    let check = staged(&repo);
    let mut subjects = check.subjects();
    subjects.sort();
    assert_eq!(subjects, [REGEN, STAMINA_DOC], "{}", check.show());
    assert_eq!(check.code(), 1, "{}", check.show());
    assert_no_raise(&check, STAMINA_MD, Some(3), STAMINA_DOC, 1);
    let regen_heading = "## Regen {#PERK-REGEN rev=2}";
    assert_finding(&check, STAMINA_MD, 12, REGEN, false, 1, 2, regen_heading);

    let run = bump(&repo, &["--staged"]);
    assert_eq!(run.code(), 0, "{}", run.show());
    let lock = repo.lock_text();
    let (doc_entries, regen_entries) = (entries_of(&lock, STAMINA_DOC), entries_of(&lock, REGEN));
    assert!(doc_entries > 0 && regen_entries > 0, "{lock}");
    assert_eq!(
        run.row_lines(),
        [
            bump_line_alone("required", STAMINA_DOC, STAMINA_MD, 3, 1, doc_entries),
            bump_line(
                "required",
                REGEN,
                STAMINA_MD,
                12,
                1,
                2,
                regen_entries,
                regen_heading
            ),
        ],
        "{}",
        run.show()
    );
    assert_eq!(
        run.json["rows"],
        json!([
            {"need": "required", "id": STAMINA_DOC, "path": STAMINA_MD, "line": 3, "rev": 1,
             "to": null, "lock_entries": doc_entries, "write": null},
            {"need": "required", "id": REGEN, "path": STAMINA_MD, "line": 12, "rev": 1,
             "to": 2, "lock_entries": regen_entries, "write": regen_heading},
        ]),
        "{}",
        run.show()
    );
}

/// Iteration 2 ("Raising": "a raised line that cannot be written: no
/// `fix`, only the `--editorial` way out"): `PIECE-COUNTER`'s `rev: +3`
/// (read as 3, not plain digits) and `GOAL-01`'s flow-style front-matter
/// (no `id:` line to put `rev:` after), each committed under its vouching
/// entries (`unwritable_raises`), changed without a raise: a finding for
/// each with no `fix` and the no-raise message, one message for both
/// their ID and revision aside; `LAW-BOUND` changed with them keeps its
/// raise. The flow mapping hides its keys from the key-presence rule, so
/// `HEAD` holds `key-missing` errors: the mode is `enforce-introduced`,
/// committed, where they never block and the rule's findings (always
/// introduced) do. M: the raise advised whatever `raised_line` gives
/// ("raise it (rev: N)" on a finding without a `fix`).
#[test]
fn unwritable_raise_finding_names_only_editorial() {
    let repo = golden("w14-unwritable");
    unwritable_raises(&repo);
    check_mode(&repo, "enforce-introduced");
    repo.commit_all("enforce-introduced");
    assert_silent(&staged(&repo), "the unwritable revisions committed");
    repo.replace(COUNTER_MD, PIECE_EDIT.0, PIECE_EDIT.1);
    repo.replace(GOAL_MD, GOAL_EDIT.0, GOAL_EDIT.1);
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    stage_all(&repo);
    let check = staged(&repo);
    let mut subjects = check.subjects();
    subjects.sort();
    assert_eq!(subjects, [GOAL, BOUND, PIECE], "{}", check.show());
    assert_eq!(check.code(), 1, "{}", check.show());
    let piece = assert_no_raise(&check, COUNTER_MD, Some(PIECE_LINE), PIECE, 3);
    // A flow mapping: the `id` key's line (2), else the file's (1).
    let goal = assert_no_raise(&check, GOAL_MD, None, GOAL, 1);
    let goal_line = check.finding(GOAL)["line"].as_u64();
    assert!(matches!(goal_line, Some(1 | 2)), "{}", check.show());
    assert_eq!(
        no_raise_shape(&piece, PIECE, 3),
        no_raise_shape(&goal, GOAL, 1)
    );
    assert_finding(
        &check,
        COUNTER_MD,
        BOUND_LINE,
        BOUND,
        false,
        2,
        3,
        "## Bound {#LAW-BOUND rev=3}",
    );
}

/// "Raising": `LAW-BOUND` at `REV_MAX` (`bound_at_max`) changed without a
/// raise: the finding keeps its exact text, `… rev 999999999, the largest
/// revision, …` and only `--editorial`, no `fix`, exit 1, its text line
/// printed. M: the largest revision's words dropped (the plain no-raise
/// message given at `REV_MAX` too).
#[test]
fn rev_max_finding_keeps_the_largest_revision_text() {
    let repo = golden("w14-max");
    bound_at_max(&repo);
    assert_silent(&staged(&repo), "LAW-BOUND at REV_MAX, committed");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    stage_all(&repo);
    let check = staged(&repo);
    assert_eq!(check.subjects(), [BOUND], "{}", check.show());
    assert_eq!(check.code(), 1, "{}", check.show());
    let wanted = format!(
        "`{BOUND}` changed at rev {MAX_REV}, the largest revision, which spec.lock vouches for: \
         have the owner approve it through the queue with --editorial"
    );
    let finding = check.finding(BOUND);
    assert_eq!(
        finding["message"].as_str(),
        Some(wanted.as_str()),
        "{finding}"
    );
    assert_eq!(finding["line"], BOUND_LINE, "{finding}");
    assert!(finding.get("fix").is_none(), "{finding}");
    let line = format!("error  {COUNTER_MD}:{BOUND_LINE}: {CODE}: {wanted}");
    assert!(
        check.lines().contains(&line.as_str()),
        "{line:?} not printed:\n{}",
        check.show()
    );
}

// ------------------------------------------- tested-evidence (8b-1) RD-7

/// RD-7 **tested_block_vouches_like_an_entry** (4b-2's rule, Q-E; M: the
/// rule ignores a block): LAW-STEP holds no entry in the golden; a block
/// vouching for its text (revision 1, its `norm_hash`) committed makes its
/// staged change without a raise a `rev-not-raised` finding, exit 1, as an
/// entry would; a block at another `spec_hash` vouches for nothing.
#[test]
fn tested_block_vouches_like_an_entry() {
    for (label, vouching) in [("rr-tested-vouching", true), ("rr-tested-other", false)] {
        let repo = golden(label);
        let spec_hash = if vouching {
            repo.norm_hash("LAW-STEP")
        } else {
            fake_hash()
        };
        let text = repo.lock_text();
        repo.write(
            LOCK,
            tested_common::with_blocks_added(
                &text,
                &[tested_common::block_at(
                    "LAW-STEP",
                    1,
                    &spec_hash,
                    '7',
                    &repo.head(),
                )],
            ),
        );
        repo.commit_all("LAW-STEP's evidence");
        repo.replace(COUNTER_MD, STEP_EDIT.0, STEP_EDIT.1);
        stage_all(&repo);
        let check = staged(&repo);
        if vouching {
            assert_eq!(check.subjects(), ["LAW-STEP"], "{}", check.show());
            assert_eq!(check.code(), 1, "{}", check.show());
        } else {
            assert_silent(&check, "a block vouching for another text");
        }
    }
}
