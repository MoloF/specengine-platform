//! docs/features/roadmap-position.md (Phase 6, slice 2a) RD-1: core's
//! pure join of a plan with a repository's task rows (`plan_tasks`,
//! canon `spec-roadmap` "Position") over the real plans of two projects of
//! different nature, `fixtures/roadmap-a` (a harbor game) and
//! `fixtures/roadmap-b` (a lending desk), parsed by the real parser with
//! their own `[ids]` and `[roadmap]`.
//!
//! - Q-C: an open task executes the first open item among its targets in
//!   `spec roadmap`'s listing order (releases in config order, each by
//!   position, then the pool, then unplaced), never the first target as
//!   written; with only done items, non-item nodes or nothing it joins the
//!   pool.
//! - Q-B: open means not `done`, `accepted` or `cancelled`; an item is in
//!   progress when one of its tasks is `in_progress`.
//! - Determinism: tasks by number (numerically, `T-9999` before
//!   `T-10000`), any row order giving one result.
//!
//! Named mutations: M1 (the first target as written used) turns
//! `best_placed_target_is_the_item` red; M2 (the result following the
//! rows' order) `position_ignores_task_row_order`.

mod common;

use common::check::fixture_input;
use common::fixture;
use specengine_core::IdSchemeToml;
use specengine_core::roadmap::{Plan, PlanTask, PlanTasks, plan, plan_tasks};
use specengine_core::roadmap_toml::RoadmapConfig;
use specengine_model::{IdScheme, TaskStatus};

// ------------------------------------------------------------------ inputs

/// The plan of `fixtures/<name>` over its own config.
fn fixture_plan(name: &str) -> Plan {
    let corpus = fixture(name);
    let toml = std::fs::read_to_string(corpus.join("specengine.toml")).expect("specengine.toml");
    let scheme = IdScheme::from_toml(&toml)
        .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    let roadmap = RoadmapConfig::from_toml(&toml, &scheme)
        .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")))
        .expect("a [roadmap] table");
    let (_, input) = fixture_input(&corpus);
    plan(&input, &roadmap)
}

/// A task row `id` in `status` targeting `targets` as written.
fn row(id: &str, status: TaskStatus, targets: &[&str]) -> PlanTask {
    PlanTask {
        id: id.to_owned(),
        status,
        title: Some(format!("Task {id}")),
        targets: targets.iter().map(|target| (*target).to_owned()).collect(),
    }
}

/// A `ready` row.
fn ready(id: &str, targets: &[&str]) -> PlanTask {
    row(id, TaskStatus::Ready, targets)
}

/// The item a lone open task targeting `targets` executes, `None` for the
/// pool; the pool and the items agree.
#[track_caller]
fn item_of(plan: &Plan, targets: &[&str]) -> Option<String> {
    let joined = plan_tasks(plan, &[ready("T-0001", targets)]);
    let items: Vec<&String> = joined.items.keys().collect();
    match (items.as_slice(), joined.pool.as_slice()) {
        ([item], []) => {
            assert_eq!(joined.of(item).len(), 1, "{targets:?}: {joined:?}");
            Some((*item).clone())
        }
        ([], [task]) => {
            assert_eq!(task.id, "T-0001");
            None
        }
        _ => panic!("{targets:?}: one place for one task: {joined:?}"),
    }
}

/// The IDs of tasks, in order.
fn ids(tasks: &[PlanTask]) -> Vec<&str> {
    tasks.iter().map(|task| task.id.as_str()).collect()
}

/// Every open item of `plan`, in the listing's order.
fn listing(plan: &Plan) -> Vec<String> {
    plan.groups
        .iter()
        .flat_map(|group| &group.items)
        .map(|item| item.id.clone())
        .collect()
}

// ------------------------------------------------------------------- RD-1

/// M1 (Q-C): of a task's targets the best placed item is the one it
/// executes: an earlier release over a later one whatever the priority,
/// a smaller position in a release, a release over the pool, the pool over
/// unplaced, the pool's own order (key: priority rank, then ID); in every
/// case the first target as written is not it. Both fixtures.
#[test]
fn best_placed_target_is_the_item() {
    let a = fixture_plan("roadmap-a");
    let cases_a: [(&[&str], &str); 9] = [
        // unplaced, pool, launch 4, beta 5, beta 1.
        (
            &["RM-0050", "RM-0041", "RM-0022", "RM-0011", "RM-0017"],
            "RM-0017",
        ),
        // launch over the pool.
        (&["RM-0041", "RM-0030"], "RM-0030"),
        // the pool over unplaced.
        (&["RM-0050", "RM-0040"], "RM-0040"),
        // beta 6 then beta 1.
        (&["RM-0012", "RM-0017"], "RM-0017"),
        // launch 1 (must) after beta 8 (off-scale priority): the release.
        (&["RM-0030", "RM-0018"], "RM-0018"),
        // a cycle's members: launch 3 then launch 2.
        (&["RM-0020", "RM-0021"], "RM-0021"),
        // the pool by key: RM-0041 (no priority) after RM-0040 (should).
        (&["RM-0041", "RM-0040"], "RM-0040"),
        // an item outside dir (launch 5) after launch 4.
        (&["RM-0060", "RM-0022"], "RM-0022"),
        // the chain in beta: 5 then 4.
        (&["RM-0011", "RM-0010"], "RM-0010"),
    ];
    for (targets, want) in cases_a {
        assert_ne!(targets[0], want, "the case must tell M1 apart");
        assert_eq!(item_of(&a, targets).as_deref(), Some(want), "{targets:?}");
    }
    // A target alone is its item, wherever it sits.
    for id in listing(&a) {
        assert_eq!(item_of(&a, &[&id]).as_deref(), Some(id.as_str()));
    }

    let b = fixture_plan("roadmap-b");
    let cases_b: [(&[&str], &str); 5] = [
        // unplaced, later 1, next 10.
        (&["WORK-051", "WORK-030", "WORK-016"], "WORK-016"),
        // later 2 (outside dir) then next 7.
        (&["WORK-031", "WORK-010"], "WORK-010"),
        // the pool: WORK-041 (no priority) after WORK-040 (p0).
        (&["WORK-041", "WORK-040"], "WORK-040"),
        // unplaced by key: WORK-050 (p1) after WORK-051 (p0).
        (&["WORK-050", "WORK-051"], "WORK-051"),
        // next 5 then next 4 (a cycle).
        (&["WORK-021", "WORK-020"], "WORK-020"),
    ];
    for (targets, want) in cases_b {
        assert_ne!(targets[0], want, "the case must tell M1 apart");
        assert_eq!(item_of(&b, targets).as_deref(), Some(want), "{targets:?}");
    }
    for id in listing(&b) {
        assert_eq!(item_of(&b, &[&id]).as_deref(), Some(id.as_str()));
    }
}

/// The rows of the order cells: tasks on one item, on another, in the
/// pool, closed ones, numbers of four and five digits.
fn order_rows() -> Vec<PlanTask> {
    vec![
        row("T-0001", TaskStatus::Draft, &["RM-0011"]),
        row("T-0002", TaskStatus::InProgress, &["RM-0041", "RM-0011"]),
        row("T-0003", TaskStatus::Ready, &["BAY-HARBOR"]),
        row("T-0004", TaskStatus::Review, &["RM-0030"]),
        row("T-0005", TaskStatus::Done, &["RM-0011"]),
        row("T-0006", TaskStatus::ChangesRequested, &["RM-0001"]),
        row("T-0010", TaskStatus::Ready, &["RM-0011"]),
        row("T-0011", TaskStatus::Cancelled, &["BAY-HARBOR"]),
        row("T-9999", TaskStatus::InProgress, &["RM-0030", "RM-0050"]),
        row("T-10000", TaskStatus::Draft, &["RM-0030"]),
        row("T-10001", TaskStatus::Draft, &[]),
        row("T-0099", TaskStatus::Accepted, &["RM-0030"]),
    ]
}

/// M2: the rows in their own order, reversed, rotated and shuffled give
/// one result, field for field.
#[test]
fn position_ignores_task_row_order() {
    let a = fixture_plan("roadmap-a");
    let rows = order_rows();
    let want = plan_tasks(&a, &rows);
    assert_eq!(
        ids(want.of("RM-0011")),
        ["T-0001", "T-0002", "T-0010"],
        "{want:?}"
    );
    assert_eq!(
        ids(want.of("RM-0030")),
        ["T-0004", "T-9999", "T-10000"],
        "{want:?}"
    );
    assert_eq!(ids(&want.pool), ["T-0003", "T-0006", "T-10001"], "{want:?}");

    let mut reversed = rows.clone();
    reversed.reverse();
    let mut orders = vec![reversed];
    for shift in 1..rows.len() {
        let mut rotated = rows.clone();
        rotated.rotate_left(shift);
        orders.push(rotated);
    }
    // A fixed shuffle: every fifth row, from each start.
    let shuffled: Vec<PlanTask> = (0..5)
        .flat_map(|start| rows.iter().skip(start).step_by(5).cloned())
        .collect();
    assert_eq!(shuffled.len(), rows.len());
    orders.push(shuffled);
    for order in orders {
        let got = plan_tasks(&a, &order);
        assert_eq!(got, want, "rows {:?}", ids(&order));
    }
    // The input is left as it was.
    assert_eq!(ids(&rows), ids(&order_rows()));
}

/// Q-C: a task whose targets are done items only (shipped, abandoned; in
/// a release and in the pool) joins the pool; a done target before an
/// open one is skipped for it.
#[test]
fn done_item_targets_join_the_pool() {
    let a = fixture_plan("roadmap-a");
    for done in ["RM-0001", "RM-0002", "RM-0019", "RM-0031", "RM-0042"] {
        assert!(!listing(&a).contains(&done.to_owned()), "{done} is done");
        assert_eq!(item_of(&a, &[done]), None, "{done}");
    }
    assert_eq!(
        item_of(&a, &["RM-0001", "RM-0002", "RM-0019", "RM-0031", "RM-0042"]),
        None
    );
    assert_eq!(
        item_of(&a, &["RM-0019", "RM-0041"]).as_deref(),
        Some("RM-0041")
    );
    assert_eq!(
        item_of(&a, &["RM-0001", "RM-0050"]).as_deref(),
        Some("RM-0050")
    );

    let b = fixture_plan("roadmap-b");
    for done in ["WORK-001", "WORK-002", "WORK-019", "WORK-033", "WORK-042"] {
        assert!(!listing(&b).contains(&done.to_owned()), "{done} is done");
        assert_eq!(item_of(&b, &[done]), None, "{done}");
    }
    assert_eq!(
        item_of(&b, &["WORK-033", "WORK-032"]).as_deref(),
        Some("WORK-032")
    );
}

/// Q-C: targets that are no item (a non-item document, a section with
/// the item prefix, a path, an ID naming nothing, a legacy-looking
/// spelling, no target at all) leave a task in the pool; one item among
/// them is its item.
#[test]
fn non_item_targets_join_the_pool() {
    let a = fixture_plan("roadmap-a");
    for targets in [
        &["BAY-HARBOR"][..],
        &["RM-0090"],
        &["docs/spec/tides.md"],
        &["plan/RM-0011.md"],
        &["RM-0099"],
        &["rm-0011"],
        &["RM-11"],
        &[],
        &["BAY-HARBOR", "RM-0090", "RM-0099"],
    ] {
        assert_eq!(item_of(&a, targets), None, "{targets:?}");
    }
    assert_eq!(
        item_of(&a, &["BAY-HARBOR", "RM-0090", "RM-0040"]).as_deref(),
        Some("RM-0040")
    );
    // An item outside `[roadmap] dir` is still an item.
    assert_eq!(
        item_of(&a, &["BAY-HARBOR", "RM-0060"]).as_deref(),
        Some("RM-0060")
    );

    let b = fixture_plan("roadmap-b");
    for targets in [&["SHELF-LOANS"][..], &["SHELF-WAIVER"], &["WORK-999"], &[]] {
        assert_eq!(item_of(&b, targets), None, "{targets:?}");
    }
}

/// Tasks by number, numerically (`T-9999` before `T-10000`), on an item
/// and in the pool; closed rows (`done`, `accepted`, `cancelled`) left
/// out; every other state is open; the item is in progress only with an
/// `in_progress` task.
#[test]
fn tasks_by_number() {
    let a = fixture_plan("roadmap-a");
    let rows = vec![
        ready("T-10000", &["RM-0013"]),
        row("T-0002", TaskStatus::Draft, &["RM-0013"]),
        ready("T-9999", &["RM-0013"]),
        row("T-0010", TaskStatus::Review, &["RM-0013"]),
        ready("T-10002", &["BAY-HARBOR"]),
        ready("T-0300", &["BAY-HARBOR"]),
        ready("T-1000", &[]),
    ];
    let joined = plan_tasks(&a, &rows);
    assert_eq!(
        ids(joined.of("RM-0013")),
        ["T-0002", "T-0010", "T-9999", "T-10000"]
    );
    assert_eq!(ids(&joined.pool), ["T-0300", "T-1000", "T-10002"]);
    assert_eq!(joined.items.len(), 1, "{joined:?}");
    assert!(!joined.in_progress("RM-0013"), "no task is in_progress");
    // The rows as given are carried whole.
    assert_eq!(joined.of("RM-0013")[0], rows[1]);

    // Each state alone: the closed three leave nothing.
    for status in TaskStatus::ALL {
        let joined = plan_tasks(&a, &[row("T-0001", status, &["RM-0013"])]);
        let closed = matches!(
            status,
            TaskStatus::Done | TaskStatus::Accepted | TaskStatus::Cancelled
        );
        assert_eq!(joined.is_empty(), closed, "{status}: {joined:?}");
        assert_eq!(
            joined.in_progress("RM-0013"),
            status == TaskStatus::InProgress,
            "{status}"
        );
        let pooled = plan_tasks(&a, &[row("T-0001", status, &["BAY-HARBOR"])]);
        assert_eq!(pooled.pool.is_empty(), closed, "{status}: pool");
    }

    // One `in_progress` among others marks the item; another item stays.
    let joined = plan_tasks(
        &a,
        &[
            ready("T-0001", &["RM-0013"]),
            row("T-0002", TaskStatus::InProgress, &["RM-0013"]),
            ready("T-0003", &["RM-0014"]),
            row("T-0004", TaskStatus::InProgress, &["BAY-HARBOR"]),
        ],
    );
    assert!(joined.in_progress("RM-0013"));
    assert!(!joined.in_progress("RM-0014"));
    assert!(
        !joined.in_progress("BAY-HARBOR"),
        "a pool task marks no item"
    );
    assert!(!joined.in_progress("RM-0017"));
    assert_eq!(joined.of("RM-0017"), &[] as &[PlanTask]);

    // No rows: nothing.
    assert_eq!(plan_tasks(&a, &[]), PlanTasks::default());
}
