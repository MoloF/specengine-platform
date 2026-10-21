//! docs/features/rev-rule.md RD-4 and TE-4: `spec bump [--staged]
//! [--json]` (`docs/features/rev-rule.md` "spec bump"), read-only, over
//! scratch repositories of `fixtures/code-a` with its golden lock
//! committed: a row per node with an ID at `HEAD` whose `norm_hash`
//! changed and whose revision is not above `HEAD`'s, by path and line,
//! `required` exactly when the rule finds it, else `suggested`; the
//! `same_revision` hint of `spec verify` naming `spec bump`, accept's
//! refusals (plain and a drift item's approve) keeping theirs. The rows
//! and texts are built from the spec's "Data", never from the output.

#![cfg(unix)]

mod common;
mod drift_common;
mod lock_common;
mod rev_rule_common;
mod tested_common;
mod verify_common;

use std::collections::BTreeMap;
use std::fs;
use std::time::{Duration, UNIX_EPOCH};

use common::snapshot;
use rev_rule_common::{
    BOUND, BOUND_EDIT, BOUND_LINE, CAP_MD, COUNTER_MD, GOAL, GOAL_EDIT, GOAL_LINE, GOAL_MD,
    LIMITS_MD, LOCK, MAX_REV, ODD_EDIT, ODD_LINE, PASS, PASS_EDIT, PASS_LINE, PIECE, PIECE_EDIT,
    PIECE_LINE, STEP_EDIT, STEP_LINE, bound_at_max, bump, bump_line, bump_line_alone, changed,
    close_goal, fake_hash, refresh, spec_at, stage_all, unwritable_raises, with_spec_hash,
};
use serde_json::{Value, json};
use verify_common::{golden, golden_format_1};

/// The keys of a JSON row, in order ("spec bump").
const ROW_KEYS: [&str; 8] = [
    "need",
    "id",
    "path",
    "line",
    "rev",
    "to",
    "lock_entries",
    "write",
];

/// The hint after the rows when one is suggested ("spec bump").
const SUGGESTED_HINT: &str = "hint: suggested: raising it makes markers naming it lag";

/// The paths whose bytes differ between two snapshots.
fn differing(
    before: &BTreeMap<String, Option<Vec<u8>>>,
    after: &BTreeMap<String, Option<Vec<u8>>>,
) -> Vec<String> {
    before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect()
}

/// A JSON row as "spec bump" gives it.
#[allow(clippy::too_many_arguments)]
fn row(
    need: &str,
    id: &str,
    path: &str,
    line: u64,
    rev: u32,
    to: u32,
    entries: usize,
    write: &str,
) -> Value {
    json!({
        "need": need,
        "id": id,
        "path": path,
        "line": line,
        "rev": rev,
        "to": to,
        "lock_entries": entries,
        "write": write,
    })
}

// ------------------------------------------------------------------ TE-4

/// TE-4: the disk against `HEAD`: `GOAL-01` changed with its entry
/// refreshed (`suggested`, its `rev: 2` inserted; one entry since
/// docs/features/lock-levels.md: its `[path, path]` binding is
/// `bad_levels`), `LAW-BOUND` changed
/// (`required`: four entries vouch, none refreshed), the unlocked
/// `LAW-STEP` (`suggested`, no entry), `LAW-ODD` at an unreadable `rev=0`
/// (`suggested`, `rev`, `to` and `write` null); `PIECE-COUNTER` raised by
/// hand and a renamed heading give no row. Rows by path and line, text
/// and JSON (eight keys in order, `{rows}` alone), both hints, exit 0.
#[test]
fn bump_rows_list_unraised_changes_by_path_and_line() {
    let repo = golden("te4-rows");
    repo.replace(GOAL_MD, GOAL_EDIT.0, GOAL_EDIT.1);
    refresh(&repo, GOAL, None);
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.replace(COUNTER_MD, STEP_EDIT.0, STEP_EDIT.1);
    repo.replace(COUNTER_MD, ODD_EDIT.0, ODD_EDIT.1);
    repo.replace(COUNTER_MD, PIECE_EDIT.0, PIECE_EDIT.1);
    repo.replace(COUNTER_MD, "\nrev: 3\n", "\nrev: 4\n");
    repo.replace(
        CAP_MD,
        "### Stepping past the cap keeps the cap {#PASS-01}",
        "### Stepping beyond the cap keeps the cap {#PASS-01}",
    );
    let run = bump(&repo, &[]);
    assert_eq!(run.code(), 0, "{}", run.show());

    let goal = bump_line("suggested", GOAL, GOAL_MD, GOAL_LINE, 1, 2, 1, "rev: 2");
    let bound = bump_line(
        "required",
        BOUND,
        COUNTER_MD,
        BOUND_LINE,
        2,
        3,
        4,
        "## Bound {#LAW-BOUND rev=3}",
    );
    let step = bump_line(
        "suggested",
        "LAW-STEP",
        COUNTER_MD,
        STEP_LINE,
        1,
        2,
        0,
        "## Step {#LAW-STEP rev=2}",
    );
    let lines = run.row_lines();
    assert_eq!(lines.len(), 4, "{}", run.show());
    assert_eq!(
        lines[..3],
        [&goal[..], &bound[..], &step[..]],
        "{}",
        run.show()
    );
    assert!(
        lines[3].starts_with(&format!(
            "suggested | LAW-ODD | {COUNTER_MD}:{ODD_LINE} | rev ?"
        )),
        "{}",
        run.show()
    );
    let all = run.all_lines();
    assert!(all.contains(&SUGGESTED_HINT), "{}", run.show());
    let required = all
        .iter()
        .find(|line| line.starts_with("hint: required: "))
        .unwrap_or_else(|| panic!("no required hint:\n{}", run.show()));
    assert!(required.contains("--editorial"), "{required}");

    let document = run.json.as_object().expect("an object");
    assert_eq!(
        document.keys().collect::<Vec<_>>(),
        ["rows"],
        "{}",
        run.show()
    );
    let rows = run.rows();
    assert_eq!(rows.len(), 4);
    assert_eq!(
        rows[..3],
        [
            &row("suggested", GOAL, GOAL_MD, GOAL_LINE, 1, 2, 1, "rev: 2"),
            &row(
                "required",
                BOUND,
                COUNTER_MD,
                BOUND_LINE,
                2,
                3,
                4,
                "## Bound {#LAW-BOUND rev=3}"
            ),
            &row(
                "suggested",
                "LAW-STEP",
                COUNTER_MD,
                STEP_LINE,
                1,
                2,
                0,
                "## Step {#LAW-STEP rev=2}"
            ),
        ]
    );
    assert_eq!(
        rows[3],
        &json!({
            "need": "suggested",
            "id": "LAW-ODD",
            "path": COUNTER_MD,
            "line": ODD_LINE,
            "rev": null,
            "to": null,
            "lock_entries": 0,
            "write": null,
        })
    );
    let printed = run.json_run.stdout.trim_end();
    let first = printed.find("{\"need\"").expect("a row");
    let at: Vec<usize> = ROW_KEYS
        .iter()
        .map(|key| first + printed[first..].find(&format!("\"{key}\":")).expect(key))
        .collect();
    assert!(at.windows(2).all(|pair| pair[0] < pair[1]), "{printed}");
}

/// TE-4: `--staged` reads the index, the default the disk: `LAW-BOUND`
/// changed in the index only (the disk set back), `LAW-STEP` on the disk
/// only: each run lists its own.
#[test]
fn bump_staged_reads_the_index_and_plain_the_disk() {
    let repo = golden("te4-staged");
    let text = repo.read(COUNTER_MD);
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.git(&["add", "--", COUNTER_MD]);
    repo.write(COUNTER_MD, text);
    repo.replace(COUNTER_MD, STEP_EDIT.0, STEP_EDIT.1);
    let from_index = bump(&repo, &["--staged"]);
    assert_eq!(from_index.code(), 0, "{}", from_index.show());
    assert_eq!(
        from_index.needs(),
        [(BOUND.to_owned(), "required".to_owned())],
        "{}",
        from_index.show()
    );
    let from_disk = bump(&repo, &[]);
    assert_eq!(
        from_disk.needs(),
        [("LAW-STEP".to_owned(), "suggested".to_owned())],
        "{}",
        from_disk.show()
    );
}

/// TE-4: nothing changed, only a heading renamed, or only the lock made
/// unparseable (no candidate: the lock unread): `nothing to raise`,
/// `{"rows": []}`, exit 0, on both sides.
#[test]
fn bump_says_nothing_to_raise() {
    let repo = golden("te4-nothing");
    let quiet = |context: &str| {
        for flags in [&[][..], &["--staged"][..]] {
            let run = bump(&repo, flags);
            assert_eq!(run.code(), 0, "{context} {flags:?}:\n{}", run.show());
            assert_eq!(
                run.text.stdout.lines().collect::<Vec<_>>(),
                ["nothing to raise"],
                "{context} {flags:?}:\n{}",
                run.show()
            );
            assert_eq!(run.json, json!({"rows": []}), "{context} {flags:?}");
        }
    };
    quiet("a clean tree");
    repo.replace(
        COUNTER_MD,
        "## Bound {#LAW-BOUND rev=2}",
        "## Upper bound {#LAW-BOUND rev=2}",
    );
    stage_all(&repo);
    quiet("a heading renamed");
    repo.write(LOCK, "this is not a lock\n");
    stage_all(&repo);
    quiet("an unparseable lock, no candidate");
}

/// TE-4: exit 2 on the rule's lock causes (a candidate and an
/// unparseable lock, on the disk or staged, the cause naming `spec.lock`)
/// and on check's run causes (an invalid config); no row printed.
#[test]
fn bump_exits_2_on_lock_and_run_causes() {
    let repo = golden("te4-causes");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.write(LOCK, "this is not a lock\n");
    stage_all(&repo);
    for flags in [&[][..], &["--staged"][..]] {
        let run = bump(&repo, flags);
        assert_eq!(run.code(), 2, "{flags:?}:\n{}", run.show());
        assert!(run.row_lines().is_empty(), "{}", run.show());
        assert!(
            run.all_lines().iter().any(|line| line.contains(LOCK)),
            "{flags:?}: the cause names spec.lock:\n{}",
            run.show()
        );
    }

    let repo = golden("te4-config");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.append("specengine.toml", "\n[check]\nmode = \"sometimes\"\n");
    let run = bump(&repo, &[]);
    assert_eq!(run.code(), 2, "{}", run.show());
    assert!(run.row_lines().is_empty(), "{}", run.show());
}

// ------------------------------------------------------------------ RD-4

/// RD-4: `spec bump`, `--staged`, `--json` write nothing: every byte
/// under the worktree (`.git` with its index, objects, refs and `HEAD`)
/// and under `HOME` (the data directory, a queue) as before, though a
/// tracked file was rewritten with its own bytes and an older mtime (its
/// stat no longer the index's, so any index refresh, a `git status`,
/// would write `.git/index`). M: any write (status, `HEAD`, index, queue).
#[test]
fn bump_writes_nothing() {
    let repo = golden("rd4-writes-nothing");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.replace(GOAL_MD, GOAL_EDIT.0, GOAL_EDIT.1);
    refresh(&repo, GOAL, Some(0));
    repo.git(&["add", "--", COUNTER_MD]);
    let _ = repo.porcelain();
    let limits = repo.read(LIMITS_MD);
    repo.write(LIMITS_MD, limits);
    fs::File::options()
        .write(true)
        .open(repo.root.join(LIMITS_MD))
        .and_then(|file| file.set_modified(UNIX_EPOCH + Duration::from_secs(1_000_000_000)))
        .expect("an older mtime");
    let tree = snapshot(&repo.root);
    let home = snapshot(&repo.home);
    for flags in [
        &["bump"][..],
        &["bump", "--json"][..],
        &["bump", "--staged"][..],
        &["bump", "--staged", "--json"][..],
    ] {
        let run = spec_at(&repo, &repo.root, flags, &[]);
        assert_eq!(run.code, 0, "{}", run.show());
        assert_eq!(
            differing(&tree, &snapshot(&repo.root)),
            Vec::<String>::new(),
            "{flags:?} wrote under the worktree"
        );
        assert_eq!(
            differing(&home, &snapshot(&repo.home)),
            Vec::<String>::new(),
            "{flags:?} wrote under HOME"
        );
    }
}

/// RD-4: `required` exactly for the nodes the rule finds (`spec check
/// --changed` on the same disk): `LAW-BOUND` (none refreshed) and
/// `GOAL-01` (one of two); `suggested` for `cap-check/PASS-01` (its entry
/// refreshed), `PIECE-COUNTER` (`HEAD`'s entries vouch for another text:
/// none in scope, 0 entries) and the unlocked `LAW-STEP`. M: a vouched
/// node marked `suggested`, or any changed one `required`. Over the
/// format-1 golden, which holds `GOAL-01`'s two entries and
/// `cap-check/PASS-01`'s one (docs/features/lock-levels.md Q10: bump
/// reads it).
#[test]
fn bump_marks_vouched_nodes_required() {
    let repo = golden_format_1("rd4-required");
    let tampered = with_spec_hash(&repo.lock_text(), PIECE, None, &fake_hash());
    repo.write(LOCK, tampered);
    repo.commit_all("PIECE-COUNTER's entries vouch for another text");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.replace(COUNTER_MD, STEP_EDIT.0, STEP_EDIT.1);
    repo.replace(COUNTER_MD, PIECE_EDIT.0, PIECE_EDIT.1);
    repo.replace(GOAL_MD, GOAL_EDIT.0, GOAL_EDIT.1);
    refresh(&repo, GOAL, Some(1));
    repo.replace(CAP_MD, PASS_EDIT.0, PASS_EDIT.1);
    refresh(&repo, PASS, None);
    let run = bump(&repo, &[]);
    assert_eq!(run.code(), 0, "{}", run.show());
    let need = |need: &str| need.to_owned();
    assert_eq!(
        run.needs(),
        [
            (PASS.to_owned(), need("suggested")),
            (GOAL.to_owned(), need("required")),
            (PIECE.to_owned(), need("suggested")),
            (BOUND.to_owned(), need("required")),
            ("LAW-STEP".to_owned(), need("suggested")),
        ],
        "{}",
        run.show()
    );
    assert_eq!(run.row(PIECE)["lock_entries"], 0);
    assert_eq!(run.row(PASS)["lock_entries"], 1);
    let pass = bump_line(
        "suggested",
        PASS,
        CAP_MD,
        PASS_LINE,
        1,
        2,
        1,
        "### Stepping past the cap keeps the cap {#PASS-01 rev=2}",
    );
    assert_eq!(
        run.row_lines().first(),
        Some(&pass.as_str()),
        "{}",
        run.show()
    );
    assert!(pass.contains("| 1 lock entry |"), "{pass}");
    assert_eq!(run.row(GOAL)["lock_entries"], 2);
    let mut required: Vec<String> = run
        .needs()
        .into_iter()
        .filter(|(_, need)| need == "required")
        .map(|(id, _)| id)
        .collect();
    required.sort();
    let check = changed(&repo);
    let mut found = check.subjects();
    found.sort();
    assert_eq!(required, found, "{}", check.show());
}

/// RD-4: `spec bump` has no `--editorial` (bump never writes; the
/// editorial way is the queue's): a usage error, exit 2, stdout empty,
/// nothing written.
#[test]
fn bump_has_no_editorial_flag() {
    let repo = golden("rd4-no-editorial");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    let tree = snapshot(&repo.root);
    for args in [
        &["bump", "--editorial"][..],
        &["bump", "--editorial", BOUND][..],
        &["bump", "--staged", "--editorial", BOUND][..],
    ] {
        let run = spec_at(&repo, &repo.root, args, &[]);
        assert_eq!(run.code, 2, "{}", run.show());
        assert_eq!(run.stdout, "", "{}", run.show());
        assert!(run.stderr.contains("--editorial"), "{}", run.show());
    }
    assert_eq!(
        differing(&tree, &snapshot(&repo.root)),
        Vec::<String>::new()
    );
}

/// RD-4: `LAW-BOUND`'s text changed at its locked revision and committed:
/// `spec verify`'s `spec_ahead same_revision` hint names `spec bump`
/// (while the edit is uncommitted) between "raise its `rev` by hand" and
/// "commit, then accept"; accept's refusal, plain and through a drift
/// item's approve, keeps its step without `spec bump` (it judges `HEAD`,
/// where bump lists nothing). M: the hint without `spec bump`, or bump
/// named in accept's refusal.
#[test]
fn same_revision_hint_names_bump_and_editorial() {
    let hint_step = "raise its `rev` by hand (`spec bump` prints the line while the edit is \
                     uncommitted), commit, then accept; or `--editorial`";
    let accept_step = "raise its `rev` by hand, commit, then accept; or `--editorial`";
    let repo = golden("rd4-hint");
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    repo.commit_all("a meaning change at rev 2, no raise");
    let run = repo.spec_piped(&repo.root, &["verify"]);
    let hint = run
        .stdout
        .lines()
        .find(|line| line.starts_with("hint: spec_ahead same_revision: "))
        .unwrap_or_else(|| panic!("no same_revision hint:\n{}", run.show()));
    assert!(hint.contains(hint_step), "{hint}");

    let refusal = repo.accept(BOUND, true).refused("text changed at rev 2");
    assert!(refusal.contains(accept_step), "{refusal}");
    assert!(!refusal.contains("spec bump"), "{refusal}");

    let raised = drift_common::raised(&repo, &[]);
    let id = raised
        .created()
        .into_iter()
        .find(|(_, node)| node == BOUND)
        .unwrap_or_else(|| panic!("no item for LAW-BOUND: {}", raised.json_text()))
        .0;
    let reason = drift_common::approve_yes(&repo, &id).refused("a drift approve at R");
    assert!(reason.contains(accept_step), "{reason}");
    assert!(!reason.contains("spec bump"), "{reason}");
    assert!(
        reason.contains(&format!("spec approve {id} --editorial")),
        "{reason}"
    );
}

// --------------------------------------------- no raise (iteration 2, W14)

/// A JSON row with no raise ("spec bump"): `to` and `write` null.
fn row_alone(need: &str, id: &str, path: &str, line: u64, rev: u32, entries: usize) -> Value {
    json!({
        "need": need,
        "id": id,
        "path": path,
        "line": line,
        "rev": rev,
        "to": null,
        "lock_entries": entries,
        "write": null,
    })
}

/// W14 (iteration 2): `GOAL-01` a document of the closed class `decision`
/// (`close_goal`), changed on the disk without a raise: its row
/// `required`, `rev 1` alone, `1 lock entry` (format 2), write `-`; JSON `to` and
/// `write` null (both keys there). `LAW-BOUND` changed with it keeps
/// `rev 2 -> 3` and its raised heading. M: the closed class not consulted
/// (`rev 1 -> 2 | … | rev: 2`).
#[test]
fn closed_class_bump_row_is_rev_alone() {
    let repo = golden("w14-bump-closed");
    close_goal(&repo);
    repo.replace(GOAL_MD, GOAL_EDIT.0, GOAL_EDIT.1);
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    let run = bump(&repo, &[]);
    assert_eq!(run.code(), 0, "{}", run.show());
    let heading = "## Bound {#LAW-BOUND rev=3}";
    assert_eq!(
        run.row_lines(),
        [
            bump_line_alone("required", GOAL, GOAL_MD, GOAL_LINE, 1, 1),
            bump_line("required", BOUND, COUNTER_MD, BOUND_LINE, 2, 3, 4, heading),
        ],
        "{}",
        run.show()
    );
    assert_eq!(
        run.rows(),
        [
            &row_alone("required", GOAL, GOAL_MD, GOAL_LINE, 1, 1),
            &row("required", BOUND, COUNTER_MD, BOUND_LINE, 2, 3, 4, heading),
        ],
        "{}",
        run.show()
    );
}

/// "spec bump" at `REV_MAX`: `LAW-BOUND` (`bound_at_max`) changed and
/// staged: `rev 999999999` alone, `4 lock entries`, `-`; JSON `rev`
/// 999999999, `to` and `write` null. M: a raise past the largest revision.
#[test]
fn rev_max_bump_row_is_rev_alone() {
    let repo = golden("w14-bump-max");
    bound_at_max(&repo);
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    stage_all(&repo);
    let run = bump(&repo, &["--staged"]);
    assert_eq!(run.code(), 0, "{}", run.show());
    assert_eq!(
        run.row_lines(),
        [bump_line_alone(
            "required", BOUND, COUNTER_MD, BOUND_LINE, MAX_REV, 4
        )],
        "{}",
        run.show()
    );
    assert_eq!(
        run.rows(),
        [&row_alone(
            "required", BOUND, COUNTER_MD, BOUND_LINE, MAX_REV, 4
        )],
        "{}",
        run.show()
    );
}

/// Iteration 2: the raises no line can write (`unwritable_raises`):
/// `GOAL-01` (flow-style front-matter) and `PIECE-COUNTER` (`rev: +3`)
/// changed on the disk: each `required`, `rev N` alone, `-`, JSON `to`
/// and `write` null, by path and line; `LAW-BOUND` changed with them
/// keeps `rev 2 -> 3` and its raised heading. M: `to` kept where no line
/// can be written (`rev 3 -> 4 | … | -`).
#[test]
fn unwritable_raise_bump_row_is_rev_alone() {
    let repo = golden("w14-bump-unwritable");
    unwritable_raises(&repo);
    repo.replace(GOAL_MD, GOAL_EDIT.0, GOAL_EDIT.1);
    repo.replace(COUNTER_MD, PIECE_EDIT.0, PIECE_EDIT.1);
    repo.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    let run = bump(&repo, &[]);
    assert_eq!(run.code(), 0, "{}", run.show());
    // A flow mapping: the line `spec check` gives it (`rev_rule.rs`).
    let goal_line = run.row(GOAL)["line"].as_u64().expect("a line");
    let heading = "## Bound {#LAW-BOUND rev=3}";
    assert_eq!(
        run.row_lines(),
        [
            bump_line_alone("required", GOAL, GOAL_MD, goal_line, 1, 1),
            bump_line_alone("required", PIECE, COUNTER_MD, PIECE_LINE, 3, 4),
            bump_line("required", BOUND, COUNTER_MD, BOUND_LINE, 2, 3, 4, heading),
        ],
        "{}",
        run.show()
    );
    assert_eq!(
        run.rows(),
        [
            &row_alone("required", GOAL, GOAL_MD, goal_line, 1, 1),
            &row_alone("required", PIECE, COUNTER_MD, PIECE_LINE, 3, 4),
            &row("required", BOUND, COUNTER_MD, BOUND_LINE, 2, 3, 4, heading),
        ],
        "{}",
        run.show()
    );
}

// ------------------------------- lock-levels-verify RD-9 (a), RD-12 (Q17)

/// The `required` hint when a row shows a line to write (both ways out).
const REQUIRED_HINT: &str = "hint: required: spec.lock vouches for the old text: write the line \
     shown and commit (a raise), or send the change through the queue (spec propose update) and \
     have the owner approve it with --editorial";

/// The `required` hint when no `required` row has a `write` (each shows
/// `-`): the queue alone.
const REQUIRED_HINT_EDITORIAL: &str = "hint: required: spec.lock vouches for the old text: send \
     the change through the queue (spec propose update) and have the owner approve it with \
     --editorial";

/// The run's `hint: required: …` line, exactly one.
fn required_hint(run: &rev_rule_common::Bump) -> String {
    let hints: Vec<&str> = run
        .all_lines()
        .into_iter()
        .filter(|line| line.starts_with("hint: required: "))
        .collect();
    assert_eq!(hints.len(), 1, "one required hint:\n{}", run.show());
    hints[0].to_owned()
}

/// docs/features/lock-levels-verify.md RD-9 (a), RD-12 (Q17): when no
/// `required` row has a line to write (`LAW-BOUND` at `REV_MAX`; a closed
/// class's `GOAL-01`; the unwritable raises of `GOAL-01` and
/// `PIECE-COUNTER`), the hint names the queue and `--editorial` alone, the
/// whole text pinned; as soon as one `required` row shows a line, both ways
/// out, today's whole text. M: the raise named where no line can be written
/// (both ways out), or dropped where one can (either way out named fails it).
#[test]
fn required_hint_without_a_line_names_only_editorial() {
    // REV_MAX: the one required row has no line.
    let max = golden("rd12-hint-max");
    bound_at_max(&max);
    max.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    let run = bump(&max, &[]);
    assert_eq!(run.code(), 0, "{}", run.show());
    assert!(
        run.rows().iter().all(|row| row["write"].is_null()),
        "the premise: no line to write\n{}",
        run.show()
    );
    assert_eq!(required_hint(&run), REQUIRED_HINT_EDITORIAL, "REV_MAX");

    // A closed class alone, then beside a writable raise.
    let closed = golden("rd12-hint-closed");
    close_goal(&closed);
    closed.replace(GOAL_MD, GOAL_EDIT.0, GOAL_EDIT.1);
    let alone = bump(&closed, &[]);
    assert_eq!(alone.code(), 0, "{}", alone.show());
    assert_eq!(
        alone.row_lines(),
        [bump_line_alone("required", GOAL, GOAL_MD, GOAL_LINE, 1, 1)],
        "{}",
        alone.show()
    );
    assert_eq!(
        required_hint(&alone),
        REQUIRED_HINT_EDITORIAL,
        "a closed class"
    );
    closed.replace(COUNTER_MD, BOUND_EDIT.0, BOUND_EDIT.1);
    let beside = bump(&closed, &[]);
    assert!(
        beside
            .rows()
            .iter()
            .any(|row| row["need"] == "required" && row["write"].is_string()),
        "the premise: a line to write\n{}",
        beside.show()
    );
    assert_eq!(required_hint(&beside), REQUIRED_HINT, "a line shown");

    // Unwritable raises alone.
    let unwritable = golden("rd12-hint-unwritable");
    unwritable_raises(&unwritable);
    unwritable.replace(GOAL_MD, GOAL_EDIT.0, GOAL_EDIT.1);
    unwritable.replace(COUNTER_MD, PIECE_EDIT.0, PIECE_EDIT.1);
    let run = bump(&unwritable, &[]);
    assert_eq!(run.code(), 0, "{}", run.show());
    assert!(
        run.rows()
            .iter()
            .filter(|row| row["need"] == "required")
            .all(|row| row["write"].is_null()),
        "the premise\n{}",
        run.show()
    );
    assert_eq!(
        required_hint(&run),
        REQUIRED_HINT_EDITORIAL,
        "unwritable raises"
    );
}

// ------------------------------------------- tested-evidence (8b-1) RD-7

/// RD-7 **bump_requires_a_raise_under_a_block** (Q-E: a block counts as
/// one entry; M: bump ignores a block): LAW-STEP's text changed without a
/// raise is `suggested` (unlocked) over the golden, `required` with `1
/// lock entry` once a block vouches for its text.
#[test]
fn bump_requires_a_raise_under_a_block() {
    let repo = golden("rd4-tested-none");
    repo.replace(COUNTER_MD, STEP_EDIT.0, STEP_EDIT.1);
    let run = bump(&repo, &[]);
    assert_eq!(
        run.needs(),
        [("LAW-STEP".to_owned(), "suggested".to_owned())],
        "{}",
        run.show()
    );

    let repo = golden("rd4-tested-block");
    let norm = repo.norm_hash("LAW-STEP");
    let text = repo.lock_text();
    repo.write(
        LOCK,
        tested_common::with_blocks_added(
            &text,
            &[tested_common::block_at(
                "LAW-STEP",
                1,
                &norm,
                '7',
                &repo.head(),
            )],
        ),
    );
    repo.commit_all("LAW-STEP's evidence");
    repo.replace(COUNTER_MD, STEP_EDIT.0, STEP_EDIT.1);
    let run = bump(&repo, &[]);
    assert_eq!(run.code(), 0, "{}", run.show());
    assert_eq!(
        run.needs(),
        [("LAW-STEP".to_owned(), "required".to_owned())],
        "{}",
        run.show()
    );
    assert_eq!(run.row("LAW-STEP")["lock_entries"], 1);
}
