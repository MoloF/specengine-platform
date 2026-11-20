//! The practices of asked disciplines (task spec `department-practices`,
//! "The index: `INDEX_FORMAT` 19"; ADR-0084): one query over the
//! `disciplines` table, joined to the worktree's live, non-generated files,
//! by path then discipline. Computed per read; nothing is cached.

use rusqlite::types::Value;
use rusqlite::{Transaction, params_from_iter};
use specengine_core::check::DocClass;

use crate::error::{Db, StoreError};
use crate::index::SqliteIndex;
use crate::read::{IndexSnapshot, with_worktree};

/// One kept `disciplines:` entry of a practice that matches an asked
/// discipline.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PracticeRow {
    pub path: String,
    pub discipline: String,
}

impl SqliteIndex {
    /// The kept entries of the live, non-generated documents of the
    /// worktree that name one of `disciplines`, by path then discipline;
    /// none for no discipline asked. [`StoreError::NotIndexed`] as every
    /// read of the worktree.
    pub fn practices_of(&self, disciplines: &[&str]) -> Result<Vec<PracticeRow>, StoreError> {
        if disciplines.is_empty() {
            return Ok(Vec::new());
        }
        with_worktree(self, |tx, wt| practices(tx, wt, disciplines))
    }
}

impl IndexSnapshot<'_> {
    /// [`SqliteIndex::practices_of`] inside the snapshot.
    pub fn practices_of(&self, disciplines: &[&str]) -> Result<Vec<PracticeRow>, StoreError> {
        if disciplines.is_empty() {
            return Ok(Vec::new());
        }
        practices(&self.tx, self.wt, disciplines)
    }
}

/// The one query: `?1` the worktree, `?2` the generated class, then each
/// discipline asked (through `disciplines_by_name`).
fn practices(
    tx: &Transaction<'_>,
    wt: i64,
    disciplines: &[&str],
) -> Result<Vec<PracticeRow>, StoreError> {
    let asked: Vec<String> = (0..disciplines.len())
        .map(|at| format!("?{}", at + 3))
        .collect();
    let sql = format!(
        "SELECT f.path, d.discipline FROM disciplines d
         JOIN files f ON f.file_id = d.file_id
         JOIN nodes n ON n.file_id = f.file_id AND n.ord = 0
         WHERE d.discipline IN ({}) AND f.wt = ?1 AND f.tier3 = 0
           AND json_extract(n.node, '$.fields.class') IS NOT ?2
         ORDER BY f.path, d.discipline",
        asked.join(", ")
    );
    let mut values: Vec<Value> = vec![
        Value::Integer(wt),
        Value::Text(DocClass::Generated.as_str().to_owned()),
    ];
    values.extend(
        disciplines
            .iter()
            .map(|discipline| Value::Text((*discipline).to_owned())),
    );
    let mut statement = tx.prepare(&sql).db()?;
    let mut rows: Vec<PracticeRow> = statement
        .query_map(params_from_iter(values), |row| {
            Ok(PracticeRow {
                path: row.get(0)?,
                discipline: row.get(1)?,
            })
        })
        .db()?
        .collect::<Result<_, _>>()
        .db()?;
    // An asked discipline given twice matches once.
    rows.dedup();
    Ok(rows)
}
