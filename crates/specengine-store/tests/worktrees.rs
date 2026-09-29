//! AC-18 of docs/features/spec-index.md: two worktrees of one project (two
//! scratch copies of spec-a, the same paths) in one DB; updating, updating
//! by path or rebuilding one leaves the other's dump unchanged, and each
//! handle reads only its own worktree.

#![cfg(unix)]

mod common;

use common::{Corpus, Scratch};
use specengine_store::{IndexWriter, SpecIndex};

#[test]
fn writing_one_worktree_never_changes_another_in_the_same_db() {
    let scratch = Scratch::new("worktrees");
    let main = Corpus::copy_of("spec-a", &scratch, "main");
    let task = Corpus::copy_of("spec-a", &scratch, "task");
    let db = scratch.db("index");
    let mut main_index = main.open(&db);
    let mut task_index = task.open(&db);
    main.update(&mut main_index);
    task.update(&mut task_index);
    let main_dump = main_index.dump_worktree().expect("dump");
    assert!(main_dump.contains(main_index.root().to_str().unwrap()));
    assert!(!main_dump.contains(task_index.root().to_str().unwrap()));

    // Edit, add and delete in the task worktree; update it.
    task.replace(
        "docs/spec/movement/stamina.md",
        "Base rate 10 units/s",
        "Base rate 14 units/s",
    );
    task.write(
        "docs/spec/lanterns.md",
        "# Lanterns {#MEC-LANTERN}\n\nLight.\n",
    );
    task.remove("docs/records/Q/Q-032.md");
    let report = task.update(&mut task_index);
    assert_eq!(report.parsed, 2, "{report:?}");
    assert_eq!(report.removed, 1, "{report:?}");
    assert_eq!(
        main_index.dump_worktree().expect("dump"),
        main_dump,
        "after update"
    );
    assert_equals_fresh_worktree(&task_index, &task, &scratch);

    // By path, including a deletion.
    task.remove("docs/records/Q/Q-031.md");
    task_index
        .update_paths(&task.tree(), &task.scheme, &["docs/records/Q/Q-031.md"])
        .expect("update_paths");
    assert_eq!(
        main_index.dump_worktree().expect("dump"),
        main_dump,
        "after update_paths"
    );

    // A rebuild.
    task_index
        .rebuild(&task.tree(), &task.scheme)
        .expect("rebuild");
    assert_eq!(
        main_index.dump_worktree().expect("dump"),
        main_dump,
        "after rebuild"
    );
    assert!(
        main_index
            .files()
            .expect("files")
            .contains(&"docs/records/Q/Q-032.md".to_owned())
    );
    assert!(
        !task_index
            .files()
            .expect("files")
            .contains(&"docs/records/Q/Q-032.md".to_owned())
    );
    assert_eq!(
        main_index.lookup_id("MEC-LANTERN").expect("lookup").len(),
        0,
        "main does not see the task's node"
    );
    assert_eq!(
        task_index.lookup_id("MEC-LANTERN").expect("lookup").len(),
        1
    );

    // And the other way round.
    let task_dump = task_index.dump_worktree().expect("dump");
    main_index
        .rebuild(&main.tree(), &main.scheme)
        .expect("rebuild");
    main.remove("docs/spec/game.md");
    main.update(&mut main_index);
    assert_eq!(
        task_index.dump_worktree().expect("dump"),
        task_dump,
        "after main's writes"
    );
    assert_equals_fresh_worktree(&main_index, &main, &scratch);
}

/// The worktree's own dump equals the one of a fresh DB holding only it.
fn assert_equals_fresh_worktree(
    index: &specengine_store::SqliteIndex,
    corpus: &Corpus,
    scratch: &Scratch,
) {
    let fresh = corpus.fresh(scratch);
    assert_eq!(
        index.dump_worktree().expect("dump"),
        fresh.dump_worktree().expect("dump"),
        "{}: the worktree dump differs from a fresh index",
        corpus.root.display()
    );
}
