//! A create's similar probes over the index of the handle's worktree (task
//! spec `intake-similar`, IS1 to IS5): an equal title, an alias equal to a
//! title, a search rank, shared link targets; the queue's probe is the
//! CLI's, over [`crate::LiveAdd`]. One deferred read transaction, nothing
//! written. Tier 3 files are never searched, as [`crate::SpecIndex::search`]
//! drops them without `archive`; the excluded node (a section form's
//! target, which the create extends) is never a row.
//!
//! Every order is fixed by keys, never by rowid: titles and aliases by
//! `(path, ord)`; search by `bm25(nodes_fts, 10.0, 5.0, 1.0)`, path, ord;
//! neighbours by the shared count descending, path, ord. A node a probe
//! found is not found again by a later one.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::types::Value;
use rusqlite::{OptionalExtension, Transaction, params_from_iter};
use specengine_core::intake::normalized_summary;
use specengine_core::similar::{
    KEYWORD_MIN_CHARS, SIMILAR_NEIGHBOURS_MAX, SIMILAR_SEARCH_MAX, SIMILAR_SHARED_MIN,
    SIMILAR_TARGET_DEGREE_MAX, SIMILAR_TERM_NODES_MAX, SimilarSource, term_words,
};
use specengine_model::{LinkOrigin, MENTIONS};

use crate::MIN_TERM_CHARS;
use crate::error::{Db, StoreError};
use crate::index::SqliteIndex;
use crate::read::with_worktree;
use crate::search::fts_string;

// A keyword is a search term: the shortest the store searches.
const _: () = assert!(KEYWORD_MIN_CHARS == MIN_TERM_CHARS);

/// One new node's title as the probes compare it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimilarTitle {
    /// The new node's name: its ID, else the create's path.
    pub new: String,
    /// Core's `normalized_summary` of its title; never empty.
    pub normalized: String,
}

/// One front-matter link target of the new document, resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimilarTarget {
    /// Its canonical name, as a reason names it.
    pub name: String,
    /// The nodes it resolves to, `(path, ord)`.
    pub ends: Vec<(String, usize)>,
}

/// What [`crate::SpecIndex::similar`] probes for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SimilarQuery {
    /// The new nodes' titles, in text order (probes 1 and 2).
    pub titles: Vec<SimilarTitle>,
    /// The search terms, lower-cased, each once (core `similar_terms`),
    /// ORed by probe 4.
    pub terms: Vec<String>,
    /// The new document's kind; `None`: no neighbour probe.
    pub kind: Option<String>,
    /// The new document's front-matter link targets but `mentions`, each
    /// once (probe 5).
    pub links: Vec<SimilarTarget>,
    /// A node never listed: a section form's target, `(path, ord)`.
    pub exclude: Option<(String, usize)>,
}

/// One node a probe found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimilarRow {
    /// `Title`, `Alias`, `Search` or `Neighbours`.
    pub source: SimilarSource,
    pub path: String,
    /// 0 the document, then its sections.
    pub ord: usize,
    pub id: Option<String>,
    pub title: Option<String>,
    pub kind: Option<String>,
    /// The new node it matched (title, alias): the first in text order.
    pub new: Option<String>,
    /// The alias (alias), the terms kept (search), the shared targets'
    /// names in the query's order (neighbours); else empty.
    pub detail: Vec<String>,
    /// Its 1-based place in the search's order, the nodes listed before
    /// counted (search alone).
    pub rank: Option<usize>,
}

/// The caps the probes run under: [`SimilarCaps::ENGINE`], core's; a unit
/// test may lower one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SimilarCaps {
    pub term_nodes_max: usize,
    pub target_degree_max: usize,
    pub search_max: usize,
    pub neighbours_max: usize,
    pub shared_min: usize,
}

impl SimilarCaps {
    pub(crate) const ENGINE: Self = Self {
        term_nodes_max: SIMILAR_TERM_NODES_MAX,
        target_degree_max: SIMILAR_TARGET_DEGREE_MAX,
        search_max: SIMILAR_SEARCH_MAX,
        neighbours_max: SIMILAR_NEIGHBOURS_MAX,
        shared_min: SIMILAR_SHARED_MIN,
    };
}

/// A node as a probe reads it.
#[derive(Debug, Clone)]
struct Found {
    path: String,
    ord: usize,
    id: Option<String>,
    title: Option<String>,
    kind: Option<String>,
}

impl Found {
    fn key(&self) -> (String, usize) {
        (self.path.clone(), self.ord)
    }

    fn row(
        self,
        source: SimilarSource,
        new: Option<String>,
        detail: Vec<String>,
        rank: Option<usize>,
    ) -> SimilarRow {
        SimilarRow {
            source,
            path: self.path,
            ord: self.ord,
            id: self.id,
            title: self.title,
            kind: self.kind,
            new,
            detail,
            rank,
        }
    }
}

/// The node columns every probe selects, `f` the file and `n` the node.
const NODE_COLUMNS: &str = "f.path, n.ord, n.id, n.title, n.kind";

/// The nodes of the worktree `?2` whose FTS row matches `?1`, Tier 3 left
/// out.
const MATCHED: &str = "FROM nodes_fts
     JOIN nodes n ON n.node_id = nodes_fts.rowid
     JOIN files f ON f.file_id = n.file_id
     WHERE nodes_fts MATCH ?1 AND f.wt = ?2 AND f.tier3 = 0";

/// [`crate::SpecIndex::similar`] under `caps`.
pub(crate) fn similar_capped(
    index: &SqliteIndex,
    query: &SimilarQuery,
    caps: &SimilarCaps,
) -> Result<Vec<SimilarRow>, StoreError> {
    with_worktree(index, |tx, wt| {
        let exclude = query
            .exclude
            .as_ref()
            .map(|(path, ord)| (path.clone(), *ord));
        let mut rows = title_rows(tx, wt, query, exclude.as_ref())?;
        let mut listed: BTreeSet<(String, usize)> =
            rows.iter().map(|row| (row.path.clone(), row.ord)).collect();
        let aliases = alias_rows(tx, wt, query, exclude.as_ref(), &listed)?;
        listed.extend(aliases.iter().map(|row| (row.path.clone(), row.ord)));
        rows.extend(aliases);
        let search = search_rows(tx, wt, query, exclude.as_ref(), &listed, caps)?;
        listed.extend(search.iter().map(|row| (row.path.clone(), row.ord)));
        rows.extend(search);
        rows.extend(neighbour_rows(
            tx,
            wt,
            query,
            exclude.as_ref(),
            &listed,
            caps,
        )?);
        Ok(rows)
    })
}

/// Probe 1: a node whose normalized title equals a new node's. Candidates
/// by FTS5 on the `title` column, the title's terms of at least
/// [`MIN_TERM_CHARS`] characters ANDed; a title without one scans every
/// titled node. By `(path, ord)`, each with the first new node it matched.
fn title_rows(
    tx: &Transaction<'_>,
    wt: i64,
    query: &SimilarQuery,
    exclude: Option<&(String, usize)>,
) -> Result<Vec<SimilarRow>, StoreError> {
    let mut found: BTreeMap<(String, usize), (Found, String)> = BTreeMap::new();
    // Every titled node with its normalized title, read and normalized
    // once, for the titles without a term to search by.
    let mut scanned: Option<Vec<(Found, String)>> = None;
    for title in &query.titles {
        if title.normalized.is_empty() {
            continue;
        }
        // Split at U+0000 too, which would end the FTS5 string early: each
        // part still lies in an equal title, so the candidates only widen.
        let terms: Vec<String> = term_words(&title.normalized)
            .filter(|term| term.chars().count() >= MIN_TERM_CHARS)
            .map(|term| format!("title : {}", fts_string(term)))
            .collect();
        let fetched: Vec<(Found, String)>;
        let candidates: &[(Found, String)] = if terms.is_empty() {
            if scanned.is_none() {
                scanned = Some(normalized(nodes_of(
                    tx,
                    &format!(
                        "SELECT {NODE_COLUMNS} FROM nodes n JOIN files f ON f.file_id = n.file_id
                         WHERE f.wt = ?1 AND f.tier3 = 0 AND n.title IS NOT NULL
                         ORDER BY f.path, n.ord"
                    ),
                    vec![Value::Integer(wt)],
                )?));
            }
            scanned.as_deref().unwrap_or_default()
        } else {
            fetched = normalized(nodes_of(
                tx,
                &format!("SELECT {NODE_COLUMNS} {MATCHED} ORDER BY f.path, n.ord"),
                vec![Value::Text(terms.join(" AND ")), Value::Integer(wt)],
            )?);
            &fetched
        };
        for (candidate, seen) in candidates {
            let key = candidate.key();
            if exclude == Some(&key) || found.contains_key(&key) {
                continue;
            }
            if *seen == title.normalized {
                found.insert(key, (candidate.clone(), title.new.clone()));
            }
        }
    }
    Ok(found
        .into_values()
        .map(|(node, new)| node.row(SimilarSource::Title, Some(new), Vec::new(), None))
        .collect())
}

/// Probe 2: a node with an `aliases:` entry whose normalized form equals a
/// new node's normalized title, compared here over the worktree's aliases.
/// By `(path, ord)`, each with the first new node it matched in text order
/// and that node's first matching alias.
fn alias_rows(
    tx: &Transaction<'_>,
    wt: i64,
    query: &SimilarQuery,
    exclude: Option<&(String, usize)>,
    listed: &BTreeSet<(String, usize)>,
) -> Result<Vec<SimilarRow>, StoreError> {
    if query.titles.is_empty() {
        return Ok(Vec::new());
    }
    let mut statement = tx
        .prepare(&format!(
            "SELECT {NODE_COLUMNS}, a.alias FROM aliases a
             JOIN nodes n ON n.node_id = a.node_id
             JOIN files f ON f.file_id = n.file_id
             WHERE f.wt = ?1 AND f.tier3 = 0
             ORDER BY f.path, n.ord, a.ord"
        ))
        .db()?;
    let mut by_node: BTreeMap<(String, usize), (Found, Vec<String>)> = BTreeMap::new();
    let mut rows = statement.query([wt]).db()?;
    while let Some(row) = rows.next().db()? {
        let node = found_of(row).db()?;
        let alias: String = row.get(5).db()?;
        by_node
            .entry(node.key())
            .or_insert_with(|| (node, Vec::new()))
            .1
            .push(alias);
    }
    let mut out = Vec::new();
    for (key, (node, aliases)) in by_node {
        if exclude == Some(&key) || listed.contains(&key) {
            continue;
        }
        let normalized: Vec<String> = aliases
            .iter()
            .map(|alias| normalized_summary(alias))
            .collect();
        let matched = query.titles.iter().find_map(|title| {
            normalized
                .iter()
                .position(|alias| !title.normalized.is_empty() && *alias == title.normalized)
                .map(|at| (title.new.clone(), aliases[at].clone()))
        });
        if let Some((new, alias)) = matched {
            out.push(node.row(SimilarSource::Alias, Some(new), vec![alias], None));
        }
    }
    Ok(out)
}

/// Probe 4: the query's terms ORed, each matching at most
/// `caps.term_nodes_max` nodes of the corpus (a commoner one dropped); the
/// top `caps.search_max` by bm25, path, ord, the nodes listed before
/// skipped but counted in the rank.
fn search_rows(
    tx: &Transaction<'_>,
    wt: i64,
    query: &SimilarQuery,
    exclude: Option<&(String, usize)>,
    listed: &BTreeSet<(String, usize)>,
    caps: &SimilarCaps,
) -> Result<Vec<SimilarRow>, StoreError> {
    let mut kept = Vec::new();
    for term in &query.terms {
        // A U+0000 would end the FTS5 string early (core's `term_words`
        // splits there; a caller's own term holding one is left out).
        if term.contains('\0') || term.chars().count() < MIN_TERM_CHARS {
            continue;
        }
        if term_nodes(tx, wt, term, caps.term_nodes_max)? <= caps.term_nodes_max {
            kept.push(term.clone());
        }
    }
    if kept.is_empty() || caps.search_max == 0 {
        return Ok(Vec::new());
    }
    let matching: Vec<String> = kept.iter().map(|term| fts_string(term)).collect();
    let mut values = vec![Value::Text(matching.join(" OR ")), Value::Integer(wt)];
    let mut filter = String::new();
    if let Some((path, ord)) = exclude {
        filter = " AND NOT (f.path = ?3 AND n.ord = ?4)".to_owned();
        values.push(Value::Text(path.clone()));
        values.push(Value::Integer(count(*ord)));
    }
    let limit = caps.search_max + listed.len();
    let limit_param = values.len() + 1;
    values.push(Value::Integer(count(limit)));
    let found = nodes_of(
        tx,
        &format!(
            "SELECT {NODE_COLUMNS} {MATCHED}{filter}
             ORDER BY bm25(nodes_fts, 10.0, 5.0, 1.0), f.path, n.ord
             LIMIT ?{limit_param}"
        ),
        values,
    )?;
    Ok(found
        .into_iter()
        .enumerate()
        .filter(|(_, node)| !listed.contains(&node.key()))
        .take(caps.search_max)
        .map(|(at, node)| node.row(SimilarSource::Search, None, kept.clone(), Some(at + 1)))
        .collect())
}

/// The corpus nodes `term` matches, counted up to `max + 1`.
fn term_nodes(tx: &Transaction<'_>, wt: i64, term: &str, max: usize) -> Result<usize, StoreError> {
    let counted: i64 = tx
        .query_row(
            &format!("SELECT count(*) FROM (SELECT 1 {MATCHED} LIMIT ?3)"),
            (fts_string(term), wt, count(max.saturating_add(1))),
            |row| row.get(0),
        )
        .db()?;
    Ok(usize::try_from(counted).unwrap_or(usize::MAX))
}

/// Probe 5: the nodes of the query's kind (strings compared) holding
/// front-matter edges (not `mentions`, the holder their source) to at least
/// `caps.shared_min` of its targets; a target with more than
/// `caps.target_degree_max` such edges skipped. The top
/// `caps.neighbours_max` by the shared count descending, path, ord.
fn neighbour_rows(
    tx: &Transaction<'_>,
    wt: i64,
    query: &SimilarQuery,
    exclude: Option<&(String, usize)>,
    listed: &BTreeSet<(String, usize)>,
    caps: &SimilarCaps,
) -> Result<Vec<SimilarRow>, StoreError> {
    let Some(kind) = query.kind.as_deref() else {
        return Ok(Vec::new());
    };
    let mut targets: Vec<&SimilarTarget> = Vec::new();
    for target in &query.links {
        if !targets.iter().any(|seen| seen.name == target.name) {
            targets.push(target);
        }
    }
    if targets.len() < caps.shared_min.max(1) || caps.neighbours_max == 0 {
        return Ok(Vec::new());
    }
    let mut statement = tx
        .prepare(
            "SELECT f.path, e.ord, e.holder FROM edges e CROSS JOIN files f
               ON f.file_id = e.file_id
             WHERE e.end_path = ?2 AND e.end_ord = ?3 AND e.names_source = 0
               AND e.origin = ?4 AND e.type <> ?5 AND f.wt = ?1 AND f.tier3 = 0
             LIMIT ?6",
        )
        .db()?;
    let frontmatter = origin_text(LinkOrigin::Frontmatter);
    let limit = count(caps.target_degree_max.saturating_add(1));
    // Holder → the targets it shares, by their place in `targets`.
    let mut holders: BTreeMap<(String, usize), BTreeSet<usize>> = BTreeMap::new();
    for (at, target) in targets.iter().enumerate() {
        let mut edges: BTreeSet<(String, i64)> = BTreeSet::new();
        let mut held: BTreeSet<(String, usize)> = BTreeSet::new();
        for (end_path, end_ord) in &target.ends {
            let mut rows = statement
                .query((wt, end_path, count(*end_ord), frontmatter, MENTIONS, limit))
                .db()?;
            while let Some(row) = rows.next().db()? {
                let path: String = row.get(0).db()?;
                let edge: i64 = row.get(1).db()?;
                let holder: i64 = row.get(2).db()?;
                edges.insert((path.clone(), edge));
                held.insert((path, usize::try_from(holder).unwrap_or(usize::MAX)));
            }
        }
        if edges.len() > caps.target_degree_max {
            continue;
        }
        for key in held {
            holders.entry(key).or_default().insert(at);
        }
    }
    let mut node = tx
        .prepare(&format!(
            "SELECT {NODE_COLUMNS} FROM files f JOIN nodes n ON n.file_id = f.file_id
             WHERE f.wt = ?1 AND f.path = ?2 AND n.ord = ?3"
        ))
        .db()?;
    let mut candidates: Vec<(usize, Found, Vec<String>)> = Vec::new();
    for ((path, ord), shared) in holders {
        let key = (path, ord);
        if shared.len() < caps.shared_min || exclude == Some(&key) || listed.contains(&key) {
            continue;
        }
        let Some(found) = node
            .query_row((wt, &key.0, count(key.1)), found_of)
            .optional()
            .db()?
        else {
            continue;
        };
        if found.kind.as_deref() != Some(kind) {
            continue;
        }
        let names = shared.iter().map(|&at| targets[at].name.clone()).collect();
        candidates.push((shared.len(), found, names));
    }
    candidates.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.path.cmp(&b.1.path))
            .then_with(|| a.1.ord.cmp(&b.1.ord))
    });
    Ok(candidates
        .into_iter()
        .take(caps.neighbours_max)
        .map(|(_, found, names)| found.row(SimilarSource::Neighbours, None, names, None))
        .collect())
}

/// The model's spelling of an edge's origin, as the edge table stores it.
fn origin_text(origin: LinkOrigin) -> &'static str {
    match origin {
        LinkOrigin::Frontmatter => "frontmatter",
        LinkOrigin::Inline => "inline",
    }
}

/// Each titled node of `nodes` with its normalized title; untitled ones
/// left out.
fn normalized(nodes: Vec<Found>) -> Vec<(Found, String)> {
    nodes
        .into_iter()
        .filter_map(|node| {
            let title = normalized_summary(node.title.as_deref()?);
            Some((node, title))
        })
        .collect()
}

/// The nodes `sql` gives over `values`, each [`NODE_COLUMNS`] first.
fn nodes_of(tx: &Transaction<'_>, sql: &str, values: Vec<Value>) -> Result<Vec<Found>, StoreError> {
    let mut statement = tx.prepare(sql).db()?;
    let mut rows = statement.query(params_from_iter(values)).db()?;
    let mut found = Vec::new();
    while let Some(row) = rows.next().db()? {
        found.push(found_of(row).db()?);
    }
    Ok(found)
}

fn found_of(row: &rusqlite::Row<'_>) -> rusqlite::Result<Found> {
    Ok(Found {
        path: row.get(0)?,
        ord: usize::try_from(row.get::<_, i64>(1)?).unwrap_or(usize::MAX),
        id: row.get(2)?,
        title: row.get(3)?,
        kind: row.get(4)?,
    })
}

fn count(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    //! docs/features/intake-similar.md "Acceptance criteria" IS4, the store's
    //! unit: the probes' caps (W5) on a small corpus this module writes to a
    //! scratch directory (neutral words, no project's prefix or kind). With
    //! every term common (the term cap set to 1) the search lists nothing; a
    //! target past the degree cap is skipped (more than the cap, never at
    //! it); the search, neighbour and shared-count caps bound their rows.

    use std::path::PathBuf;

    use specengine_core::similar::SimilarSource;
    use specengine_core::{IdSchemeToml, Paths};
    use specengine_model::IdScheme;

    use super::{
        SimilarCaps, SimilarQuery, SimilarRow, SimilarTarget, SimilarTitle, similar_capped,
    };
    use crate::IndexWriter;
    use crate::index::SqliteIndex;
    use crate::source::WorkingTree;

    /// The corpus's config: one name-shaped prefix of a neutral kind.
    const CONFIG: &str = "[project]\nname = \"caps\"\nslug = \"caps\"\n\n[ids]\n\
                          PLT = { kind = \"plot\", shape = \"name\" }\n";

    /// A scratch directory, removed on drop.
    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A document `id` titled `title`, `extra` closing its front-matter.
    fn document(id: &str, title: &str, extra: &str) -> String {
        format!(
            "---\nid: {id}\nclass: canon\nstatus: accepted\n{extra}---\n\n# {title}\n\n\
             A rope knot holds the boat.\n"
        )
    }

    /// Two targets, a holder of edges to both, and seven documents holding
    /// the words `rope` and `knot`, indexed into a scratch database.
    fn indexed(name: &str) -> (Scratch, SqliteIndex) {
        let dir = std::env::temp_dir().join(format!(
            "specengine-store-similar-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let root = dir.join("wt");
        let spec = root.join("docs/spec");
        std::fs::create_dir_all(&spec).expect("scratch");
        std::fs::write(root.join("specengine.toml"), CONFIG).expect("config");
        let mut files = vec![
            ("base.md", document("PLT-BASE", "Base", "")),
            ("tide.md", document("PLT-TIDE", "Tide", "")),
            (
                "harbor.md",
                document(
                    "PLT-HARBOR",
                    "Harbor",
                    "links:\n  depends_on: [PLT-BASE, PLT-TIDE]\n",
                ),
            ),
        ];
        for (n, word) in ["one", "two", "three", "four", "five", "six", "seven"]
            .iter()
            .enumerate()
        {
            files.push((
                [
                    "one.md", "two.md", "three.md", "four.md", "five.md", "six.md", "seven.md",
                ][n],
                document(
                    &format!("PLT-{}", word.to_uppercase()),
                    &format!("Rope {word}"),
                    "",
                ),
            ));
        }
        for (file, text) in &files {
            std::fs::write(spec.join(file), text).expect("a document");
        }
        let root = root.canonicalize().expect("canonical");
        let scheme = IdScheme::from_toml(CONFIG).expect("the scheme");
        let paths = Paths::from_toml(CONFIG).expect("the paths");
        let mut index = SqliteIndex::open(dir.join("index.db"), "caps", &root).expect("open");
        index
            .update(&WorkingTree::new(&root, &paths).expect("a tree"), &scheme)
            .expect("update");
        (Scratch(dir), index)
    }

    fn rows(index: &SqliteIndex, query: &SimilarQuery, caps: &SimilarCaps) -> Vec<SimilarRow> {
        similar_capped(index, query, caps).expect("similar")
    }

    fn of(rows: &[SimilarRow], source: SimilarSource) -> usize {
        rows.iter().filter(|row| row.source == source).count()
    }

    /// A title no node holds, two terms seven nodes hold.
    fn common_terms() -> SimilarQuery {
        SimilarQuery {
            titles: vec![SimilarTitle {
                new: "PLT-NET".to_owned(),
                normalized: "rope knot net".to_owned(),
            }],
            terms: vec!["rope".to_owned(), "knot".to_owned()],
            kind: None,
            links: Vec::new(),
            exclude: None,
        }
    }

    /// The holder's two targets, for its kind.
    fn shared_targets() -> SimilarQuery {
        SimilarQuery {
            titles: vec![SimilarTitle {
                new: "PLT-QUAY".to_owned(),
                normalized: "quay".to_owned(),
            }],
            terms: Vec::new(),
            kind: Some("plot".to_owned()),
            links: vec![
                SimilarTarget {
                    name: "PLT-BASE".to_owned(),
                    ends: vec![("docs/spec/base.md".to_owned(), 0)],
                },
                SimilarTarget {
                    name: "PLT-TIDE".to_owned(),
                    ends: vec![("docs/spec/tide.md".to_owned(), 0)],
                },
            ],
            exclude: None,
        }
    }

    #[test]
    fn every_common_term_dropped_lists_no_search_row() {
        let (_scratch, index) = indexed("terms");
        let engine = rows(&index, &common_terms(), &SimilarCaps::ENGINE);
        assert_eq!(of(&engine, SimilarSource::Search), 5, "{engine:?}");
        let one = SimilarCaps {
            term_nodes_max: 1,
            ..SimilarCaps::ENGINE
        };
        let capped = rows(&index, &common_terms(), &one);
        assert_eq!(capped, Vec::new(), "every term common: no search row");
        let two = SimilarCaps {
            search_max: 2,
            ..SimilarCaps::ENGINE
        };
        let fewer = rows(&index, &common_terms(), &two);
        assert_eq!(fewer, engine[..2].to_vec(), "the cap keeps the best two");
    }

    #[test]
    fn a_target_past_the_degree_cap_is_skipped() {
        let (_scratch, index) = indexed("degree");
        let engine = rows(&index, &shared_targets(), &SimilarCaps::ENGINE);
        assert_eq!(of(&engine, SimilarSource::Neighbours), 1, "{engine:?}");
        assert_eq!(engine[0].id.as_deref(), Some("PLT-HARBOR"));
        assert_eq!(engine[0].detail, ["PLT-BASE", "PLT-TIDE"]);
        let at_cap = SimilarCaps {
            target_degree_max: 1,
            ..SimilarCaps::ENGINE
        };
        assert_eq!(
            rows(&index, &shared_targets(), &at_cap),
            engine,
            "at the cap"
        );
        let none = SimilarCaps {
            target_degree_max: 0,
            ..SimilarCaps::ENGINE
        };
        assert_eq!(rows(&index, &shared_targets(), &none), Vec::new());
        let shut = SimilarCaps {
            neighbours_max: 0,
            ..SimilarCaps::ENGINE
        };
        assert_eq!(rows(&index, &shared_targets(), &shut), Vec::new());
        let three = SimilarCaps {
            shared_min: 3,
            ..SimilarCaps::ENGINE
        };
        assert_eq!(rows(&index, &shared_targets(), &three), Vec::new());
    }
}
