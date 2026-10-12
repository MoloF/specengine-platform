//! docs/features/spec-lock.md TE-1, TE-2, TE-4, TE-5 and the named RD
//! tests of the accept itself (RD-5, RD-7, RD-8, RD-9, RD-17): `spec lock
//! accept` over scratch repositories of `fixtures/code-a`, through the
//! library at an injected clock (the lock's bytes reproducible), the
//! binary on a pseudo-terminal where the printed text is the subject.
//!
//! The golden (`fixtures/spec-lock/code-a.lock`) is the lock after
//! accepting PIECE-COUNTER (a document, `rev: 3`; a `//!` marker; a RON
//! `@configures`), LAW-BOUND (a section, `rev=2`), cap-check/PASS-01 (a
//! feature-scoped AC under `@verifies`) and GOAL-01 (an alias marker,
//! `AIM-01@1`), in that order, at [`CLOCK`]. Its keys and order are derived
//! by hand from the spec's "Data" and `fixtures/code-a/expected.json`; its
//! hashes and OIDs are recomputed here from their sources (git's blob and
//! commit OIDs, `spec show`'s norm hash, `specengine-code`'s full
//! analysis), and the file pins their values.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;
mod lock_common;

use std::fs;
use std::path::Path;

use common::{fixture, repository_root};
use lock_common::{
    CLOCK, DAY, DECIDER, HEADER, LockRepo, PREAMBLE, at, block, blocks, item_hash, lock_of,
    node_blocks, value,
};

const COUNTER: &str = "crates/tally/src/counter.rs";
const LIB: &str = "crates/tally/src/lib.rs";
const BOUNDS: &str = "crates/tally/tests/bounds.rs";
const MAIN: &str = "crates/tally/src/main.rs";
const BROKEN: &str = "crates/tally/src/broken.rs";
const LIMITS: &str = "data/limits.ron";

/// The four accepts of the golden, in order.
const GOLDEN_REFS: [&str; 4] = ["PIECE-COUNTER", "LAW-BOUND", "cap-check/PASS-01", "GOAL-01"];

fn golden_file() -> std::path::PathBuf {
    fixture("spec-lock").join("code-a.lock")
}

/// The four accepts of the golden in `repo`: the commit each ran at.
fn accept_golden(repo: &LockRepo) -> [String; 4] {
    let mut commits = Vec::new();
    for reference in GOLDEN_REFS {
        commits.push(repo.head());
        repo.accept_ok(reference);
    }
    commits.try_into().expect("four commits")
}

/// The golden lock of `repo` after [`accept_golden`] (`commits` the
/// commits they ran at), every hash and OID recomputed from its source.
fn expected_golden(repo: &LockRepo, commits: &[String; 4]) -> String {
    let c0 = &commits[0];
    let blob = |path: &str| repo.blob_at(c0, path);
    let ast =
        |path: &str, line: usize, kind: &str| item_hash(&repo.show_blob(c0, path), line, kind);
    let goal = repo.norm_hash("GOAL-01");
    let law = repo.norm_hash("LAW-BOUND");
    let piece = repo.norm_hash("PIECE-COUNTER");
    let pass = repo.norm_hash("cap-check/PASS-01");
    let (c_piece, c_law, c_pass, c_goal) = (&commits[0], &commits[1], &commits[2], &commits[3]);
    lock_of(&[
        block(
            "GOAL-01",
            1,
            &goal,
            "implements",
            COUNTER,
            "item",
            "crates/tally::counter::Tally::value",
            Some(("function_item", &ast(COUNTER, 30, "function_item"))),
            &blob(COUNTER),
            c_goal,
            DAY,
        ),
        block(
            "GOAL-01",
            1,
            &goal,
            "verifies",
            COUNTER,
            "item",
            "crates/tally::counter::inner::limit",
            Some(("function_item", &ast(COUNTER, 78, "function_item"))),
            &blob(COUNTER),
            c_goal,
            DAY,
        ),
        block(
            "LAW-BOUND",
            2,
            &law,
            "implements",
            COUNTER,
            "item",
            "crates/tally::counter::<Tally as fmt::Display>::fmt",
            Some(("function_item", &ast(COUNTER, 49, "function_item"))),
            &blob(COUNTER),
            c_law,
            DAY,
        ),
        block(
            "LAW-BOUND",
            2,
            &law,
            "verifies",
            BOUNDS,
            "item",
            "crates/tally::test:bounds::stops_at_cap",
            Some(("function_item", &ast(BOUNDS, 6, "function_item"))),
            &blob(BOUNDS),
            c_law,
            DAY,
        ),
        block(
            "LAW-BOUND",
            2,
            &law,
            "configures",
            COUNTER,
            "item",
            "crates/tally::counter::CAP",
            Some(("const_item", &ast(COUNTER, 8, "const_item"))),
            &blob(COUNTER),
            c_law,
            DAY,
        ),
        block(
            "LAW-BOUND",
            2,
            &law,
            "configures",
            LIMITS,
            "ron",
            "root.cap",
            None,
            &blob(LIMITS),
            c_law,
            DAY,
        ),
        block(
            "PIECE-COUNTER",
            3,
            &piece,
            "implements",
            COUNTER,
            "item",
            "crates/tally::counter::Tally",
            Some(("struct_item", &ast(COUNTER, 17, "struct_item"))),
            &blob(COUNTER),
            c_piece,
            DAY,
        ),
        block(
            "PIECE-COUNTER",
            3,
            &piece,
            "implements",
            LIB,
            "module",
            "crates/tally",
            None,
            &blob(LIB),
            c_piece,
            DAY,
        ),
        block(
            "PIECE-COUNTER",
            3,
            &piece,
            "verifies",
            BOUNDS,
            "module",
            "crates/tally::test:bounds",
            None,
            &blob(BOUNDS),
            c_piece,
            DAY,
        ),
        block(
            "PIECE-COUNTER",
            3,
            &piece,
            "configures",
            LIMITS,
            "ron",
            "root",
            None,
            &blob(LIMITS),
            c_piece,
            DAY,
        ),
        block(
            "cap-check/PASS-01",
            1,
            &pass,
            "verifies",
            BOUNDS,
            "item",
            "crates/tally::test:bounds::steps_by_one",
            Some(("function_item", &ast(BOUNDS, 16, "function_item"))),
            &blob(BOUNDS),
            c_pass,
            DAY,
        ),
    ])
}

// ------------------------------------------------------------------ TE-1

/// TE-1: the lock after the golden's four accepts is, byte for byte, the
/// hand-derived golden with every hash and OID recomputed from its source;
/// and the committed golden file pins those values.
#[test]
fn golden_lock_of_code_a() {
    let repo = LockRepo::code_a("lock-golden");
    let commits = accept_golden(&repo);
    let lock = repo.lock_text();
    assert_eq!(
        lock,
        expected_golden(&repo, &commits),
        "the hand-derived golden"
    );
    let golden = fs::read_to_string(golden_file()).expect("fixtures/spec-lock/code-a.lock");
    assert_eq!(lock, golden, "the pinned golden");
    assert!(lock.starts_with(&format!("{HEADER}\n{PREAMBLE}\n[[binding]]\n")));
}

/// TE-1: what each accept of the golden asked, and the commits it made.
#[test]
fn golden_accepts_ask_and_commit_once_each() {
    let repo = LockRepo::code_a("lock-golden-ask");
    let expected = [
        (
            "PIECE-COUNTER",
            "lock PIECE-COUNTER rev 3 (accept: 4 new, 0 refreshed, 0 kept, 0 dropped)",
        ),
        (
            "LAW-BOUND",
            "lock LAW-BOUND rev 2 (accept: 4 new, 0 refreshed, 0 kept, 0 dropped)",
        ),
        (
            "cap-check/PASS-01",
            "lock cap-check/PASS-01 rev 1 (accept: 1 new, 0 refreshed, 0 kept, 0 dropped)",
        ),
        (
            "GOAL-01",
            "lock GOAL-01 rev 1 (accept: 2 new, 0 refreshed, 0 kept, 0 dropped)",
        ),
    ];
    for (reference, head) in expected {
        let before = repo.head();
        let attempt = repo.accept(reference, true);
        let outcome = attempt.committed(reference);
        assert_eq!(
            attempt.question(),
            format!("{head}{}", at(&before, "main", &repo.root)),
            "{reference}"
        );
        let commit = outcome.commit.clone().expect("a commit");
        assert_eq!(repo.head(), commit, "{reference}: HEAD is the lock commit");
        assert_eq!(repo.parents(&commit), vec![before.clone()], "{reference}");
        assert_eq!(repo.changed(&commit), vec!["spec.lock"], "{reference}");
        assert_eq!(
            repo.message(&commit).trim_end(),
            format!(
                "spec: lock {reference}\n\nLock: {reference}\nLock-mode: accept\nDecided-by: {DECIDER}"
            ),
            "{reference}"
        );
        assert!(
            repo.porcelain().is_empty(),
            "{reference}: the tree is clean"
        );
    }
}

/// TE-1: the binary on a terminal: the rows on stdout before the
/// question, the question on stderr, `y` commits; stdout ends with the
/// commit. The lock equals the library's but for `verified_at`, the
/// process clock's UTC date.
#[test]
fn accept_on_a_terminal_commits_the_lock() {
    let repo = LockRepo::code_a("lock-pty");
    let head = repo.head();
    let run = repo.spec_pty(&repo.root, &["lock", "accept", "PIECE-COUNTER"], Some("y"));
    run.code(0);
    assert!(run.asked, "{}", run.show());
    let rows = "new | implements | crates/tally/src/counter.rs | item crates/tally::counter::Tally struct_item\n\
not locked | implements | crates/tally/src/counter.rs | module crates/tally::counter | spec_ahead\n\
not locked | implements | crates/tally/src/counter.rs | unanchored in_body | unanchored\n\
new | implements | crates/tally/src/lib.rs | module crates/tally\n\
new | verifies | crates/tally/tests/bounds.rs | module crates/tally::test:bounds\n\
new | configures | data/limits.ron | ron root\n";
    let commit = repo.head();
    assert_eq!(
        run.stdout,
        format!("{rows}committed {commit}\n"),
        "{}",
        run.show()
    );
    assert_eq!(
        run.stderr.trim_end(),
        format!(
            "lock PIECE-COUNTER rev 3 (accept: 4 new, 0 refreshed, 0 kept, 0 dropped){}",
            at(&head, "main", &repo.root)
        ),
        "{}",
        run.show()
    );
    let lock = repo.lock_text();
    let dates: Vec<String> = blocks(&lock)
        .iter()
        .map(|block| value(block, "verified_at").expect("verified_at").to_owned())
        .collect();
    assert_eq!(dates.len(), 4);
    let today = specengine_cli::utc_now()[..10].to_owned();
    for date in &dates {
        assert!(
            date.len() == 10 && (date.as_str() == today || date.as_str() < today.as_str()),
            "verified_at {date} is the run's UTC date ({today})"
        );
    }
    // The same accept through the library at CLOCK: equal but the date.
    let twin = LockRepo::code_a("lock-pty-twin");
    twin.accept_ok("PIECE-COUNTER");
    assert_eq!(
        lock.replace(
            &format!("verified_at = \"{}\"", dates[0]),
            &format!("verified_at = \"{DAY}\"")
        ),
        twin.lock_text()
    );
}

// ------------------------------------------------------------------ TE-2

/// TE-2, RD-7: every marker of PIECE-COUNTER, LAW-BOUND, GOAL-01, LAW-STEP
/// and PREM-001 as a row, in key order: `item` with a v2 digest, `module`,
/// `ron` locked (`new`); `spec_ahead`, `predated`, `bad_rev`,
/// `ambiguous <reason>`, `unanchored`, `cannot_verify` not (hand-derived
/// from `fixtures/code-a/expected.json` and "Lockability").
#[test]
fn every_row_of_code_a_is_judged_by_lockability() {
    let cases: [(&str, &str, &str); 5] = [
        (
            "PIECE-COUNTER",
            "lock PIECE-COUNTER rev 3 (accept: 4 new, 0 refreshed, 0 kept, 0 dropped)",
            "new | implements | crates/tally/src/counter.rs | item crates/tally::counter::Tally struct_item
not locked | implements | crates/tally/src/counter.rs | module crates/tally::counter | spec_ahead
not locked | implements | crates/tally/src/counter.rs | unanchored in_body | unanchored
new | implements | crates/tally/src/lib.rs | module crates/tally
new | verifies | crates/tally/tests/bounds.rs | module crates/tally::test:bounds
new | configures | data/limits.ron | ron root
",
        ),
        (
            "LAW-BOUND",
            "lock LAW-BOUND rev 2 (accept: 4 new, 0 refreshed, 0 kept, 0 dropped)",
            "new | implements | crates/tally/src/counter.rs | item crates/tally::counter::<Tally as fmt::Display>::fmt function_item
not locked | implements | crates/tally/src/counter.rs | item crates/tally::counter::impl Tally impl_item | predated
not locked | implements | crates/tally/src/counter.rs | unanchored not_an_item | unanchored
not locked | implements | crates/tally/src/extra/moved.rs | item crates/tally::extra::moved::shift function_item | spec_ahead
new | verifies | crates/tally/tests/bounds.rs | item crates/tally::test:bounds::stops_at_cap function_item
new | configures | crates/tally/src/counter.rs | item crates/tally::counter::CAP const_item
new | configures | data/limits.ron | ron root.cap
",
        ),
        (
            "GOAL-01",
            "lock GOAL-01 rev 1 (accept: 2 new, 0 refreshed, 0 kept, 0 dropped)",
            "not locked | implements | crates/tally/src/broken.rs | cannot_verify other | cannot_verify
not locked | implements | crates/tally/src/counter.rs | item crates/tally::counter::Tally::reset function_item | bad_rev
new | implements | crates/tally/src/counter.rs | item crates/tally::counter::Tally::value function_item
not locked | implements | crates/tally/src/counter.rs | unanchored in_body | unanchored
not locked | implements | crates/tally/src/counter.rs | unanchored in_macro | unanchored
not locked | implements | crates/tally/src/counter.rs | unanchored not_an_item | bad_rev
not locked | implements | crates/tally/src/counter.rs | unanchored not_an_item | unanchored
not locked | implements | crates/tally/src/lib.rs | unanchored no_item | unanchored
not locked | implements | crates/tally/src/lib.rs | unanchored not_an_item | unanchored
not locked | implements | tools/loose.rs | module . | ambiguous unrooted
new | verifies | crates/tally/src/counter.rs | item crates/tally::counter::inner::limit function_item
not locked | configures | data/limits.ron | unanchored no_value | unanchored
",
        ),
        (
            "LAW-STEP",
            "lock LAW-STEP rev 1 (accept: 3 new, 0 refreshed, 0 kept, 0 dropped)",
            "new | implements | crates/tally/src/broken.rs | item crates/tally::broken::intact function_item
new | implements | crates/tally/src/counter.rs | item crates/tally::counter::Tally::step function_item
not locked | implements | crates/tally/src/counter.rs | module crates/tally::counter::inner | predated
not locked | implements | crates/tally/src/counter.rs | unanchored not_an_item | unanchored
new | implements | crates/tally/src/lib.rs | item crates/tally::moved mod_item
not locked | configures | data/limits.ron | ron root.step | ambiguous duplicate
",
        ),
        (
            "PREM-001",
            "lock PREM-001 rev 1 (accept: 1 new, 0 refreshed, 0 kept, 0 dropped)",
            "new | implements | crates/tally/src/main.rs | item crates/tally::bin:tally-kit::main function_item
not locked | implements | tools/loose.rs | item .::seed function_item | ambiguous unrooted
not locked | assumes | crates/tally/src/counter.rs | unanchored no_item | unanchored
",
        ),
    ];
    let repo = LockRepo::code_a("lock-rows");
    let mut failures = Vec::new();
    for (reference, question, rows) in cases {
        let head = repo.head();
        let attempt = repo.accept(reference, false);
        let refusal = attempt.refused(reference);
        assert!(refusal.contains("not confirmed"), "{reference}: {refusal}");
        if attempt.rows() != rows {
            failures.push(format!(
                "{reference} rows:\n--- expected\n{rows}--- got\n{}",
                attempt.rows()
            ));
        }
        let expected_question = format!("{question}{}", at(&head, "main", &repo.root));
        if attempt.question() != expected_question {
            failures.push(format!(
                "{reference} question:\n  expected {expected_question}\n  got      {}",
                attempt.question()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(repo.lock().is_none(), "declined: no lock written");
}

/// RD-7 (M: `cannot_verify` locked): GOAL-01's marker on the broken item
/// is never an entry; an entry locked while its item was intact is kept,
/// byte for byte, once the item breaks (`kept | … | cannot_verify …`),
/// never refreshed.
#[test]
fn cannot_verify_binding_is_never_locked() {
    let repo = LockRepo::code_a("lock-cannot-verify");
    repo.accept_ok("GOAL-01");
    let lock = repo.lock_text();
    assert!(
        !lock.contains(BROKEN),
        "the broken item's marker is locked:\n{lock}"
    );
    // LAW-STEP's `intact` in broken.rs is locked while intact, then broken.
    repo.accept_ok("LAW-STEP");
    let before = repo.lock_text();
    let intact: Vec<String> = node_blocks(&before, "LAW-STEP")
        .into_iter()
        .filter(|block| block.contains(BROKEN))
        .collect();
    assert_eq!(intact.len(), 1, "{before}");
    repo.replace(
        BROKEN,
        "pub fn intact() -> u32 {\n    7\n}",
        "pub fn intact() -> u32 {\n    7 7\n}",
    );
    repo.commit_all("break intact");
    let attempt = repo.accept("LAW-STEP", true);
    let rows = attempt.rows();
    assert!(
        rows.contains(
            "kept | implements | crates/tally/src/broken.rs | item crates/tally::broken::intact function_item | cannot_verify"
        ),
        "{rows}"
    );
    // Nothing else changed: no commit, the entry as it was.
    let outcome = attempt
        .outcome
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(outcome.commit.is_none() || outcome.unchanged, "{outcome:?}");
    let after = repo.lock_text();
    let still: Vec<String> = node_blocks(&after, "LAW-STEP")
        .into_iter()
        .filter(|block| block.contains(BROKEN))
        .collect();
    assert_eq!(still, intact, "the old entry kept byte for byte");
}

/// RD-7 (M: `spec_ahead` locked): `//! @implements PIECE-COUNTER@2` in
/// counter.rs, behind revision 3, is never an entry.
#[test]
fn marker_behind_the_revision_is_not_locked() {
    let repo = LockRepo::code_a("lock-behind");
    let outcome = repo.accept_ok("PIECE-COUNTER");
    assert!(
        outcome.rows.iter().any(|row| row.change == "not locked"
            && row.kind == "module"
            && row.symbol == "crates/tally::counter"
            && row.state.as_deref() == Some("spec_ahead")),
        "{:?}",
        outcome.rows
    );
    let lock = repo.lock_text();
    assert!(
        !lock.contains("symbol = \"crates/tally::counter\"\n"),
        "the module behind the revision is locked:\n{lock}"
    );
    // LAW-BOUND@1 in moved.rs, behind rev 2: not locked either.
    repo.accept_ok("LAW-BOUND");
    assert!(!repo.lock_text().contains("crates/tally/src/extra/moved.rs"));
}

/// RD-7 (M: an ambiguous qpath locked): an `unrooted` item and module,
/// a `path_attribute` item, a `duplicate` RON path are never entries.
#[test]
fn ambiguous_binding_is_not_locked() {
    let repo = LockRepo::code_a("lock-ambiguous");
    for reference in ["PREM-001", "GOAL-01", "LAW-STEP"] {
        repo.accept_ok(reference);
    }
    let lock = repo.lock_text();
    for absent in ["tools/loose.rs", "symbol = \"root.step\""] {
        assert!(!lock.contains(absent), "{absent} is locked:\n{lock}");
    }
    let attempt = repo.accept("PREM-001", true);
    attempt.unchanged("a second PREM-001");
    assert!(
        attempt
            .rows()
            .contains("not locked | implements | tools/loose.rs | item .::seed function_item | ambiguous unrooted"),
        "{}",
        attempt.rows()
    );
}

/// RD-7 (M: a RON `ast_hash`): a RON value's entry has no `item_kind` and
/// no `ast_hash`; its `file_blob` is the file's blob at HEAD; a change
/// anywhere in the file refreshes it (by `file_blob` alone), a change
/// elsewhere leaves it unchanged.
#[test]
fn ron_binding_is_locked_by_blob_only() {
    let repo = LockRepo::code_a("lock-ron");
    repo.accept_ok("PIECE-COUNTER");
    let lock = repo.lock_text();
    let ron: Vec<_> = blocks(&lock)
        .into_iter()
        .filter(|block| value(block, "kind") == Some("ron"))
        .collect();
    assert_eq!(ron.len(), 1, "{lock}");
    let keys: Vec<&str> = ron[0].iter().map(|(key, _)| key.as_str()).collect();
    let expected: Vec<&str> = lock_common::ENTRY_KEYS
        .iter()
        .copied()
        .filter(|key| *key != "item_kind" && *key != "ast_hash")
        .collect();
    assert_eq!(keys, expected);
    let head = repo.head();
    assert_eq!(
        value(&ron[0], "file_blob"),
        Some(repo.blob_at(&head, LIMITS).as_str())
    );
    // A value the marker does not bind changes: the entry is refreshed.
    repo.replace(LIMITS, "step: 2,", "step: 3,");
    repo.commit_all("another step");
    let attempt = repo.accept("PIECE-COUNTER", true);
    attempt.committed("refresh");
    assert!(
        attempt
            .rows()
            .contains("refreshed | configures | data/limits.ron | ron root"),
        "{}",
        attempt.rows()
    );
    let head = repo.head();
    let refreshed = blocks(&repo.lock_text())
        .into_iter()
        .find(|block| value(block, "kind") == Some("ron"))
        .expect("the RON entry");
    assert_eq!(
        value(&refreshed, "file_blob"),
        Some(repo.blob_at(&head, LIMITS).as_str())
    );
    assert!(value(&refreshed, "ast_hash").is_none());
}

/// TE-2: `id_state` `ambiguous` (two holders of LAW-SOURCE) and
/// `homoglyph` (a Cyrillic look-alike of LAW-DEFAULT) are never locked,
/// beside a `resolved` marker of the same node that is.
#[test]
fn ambiguous_and_look_alike_ids_are_not_locked() {
    let repo = LockRepo::code_a("lock-id-states");
    repo.append(
        MAIN,
        "\n// @implements LAW-DEFAULT\nfn default_cap() -> u32 {\n    10\n}\n\n\
         // @implements L\u{0410}W-DEFAULT@1\nfn look_alike() -> u32 {\n    10\n}\n\n\
         // @implements LAW-SOURCE\nfn source() -> u32 {\n    10\n}\n\n\
         // @implements PIECE-LIMITS#LAW-SOURCE\nfn sourced() -> u32 {\n    10\n}\n",
    );
    repo.write(
        "docs/spec/copy.md",
        "---\nclass: canon\nid: PIECE-COPY\nowner: owner\nreviewed: 2026-10-10\n---\n\n# Copy\n\n## Source again {#LAW-SOURCE}\n\nA second holder of the same ID.\n",
    );
    repo.commit_all("look-alike and ambiguous markers");
    let attempt = repo.accept("LAW-DEFAULT", true);
    attempt.committed("LAW-DEFAULT");
    assert_eq!(
        attempt.rows(),
        "new | implements | crates/tally/src/main.rs | item crates/tally::bin:tally-kit::default_cap function_item\n\
not locked | implements | crates/tally/src/main.rs | item crates/tally::bin:tally-kit::look_alike function_item | homoglyph\n"
    );
    let attempt = repo.accept("PIECE-LIMITS#LAW-SOURCE", true);
    attempt.committed("PIECE-LIMITS#LAW-SOURCE");
    assert_eq!(
        attempt.rows(),
        "not locked | implements | crates/tally/src/main.rs | item crates/tally::bin:tally-kit::source function_item | ambiguous\n\
new | implements | crates/tally/src/main.rs | item crates/tally::bin:tally-kit::sourced function_item\n"
    );
    let lock = repo.lock_text();
    for absent in ["::look_alike", "::source\""] {
        assert!(!lock.contains(absent), "{absent} locked:\n{lock}");
    }
    assert!(lock.contains("::sourced\""), "{lock}");
}

/// RD-2, RD-3 (code review iteration 1, minor): a RON file whose name
/// holds a TAB (a legal git path) bound to LAW-DEFAULT: whatever the
/// accept makes of it, the lock it leaves reads back, so the next accept
/// of another node is not exit 2 on `spec.lock`.
#[test]
fn a_path_with_a_tab_never_makes_the_lock_unreadable() {
    let repo = LockRepo::code_a("lock-tab-path");
    repo.write(
        "data/tab\there.ron",
        "// @configures LAW-DEFAULT\nLimits(\n    cap: 10,\n)\n",
    );
    repo.commit_all("a RON file with a TAB in its name");
    let first = repo.accept("LAW-DEFAULT", true);
    assert!(first.outcome.is_ok(), "{:?}", first.outcome);
    // Iteration 2 (M: the writer skips the key check): the binding is
    // `not locked`, its problem named, and never written.
    let rows = first.rows();
    assert!(
        rows.lines()
            .any(|row| row.starts_with("not locked | configures | data/tab")
                && row.contains("| ron root | ")),
        "{rows}"
    );
    assert!(
        !String::from_utf8_lossy(&repo.lock().unwrap_or_default()).contains("tab"),
        "the TAB path is written"
    );
    let second = repo.accept("PIECE-COUNTER", true);
    if let Err(error) = &second.outcome {
        panic!(
            "after LAW-DEFAULT's accept ({:?}), the lock no longer reads: {error}\n{}",
            first.outcome.as_ref().map(|outcome| &outcome.rows),
            String::from_utf8_lossy(&repo.lock().unwrap_or_default())
        );
    }
}

// ----------------------------------------------------------------- RD-5

/// RD-5 (M: worktree bytes): with `*.rs ident` and `"$Id$"` in a bound
/// `fn`, the worktree holds `$Id: <oid> $` and HEAD `$Id$`: the lock's
/// `ast_hash` is the committed blob's item, its `file_blob` HEAD's.
#[test]
fn lock_hashes_the_committed_blob() {
    let repo = LockRepo::code_a("lock-ident");
    repo.write(".gitattributes", "*.rs ident\n");
    repo.append(
        MAIN,
        "\n// @implements LAW-DEFAULT\nfn stamp() -> &'static str {\n    \"$Id$\"\n}\n",
    );
    repo.commit_all("an ident stamp");
    fs::remove_file(repo.root.join(MAIN)).expect("remove");
    repo.git(&["checkout", "--", MAIN]);
    let worktree = fs::read(repo.root.join(MAIN)).expect("main.rs");
    assert!(
        String::from_utf8_lossy(&worktree).contains("$Id: "),
        "ident expanded in the worktree"
    );
    assert!(repo.porcelain().is_empty(), "the tree is clean");
    let head = repo.head();
    let committed = repo.show_blob(&head, MAIN);
    assert!(String::from_utf8_lossy(&committed).contains("\"$Id$\""));
    repo.accept_ok("LAW-DEFAULT");
    let lock = repo.lock_text();
    let entry = blocks(&lock)
        .into_iter()
        .find(|block| value(block, "node") == Some("LAW-DEFAULT"))
        .expect("LAW-DEFAULT's entry");
    let line = String::from_utf8_lossy(&committed)
        .lines()
        .position(|line| line.starts_with("fn stamp()"))
        .expect("stamp")
        + 1;
    let from_head = item_hash(&committed, line, "function_item");
    let from_worktree = item_hash(&worktree, line, "function_item");
    assert_ne!(
        from_head, from_worktree,
        "the expansion changes the item's hash"
    );
    assert_eq!(value(&entry, "ast_hash"), Some(from_head.as_str()));
    assert_eq!(
        value(&entry, "file_blob"),
        Some(repo.blob_at(&head, MAIN).as_str())
    );
}

// ----------------------------------------------------------------- RD-8

/// RD-8 (M: a new `spec_hash` at R): PIECE-COUNTER's text changed and
/// committed without a revision raise: exit 1 naming `--editorial`, no
/// question, the lock and HEAD as they were.
#[test]
fn meaning_change_without_a_revision_is_refused() {
    let repo = LockRepo::code_a("lock-meaning");
    repo.accept_ok("PIECE-COUNTER");
    repo.replace(
        "docs/spec/counter.md",
        "A count that grows one step at a time",
        "A count that grows two steps at a time",
    );
    repo.commit_all("a new meaning, no raise");
    let before = repo.state();
    let attempt = repo.accept("PIECE-COUNTER", true);
    let refusal = attempt.refused("meaning changed at rev 3");
    assert!(attempt.prompts.is_empty(), "asked: {:?}", attempt.prompts);
    for part in ["PIECE-COUNTER", "rev 3", "--editorial", "nothing written"] {
        assert!(refusal.contains(part), "{part:?} not in {refusal}");
    }
    assert_eq!(repo.state(), before);
}

/// RD-8 (M: others re-rendered): GOAL-01's accept after counter.rs and
/// LAW-BOUND's text changed leaves every LAW-BOUND and PIECE-COUNTER block
/// byte for byte (their old `file_blob`, `commit`, `spec_hash`).
#[test]
fn other_nodes_entries_are_untouched() {
    let repo = LockRepo::code_a("lock-others");
    for reference in ["PIECE-COUNTER", "LAW-BOUND", "GOAL-01"] {
        repo.accept_ok(reference);
    }
    let before = repo.lock_text();
    repo.replace(
        COUNTER,
        "        self.count\n",
        "        self.count.min(CAP)\n",
    );
    repo.replace(
        "docs/spec/counter.md",
        "The count never exceeds the cap.",
        "The count never exceeds the cap, whatever the step.",
    );
    repo.commit_all("value and bound changed");
    let attempt = repo.accept("GOAL-01", true);
    attempt.committed("GOAL-01 again");
    assert!(
        attempt.rows().contains(
            "refreshed | implements | crates/tally/src/counter.rs | item crates/tally::counter::Tally::value function_item"
        ),
        "{}",
        attempt.rows()
    );
    let after = repo.lock_text();
    for node in ["LAW-BOUND", "PIECE-COUNTER"] {
        assert_eq!(
            node_blocks(&after, node),
            node_blocks(&before, node),
            "{node}"
        );
    }
    assert_ne!(
        node_blocks(&after, "GOAL-01"),
        node_blocks(&before, "GOAL-01")
    );
}

// ----------------------------------------------------------------- RD-9

/// RD-9 (M: re-stamped): a second accept, also after an unrelated commit
/// and on a later day: exit 0 `nothing to change`, no question, HEAD
/// unmoved, the lock byte for byte (`commit`, `verified_at` the first's).
#[test]
fn second_accept_changes_nothing() {
    let repo = LockRepo::code_a("lock-second");
    repo.accept_ok("PIECE-COUNTER");
    let lock = repo.lock();
    let head = repo.head();
    let attempt = repo.accept("PIECE-COUNTER", true);
    attempt.unchanged("second");
    assert!(
        attempt
            .rows()
            .starts_with("unchanged | implements | crates/tally/src/counter.rs | item crates/tally::counter::Tally struct_item\n"),
        "{}",
        attempt.rows()
    );
    assert_eq!(repo.head(), head);
    assert_eq!(repo.lock(), lock);
    // HEAD moves (a file no binding reads), the day changes.
    repo.write("NOTES.txt", "unrelated\n");
    repo.commit_all("unrelated");
    let head = repo.head();
    let attempt = repo.attempt_at(&repo.root, "PIECE-COUNTER", false, lock_common::LATER, true);
    attempt.unchanged("after an unrelated commit, a day later");
    assert_eq!(repo.head(), head);
    assert_eq!(repo.lock(), lock);
    // The binary says so on stdout, exit 0, no prompt.
    let run = repo.spec_pty(&repo.root, &["lock", "accept", "PIECE-COUNTER"], None);
    run.code(0);
    assert!(!run.asked, "{}", run.show());
    assert!(
        run.stdout.ends_with("nothing to change\n"),
        "{}",
        run.show()
    );
    assert_eq!(repo.head(), head);
}

// ---------------------------------------------------------------- RD-17

/// RD-17 (M: exit 1 "names nothing"): LAW-STEP's section deleted and
/// committed: `accept LAW-STEP` drops exactly its entries after `y`, one
/// commit of the lock alone, every other block byte for byte.
#[test]
fn gone_node_entries_are_dropped() {
    let repo = LockRepo::code_a("lock-gone");
    for reference in ["LAW-STEP", "PIECE-COUNTER"] {
        repo.accept_ok(reference);
    }
    let before = repo.lock_text();
    let step_blocks = node_blocks(&before, "LAW-STEP").len();
    assert_eq!(step_blocks, 3, "{before}");
    let text = repo.read("docs/spec/counter.md");
    let start = text.find("## Step {#LAW-STEP}").expect("the section");
    let end = text.find("## Odd {#LAW-ODD").expect("the next one");
    repo.write(
        "docs/spec/counter.md",
        format!("{}{}", &text[..start], &text[end..]),
    );
    repo.commit_all("LAW-STEP deleted");
    let head = repo.head();
    let attempt = repo.accept("LAW-STEP", true);
    let outcome = attempt.committed("gone LAW-STEP");
    assert_eq!(
        attempt.question(),
        format!(
            "lock LAW-STEP (gone: 3 dropped){}",
            at(&head, "main", &repo.root)
        )
    );
    assert_eq!(
        attempt.rows(),
        "dropped | implements | crates/tally/src/broken.rs | item crates/tally::broken::intact function_item\n\
dropped | implements | crates/tally/src/counter.rs | item crates/tally::counter::Tally::step function_item\n\
dropped | implements | crates/tally/src/lib.rs | item crates/tally::moved mod_item\n"
    );
    let commit = outcome.commit.clone().expect("a commit");
    assert_eq!(repo.parents(&commit), vec![head]);
    assert_eq!(repo.changed(&commit), vec!["spec.lock"]);
    assert_eq!(
        repo.message(&commit).trim_end(),
        format!("spec: lock LAW-STEP\n\nLock: LAW-STEP\nLock-mode: accept\nDecided-by: {DECIDER}")
    );
    let after = repo.lock_text();
    assert!(node_blocks(&after, "LAW-STEP").is_empty(), "{after}");
    assert_eq!(
        node_blocks(&after, "PIECE-COUNTER"),
        node_blocks(&before, "PIECE-COUNTER")
    );
    // Nothing left of it: REF names nothing now.
    let again = repo.accept("LAW-STEP", true);
    again.refused("gone twice");
    assert!(again.prompts.is_empty());
}

/// RD-17 (M: the alias resolved first): GOAL-01 renamed GOAL-02 with
/// `aliases: [GOAL-01]`: `accept GOAL-01` drops GOAL-01's entries and
/// locks nothing of GOAL-02.
#[test]
fn alias_of_a_gone_id_drops_its_entries() {
    let repo = LockRepo::code_a("lock-gone-alias");
    for reference in ["GOAL-01", "PREM-001"] {
        repo.accept_ok(reference);
    }
    let before = repo.lock_text();
    assert_eq!(node_blocks(&before, "GOAL-01").len(), 2, "{before}");
    repo.replace(
        "docs/records/GOAL-01.md",
        "id: GOAL-01\n",
        "id: GOAL-02\naliases: [GOAL-01]\n",
    );
    repo.commit_all("GOAL-01 renamed GOAL-02");
    let shown = repo.spec_piped(&repo.root, &["show", "GOAL-01"]);
    shown.code(0);
    assert!(shown.stdout.starts_with("GOAL-02 | "), "{}", shown.show());
    let attempt = repo.accept("GOAL-01", true);
    attempt.committed("gone alias");
    assert!(
        attempt
            .question()
            .starts_with("lock GOAL-01 (gone: 2 dropped) at "),
        "{}",
        attempt.question()
    );
    let after = repo.lock_text();
    assert!(node_blocks(&after, "GOAL-01").is_empty(), "{after}");
    assert!(node_blocks(&after, "GOAL-02").is_empty(), "{after}");
    assert_eq!(
        node_blocks(&after, "PREM-001"),
        node_blocks(&before, "PREM-001")
    );
}

/// Iteration 2 (review nit; M: `@rev` kept): `REF` with a revision that
/// the read ignores (`LAW-STEP@1`) still finds a gone node's entries: the
/// drop proceeds.
#[test]
fn a_gone_id_with_a_revision_drops_its_entries() {
    let repo = LockRepo::code_a("lock-gone-rev");
    repo.accept_ok("LAW-STEP");
    let text = repo.read("docs/spec/counter.md");
    let start = text.find("## Step {#LAW-STEP}").expect("the section");
    let end = text.find("## Odd {#LAW-ODD").expect("the next one");
    repo.write(
        "docs/spec/counter.md",
        format!("{}{}", &text[..start], &text[end..]),
    );
    repo.commit_all("LAW-STEP deleted");
    let attempt = repo.accept("LAW-STEP@1", true);
    attempt.committed("gone with @1");
    assert!(
        attempt
            .question()
            .starts_with("lock LAW-STEP (gone: 3 dropped) at "),
        "{}",
        attempt.question()
    );
    assert!(!repo.lock_text().contains("node = \"LAW-STEP\""));
}

/// Iteration 2 (`blob_oid` reads `:(literal)<path>`; M: a bare pathspec):
/// a `[code]` root whose name starts with `:` is an input git finds at
/// HEAD (not "not tracked"); its unrooted binding is `not locked`, the
/// rest commits.
#[test]
fn a_code_path_starting_with_a_colon_is_read_literally() {
    let repo = LockRepo::code_a("lock-colon");
    repo.replace(
        "specengine.toml",
        "roots   = [\"crates\", \"data\", \"tools/loose.rs\"]",
        "roots   = [\"crates\", \"data\", \"tools/loose.rs\", \":stamp.rs\"]",
    );
    repo.write(
        ":stamp.rs",
        "// @implements PIECE-COUNTER\npub fn stamp() {}\n",
    );
    repo.commit_all("a root named with a colon");
    let attempt = repo.accept("PIECE-COUNTER", true);
    attempt.committed("a colon path");
    assert!(
        attempt
            .rows()
            .lines()
            .any(|row| row.starts_with("not locked | implements | :stamp.rs | ")),
        "{}",
        attempt.rows()
    );
}

// ------------------------------------------------------------------ TE-4

/// TE-4 (M: a path in a hash input): one commit accepted from two roots,
/// from another cwd, in a copy made in reverse creation order, with
/// `core.autocrlf=true`, and through the alias REF (`AIM-01` for
/// GOAL-01): identical bytes.
#[test]
fn one_commit_gives_one_lock() {
    let run = |repo: &LockRepo, cwd: &Path, goal: &str| {
        for reference in ["PIECE-COUNTER", "LAW-BOUND", "cap-check/PASS-01", goal] {
            repo.attempt_at(cwd, reference, false, CLOCK, true)
                .committed(reference);
        }
        repo.lock().expect("a lock")
    };
    let first = LockRepo::code_a("lock-det-a");
    let reference = run(&first, &first.root, "GOAL-01");
    let commit = first.git_text(&["rev-parse", "HEAD~4"]);

    let second = LockRepo::of("lock-det-b", "code-a", "elsewhere/deeper", &[]);
    assert_eq!(
        second.git_text(&["rev-parse", "HEAD"]),
        commit,
        "the same commit"
    );
    let sub = second.root.join("crates").join("tally");
    assert_eq!(
        run(&second, &sub, "AIM-01"),
        reference,
        "another root, cwd, alias REF"
    );

    // A copy made in reverse creation order.
    let third = LockRepo::uncommitted("lock-det-c");
    fs::remove_dir_all(&third.root).expect("clear");
    let mut files: Vec<_> = common::snapshot(&fixture("code-a"))
        .into_iter()
        .filter(|(path, bytes)| bytes.is_some() && path != "expected.json")
        .collect();
    files.reverse();
    for (path, bytes) in files {
        third.write(&path, bytes.expect("a file"));
    }
    third.git.init(&third.root);
    third.commit_all("the fixture");
    assert_eq!(third.head(), commit, "the same commit");
    assert_eq!(
        run(&third, &third.root, "GOAL-01"),
        reference,
        "reverse creation order"
    );

    // core.autocrlf=true: CRLF in the worktree, LF committed.
    let fourth = LockRepo::code_a("lock-det-d");
    fourth.git(&["config", "core.autocrlf", "true"]);
    for path in [COUNTER, LIB, BOUNDS, LIMITS, "docs/spec/counter.md"] {
        fs::remove_file(fourth.root.join(path)).expect("remove");
        fourth.git(&["checkout", "--", path]);
    }
    assert!(
        fs::read(fourth.root.join(COUNTER))
            .expect("counter")
            .windows(2)
            .any(|w| w == b"\r\n"),
        "CRLF in the worktree"
    );
    assert!(fourth.porcelain().is_empty(), "clean under autocrlf");
    assert_eq!(
        run(&fourth, &fourth.root, "GOAL-01"),
        reference,
        "core.autocrlf=true"
    );
}

/// TE-4: a SHA-256 repository gives 64-hex `file_blob` and `commit`, the
/// same `b3:` hashes and everything else as a SHA-1 one.
#[test]
fn a_sha256_repository_gives_the_same_hashes() {
    let sha1 = LockRepo::code_a("lock-sha1");
    sha1.accept_ok("PIECE-COUNTER");
    let sha256 = LockRepo::of("lock-sha256", "code-a", "repo", &["--object-format=sha256"]);
    sha256.accept_ok("PIECE-COUNTER");
    let (one, two) = (sha1.lock_text(), sha256.lock_text());
    let strip = |text: &str| {
        text.lines()
            .filter(|line| !line.starts_with("file_blob = ") && !line.starts_with("commit = "))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(strip(&one), strip(&two));
    for block in blocks(&two) {
        for key in ["file_blob", "commit"] {
            let oid = value(&block, key).expect(key);
            assert_eq!(oid.len(), 64, "{key} {oid}");
        }
        let file = value(&block, "file").expect("file");
        let head = sha256.git_text(&["rev-parse", "HEAD~1"]);
        assert_eq!(
            value(&block, "file_blob"),
            Some(sha256.blob_at(&head, file).as_str())
        );
        assert_eq!(value(&block, "commit"), Some(head.as_str()));
    }
}

// ------------------------------------------------------------------ TE-5

/// TE-5 (M: a line): the lock holds no absolute path, name, e-mail, time
/// of day, abbreviated OID, line or column: every block has exactly the
/// keys of "Entry", `file` a clean relative path, every OID 40 or 64 hex,
/// `verified_at` a bare date.
#[test]
fn lock_has_no_positions() {
    let repo = LockRepo::code_a("lock-positions");
    accept_golden(&repo);
    let lock = repo.lock_text();
    let scratch = repo.scratch.path().display().to_string();
    // Absolute home and temp roots, assembled at run time so this file does
    // not contain the literal prefixes the anonymity test forbids.
    let roots: Vec<String> = ["Users", "home", "Volumes", "private/tmp", "var/folders"]
        .iter()
        .map(|root| format!("/{root}/"))
        .collect();
    for root in &roots {
        assert!(
            !lock.contains(root.as_str()),
            "{root:?} in the lock:\n{lock}"
        );
    }
    for forbidden in [
        scratch.as_str(),
        "/tmp/",
        "Scratch",
        "example.invalid",
        "T09:30",
        ":30:00",
    ] {
        assert!(
            !lock.contains(forbidden),
            "{forbidden:?} in the lock:\n{lock}"
        );
    }
    for block in blocks(&lock) {
        let keys: Vec<&str> = block.iter().map(|(key, _)| key.as_str()).collect();
        let item = value(&block, "kind") == Some("item");
        let expected: Vec<&str> = lock_common::ENTRY_KEYS
            .iter()
            .copied()
            .filter(|key| item || (*key != "item_kind" && *key != "ast_hash"))
            .collect();
        assert_eq!(keys, expected, "{block:?}");
        let file = value(&block, "file").expect("file");
        assert!(
            !file.starts_with('/')
                && !file.contains(':')
                && !file.contains('\\')
                && !file
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == ".."),
            "file {file:?}"
        );
        let symbol = value(&block, "symbol").expect("symbol");
        assert!(
            !symbol
                .rsplit(':')
                .next()
                .is_some_and(|tail| !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit())),
            "a position in symbol {symbol:?}"
        );
        for key in ["file_blob", "commit"] {
            let oid = value(&block, key).expect(key);
            assert!(
                (oid.len() == 40 || oid.len() == 64)
                    && oid
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "{key} {oid}"
            );
        }
        let date = value(&block, "verified_at").expect("verified_at");
        assert_eq!(date, DAY);
        assert_eq!(value(&block, "verified_by"), Some("owner"));
    }
}

/// The golden file is the repository's: not under a walked `[paths]` root
/// of SpecEngine's own config (`fixtures/**` excluded), named apart from
/// `spec.lock` so no tool reads it as one.
#[test]
fn the_golden_lives_in_fixtures() {
    let golden = golden_file();
    assert!(golden.starts_with(repository_root().join("fixtures")));
    assert_ne!(
        golden.file_name().and_then(|name| name.to_str()),
        Some("spec.lock")
    );
}
