//! docs/features/reference-promotion.md (RM-0138, ADR-0089) "Acceptance
//! criteria", the store's half of PR11 and PR12: `SqliteQueue::open` and
//! `SqliteIndex` through the public API, older schemas and raw rows through
//! the system's `sqlite3`.
//!
//! - PR11: `QUEUE_SCHEMA_VERSION` 17; a new queue's `proposals` holds 52
//!   columns, `ref_id TEXT` last, `PROPOSAL_COLUMNS` equal to `PRAGMA
//!   table_info`, the index `proposals_ref (project, ref_id)`. A schema-16
//!   queue with proposals, cards, comments and refs steps to 17 at its first
//!   open in one transaction, every row as it was, `ref_id` NULL; a second
//!   open steps nothing; a failure inside the step (a hand-made index named
//!   `proposals_ref`, the step's last statement) leaves 16 and no `ref_id`;
//!   18 is `SchemaTooNew`. A `reference` row without `ref_id` (or not an RF
//!   ID), an `update` with one, and a `reference` whose `new_text` is no
//!   entry the key keeps are corrupt rows naming `ref_id` (the last, its
//!   text). M: `ref_id` left out of the dump (the dump's own check is the
//!   CLI's; here the row round trip).
//! - PR12: `promoted_of` lists each asked document's kept entries,
//!   root-relative, in written order, the documents in the order asked (one
//!   asked twice once), none for no document; an edited key replaces its rows
//!   at the next `update`, a deleted file drops them. M: `update` skipping
//!   `promoted` for a changed file.
//!
//! Every expected value is worked out from the spec's DDL, never read back
//! from a run.

#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{Corpus, Scratch};
use specengine_store::{
    PROPOSAL_COLUMNS, PromotedRow, ProposalQueue as _, QUEUE_SCHEMA_VERSION, QueueError,
    SqliteQueue,
};

const PROJECT: &str = "lantern-keep";

fn sqlite3(db: &Path, sql: &str) -> String {
    let client = ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
        .expect("a sqlite3 client");
    let output = Command::new(client)
        .arg(db)
        .arg(sql)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn user_version(db: &Path) -> String {
    sqlite3(db, "PRAGMA user_version;")
}

fn column_names(db: &Path, table: &str) -> Vec<String> {
    sqlite3(
        db,
        &format!("SELECT name FROM pragma_table_info('{table}') ORDER BY cid;"),
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

/// `(table, columns)` of the index `name`, its columns one per line.
fn index_of(db: &Path, name: &str) -> (String, String) {
    (
        sqlite3(
            db,
            &format!(
                "SELECT tbl_name FROM sqlite_master WHERE type = 'index' AND name = '{name}';"
            ),
        ),
        sqlite3(
            db,
            &format!("SELECT name FROM pragma_index_info('{name}') ORDER BY seqno;"),
        ),
    )
}

fn objects(db: &Path) -> String {
    sqlite3(
        db,
        "SELECT type || ' ' || name FROM sqlite_master ORDER BY type, name;",
    )
}

/// PR11: 17; `proposals` of 52 columns, `ref_id TEXT` last and nullable,
/// equal to `PROPOSAL_COLUMNS`; `proposals_ref` on `(project, ref_id)`.
#[test]
fn pr11_a_new_queue_stands_at_17_with_ref_id() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 17);
    assert_eq!(PROPOSAL_COLUMNS.len(), 52);
    assert_eq!(PROPOSAL_COLUMNS.last(), Some(&"ref_id"));
    let scratch = Scratch::new("pr-pr11-new");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    assert_eq!(user_version(&db), "17");
    assert_eq!(column_names(&db, "proposals"), PROPOSAL_COLUMNS);
    assert_eq!(
        sqlite3(
            &db,
            "SELECT cid || ' ' || name || ' ' || type || ' ' || \"notnull\" || ' ' || pk \
             FROM pragma_table_info('proposals') WHERE name = 'ref_id';"
        ),
        "51 ref_id TEXT 0 0"
    );
    assert_eq!(
        index_of(&db, "proposals_ref"),
        ("proposals".to_owned(), "project\nref_id".to_owned())
    );
}

/// Rows of every queue table of schema 16, written raw: two proposals (one
/// naming a comment), a card, a task, a run, a comment, a comment version,
/// two references and four events.
fn some_rows(db: &Path) {
    sqlite3(
        db,
        "INSERT INTO proposals (id, project, kind, status, target_id, target_path, \
         git_common_dir, worktree, root_rel, branch, base_commit, author, diagnostics, \
         created_at, updated_at, summary, card_id) VALUES ('PR-0001', 'lantern-keep', \
         'question', 'open', 'MEC-SPRINT', 'docs/spec/movement/sprint.md', \
         '/srv/keep/.git', '/srv/keep', '', 'main', \
         '03d1f64839f2b9554ffeb4e41e3335e7669ca358', \
         '{\"type\":\"agent\",\"role\":\"developer\",\"model\":null,\"run\":null}', '[]', \
         '2026-11-22T09:00:00Z', '2026-11-22T09:00:00Z', 'Does a sprint end at zero?', 'C-0001'); \
         INSERT INTO proposals (id, project, kind, status, target_id, base_text, new_text, \
         created_at, updated_at, comment_id) VALUES ('PR-0002', 'lantern-keep', 'update', \
         'rejected', 'MEC-STAMINA', 'old', 'new', '2026-11-22T09:01:00Z', \
         '2026-11-22T09:02:00Z', 'CM-0001'); \
         INSERT INTO cards (id, project, flow, col, position, title, targets, created_by, \
         created_at, updated_at, revision, tasks) VALUES ('C-0001', 'lantern-keep', 'plain', \
         'backlog', 1, 'Write the sprint foreword', '[\"MEC-SPRINT\"]', 'local', \
         '2026-11-22T08:59:00Z', '2026-11-22T08:59:00Z', '2', '[\"T-0001\"]'); \
         INSERT INTO tasks (id, project, status, title, targets, created_at, updated_at, \
         revision) VALUES ('T-0001', 'lantern-keep', 'draft', 'Tune the sprint', \
         '[\"MEC-SPRINT\"]', '2026-11-22T08:58:00Z', '2026-11-22T08:58:00Z', '1'); \
         INSERT INTO runs (task_id, run, role, outcome) VALUES ('T-0001', 1, 'developer', \
         'done'); \
         INSERT INTO comments (id, project, on_kind, on_id, on_rev, reply_to, actor, author, \
         version, body, mentions, created_at, updated_at) VALUES ('CM-0001', 'lantern-keep', \
         'card', 'C-0001', NULL, NULL, 'mira', \
         '{\"type\":\"human\",\"role\":null,\"model\":null,\"run\":null}', 2, \
         'Keep the rail at 1.1 m?', '[]', '2026-11-22T09:03:00Z', '2026-11-22T09:04:00Z'); \
         INSERT INTO comment_versions (comment_id, project, version, body, written_at) VALUES \
         ('CM-0001', 'lantern-keep', 1, 'Keep the rail at 1.2 m?', '2026-11-22T09:03:00Z'); \
         INSERT INTO refs (id, project, on_kind, on_id, on_rev, actor, author, kind, title, \
         matters, url, name, b3, agent_b3, size, media, meta, created_at) VALUES \
         ('RF-0001', 'lantern-keep', 'card', 'C-0001', NULL, 'mira', \
         '{\"type\":\"human\",\"role\":null,\"model\":null,\"run\":null}', 'link', 'Board', \
         NULL, 'https://example.org/board', NULL, NULL, NULL, NULL, NULL, NULL, \
         '2026-11-22T09:06:00Z'), \
         ('RF-0002', 'lantern-keep', 'card', 'C-0001', NULL, 'lev', \
         '{\"type\":\"human\",\"role\":null,\"model\":null,\"run\":null}', 'file', NULL, \
         'the light', NULL, 'harbour.png', \
         'b3:0000000000000000000000000000000000000000000000000000000000000001', NULL, 1559, \
         'image/png', '{\"width\":64,\"height\":48}', '2026-11-22T09:07:00Z'); \
         INSERT INTO events (seq, project, type, payload, at) VALUES \
         (1, 'lantern-keep', 'card.created', '{\"id\":\"C-0001\"}', '2026-11-22T08:59:00Z'), \
         (2, 'lantern-keep', 'proposal.created', '{\"id\":\"PR-0001\"}', \
         '2026-11-22T09:00:00Z'), \
         (3, 'lantern-keep', 'comment.created', \
         '{\"id\":\"CM-0001\",\"on\":\"C-0001\",\"as\":\"mira\"}', '2026-11-22T09:03:00Z'), \
         (4, 'lantern-keep', 'reference.created', \
         '{\"id\":\"RF-0001\",\"on\":\"C-0001\",\"as\":\"mira\"}', '2026-11-22T09:06:00Z');",
    );
}

/// A schema-16 queue holding [`some_rows`]: this build's tables with step
/// 17 undone.
fn schema_16(db: &Path) {
    drop(SqliteQueue::open(db, PROJECT).expect("open"));
    some_rows(db);
    sqlite3(
        db,
        "DROP INDEX proposals_ref; ALTER TABLE proposals DROP COLUMN ref_id; \
         PRAGMA user_version = 16;",
    );
    assert_eq!(
        column_names(db, "proposals").len(),
        51,
        "premise: schema 16's 51 columns"
    );
}

/// Every row of the eight queue tables of schema 16, the proposals by their
/// 51 columns of schema 16, in a stable order.
fn rows_of_16(db: &Path) -> String {
    let columns = PROPOSAL_COLUMNS[..51].join(", ");
    sqlite3(
        db,
        &format!(
            "SELECT {columns} FROM proposals ORDER BY id; SELECT * FROM cards ORDER BY id; \
             SELECT * FROM tasks ORDER BY id; SELECT * FROM runs ORDER BY task_id, run; \
             SELECT * FROM comments ORDER BY id; \
             SELECT * FROM comment_versions ORDER BY comment_id, version; \
             SELECT * FROM refs ORDER BY id; SELECT * FROM events ORDER BY seq;"
        ),
    )
}

/// PR11: a schema-16 queue steps at its first open: every row as it was,
/// `ref_id` there and NULL, its index there; a second open steps nothing.
#[test]
fn pr11_a_schema_16_queue_steps_keeping_every_row() {
    let scratch = Scratch::new("pr-pr11-step");
    let db = scratch.db("q");
    schema_16(&db);
    let before = rows_of_16(&db);
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM refs;"), "2");

    let queue = SqliteQueue::open(&db, PROJECT).expect("open steps");
    assert_eq!(user_version(&db), "17");
    assert_eq!(rows_of_16(&db), before, "every row as it was");
    assert_eq!(column_names(&db, "proposals"), PROPOSAL_COLUMNS);
    assert_eq!(
        sqlite3(
            &db,
            "SELECT count(*) FROM proposals WHERE ref_id IS NOT NULL;"
        ),
        "0"
    );
    assert_eq!(
        index_of(&db, "proposals_ref"),
        ("proposals".to_owned(), "project\nref_id".to_owned())
    );
    // The step itself reads no row: the rows are as written, raw.
    drop(queue);
    let stepped = objects(&db);
    drop(SqliteQueue::open(&db, PROJECT).expect("open again"));
    assert_eq!(user_version(&db), "17", "stepped once");
    assert_eq!(rows_of_16(&db), before);
    assert_eq!(objects(&db), stepped);
}

/// PR11: a failure inside the step: a hand-made index named `proposals_ref`
/// on another table, the step's last statement; the file stays at 16 with
/// no `ref_id` column: one transaction.
#[test]
fn pr11_a_failure_inside_the_step_leaves_16() {
    let scratch = Scratch::new("pr-pr11-fail");
    let db = scratch.db("q");
    schema_16(&db);
    sqlite3(&db, "CREATE INDEX proposals_ref ON cards (title);");
    let before = rows_of_16(&db);
    let objects_before = objects(&db);
    assert!(
        SqliteQueue::open(&db, PROJECT).is_err(),
        "step 17 ran over a hand-made index"
    );
    assert_eq!(user_version(&db), "16");
    assert_eq!(rows_of_16(&db), before, "nothing added or lost");
    assert_eq!(objects(&db), objects_before, "no ref_id left behind");
    assert_eq!(column_names(&db, "proposals").len(), 51);
}

/// PR11: 18 (this build's 17 + 1) is `SchemaTooNew` for `open` and
/// `open_existing`, the file left alone.
#[test]
fn pr11_a_newer_schema_is_refused() {
    let scratch = Scratch::new("pr-pr11-newer");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    let newer = QUEUE_SCHEMA_VERSION + 1;
    assert_eq!(newer, 18);
    sqlite3(&db, &format!("PRAGMA user_version = {newer};"));
    for result in [
        SqliteQueue::open(&db, PROJECT).map(|_| ()),
        SqliteQueue::open_existing(&db, PROJECT).map(|_| ()),
    ] {
        match result {
            Err(QueueError::SchemaTooNew { found }) => assert_eq!(found, newer),
            other => panic!("expected SchemaTooNew {newer}, got {other:?}"),
        }
    }
    assert_eq!(user_version(&db), "18", "left alone");
}

/// A `reference` row of `ref_id` and `new_text`, written raw.
fn reference_row(db: &Path, id: &str, kind: &str, ref_id: Option<&str>, new_text: &str) {
    let ref_id = ref_id.map_or("NULL".to_owned(), |rf| format!("'{rf}'"));
    sqlite3(
        db,
        &format!(
            "INSERT INTO proposals (id, project, kind, status, target_id, target_path, \
             git_common_dir, worktree, root_rel, branch, base_commit, \
             target_ids, base_hash, base_text, new_text, patch_hash, rationale, author, \
             diagnostics, created_at, updated_at, ref_id) VALUES ('{id}', 'lantern-keep', '{kind}', 'open', \
             'LOC-HARBOUR', 'docs/spec/world/harbour.md', '/srv/keep/.git', '/srv/keep', '', \
             'main', '03d1f64839f2b9554ffeb4e41e3335e7669ca358', '[\"LOC-HARBOUR\"]', \
             'b3:0000000000000000000000000000000000000000000000000000000000000002', \
             '---\nid: LOC-HARBOUR\n---\n', '{new_text}', \
             'b3:0000000000000000000000000000000000000000000000000000000000000003', 'R', \
             '{{\"type\":\"human\",\"role\":null,\"model\":null,\"run\":null}}', '[]', \
             '2026-11-23T09:00:00Z', '2026-11-23T09:00:00Z', {ref_id});"
        ),
    );
}

/// The store's answer for `id`, which must be a corrupt row naming `word`.
#[track_caller]
fn assert_corrupt(queue: &SqliteQueue, id: &str, word: &str) {
    match queue.get(id) {
        Err(error) => {
            let text = error.to_string();
            assert!(text.contains(word), "{id}: {word} in {text}");
            assert!(text.contains(id), "{id}: the row named: {text}");
        }
        Ok(row) => panic!("{id} read: {row:?}"),
    }
}

/// PR11: a `reference` without `ref_id`, or with a non-RF one; an `update`
/// with one; a `reference` whose text is no entry the key keeps: corrupt.
#[test]
fn pr11_a_misplaced_ref_id_is_a_corrupt_row() {
    let scratch = Scratch::new("pr-pr11-corrupt");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    reference_row(&db, "PR-0001", "reference", None, "media/harbour.png");
    reference_row(&db, "PR-0002", "update", Some("RF-0001"), "The harbour.\n");
    reference_row(
        &db,
        "PR-0003",
        "reference",
        Some("C-0001"),
        "media/harbour.png",
    );
    reference_row(
        &db,
        "PR-0004",
        "reference",
        Some("RF-0001"),
        "/abs/harbour.png",
    );
    reference_row(
        &db,
        "PR-0005",
        "reference",
        Some("RF-0001"),
        "media/harbour.png",
    );
    let queue = SqliteQueue::open(&db, PROJECT).expect("open");
    assert_corrupt(&queue, "PR-0001", "ref_id");
    assert_corrupt(&queue, "PR-0002", "ref_id");
    assert_corrupt(&queue, "PR-0003", "ref_id");
    match queue.get("PR-0004") {
        Err(error) => assert!(error.to_string().contains("PR-0004"), "{error}"),
        Ok(row) => panic!("a text no key keeps read: {row:?}"),
    }
    let good = queue.get("PR-0005").expect("read").expect("a row");
    assert_eq!(good.ref_id.as_deref(), Some("RF-0001"));
    assert_eq!(good.kind.as_str(), "reference");
}

// ------------------------------------------------------------------ PR12

/// `(document, node, path, matters)` of the rows.
fn rows(found: &[PromotedRow]) -> Vec<(String, String, String, Option<String>)> {
    found
        .iter()
        .map(|row| {
            (
                row.document.clone(),
                row.node.clone(),
                row.path.clone(),
                row.matters.clone(),
            )
        })
        .collect()
}

fn row(
    document: &str,
    node: &str,
    path: &str,
    matters: Option<&str>,
) -> (String, String, String, Option<String>) {
    (
        document.to_owned(),
        node.to_owned(),
        path.to_owned(),
        matters.map(str::to_owned),
    )
}

/// `text` with `lines` before its front-matter's closing fence.
fn before_fence(text: &str, lines: &str) -> String {
    let close = 4 + text[4..].find("\n---\n").expect("a closing fence") + 1;
    format!("{}{lines}{}", &text[..close], &text[close..])
}

/// PR12: `promoted_of` over `fixtures/promote-mini` with entries written by
/// hand: each asked document's kept entries, root-relative, in written
/// order, the documents in the order asked; an edit replaces, a delete drops.
#[test]
fn pr12_promoted_of_lists_the_kept_entries() {
    let scratch = Scratch::new("pr-pr12-store");
    let corpus = Corpus::copy_of("promote-mini", &scratch, "wt");
    let harbour = corpus.read_text("docs/spec/world/harbour.md");
    let mira = corpus.read_text("docs/spec/cast/mira.md");
    corpus.write(
        "docs/spec/world/harbour.md",
        before_fence(
            &harbour,
            "references:\n  - \"media/harbour.png | the light at dusk, not the boats\"\n  - media/gulls.wav\n  - \"notes.md\"\n",
        ),
    );
    corpus.write(
        "docs/spec/cast/mira.md",
        before_fence(&mira, "references:\n  - \"art/mira.png\"\n"),
    );
    let mut index = corpus.open(&scratch.fresh_db());
    corpus.update(&mut index);
    assert_eq!(index.promoted_of(&[]).expect("none asked"), Vec::new());
    let found = index
        .promoted_of(&[
            "docs/spec/cast/mira.md",
            "docs/spec/world/harbour.md",
            "docs/spec/cast/mira.md",
            "docs/spec/tide.md",
        ])
        .expect("promoted_of");
    assert_eq!(
        rows(&found),
        vec![
            row(
                "docs/spec/cast/mira.md",
                "CHR-MIRA",
                "docs/spec/cast/art/mira.png",
                None
            ),
            row(
                "docs/spec/world/harbour.md",
                "LOC-HARBOUR",
                "docs/spec/world/media/harbour.png",
                Some("the light at dusk, not the boats")
            ),
            row(
                "docs/spec/world/harbour.md",
                "LOC-HARBOUR",
                "docs/spec/world/media/gulls.wav",
                None
            ),
        ],
        "a Markdown entry is not kept; a document asked twice is listed once"
    );

    // An edited key replaces its rows at the next update.
    corpus.write(
        "docs/spec/world/harbour.md",
        before_fence(
            &harbour,
            "references:\n  - \"media/bell.flac | the bell\"\n",
        ),
    );
    corpus.update(&mut index);
    assert_eq!(
        rows(
            &index
                .promoted_of(&["docs/spec/world/harbour.md"])
                .expect("read")
        ),
        vec![row(
            "docs/spec/world/harbour.md",
            "LOC-HARBOUR",
            "docs/spec/world/media/bell.flac",
            Some("the bell")
        )],
        "M: update skipping promoted for a changed file"
    );

    // A deleted file drops them.
    corpus.remove("docs/spec/cast/mira.md");
    corpus.update(&mut index);
    assert_eq!(
        index
            .promoted_of(&["docs/spec/cast/mira.md"])
            .expect("read"),
        Vec::new()
    );
}

/// "Queue schema 17": `ProposalKind::Reference`, the ninth kind, named by
/// core's `REFERENCE_KIND`, applies (`family()` `Applies`), adds no node and
/// amends nothing; its article `a reference`.
#[test]
fn pr11_the_reference_kind_applies_and_adds_nothing() {
    use specengine_store::{KindFamily, ProposalKind};
    let kind = ProposalKind::Reference;
    assert_eq!(ProposalKind::ALL.len(), 9);
    assert_eq!(ProposalKind::ALL[8], kind, "appended last");
    assert_eq!(kind.as_str(), specengine_core::intake::REFERENCE_KIND);
    assert_eq!(ProposalKind::parse("reference"), Some(kind));
    assert_eq!(kind.family(), KindFamily::Applies);
    assert!(!kind.adds_nodes());
    assert!(!kind.amends());
    assert_eq!(kind.with_article(), "a reference");
    assert_eq!(
        serde_json::to_string(&kind).expect("serialize"),
        "\"reference\""
    );
}
