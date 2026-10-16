//! The role read of the code index (`docs/features/bevy-roles.md`
//! "Links", L2, L3): every stored `code_roles` row of the worktree with its
//! file's path and package, and for each row with a subject its link
//! candidates, read in one transaction. Never stored: a link is computed by
//! the reader from these candidates (L1), so a renamed or removed item
//! shows at the next read without its row's file re-derived.
//!
//! - **Candidates**: the worktree's symbols whose `name` is the row's
//!   `subject_name` and whose `item_kind` is among the kinds the caller
//!   gives for the row's word, each with its file's package and its tier
//!   for the subject: [`SymbolTier::Exact`] (its `qpath` is the subject),
//!   [`SymbolTier::Suffix`] (a stored suffix is), else
//!   [`SymbolTier::Name`]. A cfg twin is at the tier `subject ‖ ' #' ‖
//!   discriminator` reaches, as the twin pass reads twins
//!   (`crate::code::TWIN_SQL`): its stored qpath and suffixes are compared
//!   whole, never split.
//! - **Index searches only** ([`CANDIDATES_SQL`]): `symbols_by_name`, then
//!   the candidate's file and suffixes by key; one lookup per distinct
//!   (subject, name), whatever the rows naming it.
//! - **Strings only**: the store knows no word, kind or framework; the
//!   caller's map pairs them.

use std::collections::BTreeMap;

use rusqlite::{Statement, Transaction};

use crate::code::{
    SYMBOL_COLUMN_COUNT, StoredSymbol, stored_code_fp, stored_symbol, symbol_columns,
};
use crate::error::{Db, StoreError};
use crate::index::SqliteIndex;
use crate::read::with_worktree;
use crate::symbols::SymbolTier;

/// One stored role row with its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRole {
    pub path: String,
    /// The file's package directory (`code_files.package`); `None` outside
    /// every package.
    pub package: Option<String>,
    pub ord: u32,
    pub word: String,
    pub source: String,
    pub form: Option<String>,
    pub origin: String,
    pub group_text: Option<String>,
    /// The `ord` of the symbol of this file it sits on.
    pub symbol_ord: Option<u32>,
    pub subject: Option<String>,
    pub subject_name: Option<String>,
    pub certainty: String,
    pub category: Option<String>,
    pub line: u32,
    /// For a row with a subject: the symbols it may name, by tier, then
    /// qpath, path, start line and `ord`; none otherwise.
    pub candidates: Vec<RoleCandidate>,
}

/// A symbol a role's subject may name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleCandidate {
    /// Its tier for the subject, a cfg twin's lifted.
    pub tier: SymbolTier,
    /// Its file's package directory.
    pub package: Option<String>,
    pub symbol: StoredSymbol,
}

/// Every role row of the worktree (`?1`), by path and `ord`.
pub(crate) const ROLES_SQL: &str = "SELECT f.path, f.package, r.ord, r.word, r.source, r.form,
            r.origin, r.group_text, r.symbol_ord, r.subject, r.subject_name, r.certainty,
            r.category, r.line
     FROM code_files f CROSS JOIN code_roles r ON r.file = f.file_id
     WHERE f.wt = ?1 ORDER BY f.path, r.ord";

/// The symbols of the worktree (`?3`) named `?2` with their file's package
/// and whether the subject `?1` meets them exactly (their qpath, a twin's
/// with its discriminator) or as a suffix (one of their stored suffixes,
/// the same for a twin): from the `name` index, then by key.
pub(crate) const CANDIDATES_SQL: &str = concat!(
    "SELECT ",
    symbol_columns!(),
    ", f.package,
       s.qpath = ?1 OR (s.discriminator IS NOT NULL AND s.qpath = ?1 || ' #' || s.discriminator),
       EXISTS (SELECT 1 FROM symbol_suffixes x WHERE x.file = s.file AND x.ord = s.ord
               AND (x.suffix = ?1
                    OR (s.discriminator IS NOT NULL AND x.suffix = ?1 || ' #' || s.discriminator)))
     FROM symbols s CROSS JOIN code_files f ON f.file_id = s.file
     WHERE s.name = ?2 AND f.wt = ?3"
);

/// The first column after [`symbol_columns!`] in [`CANDIDATES_SQL`].
const AFTER_SYMBOL: usize = SYMBOL_COLUMN_COUNT;

impl SqliteIndex {
    /// Every role row of the worktree with its candidates, `kinds` each
    /// word's item kinds (a word it does not name: no candidate), in one
    /// read transaction; `Ok(None)` when no code is indexed.
    pub fn code_roles(
        &self,
        kinds: &BTreeMap<&str, Vec<&str>>,
    ) -> Result<Option<Vec<StoredRole>>, StoreError> {
        with_worktree(self, |tx, wt| {
            if stored_code_fp(tx, wt)?.is_none() {
                return Ok(None);
            }
            Ok(Some(roles_in(tx, wt, kinds)?))
        })
    }
}

/// [`SqliteIndex::code_roles`] inside the caller's transaction (a later
/// read's snapshot reads it beside its symbols).
pub(crate) fn roles_in(
    tx: &Transaction<'_>,
    wt: i64,
    kinds: &BTreeMap<&str, Vec<&str>>,
) -> Result<Vec<StoredRole>, StoreError> {
    let mut statement = tx.prepare(ROLES_SQL).db()?;
    let mut roles = statement
        .query_map([wt], |row| {
            Ok(StoredRole {
                path: row.get(0)?,
                package: row.get(1)?,
                ord: row.get(2)?,
                word: row.get(3)?,
                source: row.get(4)?,
                form: row.get(5)?,
                origin: row.get(6)?,
                group_text: row.get(7)?,
                symbol_ord: row.get(8)?,
                subject: row.get(9)?,
                subject_name: row.get(10)?,
                certainty: row.get(11)?,
                category: row.get(12)?,
                line: row.get(13)?,
                candidates: Vec::new(),
            })
        })
        .db()?
        .collect::<Result<Vec<_>, _>>()
        .db()?;
    let mut lookup = tx.prepare(CANDIDATES_SQL).db()?;
    // subject → name → every symbol named so, any kind: looked up by the
    // row's borrowed strings, owned once per distinct pair.
    let mut found: BTreeMap<String, BTreeMap<String, Vec<RoleCandidate>>> = BTreeMap::new();
    for role in &mut roles {
        let (Some(subject), Some(name)) = (&role.subject, &role.subject_name) else {
            continue;
        };
        let known = found
            .get(subject.as_str())
            .is_some_and(|names| names.contains_key(name.as_str()));
        if !known {
            let candidates = candidates(&mut lookup, subject, name, wt)?;
            found
                .entry(subject.clone())
                .or_default()
                .insert(name.clone(), candidates);
        }
        let wanted = kinds.get(role.word.as_str());
        role.candidates = found
            .get(subject.as_str())
            .and_then(|names| names.get(name.as_str()))
            .into_iter()
            .flatten()
            .filter(|candidate| {
                wanted.is_some_and(|kinds| kinds.contains(&candidate.symbol.item_kind.as_str()))
            })
            .cloned()
            .collect();
    }
    Ok(roles)
}

/// Every symbol of the worktree `wt` named `name`, with its tier for
/// `subject` ([`CANDIDATES_SQL`]), in [`order_key`] order.
fn candidates(
    lookup: &mut Statement<'_>,
    subject: &str,
    name: &str,
    wt: i64,
) -> Result<Vec<RoleCandidate>, StoreError> {
    let mut candidates = lookup
        .query_map((subject, name, wt), |row| {
            let exact: bool = row.get(AFTER_SYMBOL + 1)?;
            let suffix: bool = row.get(AFTER_SYMBOL + 2)?;
            Ok(RoleCandidate {
                tier: if exact {
                    SymbolTier::Exact
                } else if suffix {
                    SymbolTier::Suffix
                } else {
                    SymbolTier::Name
                },
                package: row.get(AFTER_SYMBOL)?,
                symbol: stored_symbol(row)?,
            })
        })
        .db()?
        .collect::<Result<Vec<_>, _>>()
        .db()?;
    candidates.sort_by(|a, b| order_key(a).cmp(&order_key(b)));
    Ok(candidates)
}

/// A candidate's place: tier, qpath, path, start line, `ord`.
fn order_key(candidate: &RoleCandidate) -> (SymbolTier, &str, &str, u32, u32) {
    (
        candidate.tier,
        candidate.symbol.qpath.as_str(),
        candidate.symbol.path.as_str(),
        candidate.symbol.start_line,
        candidate.symbol.ord,
    )
}
