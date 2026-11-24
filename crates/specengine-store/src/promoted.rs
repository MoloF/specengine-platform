//! The files promoted beside documents (task spec `reference-promotion`,
//! "Index format 22"; ADR-0089): one query over the `promoted` table,
//! joined to the worktree's live, non-generated files, the asked documents
//! in the order asked, each document's entries in written order. Computed
//! per read; nothing is cached.

use rusqlite::Transaction;
use specengine_core::check::DocClass;

use crate::error::{Db, StoreError};
use crate::index::SqliteIndex;
use crate::read::{IndexSnapshot, with_worktree};

/// One kept `references:` entry of an asked document.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PromotedRow {
    /// The document's root-relative path.
    pub document: String,
    /// The document's ID, else its path.
    pub node: String,
    /// The file's root-relative path.
    pub path: String,
    pub matters: Option<String>,
}

impl SqliteIndex {
    /// The kept entries of each live, non-generated document of the
    /// worktree among `documents` (root-relative paths), the documents in
    /// the order asked (one asked twice listed once), each one's entries in
    /// written order; none for no document asked. [`StoreError::NotIndexed`]
    /// as every read of the worktree.
    pub fn promoted_of(&self, documents: &[&str]) -> Result<Vec<PromotedRow>, StoreError> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }
        with_worktree(self, |tx, wt| promoted(tx, wt, documents))
    }
}

impl SqliteIndex {
    /// [`Self::promoted_of`] of the documents `targets` reach, in target
    /// order, each document once: a root-relative `.md` path is its own
    /// document; an ID (`slug/ID` its bare ID) is held by the live,
    /// non-generated file whose node declares it (a `slug/ID` the one whose
    /// feature stem under `features`, the `[paths] features` directory, is
    /// the slug; a bare ID the first by path outside the features, else the
    /// first by path); a section reaches its
    /// document. A target no file holds reaches none.
    pub fn promoted_for(
        &self,
        targets: &[&str],
        features: &str,
    ) -> Result<Vec<PromotedRow>, StoreError> {
        if targets.is_empty() {
            return Ok(Vec::new());
        }
        with_worktree(self, |tx, wt| {
            let documents = documents_of(tx, wt, targets, features)?;
            let asked: Vec<&str> = documents.iter().map(String::as_str).collect();
            promoted(tx, wt, &asked)
        })
    }
}

/// The document each target reaches, in target order, each once.
fn documents_of(
    tx: &Transaction<'_>,
    wt: i64,
    targets: &[&str],
    features: &str,
) -> Result<Vec<String>, StoreError> {
    let mut statement = tx
        .prepare(
            "SELECT f.path FROM nodes n JOIN files f ON f.file_id = n.file_id
             WHERE f.wt = ?1 AND n.id = ?2 AND f.tier3 = 0 ORDER BY f.path",
        )
        .db()?;
    let mut documents: Vec<String> = Vec::new();
    for target in targets {
        let document = if target.ends_with(specengine_core::DOCUMENT_EXTENSION) {
            Some((*target).to_owned())
        } else {
            let (slug, id) = match target.rsplit_once('/') {
                Some((slug, id)) => (Some(slug), id),
                None => (None, *target),
            };
            let mut rows = statement.query((wt, id)).db()?;
            let mut holders: Vec<String> = Vec::new();
            while let Some(row) = rows.next().db()? {
                holders.push(row.get(0).db()?);
            }
            match slug {
                Some(slug) => holders.into_iter().find(|path| {
                    specengine_core::check::resolve::feature_stem(features, path) == Some(slug)
                }),
                // A bare ID: a holder outside the features first (a feature's
                // own IDs are `slug/ID`), else the first by path.
                None => {
                    let outside = holders.iter().position(|path| {
                        specengine_core::check::resolve::feature_stem(features, path).is_none()
                    });
                    match outside {
                        Some(at) => Some(holders.swap_remove(at)),
                        None => holders.into_iter().next(),
                    }
                }
            }
        };
        if let Some(document) = document
            && !documents.contains(&document)
        {
            documents.push(document);
        }
    }
    Ok(documents)
}

impl IndexSnapshot<'_> {
    /// [`SqliteIndex::promoted_of`] inside the snapshot.
    pub fn promoted_of(&self, documents: &[&str]) -> Result<Vec<PromotedRow>, StoreError> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }
        promoted(&self.tx, self.wt, documents)
    }
}

/// One statement, run per asked document: `?1` the worktree, `?2` the
/// generated class, `?3` the document's path; rows stepped one at a time.
fn promoted(
    tx: &Transaction<'_>,
    wt: i64,
    documents: &[&str],
) -> Result<Vec<PromotedRow>, StoreError> {
    let mut statement = tx
        .prepare(
            "SELECT f.path, n.id, p.path, p.matters FROM promoted p
             JOIN files f ON f.file_id = p.file_id
             JOIN nodes n ON n.file_id = f.file_id AND n.ord = 0
             WHERE f.wt = ?1 AND f.path = ?3 AND f.tier3 = 0
               AND json_extract(n.node, '$.fields.class') IS NOT ?2
             ORDER BY p.ord",
        )
        .db()?;
    let generated = DocClass::Generated.as_str();
    let mut out = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for document in documents {
        if seen.contains(document) {
            continue;
        }
        seen.push(document);
        let mut rows = statement.query((wt, generated, *document)).db()?;
        while let Some(row) = rows.next().db()? {
            let path: String = row.get(0).db()?;
            let id: Option<String> = row.get(1).db()?;
            out.push(PromotedRow {
                node: id.unwrap_or_else(|| path.clone()),
                document: path,
                path: row.get(2).db()?,
                matters: row.get(3).db()?,
            });
        }
    }
    Ok(out)
}
