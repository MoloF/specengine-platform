//! AC-18 of docs/features/spec-parser.md: parsing cost is linear; its bar
//! made relative by docs/features/write-cost.md (RM-0125) "Test hygiene"
//! and WC12. Each generated input is built at full size and at one tenth:
//! a 1 MB single line against 100 KB, 100 000 `{#`, `[[` and ID-like tokens
//! against 10 000. Both parse on the same kind of spawned thread (an 8 MiB
//! stack, the main thread's size: stack depth is `crafted_yaml.rs`'s
//! concern, not this file's), interleaved, best of three, and must satisfy
//! T(full) <= 30 x T(tenth) + 250 ms ([`assert_same_order`]). Linear gives
//! a ratio of 10 and n log n about 12.5; a rescan from the line start per
//! opener is quadratic, a ratio of 100. A parse that never ends fails on
//! its 60 s guard ([`GUARD`]) instead of hanging the run.
//!
//! The bar is a ratio, not a wall-clock limit, so it holds under another
//! build's load (WC12 is run with twice the cores of `yes` beside it).
//!
//! WC12's mutations: the `{#` scan restarted per opener
//! (`hundred_thousand_attribute_openers`); a per-heading scan from the
//! start of the file (`hundred_thousand_nested_headings`).

mod common;

use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use specengine_model::{IdScheme, ParsedFile, PrefixSpec};

/// The full count of openers, tokens and headings; the tenth is a tenth.
const N: usize = 100_000;
/// The full single line, in bytes.
const ONE_MB: usize = 1 << 20;
/// T(full) <= FACTOR x T(tenth) + ALLOWANCE.
const FACTOR: u32 = 30;
const ALLOWANCE: Duration = Duration::from_millis(250);
/// The most one parse may take before the run calls it a hang.
const GUARD: Duration = Duration::from_secs(60);
/// The worker thread's stack: the main thread's size.
const STACK: usize = 8 << 20;

fn scheme() -> IdScheme {
    IdScheme::new(vec![
        PrefixSpec::number("R", "requirement", 2),
        PrefixSpec::number("AC", "criterion", 2),
        PrefixSpec::name("RULE", "rule"),
    ])
    .unwrap()
}

/// `text` parsed on a fresh worker thread: the parse and its time, measured
/// on that thread. Waits at most [`GUARD`].
fn parse_on_worker(name: &str, text: Arc<String>) -> (ParsedFile, Duration) {
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name(name.to_owned())
        .stack_size(STACK)
        .spawn(move || {
            let started = Instant::now();
            let parsed = specengine_core::parse("big.md", text.as_bytes(), &scheme());
            let elapsed = started.elapsed();
            let _ = sender.send((parsed, elapsed));
        })
        .expect("worker thread");
    match receiver.recv_timeout(GUARD) {
        Ok(done) => done,
        Err(error) => panic!("{name}: not parsed within {GUARD:?} ({error})"),
    }
}

/// `slow` costs the same order as `fast`, ten times smaller: at most
/// [`FACTOR`] times it plus [`ALLOWANCE`] for scheduler noise.
fn assert_same_order(what: &str, slow: Duration, fast: Duration) {
    let bound = fast * FACTOR + ALLOWANCE;
    eprintln!(
        "{what}: {slow:?} against {fast:?} (ratio {:.1}), bound {bound:?}",
        slow.as_secs_f64() / fast.as_secs_f64().max(1e-9)
    );
    assert!(
        slow <= bound,
        "{what}: {slow:?} against a tenth's {fast:?} (bound {bound:?}): the parse is not linear"
    );
}

/// The input `build(full)` against `build(full / 10)`: both parsed three
/// times, interleaved (tenth, full, tenth, full, ...), the best of each
/// compared; the full input's parse.
fn relative(name: &str, full: usize, build: impl Fn(usize) -> String) -> ParsedFile {
    let tenth_text = Arc::new(build(full / 10));
    let full_text = Arc::new(build(full));
    let mut best_tenth = Duration::MAX;
    let mut best_full = Duration::MAX;
    let mut parsed = None;
    for round in 0..3 {
        let (_, tenth) = parse_on_worker(&format!("{name}/10#{round}"), Arc::clone(&tenth_text));
        best_tenth = best_tenth.min(tenth);
        let (file, whole) = parse_on_worker(&format!("{name}#{round}"), Arc::clone(&full_text));
        best_full = best_full.min(whole);
        parsed = Some(file);
    }
    assert_same_order(name, best_full, best_tenth);
    parsed.expect("three rounds")
}

/// `unit` repeated to at least `bytes` bytes, on one line.
fn line_of(unit: &str, bytes: usize) -> String {
    let mut line = String::with_capacity(bytes + unit.len());
    while line.len() < bytes {
        line.push_str(unit);
    }
    line
}

#[test]
fn a_one_megabyte_single_line_of_references() {
    let parsed = relative("1mb-refs", ONE_MB, |bytes| {
        line_of(
            "R-12 a/R-1 p:s/AC-3@2 [[R-2|x]] RULE-A-B#RULE-C {#R-4} ",
            bytes,
        )
    });
    assert!(parsed.links.len() > 60_000, "{}", parsed.links.len());
}

#[test]
fn a_one_megabyte_single_line_without_references() {
    relative("1mb-words", ONE_MB, |bytes| {
        line_of("lorem-ipsum dolor_sit amet-12 ", bytes)
    });
}

#[test]
fn a_one_megabyte_single_line_of_one_word() {
    // One letter-digit run followed by a dash: the candidate run is bounded.
    let parsed = relative("1mb-run", ONE_MB, |bytes| {
        let mut text = "R".repeat(bytes);
        text.push_str("-12");
        text
    });
    assert!(parsed.links.is_empty());
}

#[test]
fn a_one_megabyte_slug_before_one_reference() {
    let parsed = relative("1mb-slug", ONE_MB, |bytes| {
        let mut text = "a".repeat(bytes);
        text.push_str("/R-12");
        text
    });
    assert_eq!(parsed.links.len(), 1);
}

#[test]
fn hundred_thousand_attribute_openers() {
    relative("{#-paragraph", N, |n| "{#".repeat(n));
    relative("{#-heading", N, |n| format!("## T {}\n", "{#".repeat(n)));
    relative("{#R-", N, |n| format!("## T {}\n", "{#R-".repeat(n)));
    let parsed = relative("{#-headings", N, |n| {
        (0..n).map(|i| format!("## H {{#R-{i}}}\n")).collect()
    });
    assert_eq!(parsed.sections().len(), N);
}

#[test]
fn hundred_thousand_wiki_openers() {
    relative("[[", N, |n| "[[".repeat(n));
    relative("[[R-1|", N, |n| "[[R-1|".repeat(n));
    let parsed = relative("[[R-1]]", N, |n| "[[R-1]] ".repeat(n));
    assert_eq!(parsed.links.len(), N);
    relative("[[R-1@", N, |n| "[[R-1@".repeat(n));
}

#[test]
fn hundred_thousand_id_like_tokens() {
    let parsed = relative("R-1", N, |n| "R-1 ".repeat(n));
    assert_eq!(parsed.links.len(), N);
    relative("R-", N, |n| "R-".repeat(n));
    relative("R-1-", N, |n| "R-1-".repeat(n));
    relative("RULE-A-", N, |n| "RULE-A-".repeat(n));
    relative("R-1#", N, |n| "R-1#".repeat(n));
    relative("R-1@", N, |n| "R-1@".repeat(n));
    relative("s/", N, |n| "s/R-1".repeat(n));
    relative("p:", N, |n| "p:s/".repeat(n));
    relative("R-lines", N, |n| {
        (0..n).map(|i| format!("R-{i}\n")).collect()
    });
}

#[test]
fn hundred_thousand_nested_headings() {
    // Alternating levels: section extents must be found with a stack.
    let parsed = relative("levels", N, |n| {
        (0..n)
            .map(|i| format!("{} H {{#R-{i}}}\n", "#".repeat(1 + i % 6)))
            .collect()
    });
    assert_eq!(parsed.sections().len(), N);
}
