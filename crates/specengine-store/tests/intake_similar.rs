//! docs/features/intake-similar.md (RM-0059) "Acceptance criteria", the
//! store's part of IS1 to IS5: `SpecIndex::similar` (probes 1, 2, 4 and 5)
//! over scratch copies of `fixtures/spec-a`, indexed by `update`, with
//! invented documents added (a lamp with an alias, a night watch twice and in
//! a shipped feature, an ox).
//!
//! - IS1: an equal normalized title is a `Title` row naming the first new
//!   node it matched; the node found by title is never found again by search
//!   or as a neighbour. M: the title probe skipped.
//! - IS3: an alias equal to a title is an `Alias` row, its `detail` the
//!   alias. M: aliases not probed.
//! - IS4: search rows at most five, ranked; a neighbour shares at least two
//!   targets of the same kind, its `detail` the shared targets; no kind, no
//!   neighbour.
//! - IS5: path order, never rowid; a reversed walk and the edit path give
//!   the rows of a fresh index; the excluded node and Tier 3 files are never
//!   rows; a title without a term of three characters is scanned. M: order
//!   by rowid; the target not excluded.

#![cfg(unix)]

mod common;

use common::{Corpus, Reversed, Scratch};
use specengine_core::similar::SimilarSource;
use specengine_store::{
    IndexWriter, SimilarQuery, SimilarRow, SimilarTarget, SimilarTitle, SpecIndex, SqliteIndex,
};

const STAMINA: &str = "docs/spec/movement/stamina.md";
const SPRINT: &str = "docs/spec/movement/sprint.md";
const R12: &str = "docs/records/R/R-12.md";

/// A spec-a canon document under `DOM-GAME`.
fn canon(id: &str, title: &str, extra: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\ntier: 2\ntitle: {title}\nparent: DOM-GAME\nstatus: \
         accepted\nowner: owner\nreviewed: 2026-09-20\n{extra}---\n\n# {title}\n\nA body of \
         plain words.\n"
    )
}

fn title(new: &str, normalized: &str) -> SimilarTitle {
    SimilarTitle {
        new: new.to_owned(),
        normalized: normalized.to_owned(),
    }
}

/// MEC-SPRINT and R-12, the "Text" example's targets, resolved.
fn targets() -> Vec<SimilarTarget> {
    vec![
        SimilarTarget {
            name: "MEC-SPRINT".to_owned(),
            ends: vec![(SPRINT.to_owned(), 0)],
        },
        SimilarTarget {
            name: "R-12".to_owned(),
            ends: vec![(R12.to_owned(), 0)],
        },
    ]
}

fn similar(index: &SqliteIndex, query: &SimilarQuery) -> Vec<SimilarRow> {
    index
        .similar(query)
        .unwrap_or_else(|error| panic!("similar {query:?}: {error}"))
}

/// `(source, path, ord)` of each row.
fn keys(rows: &[SimilarRow]) -> Vec<(SimilarSource, String, usize)> {
    rows.iter()
        .map(|row| (row.source, row.path.clone(), row.ord))
        .collect()
}

/// The "Text" example's query: title "stamina", the keyword `drained`,
/// kind `mechanic`, the two targets.
fn example() -> SimilarQuery {
    SimilarQuery {
        titles: vec![title("MEC-ENDURANCE", "stamina")],
        terms: vec!["stamina".to_owned(), "drained".to_owned()],
        kind: Some("mechanic".to_owned()),
        links: targets(),
        exclude: None,
    }
}

/// IS1 and IS4 on the example: MEC-STAMINA a `Title` row first, naming
/// `MEC-ENDURANCE`; then at most five `Search` rows, ranked rising; never
/// MEC-STAMINA again (it shares both targets, but is listed by title).
#[test]
fn is1_the_title_row_first_and_never_again() {
    let scratch = Scratch::new("sim-is1");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let index = corpus.fresh(&scratch);
    let rows = similar(&index, &example());
    let first = rows.first().expect("a row");
    assert_eq!(first.source, SimilarSource::Title, "{rows:?}");
    assert_eq!(first.path, STAMINA);
    assert_eq!(first.ord, 0);
    assert_eq!(first.id.as_deref(), Some("MEC-STAMINA"));
    assert_eq!(first.title.as_deref(), Some("Stamina"));
    assert_eq!(first.kind.as_deref(), Some("mechanic"));
    assert_eq!(first.new.as_deref(), Some("MEC-ENDURANCE"));
    assert_eq!(
        rows.iter()
            .filter(|row| (row.path.as_str(), row.ord) == (STAMINA, 0))
            .count(),
        1,
        "{rows:?}"
    );
    let search: Vec<&SimilarRow> = rows
        .iter()
        .filter(|row| row.source == SimilarSource::Search)
        .collect();
    assert!(!search.is_empty() && search.len() <= 5, "{rows:?}");
    let ranks: Vec<usize> = search
        .iter()
        .map(|row| row.rank.expect("a search rank"))
        .collect();
    assert!(ranks.windows(2).all(|pair| pair[0] < pair[1]), "{ranks:?}");
    assert!(
        rows.iter()
            .all(|row| row.source == SimilarSource::Title || row.source == SimilarSource::Search),
        "{rows:?}"
    );
    // The search's terms kept are its detail.
    for row in &search {
        assert!(!row.detail.is_empty(), "{row:?}");
        assert!(
            row.detail
                .iter()
                .all(|term| term == "stamina" || term == "drained"),
            "{row:?}"
        );
    }
}

/// IS4: with no equal title, MEC-STAMINA is a `Neighbours` row sharing the
/// two targets, named in the query's order; a kind no node holds lists no
/// neighbour, nor does no kind; one shared target is not enough.
#[test]
fn is4_a_neighbour_shares_two_targets_of_the_same_kind() {
    let scratch = Scratch::new("sim-is4");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let index = corpus.fresh(&scratch);
    let query = SimilarQuery {
        titles: vec![title("MEC-ENDURANCE-POOL", "endurance pool")],
        terms: vec!["endurance".to_owned(), "pool".to_owned()],
        kind: Some("mechanic".to_owned()),
        links: targets(),
        exclude: None,
    };
    let rows = similar(&index, &query);
    let neighbours: Vec<&SimilarRow> = rows
        .iter()
        .filter(|row| row.source == SimilarSource::Neighbours)
        .collect();
    assert_eq!(neighbours.len(), 1, "{rows:?}");
    assert_eq!(neighbours[0].id.as_deref(), Some("MEC-STAMINA"));
    assert_eq!(neighbours[0].detail, ["MEC-SPRINT", "R-12"]);
    for kind in [Some("rule".to_owned()), None] {
        let mut other = query.clone();
        other.kind = kind.clone();
        assert!(
            similar(&index, &other)
                .iter()
                .all(|row| row.source != SimilarSource::Neighbours),
            "{kind:?}"
        );
    }
    let mut one = query.clone();
    one.links.truncate(1);
    assert!(
        similar(&index, &one)
            .iter()
            .all(|row| row.source != SimilarSource::Neighbours),
        "one shared target"
    );
}

/// IS3: an `aliases:` entry equal to the title is an `Alias` row, its
/// `detail` the alias as written.
#[test]
fn is3_an_alias_equal_to_the_title_is_a_row() {
    let scratch = Scratch::new("sim-is3");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    corpus.write(
        "docs/spec/keep/lamp.md",
        canon("MEC-LAMP", "Lamp", "aliases: [\"Old lamp\"]\n"),
    );
    let index = corpus.fresh(&scratch);
    let query = SimilarQuery {
        titles: vec![title("MEC-OLD-LAMP", "old lamp")],
        terms: vec!["old".to_owned(), "lamp".to_owned()],
        kind: None,
        links: Vec::new(),
        exclude: None,
    };
    let rows = similar(&index, &query);
    let first = rows.first().expect("a row");
    assert_eq!(first.source, SimilarSource::Alias, "{rows:?}");
    assert_eq!(first.id.as_deref(), Some("MEC-LAMP"));
    assert_eq!(first.path, "docs/spec/keep/lamp.md");
    assert_eq!(first.detail, ["Old lamp"]);
    assert_eq!(first.new.as_deref(), Some("MEC-OLD-LAMP"));
    assert_eq!(
        rows.iter()
            .filter(|row| row.id.as_deref() == Some("MEC-LAMP"))
            .count(),
        1,
        "{rows:?}"
    );
}

/// The night watch: `a.md` and `b.md` sharing a title, a shipped feature
/// holding it too; `a.md` edited after the first index.
fn night_watch(corpus: &Corpus) {
    corpus.write("docs/spec/a.md", canon("DOM-ALPHA", "Night watch", ""));
    corpus.write("docs/spec/b.md", canon("DOM-BETA", "Night  WATCH", ""));
    corpus.write(
        "docs/features/night-watch.md",
        "---\nclass: spec\nstatus: shipped\nscope: [keep]\n---\n\n# Night watch\n\nTuned.\n",
    );
}

/// IS5: `a.md` then `b.md` by path though `a.md`'s rows are the newer;
/// the Tier 3 feature never a row; a fresh index, a reversed walk and the
/// edit path give equal rows. M: order by rowid.
#[test]
fn is5_path_order_never_rowid_and_one_state_one_list() {
    let scratch = Scratch::new("sim-is5");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    night_watch(&corpus);
    let mut index = corpus.open(&scratch.db("edited"));
    corpus.update(&mut index);
    corpus.replace(
        "docs/spec/a.md",
        "A body of plain words.",
        "A body of plain words, edited.",
    );
    corpus.update(&mut index);
    let query = SimilarQuery {
        titles: vec![title("DOM-WATCH", "night watch")],
        terms: vec!["night".to_owned(), "watch".to_owned()],
        kind: Some("domain".to_owned()),
        links: Vec::new(),
        exclude: None,
    };
    let rows = similar(&index, &query);
    let titled: Vec<(String, Option<String>)> = rows
        .iter()
        .filter(|row| row.source == SimilarSource::Title)
        .map(|row| (row.path.clone(), row.id.clone()))
        .collect();
    assert_eq!(
        titled,
        [
            ("docs/spec/a.md".to_owned(), Some("DOM-ALPHA".to_owned())),
            ("docs/spec/b.md".to_owned(), Some("DOM-BETA".to_owned()))
        ],
        "{rows:?}"
    );
    assert!(
        rows.iter()
            .all(|row| row.path != "docs/features/night-watch.md"),
        "Tier 3 never a row: {rows:?}"
    );
    let fresh = corpus.fresh(&scratch);
    assert_eq!(similar(&fresh, &query), rows, "fresh");
    let mut reversed = corpus.open(&scratch.db("reversed"));
    reversed
        .update(&Reversed(&corpus.tree()), &corpus.scheme)
        .expect("a reversed update");
    assert_eq!(similar(&reversed, &query), rows, "reversed walk");
    assert_eq!(similar(&index, &query), rows, "a second read");
}

/// IS5: the excluded node (a section form's target) is never a row, by
/// title, search or as a neighbour.
#[test]
fn is5_the_excluded_node_is_never_a_row() {
    let scratch = Scratch::new("sim-is5-exclude");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let index = corpus.fresh(&scratch);
    let mut query = example();
    query.exclude = Some((STAMINA.to_owned(), 0));
    let rows = similar(&index, &query);
    assert!(
        rows.iter()
            .all(|row| (row.path.as_str(), row.ord) != (STAMINA, 0)),
        "{rows:?}"
    );
    assert!(
        rows.iter().all(|row| row.source != SimilarSource::Title),
        "{rows:?}"
    );
    // Without it the target is the title row.
    assert_eq!(similar(&index, &example())[0].path, STAMINA);
}

/// W9: a title with no term of three characters is scanned: "Ox" finds the
/// document titled "OX"; no search row without a term.
#[test]
fn a_title_without_a_long_term_is_scanned() {
    let scratch = Scratch::new("sim-short");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    corpus.write("docs/spec/keep/ox.md", canon("MEC-OX", "OX", ""));
    let index = corpus.fresh(&scratch);
    let query = SimilarQuery {
        titles: vec![title("MEC-YOKE", "ox")],
        terms: Vec::new(),
        kind: None,
        links: Vec::new(),
        exclude: None,
    };
    let rows = similar(&index, &query);
    assert_eq!(
        keys(&rows),
        [(SimilarSource::Title, "docs/spec/keep/ox.md".to_owned(), 0)],
        "{rows:?}"
    );
}

/// IS1, the review's iteration 2: a caller's term holding a U+0000 is left
/// out of the search (it would end the FTS5 string early), the call `Ok`, the
/// other terms searched; a title holding one splits its candidates' terms.
#[test]
fn a_term_holding_a_nul_is_left_out() {
    let scratch = Scratch::new("sim-nul");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let index = corpus.fresh(&scratch);
    let query = SimilarQuery {
        titles: vec![title("MEC-OIL", "lan\0tern oil")],
        terms: vec!["stam\0ina".to_owned(), "drained".to_owned()],
        kind: None,
        links: Vec::new(),
        exclude: None,
    };
    let rows = index.similar(&query).expect("Ok, the NUL term left out");
    assert!(!rows.is_empty(), "`drained` is searched");
    for row in &rows {
        assert_eq!(row.source, SimilarSource::Search, "{row:?}");
        assert_eq!(row.detail, ["drained"], "{row:?}");
    }
    let only = SimilarQuery {
        terms: vec!["\0".to_owned(), "a\0b".to_owned()],
        ..query
    };
    assert_eq!(index.similar(&only).expect("Ok"), Vec::new());
}
