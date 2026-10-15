//! docs/features/rev-discipline.md RD-1 (the raise at apply), RD-3 (a
//! proposal changing a revision refused at creation) and RD-6 (the review
//! document's `revisions`), through the library over a scratch repository
//! of `fixtures/spec-a` (a game's design spec): the proposal raised in the
//! linked worktree on `t1`, approved from the main worktree, the owner's
//! question recorded (`common::proposal::Pair`).
//!
//! The expected files, questions and trailers are built from the spec's
//! "Data" ("Raising at apply", "Creation", "Review document, stage,
//! queue"), never from the command's output. Each test names the mutation
//! that must turn it red.

#![cfg(unix)]

mod common;
mod rev_common;

use common::proposal::{Pair, edit};
use common::{read, read_text, replace, write};
use rev_common::{
    At, REVISION_ENTRY_KEYS, after_base, after_step_6, approve_at, assert_entry_keys,
    notes_starting, printed, propose_create_text, propose_text, proposed, refused_entry, review_at,
    review_printed, revision_entry,
};
use serde_json::{Value, json};

const SPRINT: &str = "docs/spec/movement/sprint.md";
const COST: &str = "RULE-SPRINT-COST";
const EMPTY: &str = "EDGE-SPRINT-EMPTY";
const DOC: &str = "MEC-SPRINT";
const COST_HEADING: &str = "## Cost {#RULE-SPRINT-COST rev=3}";
const EMPTY_HEADING: &str = "### Empty tank {#EDGE-SPRINT-EMPTY .playtest}";
const COST_LINE: (&str, &str) = ("costs 12 units/s", "costs 10 units/s");
const EMPTY_LINE: (&str, &str) = ("the sprint ends;", "the sprint ends at once;");
/// A change of `MEC-SPRINT`'s own text (its intro paragraph).
const INTRO: (&str, &str) = (
    "Hold the sprint key to run;",
    "Hold the sprint key to dash;",
);
/// `RULE-SPRINT-COST` at `REV_MAX` (999 999 999).
const MAX_HEADING: &str = "## Cost {#RULE-SPRINT-COST rev=999999999}";
/// Q11's step-6 line for `MEC-SPRINT` once `canon` is closed without `rev`
/// ("Raising at apply").
const CLOSED_REASON: &str = "`MEC-SPRINT`: class `canon` is closed and does not admit `rev`: \
                             admit it in [classes], or approve with --editorial";
/// The review's note per refused raise, before its `refused` (Q15).
const NOT_APPLICABLE: &str = "meaning approve not applicable (step 6): ";

/// The library's view from the main worktree (approve, review).
fn main_at(pair: &Pair) -> At<'_> {
    At::new(&pair.main, &pair.home, pair.git_env(&pair.main))
}

/// The library's view from the linked worktree (propose).
fn linked_at(pair: &Pair) -> At<'_> {
    At::new(&pair.linked, &pair.home, pair.git_env(&pair.linked))
}

/// `from` -> `to` in `t1`'s `relative`, committed on `t1` (a hand edit).
fn commit_edit(pair: &Pair, relative: &str, from: &str, to: &str) {
    replace(&pair.linked, relative, from, to);
    pair.commit_all(&pair.linked, "a hand edit");
}

/// Proposes `target`'s span with each `(from, to)` applied: its ID.
fn propose_edits(pair: &Pair, target: &str, edits: &[(&str, &str)]) -> String {
    let (hash, mut text) = pair.span(&pair.linked, target);
    for (from, to) in edits {
        text = edit(&text, from, to);
    }
    proposed(&linked_at(pair), target, &hash, &text)
}

/// The apply question of `id` on `t1`, `tail` after the preview.
fn question(pair: &Pair, id: &str, tail: &str) -> String {
    format!(
        "apply {id} to {SPRINT} on t1 in {} ({tail})? [y/N]",
        pair.linked.display()
    )
}

/// The message of `t1`'s tip.
fn tip_message(pair: &Pair) -> String {
    pair.git_text(&pair.main, &["log", "-1", "--format=%B", "t1"])
}

/// `relative` in `t1` with each `(from, to)` applied (the expected file).
fn expected(before: &str, edits: &[(&str, &str)]) -> String {
    let mut text = before.to_owned();
    for (from, to) in edits {
        text = edit(&text, from, to);
    }
    text
}

/// `[classes]` appended to `t1`'s config, committed: `canon` closed,
/// admitting every key spec-a's canon documents write but `rev`.
fn close_canon(pair: &Pair) {
    let mut config = read_text(&pair.linked, "specengine.toml");
    config.push_str(
        "\n[classes]\ncanon = { required = [\"class\"], optional = [\"id\", \"tier\", \
         \"parent\", \"owner\", \"reviewed\", \"links\"], closed = true }\n",
    );
    write(&pair.linked, "specengine.toml", config);
    pair.commit_all(&pair.linked, "close canon");
}

// ------------------------------------------------------------------ RD-1

/// RD-1: a meaning approve raises each node whose `norm_hash` the change
/// alters, in file order: a section's readable `rev=3` replaced in place
/// (3 -> 4), an unwritten one ` rev=2` appended before its heading's `}`
/// (1 -> 2); the question names both; one `Revision:` trailer per raised
/// node after `Base-commit:`; the document, unchanged, keeps its.
/// M: the raise dropped (the text written as sent).
#[test]
fn meaning_approve_raises_each_changed_node() {
    let pair = Pair::new("rd1-each", "spec-a");
    let before = read_text(&pair.linked, SPRINT);
    let id = propose_edits(&pair, COST, &[COST_LINE, EMPTY_LINE]);
    let asked = approve_at(&main_at(&pair), &id, false, true);
    asked.applied("a meaning approve");
    assert_eq!(
        asked.question(),
        question(
            &pair,
            &id,
            "applies; raises RULE-SPRINT-COST 3 -> 4, EDGE-SPRINT-EMPTY 1 -> 2"
        )
    );
    let want = expected(
        &before,
        &[
            COST_LINE,
            EMPTY_LINE,
            (COST_HEADING, "## Cost {#RULE-SPRINT-COST rev=4}"),
            (
                EMPTY_HEADING,
                "### Empty tank {#EDGE-SPRINT-EMPTY .playtest rev=2}",
            ),
        ],
    );
    assert_eq!(read_text(&pair.linked, SPRINT), want);
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(
        pair.git
            .git(&pair.main, &["show", &format!("{head}:{SPRINT}")]),
        want.as_bytes(),
        "the committed blob is the raised file"
    );
    assert_eq!(
        after_base(&tip_message(&pair)),
        [
            "Revision: RULE-SPRINT-COST 3 -> 4",
            "Revision: EDGE-SPRINT-EMPTY 1 -> 2"
        ]
    );
    assert!(
        !read_text(&pair.linked, SPRINT).contains("\nrev:"),
        "the document's meaning is unchanged: no rev written"
    );
    assert_eq!(pair.node(&pair.linked, COST).rev, Some(4));
    assert_eq!(pair.node(&pair.linked, EMPTY).rev, Some(2));
}

/// RD-1: `spec approve --editorial` keeps every revision: the file is the
/// text as sent, the question names what stays, the trailers record
/// `Revision: ID N (editorial)`; with no `spec.lock` no `Lock:` trailer.
/// M: `--editorial` raising anyway.
#[test]
fn editorial_approve_keeps_revisions() {
    let pair = Pair::new("rd1-editorial", "spec-a");
    let before = read_text(&pair.linked, SPRINT);
    let id = propose_edits(&pair, COST, &[COST_LINE, EMPTY_LINE]);
    let asked = approve_at(&main_at(&pair), &id, true, true);
    asked.applied("an editorial approve");
    assert_eq!(
        asked.question(),
        question(
            &pair,
            &id,
            "applies, editorial: RULE-SPRINT-COST stays 3, EDGE-SPRINT-EMPTY stays 1"
        )
    );
    assert_eq!(
        read_text(&pair.linked, SPRINT),
        expected(&before, &[COST_LINE, EMPTY_LINE])
    );
    let trailers = after_base(&tip_message(&pair));
    assert_eq!(
        trailers,
        [
            "Revision: RULE-SPRINT-COST 3 (editorial)",
            "Revision: EDGE-SPRINT-EMPTY 1 (editorial)"
        ],
        "no lock: the path alone, no Lock: trailer"
    );
    let head = pair.rev(&pair.linked, "t1");
    assert_eq!(
        pair.git_text(
            &pair.main,
            &["diff-tree", "--no-commit-id", "--name-only", "-r", &head]
        ),
        SPRINT
    );
    assert_eq!(pair.node(&pair.linked, COST).rev, Some(3));
    assert_eq!(pair.node(&pair.linked, EMPTY).rev, None);
}

/// RD-1 (ADR-0040): a whitespace-only change (trailing blanks, a run of
/// empty lines) and a heading-only change (a renamed title) keep every
/// revision: today's question, no `Revision:` trailer, the text as sent.
/// `--editorial` on such a change adds the note "no node's meaning
/// changes". M: the raise keyed on the span hash instead of `norm_hash`.
#[test]
fn whitespace_only_change_keeps_revision() {
    let pair = Pair::new("rd1-whitespace", "spec-a");
    let before = read_text(&pair.linked, SPRINT);
    let edits = [
        ("bounded by A-101.", "bounded by A-101.   "),
        ("\n\n### Empty tank", "\n\n\n\n### Empty tank"),
        ("## Cost {#", "## Sprint cost {#"),
    ];
    let id = propose_edits(&pair, COST, &edits);
    let asked = approve_at(&main_at(&pair), &id, false, true);
    asked.applied("a whitespace-only approve");
    assert_eq!(asked.question(), question(&pair, &id, "applies"));
    assert_eq!(read_text(&pair.linked, SPRINT), expected(&before, &edits));
    assert_eq!(after_base(&tip_message(&pair)), Vec::<String>::new());

    let id = propose_edits(&pair, EMPTY, &[("Q-031.", "Q-031.  ")]);
    let asked = approve_at(&main_at(&pair), &id, true, true);
    let outcome = asked.applied("an editorial approve that changes no meaning");
    assert_eq!(
        asked.question(),
        format!(
            "note: no node's meaning changes; --editorial changes nothing\n{}",
            question(&pair, &id, "applies")
        ),
        "today's question, the note above it"
    );
    let notes: Vec<String> = outcome
        .messages
        .iter()
        .map(|message| message.line())
        .collect();
    assert!(
        notes.contains(&"note: no node's meaning changes; --editorial changes nothing".to_owned()),
        "{notes:?}"
    );
}

/// RD-1: a revision is raised from the file as it is at apply, never
/// lowered: a section raised by hand to `rev=009` after the proposal was
/// raised against `rev=3` goes to 10 (in place, the leading zeros gone),
/// never back to 4. M: the raise computed from the proposal's revision.
#[test]
fn apply_never_lowers_a_revision() {
    let pair = Pair::new("rd1-never-lower", "spec-a");
    // A second paragraph, so the hand raise and the proposal's edit are
    // lines apart (git merge-file conflicts on adjacent lines).
    commit_edit(
        &pair,
        SPRINT,
        "bounded by A-101.\n",
        "bounded by A-101.\n\nThe cost applies each second.\n",
    );
    let id = propose_edits(&pair, COST, &[("each second.", "every second.")]);
    commit_edit(
        &pair,
        SPRINT,
        COST_HEADING,
        "## Cost {#RULE-SPRINT-COST rev=009}",
    );
    let before = read_text(&pair.linked, SPRINT);
    let asked = approve_at(&main_at(&pair), &id, false, true);
    asked.applied("a rebased approve");
    assert_eq!(
        asked.question(),
        question(&pair, &id, "rebases; raises RULE-SPRINT-COST 9 -> 10")
    );
    assert_eq!(
        read_text(&pair.linked, SPRINT),
        expected(
            &before,
            &[
                ("each second.", "every second."),
                (
                    "## Cost {#RULE-SPRINT-COST rev=009}",
                    "## Cost {#RULE-SPRINT-COST rev=10}"
                ),
            ]
        )
    );
    assert_eq!(
        after_base(&tip_message(&pair)),
        ["Revision: RULE-SPRINT-COST 9 -> 10"]
    );
}

/// RD-1: a change to a section's own text raises it alone: its nested ID
/// section, unchanged, keeps its written `rev=5`; a change to the nested
/// section alone raises only it, the parent's `rev` kept.
/// M: the nested section raised with its parent (the span compared).
#[test]
fn unchanged_nested_section_keeps_its_revision() {
    let pair = Pair::new("rd1-nested", "spec-a");
    let nested = "### Empty tank {#EDGE-SPRINT-EMPTY .playtest rev=5}";
    commit_edit(&pair, SPRINT, EMPTY_HEADING, nested);
    let before = read_text(&pair.linked, SPRINT);
    let id = propose_edits(&pair, COST, &[COST_LINE]);
    let asked = approve_at(&main_at(&pair), &id, false, true);
    asked.applied("the parent's change");
    assert_eq!(
        asked.question(),
        question(&pair, &id, "applies; raises RULE-SPRINT-COST 3 -> 4")
    );
    let raised = "## Cost {#RULE-SPRINT-COST rev=4}";
    assert_eq!(
        read_text(&pair.linked, SPRINT),
        expected(&before, &[COST_LINE, (COST_HEADING, raised)])
    );

    let before = read_text(&pair.linked, SPRINT);
    let id = propose_edits(&pair, COST, &[EMPTY_LINE]);
    let asked = approve_at(&main_at(&pair), &id, false, true);
    asked.applied("the nested section's change");
    assert_eq!(
        asked.question(),
        question(&pair, &id, "applies; raises EDGE-SPRINT-EMPTY 5 -> 6")
    );
    assert_eq!(
        read_text(&pair.linked, SPRINT),
        expected(
            &before,
            &[
                EMPTY_LINE,
                (
                    nested,
                    "### Empty tank {#EDGE-SPRINT-EMPTY .playtest rev=6}"
                )
            ]
        )
    );
    assert_eq!(
        after_base(&tip_message(&pair)),
        ["Revision: EDGE-SPRINT-EMPTY 5 -> 6"]
    );
}

/// RD-1: a document's raise: no `rev:` written -> `rev: 2` inserted right
/// after its `id:` line with that line's ending (LF, and CRLF in a CRLF
/// file); a written `rev: 3` elsewhere in the front-matter replaced in
/// place (4). M: `rev:` appended at the front-matter's end, or with LF in
/// a CRLF file.
#[test]
fn document_rev_line_follows_id_with_its_line_ending() {
    let intro = (
        "Hold the sprint key to run;",
        "Hold the sprint key to dash;",
    );
    // LF, no rev written.
    let pair = Pair::new("rd1-doc-lf", "spec-a");
    let before = read_text(&pair.linked, SPRINT);
    let id = propose_edits(&pair, DOC, &[intro]);
    let asked = approve_at(&main_at(&pair), &id, false, true);
    asked.applied("a document's raise");
    assert_eq!(
        asked.question(),
        question(&pair, &id, "applies; raises MEC-SPRINT 1 -> 2")
    );
    assert_eq!(
        read_text(&pair.linked, SPRINT),
        expected(
            &before,
            &[intro, ("id: MEC-SPRINT\n", "id: MEC-SPRINT\nrev: 2\n")]
        )
    );
    assert_eq!(
        after_base(&tip_message(&pair)),
        ["Revision: MEC-SPRINT 1 -> 2"]
    );

    // CRLF, no rev written.
    let pair = Pair::new("rd1-doc-crlf", "spec-a");
    let crlf = read_text(&pair.linked, SPRINT).replace('\n', "\r\n");
    write(&pair.linked, SPRINT, &crlf);
    pair.commit_all(&pair.linked, "CRLF");
    let id = propose_edits(&pair, DOC, &[intro]);
    approve_at(&main_at(&pair), &id, false, true).applied("a CRLF document's raise");
    let after = read(&pair.linked, SPRINT);
    assert_eq!(
        String::from_utf8(after).unwrap(),
        expected(
            &crlf,
            &[
                intro,
                ("id: MEC-SPRINT\r\n", "id: MEC-SPRINT\r\nrev: 2\r\n")
            ]
        )
    );

    // A written `rev: 3`, not right after `id:`: replaced in place.
    let pair = Pair::new("rd1-doc-written", "spec-a");
    commit_edit(&pair, SPRINT, "tier: 2\n", "tier: 2\nrev: 3\n");
    let before = read_text(&pair.linked, SPRINT);
    let id = propose_edits(&pair, DOC, &[intro]);
    approve_at(&main_at(&pair), &id, false, true).applied("a written rev raised");
    assert_eq!(
        read_text(&pair.linked, SPRINT),
        expected(&before, &[intro, ("rev: 3\n", "rev: 4\n")])
    );
}

/// RD-1: a changed node whose current revision is unreadable (`rev=0`, a
/// `bad-rev`) refuses at step 6, exit 1 naming it, before the question;
/// nothing written. M: an unreadable revision raised as 1 -> 2.
#[test]
fn unreadable_current_revision_refuses_at_step_6() {
    let pair = Pair::new("rd1-unreadable", "spec-a");
    commit_edit(
        &pair,
        SPRINT,
        COST_HEADING,
        "## Cost {#RULE-SPRINT-COST rev=0}",
    );
    let id = propose_edits(&pair, COST, &[COST_LINE]);
    let before = pair.state();
    let asked = approve_at(&main_at(&pair), &id, false, true);
    let reason = asked.refused("an unreadable revision");
    assert!(reason.contains("(step 6)"), "{reason}");
    assert!(reason.contains("`RULE-SPRINT-COST`"), "{reason}");
    assert!(reason.contains("unreadable"), "{reason}");
    assert!(asked.questions.is_empty(), "refused before the question");
    assert_eq!(pair.state().linked_files, before.linked_files);
    assert_eq!(pair.proposal(&id).status.as_str(), "open");
    assert_eq!(
        pair.events_of(&id).last().unwrap(),
        &("proposal.apply_failed".to_owned(), Some(6))
    );
}

/// RD-1: a node at `REV_MAX` (999 999 999) cannot be raised: refused at
/// step 6, exit 1 naming it, before the question; `--editorial` keeps it
/// and applies. M: the raise wrapping or writing 1 000 000 000.
#[test]
fn raise_at_max_revision_refuses() {
    let pair = Pair::new("rd1-max", "spec-a");
    let max = "## Cost {#RULE-SPRINT-COST rev=999999999}";
    commit_edit(&pair, SPRINT, COST_HEADING, max);
    let id = propose_edits(&pair, COST, &[COST_LINE]);
    let before = pair.state();
    let asked = approve_at(&main_at(&pair), &id, false, true);
    let reason = asked.refused("a revision at REV_MAX");
    assert!(reason.contains("(step 6)"), "{reason}");
    assert!(reason.contains("`RULE-SPRINT-COST`"), "{reason}");
    assert!(reason.contains("999999999"), "{reason}");
    assert!(asked.questions.is_empty(), "refused before the question");
    assert_eq!(pair.state().linked_files, before.linked_files);

    let file = read_text(&pair.linked, SPRINT);
    let asked = approve_at(&main_at(&pair), &id, true, true);
    asked.applied("--editorial at REV_MAX");
    assert_eq!(
        asked.question(),
        question(
            &pair,
            &id,
            "applies, editorial: RULE-SPRINT-COST stays 999999999"
        )
    );
    assert_eq!(
        read_text(&pair.linked, SPRINT),
        expected(&file, &[COST_LINE])
    );
}

/// RD-1, TE-1's rebase: two proposals raised against `rev=3`; the first
/// applies (3 -> 4), the second rebases over it and raises from the file
/// as read at its apply (4 -> 5), never from its base.
/// M: the raise read from the proposal's base.
#[test]
fn rebased_apply_raises_from_the_current_revision() {
    let pair = Pair::new("rd1-rebase", "spec-a");
    commit_edit(
        &pair,
        SPRINT,
        "bounded by A-101.\n",
        "bounded by A-101.\n\nThe cost applies each second.\n",
    );
    let first = propose_edits(&pair, COST, &[COST_LINE]);
    let second = propose_edits(&pair, COST, &[("each second.", "every second.")]);
    let before = read_text(&pair.linked, SPRINT);
    approve_at(&main_at(&pair), &first, false, true).applied("the first");
    let asked = approve_at(&main_at(&pair), &second, false, true);
    asked.applied("the second, rebased");
    assert_eq!(
        asked.question(),
        question(&pair, &second, "rebases; raises RULE-SPRINT-COST 4 -> 5")
    );
    assert_eq!(
        read_text(&pair.linked, SPRINT),
        expected(
            &before,
            &[
                COST_LINE,
                ("each second.", "every second."),
                (COST_HEADING, "## Cost {#RULE-SPRINT-COST rev=5}"),
            ]
        )
    );
    assert_eq!(
        after_base(&tip_message(&pair)),
        ["Revision: RULE-SPRINT-COST 4 -> 5"]
    );
}

/// RD-1 (Q11): a document's raise into a closed class whose contract
/// lists `rev` in neither `required` nor `optional` refuses at step 6,
/// before the question, exit 1, naming the node, the class and both ways
/// out; a section in that document is raised as ever (its `rev=` is a
/// heading attribute); `--editorial` passes, the document's revision kept.
/// M: the closed-class check dropped (a `rev:` the contract refuses
/// written).
#[test]
fn raise_into_a_closed_contract_refuses_at_step_6() {
    let pair = Pair::new("rd1-closed", "spec-a");
    close_canon(&pair);
    let intro = (
        "Hold the sprint key to run;",
        "Hold the sprint key to dash;",
    );
    let id = propose_edits(&pair, DOC, &[intro]);
    let before = pair.state();
    let asked = approve_at(&main_at(&pair), &id, false, true);
    let reason = asked.refused("a closed class without rev");
    assert!(reason.contains("(step 6)"), "{reason}");
    assert!(
        reason.ends_with(
            "`MEC-SPRINT`: class `canon` is closed and does not admit `rev`: admit it in \
             [classes], or approve with --editorial"
        ),
        "{reason}"
    );
    assert!(asked.questions.is_empty(), "refused before the question");
    assert_eq!(pair.state().linked_files, before.linked_files);

    // A section of that document, the document's own text unchanged:
    // raised as ever (a heading attribute, no front-matter key).
    let section = propose_edits(&pair, COST, &[COST_LINE]);
    let file = read_text(&pair.linked, SPRINT);
    let asked = approve_at(&main_at(&pair), &section, false, true);
    asked.applied("a section of a closed-class document");
    assert_eq!(
        read_text(&pair.linked, SPRINT),
        expected(
            &file,
            &[
                COST_LINE,
                (COST_HEADING, "## Cost {#RULE-SPRINT-COST rev=4}")
            ]
        )
    );
    // `--editorial` passes, the document's revision kept.
    let file = read_text(&pair.linked, SPRINT);
    let asked = approve_at(&main_at(&pair), &id, true, true);
    asked.applied("--editorial into a closed class");
    assert_eq!(
        asked.question(),
        question(&pair, &id, "rebases, editorial: MEC-SPRINT stays 1")
    );
    assert_eq!(read_text(&pair.linked, SPRINT), expected(&file, &[intro]));
    assert_eq!(
        after_base(&tip_message(&pair)),
        ["Revision: MEC-SPRINT 1 (editorial)"]
    );
}

// ------------------------------------------------------------------ RD-3

/// RD-3 (Q7): a proposal whose text changes a revision is refused at
/// creation, exit 1, nothing stored: a section's `rev=3` -> `rev=4`, a
/// nested section's written revision, a document's `rev:`.
/// M: a revision change accepted.
#[test]
fn proposal_changing_a_revision_is_refused() {
    let pair = Pair::new("rd3-changed", "spec-a");
    let at = linked_at(&pair);
    let (hash, text) = pair.span(&pair.linked, COST);
    let raised = edit(&text, "rev=3}", "rev=4}");
    let outcome = propose_text(&at, COST, &hash, &edit(&raised, COST_LINE.0, COST_LINE.1));
    let reason = common::proposal::refused(&outcome, "a raised rev");
    assert!(
        reason.ends_with(
            "`RULE-SPRINT-COST`'s revision changed (3 -> 4): keep rev as read; approval sets \
             revisions"
        ),
        "{reason}"
    );
    // Lowered too.
    let lowered = edit(&text, "rev=3}", "rev=2}");
    let reason =
        common::proposal::refused(&propose_text(&at, COST, &hash, &lowered), "a lowered rev");
    assert!(reason.contains("(3 -> 2)"), "{reason}");
    // A document's `rev:` written.
    let (hash, text) = pair.span(&pair.linked, DOC);
    let written = edit(&text, "id: MEC-SPRINT\n", "id: MEC-SPRINT\nrev: 2\n");
    let reason =
        common::proposal::refused(&propose_text(&at, DOC, &hash, &written), "a document's rev");
    assert!(
        reason.ends_with(
            "`MEC-SPRINT`'s revision changed (none written -> 2): keep rev as read; approval \
             sets revisions"
        ),
        "{reason}"
    );
    assert!(pair.proposals().is_empty(), "nothing stored");
}

/// RD-3: the `Revision` variant and its value are both compared: an
/// unwritten revision (read as 1) written as `rev=1` is a change, refused.
/// M: revisions compared by `number()`.
#[test]
fn unwritten_to_written_one_is_refused() {
    let pair = Pair::new("rd3-unwritten", "spec-a");
    let at = linked_at(&pair);
    let (hash, text) = pair.span(&pair.linked, EMPTY);
    let written = edit(&text, ".playtest}", ".playtest rev=1}");
    let reason = common::proposal::refused(
        &propose_text(&at, EMPTY, &hash, &written),
        "an unwritten revision written as 1",
    );
    assert!(
        reason.ends_with(
            "`EDGE-SPRINT-EMPTY`'s revision changed (none written -> 1): keep rev as read; \
             approval sets revisions"
        ),
        "{reason}"
    );
    // And back: a written `rev=1` dropped.
    commit_edit(
        &pair,
        SPRINT,
        EMPTY_HEADING,
        "### Empty tank {#EDGE-SPRINT-EMPTY .playtest rev=1}",
    );
    let (hash, text) = pair.span(&pair.linked, EMPTY);
    let dropped = edit(&text, " rev=1}", "}");
    let reason = common::proposal::refused(
        &propose_text(&at, EMPTY, &hash, &dropped),
        "a written 1 dropped",
    );
    assert!(reason.contains("(1 -> none written)"), "{reason}");
    assert!(pair.proposals().is_empty(), "nothing stored");
}

// ------------------------------------------------------------------ RD-6

/// RD-6: the review document has 48 keys (47 before drift-queue's
/// `drift`, right after `revisions`), `revisions` right after
/// `conflict`; an update's `revisions` lists each node whose meaning it
/// changes with its revision now, a meaning approve's, and the lock
/// entries an editorial one refreshes (0 without a lock), each entry
/// exactly [`REVISION_ENTRY_KEYS`]; the text line `revisions: …`.
/// M: the key dropped or moved; `to` computed by the reader.
#[test]
fn review_document_every_key_present() {
    let pair = Pair::new("rd6-keys", "spec-a");
    let id = propose_edits(&pair, COST, &[COST_LINE, EMPTY_LINE]);
    let (text, value) = review_at(&main_at(&pair), &id);
    let keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    // 48 since docs/features/drift-queue.md added `drift` after
    // `revisions` (`null` on an update).
    assert_eq!(keys.len(), 48, "{keys:?}");
    assert_eq!(value["drift"], Value::Null);
    let json_text = serde_json::to_string(&value).unwrap();
    let printed = rev_common::printed(
        &specengine_cli::review(
            &main_at(&pair).env,
            &specengine_cli::Globals::default(),
            &specengine_cli::ReviewRequest {
                id: id.clone(),
                git: pair.git_env(&pair.main),
            },
        )
        .unwrap(),
    )
    .1;
    let conflict = printed.find("\"conflict\":").unwrap();
    let revisions = printed.find("\"revisions\":").unwrap();
    let drift = printed.find("\"drift\":").unwrap();
    let decided_by = printed.find("\"decided_by\":").unwrap();
    assert!(
        conflict < revisions && revisions < drift && drift < decided_by,
        "{printed}"
    );
    assert_eq!(
        value["revisions"],
        json!([
            revision_entry(COST, 3, 4, Some(0)),
            revision_entry(EMPTY, 1, 2, Some(0)),
        ])
    );
    assert_entry_keys(&printed, &value);
    assert_eq!(REVISION_ENTRY_KEYS.len(), 5);
    let line = text
        .lines()
        .find(|line| line.starts_with("revisions:"))
        .unwrap_or_else(|| panic!("no revisions line: {text}"));
    assert_eq!(
        line,
        "revisions: RULE-SPRINT-COST 3 -> 4 (0 lock entries), EDGE-SPRINT-EMPTY 1 -> 2 (0 lock \
         entries)"
    );
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with("  "))
        .map(|line| line.split_once(':').unwrap().0)
        .collect();
    let at = lines.iter().position(|key| *key == "revisions").unwrap();
    assert_eq!(lines[at - 1], "conflict", "{text}");
    assert_eq!(lines[at + 1], "drift", "{text}");
    assert_eq!(lines.len(), 48, "{text}");
    assert!(!json_text.is_empty());
}

/// RD-6: `revisions` is `null` (text `-`) for a kind writing no existing
/// node (a new file's create; a question) and for a preview that
/// conflicts; a section-form create changing no existing node's meaning
/// gives `[]`. M: `[]` for a new file.
#[test]
fn revisions_null_for_a_new_file() {
    let pair = Pair::new("rd6-null", "spec-a");
    let at = linked_at(&pair);
    let new_file = "---\nclass: canon\nid: MEC-DASH\nowner: owner\nreviewed: 2026-09-20\n---\n\n\
                    # Dash\n\nA short burst.\n";
    let outcome = propose_create_text(&at, "docs/spec/movement/dash.md", None, new_file).unwrap();
    let created = outcome.document.id.clone().expect("a create stored");
    let (text, value) = review_at(&main_at(&pair), &created);
    assert_eq!(value["revisions"], Value::Null, "a new file");
    assert!(text.contains("\nrevisions: -\n"), "{text}");

    // A conflicting preview.
    let first = propose_edits(&pair, COST, &[COST_LINE]);
    let second = propose_edits(&pair, COST, &[("costs 12 units/s", "costs 11 units/s")]);
    approve_at(&main_at(&pair), &first, false, true).applied("the first");
    let (_, value) = review_at(&main_at(&pair), &second);
    assert_eq!(value["preview"], "conflicts", "{value}");
    assert_eq!(value["revisions"], Value::Null, "a conflict");

    // A section-form create: the parent's own text unchanged.
    let (hash, text) = pair.span(&pair.linked, EMPTY);
    let with_section = format!("{text}\n\n#### Recovery {{#EDGE-SPRINT-RECOVER}}\n\nIt recovers.");
    let outcome = propose_create_text(&at, EMPTY, Some(&hash), &with_section).unwrap();
    let section = outcome
        .document
        .id
        .clone()
        .expect("a section-form create stored");
    let (_, value) = review_at(&main_at(&pair), &section);
    assert_eq!(value["revisions"], json!([]), "{value}");
}

/// RD-6: `revisions` reads the file as it is now, not the proposal's base:
/// after another proposal raised `RULE-SPRINT-COST` to 4, a second one
/// raised against `rev=3` reviews as `4 -> 5`.
/// M: `rev` read from the proposal's base text.
#[test]
fn revisions_read_the_current_file() {
    let pair = Pair::new("rd6-current", "spec-a");
    commit_edit(
        &pair,
        SPRINT,
        "bounded by A-101.\n",
        "bounded by A-101.\n\nThe cost applies each second.\n",
    );
    let first = propose_edits(&pair, COST, &[COST_LINE]);
    let second = propose_edits(&pair, COST, &[("each second.", "every second.")]);
    let (_, value) = review_at(&main_at(&pair), &second);
    assert_eq!(
        value["revisions"],
        json!([revision_entry(COST, 3, 4, Some(0))])
    );
    approve_at(&main_at(&pair), &first, false, true).applied("the first");
    let (text, value) = review_at(&main_at(&pair), &second);
    assert_eq!(value["preview"], "rebases");
    assert_eq!(
        value["revisions"],
        json!([revision_entry(COST, 4, 5, Some(0))])
    );
    assert!(
        text.contains("\nrevisions: RULE-SPRINT-COST 4 -> 5 (0 lock entries)\n"),
        "{text}"
    );
}

/// The review text's `revisions:` line.
fn revisions_line(text: &str) -> &str {
    text.lines()
        .find(|line| line.starts_with("revisions:"))
        .unwrap_or_else(|| panic!("no revisions line: {text}"))
}

/// RD-6 (Q15): a raise step 6 would refuse no longer makes the preview
/// `unavailable`: the review previews the text an editorial approve writes
/// (`applies`), `revisions` filled, the refused entry's `to` `null` and its
/// `refused` the step-6 reason as a refusal prints it after `(step 6): `
/// (naming `--editorial`), the other nodes keeping their `to`; the text
/// `ID N -> refused (0 lock entries)` and one note `meaning approve not
/// applicable (step 6): <refused>` per refused node. Two causes: a document
/// of a closed class without `rev` (Q11's line), and a section at `REV_MAX`
/// beside one that raises 1 -> 2.
/// M: the preview made `unavailable` on a raise refusal.
#[test]
fn revisions_name_a_refused_raise() {
    let pair = Pair::new("rd6-refused", "spec-a");
    close_canon(&pair);

    // A closed class without `rev`: the document's raise refused.
    let closed = propose_edits(&pair, DOC, &[INTRO]);
    let (text, json_text, value) = review_printed(&main_at(&pair), &closed);
    assert_eq!(value["preview"], "applies", "{value}");
    assert_eq!(
        value["revisions"],
        json!([refused_entry(DOC, 1, Some(0), CLOSED_REASON)])
    );
    assert_entry_keys(&json_text, &value);
    assert!(
        json_text.contains(&format!(
            "\"revisions\":[{{\"id\":\"{DOC}\",\"rev\":1,\"to\":null,\"lock_entries\":0,\"refused\":"
        )),
        "{json_text}"
    );
    assert_eq!(
        notes_starting(&value, NOT_APPLICABLE),
        [format!("{NOT_APPLICABLE}{CLOSED_REASON}")]
    );
    assert_eq!(
        notes_starting(&value, "not applicable now"),
        Vec::<String>::new()
    );
    assert_eq!(
        revisions_line(&text),
        "revisions: MEC-SPRINT 1 -> refused (0 lock entries)"
    );
    assert!(
        text.contains(&format!("\n  {NOT_APPLICABLE}{CLOSED_REASON}\n")),
        "{text}"
    );

    // `REV_MAX` beside a node that raises: the one refused, the other's
    // `to` kept.
    commit_edit(&pair, SPRINT, COST_HEADING, MAX_HEADING);
    let max = propose_edits(&pair, COST, &[COST_LINE, EMPTY_LINE]);
    let (text, json_text, value) = review_printed(&main_at(&pair), &max);
    assert_eq!(value["preview"], "applies", "{value}");
    let refused = value["revisions"][0]["refused"]
        .as_str()
        .unwrap_or_else(|| panic!("a refused entry first: {value}"))
        .to_owned();
    for named in ["`RULE-SPRINT-COST`", "999999999", "--editorial"] {
        assert!(refused.contains(named), "{named} in {refused}");
    }
    assert_eq!(
        value["revisions"],
        json!([
            refused_entry(COST, 999_999_999, Some(0), &refused),
            revision_entry(EMPTY, 1, 2, Some(0)),
        ])
    );
    assert_entry_keys(&json_text, &value);
    assert_eq!(
        notes_starting(&value, NOT_APPLICABLE),
        [format!("{NOT_APPLICABLE}{refused}")]
    );
    assert_eq!(
        revisions_line(&text),
        "revisions: RULE-SPRINT-COST 999999999 -> refused (0 lock entries), EDGE-SPRINT-EMPTY 1 \
         -> 2 (0 lock entries)"
    );
    // `refused` is what a meaning approve prints after `(step 6): `.
    let asked = approve_at(&main_at(&pair), &max, false, true);
    assert_eq!(
        after_step_6(&asked.refused("REV_MAX beside a raise")),
        refused
    );
}

/// Q15 with RD-1: a meaning approve over a refused raise still refuses at
/// step 6, before the question, exit 1, with the step-6 message, even with
/// another node that raises: `REV_MAX`'s; with two refused nodes the first
/// in file order (the closed document's Q11 line before the section at
/// `REV_MAX`). Nothing written, `open`, one `apply_failed` at step 6, the
/// failed document's preview `unavailable`; `--editorial` then applies the
/// text as sent, every revision kept.
/// M: the refusal dropped once the review keeps the plan (the raise
/// written for the other nodes).
#[test]
fn meaning_approve_over_a_refused_raise_refuses_at_step_6() {
    let pair = Pair::new("rd6-refused-approve", "spec-a");
    close_canon(&pair);
    commit_edit(&pair, SPRINT, COST_HEADING, MAX_HEADING);
    let id = propose_edits(&pair, COST, &[COST_LINE, EMPTY_LINE]);
    let before = pair.state();
    let asked = approve_at(&main_at(&pair), &id, false, true);
    let reason = asked.refused("REV_MAX beside a raise");
    assert_eq!(
        after_step_6(&reason),
        "`RULE-SPRINT-COST` is at rev 999999999, the largest revision: it cannot be raised; \
         approve with --editorial"
    );
    assert!(asked.questions.is_empty(), "refused before the question");
    assert_eq!(pair.state().linked_files, before.linked_files);
    assert_eq!(pair.proposal(&id).status.as_str(), "open");
    let failed: Vec<(String, Option<u64>)> = pair
        .events_of(&id)
        .into_iter()
        .filter(|(event, _)| event == "proposal.apply_failed")
        .collect();
    assert_eq!(failed, [("proposal.apply_failed".to_owned(), Some(6))]);
    let (_, json) = printed(asked.outcome.as_ref().unwrap());
    assert!(json.contains("\"preview\":\"unavailable\""), "{json}");

    // Two refused nodes: the first in file order is named.
    let both = propose_edits(&pair, DOC, &[INTRO, COST_LINE]);
    let asked = approve_at(&main_at(&pair), &both, false, true);
    assert_eq!(
        after_step_6(&asked.refused("two refused raises")),
        CLOSED_REASON
    );
    assert!(asked.questions.is_empty(), "refused before the question");

    // `--editorial` applies the first, every revision kept.
    let file = read_text(&pair.linked, SPRINT);
    let asked = approve_at(&main_at(&pair), &id, true, true);
    asked.applied("--editorial over a refused raise");
    assert_eq!(
        asked.question(),
        question(
            &pair,
            &id,
            "applies, editorial: RULE-SPRINT-COST stays 999999999, EDGE-SPRINT-EMPTY stays 1"
        )
    );
    assert_eq!(
        read_text(&pair.linked, SPRINT),
        expected(&file, &[COST_LINE, EMPTY_LINE])
    );
}

/// RD-6 beside Q15: an unreadable current revision is no refused raise (it
/// refuses either approve, "Raising at apply"): the preview stays
/// `unavailable`, `revisions` `null` (text `-`), one note `not applicable
/// now (step 6): …` naming the node, none `meaning approve not
/// applicable`; `spec approve --editorial` refuses at step 6 too, before
/// the question. M: an unreadable revision listed as a refused raise.
#[test]
fn unreadable_revision_keeps_the_preview_unavailable() {
    let pair = Pair::new("rd6-unreadable", "spec-a");
    commit_edit(
        &pair,
        SPRINT,
        COST_HEADING,
        "## Cost {#RULE-SPRINT-COST rev=0}",
    );
    let id = propose_edits(&pair, COST, &[COST_LINE]);
    let (text, value) = review_at(&main_at(&pair), &id);
    assert_eq!(value["preview"], "unavailable", "{value}");
    assert_eq!(value["revisions"], Value::Null, "{value}");
    assert!(text.contains("\nrevisions: -\n"), "{text}");
    let notes = notes_starting(&value, "not applicable now (step 6): ");
    assert_eq!(notes.len(), 1, "{value}");
    assert!(
        notes[0].contains("`RULE-SPRINT-COST`") && notes[0].contains("unreadable"),
        "{notes:?}"
    );
    assert_eq!(notes_starting(&value, NOT_APPLICABLE), Vec::<String>::new());
    let before = pair.state();
    let asked = approve_at(&main_at(&pair), &id, true, true);
    let reason = asked.refused("--editorial over an unreadable revision");
    assert!(reason.contains("(step 6)"), "{reason}");
    assert!(reason.contains("`RULE-SPRINT-COST`"), "{reason}");
    assert!(reason.contains("unreadable"), "{reason}");
    assert!(asked.questions.is_empty(), "refused before the question");
    assert_eq!(pair.state().linked_files, before.linked_files);
}

/// Q15, `write_revisions`' attribution: `MEC-SPRINT`'s front-matter
/// written flow-style has no `id:` line to put `rev:` after, so its raise
/// is refused by name as a revision that cannot be written (naming
/// `--editorial`); the two sections the same proposal changes keep their
/// `to` (3 -> 4, 1 -> 2). The meaning approve refuses at step 6 with that
/// reason; `--editorial` applies the text as sent.
/// M: every raise blamed when the raises written together fail.
#[test]
fn unwritable_revision_is_refused_alone() {
    let pair = Pair::new("rd6-flow", "spec-a");
    let text = read_text(&pair.linked, SPRINT);
    let body = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .expect("a front-matter block")
        .1;
    write(
        &pair.linked,
        SPRINT,
        format!(
            "---\n{{id: MEC-SPRINT, class: canon, tier: 2, parent: DOM-MOVEMENT, owner: owner, \
             reviewed: 2026-09-20, links: {{depends_on: [MEC-STAMINA], constrains: \
             [RULE-STAM-REGEN]}}}}\n---\n{body}"
        ),
    );
    pair.commit_all(&pair.linked, "flow-style front-matter");
    assert_eq!(pair.node(&pair.linked, DOC).rev, None, "read as unwritten");
    let id = propose_edits(&pair, DOC, &[INTRO, COST_LINE, EMPTY_LINE]);
    let (text, json_text, value) = review_printed(&main_at(&pair), &id);
    assert_eq!(value["preview"], "applies", "{value}");
    let refused = value["revisions"][0]["refused"]
        .as_str()
        .unwrap_or_else(|| panic!("the document refused: {value}"))
        .to_owned();
    assert!(
        refused.starts_with("`MEC-SPRINT`") && refused.contains("cannot be written"),
        "{refused}"
    );
    assert!(refused.contains("--editorial"), "{refused}");
    assert_eq!(
        value["revisions"],
        json!([
            refused_entry(DOC, 1, Some(0), &refused),
            revision_entry(COST, 3, 4, Some(0)),
            revision_entry(EMPTY, 1, 2, Some(0)),
        ])
    );
    assert_entry_keys(&json_text, &value);
    assert_eq!(
        notes_starting(&value, NOT_APPLICABLE),
        [format!("{NOT_APPLICABLE}{refused}")]
    );
    assert_eq!(
        revisions_line(&text),
        "revisions: MEC-SPRINT 1 -> refused (0 lock entries), RULE-SPRINT-COST 3 -> 4 (0 lock \
         entries), EDGE-SPRINT-EMPTY 1 -> 2 (0 lock entries)"
    );
    let before = pair.state();
    let asked = approve_at(&main_at(&pair), &id, false, true);
    assert_eq!(
        after_step_6(&asked.refused("an unwritable revision")),
        refused
    );
    assert!(asked.questions.is_empty(), "refused before the question");
    assert_eq!(pair.state().linked_files, before.linked_files);

    let file = read_text(&pair.linked, SPRINT);
    approve_at(&main_at(&pair), &id, true, true).applied("--editorial");
    assert_eq!(
        read_text(&pair.linked, SPRINT),
        expected(&file, &[INTRO, COST_LINE, EMPTY_LINE])
    );
}

// ------------------------------------------------------------------ TE-1

/// TE-1 (spec-a, a game's design spec): through a `pre-commit` hook
/// running `spec check --staged --root .` (the built binary), an
/// interpretation of `DEC-0023` applied as a meaning change raises
/// `RULE-STAM-REGEN` 1 -> 2, its trailers `Revision:` after `Base-commit:`
/// and `Interprets:` last; a second one approved `--editorial` keeps 2,
/// `Revision: … 2 (editorial)` before `Interprets:`. Both commits passed
/// the hook.
#[test]
fn te1_interpretation_through_the_hook_raises_before_interprets() {
    let pair = Pair::new("te1-interpretation", "spec-a");
    // spec-a's one blocking finding fixed, so the hook passes clean.
    commit_edit(
        &pair,
        "docs/features/stamina-tuning.md",
        "tier: two\n",
        "tier: 2\n",
    );
    let log = pair.scratch.join("hook.log");
    let hooks = pair.main.join(".git/hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("pre-commit");
    std::fs::write(
        &hook,
        format!(
            "#!/bin/sh\n\"{}\" check --staged --root . >/dev/null || exit 1\necho ran >> '{}'\n",
            common::SPEC,
            log.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let stamina = "docs/spec/movement/stamina.md";
    let interpret = |from: &str, to: &str| -> String {
        let (hash, text) = pair.span(&pair.linked, "RULE-STAM-REGEN");
        let outcome = specengine_cli::propose_interpretation(
            &pair.env(&pair.linked),
            &specengine_cli::Globals::default(),
            &specengine_cli::InterpretRequest {
                target: "RULE-STAM-REGEN".to_owned(),
                base: hash,
                decision: "DEC-0023".to_owned(),
                how: "Rest is any speed under 40 % of walking.".to_owned(),
                why: "DEC-0023 says rest; the rule names no speed.".to_owned(),
                text: specengine_cli::ProposedText::Given(edit(&text, from, to).into_bytes()),
                rationale: "Carry DEC-0023 into the rule.".to_owned(),
                author_role: Some("spec-author".to_owned()),
                author_model: Some("claude-opus-5-5".to_owned()),
                run: None,
                now: rev_common::NOW.to_owned(),
                git: pair.git_env(&pair.linked),
            },
            None,
        )
        .unwrap_or_else(|error| panic!("interpret: {error}"));
        outcome.document.id.clone().expect("stored")
    };
    let id = interpret("Base rate 10 units/s,", "Base rate 10 units/s at rest,");
    let asked = approve_at(&main_at(&pair), &id, false, true);
    asked.applied("an interpretation");
    assert_eq!(
        asked.question(),
        format!(
            "interprets DEC-0023 (docs/records/DEC/DEC-0023.md)\napply {id} to {stamina} on t1 \
             in {} (applies; raises RULE-STAM-REGEN 1 -> 2)? [y/N]",
            pair.linked.display()
        )
    );
    assert_eq!(
        after_base(&tip_message(&pair)),
        ["Revision: RULE-STAM-REGEN 1 -> 2", "Interprets: DEC-0023"]
    );
    assert!(read_text(&pair.linked, stamina).contains("{#RULE-STAM-REGEN rev=2}"));

    let id = interpret("rate \u{d7} 0.5", "rate \u{d7} 0.5 (halved)");
    approve_at(&main_at(&pair), &id, true, true).applied("an editorial interpretation");
    assert_eq!(
        after_base(&tip_message(&pair)),
        [
            "Revision: RULE-STAM-REGEN 2 (editorial)",
            "Interprets: DEC-0023"
        ]
    );
    assert!(read_text(&pair.linked, stamina).contains("{#RULE-STAM-REGEN rev=2}"));
    let runs = std::fs::read_to_string(&log).unwrap_or_default();
    assert_eq!(runs.lines().count(), 2, "both commits passed the hook");
}

/// RD-3 for the other kinds that write an existing node: a section-form
/// create whose text changes its target's `rev=3`, and an interpretation
/// doing the same, are refused at creation alike, nothing stored.
/// M: the check run for updates only.
#[test]
fn every_kind_writing_a_node_refuses_a_revision_change() {
    let pair = Pair::new("rd3-kinds", "spec-a");
    let at = linked_at(&pair);
    let (hash, text) = pair.span(&pair.linked, COST);
    let raised = edit(&text, "rev=3}", "rev=4}");
    let with_section =
        format!("{raised}\n\n#### Recovery {{#EDGE-SPRINT-RECOVER}}\n\nIt recovers.");
    let outcome = propose_create_text(&at, COST, Some(&hash), &with_section);
    let reason = common::proposal::refused(&outcome, "a section-form create raising rev");
    assert!(
        reason.contains("`RULE-SPRINT-COST`'s revision changed (3 -> 4)"),
        "{reason}"
    );
    let (hash, text) = pair.span(&pair.linked, "RULE-STAM-REGEN");
    let outcome = specengine_cli::propose_interpretation(
        &pair.env(&pair.linked),
        &specengine_cli::Globals::default(),
        &specengine_cli::InterpretRequest {
            target: "RULE-STAM-REGEN".to_owned(),
            base: hash,
            decision: "DEC-0023".to_owned(),
            how: "Rest is any speed under 40 % of walking.".to_owned(),
            why: "DEC-0023 says rest; the rule names no speed.".to_owned(),
            text: specengine_cli::ProposedText::Given(
                edit(&text, "{#RULE-STAM-REGEN}", "{#RULE-STAM-REGEN rev=2}").into_bytes(),
            ),
            rationale: "Carry DEC-0023 into the rule.".to_owned(),
            author_role: Some("spec-author".to_owned()),
            author_model: Some("claude-opus-5-5".to_owned()),
            run: None,
            now: rev_common::NOW.to_owned(),
            git: pair.git_env(&pair.linked),
        },
        None,
    );
    let reason = common::proposal::refused(&outcome, "an interpretation writing rev");
    assert!(
        reason.contains("`RULE-STAM-REGEN`'s revision changed (none written -> 2)"),
        "{reason}"
    );
    assert!(pair.proposals().is_empty(), "nothing stored");
}
