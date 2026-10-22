//! The symbol lookup over the code index (`docs/canon/symbol-reads.md`
//! "Lookup"): a query meets the worktree's stored symbols in tiers, each
//! one B-tree search over 6a-2's tables — [`SymbolTier::Exact`] (`qpath`
//! equals the query, the `qpath` index), [`SymbolTier::Suffix`] (a stored
//! suffix equals it, the `suffix` index), [`SymbolTier::Name`] (`name`
//! equals the key the caller derives, the `name` index) — once per symbol
//! at its best tier, then by qpath, path and start line, never by rowid.
//! And one file's symbols, by `ord`. Each read is one transaction; inside
//! a [`SqliteIndex::code_snapshot`] they share the snapshot's.
//!
//! Strings only: the store splits no name and reads no `::`; the `name`
//! key comes from the code crate's `query_name` (Q27).

use std::collections::BTreeMap;

use rusqlite::Transaction;

use crate::code::{StoredSymbol, stored_symbol, symbol_columns};
use crate::error::{Db, StoreError};
use crate::index::SqliteIndex;
use crate::read::with_worktree;

/// How a stored symbol meets a lookup query; the order is the rank, the
/// best first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SymbolTier {
    /// Its `qpath` is the query.
    Exact,
    /// One of its suffixes is the query.
    Suffix,
    /// Its `name` is the query's name key.
    Name,
}

impl SymbolTier {
    /// Every tier, by rank.
    pub const ALL: [Self; 3] = [Self::Exact, Self::Suffix, Self::Name];

    /// The tier's word in a read's output.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Suffix => "suffix",
            Self::Name => "name",
        }
    }
}

/// One symbol a lookup answers, at its best tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolHit {
    pub tier: SymbolTier,
    pub symbol: StoredSymbol,
}

/// The lookup's statements, one per tier in [`SymbolTier::ALL`]'s order:
/// `?1` the tier's key, `?2` the worktree. Each starts at its index
/// (`CROSS JOIN` keeps it outermost), then reaches the symbol and its file
/// by key: no statement scans a table. One per tier, by type: a tier is
/// never dropped by the zip that pairs them.
pub(crate) const LOOKUP_SQL: [&str; SymbolTier::ALL.len()] = [
    concat!(
        "SELECT ",
        symbol_columns!(),
        " FROM symbols s CROSS JOIN code_files f ON f.file_id = s.file
         WHERE s.qpath = ?1 AND f.wt = ?2"
    ),
    concat!(
        "SELECT ",
        symbol_columns!(),
        " FROM symbol_suffixes x
         CROSS JOIN symbols s ON s.file = x.file AND s.ord = x.ord
         CROSS JOIN code_files f ON f.file_id = s.file
         WHERE x.suffix = ?1 AND f.wt = ?2"
    ),
    concat!(
        "SELECT ",
        symbol_columns!(),
        " FROM symbols s CROSS JOIN code_files f ON f.file_id = s.file
         WHERE s.name = ?1 AND f.wt = ?2"
    ),
];

impl SqliteIndex {
    /// Every stored symbol of the worktree `query` meets, once, at its
    /// best tier: its `qpath` is `query`, else one of its suffixes is,
    /// else its `name` is `name` (the caller's name key of `query`). By
    /// tier, then qpath, path and start line (bytes; `ord` last), never by
    /// storage order. No code indexed: none.
    pub fn lookup_symbols(&self, query: &str, name: &str) -> Result<Vec<SymbolHit>, StoreError> {
        with_worktree(self, |tx, wt| lookup_in(tx, wt, query, name))
    }

    /// The stored symbols of the file at `path`, by `ord` (source order);
    /// none when the path holds no symbol or is not stored.
    pub fn file_symbols(&self, path: &str) -> Result<Vec<StoredSymbol>, StoreError> {
        with_worktree(self, |tx, wt| file_symbols_in(tx, wt, path))
    }
}

/// [`SqliteIndex::lookup_symbols`] inside the caller's transaction.
pub(crate) fn lookup_in(
    tx: &Transaction<'_>,
    wt: i64,
    query: &str,
    name: &str,
) -> Result<Vec<SymbolHit>, StoreError> {
    let mut best: BTreeMap<(String, u32), SymbolHit> = BTreeMap::new();
    for (tier, sql) in SymbolTier::ALL.into_iter().zip(LOOKUP_SQL) {
        let key = match tier {
            SymbolTier::Exact | SymbolTier::Suffix => query,
            SymbolTier::Name => name,
        };
        let mut statement = tx.prepare(sql).db()?;
        let symbols = statement
            .query_map((key, wt), stored_symbol)
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?;
        for symbol in symbols {
            // The tiers are read best first: the first hit stays.
            best.entry((symbol.path.clone(), symbol.ord))
                .or_insert(SymbolHit { tier, symbol });
        }
    }
    let mut hits: Vec<SymbolHit> = best.into_values().collect();
    hits.sort_by(|a, b| order_key(a).cmp(&order_key(b)));
    Ok(hits)
}

/// The statement of [`file_symbols_in`]: `?1` the worktree, `?2` the path.
const FILE_SYMBOLS_SQL: &str = concat!(
    "SELECT ",
    symbol_columns!(),
    " FROM code_files f CROSS JOIN symbols s ON s.file = f.file_id
     WHERE f.wt = ?1 AND f.path = ?2 ORDER BY s.ord"
);

/// [`SqliteIndex::file_symbols`] inside the caller's transaction.
pub(crate) fn file_symbols_in(
    tx: &Transaction<'_>,
    wt: i64,
    path: &str,
) -> Result<Vec<StoredSymbol>, StoreError> {
    files_symbols_in(tx, wt, [path])
}

/// [`file_symbols_in`] of each of `paths` in turn, its statement prepared
/// once: by path as given, then `ord`.
pub(crate) fn files_symbols_in<'p>(
    tx: &Transaction<'_>,
    wt: i64,
    paths: impl IntoIterator<Item = &'p str>,
) -> Result<Vec<StoredSymbol>, StoreError> {
    let mut statement = tx.prepare(FILE_SYMBOLS_SQL).db()?;
    let mut symbols = Vec::new();
    for path in paths {
        let rows = statement
            .query_map((wt, path), stored_symbol)
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?;
        symbols.extend(rows);
    }
    Ok(symbols)
}

/// A hit's place in the answer: tier, qpath, path, start line, `ord`.
fn order_key(hit: &SymbolHit) -> (SymbolTier, &str, &str, u32, u32) {
    (
        hit.tier,
        hit.symbol.qpath.as_str(),
        hit.symbol.path.as_str(),
        hit.symbol.start_line,
        hit.symbol.ord,
    )
}

#[cfg(test)]
mod tests {
    //! docs/features/symbol-reads.md RD-1. M: a scan. Every statement the
    //! lookup runs is planned by SQLite as a search of an index
    //! (`EXPLAIN QUERY PLAN`: `SEARCH`, no `SCAN` of a table), the three
    //! tiers through the `qpath`, `suffix` and `name` columns; and the
    //! lookup's cost at 100 000 symbols stays within three times its cost
    //! at 10 000 (best of three), where a scan costs ten. Without
    //! `ANALYZE` statistics SQLite plans for large tables, so an empty
    //! database shows the plan a large one gets. The rows of the cost test
    //! are made by SQL, every column the schema requires filled from
    //! `PRAGMA table_info`, so the test needs no analyzer.
    //!
    //! docs/features/layer-c-reads.md TE-8: the moniker tier's one raw-text
    //! search is a `SEARCH` of its index too (`moniker_lookup_searches_its_index`).

    use std::time::{Duration, Instant};

    use super::LOOKUP_SQL;
    use crate::index::SqliteIndex;

    /// A scratch directory, removed on drop.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "specengine-store-lookup-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(path.join("wt")).expect("scratch");
            std::fs::create_dir_all(path.join("data")).expect("scratch");
            Self(std::fs::canonicalize(path).expect("canonical"))
        }

        fn open(&self) -> SqliteIndex {
            SqliteIndex::open(self.0.join("data/t.db"), "plan", self.0.join("wt")).expect("open")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The store's lookup over `query` and its `query_name` `name`, as the
    /// CLI calls it; the number of symbols answered.
    fn hits(index: &SqliteIndex, query: &str, name: &str) -> usize {
        index
            .lookup_symbols(query, name)
            .expect("the lookup answers")
            .len()
    }

    /// The plan's `detail` rows of `sql`, every parameter bound to a text.
    fn plan(index: &SqliteIndex, sql: &str) -> Vec<String> {
        let mut statement = index
            .conn
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap_or_else(|error| panic!("{sql}: {error}"));
        let count = statement.parameter_count();
        let values: Vec<String> = (0..count).map(|n| format!("q{n}")).collect();
        statement
            .query_map(rusqlite::params_from_iter(values.iter()), |row| {
                row.get::<_, String>(3)
            })
            .expect("plan")
            .collect::<Result<Vec<_>, _>>()
            .expect("plan rows")
    }

    #[test]
    fn symbol_lookup_uses_the_suffix_index() {
        let scratch = Scratch::new("plan");
        let index = scratch.open();
        assert!(!LOOKUP_SQL.is_empty(), "the lookup reads somewhere");
        let mut searched = Vec::new();
        for sql in LOOKUP_SQL {
            let details = plan(&index, sql);
            assert!(
                details.iter().any(|detail| detail.starts_with("SEARCH")),
                "{sql}: no search: {details:?}"
            );
            assert!(
                !details
                    .iter()
                    .any(|detail| detail.starts_with("SCAN") && !detail.contains("CONSTANT ROW")),
                "{sql}: a scan: {details:?}"
            );
            searched.extend(
                details
                    .into_iter()
                    .filter(|detail| detail.starts_with("SEARCH")),
            );
        }
        for column in ["qpath=", "suffix=", "name="] {
            assert!(
                searched.iter().any(|detail| detail.contains(column)),
                "no tier searches by {column}: {searched:?}"
            );
        }
    }

    /// docs/features/layer-c-reads.md TE-8 ("Tier moniker" step 2). M: a
    /// scan, or a search keyed by anything but the raw moniker. The one
    /// statement the moniker tier adds (`crate::layer_c::FOUND_SQL`) is
    /// planned as a `SEARCH` of `code_monikers` through
    /// `code_monikers_by_moniker` on `moniker=?`, with no `SCAN` of a
    /// table; and the shared lookup's statements stay three, one per tier
    /// of [`super::SymbolTier::ALL`] (R2: the tier never joins
    /// `lookup_in`).
    #[test]
    fn moniker_lookup_searches_its_index() {
        let scratch = Scratch::new("moniker-plan");
        let index = scratch.open();
        let sql = crate::layer_c::FOUND_SQL;
        let details = plan(&index, sql);
        assert!(
            details.iter().any(|detail| {
                detail.starts_with("SEARCH")
                    && detail.contains("code_monikers")
                    && detail.contains("code_monikers_by_moniker")
                    && detail.contains("moniker=?")
            }),
            "{sql}: no search of code_monikers_by_moniker: {details:?}"
        );
        assert!(
            !details
                .iter()
                .any(|detail| detail.starts_with("SCAN") && !detail.contains("CONSTANT ROW")),
            "{sql}: a scan: {details:?}"
        );
        assert_eq!(LOOKUP_SQL.len(), 3, "the shared lookup keeps three tiers");
        assert!(
            LOOKUP_SQL.iter().all(|sql| !sql.contains("code_monikers")),
            "no lookup statement reads layer C"
        );
    }

    /// `(name, type, not null, has a default, primary key position)` of
    /// every column of `table`.
    fn columns(index: &SqliteIndex, table: &str) -> Vec<(String, String, bool, bool, i64)> {
        let mut statement = index
            .conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("table_info");
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)? != 0,
                    row.get::<_, Option<String>>(4)?.is_some(),
                    row.get::<_, i64>(5)?,
                ))
            })
            .expect("columns")
            .collect::<Result<Vec<_>, _>>()
            .expect("column rows")
    }

    /// `WITH RECURSIVE n(i) … INSERT INTO <table> … SELECT … <from>`: the
    /// `given` columns as SQL expressions, every other required column a
    /// filler of its type, a sole integer key left to SQLite.
    fn fill(index: &SqliteIndex, table: &str, given: &[(&str, String)], count: usize, from: &str) {
        let columns = columns(index, table);
        assert!(!columns.is_empty(), "the schema has `{table}`");
        let keys = columns.iter().filter(|column| column.4 > 0).count();
        let mut names = Vec::new();
        let mut values = Vec::new();
        for (name, kind, not_null, has_default, key) in &columns {
            if let Some((_, value)) = given.iter().find(|(given, _)| given == name) {
                names.push(name.clone());
                values.push(value.clone());
                continue;
            }
            let rowid_alias = *key > 0 && keys == 1 && kind.eq_ignore_ascii_case("INTEGER");
            if rowid_alias || !*not_null || *has_default {
                continue;
            }
            names.push(name.clone());
            values.push(
                match kind.to_ascii_uppercase().as_str() {
                    "INTEGER" | "INT" => "0",
                    "REAL" => "0.0",
                    "BLOB" => "x''",
                    _ => "''",
                }
                .to_owned(),
            );
        }
        let sql = format!(
            "WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM n WHERE i + 1 < {count}) \
             INSERT INTO {table} ({}) SELECT {} {from}",
            names.join(", "),
            values.join(", ")
        );
        index
            .conn
            .execute_batch(&sql)
            .unwrap_or_else(|error| panic!("{sql}: {error}"));
    }

    /// An index of `count` symbols, 1 000 per file: symbol `i` is
    /// `pkg::m<f>::s<i>` (`f` = `i` / 1 000) with its three suffix rows.
    fn populated(scratch: &Scratch, count: usize) -> SqliteIndex {
        let index = scratch.open();
        index
            .conn
            .execute(
                "INSERT INTO worktrees (project, root, scheme_fp) VALUES (?1, ?2, 'x')",
                rusqlite::params![index.project, index.root_text],
            )
            .expect("a worktree");
        let wt = index.conn.last_insert_rowid();
        fill(
            &index,
            "code_worktrees",
            &[("wt", wt.to_string()), ("code_fp", "'x'".to_owned())],
            1,
            "FROM n",
        );
        let files = count.div_ceil(1000);
        fill(
            &index,
            "code_files",
            &[
                ("wt", wt.to_string()),
                ("path", "'p/f' || i || '.rs'".to_owned()),
                ("lang", "'rs'".to_owned()),
                ("utf8", "1".to_owned()),
            ],
            files,
            "FROM n",
        );
        fill(
            &index,
            "symbols",
            &[
                (
                    "file",
                    format!(
                        "(SELECT rowid FROM code_files WHERE wt = {wt} AND path = 'p/f' || (i / 1000) || '.rs')"
                    ),
                ),
                ("ord", "i % 1000".to_owned()),
                ("item_kind", "'function_item'".to_owned()),
                ("name", "'s' || i".to_owned()),
                ("qpath", "'pkg::m' || (i / 1000) || '::s' || i".to_owned()),
                ("start_line", "i % 1000 + 1".to_owned()),
                ("end_line", "i % 1000 + 1".to_owned()),
                ("signature", "'fn s' || i || '()'".to_owned()),
                ("level_state", "'hashed'".to_owned()),
                ("level_categories", "'[]'".to_owned()),
                ("layer", "'a'".to_owned()),
                ("twin_files", "0".to_owned()),
            ],
            count,
            "FROM n",
        );
        for (depth, suffix) in [(1, "name"), (2, "substr(qpath, 6)"), (3, "qpath")] {
            fill(
                &index,
                "symbol_suffixes",
                &[
                    ("file", "file".to_owned()),
                    ("ord", "ord".to_owned()),
                    ("depth", depth.to_string()),
                    ("suffix", suffix.to_owned()),
                ],
                1,
                "FROM symbols",
            );
        }
        let stored: i64 = index
            .conn
            .query_row("SELECT count(*) FROM symbol_suffixes", [], |row| row.get(0))
            .expect("count");
        assert_eq!(stored, 3 * i64::try_from(count).expect("i64"));
        index
    }

    /// The queries timed: an exact qpath, a suffix, a name past another
    /// module, a miss; each with its `query_name` and the hits it must
    /// give at any size (one symbol at its best tier, or none).
    const QUERIES: [(&str, &str, usize); 4] = [
        ("pkg::m3::s3333", "s3333", 1),
        ("m3::s3333", "s3333", 1),
        ("elsewhere::s3333", "s3333", 1),
        ("nothing::here", "here", 0),
    ];

    /// The best of three runs of 20 rounds of [`QUERIES`].
    fn cost(index: &SqliteIndex) -> Duration {
        (0..3)
            .map(|_| {
                let started = Instant::now();
                for _ in 0..20 {
                    for (query, name, want) in QUERIES {
                        assert_eq!(hits(index, query, name), want, "{query}");
                    }
                }
                started.elapsed()
            })
            .min()
            .expect("three runs")
    }

    #[test]
    fn symbol_lookup_cost_is_sublinear() {
        let (small_dir, large_dir) = (Scratch::new("10k"), Scratch::new("100k"));
        let small = populated(&small_dir, 10_000);
        let large = populated(&large_dir, 100_000);
        let (at_10k, at_100k) = (cost(&small), cost(&large));
        eprintln!("lookup cost, best of three: 10k {at_10k:?}, 100k {at_100k:?}");
        let floor = Duration::from_millis(2).max(at_10k);
        assert!(
            at_100k <= floor * 3,
            "10x the symbols cost {at_100k:?} against {at_10k:?}: a scan"
        );
    }
}
