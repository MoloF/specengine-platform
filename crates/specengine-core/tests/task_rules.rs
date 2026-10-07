//! docs/features/task-package.md, core's words of a task ("Data"): the
//! pure `transition` against the table written out here (every state of
//! the ten, every action; `analysis`, `in_review`, `accepted` never
//! entered and never left), the "needs" message, the caps of
//! docs/canon/task-package.md "Caps", the
//! `T-NNNN` ID and its look-alikes (ADR-0009), `T` in `[ids]`, `[project]
//! profile` (at most 64 bytes, verbatim, else an error at its line) and
//! `[budgets] bundle_task` (read as narrowly as `bundle_node`).

use specengine_core::check::bundle_task_from_toml;
use specengine_core::task::{
    AFFECTED_MAX, CHANGED_FILE_MAX, CHANGED_FILES_MAX, CRITERIA_MAX, CRITERION_MAX,
    DEFAULT_TASK_BUDGET, DIFF_MAX, DIFFS_TOTAL_MAX, GOAL_MAX, NODES_MAX, NOTE_MAX, PLAN_MAX,
    RUN_SUMMARY_MAX, SNAPSHOT_MAX, TITLE_MAX, TaskAction, TaskIdError, needs_message,
    parse_task_id, run_outcome, task_id, task_number, task_prefix_clash, transition,
};
use specengine_core::{ProjectConfig, project_from_toml};
use specengine_model::{RunOutcome, TaskStatus};

/// "Data"'s table: `(action, from, to)`.
const TABLE: [(TaskAction, &[&str], &str); 7] = [
    (TaskAction::Plan, &["draft", "changes_requested"], "review"),
    (
        TaskAction::Approve,
        &["draft", "review", "changes_requested", "ready"],
        "ready",
    ),
    (TaskAction::Changes, &["review"], "changes_requested"),
    (TaskAction::Claim, &["ready"], "in_progress"),
    (TaskAction::Report, &["in_progress"], "in_progress"),
    (TaskAction::Complete, &["in_progress"], "done"),
    (
        TaskAction::Cancel,
        &[
            "draft",
            "review",
            "changes_requested",
            "ready",
            "in_progress",
        ],
        "cancelled",
    ),
];

/// Every (state, action) pair of the ten states and seven actions on a
/// task: the table's go to its state, every other is refused with the
/// states it needs; `new` only without a task. M: claim accepts `review`.
#[test]
fn the_transition_table_is_datas() {
    assert_eq!(TaskStatus::ALL.len(), 10);
    let mut allowed = 0;
    for status in TaskStatus::ALL {
        assert_eq!(TaskStatus::parse(status.as_str()), Some(status));
        assert!(
            transition(Some(status), TaskAction::New).is_err(),
            "{status}: new"
        );
        for (action, from, to) in TABLE {
            let got = transition(Some(status), action);
            if from.contains(&status.as_str()) {
                assert_eq!(
                    got.map(TaskStatus::as_str),
                    Ok(to),
                    "{status}: {}",
                    action.as_str()
                );
                allowed += 1;
            } else {
                let needs = got.expect_err("refused");
                assert_eq!(
                    needs.iter().map(|state| state.as_str()).collect::<Vec<_>>(),
                    from,
                    "{status}: {}",
                    action.as_str()
                );
            }
        }
    }
    assert_eq!(allowed, 2 + 4 + 1 + 1 + 1 + 1 + 5);
    assert_eq!(transition(None, TaskAction::New), Ok(TaskStatus::Draft));
    assert!(transition(None, TaskAction::Claim).is_err());
    for never in [
        TaskStatus::Analysis,
        TaskStatus::InReview,
        TaskStatus::Accepted,
    ] {
        for (action, _, _) in TABLE {
            assert!(transition(Some(never), action).is_err(), "{never}");
        }
        assert!(
            !TABLE.iter().any(|(_, _, to)| *to == never.as_str()),
            "{never} is never entered"
        );
    }
    assert_eq!(
        needs_message(
            "T-0001",
            TaskStatus::Done,
            TaskAction::Cancel,
            TaskAction::Cancel.from()
        ),
        "T-0001 is done: `cancel` needs draft, review, changes_requested, ready or \
         in_progress; nothing changed"
    );
    assert_eq!(
        needs_message(
            "T-0002",
            TaskStatus::Review,
            TaskAction::Claim,
            &[TaskStatus::Ready]
        ),
        "T-0002 is review: `claim` needs ready; nothing changed"
    );
    assert!(TaskStatus::Done.is_closed() && TaskStatus::Cancelled.is_closed());
    assert!(!TaskStatus::InProgress.is_closed());
}

/// docs/canon/task-package.md "Caps": the numbers and the run outcomes.
#[test]
fn the_caps_and_outcomes_are_datas() {
    assert_eq!(
        [
            TITLE_MAX,
            GOAL_MAX,
            PLAN_MAX,
            CRITERIA_MAX,
            CRITERION_MAX,
            NODES_MAX,
            AFFECTED_MAX,
            SNAPSHOT_MAX,
            NOTE_MAX,
            RUN_SUMMARY_MAX,
            CHANGED_FILES_MAX,
            CHANGED_FILE_MAX,
            DIFF_MAX,
            DIFFS_TOTAL_MAX,
        ],
        [
            256, 4096, 16_384, 32, 1024, 64, 64, 128, 4096, 4096, 256, 512, 8192, 262_144
        ]
    );
    assert_eq!(DEFAULT_TASK_BUDGET, 10_000);
    for (written, outcome) in [
        ("completed", RunOutcome::Completed),
        ("partial", RunOutcome::Partial),
        ("failed", RunOutcome::Failed),
        ("abandoned", RunOutcome::Abandoned),
    ] {
        assert_eq!(run_outcome(written), Ok(outcome));
    }
    assert_eq!(
        run_outcome("done"),
        Err("outcome: `done` is not `completed`, `partial`, `failed` or `abandoned`".to_owned())
    );
    assert!(run_outcome("Completed").is_err());
}

/// Task IDs: `T-` and four or more digits, as written (`T-0001`,
/// `T-10000`; never `T-001`, `T-00001`, `t-0001`, `T0001`); a look-alike
/// (Cyrillic `\u{0422}`, a fullwidth digit) refused with its Latin form;
/// other text no ID. Mixed scripts never pass (ADR-0009).
#[test]
fn task_ids_are_latin_and_look_alikes_name_their_fix() {
    assert_eq!(task_id(1), "T-0001");
    assert_eq!(task_id(12_345), "T-12345");
    for (written, number) in [("T-0001", Some(1)), ("T-10000", Some(10_000))] {
        assert_eq!(task_number(written), number, "{written}");
        assert_eq!(
            parse_task_id(&format!(" {written} ")),
            Ok(written.to_owned())
        );
    }
    for written in [
        "T-001", "T-00001", "t-0001", "T0001", "T-", "T-12a4", "PR-0001", "",
    ] {
        assert_eq!(task_number(written), None, "{written}");
        assert_eq!(
            parse_task_id(written),
            Err(TaskIdError::NotAnId),
            "{written}"
        );
    }
    for written in ["\u{0422}-0001", "T-000\u{FF11}"] {
        assert_eq!(
            parse_task_id(written),
            Err(TaskIdError::LookAlike {
                fix: "T-0001".to_owned()
            }),
            "{written:?}"
        );
    }
}

/// `T` as a prefix of `[ids]`, or in an `aliases_from` (a look-alike `T`
/// too), clashes with the engine's task prefix; `TK`, `ST` do not.
#[test]
fn t_in_ids_clashes_with_the_task_prefix() {
    let scheme = |ids: &str| {
        ProjectConfig::from_toml(&format!("[project]\nslug = \"p\"\n\n[ids]\n{ids}\n"))
            .expect("a config")
            .scheme
    };
    for ids in [
        "T = { kind = \"ticket\", width = 4 }",
        "TK = { kind = \"ticket\", width = 4, aliases_from = [\"T\"] }",
        "TK = { kind = \"ticket\", width = 4, aliases_from = [\"\u{0422}\"] }",
    ] {
        let clash = task_prefix_clash(&scheme(ids)).unwrap_or_else(|| panic!("{ids}"));
        assert!(clash.contains("task prefix"), "{clash}");
    }
    for ids in [
        "TK = { kind = \"ticket\", width = 4 }",
        "ST = { kind = \"step\", shape = \"name\", aliases_from = [\"TS\"] }",
    ] {
        assert_eq!(task_prefix_clash(&scheme(ids)), None, "{ids}");
    }
}

/// `[project] profile`: optional, verbatim (any UTF-8, 64 bytes at most,
/// counted in bytes), never parsed; 65 bytes or a non-string an error at
/// its line.
#[test]
fn the_profile_is_at_most_64_bytes_verbatim() {
    let config = |profile: &str| format!("[project]\nslug = \"p\"\nprofile = {profile}\n");
    let loaded = project_from_toml("[project]\nslug = \"p\"\n").unwrap();
    assert_eq!(loaded.project.profile, None);
    for profile in [
        "web service; tests: unit".to_owned(),
        "p".repeat(64),
        "\u{0436}".repeat(32),
    ] {
        let loaded = project_from_toml(&config(&format!("\"{profile}\""))).unwrap();
        assert_eq!(loaded.project.profile.as_deref(), Some(profile.as_str()));
    }
    for profile in [
        format!("\"{}\"", "p".repeat(65)),
        format!("\"{}p\"", "\u{0436}".repeat(32)),
    ] {
        let error = project_from_toml(&config(&profile)).expect_err(&profile);
        assert_eq!(error.line, Some(3), "{profile}: {}", error.message);
        assert!(
            error
                .message
                .contains("`profile`: 65 bytes; a profile has at most 64"),
            "{}",
            error.message
        );
    }
    // A wrong type: at its line, as every `[project]` key's.
    let error = project_from_toml(&config("7")).expect_err("a number");
    assert_eq!(error.line, Some(3), "{}", error.message);
}

/// `[budgets] bundle_task`: absent → `None`; 1 to `u32::MAX` → its tokens
/// and line; 0, a negative, past `u32::MAX` or a string → an error at its
/// line; another table's defect never stops it.
#[test]
fn bundle_task_is_read_as_narrowly_as_bundle_node() {
    assert_eq!(
        bundle_task_from_toml("[project]\nslug = \"p\"\n").unwrap(),
        None
    );
    let found = bundle_task_from_toml("[budgets]\nbundle_task = 2000\n")
        .unwrap()
        .expect("a budget");
    assert_eq!((found.tokens, found.line), (2000, 2));
    let found = bundle_task_from_toml(&format!(
        "[classes]\nbogus = 1\n[budgets]\nbundle_node = 0\nbundle_task = {}\n",
        u32::MAX
    ))
    .unwrap()
    .expect("a budget");
    assert_eq!(found.tokens, u32::MAX);
    for value in ["0", "-1", "4294967296", "\"many\""] {
        let error =
            bundle_task_from_toml(&format!("[budgets]\nbundle_task = {value}\n")).expect_err(value);
        assert_eq!(error.line, Some(2), "{value}: {}", error.message);
        assert!(error.message.contains("bundle_task"), "{}", error.message);
    }
}
