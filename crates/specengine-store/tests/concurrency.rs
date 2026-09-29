//! AC-17 of docs/features/spec-index.md: one DB, a handle per thread — two
//! writers doing 20 edit-and-update rounds each, a rebuilder, a reader
//! looping `search` and `files()` — within 60 s: no error reaches a caller
//! (WAL, `Immediate` writes, `busy_timeout`), `files()` never shrinks after
//! the first index (a rebuild replaces rows in one transaction), and after
//! the join one update gives a fresh index's dump.

#![cfg(unix)]

mod common;

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use common::{Corpus, Scratch, assert_equals_fresh, open};
use specengine_store::{IndexWriter, SearchQuery, SpecIndex};

const ROUNDS: usize = 20;
const BOUND: Duration = Duration::from_secs(60);
/// The loops stop here at the latest, so the test ends inside `BOUND`.
const DEADLINE: Duration = Duration::from_secs(50);

#[test]
fn writers_a_rebuilder_and_a_reader_share_one_db_without_errors() {
    let started = Instant::now();
    let scratch = Scratch::new("concurrency");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let db = scratch.db("index");
    let initial = {
        let mut index = corpus.open(&db);
        corpus.update(&mut index);
        index.files().expect("files").len()
    };
    assert!(initial > 0);

    let errors: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let writers_done = AtomicUsize::new(0);
    let rebuilds = AtomicUsize::new(0);
    let reads = AtomicUsize::new(0);
    let smallest = AtomicUsize::new(usize::MAX);
    let record = |who: &str, error: String| {
        errors.lock().unwrap().push(format!("{who}: {error}"));
    };
    let running = || writers_done.load(Ordering::SeqCst) < 2 && started.elapsed() < DEADLINE;

    std::thread::scope(|scope| {
        for (writer, path) in [(1, "docs/spec/game.md"), (2, "docs/records/R/R-12.md")] {
            let corpus = &corpus;
            let db = &db;
            let record = &record;
            let writers_done = &writers_done;
            scope.spawn(move || {
                let mut index = open(db, &corpus.root);
                let tree = corpus.tree();
                for round in 0..ROUNDS {
                    if started.elapsed() >= DEADLINE {
                        record(&format!("writer {writer}"), "deadline reached".to_owned());
                        break;
                    }
                    let text = corpus.read_text(path);
                    corpus.write(path, format!("{text}\nRound {round} of writer {writer}.\n"));
                    if let Err(error) = index.update(&tree, &corpus.scheme) {
                        record(&format!("writer {writer} round {round}"), error.to_string());
                    }
                }
                writers_done.fetch_add(1, Ordering::SeqCst);
            });
        }
        scope.spawn(|| {
            let mut index = open(&db, &corpus.root);
            let tree = corpus.tree();
            while running() {
                match index.rebuild(&tree, &corpus.scheme) {
                    Ok(_) => {
                        rebuilds.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(error) => record("rebuilder", error.to_string()),
                }
            }
        });
        scope.spawn(|| {
            let index = open(&db, &corpus.root);
            let query = SearchQuery::new("stamina");
            while running() {
                match index.files() {
                    Ok(files) => {
                        smallest.fetch_min(files.len(), Ordering::SeqCst);
                        if files.len() < initial {
                            record("reader", format!("files() shrank to {}", files.len()));
                        }
                    }
                    Err(error) => record("reader files()", error.to_string()),
                }
                match index.search(&query) {
                    Ok(results) if results.hits.is_empty() => {
                        record("reader", "search found nothing".to_owned());
                    }
                    Ok(_) => {}
                    Err(error) => record("reader search()", error.to_string()),
                }
                reads.fetch_add(1, Ordering::SeqCst);
            }
        });
    });

    let errors = errors.into_inner().unwrap();
    assert!(
        errors.is_empty(),
        "{} errors reached a caller, first ones:\n{}",
        errors.len(),
        errors
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(rebuilds.load(Ordering::SeqCst) > 0, "the rebuilder ran");
    assert!(reads.load(Ordering::SeqCst) > 0, "the reader ran");
    assert!(
        smallest.load(Ordering::SeqCst) >= initial,
        "files() shrank to {}",
        smallest.load(Ordering::SeqCst)
    );

    let mut index = corpus.open(&db);
    corpus.update(&mut index);
    assert_equals_fresh(&index, &corpus, &scratch, "the concurrent rounds");
    index.check_fts().expect("FTS5 integrity-check");
    let elapsed = started.elapsed();
    eprintln!(
        "concurrency: {} rebuilds, {} reads in {elapsed:?}",
        rebuilds.load(Ordering::SeqCst),
        reads.load(Ordering::SeqCst)
    );
    assert!(elapsed < BOUND, "took {elapsed:?}, the bound is {BOUND:?}");
}
