//! docs/features/spec-lock.md TE-1, TE-2, TE-4, TE-5 and the named RD
//! tests of the accept itself (RD-5, RD-7, RD-8, RD-9, RD-17), and
//! docs/features/lock-levels.md RD-3 (with its M5) and TE-2: `spec lock
//! accept` over scratch repositories of `fixtures/code-a`, through the
//! library at an injected clock (the lock's bytes reproducible), the
//! binary on a pseudo-terminal where the printed text is the subject.
//!
//! The golden (`fixtures/spec-lock/code-a.lock`, format 2) is the lock
//! after accepting PIECE-COUNTER (a document, `rev: 3`; a `//!` marker; a
//! RON `@configures`), LAW-BOUND (a section, `rev=2`), cap-check/PASS-01
//! (a feature-scoped AC under `@verifies`, its only marker `[Sig]`:
//! `bad_levels`, nothing lockable, exit 1, no commit) and GOAL-01 (an
//! alias marker, `AIM-01@1 [body]`; its `[path, path]` binding
//! `bad_levels`), in that order, at [`CLOCK`]: 9 entries. Its keys and
//! order are derived by hand from lock-levels.md "Format 2" and
//! `fixtures/code-a/expected.json`; its digests and OIDs are recomputed
//! here from their sources (git's blob and commit OIDs, `spec show`'s norm
//! hash, `specengine-code`'s `item_levels`, `module_levels`,
//! `ron::value_hashes` and the empty set's `deps`), and the file pins
//! their values. The format-1 golden (`code-a.format-1.lock`) is the
//! bytes before 5b-1a, verbatim.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;
mod lock_common;
mod tested_common;
mod verify_common;

use std::fs;
use std::path::Path;

use common::{fixture, repository_root};
use lock_common::{
    CLOCK, DAY, DECIDER, HEADER, LockRepo, PREAMBLE, at, block, blocks, entry_keys, item_code,
    lock_of, module_code, node_blocks, ron_code, value,
};

const COUNTER: &str = "crates/tally/src/counter.rs";
const LIB: &str = "crates/tally/src/lib.rs";
const BOUNDS: &str = "crates/tally/tests/bounds.rs";
const MAIN: &str = "crates/tally/src/main.rs";
const BROKEN: &str = "crates/tally/src/broken.rs";
const LIMITS: &str = "data/limits.ron";

/// The four accepts of the golden, in order; the third
/// (`cap-check/PASS-01`) locks nothing.
const GOLDEN_REFS: [&str; 4] = ["PIECE-COUNTER", "LAW-BOUND", "cap-check/PASS-01", "GOAL-01"];

/// `cap-check/PASS-01`'s only binding, refused (`[Sig]`: `unknown`).
const PASS_ROW: &str = "not locked | verifies | crates/tally/tests/bounds.rs | item crates/tally::test:bounds::steps_by_one function_item | bad_levels\n";

/// The default levels of a marker without a list.
const DEFAULT: [&str; 2] = ["sig", "body"];

fn golden_file() -> std::path::PathBuf {
    fixture("spec-lock").join("code-a.lock")
}

/// The four accepts of the golden in `repo`: the commit each ran at.
/// `cap-check/PASS-01`'s exits 1, nothing lockable, no question, no
/// commit.
fn accept_golden(repo: &LockRepo) -> [String; 4] {
    let mut commits = Vec::new();
    for reference in GOLDEN_REFS {
        commits.push(repo.head());
        if reference == "cap-check/PASS-01" {
            let attempt = repo.accept(reference, true);
            let refusal = attempt.refused(reference);
            assert!(refusal.contains("can be locked"), "{refusal}");
            assert!(attempt.prompts.is_empty(), "{reference} asked");
            assert_eq!(attempt.rows(), PASS_ROW);
        } else {
            repo.accept_ok(reference);
        }
    }
    commits.try_into().expect("four commits")
}

/// The golden lock of `repo` after [`accept_golden`] (`commits` the
/// commits they ran at), every digest and OID recomputed from its source.
fn expected_golden(repo: &LockRepo, commits: &[String; 4]) -> String {
    let c0 = &commits[0];
    let blob = |path: &str| repo.blob_at(c0, path);
    let source = |path: &str| repo.show_blob(c0, path);
    let item = |path: &str, line: usize, kind: &str, levels: &[&str]| {
        item_code(&source(path), line, kind, levels)
    };
    let module = |path: &str| module_code(&source(path), &DEFAULT);
    let ron = |line: usize| ron_code(&source(LIMITS), line, &DEFAULT);
    let goal = repo.norm_hash("GOAL-01");
    let law = repo.norm_hash("LAW-BOUND");
    let piece = repo.norm_hash("PIECE-COUNTER");
    let (c_piece, c_law, c_goal) = (&commits[0], &commits[1], &commits[3]);
    assert_eq!(commits[2], commits[3], "PASS-01's accept commits nothing");
    lock_of(&[
        block(
            "GOAL-01",
            1,
            &goal,
            "implements",
            COUNTER,
            "item",
            "crates/tally::counter::Tally::value",
            Some("function_item"),
            &item(COUNTER, 30, "function_item", &["body"]),
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
            Some("function_item"),
            &item(COUNTER, 49, "function_item", &DEFAULT),
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
            Some("function_item"),
            &item(BOUNDS, 6, "function_item", &["body"]),
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
            Some("const_item"),
            &item(COUNTER, 8, "const_item", &DEFAULT),
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
            &ron(4),
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
            Some("struct_item"),
            &item(COUNTER, 17, "struct_item", &DEFAULT),
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
            &module(LIB),
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
            &module(BOUNDS),
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
            &ron(2),
            &blob(LIMITS),
            c_piece,
            DAY,
        ),
    ])
}

// ------------------------------------------------------------------ TE-1

/// TE-1, lock-levels.md RD-3: the lock after the golden's four accepts
/// is, byte for byte, the hand-derived format-2 golden with every digest
/// and OID recomputed from its source; and the committed golden file pins
/// those values: 9 entries, `PIECE-COUNTER`'s 4 at the fixture's commit
/// (`2ba8a3b…`, as in format 1), `LAW-BOUND`'s 4 and `GOAL-01`'s 1 at
/// commits other than format 1's (`82f6593…`, `85a2c98…`), `verified_at`
/// [`DAY`].
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
    // The pinned file is regenerated only from this accept, once it equals
    // the hand-derived golden (digests are never hand-written).
    if std::env::var_os("SPECENGINE_WRITE_LOCK_GOLDEN").is_some_and(|value| value == "1") {
        fs::write(golden_file(), &lock).expect("write fixtures/spec-lock/code-a.lock");
        return;
    }
    let golden = fs::read_to_string(golden_file()).unwrap_or_else(|error| {
        panic!(
            "fixtures/spec-lock/code-a.lock: {error}; regenerate it with \
             SPECENGINE_WRITE_LOCK_GOLDEN=1 cargo nextest run -p specengine-cli --test \
             lock_accept golden_lock_of_code_a"
        )
    });
    assert_eq!(
        lock, golden,
        "the pinned golden (regenerate with SPECENGINE_WRITE_LOCK_GOLDEN=1)"
    );
    assert!(lock.starts_with(&format!("{HEADER}\n{PREAMBLE}\n[[binding]]\n")));
    let entries = blocks(&lock);
    assert_eq!(entries.len(), 9);
    let count = |node: &str| node_blocks(&lock, node).len();
    assert_eq!(
        (count("PIECE-COUNTER"), count("LAW-BOUND"), count("GOAL-01")),
        (4, 4, 1)
    );
    assert_eq!(count("cap-check/PASS-01"), 0);
    let commit_of = |node: &str| -> Vec<String> {
        entries
            .iter()
            .filter(|block| value(block, "node") == Some(node))
            .map(|block| value(block, "commit").expect("commit").to_owned())
            .collect()
    };
    assert!(
        commit_of("PIECE-COUNTER")
            .iter()
            .all(|commit| commit.starts_with("2ba8a3b")),
        "{lock}"
    );
    for (node, format_1) in [("LAW-BOUND", "82f6593"), ("GOAL-01", "85a2c98")] {
        assert!(
            commit_of(node)
                .iter()
                .all(|commit| !commit.starts_with(format_1)),
            "{node}: {lock}"
        );
    }
    assert!(
        entries
            .iter()
            .all(|block| value(block, "verified_at") == Some(DAY))
    );
}

/// lock-levels.md "Fixtures and the golden" (Q30): the format-1 golden is
/// the bytes the golden held before 5b-1a, verbatim (its SHA-256 at the
/// copy pinned), format 1 of v2's `code_recipe`, 11 entries; the format-2
/// golden is not it.
#[test]
fn the_format_1_golden_is_kept_verbatim() {
    let format_1 = fs::read(fixture("spec-lock").join("code-a.format-1.lock"))
        .expect("fixtures/spec-lock/code-a.format-1.lock");
    let text = String::from_utf8(format_1).expect("UTF-8");
    assert!(
        text.starts_with(&format!(
            "{HEADER}\n{}\n[[binding]]\n",
            lock_common::PREAMBLE_FORMAT_1
        )),
        "{text}"
    );
    assert_eq!(blocks(&text).len(), 11);
    assert_eq!(
        blocks(&text)
            .iter()
            .filter(|block| value(block, "ast_hash").is_some())
            .count(),
        7,
        "an item's `ast_hash`"
    );
    let golden = fs::read_to_string(golden_file()).expect("the golden");
    assert_ne!(golden, text);
}

/// TE-1: what each accept of the golden asked, and the commits it made;
/// `cap-check/PASS-01`'s asked nothing and made none.
#[test]
fn golden_accepts_ask_and_commit_once_each() {
    let repo = LockRepo::code_a("lock-golden-ask");
    let expected = [
        (
            "PIECE-COUNTER",
            Some("lock PIECE-COUNTER rev 3 (accept: 4 new, 0 refreshed, 0 kept, 0 dropped)"),
        ),
        (
            "LAW-BOUND",
            Some("lock LAW-BOUND rev 2 (accept: 4 new, 0 refreshed, 0 kept, 0 dropped)"),
        ),
        ("cap-check/PASS-01", None),
        (
            "GOAL-01",
            Some("lock GOAL-01 rev 1 (accept: 1 new, 0 refreshed, 0 kept, 0 dropped)"),
        ),
    ];
    for (reference, head) in expected {
        let before = repo.head();
        let Some(head) = head else {
            let state = repo.state();
            let attempt = repo.accept(reference, true);
            attempt.refused(reference);
            assert!(attempt.prompts.is_empty(), "{reference}");
            assert_eq!(attempt.rows(), PASS_ROW);
            assert_eq!(repo.state(), state, "{reference}: nothing written");
            continue;
        };
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
/// and PREM-001 as a row, in key order: `item`, `module` with levels
/// hashed, `ron` with a value hash locked (`new`); `spec_ahead`,
/// `predated`, `bad_rev`, `bad_levels` (lock-levels.md "Accept": GOAL-01's
/// `[path, path]`), `ambiguous <reason>`, `unanchored`, `cannot_verify`
/// not (hand-derived
/// from `fixtures/code-a/expected.json` and "Lockability"). Since
/// docs/features/symbol-discriminator.md (TE-1) the two adjacent
/// `impl Tally` blocks are two keys, `#1` `predated` and `#2` `bad_rev`.
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
not locked | implements | crates/tally/src/counter.rs | item crates/tally::counter::impl Tally #1 impl_item | predated
not locked | implements | crates/tally/src/counter.rs | item crates/tally::counter::impl Tally #2 impl_item | bad_rev
not locked | implements | crates/tally/src/counter.rs | unanchored not_an_item | unanchored
not locked | implements | crates/tally/src/extra/moved.rs | item crates/tally::extra::moved::shift function_item | spec_ahead
new | verifies | crates/tally/tests/bounds.rs | item crates/tally::test:bounds::stops_at_cap function_item
new | configures | crates/tally/src/counter.rs | item crates/tally::counter::CAP const_item
new | configures | data/limits.ron | ron root.cap
",
        ),
        (
            "GOAL-01",
            "lock GOAL-01 rev 1 (accept: 1 new, 0 refreshed, 0 kept, 0 dropped)",
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
not locked | verifies | crates/tally/src/counter.rs | item crates/tally::counter::inner::limit function_item | bad_levels
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

/// RD-7, lock-levels.md "Format 2": a RON value's entry has no
/// `item_kind` and no fingerprint; its `sig_hash` and `body_hash` are the
/// value hash (`ron::value_hashes`, keyed by the comment's range), its
/// `deps_hash` the empty set's digest, its `file_blob` the file's blob at
/// HEAD; another value of the file changing refreshes it (the root's value
/// hash and the blob), a change elsewhere leaves it unchanged.
#[test]
fn ron_binding_is_locked_by_its_value_hash() {
    let repo = LockRepo::code_a("lock-ron");
    repo.accept_ok("PIECE-COUNTER");
    let lock = repo.lock_text();
    let ron: Vec<_> = blocks(&lock)
        .into_iter()
        .filter(|block| value(block, "kind") == Some("ron"))
        .collect();
    assert_eq!(ron.len(), 1, "{lock}");
    let keys: Vec<&str> = ron[0].iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(keys, entry_keys("ron"));
    let head = repo.head();
    assert_eq!(
        value(&ron[0], "file_blob"),
        Some(repo.blob_at(&head, LIMITS).as_str())
    );
    let code = |repo: &LockRepo| ron_code(&repo.show_blob(&repo.head(), LIMITS), 2, &DEFAULT);
    let want = code(&repo);
    assert_eq!(want.sig, want.body);
    assert_eq!(value(&ron[0], "sig_hash"), Some(want.sig.as_str()));
    assert_eq!(value(&ron[0], "body_hash"), Some(want.body.as_str()));
    assert_eq!(value(&ron[0], "deps_hash"), Some(want.deps.as_str()));
    assert_eq!(
        value(&ron[0], "levels"),
        Some("[\"sig\", \"body\"]"),
        "a marker without a list"
    );
    // A change elsewhere (a Rust file no RON entry names): unchanged.
    repo.replace(MAIN, "fn main()", "fn main( )");
    repo.commit_all("elsewhere");
    let attempt = repo.accept("PIECE-COUNTER", true);
    attempt.unchanged("a change elsewhere");
    assert!(
        attempt
            .rows()
            .contains("unchanged | configures | data/limits.ron | ron root"),
        "{}",
        attempt.rows()
    );
    // A value the root holds changes: the entry is refreshed.
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
    let now = ron_code(&repo.show_blob(&head, LIMITS), 2, &DEFAULT);
    assert_ne!(now.sig, want.sig, "the root's value changed");
    assert_eq!(value(&refreshed, "sig_hash"), Some(now.sig.as_str()));
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
/// level digests are the committed blob's item (`item_levels` over HEAD's
/// bytes), its `file_blob` HEAD's.
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
    let from_head = item_code(&committed, line, "function_item", &DEFAULT);
    let from_worktree = item_code(&worktree, line, "function_item", &DEFAULT);
    assert_ne!(
        from_head.body, from_worktree.body,
        "the expansion changes the item's body"
    );
    let block_text = node_blocks(&lock, "LAW-DEFAULT")
        .into_iter()
        .next()
        .expect("the block");
    assert!(block_text.contains(&from_head.lines()), "{block_text}");
    assert_eq!(value(&entry, "body_hash"), Some(from_head.body.as_str()));
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
    // One entry: GOAL-01's `[path, path]` binding is `bad_levels`
    // (docs/features/lock-levels.md "Fixtures and the golden").
    assert_eq!(node_blocks(&before, "GOAL-01").len(), 1, "{before}");
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
            .starts_with("lock GOAL-01 (gone: 1 dropped) at "),
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

/// What one golden run printed: per accept its rows (stdout) and its
/// question or refusal (stderr), the worktree's path made `<worktree>`;
/// then the lock's bytes.
type Printed = (Vec<(String, String)>, Vec<u8>);

/// The golden's accepts from `cwd` (GOAL-01 as `goal`), each at [`CLOCK`]:
/// what they printed and the lock.
fn golden_run(repo: &LockRepo, cwd: &Path, goal: &str) -> Printed {
    let worktree = repo.root.display().to_string();
    let mut printed = Vec::new();
    for reference in ["PIECE-COUNTER", "LAW-BOUND", "cap-check/PASS-01", goal] {
        let attempt = repo.attempt_at(cwd, reference, false, CLOCK, true);
        let said = if reference == "cap-check/PASS-01" {
            attempt.refused(reference)
        } else {
            attempt.committed(reference);
            attempt.question().to_owned()
        };
        printed.push((attempt.rows(), said.replace(&worktree, "<worktree>")));
    }
    (printed, repo.lock().expect("a lock"))
}

/// TE-4 (M: a path in a hash input), lock-levels.md TE-2: one commit
/// accepted from two roots, from another cwd, in a copy made in reverse
/// creation order, with `core.autocrlf=true`, and through the alias REF
/// (`AIM-01` for GOAL-01): identical lock bytes, rows (stdout) and
/// questions and refusals (stderr).
#[test]
fn one_commit_gives_one_lock() {
    let first = LockRepo::code_a("lock-det-a");
    let reference = golden_run(&first, &first.root, "GOAL-01");
    let commit = first.git_text(&["rev-parse", "HEAD~3"]);
    assert_eq!(
        reference.1,
        fs::read(golden_file()).expect("the golden"),
        "the golden"
    );

    let second = LockRepo::of("lock-det-b", "code-a", "elsewhere/deeper", &[]);
    assert_eq!(
        second.git_text(&["rev-parse", "HEAD"]),
        commit,
        "the same commit"
    );
    let sub = second.root.join("crates").join("tally");
    let other = golden_run(&second, &sub, "AIM-01");
    assert_eq!(
        other.1, reference.1,
        "another root, cwd, alias REF: the lock"
    );
    // The alias REF names itself in the question: compare the others.
    assert_eq!(other.0[..3], reference.0[..3], "another root, cwd: printed");
    assert_eq!(other.0[3].0, reference.0[3].0, "the alias REF's rows");

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
        golden_run(&third, &third.root, "GOAL-01"),
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
        golden_run(&fourth, &fourth.root, "GOAL-01"),
        reference,
        "core.autocrlf=true"
    );
}

/// lock-levels.md TE-2: an accept over a lock whose entries are written in
/// reverse key order (committed so) gives the bytes, rows and question an
/// accept over the canonical lock gives: the merge renders in key order,
/// whatever order it read. Each run's `HEAD` before GOAL-01's accept (the
/// one place the two histories differ) is made `<head>`.
#[test]
fn reversed_entries_give_the_same_lock() {
    let mut runs = Vec::new();
    for (label, reverse) in [("lock-rev-canonical", false), ("lock-rev-reversed", true)] {
        let repo = LockRepo::code_a(label);
        repo.accept_ok("PIECE-COUNTER");
        repo.accept_ok("LAW-BOUND");
        let text = repo.lock_text();
        let mut chunks: Vec<&str> = text.split("\n[[binding]]\n").collect();
        let preamble = chunks.remove(0);
        if reverse {
            chunks.reverse();
        }
        let mut written = preamble.to_owned();
        for chunk in &chunks {
            written.push_str("\n[[binding]]\n");
            written.push_str(chunk.trim_end_matches('\n'));
            written.push('\n');
        }
        // Both histories commit once more: the reversed lock, or a note.
        if reverse {
            assert_ne!(written, text, "reversed");
            repo.write("spec.lock", &written);
        } else {
            assert_eq!(written, text, "canonical");
            repo.write("NOTES.txt", "a note\n");
        }
        repo.commit_all("one more commit");
        let head = repo.head();
        let attempt = repo.accept("GOAL-01", true);
        attempt.committed("GOAL-01");
        let normalize = |text: &str| {
            text.replace(&head, "<head>")
                .replace(&repo.root.display().to_string(), "<worktree>")
        };
        runs.push((
            normalize(&attempt.rows()),
            normalize(attempt.question()),
            normalize(&repo.lock_text()),
        ));
    }
    assert_eq!(runs[1], runs[0]);
    assert!(runs[0].2.contains("commit = \"<head>\""), "{}", runs[0].2);
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
/// keys of its kind (lock-levels.md "Format 2"), `file` a clean relative
/// path, every OID 40 or 64 hex,
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
        let kind = value(&block, "kind").expect("kind");
        assert_eq!(keys, entry_keys(kind), "{block:?}");
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

// --------------------------------------------- lock-levels.md RD-3

/// `stops_at_cap`'s marker as code-a writes it.
const STOPS_MARKER: &str = "// @verifies LAW-BOUND@2 [body] mutation: drop the cap";

/// `stops_at_cap`'s row tail.
const STOPS: &str = "verifies | crates/tally/tests/bounds.rs | item crates/tally::test:bounds::stops_at_cap function_item";

/// The block of `node`'s entry whose `symbol` is `symbol`.
fn block_of(lock: &str, node: &str, symbol: &str) -> String {
    let found: Vec<String> = node_blocks(lock, node)
        .into_iter()
        .filter(|block| block.contains(&format!("\nsymbol = \"{symbol}\"\n")))
        .collect();
    assert_eq!(found.len(), 1, "one {node} {symbol}:\n{lock}");
    found[0].clone()
}

/// RD-3 (`levels_are_the_union_of_the_markers`): an entry's `levels` is
/// the union of the effective levels of the markers giving its key, in
/// canonical order, a marker without a list as `sig, body`; its digests
/// every level's whatever the list; a list that changes the union
/// refreshes the entry.
#[test]
fn levels_are_the_union_of_the_markers() {
    let repo = LockRepo::code_a("lock-levels-union");
    repo.replace(
        BOUNDS,
        STOPS_MARKER,
        &format!("// @verifies LAW-BOUND@2 [deps, path]\n{STOPS_MARKER}"),
    );
    repo.replace(
        COUNTER,
        "/// The largest count; see LAW-BOUND.\npub const CAP",
        "/// The largest count; see LAW-BOUND.\n// @configures LAW-BOUND@2 [deps]\npub const CAP",
    );
    repo.commit_all("more markers");
    let attempt = repo.accept("LAW-BOUND", true);
    attempt.committed("LAW-BOUND");
    assert!(
        attempt
            .question()
            .starts_with("lock LAW-BOUND rev 2 (accept: 4 new, 0 refreshed, 0 kept, 0 dropped)"),
        "{}",
        attempt.question()
    );
    let head = repo.head();
    let lock = repo.lock_text();
    let counter = repo.show_blob(&head, COUNTER);
    let bounds = repo.show_blob(&head, BOUNDS);
    let line_of = |source: &[u8], start: &str| {
        String::from_utf8_lossy(source)
            .lines()
            .position(|line| line.starts_with(start))
            .expect("the item")
            + 1
    };
    for (symbol, source, start, kind, levels) in [
        (
            "crates/tally::test:bounds::stops_at_cap",
            &bounds,
            "fn stops_at_cap",
            "function_item",
            &["path", "body", "deps"][..],
        ),
        (
            "crates/tally::counter::CAP",
            &counter,
            "pub const CAP",
            "const_item",
            &["sig", "body", "deps"][..],
        ),
        (
            "crates/tally::counter::<Tally as fmt::Display>::fmt",
            &counter,
            "    fn fmt(",
            "function_item",
            &DEFAULT[..],
        ),
    ] {
        let block = block_of(&lock, "LAW-BOUND", symbol);
        let code = item_code(source, line_of(source, start), kind, levels);
        assert!(block.contains(&code.lines()), "{symbol}:\n{block}");
    }
    let ron = block_of(&lock, "LAW-BOUND", "root.cap");
    assert!(ron.contains("\nlevels = [\"sig\", \"body\"]\n"), "{ron}");

    // The `[deps, path]` marker made `[path]`: the union `path, body`, a
    // refresh of that entry alone.
    repo.replace(BOUNDS, "[deps, path]", "[path]");
    repo.commit_all("a narrower list");
    let attempt = repo.accept("LAW-BOUND", true);
    attempt.committed("the narrower union");
    assert!(
        attempt
            .question()
            .starts_with("lock LAW-BOUND rev 2 (accept: 0 new, 1 refreshed, 0 kept, 0 dropped)"),
        "{}",
        attempt.question()
    );
    assert!(
        attempt.rows().contains(&format!("refreshed | {STOPS}\n")),
        "{}",
        attempt.rows()
    );
    let block = block_of(
        &repo.lock_text(),
        "LAW-BOUND",
        "crates/tally::test:bounds::stops_at_cap",
    );
    assert!(
        block.contains("\nlevels = [\"path\", \"body\"]\n"),
        "{block}"
    );
}

/// RD-3 **M5** (`Invalid` locked as the default): a binding whose markers
/// carry an `Invalid` level list (`unclosed`, `unknown`, `duplicate`,
/// `empty`) is never locked: `not locked … | bad_levels`;
/// `cap-check/PASS-01`'s only marker `[Sig]` leaves nothing lockable (exit
/// 1, no question, nothing written); one invalid marker among valid ones
/// for a key refuses it; an entry locked while its list was valid is
/// `kept`, byte for byte, whichever way the list breaks.
#[test]
fn invalid_levels_are_not_locked() {
    let repo = LockRepo::code_a("lock-levels-invalid");
    let state = repo.state();
    let attempt = repo.accept("cap-check/PASS-01", true);
    let refusal = attempt.refused("PASS-01");
    assert!(refusal.contains("can be locked"), "{refusal}");
    assert!(attempt.prompts.is_empty());
    assert_eq!(attempt.rows(), PASS_ROW);
    assert_eq!(repo.state(), state, "nothing written");
    assert!(repo.lock().is_none());

    repo.accept_ok("GOAL-01");
    let lock = repo.lock_text();
    assert_eq!(node_blocks(&lock, "GOAL-01").len(), 1, "{lock}");
    assert!(!lock.contains("inner::limit"), "{lock}");

    repo.accept_ok("LAW-BOUND");
    let locked = block_of(
        &repo.lock_text(),
        "LAW-BOUND",
        "crates/tally::test:bounds::stops_at_cap",
    );
    assert!(locked.contains("\nlevels = [\"body\"]\n"), "{locked}");
    let mut current = STOPS_MARKER.to_owned();
    for (label, marker) in [
        (
            "unclosed",
            "// @verifies LAW-BOUND@2 [body mutation: drop the cap",
        ),
        (
            "unknown",
            "// @verifies LAW-BOUND@2 [Body] mutation: drop the cap",
        ),
        (
            "duplicate",
            "// @verifies LAW-BOUND@2 [body, body] mutation: drop the cap",
        ),
        (
            "empty",
            "// @verifies LAW-BOUND@2 [] mutation: drop the cap",
        ),
    ] {
        repo.replace(BOUNDS, &current, marker);
        current = marker.to_owned();
        repo.commit_all(label);
        let attempt = repo.accept("LAW-BOUND", true);
        let rows = attempt.rows();
        assert!(
            rows.contains(&format!("kept | {STOPS} | bad_levels\n")),
            "{label}:\n{rows}"
        );
        let outcome = attempt
            .outcome
            .as_ref()
            .unwrap_or_else(|error| panic!("{label}: {error}"));
        assert!(outcome.commit.is_none(), "{label}: {outcome:?}");
        assert_eq!(
            block_of(
                &repo.lock_text(),
                "LAW-BOUND",
                "crates/tally::test:bounds::stops_at_cap"
            ),
            locked,
            "{label}: kept byte for byte"
        );
    }

    // One key from two markers, the first `[Sig]`: the key refused.
    let two = LockRepo::code_a("lock-levels-invalid-two");
    two.replace(
        BOUNDS,
        STOPS_MARKER,
        &format!("// @verifies LAW-BOUND@2 [Sig]\n{STOPS_MARKER}"),
    );
    two.commit_all("an invalid marker first");
    let attempt = two.accept("LAW-BOUND", true);
    attempt.committed("LAW-BOUND");
    assert!(
        attempt
            .rows()
            .contains(&format!("not locked | {STOPS} | bad_levels\n")),
        "{}",
        attempt.rows()
    );
    assert!(
        attempt
            .question()
            .starts_with("lock LAW-BOUND rev 2 (accept: 3 new, 0 refreshed, 0 kept, 0 dropped)"),
        "{}",
        attempt.question()
    );
    assert!(!two.lock_text().contains("stops_at_cap"));
}

/// The row of `attempt` naming `place`, its state the last field.
fn row_naming<'a>(rows: &'a str, place: &str) -> &'a str {
    let found: Vec<&str> = rows.lines().filter(|row| row.contains(place)).collect();
    assert_eq!(found.len(), 1, "one row naming {place}:\n{rows}");
    found[0]
}

/// RD-3 (Q31): a `module` whose file has any parse error is never locked,
/// `not locked … | cannot_verify <category>`; an entry locked while the
/// file parsed is `kept`, byte for byte.
#[test]
fn module_file_with_a_parse_error_is_not_locked() {
    let module = "verifies | crates/tally/tests/bounds.rs | module crates/tally::test:bounds";
    let broken = "\nfn broken( {\n";

    let repo = LockRepo::code_a("lock-module-error");
    repo.accept_ok("PIECE-COUNTER");
    let locked = block_of(
        &repo.lock_text(),
        "PIECE-COUNTER",
        "crates/tally::test:bounds",
    );
    repo.append(BOUNDS, broken);
    repo.commit_all("an error in bounds.rs");
    let attempt = repo.accept("PIECE-COUNTER", true);
    let rows = attempt.rows();
    let row = row_naming(&rows, module);
    let state = row
        .strip_prefix(&format!("kept | {module} | "))
        .unwrap_or_else(|| panic!("kept: {row}"));
    assert!(
        state.starts_with("cannot_verify ") && state.len() > "cannot_verify ".len(),
        "a category: {row}"
    );
    let outcome = attempt.outcome.as_ref().expect("an answer");
    assert!(outcome.commit.is_none(), "{outcome:?}");
    assert_eq!(
        block_of(
            &repo.lock_text(),
            "PIECE-COUNTER",
            "crates/tally::test:bounds"
        ),
        locked
    );

    let fresh = LockRepo::code_a("lock-module-error-fresh");
    fresh.append(BOUNDS, broken);
    fresh.commit_all("an error in bounds.rs");
    let attempt = fresh.accept("PIECE-COUNTER", true);
    attempt.committed("the other three");
    let rows = attempt.rows();
    let row = row_naming(&rows, module);
    assert!(
        row.starts_with(&format!("not locked | {module} | cannot_verify ")),
        "{row}"
    );
    assert!(
        !fresh
            .lock_text()
            .contains("symbol = \"crates/tally::test:bounds\"\n"),
        "the broken module locked"
    );
}

/// RD-3 (Q31): a `ron` value of a file `RonAnalysis` rejects (trailing
/// content) is never locked, `not locked … | cannot_verify
/// trailing_content`; an entry locked while the file was read is `kept`,
/// byte for byte.
#[test]
fn ron_value_of_a_rejected_file_is_not_locked() {
    let ron = "configures | data/limits.ron | ron root";

    let repo = LockRepo::code_a("lock-ron-rejected");
    repo.accept_ok("PIECE-COUNTER");
    let locked = block_of(&repo.lock_text(), "PIECE-COUNTER", "root");
    repo.append(LIMITS, "extra\n");
    repo.commit_all("trailing content");
    let attempt = repo.accept("PIECE-COUNTER", true);
    let rows = attempt.rows();
    assert_eq!(
        row_naming(&rows, &format!("{ron} ")),
        format!("kept | {ron} | cannot_verify trailing_content")
    );
    let outcome = attempt.outcome.as_ref().expect("an answer");
    assert!(outcome.commit.is_none(), "{outcome:?}");
    assert_eq!(block_of(&repo.lock_text(), "PIECE-COUNTER", "root"), locked);

    let fresh = LockRepo::code_a("lock-ron-rejected-fresh");
    fresh.append(LIMITS, "extra\n");
    fresh.commit_all("trailing content");
    let attempt = fresh.accept("PIECE-COUNTER", true);
    attempt.committed("the other three");
    let rows = attempt.rows();
    assert_eq!(
        row_naming(&rows, &format!("{ron} ")),
        format!("not locked | {ron} | cannot_verify trailing_content")
    );
    assert!(!fresh.lock_text().contains("kind = \"ron\""));
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

// ------------------------------------------- tested-evidence (8b-1) RD-7

const STAMINA_RS: &str = "crates/game/src/stamina.rs";
const STAMINA_MD: &str = "docs/spec/stamina.md";

/// The blocks of a lock's text, by node.
fn blocks_of(text: &str) -> Vec<(String, String)> {
    tested_common::tested_blocks(text)
}

/// The JSON accept prints for `outcome`.
fn accept_json(outcome: &specengine_cli::LockOutcome) -> serde_json::Value {
    let text =
        specengine_cli::render_json(&specengine_cli::Outcome::Lock(Box::new(outcome.clone())));
    serde_json::from_str(text.trim_end()).expect("accept's document")
}

/// RD-7 (docs/features/tested-evidence.md "Other lock writers", accept:
/// kept byte for byte; M: accept rewrites a block): over the ECS corpus
/// with a block per node state, PERK-DRAIN's code changed and accepted:
/// its entry refreshed, every block (its own too) byte for byte, `format
/// = 3`; its block's row `kept | tested` after the entries' rows, the
/// ninth JSON key `tested` `"kept"`.
#[test]
fn accept_keeps_tested_bytes() {
    let (repo, _, _) = tested_common::ecs_with_blocks("la-tested-keep");
    let before = repo.lock_text();
    repo.replace(
        STAMINA_RS,
        "(stamina.current - cost * dt).max(0.0)",
        "(stamina.current - cost * dt * 2.0).max(0.0)",
    );
    repo.commit_all("drain doubles");
    let attempt = repo.accept("PERK-DRAIN", true);
    let outcome = attempt.committed("PERK-DRAIN").clone();
    let after = repo.lock_text();
    assert_eq!(after.lines().nth(1), Some("format = 3"));
    assert_eq!(blocks_of(&after), blocks_of(&before), "every block kept");
    assert_ne!(
        node_blocks(&after, "PERK-DRAIN"),
        node_blocks(&before, "PERK-DRAIN"),
        "its entry refreshed"
    );
    assert_eq!(outcome.tested, Some("kept"));
    let rows = attempt.rows();
    assert_eq!(rows.lines().last(), Some("kept | tested"), "{rows}");
    assert_eq!(accept_json(&outcome)["tested"], "kept");
    // A node without a block: no row, `null`.
    repo.replace(
        STAMINA_RS,
        "pub const BASE_REGEN: f32 = 2.5;",
        "pub const BASE_REGEN: f32 = 3.5;",
    );
    repo.commit_all("a new base regen");
    let attempt = repo.accept("TUNE-01", true);
    let outcome = attempt.committed("TUNE-01").clone();
    assert_eq!(outcome.tested, None);
    assert!(!attempt.rows().contains("| tested"), "{}", attempt.rows());
    assert_eq!(accept_json(&outcome)["tested"], serde_json::Value::Null);
}

/// RD-7 (accept: "the node's block at R with another `spec_hash` refuses as
/// an entry"; M: a block at R with another hash passes accept): PERK-REGEN
/// holding a block and no entry, its text changed without a raise and
/// committed: accept refuses as over an entry (exit 1, `--editorial`,
/// nothing written, no question), its parenthesis `([[tested]]: spec_hash
/// <old>, now <new>)`.
#[test]
fn accept_refuses_text_changed_under_a_block() {
    let repo = verify_common::ecs("la-tested-text");
    let text = repo.lock_text();
    let block = tested_common::vouching_block(&text, "PERK-REGEN", &repo.head());
    let old = block.spec_hash.clone();
    repo.write(
        "spec.lock",
        tested_common::with_blocks_added(
            &tested_common::without_entries(&text, "PERK-REGEN"),
            &[block],
        ),
    );
    repo.commit_all("PERK-REGEN's evidence alone");
    repo.replace(
        STAMINA_MD,
        "Stamina regains a fixed amount per second while the runner rests.",
        "Stamina regains a fixed amount each second while the runner rests.",
    );
    repo.commit_all("a new meaning, no raise");
    let new = repo.norm_hash("PERK-REGEN");
    assert_ne!(new, old);
    let before = repo.state();
    let attempt = repo.accept("PERK-REGEN", true);
    let refusal = attempt.refused("text changed under a block");
    assert!(attempt.prompts.is_empty(), "asked: {:?}", attempt.prompts);
    for part in [
        "PERK-REGEN".to_owned(),
        "--editorial".to_owned(),
        "nothing written".to_owned(),
        format!("([[tested]]: spec_hash {old}, now {new})"),
    ] {
        assert!(refusal.contains(&part), "{part:?} not in {refusal}");
    }
    assert_eq!(repo.state(), before);
}

/// RD-7 (a gone node: "dropped with the entries, `dropped | tested`; a
/// block alone makes the node gone"; M: a gone node's block kept):
/// PERK-DRAIN's section deleted: accept drops its entry and its block, the
/// question counting the block as an entry (`gone: 2 dropped`), its rows
/// the entry's then `dropped | tested`; the other blocks byte for byte;
/// GONE-01, holding a block alone, is gone too: `gone: 1 dropped`; the
/// last block dropped writes format 2.
#[test]
fn gone_accept_drops_tested() {
    let (repo, _, _) = tested_common::ecs_with_blocks("la-tested-gone");
    let before = repo.lock_text();
    repo.replace(
        STAMINA_MD,
        "## Drain {#PERK-DRAIN}\n\nSprinting drains stamina; at zero the runner walks.\n",
        "",
    );
    repo.commit_all("PERK-DRAIN deleted");
    let head = repo.head();
    let attempt = repo.accept("PERK-DRAIN", true);
    let outcome = attempt.committed("gone PERK-DRAIN").clone();
    assert_eq!(
        attempt.question(),
        format!(
            "lock PERK-DRAIN (gone: 2 dropped){}",
            at(&head, "main", &repo.root)
        )
    );
    let rows = attempt.rows();
    assert!(rows.starts_with("dropped | implements | "), "{rows}");
    assert_eq!(rows.lines().last(), Some("dropped | tested"), "{rows}");
    assert_eq!(outcome.tested, Some("dropped"));
    let after = repo.lock_text();
    assert!(tested_common::block_of(&after, "PERK-DRAIN").is_none());
    assert!(node_blocks(&after, "PERK-DRAIN").is_empty());
    for node in ["GONE-01", "PERK-REGEN", "PERK-STAMINA"] {
        assert_eq!(
            tested_common::block_of(&after, node),
            tested_common::block_of(&before, node),
            "{node}"
        );
    }

    let head = repo.head();
    let attempt = repo.accept("GONE-01", true);
    attempt.committed("gone GONE-01, a block alone");
    assert_eq!(
        attempt.question(),
        format!(
            "lock GONE-01 (gone: 1 dropped){}",
            at(&head, "main", &repo.root)
        )
    );
    assert_eq!(attempt.rows(), "dropped | tested\n");
    assert!(tested_common::block_of(&repo.lock_text(), "GONE-01").is_none());

    // The last block dropped: format 2, the entries' bytes as before.
    let repo = verify_common::ecs("la-tested-gone-last");
    let text = repo.lock_text();
    repo.write(
        "spec.lock",
        tested_common::with_blocks_added(
            &text,
            &[tested_common::made_up_block(
                "GONE-01",
                1,
                '9',
                &repo.head(),
            )],
        ),
    );
    repo.commit_all("one block");
    repo.accept("GONE-01", true).committed("the last block");
    assert_eq!(repo.lock_text(), text, "format 2 again, byte for byte");
}
