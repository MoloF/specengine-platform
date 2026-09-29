//! AC-03 of docs/features/spec-index.md: a reopened connection reads every
//! "Connection" PRAGMA back — the persistent ones (`journal_mode=WAL`,
//! `auto_vacuum=INCREMENTAL`), set before the first table, and the
//! per-connection ones — and the bundled SQLite has FTS5 compiled in.

#![cfg(unix)]

mod common;

use common::{Corpus, Scratch};
use specengine_store::{DbSettings, SqliteIndex};

fn assert_connection_settings(settings: &DbSettings, context: &str) {
    assert_eq!(
        settings.journal_mode.to_ascii_lowercase(),
        "wal",
        "{context}: journal_mode"
    );
    assert_eq!(
        settings.auto_vacuum, 2,
        "{context}: auto_vacuum (2 = INCREMENTAL)"
    );
    assert_eq!(settings.busy_timeout, 5000, "{context}: busy_timeout");
    assert_eq!(settings.foreign_keys, 1, "{context}: foreign_keys");
    assert_eq!(
        settings.synchronous, 1,
        "{context}: synchronous (1 = NORMAL)"
    );
    assert_eq!(
        settings.journal_size_limit, 67_108_864,
        "{context}: journal_size_limit"
    );
    assert_eq!(settings.trusted_schema, 0, "{context}: trusted_schema");
    assert_eq!(
        settings.recursive_triggers, 0,
        "{context}: recursive_triggers"
    );
    assert!(settings.fts5, "{context}: ENABLE_FTS5 compiled in");
}

#[test]
fn a_reopened_connection_reads_every_connection_pragma_back() {
    let scratch = Scratch::new("schema");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let db = scratch.db("index");
    {
        let mut index = corpus.open(&db);
        assert_connection_settings(&index.settings().expect("settings"), "new DB");
        corpus.update(&mut index);
    }
    // A second handle on the existing file: the persistent PRAGMAs come from
    // the file, the per-connection ones from the handle's own setup.
    let reopened = SqliteIndex::open(&db, "another-project", &corpus.root).expect("reopen");
    let settings = reopened.settings().expect("settings");
    assert_connection_settings(&settings, "reopened DB");
    assert_eq!(
        settings.sqlite_version, "3.53.2",
        "the bundled SQLite of the pinned crate (spec: SQLite 3.53.2)"
    );
    // Two handles at once share the file; each has its own connection setup.
    let first = corpus.open(&db);
    let second = corpus.open(&db);
    assert_connection_settings(&first.settings().expect("settings"), "first of two");
    assert_connection_settings(&second.settings().expect("settings"), "second of two");
}
