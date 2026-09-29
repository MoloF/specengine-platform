//! AC-18 of docs/features/spec-parser.md: parsing cost is linear. Each
//! generated input parses within 2 s in a debug build: a 1 MB single line;
//! 100 000 `{#`; 100 000 `[[`; 100 000 ID-like tokens. A rescan from the
//! line start per opener turns any of them quadratic (a timeout).
//!
//! The parse runs on a worker thread with an 8 MiB stack (the main-thread
//! size: stack depth is `crafted_yaml.rs`'s concern, not this file's).

mod common;

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use specengine_model::{IdScheme, ParsedFile, PrefixSpec};

const LIMIT: Duration = Duration::from_secs(2);
const N: usize = 100_000;

fn scheme() -> IdScheme {
    IdScheme::new(vec![
        PrefixSpec::number("R", "requirement", 2),
        PrefixSpec::number("AC", "criterion", 2),
        PrefixSpec::name("RULE", "rule"),
    ])
    .unwrap()
}

fn timed(name: &str, text: String) -> ParsedFile {
    let (sender, receiver) = mpsc::channel();
    let started = Instant::now();
    thread::Builder::new()
        .name(name.to_owned())
        .stack_size(8 << 20)
        .spawn(move || {
            let parsed = specengine_core::parse("big.md", text.as_bytes(), &scheme());
            let _ = sender.send(parsed);
        })
        .expect("worker thread");
    match receiver.recv_timeout(LIMIT) {
        Ok(parsed) => {
            eprintln!("{name}: {:?}", started.elapsed());
            parsed
        }
        Err(error) => panic!("{name}: not parsed within {LIMIT:?} ({error})"),
    }
}

fn one_mb_line(unit: &str) -> String {
    let mut line = String::with_capacity(1 << 20);
    while line.len() < 1 << 20 {
        line.push_str(unit);
    }
    line
}

#[test]
fn a_one_megabyte_single_line_of_references() {
    let parsed = timed(
        "1mb-refs",
        one_mb_line("R-12 a/R-1 p:s/AC-3@2 [[R-2|x]] RULE-A-B#RULE-C {#R-4} "),
    );
    assert!(parsed.links.len() > 60_000, "{}", parsed.links.len());
}

#[test]
fn a_one_megabyte_single_line_without_references() {
    timed("1mb-words", one_mb_line("lorem-ipsum dolor_sit amet-12 "));
}

#[test]
fn a_one_megabyte_single_line_of_one_word() {
    // One letter-digit run followed by a dash: the candidate run is bounded.
    let mut text = "R".repeat(1 << 20);
    text.push_str("-12");
    let parsed = timed("1mb-run", text);
    assert!(parsed.links.is_empty());
}

#[test]
fn a_one_megabyte_slug_before_one_reference() {
    let mut text = "a".repeat(1 << 20);
    text.push_str("/R-12");
    let parsed = timed("1mb-slug", text);
    assert_eq!(parsed.links.len(), 1);
}

#[test]
fn hundred_thousand_attribute_openers() {
    timed("{#-paragraph", "{#".repeat(N));
    timed("{#-heading", format!("## T {}\n", "{#".repeat(N)));
    timed("{#R-", format!("## T {}\n", "{#R-".repeat(N)));
    let headings: String = (0..N).map(|i| format!("## H {{#R-{i}}}\n")).collect();
    let parsed = timed("{#-headings", headings);
    assert_eq!(parsed.sections().len(), N);
}

#[test]
fn hundred_thousand_wiki_openers() {
    timed("[[", "[[".repeat(N));
    timed("[[R-1|", "[[R-1|".repeat(N));
    let parsed = timed("[[R-1]]", "[[R-1]] ".repeat(N));
    assert_eq!(parsed.links.len(), N);
    timed("[[R-1@", "[[R-1@".repeat(N));
}

#[test]
fn hundred_thousand_id_like_tokens() {
    let parsed = timed("R-1", "R-1 ".repeat(N));
    assert_eq!(parsed.links.len(), N);
    timed("R-", "R-".repeat(N));
    timed("R-1-", "R-1-".repeat(N));
    timed("RULE-A-", "RULE-A-".repeat(N));
    timed("R-1#", "R-1#".repeat(N));
    timed("R-1@", "R-1@".repeat(N));
    timed("s/", "s/R-1".repeat(N));
    timed("p:", "p:s/".repeat(N));
    let lines: String = (0..N).map(|i| format!("R-{i}\n")).collect();
    timed("R-lines", lines);
}

#[test]
fn hundred_thousand_nested_headings() {
    // Alternating levels: section extents must be found with a stack.
    let text: String = (0..N)
        .map(|i| format!("{} H {{#R-{i}}}\n", "#".repeat(1 + i % 6)))
        .collect();
    let parsed = timed("levels", text);
    assert_eq!(parsed.sections().len(), N);
}
