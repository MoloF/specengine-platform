//! docs/features/spec-lock.md TE-7, RD-13 and regen's exits (TE-3):
//! `spec lock regen` after a real `git merge` of scratch branches whose
//! `spec.lock` the test writes (regen computes no hash, so its entries are
//! synthetic: valid forms, made-up digests), over scratch repositories of
//! `fixtures/code-a` (its `[code]` table; no code or spec file is read).
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;
mod lock_common;

use std::fs;

use lock_common::{
    Code, LockRepo, REGEN_TEXT, block, block_format_1, fake_b3, fake_oid, lock_of, lock_of_format_1,
};
use specengine_cli::LockPrompt;

const NEXT: &str = "next: git add spec.lock, then commit the merge";

/// One synthetic format-2 item entry of `node`, version `v` (its digests,
/// fingerprints, blob and commit made of `v`), verified on `date`.
fn entry(node: &str, v: char, date: &str) -> String {
    let symbol = format!(
        "crates/tally::{}",
        node.to_ascii_lowercase().replace('-', "_")
    );
    block(
        node,
        1,
        &fake_b3(v),
        "implements",
        "crates/tally/src/lib.rs",
        "item",
        &symbol,
        Some("function_item"),
        &Code::fake(v, true),
        &fake_oid(v),
        &fake_oid(v),
        date,
    )
}

/// [`entry`] as a format-1 block (`ast_hash`).
fn entry_format_1(node: &str, v: char, date: &str) -> String {
    let symbol = format!(
        "crates/tally::{}",
        node.to_ascii_lowercase().replace('-', "_")
    );
    block_format_1(
        node,
        1,
        &fake_b3(v),
        "implements",
        "crates/tally/src/lib.rs",
        "item",
        &symbol,
        Some(("function_item", &fake_b3(v))),
        &fake_oid(v),
        &fake_oid(v),
        date,
    )
}

/// The row of `node` as regen prints it.
fn row(choice: &str, node: &str) -> String {
    format!(
        "{choice} | {node} | implements | crates/tally/src/lib.rs | item crates/tally::{}\n",
        node.to_ascii_lowercase().replace('-', "_")
    )
}

const BASE_DAY: &str = "2026-10-01";

/// The three locks of the many-case merge (TE-7): base, ours, theirs,
/// and the result regen must write, with its rows (ours = `main`).
struct Case {
    base: Option<String>,
    ours: Option<String>,
    theirs: Option<String>,
}

fn many_cases() -> (Case, String, String, String) {
    let base = lock_of(&[
        entry("AAA-01", '1', BASE_DAY),
        entry("BBB-01", '1', BASE_DAY),
        entry("CCC-01", '1', BASE_DAY),
        entry("DDD-01", '1', BASE_DAY),
        entry("EEE-01", '1', BASE_DAY),
        entry("FFF-01", '1', BASE_DAY),
        entry("GGG-01", '1', BASE_DAY),
        entry("JJJ-01", '1', BASE_DAY),
        entry("KKK-01", '1', BASE_DAY),
    ]);
    let ours = lock_of(&[
        entry("AAA-01", '1', BASE_DAY),
        entry("BBB-01", '2', "2026-10-05"),
        entry("CCC-01", '1', BASE_DAY),
        entry("FFF-01", '5', "2026-10-09"),
        entry("GGG-01", '8', "2026-10-08"),
        entry("HHH-01", '9', "2026-10-09"),
        entry("III-01", 'b', "2026-10-06"),
        entry("KKK-01", 'c', "2026-10-06"),
    ]);
    let theirs = lock_of(&[
        entry("AAA-01", '1', BASE_DAY),
        entry("BBB-01", '1', BASE_DAY),
        entry("CCC-01", '3', "2026-10-04"),
        entry("DDD-01", '1', BASE_DAY),
        entry("EEE-01", '4', "2026-10-04"),
        entry("FFF-01", '6', "2026-10-07"),
        entry("GGG-01", '7', "2026-10-08"),
        entry("HHH-01", 'a', "2026-10-08"),
        entry("III-01", 'b', "2026-10-06"),
        entry("KKK-01", 'c', "2026-10-06"),
    ]);
    let result = lock_of(&[
        entry("AAA-01", '1', BASE_DAY),
        entry("BBB-01", '2', "2026-10-05"),
        entry("CCC-01", '3', "2026-10-04"),
        entry("EEE-01", '4', "2026-10-04"),
        entry("FFF-01", '5', "2026-10-09"),
        entry("GGG-01", '7', "2026-10-08"),
        entry("HHH-01", '9', "2026-10-09"),
        entry("III-01", 'b', "2026-10-06"),
        entry("KKK-01", 'c', "2026-10-06"),
    ]);
    let rows = |ours: &str, theirs: &str| {
        [
            row("both", "AAA-01"),
            row(ours, "BBB-01"),
            row(theirs, "CCC-01"),
            row("deleted", "DDD-01"),
            row(theirs, "EEE-01"),
            row("later", "FFF-01"),
            row("bytes", "GGG-01"),
            row("later", "HHH-01"),
            row("both", "III-01"),
            row("deleted", "JJJ-01"),
            row("both", "KKK-01"),
        ]
        .concat()
    };
    (
        Case {
            base: Some(base),
            ours: Some(ours),
            theirs: Some(theirs),
        },
        result,
        rows("ours", "theirs"),
        rows("theirs", "ours"),
    )
}

/// `spec.lock` as `text` (`None`: removed), committed.
fn put(repo: &LockRepo, text: Option<&str>, message: &str) {
    match text {
        Some(text) => repo.write("spec.lock", text),
        None => {
            let _ = fs::remove_file(repo.root.join("spec.lock"));
        }
    }
    repo.commit_all(message);
}

/// A code-a repository with `case`'s base committed on `main`, `theirs`
/// on the branch `theirs`, `ours` on `main`; then `into` (`main` or
/// `theirs`) merges the other. Whether git merged cleanly.
fn merged(label: &str, case: &Case, into: &str) -> (LockRepo, bool) {
    let repo = LockRepo::code_a(label);
    if case.base.is_some() {
        put(&repo, case.base.as_deref(), "base lock");
    }
    repo.git(&["switch", "-q", "-c", "theirs"]);
    put(&repo, case.theirs.as_deref(), "their lock");
    repo.git(&["switch", "-q", "main"]);
    put(&repo, case.ours.as_deref(), "our lock");
    let other = if into == "main" { "theirs" } else { "main" };
    repo.git(&["switch", "-q", into]);
    let merge = repo
        .git
        .git_output(&repo.root, &["merge", "-q", "--no-edit", other], &[]);
    (repo, merge.status.success())
}

/// TE-7: one merge holding every per-entry case (same on both; changed on
/// one side; deleted on one side, unchanged or changed on the other;
/// changed differently, by `verified_at` or by bytes; add/add without a
/// base, different and identical; deleted on both): regen asks once,
/// writes exactly the hand-derived lock to the worktree only, prints the
/// rows and `next: …`.
#[test]
fn regen_merges_every_case() {
    let (case, result, rows, _) = many_cases();
    let (repo, clean) = merged("lock-regen-many", &case, "main");
    assert!(!clean, "the merge conflicts in spec.lock");
    let unmerged = repo.git_text(&["ls-files", "-u", "--", "spec.lock"]);
    assert!(!unmerged.is_empty());
    let index = repo.git_text(&["ls-files", "-s"]);
    let attempt = repo.regen(true);
    attempt.written("many");
    assert_eq!(attempt.rows(), rows);
    assert_eq!(
        attempt.question(),
        format!(
            "merge spec.lock (9 entries, 3 by verified_at or bytes) in {}? [y/N]",
            repo.root.display()
        )
    );
    assert_eq!(repo.lock_text(), result);
    assert_eq!(
        repo.git_text(&["ls-files", "-s"]),
        index,
        "the index untouched"
    );
    // The owner's next step works.
    repo.git(&["add", "spec.lock"]);
    repo.git(&["commit", "-q", "--no-verify", "--no-edit"]);
    assert_eq!(repo.show_blob("HEAD", "spec.lock"), result.into_bytes());
}

/// RD-13 (M: always ours): the same branches merged the other way round
/// give the same file; the rows name the sides swapped.
#[test]
fn regen_is_symmetric() {
    let (case, result, rows, swapped) = many_cases();
    let (one, _) = merged("lock-regen-sym-a", &case, "main");
    let (two, _) = merged("lock-regen-sym-b", &case, "theirs");
    let first = one.regen(true);
    first.written("main <- theirs");
    let second = two.regen(true);
    second.written("theirs <- main");
    assert_eq!(first.rows(), rows);
    assert_eq!(second.rows(), swapped);
    assert_eq!(one.lock_text(), result);
    assert_eq!(two.lock_text(), result);
}

/// RD-13 (M: a hash computed): with every code and spec file removed from
/// the worktree, regen prints and writes the same.
#[test]
fn regen_reads_no_code_and_no_spec() {
    let (case, result, rows, _) = many_cases();
    let (repo, _) = merged("lock-regen-blind", &case, "main");
    for dir in ["crates", "data", "tools", "docs"] {
        fs::remove_dir_all(repo.root.join(dir)).expect("remove");
    }
    let attempt = repo.regen(true);
    attempt.written("no code, no spec");
    assert_eq!(attempt.rows(), rows);
    assert_eq!(repo.lock_text(), result);
}

/// TE-7: add/add (no base), modify/delete and delete/modify of the whole
/// file, each a conflict regen resolves per entry.
#[test]
fn regen_resolves_whole_file_conflicts() {
    // add/add: no base lock.
    let add = Case {
        base: None,
        ours: Some(lock_of(&[
            entry("XXX-01", '1', "2026-10-02"),
            entry("YYY-01", '2', "2026-10-02"),
        ])),
        theirs: Some(lock_of(&[
            entry("XXX-01", '3', "2026-10-03"),
            entry("ZZZ-01", '4', "2026-10-02"),
        ])),
    };
    let (repo, clean) = merged("lock-regen-addadd", &add, "main");
    assert!(!clean);
    let attempt = repo.regen(true);
    attempt.written("add/add");
    assert_eq!(
        attempt.rows(),
        [
            row("later", "XXX-01"),
            row("ours", "YYY-01"),
            row("theirs", "ZZZ-01")
        ]
        .concat()
    );
    assert_eq!(
        repo.lock_text(),
        lock_of(&[
            entry("XXX-01", '3', "2026-10-03"),
            entry("YYY-01", '2', "2026-10-02"),
            entry("ZZZ-01", '4', "2026-10-02"),
        ])
    );

    // Ours deletes the lock; theirs changes one entry, keeps the other.
    let base = lock_of(&[
        entry("XXX-01", '1', BASE_DAY),
        entry("YYY-01", '1', BASE_DAY),
    ]);
    let delete = Case {
        base: Some(base.clone()),
        ours: None,
        theirs: Some(lock_of(&[
            entry("XXX-01", '5', "2026-10-05"),
            entry("YYY-01", '1', BASE_DAY),
        ])),
    };
    let (repo, clean) = merged("lock-regen-delmod", &delete, "main");
    assert!(!clean, "modify/delete conflicts");
    let attempt = repo.regen(true);
    attempt.written("delete/modify");
    assert_eq!(
        attempt.rows(),
        [row("theirs", "XXX-01"), row("deleted", "YYY-01")].concat()
    );
    assert_eq!(
        repo.lock_text(),
        lock_of(&[entry("XXX-01", '5', "2026-10-05")])
    );

    // The same, sides swapped.
    let (repo, _) = merged("lock-regen-model", &delete, "theirs");
    let attempt = repo.regen(true);
    attempt.written("modify/delete");
    assert_eq!(
        attempt.rows(),
        [row("ours", "XXX-01"), row("deleted", "YYY-01")].concat()
    );
    assert_eq!(
        repo.lock_text(),
        lock_of(&[entry("XXX-01", '5', "2026-10-05")])
    );
}

/// TE-7: blocks added far apart (ours near the start, theirs near the
/// end) merge cleanly into the canonical lock of both; regen then has no
/// conflict (exit 1).
#[test]
fn non_adjacent_blocks_merge_cleanly() {
    let base = lock_of(&[
        entry("AAA-01", '1', BASE_DAY),
        entry("MMM-01", '1', BASE_DAY),
        entry("ZZZ-01", '1', BASE_DAY),
    ]);
    let case = Case {
        base: Some(base),
        ours: Some(lock_of(&[
            entry("AAA-01", '1', BASE_DAY),
            entry("BBB-01", '2', "2026-10-02"),
            entry("MMM-01", '1', BASE_DAY),
            entry("ZZZ-01", '1', BASE_DAY),
        ])),
        theirs: Some(lock_of(&[
            entry("AAA-01", '1', BASE_DAY),
            entry("MMM-01", '1', BASE_DAY),
            entry("YYY-01", '3', "2026-10-03"),
            entry("ZZZ-01", '1', BASE_DAY),
        ])),
    };
    let (repo, clean) = merged("lock-regen-apart", &case, "main");
    assert!(clean, "git merges blocks apart cleanly");
    assert_eq!(
        repo.lock_text(),
        lock_of(&[
            entry("AAA-01", '1', BASE_DAY),
            entry("BBB-01", '2', "2026-10-02"),
            entry("MMM-01", '1', BASE_DAY),
            entry("YYY-01", '3', "2026-10-03"),
            entry("ZZZ-01", '1', BASE_DAY),
        ])
    );
    let before = repo.state();
    let refusal = repo.regen(true).refused("merged cleanly");
    assert!(refusal.contains("not unmerged"), "{refusal}");
    assert_eq!(repo.state(), before);
}

/// RD-13 (M: exit 0): no conflict in spec.lock (no merge; identical sides
/// merged; a conflict in another file only): exit 1, nothing written, no
/// question.
#[test]
fn regen_without_a_conflict_is_exit_1() {
    let repo = LockRepo::code_a("lock-regen-none");
    repo.accept_ok("PREM-001");
    let before = repo.state();
    let attempt = repo.regen(true);
    attempt.refused("no merge");
    assert!(attempt.prompts.is_empty());
    assert_eq!(repo.state(), before);

    let same = lock_of(&[entry("XXX-01", '2', "2026-10-02")]);
    let identical = Case {
        base: Some(lock_of(&[entry("XXX-01", '1', BASE_DAY)])),
        ours: Some(same.clone()),
        theirs: Some(same),
    };
    let (repo, clean) = merged("lock-regen-identical", &identical, "main");
    assert!(clean, "identical sides merge");
    let before = repo.state();
    repo.regen(true).refused("identical sides");
    assert_eq!(repo.state(), before);

    // A conflict elsewhere only.
    let repo = LockRepo::code_a("lock-regen-elsewhere");
    repo.git(&["switch", "-q", "-c", "side"]);
    repo.write("NOTES.txt", "side\n");
    repo.commit_all("side");
    repo.git(&["switch", "-q", "main"]);
    repo.write("NOTES.txt", "main\n");
    repo.commit_all("main");
    let merge = repo
        .git
        .git_output(&repo.root, &["merge", "-q", "side"], &[]);
    assert!(!merge.status.success());
    let before = repo.state();
    let attempt = repo.regen(true);
    attempt.refused("a conflict elsewhere");
    assert!(attempt.prompts.is_empty());
    assert_eq!(repo.state(), before);
}

/// TE-7, TE-3, docs/features/lock-levels.md Q28: a stage of format 1, or
/// stages differing in `code_recipe`, exit 2 with regen's one text (`git
/// merge --abort`, `spec lock rebase` on the lagging branch, merge again),
/// before any question, nothing written; a stage that cannot be read
/// exits 2 naming `spec.lock`. Each side: ours or theirs the odd one.
#[test]
fn regen_refuses_other_headers_and_unreadable_stages() {
    let other_code = lock_of(&[entry("XXX-01", '2', "2026-10-02")]).replace(
        "specengine-hash/v3 tree-sitter-rust/0.24.2 abi/15",
        "specengine-hash/v3 tree-sitter-rust/0.25.0 abi/15",
    );
    let format_1 = lock_of_format_1(&[entry_format_1("XXX-01", '2', "2026-10-02")]);
    let unreadable = "format = 2\nthis is not a lock\n".to_owned();
    let mut failures = Vec::new();
    for (context, odd, named) in [
        ("another code_recipe", &other_code, REGEN_TEXT),
        ("format 1", &format_1, REGEN_TEXT),
        ("unreadable", &unreadable, "spec.lock"),
    ] {
        for odd_side in ["theirs", "ours"] {
            let normal = lock_of(&[entry("XXX-01", '3', "2026-10-03")]);
            let (ours, theirs) = if odd_side == "theirs" {
                (normal, odd.clone())
            } else {
                (odd.clone(), normal)
            };
            let case = Case {
                base: Some(lock_of(&[entry("XXX-01", '1', BASE_DAY)])),
                ours: Some(ours),
                theirs: Some(theirs),
            };
            let label = format!("lock-regen-header-{}", context.replace(' ', "-"));
            let (repo, clean) = merged(&label, &case, "main");
            assert!(!clean, "{context}: conflicts");
            let before = repo.state();
            let attempt = repo.regen(true);
            let message = attempt.cannot(context);
            let ok = if named == REGEN_TEXT {
                message == REGEN_TEXT
            } else {
                message.contains(named)
            };
            if !ok {
                failures.push(format!("{context} ({odd_side}): {message}"));
            }
            assert!(attempt.prompts.is_empty(), "{context} ({odd_side})");
            assert_eq!(repo.state(), before, "{context} ({odd_side})");
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Q28: stages of format 2 sharing one `code_recipe` merge as today even
/// when it is not this build's (regen hashes nothing: ADR-0041), the
/// header kept as the stages wrote it.
#[test]
fn regen_merges_stages_of_another_shared_code_recipe() {
    let other = |text: String| {
        text.replace(
            "specengine-hash/v3 tree-sitter-rust/0.24.2 abi/15",
            "specengine-hash/v3 tree-sitter-rust/0.25.0 abi/15",
        )
    };
    let case = Case {
        base: Some(other(lock_of(&[entry("XXX-01", '1', BASE_DAY)]))),
        ours: Some(other(lock_of(&[entry("XXX-01", '2', "2026-10-02")]))),
        theirs: Some(other(lock_of(&[entry("XXX-01", '3', "2026-10-03")]))),
    };
    let (repo, clean) = merged("lock-regen-shared-other", &case, "main");
    assert!(!clean, "conflicts");
    let attempt = repo.regen(true);
    attempt.written("another shared code_recipe");
    assert_eq!(
        repo.lock_text(),
        other(lock_of(&[entry("XXX-01", '3', "2026-10-03")])),
        "the later verified_at, the header the stages'"
    );
}

/// TE-3: regen declined, or the file changed while the question waits:
/// exit 1, the worktree file as the merge (or the change) left it.
#[test]
fn regen_declined_or_changed_is_exit_1() {
    let (case, _, _, _) = many_cases();
    let (repo, _) = merged("lock-regen-declined", &case, "main");
    let before = repo.state();
    let refusal = repo.regen(false).refused("declined");
    assert!(refusal.contains("not confirmed"), "{refusal}");
    assert_eq!(repo.state(), before);

    let mut consent = |_: &LockPrompt<'_>| {
        repo.append("spec.lock", "\n");
        true
    };
    let outcome = repo
        .regen_with(&repo.root, &mut consent)
        .expect("regen runs");
    assert!(
        outcome
            .refusal
            .as_deref()
            .is_some_and(|reason| reason.contains("changed while")),
        "{outcome:?}"
    );
    assert!(!outcome.written);
}

/// TE-7: regen on a terminal: rows on stdout, the question on stderr, `y`
/// writes, stdout ends with `next: …`.
#[test]
fn regen_on_a_terminal() {
    let (case, result, rows, _) = many_cases();
    let (repo, _) = merged("lock-regen-pty", &case, "main");
    let run = repo.spec_pty(&repo.root, &["lock", "regen"], Some("y"));
    run.code(0);
    assert_eq!(run.stdout, format!("{rows}{NEXT}\n"), "{}", run.show());
    assert_eq!(
        run.stderr.trim_end(),
        format!(
            "merge spec.lock (9 entries, 3 by verified_at or bytes) in {}? [y/N]",
            repo.root.display()
        )
    );
    assert_eq!(repo.lock_text(), result);
}
