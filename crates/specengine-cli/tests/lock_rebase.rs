//! docs/features/lock-rebase.md RD-1..RD-6 (with M1-M6) and TE-1..TE-4,
//! TE-6, TE-8: `spec lock rebase` over scratch repositories of
//! `fixtures/code-a`, through the library (`lock_rebase`, the owner's
//! answer a closure; no clock: a rebase keeps `verified_at` and `commit`)
//! and the `spec` binary on a pseudo-terminal where the printed text, the
//! terminal or the JSON document is the subject.
//!
//! The input is the format-1 golden (`fixtures/spec-lock/code-a.format-1.lock`,
//! 11 entries at v2's `code_recipe`), committed on the fixture's commit,
//! or accept's format-2 golden (`code-a.lock`) under another grammar's
//! `code_recipe` (`Variant::OtherCode`, a simulated grammar bump); each
//! drop reason is made by editing one entry: its `file_blob` an OID no
//! object has, a commit's or a tree's, a blob of bytes that are not UTF-8,
//! that do not parse, whose marker lists are invalid (written by `git
//! hash-object -w`: a blob no commit holds, read by OID alone), its
//! `symbol` one the blob lacks, its `ast_hash` another item's. The
//! rebased golden (`code-a.rebased.lock`) is derived from the two goldens
//! by key, never hand-written. The expected texts are the spec's "Data",
//! never the command's own output.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod drift_common;
mod lock_common;
mod verify_common;

use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use specengine_code::{LevelState, RustParser, module_levels};
use specengine_store::ProposalStatus;

use common::fixture;
use lock_common::rebase::{
    CHANGED_WHILE_REBASING, GOLDEN_DROPS, NO_LOCK, NOT_UTF8, NOTHING_TO_CHANGE, REBASED_GOLDEN,
    WRITE_REBASED_GOLDEN, chunks, derive_rebased_golden, edit_block, golden_rows, golden_text,
    head_note, joined, key_of, merge_text, not_this_builds_note, question, rebase_message,
    rebased_golden_file, rebased_golden_text, reversed, row, set_line, set_string, without_block,
};
use lock_common::{
    CODE_RECIPE, CODE_RECIPE_FORMAT_1, DAY, LockRepo, NORM_RECIPE, Variant, block_format_1, blocks,
    fake_oid, golden_format_1_text, item_code, item_hash, newer_format_text, node_blocks,
    older_format_text, value,
};
use verify_common::{Verify, verify};

const COUNTER: &str = "crates/tally/src/counter.rs";
const LIB: &str = "crates/tally/src/lib.rs";
const BOUNDS: &str = "crates/tally/tests/bounds.rs";
const LIMITS: &str = "data/limits.ron";

/// The keys edited below, as (`node`, `relation`, `symbol`).
type Named = (&'static str, &'static str, &'static str);
const CAP: Named = ("LAW-BOUND", "configures", "crates/tally::counter::CAP");
const STOPS: Named = (
    "LAW-BOUND",
    "verifies",
    "crates/tally::test:bounds::stops_at_cap",
);
const FMT: Named = (
    "LAW-BOUND",
    "implements",
    "crates/tally::counter::<Tally as fmt::Display>::fmt",
);
const RON_CAP: Named = ("LAW-BOUND", "configures", "root.cap");
const RON_ROOT: Named = ("PIECE-COUNTER", "configures", "root");
const BOUNDS_MODULE: Named = ("PIECE-COUNTER", "verifies", "crates/tally::test:bounds");
const LIB_MODULE: Named = ("PIECE-COUNTER", "implements", "crates/tally");
const VALUE: Named = (
    "GOAL-01",
    "implements",
    "crates/tally::counter::Tally::value",
);

/// The rows' `<kind> <symbol>[ <item_kind>]` of the keys above.
const CAP_WHAT: &str = "item crates/tally::counter::CAP const_item";
const STOPS_WHAT: &str = "item crates/tally::test:bounds::stops_at_cap function_item";
const FMT_WHAT: &str = "item crates/tally::counter::<Tally as fmt::Display>::fmt function_item";

/// The text of `spec refs` and accept when the config has no `[code]`.
const NO_CODE: &str =
    "spec: specengine.toml has no [code] table: the code layer is off (add [code] with roots)";

/// The format-1 golden's `commit`s (Q30, checked 2026-10-20): GOAL-01's
/// and LAW-BOUND's differ from accept's golden, PIECE-COUNTER's do not.
const GOAL_COMMITS: (&str, &str) = (
    "79ea966e9deb2ecb14ff0c11062c27376c8162b0",
    "85a2c98493892e599cf32e176533225ba892f410",
);
const BOUND_COMMITS: (&str, &str) = (
    "20825b3f7c607909aad25383081483a984ec963b",
    "82f6593a2654f7d98414f298d4a13064cf2f6e8f",
);
const PIECE_COMMIT: &str = "2ba8a3bce6422b0b9a15afe89954c2c6e0de98f8";

// ------------------------------------------------------------- helpers

/// A rebase that must be refused (exit 1) before any question, nothing
/// changed (TE-6): the refusal.
fn refused_quietly(repo: &LockRepo, context: &str) -> String {
    let before = repo.state();
    let attempt = repo.rebase(true);
    let refusal = attempt.refused(context);
    assert!(
        attempt.prompts.is_empty(),
        "{context}: asked {:?}",
        attempt.prompts
    );
    assert_eq!(repo.state(), before, "{context}: something changed");
    refusal
}

/// A rebase that cannot run (exit 2), no question, nothing changed
/// (TE-6): the message.
fn cannot_quietly(repo: &LockRepo, context: &str) -> String {
    let before = repo.state();
    let attempt = repo.rebase(true);
    let message = attempt.cannot(context);
    assert!(
        attempt.prompts.is_empty(),
        "{context}: asked {:?}",
        attempt.prompts
    );
    assert_eq!(repo.state(), before, "{context}: something changed");
    message
}

/// The format-1 golden with the one block of `named` edited by `edit`.
fn format_1_with(named: Named, edit: impl FnOnce(&str) -> String) -> String {
    edit_block(&golden_format_1_text(), named, edit)
}

/// [`golden_rows`] with the one row of `node` naming `what` replaced by
/// `new` (a whole row, LF-ended).
fn golden_rows_with(edits: &[(&str, &str, String)]) -> String {
    let mut rows: Vec<String> = golden_rows()
        .lines()
        .map(|line| format!("{line}\n"))
        .collect();
    for (node, what, new) in edits {
        let found: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, line)| {
                line.contains(&format!(" | {node} | "))
                    && (line.ends_with(&format!(" | {what}\n"))
                        || line.contains(&format!(" | {what} | ")))
            })
            .map(|(index, _)| index)
            .collect();
        assert_eq!(found.len(), 1, "one golden row {node} {what}");
        rows[found[0]] = new.clone();
    }
    rows.concat()
}

/// A golden row of `named`, `what`, made `dropped` with `reason`.
fn dropped(named: Named, file: &str, what: &str, reason: &str) -> (&'static str, String, String) {
    (
        named.0,
        what.to_owned(),
        row("dropped", named.0, named.1, file, what, Some(reason)),
    )
}

/// [`golden_rows_with`] of [`dropped`] rows.
fn golden_rows_dropping(drops: &[(&'static str, String, String)]) -> String {
    let edits: Vec<(&str, &str, String)> = drops
        .iter()
        .map(|(node, what, new)| (*node, what.as_str(), new.clone()))
        .collect();
    golden_rows_with(&edits)
}

/// A commit's raw message (`git cat-file commit`: after the header's
/// empty line), as git stores it.
fn raw_message(repo: &LockRepo, commit: &str) -> String {
    let raw = String::from_utf8(repo.git(&["cat-file", "commit", commit])).expect("UTF-8");
    raw.split_once("\n\n")
        .map(|(_, message)| message.to_owned())
        .expect("a message")
}

/// Whether `oid` names any object in `repo`.
fn has_object(repo: &LockRepo, oid: &str) -> bool {
    repo.git
        .git_output(&repo.root, &["cat-file", "-e", oid], &[])
        .status
        .success()
}

/// The one block of `text` naming `named`, its lines.
fn block_named(text: &str, named: (&str, &str, &str)) -> String {
    let (_, chunked) = chunks(text);
    let found: Vec<&String> = chunked
        .iter()
        .filter(|chunk| {
            let lines: Vec<&str> = chunk.lines().collect();
            lines.contains(&format!("node = \"{}\"", named.0).as_str())
                && lines.contains(&format!("relation = \"{}\"", named.1).as_str())
                && lines.contains(&format!("symbol = \"{}\"", named.2).as_str())
        })
        .collect();
    assert_eq!(found.len(), 1, "one block {named:?} in:\n{text}");
    found[0].clone()
}

/// A format-2 block with its code lines (`levels` through the last code
/// key before `file_blob`) replaced by `code`'s.
fn with_code(block: &str, code: &lock_common::Code) -> String {
    let lines: Vec<&str> = block.lines().collect();
    let from = lines
        .iter()
        .position(|line| line.starts_with("levels = "))
        .expect("a levels line");
    let to = lines
        .iter()
        .position(|line| line.starts_with("file_blob = "))
        .expect("a file_blob line");
    let mut out: String = lines[..from]
        .iter()
        .map(|line| format!("{line}\n"))
        .collect();
    out.push_str(&code.lines());
    for line in &lines[to..] {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// The verify rows of `run`, each with `lock.commit` nulled (it is the
/// one field a rebase keeps from format 1 that accept's golden does not
/// hold: `spec-verify-output.md` "JSON").
fn rows_without_lock_commit(run: &Verify) -> Vec<Value> {
    run.rows()
        .into_iter()
        .map(|row| {
            let mut row = row.clone();
            if let Some(lock) = row.get_mut("lock").and_then(Value::as_object_mut) {
                lock.insert("commit".to_owned(), Value::Null);
            }
            row
        })
        .collect()
}

/// The `cannot` lines of a verify run.
fn causes(run: &Verify) -> Vec<String> {
    run.json["cannot"]
        .as_array()
        .expect("`cannot` is an array")
        .iter()
        .map(|cause| cause.as_str().expect("a cause").to_owned())
        .collect()
}

/// The `notes` of a verify run.
fn notes(run: &Verify) -> Vec<String> {
    run.json["notes"]
        .as_array()
        .expect("`notes` is an array")
        .iter()
        .map(|note| note.as_str().expect("a note").to_owned())
        .collect()
}

// ------------------------------------------------------------------ RD-2

/// RD-2, "Fixtures" (Q30, R12): the rebased golden is `code-a.lock` with
/// each entry's `commit` taken from the same key in `code-a.format-1.lock`
/// (derived, never hand-written; a regenerated `code-a.lock` cannot leave
/// it stale). The derivation checked by hand: 9 keys kept, each holding
/// the same `rev`, `spec_hash`, `file_blob`, `verified_by`, `verified_at`
/// in both goldens; the format-1 golden's other 2 keys the `bad_levels`
/// drops; exactly 5 lines differ from `code-a.lock`, all `commit` lines
/// (`GOAL-01` `79ea966e…` -> `85a2c984…`, `LAW-BOUND`'s 4 `20825b3f…` ->
/// `82f6593a…`; `PIECE-COUNTER`'s 4 keep `2ba8a3bc…`).
/// `SPECENGINE_WRITE_REBASED_GOLDEN=1` writes the file.
#[test]
fn rebased_golden_is_accepts_with_format_1_commits() {
    let format_2 = golden_text();
    let format_1 = golden_format_1_text();
    let derived = derive_rebased_golden(&format_2, &format_1);

    let kept = blocks(&format_2);
    let old = blocks(&format_1);
    assert_eq!((kept.len(), old.len()), (9, 11));
    for block in &kept {
        let key = key_of(block);
        let at = old
            .iter()
            .find(|other| key_of(other) == key)
            .unwrap_or_else(|| panic!("{key:?}: no format-1 entry"));
        for field in [
            "rev",
            "spec_hash",
            "file_blob",
            "verified_by",
            "verified_at",
        ] {
            assert_eq!(value(block, field), value(at, field), "{key:?}: {field}");
        }
    }
    let gone: Vec<(String, String, String)> = old
        .iter()
        .filter(|block| !kept.iter().any(|other| key_of(other) == key_of(block)))
        .map(|block| {
            let (node, relation, _, _, symbol, _) = key_of(block);
            (node, relation, symbol)
        })
        .collect();
    let drops: Vec<(String, String, String)> = GOLDEN_DROPS
        .iter()
        .map(|(node, relation, symbol)| {
            (
                (*node).to_owned(),
                (*relation).to_owned(),
                (*symbol).to_owned(),
            )
        })
        .collect();
    assert_eq!(gone, drops, "the two `bad_levels` drops");

    assert_eq!(derived.lines().count(), format_2.lines().count());
    let differing: Vec<(&str, &str)> = format_2
        .lines()
        .zip(derived.lines())
        .filter(|(from, to)| from != to)
        .collect();
    let commit_line = |oid: &str| format!("commit = \"{oid}\"");
    let goal = (commit_line(GOAL_COMMITS.0), commit_line(GOAL_COMMITS.1));
    let bound = (commit_line(BOUND_COMMITS.0), commit_line(BOUND_COMMITS.1));
    assert_eq!(differing.len(), 5, "{differing:#?}");
    assert_eq!(
        differing
            .iter()
            .filter(|(from, to)| (*from, *to) == (goal.0.as_str(), goal.1.as_str()))
            .count(),
        1,
        "{differing:#?}"
    );
    assert_eq!(
        differing
            .iter()
            .filter(|(from, to)| (*from, *to) == (bound.0.as_str(), bound.1.as_str()))
            .count(),
        4,
        "{differing:#?}"
    );
    assert_eq!(
        derived.matches(&commit_line(PIECE_COMMIT)).count(),
        4,
        "PIECE-COUNTER's 4 keep theirs"
    );
    assert!(derived.ends_with('\n') && !derived.contains('\r'));

    if std::env::var_os(WRITE_REBASED_GOLDEN).is_some_and(|value| value == "1") {
        fs::write(rebased_golden_file(), &derived)
            .unwrap_or_else(|error| panic!("write fixtures/spec-lock/{REBASED_GOLDEN}: {error}"));
        return;
    }
    assert_eq!(
        rebased_golden_text(),
        derived,
        "the pinned rebased golden (rewrite it with {WRITE_REBASED_GOLDEN}=1)"
    );
}

/// RD-2 **M2** (an `Invalid` list taken as `sig, body`): the format-1
/// golden committed on the fixture's commit and rebased gives the rebased
/// golden byte for byte; 9 rows `rebased`, 2 `dropped bad_levels`
/// (`counter.rs:77` `[path, path]`, `bounds.rs:14` `[Sig]`), in key
/// order; the question names both formats and recipes, both counts; one
/// commit of the lock alone, its message the rebase's; no note (the
/// golden is at this build's v2 `code_recipe`: cross-checked).
#[test]
fn rebase_of_the_format_1_golden_is_the_rebased_golden() {
    let repo = LockRepo::format_1_golden("lr-golden");
    let head = repo.head();
    let attempt = repo.rebase(true);
    let outcome = attempt.committed("the format-1 golden");
    assert_eq!(repo.lock_text(), rebased_golden_text(), "the lock's bytes");
    assert_eq!(attempt.rows(), golden_rows(), "the rows");
    assert_eq!(
        attempt.question(),
        question(1, CODE_RECIPE_FORMAT_1, 9, 2, &head, &repo.root),
        "the question"
    );
    assert!(attempt.notes().is_empty(), "{:?}", attempt.notes());
    let commit = outcome.commit.clone().expect("a commit");
    assert_eq!(repo.head(), commit);
    assert_eq!(repo.parents(&commit), [head]);
    assert_eq!(repo.changed(&commit), ["spec.lock"]);
    assert_eq!(raw_message(&repo, &commit), rebase_message());
    assert_eq!(
        repo.show_blob(&commit, "spec.lock"),
        rebased_golden_text().into_bytes()
    );
    assert!(repo.porcelain().is_empty(), "clean after the commit");
}

/// RD-2 **M2**: a format-1 entry whose key a marker with an `Invalid`
/// level list gives is dropped `bad_levels`, never given the default (Q15,
/// accept's rule): the golden's two, and `stops_at_cap`'s list made
/// invalid four ways in a blob of its own (unclosed, an unknown name, a
/// duplicate, empty), and a key two markers give, the first `[Sig]`.
/// Each entry's `ast_hash` is its blob's v2 digest, so nothing but the
/// levels drops it.
#[test]
fn rebase_drops_invalid_level_entries() {
    let repo = LockRepo::format_1_golden("lr-invalid-golden");
    let attempt = repo.rebase(true);
    attempt.committed("the golden");
    let rows = attempt.rows();
    let lock = repo.lock_text();
    for (node, relation, symbol) in GOLDEN_DROPS {
        let dropped_row = rows
            .lines()
            .find(|line| {
                line.contains(&format!(" | {node} | {relation} | ")) && line.contains(symbol)
            })
            .unwrap_or_else(|| panic!("a row for {node} {symbol}:\n{rows}"));
        assert!(
            dropped_row.starts_with("dropped | ") && dropped_row.ends_with(" | bad_levels"),
            "{dropped_row}"
        );
        assert!(
            !lock.contains(&format!("symbol = \"{symbol}\"")),
            "{symbol} locked:\n{lock}"
        );
    }

    let source = fs::read_to_string(fixture("code-a").join(BOUNDS)).expect("bounds.rs");
    const MARKER: &str = "// @verifies LAW-BOUND@2 [body] mutation: drop the cap";
    let two = format!("// @verifies LAW-BOUND@2 [Sig]\n{MARKER}");
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
        ("two", two.as_str()),
    ] {
        let bytes = source.replacen(MARKER, marker, 1);
        assert_ne!(bytes, source, "{label}: the scenario");
        let repo = LockRepo::code_a(&format!("lr-invalid-{label}"));
        let blob = repo.hash_object(bytes.as_bytes());
        let line = bytes
            .lines()
            .position(|line| line == "fn stops_at_cap() {")
            .expect("stops_at_cap")
            + 1;
        let ast = item_hash(bytes.as_bytes(), line, "function_item");
        let text = format_1_with(STOPS, |block| {
            set_string(&set_string(block, "file_blob", &blob), "ast_hash", &ast)
        });
        repo.write("spec.lock", &text);
        repo.commit_all("the lock");
        let attempt = repo.rebase(true);
        attempt.committed(label);
        assert_eq!(
            attempt.rows(),
            golden_rows_dropping(&[dropped(STOPS, BOUNDS, STOPS_WHAT, "bad_levels")]),
            "{label}"
        );
        assert_eq!(
            repo.lock_text(),
            without_block(&rebased_golden_text(), STOPS),
            "{label}"
        );
    }
}

// ------------------------------------------------------------------ RD-1

/// RD-1 **M1** (an entry read from `HEAD`'s blob at its `file`, not its
/// `file_blob`): a later commit edits every file the format-1 golden's
/// entries name (the cap, a body, a test, the RON value, an attribute);
/// the rebase still gives the rebased golden byte for byte, each digest
/// its own blob's.
#[test]
fn rebase_uses_each_entrys_own_blob() {
    let repo = LockRepo::format_1_golden("lr-own-blob");
    repo.replace(COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
    repo.replace(
        COUNTER,
        "    pub fn value(&self) -> u32 {\n        self.count\n",
        "    pub fn value(&self) -> u32 {\n        self.count.min(CAP)\n",
    );
    repo.replace(BOUNDS, "for _ in 0..11 {", "for _ in 0..13 {");
    repo.replace(LIMITS, "cap: 10,", "cap: 12,");
    repo.replace(LIB, "#![forbid(unsafe_code)]", "#![deny(unsafe_code)]");
    repo.commit_all("a later edit");
    let head = repo.head();
    let lock = repo.lock_text();
    for file in [COUNTER, BOUNDS, LIMITS, LIB] {
        let at_head = repo.blob_at(&head, file);
        for block in blocks(&lock)
            .iter()
            .filter(|block| value(block, "file") == Some(file))
        {
            assert_ne!(
                value(block, "file_blob"),
                Some(at_head.as_str()),
                "the scenario: {file} differs at HEAD"
            );
        }
    }
    let attempt = repo.rebase(true);
    attempt.committed("after a later edit");
    assert_eq!(repo.lock_text(), rebased_golden_text());
    assert_eq!(attempt.rows(), golden_rows());
}

/// RD-1: code dirty in the worktree (modified, staged, untracked,
/// deleted) is ignored: no dirty input, the rebased golden, and the dirty
/// code left as it was, a staged path still staged (the commit takes the
/// lock alone).
#[test]
fn rebase_ignores_uncommitted_code() {
    let repo = LockRepo::format_1_golden("lr-uncommitted");
    repo.replace(COUNTER, "CAP: u32 = 10;", "CAP: u32 = 11;");
    repo.replace(BOUNDS, "for _ in 0..11 {", "for _ in 0..12 {");
    repo.git(&["add", "--", BOUNDS]);
    repo.write(
        "crates/tally/src/fresh.rs",
        "// @implements GOAL-01\npub fn fresh() {}\n",
    );
    fs::remove_file(repo.root.join(LIMITS)).expect("remove limits.ron");
    let status = repo.porcelain();
    assert!(!status.is_empty(), "the scenario: dirty code");
    let counter = repo.read(COUNTER);
    let attempt = repo.rebase(true);
    let commit = attempt
        .committed("dirty code")
        .commit
        .clone()
        .expect("a commit");
    assert_eq!(repo.lock_text(), rebased_golden_text());
    assert_eq!(repo.changed(&commit), ["spec.lock"]);
    assert_eq!(repo.porcelain(), status, "the dirty code as it was");
    assert_eq!(repo.read(COUNTER), counter);
    assert!(!repo.root.join(LIMITS).exists());
}

/// RD-1: `verified_at` and `commit` are kept byte for byte, an old date
/// and a decayed `commit` (no object of that OID) too: a rebase never
/// reads `commit`.
#[test]
fn rebase_keeps_verified_at_and_commit() {
    let decayed = fake_oid('e');
    let edit = |text: &str| {
        let text = edit_block(text, CAP, |block| {
            set_string(
                &set_string(block, "verified_at", "2019-02-03"),
                "commit",
                &decayed,
            )
        });
        edit_block(&text, LIB_MODULE, |block| {
            set_string(block, "verified_at", "2020-12-31")
        })
    };
    let repo = LockRepo::with_lock("lr-keeps", &edit(&golden_format_1_text()));
    assert!(!has_object(&repo, &decayed), "the scenario: decayed");
    let attempt = repo.rebase(true);
    attempt.committed("an old date, a decayed commit");
    assert_eq!(attempt.rows(), golden_rows());
    assert_eq!(repo.lock_text(), edit(&rebased_golden_text()));
}

// ------------------------------------------------------------------ RD-3

/// RD-3 **M3** (a missing blob exits 2 instead of dropping), TE-4: an
/// entry whose `file_blob` no object has is dropped `blob_missing` (a
/// rebase never guesses); the others are rebased; after the commit verify
/// shows the binding `unlocked`.
#[test]
fn missing_blob_drops_its_entry() {
    let missing = fake_oid('d');
    let repo = LockRepo::with_lock(
        "lr-missing",
        &format_1_with(CAP, |block| set_string(block, "file_blob", &missing)),
    );
    assert!(!has_object(&repo, &missing), "the scenario: no such object");
    let head = repo.head();
    let attempt = repo.rebase(true);
    attempt.committed("a missing blob");
    assert_eq!(
        attempt.rows(),
        golden_rows_dropping(&[dropped(CAP, COUNTER, CAP_WHAT, "blob_missing")])
    );
    assert_eq!(
        attempt.question(),
        question(1, CODE_RECIPE_FORMAT_1, 8, 3, &head, &repo.root)
    );
    assert_eq!(repo.lock_text(), without_block(&rebased_golden_text(), CAP));

    // TE-4: the binding is `unlocked`.
    let run = verify(&repo, &[]);
    let row = run.row(
        "LAW-BOUND",
        "configures",
        COUNTER,
        Some("crates/tally::counter::CAP"),
    );
    assert_eq!(row["state"], "unlocked", "{row}");
    assert!(row["lock"].is_null(), "{row}");
}

/// RD-3 (Q40): an OID naming an object of another type (a commit, a tree)
/// is dropped `blob_missing`, as a missing one.
#[test]
fn foreign_object_drops_as_blob_missing() {
    let repo = LockRepo::code_a("lr-foreign");
    let commit = repo.head();
    let tree = repo.git_text(&["rev-parse", "HEAD^{tree}"]);
    let text = edit_block(
        &format_1_with(CAP, |block| set_string(block, "file_blob", &commit)),
        RON_CAP,
        |block| set_string(block, "file_blob", &tree),
    );
    repo.write("spec.lock", &text);
    repo.commit_all("the lock");
    let attempt = repo.rebase(true);
    attempt.committed("foreign objects");
    assert_eq!(
        attempt.rows(),
        golden_rows_dropping(&[
            dropped(CAP, COUNTER, CAP_WHAT, "blob_missing"),
            dropped(RON_CAP, LIMITS, "ron root.cap", "blob_missing"),
        ])
    );
    assert_eq!(
        repo.lock_text(),
        without_block(&without_block(&rebased_golden_text(), CAP), RON_CAP)
    );
}

/// RD-3 **M4** (the v2 cross-check skipped): a format-1 `item` entry at
/// this build's v2 `code_recipe` whose `ast_hash` is another well-formed
/// digest (`Tally`'s) is dropped `hash_mismatch`; no note.
#[test]
fn v2_mismatch_drops_the_entry() {
    let tally = blocks(&golden_format_1_text())
        .iter()
        .find(|block| value(block, "symbol") == Some("crates/tally::counter::Tally"))
        .and_then(|block| value(block, "ast_hash").map(str::to_owned))
        .expect("Tally's ast_hash");
    let text = format_1_with(CAP, |block| set_string(block, "ast_hash", &tally));
    let repo = LockRepo::with_lock("lr-mismatch", &text);
    let attempt = repo.rebase(true);
    attempt.committed("a mismatching ast_hash");
    assert_eq!(
        attempt.rows(),
        golden_rows_dropping(&[dropped(CAP, COUNTER, CAP_WHAT, "hash_mismatch")])
    );
    assert_eq!(repo.lock_text(), without_block(&rebased_golden_text(), CAP));
    assert!(attempt.notes().is_empty(), "{:?}", attempt.notes());
}

/// RD-3 (Q16, R11): a format-1 lock of another `code_recipe` cannot be
/// cross-checked: the same mismatching `ast_hash` is located and rebased,
/// with one note naming the old recipe; the lock is the rebased golden.
#[test]
fn another_recipe_is_located_without_a_cross_check() {
    let old = "specengine-hash/v2 tree-sitter-rust/0.24.1 abi/15";
    let tally = blocks(&golden_format_1_text())
        .iter()
        .find(|block| value(block, "symbol") == Some("crates/tally::counter::Tally"))
        .and_then(|block| value(block, "ast_hash").map(str::to_owned))
        .expect("Tally's ast_hash");
    let text = format_1_with(CAP, |block| set_string(block, "ast_hash", &tally)).replacen(
        CODE_RECIPE_FORMAT_1,
        old,
        1,
    );
    let repo = LockRepo::with_lock("lr-other-v2", &text);
    let head = repo.head();
    let attempt = repo.rebase(true);
    attempt.committed("another v2 recipe");
    assert_eq!(attempt.rows(), golden_rows());
    assert_eq!(
        attempt.question(),
        question(1, old, 9, 2, &head, &repo.root)
    );
    assert_eq!(attempt.notes(), [not_this_builds_note(old)]);
    assert_eq!(repo.lock_text(), rebased_golden_text());
}

/// RD-3: an entry whose blob is not UTF-8 is dropped `not_utf8`.
#[test]
fn not_utf8_blob_drops_its_entry() {
    let repo = LockRepo::code_a("lr-not-utf8");
    let mut bytes = fs::read(fixture("code-a").join(COUNTER)).expect("counter.rs");
    bytes.extend_from_slice(b"// \xff\n");
    let blob = repo.hash_object(&bytes);
    repo.write(
        "spec.lock",
        format_1_with(CAP, |block| set_string(block, "file_blob", &blob)),
    );
    repo.commit_all("the lock");
    let attempt = repo.rebase(true);
    attempt.committed("a blob not UTF-8");
    assert_eq!(
        attempt.rows(),
        golden_rows_dropping(&[dropped(CAP, COUNTER, CAP_WHAT, "not_utf8")])
    );
    assert_eq!(repo.lock_text(), without_block(&rebased_golden_text(), CAP));
}

/// RD-3: an entry whose symbol its blob lacks (no candidate) is dropped
/// `symbol_gone`, its row naming the entry's own key.
#[test]
fn gone_symbol_drops_its_entry() {
    let gone = "crates/tally::counter::LIMIT";
    let text = format_1_with(CAP, |block| set_string(block, "symbol", gone));
    let repo = LockRepo::with_lock("lr-gone", &text);
    let attempt = repo.rebase(true);
    attempt.committed("a gone symbol");
    assert_eq!(
        attempt.rows(),
        golden_rows_with(&[(
            "LAW-BOUND",
            CAP_WHAT,
            row(
                "dropped",
                "LAW-BOUND",
                "configures",
                COUNTER,
                &format!("item {gone} const_item"),
                Some("symbol_gone"),
            ),
        )])
    );
    assert_eq!(repo.lock_text(), without_block(&rebased_golden_text(), CAP));
}

/// RD-3, "Ties" (Q37): the file's module is always a candidate, an inline
/// `mod` by its in-file path; of several, those a blob marker binds (node
/// and relation) remain. A file whose module and an inline `mod` are both
/// marked for one node and relation (accept locks both) drops the `mod`'s
/// entry `ambiguous`, the module's rebased; in `a.rs` holding `mod a`
/// both are `ambiguous` (never "longest match").
#[test]
fn module_and_inline_mod_both_marked_are_ambiguous() {
    // counter.rs and its `mod inner`.
    let repo = LockRepo::code_a("lr-ambiguous-inner");
    repo.replace(
        COUNTER,
        "//! @implements PIECE-COUNTER@2\n",
        "//! @implements PIECE-COUNTER@3\n",
    );
    repo.replace(
        COUNTER,
        "    //! @implements LAW-STEP@2\n",
        "    //! @implements LAW-STEP@2\n    //! @implements PIECE-COUNTER@3\n",
    );
    repo.commit_all("a module and its inline mod marked");
    repo.accept_ok("PIECE-COUNTER");
    let accepted = repo.lock_text();
    let inner: Named = (
        "PIECE-COUNTER",
        "implements",
        "crates/tally::counter::inner",
    );
    let module: Named = ("PIECE-COUNTER", "implements", "crates/tally::counter");
    block_named(&accepted, inner);
    block_named(&accepted, module);
    repo.write("spec.lock", Variant::OtherCode.text(&accepted));
    repo.commit_all("another grammar");
    let attempt = repo.rebase(true);
    attempt.committed("module and inline mod");
    let rows = attempt.rows();
    for expected in [
        row(
            "dropped",
            inner.0,
            inner.1,
            COUNTER,
            "module crates/tally::counter::inner",
            Some("ambiguous"),
        ),
        row(
            "rebased",
            module.0,
            module.1,
            COUNTER,
            "module crates/tally::counter",
            None,
        ),
    ] {
        assert!(rows.contains(&expected), "{expected:?} not in:\n{rows}");
    }
    assert_eq!(repo.lock_text(), without_block(&accepted, inner));

    // a.rs holding `mod a`: the module's symbol ends with the mod's path.
    let repo = LockRepo::code_a("lr-ambiguous-a");
    repo.write(
        "crates/tally/src/a.rs",
        "//! @implements PIECE-COUNTER@3\npub mod a {\n    //! @implements PIECE-COUNTER@3\n    pub fn one() -> u32 {\n        1\n    }\n}\n",
    );
    repo.replace(LIB, "pub mod counter;\n", "pub mod counter;\npub mod a;\n");
    repo.commit_all("a.rs holding mod a");
    repo.accept_ok("PIECE-COUNTER");
    let accepted = repo.lock_text();
    let file: Named = ("PIECE-COUNTER", "implements", "crates/tally::a");
    let nested: Named = ("PIECE-COUNTER", "implements", "crates/tally::a::a");
    block_named(&accepted, file);
    block_named(&accepted, nested);
    repo.write("spec.lock", Variant::OtherCode.text(&accepted));
    repo.commit_all("another grammar");
    let attempt = repo.rebase(true);
    attempt.committed("a.rs");
    let rows = attempt.rows();
    for what in ["module crates/tally::a", "module crates/tally::a::a"] {
        let expected = row(
            "dropped",
            "PIECE-COUNTER",
            "implements",
            "crates/tally/src/a.rs",
            what,
            Some("ambiguous"),
        );
        assert!(rows.contains(&expected), "{expected:?} not in:\n{rows}");
    }
    assert_eq!(
        repo.lock_text(),
        without_block(&without_block(&accepted, file), nested)
    );
}

/// RD-3, "Ties" (Q37): two `function_item` candidates for the impl's
/// `fmt` (a free `fn fmt` added): the blob's marker giving the key
/// (`PIECE-COUNTER#LAW-BOUND@2`, resolved to `LAW-BOUND`, `implements`)
/// leaves one, rebased with its blob's digests; a key no marker gives
/// (`configures` of the same symbol) leaves none: `ambiguous`.
#[test]
fn ties_are_broken_by_the_blobs_markers() {
    let repo = LockRepo::code_a("lr-ties");
    repo.append(COUNTER, "\npub fn fmt() {}\n");
    repo.commit_all("a second fmt");
    let head = repo.head();
    let source = repo.show_blob(&head, COUNTER);
    let blob = repo.blob_at(&head, COUNTER);
    let ast = item_hash(&source, 49, "function_item");
    let base = format_1_with(FMT, |block| {
        set_string(&set_string(block, "file_blob", &blob), "ast_hash", &ast)
    });
    let fmt_block = blocks(&base)
        .into_iter()
        .find(|block| value(block, "symbol") == Some(FMT.2))
        .expect("the fmt block");
    let configures = block_format_1(
        "LAW-BOUND",
        2,
        value(&fmt_block, "spec_hash").expect("spec_hash"),
        "configures",
        COUNTER,
        "item",
        FMT.2,
        Some(("function_item", &ast)),
        &blob,
        value(&fmt_block, "commit").expect("commit"),
        DAY,
    );
    repo.write("spec.lock", format!("{base}\n[[binding]]\n{configures}"));
    repo.commit_all("the lock");
    let attempt = repo.rebase(true);
    attempt.committed("ties");
    let expected_rows: String = golden_rows()
        .lines()
        .flat_map(|line| {
            let mut out = Vec::new();
            if line.ends_with(&format!(" | {CAP_WHAT}")) {
                out.push(row(
                    "dropped",
                    "LAW-BOUND",
                    "configures",
                    COUNTER,
                    FMT_WHAT,
                    Some("ambiguous"),
                ));
            }
            out.push(format!("{line}\n"));
            out
        })
        .collect();
    assert_eq!(attempt.rows(), expected_rows);
    let code = item_code(&source, 49, "function_item", &["sig", "body"]);
    assert_eq!(
        repo.lock_text(),
        edit_block(&rebased_golden_text(), FMT, |block| {
            set_string(&with_code(block, &code), "file_blob", &blob)
        })
    );
}

/// RD-3: a located unit giving `CannotVerify` is dropped `cannot_verify
/// <category>`: a module whose blob has a parse error (its category the
/// first `module_levels` gives, as accept's row), the RON values of a blob
/// the RON analysis rejects (`trailing_content`).
#[test]
fn cannot_verify_unit_drops_its_entry() {
    let repo = LockRepo::code_a("lr-cannot-verify");
    let broken = format!(
        "{}\nfn broken( {{\n",
        fs::read_to_string(fixture("code-a").join(BOUNDS)).expect("bounds.rs")
    );
    let category = {
        let mut parser = RustParser::new().expect("the Rust grammar");
        let tree = parser.parse(&broken).expect("a tree");
        match module_levels(&tree, &broken) {
            LevelState::CannotVerify { categories } => categories
                .first()
                .map(|category| category.as_str().to_owned())
                .expect("a category"),
            LevelState::Hashed(_) => panic!("the scenario: the module cannot be hashed"),
        }
    };
    let module_blob = repo.hash_object(broken.as_bytes());
    let ron = format!(
        "{}extra\n",
        fs::read_to_string(fixture("code-a").join(LIMITS)).expect("limits.ron")
    );
    let ron_blob = repo.hash_object(ron.as_bytes());
    let mut text = format_1_with(BOUNDS_MODULE, |block| {
        set_string(block, "file_blob", &module_blob)
    });
    for named in [RON_CAP, RON_ROOT] {
        text = edit_block(&text, named, |block| {
            set_string(block, "file_blob", &ron_blob)
        });
    }
    repo.write("spec.lock", &text);
    repo.commit_all("the lock");
    let attempt = repo.rebase(true);
    attempt.committed("cannot verify");
    let module_reason = format!("cannot_verify {category}");
    assert_eq!(
        attempt.rows(),
        golden_rows_dropping(&[
            dropped(
                BOUNDS_MODULE,
                BOUNDS,
                "module crates/tally::test:bounds",
                &module_reason
            ),
            dropped(
                RON_CAP,
                LIMITS,
                "ron root.cap",
                "cannot_verify trailing_content"
            ),
            dropped(
                RON_ROOT,
                LIMITS,
                "ron root",
                "cannot_verify trailing_content"
            ),
        ])
    );
    let mut expected = rebased_golden_text();
    for named in [BOUNDS_MODULE, RON_CAP, RON_ROOT] {
        expected = without_block(&expected, named);
    }
    assert_eq!(repo.lock_text(), expected);
}

// ---------------------------------------------------------- RD-3: levels

/// "Levels" (Q4): a format-2 entry's levels are kept as written (markers
/// only break ties): `CAP`'s made `["deps"]`, `GOAL-01`'s `["sig",
/// "body"]` (its marker says `[body]`), under another grammar's recipe:
/// the rebase keeps both lists, its digests this build's.
#[test]
fn format_2_levels_are_kept_as_written() {
    let edit = |text: &str| {
        let text = edit_block(text, CAP, |block| set_line(block, "levels", "[\"deps\"]"));
        edit_block(&text, VALUE, |block| {
            set_line(block, "levels", "[\"sig\", \"body\"]")
        })
    };
    let written = edit(&golden_text());
    let repo = LockRepo::with_lock("lr-format-2-levels", &Variant::OtherCode.text(&written));
    repo.rebase(true).committed("format 2, levels as written");
    assert_eq!(repo.lock_text(), written);
}

/// "Levels" (Q4): a format-1 entry no blob marker gives (only an alias
/// gone from the corpus: `AIM` no longer `GOAL`'s) takes `sig, body`, what
/// format 1's `ast_hash` compared; located without markers (one
/// candidate).
#[test]
fn format_1_entry_no_marker_gives_takes_sig_body() {
    let repo = LockRepo::format_1_golden("lr-alias-gone");
    repo.replace("specengine.toml", ", aliases_from = [\"AIM\"] }", " }");
    repo.commit_all("AIM no longer GOAL's alias");
    let attempt = repo.rebase(true);
    attempt.committed("an alias gone");
    assert_eq!(attempt.rows(), golden_rows());
    assert_eq!(
        repo.lock_text(),
        edit_block(&rebased_golden_text(), VALUE, |block| {
            set_line(block, "levels", "[\"sig\", \"body\"]")
        })
    );
}

// ------------------------------------------------------------------ RD-4

/// RD-4 (step 1): with stdin no terminal, `spec lock rebase` refuses with
/// the terminal's message whatever the project, `--json` too: exit 2,
/// stdout empty, nothing read (no data directory made), nothing changed.
#[test]
fn rebase_off_a_terminal_reads_nothing() {
    let repo = LockRepo::format_1_golden("lr-terminal");
    let outside = repo.scratch.dir("outside");
    for (cwd, args) in [
        (&repo.root, vec!["lock", "rebase"]),
        (&repo.root, vec!["lock", "rebase", "--json"]),
        (&outside, vec!["lock", "rebase"]),
    ] {
        let before = repo.state();
        let run = repo.spec_piped(cwd, &args);
        run.code(2);
        assert!(run.stdout.is_empty(), "{}", run.show());
        assert!(
            run.stderr.contains("`spec lock rebase`") && run.stderr.contains("terminal"),
            "{args:?}: {}",
            run.show()
        );
        assert_eq!(repo.state(), before, "{args:?}");
        assert!(
            common::snapshot(&repo.home).is_empty(),
            "{args:?}: the data directory was made"
        );
    }
}

/// RD-4 (step 4, Q32): a lock of another `norm_recipe` exits 2 with the
/// `Some` reader's text verbatim (no rebase crosses it), format 2 and
/// format 1 alike; nothing changed.
#[test]
fn other_norm_recipe_exits_2() {
    let repo = LockRepo::with_lock("lr-norm-2", &Variant::OtherNorm.text(&golden_text()));
    let message = cannot_quietly(&repo, "format 2, another norm");
    assert!(message.contains(&Variant::OtherNorm.message()), "{message}");
    let repo = LockRepo::with_lock(
        "lr-norm-1",
        &golden_format_1_text().replacen(NORM_RECIPE, Variant::OTHER_NORM, 1),
    );
    let message = cannot_quietly(&repo, "format 1, another norm");
    assert!(message.contains(&Variant::OtherNorm.message()), "{message}");
}

/// RD-4 (step 4): a lock `parse_lock` without the build cannot read exits
/// 2 with that error: `format = 3`, `format = 0`, not TOML.
#[test]
fn unreadable_and_other_format_locks_exit_2() {
    let cases = [
        (
            "format 3",
            Variant::Format3.text(&golden_text()),
            format!("spec.lock:2: {}", newer_format_text(3)),
        ),
        (
            "format 0",
            golden_format_1_text().replacen("\nformat = 1\n", "\nformat = 0\n", 1),
            format!("spec.lock:2: {}", older_format_text(0)),
        ),
        (
            "not TOML",
            format!("{}[[binding\n", golden_format_1_text()),
            "spec.lock:".to_owned(),
        ),
    ];
    for (label, text, want) in cases {
        let repo =
            LockRepo::with_lock(&format!("lr-unreadable-{}", label.replace(' ', "-")), &text);
        let message = cannot_quietly(&repo, label);
        assert!(message.contains(&want), "{label}: {message}");
    }
}

/// RD-4 (step 4): a lock that is not UTF-8 exits 2 with `read_lock`'s
/// text.
#[test]
fn not_utf8_lock_exits_2() {
    let repo = LockRepo::code_a("lr-lock-not-utf8");
    let mut bytes = golden_format_1_text().into_bytes();
    bytes.extend_from_slice(b"# \xff\n");
    repo.write("spec.lock", bytes);
    repo.commit_all("the lock");
    let message = cannot_quietly(&repo, "not UTF-8");
    assert!(message.contains(NOT_UTF8), "{message}");
}

/// RD-4 (step 2, R7, Q28): an operation in progress exits 2 with
/// `Repo::open`'s text: a merge stopped on another file, and an unmerged
/// `spec.lock` (a merge in progress, regen never named); no worktree, an
/// unborn and a detached `HEAD` exit 2.
#[test]
fn rebase_refuses_an_operation_in_progress() {
    // A merge stopped on a file no entry reads.
    let repo = LockRepo::format_1_golden("lr-merging");
    repo.git(&["switch", "-q", "-c", "side"]);
    repo.write("NOTES.txt", "side\n");
    repo.commit_all("side");
    repo.git(&["switch", "-q", "main"]);
    repo.write("NOTES.txt", "main\n");
    repo.commit_all("main");
    let merge = repo
        .git
        .git_output(&repo.root, &["merge", "-q", "side"], &[]);
    assert!(!merge.status.success(), "the merge conflicts");
    let message = cannot_quietly(&repo, "a merge in progress");
    assert!(message.contains(&merge_text(&repo.root)), "{message}");

    // An unmerged `spec.lock`.
    let repo = LockRepo::format_1_golden("lr-unmerged");
    repo.git(&["switch", "-q", "-c", "side"]);
    repo.write(
        "spec.lock",
        format_1_with(CAP, |block| set_string(block, "verified_at", "2026-10-09")),
    );
    repo.commit_all("side's lock");
    repo.git(&["switch", "-q", "main"]);
    repo.write(
        "spec.lock",
        format_1_with(CAP, |block| set_string(block, "verified_at", "2026-10-10")),
    );
    repo.commit_all("main's lock");
    let merge = repo
        .git
        .git_output(&repo.root, &["merge", "-q", "side"], &[]);
    assert!(!merge.status.success(), "the merge conflicts in spec.lock");
    assert!(
        !repo
            .git_text(&["ls-files", "-u", "--", "spec.lock"])
            .is_empty(),
        "the scenario: spec.lock unmerged"
    );
    let message = cannot_quietly(&repo, "spec.lock unmerged");
    assert!(message.contains(&merge_text(&repo.root)), "{message}");
    assert!(!message.contains("regen"), "{message}");

    // No worktree, unborn, detached.
    let repo = LockRepo::uncommitted("lr-nogit");
    repo.write("spec.lock", golden_format_1_text());
    let before = common::snapshot(&repo.root);
    repo.rebase(true).cannot("no worktree");
    assert_eq!(common::snapshot(&repo.root), before);
    let repo = LockRepo::uncommitted("lr-unborn");
    repo.git.init(&repo.root);
    repo.write("spec.lock", golden_format_1_text());
    cannot_quietly(&repo, "unborn HEAD");
    let repo = LockRepo::format_1_golden("lr-detached");
    repo.git(&["checkout", "-q", "--detach"]);
    cannot_quietly(&repo, "detached HEAD");
}

/// RD-4 (step 3): `spec.lock` and `specengine.toml` must be tracked and
/// equal to `HEAD`, each dirty one named with its state: exit 1 `…: commit
/// them first; nothing written`, nothing changed.
#[test]
fn dirty_inputs_refuse_the_rebase() {
    type Setup = fn(&LockRepo);
    let cases: [(&str, Setup, &str); 4] = [
        (
            "spec.lock modified",
            |repo| repo.append("spec.lock", "\n"),
            "`spec.lock` modified",
        ),
        (
            "spec.lock staged",
            |repo| {
                repo.append("spec.lock", "\n");
                repo.git(&["add", "--", "spec.lock"]);
            },
            "`spec.lock` staged",
        ),
        (
            "specengine.toml modified",
            |repo| repo.append("specengine.toml", "\n"),
            "`specengine.toml` modified",
        ),
        (
            "spec.lock untracked",
            |repo| {
                repo.git(&["rm", "-q", "--cached", "--", "spec.lock"]);
                repo.git.commit(&repo.root, "the lock untracked");
            },
            "`spec.lock` untracked",
        ),
    ];
    for (label, setup, named) in cases {
        let repo =
            LockRepo::format_1_golden(&format!("lr-dirty-{}", label.replace([' ', '.'], "-")));
        setup(&repo);
        let refusal = refused_quietly(&repo, label);
        assert!(refusal.contains(named), "{label}: {refusal}");
        assert!(
            refusal.contains("commit them first; nothing written"),
            "{label}: {refusal}"
        );
    }
}

/// RD-4 (step 3): no lock: exit 1 `no spec.lock: nothing to rebase`.
#[test]
fn no_lock_is_nothing_to_rebase() {
    let repo = LockRepo::code_a("lr-no-lock");
    let refusal = refused_quietly(&repo, "no lock");
    assert!(refusal.contains(NO_LOCK), "{refusal}");
}

/// RD-4 (step 4): a lock already of this build's format and recipes:
/// exit 0 `nothing to change`, no question, nothing written; on a
/// terminal too.
#[test]
fn a_lock_of_this_build_is_nothing_to_change() {
    let repo = LockRepo::with_lock("lr-nothing", &golden_text());
    let before = repo.state();
    repo.rebase(true).unchanged("this build's lock");
    assert_eq!(repo.state(), before);
    let run = repo.spec_pty(&repo.root, &["lock", "rebase"], None);
    run.code(0);
    assert!(!run.asked, "{}", run.show());
    assert_eq!(
        run.stdout,
        format!("{NOTHING_TO_CHANGE}\n"),
        "{}",
        run.show()
    );
    assert_eq!(repo.state(), before);
}

/// RD-4 (step 1): usage (an argument, `--yes`), discovery (no project
/// above the cwd), no `[code]` (`spec refs`' text), `--config` naming
/// another file: exit 2, no question, nothing changed.
#[test]
fn usage_discovery_and_config_errors_exit_2() {
    let repo = LockRepo::format_1_golden("lr-usage");
    let before = repo.state();
    for args in [
        vec!["lock", "rebase", "spec.lock"],
        vec!["lock", "rebase", "--yes"],
    ] {
        let run = repo.spec_pty(&repo.root, &args, None);
        run.code(2);
        assert!(!run.asked, "{}", run.show());
        assert!(!run.stderr.contains("stdin is not one"), "{}", run.show());
    }
    let outside = repo.scratch.dir("outside");
    repo.rebase_at(&outside, true).cannot("no project");
    fs::copy(
        repo.root.join("specengine.toml"),
        repo.scratch.join("other.toml"),
    )
    .expect("another config");
    let other = repo.scratch.join("other.toml");
    let run = repo.spec_pty(
        &repo.root,
        &["--config", other.to_str().expect("UTF-8"), "lock", "rebase"],
        None,
    );
    run.code(2);
    assert!(!run.asked, "{}", run.show());
    assert_eq!(repo.state(), before);

    let no_code = LockRepo::of("lr-no-code", "spec-c", "repo", &[]);
    no_code.write("spec.lock", golden_format_1_text());
    no_code.commit_all("a lock");
    assert_eq!(cannot_quietly(&no_code, "no [code]"), NO_CODE);
}

// ------------------------------------------------------------------ RD-5

/// RD-5: the rebase commit names no node: `spec: lock rebase`, an empty
/// line, `Lock-mode: rebase`, `Decided-by:`, no `Lock:`, no `Proposal:`.
#[test]
fn rebase_commit_names_no_node() {
    let repo = LockRepo::format_1_golden("lr-message");
    let commit = repo.rebase_ok().commit.expect("a commit");
    let message = raw_message(&repo, &commit);
    assert_eq!(message, rebase_message());
    for key in ["Lock:", "Proposal:"] {
        assert!(
            !message.lines().any(|line| line.starts_with(key)),
            "{key} in {message:?}"
        );
    }
}

/// RD-5 **M5** (check 3 passing a rebase commit without `Lock-mode:
/// rebase`): a `commit-msg` hook drops that trailer; git makes the commit,
/// the check fails: exit 1 naming the commit and the trailer, advice `git
/// reset --soft <commit>^`, the commit kept (unverified, never rewritten).
/// A hook adding a `Lock:` trailer fails it too ("and no `Lock:`").
#[test]
fn rebase_commit_without_its_mode_trailer_fails_a_check() {
    for (label, hook, named) in [
        (
            "mode-dropped",
            "grep -v '^Lock-mode: ' \"$1\" > \"$1.tmp\" && mv \"$1.tmp\" \"$1\"",
            "Lock-mode: rebase",
        ),
        ("lock-added", "printf 'Lock: GOAL-01\\n' >> \"$1\"", "Lock:"),
    ] {
        let repo = LockRepo::format_1_golden(&format!("lr-check-{label}"));
        repo.hook("commit-msg", hook);
        let head = repo.head();
        let attempt = repo.rebase(true);
        let refusal = attempt.refused(label);
        let after = repo.head();
        assert_ne!(after, head, "{label}: git made the commit");
        assert_eq!(repo.changed(&after), ["spec.lock"], "{label}");
        assert_eq!(repo.parents(&after), std::slice::from_ref(&head), "{label}");
        assert!(
            refusal.contains(&after),
            "{label}: names the commit: {refusal}"
        );
        assert!(refusal.contains(named), "{label}: names {named}: {refusal}");
        assert!(
            refusal.contains(&format!("git reset --soft {after}^")),
            "{label}: {refusal}"
        );
        let outcome = attempt.outcome.as_ref().expect("an outcome");
        assert_eq!(outcome.commit.as_deref(), Some(after.as_str()), "{label}");
        assert_eq!(attempt.prompts.len(), 1, "{label}");
    }
}

/// RD-5 (step 7): the lock's bytes or `HEAD` changed while the owner reads
/// the question: exit 1 `spec.lock changed while being rebased; nothing
/// written`, the lock as the change left it.
#[test]
fn a_change_during_the_question_is_refused() {
    let repo = LockRepo::format_1_golden("lr-changed-lock");
    let head = repo.head();
    let attempt = repo.rebase_during(&repo.root, true, &mut || {
        repo.append("spec.lock", "# a line\n");
    });
    let refusal = attempt.refused("the lock changed");
    assert!(refusal.contains(CHANGED_WHILE_REBASING), "{refusal}");
    assert_eq!(repo.head(), head);
    assert_eq!(
        repo.lock_text(),
        format!("{}# a line\n", golden_format_1_text())
    );

    let repo = LockRepo::format_1_golden("lr-changed-head");
    let attempt = repo.rebase_during(&repo.root, true, &mut || {
        repo.write("NOTES.txt", "meanwhile\n");
        repo.commit_all("meanwhile");
    });
    let refusal = attempt.refused("HEAD moved");
    assert!(refusal.contains(CHANGED_WHILE_REBASING), "{refusal}");
    assert_eq!(repo.lock_text(), golden_format_1_text());
    assert_eq!(repo.message(&repo.head()).trim_end(), "meanwhile");
}

/// "Rules" (R5): a rebase commit completes no `drift` item: an item raised
/// over accept's golden stays `open` after the rebase of the same lock
/// under another grammar's recipe; its approve then makes its own lock
/// commit (`Proposal:`, `Lock-mode: accept`) on top.
#[test]
fn a_rebase_commit_completes_no_drift_item() {
    let repo = drift_common::drifted("lr-drift", drift_common::cap_changed);
    let raised = drift_common::raised(&repo, &["LAW-BOUND"]);
    let created = raised.created();
    assert_eq!(created.len(), 1, "{created:?}");
    let id = created[0].0.clone();
    let lock = repo.lock_text();
    repo.write("spec.lock", Variant::OtherCode.text(&lock));
    repo.commit_all("another grammar");
    let rebase = repo.rebase_ok().commit.expect("a commit");
    assert_eq!(repo.lock_text(), lock, "rebased to accept's bytes");
    assert_eq!(
        drift_common::proposal(&repo, "code-a", &id).status,
        ProposalStatus::Open
    );
    assert!(!raw_message(&repo, &rebase).contains("Proposal:"));
    drift_common::approve_yes(&repo, &id).done("the drift approve");
    assert_eq!(
        drift_common::proposal(&repo, "code-a", &id).status,
        ProposalStatus::Applied
    );
    let head = repo.head();
    assert_ne!(head, rebase);
    assert_eq!(repo.parents(&head), [rebase]);
    let message = raw_message(&repo, &head);
    assert!(message.contains(&format!("Proposal: {id}")), "{message}");
    assert!(message.contains("Lock-mode: accept"), "{message}");
}

// ------------------------------------------------------------------ RD-6

/// The rows and counts of a `HEAD` holding `head` and a judged lock
/// `judged` (on disk; `staged`: in the index too) under `flag`.
fn base_run(label: &str, head: &str, judged: &str, flag: &str) -> (LockRepo, Verify) {
    let repo = LockRepo::with_lock(label, head);
    repo.write("spec.lock", judged);
    if flag == "--staged" {
        repo.git(&["add", "--", "spec.lock"]);
    }
    let run = verify(&repo, &[flag]);
    (repo, run)
}

/// RD-6 **M6** (`HEAD`'s old-format lock as cause (7)), "The base" (Q14,
/// Q33, Q34): under `--changed` (the rebased lock on disk) and `--staged`
/// (staged), a `HEAD` lock of format 1, or of another `code_recipe`, gives
/// way to the judged lock: no cause (7), every row as a `HEAD` holding the
/// judged lock gives it (every finding Pre-existing), one note on stderr
/// and in `notes`. Exit 2 throughout (code-a's Cannot rows).
#[test]
fn head_lock_of_another_format_is_judged_with_the_current_lock() {
    let golden = golden_text();
    let cases = [
        (
            "format-1",
            golden_format_1_text(),
            rebased_golden_text(),
            head_note(1, CODE_RECIPE_FORMAT_1),
        ),
        (
            "other-code",
            Variant::OtherCode.text(&golden),
            golden.clone(),
            head_note(2, Variant::OTHER_CODE),
        ),
    ];
    let mut failures = Vec::new();
    for (label, head, judged, note) in cases {
        for flag in ["--changed", "--staged"] {
            let context = format!("{label} {flag}");
            let (_, run) = base_run(
                &format!("lr-base-{label}-{}", flag.trim_start_matches('-')),
                &head,
                &judged,
                flag,
            );
            let (_, reference) = base_run(
                &format!("lr-base-ref-{label}-{}", flag.trim_start_matches('-')),
                &judged,
                &judged,
                flag,
            );
            let causes = causes(&run);
            if !causes.is_empty() {
                failures.push(format!("{context}: causes {causes:?}"));
                continue;
            }
            if run.rows().is_empty() {
                failures.push(format!("{context}: no row judged"));
                continue;
            }
            let unjudged: Vec<&Value> = run
                .rows()
                .into_iter()
                .filter(|row| row["state"] != "ok" && row["pre_existing"] != json!(true))
                .collect();
            if !unjudged.is_empty() {
                failures.push(format!("{context}: not Pre-existing: {unjudged:#?}"));
            }
            if run.rows() != reference.rows() {
                failures.push(format!(
                    "{context}: rows differ from a HEAD holding the judged lock"
                ));
            }
            if run.json["counts"] != reference.json["counts"] {
                failures.push(format!(
                    "{context}: counts {} vs {}",
                    run.json["counts"], reference.json["counts"]
                ));
            }
            if notes(&run) != [note.clone()] {
                failures.push(format!(
                    "{context}: notes {:?}, want [{note:?}]",
                    notes(&run)
                ));
            }
            if !run
                .text
                .stderr_lines()
                .contains(&format!("note: {note}").as_str())
            {
                failures.push(format!("{context}: stderr {:?}", run.text.stderr));
            }
            if run.code() != 2 || reference.code() != 2 {
                failures.push(format!(
                    "{context}: exit {} (reference {})",
                    run.code(),
                    reference.code()
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// "The base": a `HEAD` lock of another `norm_recipe`, or unreadable, is
/// still cause (7), its text as today (`HEAD's <error>`), no row judged;
/// a judged lock not this build's is cause (6) alone, as today.
#[test]
fn other_head_locks_keep_their_causes() {
    let golden = golden_text();
    for (label, head) in [
        ("other-norm", Variant::OtherNorm.text(&golden)),
        ("unreadable", format!("{golden}[[binding\n")),
    ] {
        for flag in ["--changed", "--staged"] {
            let (_, run) = base_run(
                &format!("lr-cause-{label}-{}", flag.trim_start_matches('-')),
                &head,
                &golden,
                flag,
            );
            let causes = causes(&run);
            assert_eq!(run.code(), 2, "{label} {flag}");
            assert!(run.rows().is_empty(), "{label} {flag}: rows judged");
            assert!(
                causes
                    .first()
                    .is_some_and(|cause| cause.starts_with("HEAD's spec.lock")),
                "{label} {flag}: {causes:?}"
            );
            if label == "other-norm" {
                assert!(
                    causes[0].contains(&Variant::OtherNorm.message()),
                    "{label} {flag}: {causes:?}"
                );
            }
        }
    }

    // The judged lock format 1 (not rebased), HEAD's too: cause (6) alone,
    // as today (decided 2026-10-20, owner pre-approved: verify reads no
    // `HEAD` after an earlier cause).
    let repo = LockRepo::format_1_golden("lr-cause-judged");
    let run = verify(&repo, &["--changed"]);
    let causes = causes(&run);
    assert_eq!(run.code(), 2);
    assert!(run.rows().is_empty(), "rows judged");
    assert_eq!(causes, [Variant::Format1.message()], "cause (6) alone");
}

/// "The base" (decided 2026-10-20, owner pre-approved): an absent judged
/// lock is an empty lock of this build's, as everywhere in verify, so
/// `HEAD`'s format-1 rows are judged against it: no cause (7), no row
/// holding a lock entry, every finding Pre-existing (each binding
/// `unlocked` at `HEAD` as on disk), under `--changed` and `--staged`.
#[test]
fn an_absent_judged_lock_judges_heads_rows_unlocked() {
    for flag in ["--changed", "--staged"] {
        let repo =
            LockRepo::format_1_golden(&format!("lr-absent-{}", flag.trim_start_matches('-')));
        fs::remove_file(repo.root.join("spec.lock")).expect("remove the lock");
        if flag == "--staged" {
            repo.git(&["rm", "-q", "--cached", "--", "spec.lock"]);
        }
        let run = verify(&repo, &[flag]);
        assert!(causes(&run).is_empty(), "{flag}: {:?}", causes(&run));
        assert!(!run.rows().is_empty(), "{flag}: no row judged");
        assert!(
            run.lock_rows().is_empty(),
            "{flag}: a row holds a lock entry"
        );
        let unlocked = run.rows_where(|row| row["state"] == "unlocked");
        assert!(!unlocked.is_empty(), "{flag}: no unlocked row");
        let not_pre: Vec<&Value> = run
            .rows()
            .into_iter()
            .filter(|row| row["state"] != "ok" && row["pre_existing"] != json!(true))
            .collect();
        assert!(not_pre.is_empty(), "{flag}: not Pre-existing: {not_pre:#?}");
    }
}

// ------------------------------------------------------------------ TE-1

/// TE-1, "Output": on a terminal the rows go to stdout in key order, the
/// question to stderr, `y` commits, stdout ends `committed <oid>`.
#[test]
fn rebase_on_a_terminal_prints_rows_and_commits() {
    let repo = LockRepo::format_1_golden("lr-pty");
    let head = repo.head();
    let run = repo.spec_pty(&repo.root, &["lock", "rebase"], Some("y"));
    run.code(0);
    assert!(run.asked, "{}", run.show());
    let commit = repo.head();
    assert_ne!(commit, head);
    assert_eq!(
        run.stdout,
        format!("{}committed {commit}\n", golden_rows()),
        "{}",
        run.show()
    );
    assert_eq!(
        run.stderr.trim_end(),
        question(1, CODE_RECIPE_FORMAT_1, 9, 2, &head, &repo.root),
        "{}",
        run.show()
    );
    assert_eq!(repo.lock_text(), rebased_golden_text());
}

/// The levels of `named`'s block in `text`, as JSON strings.
fn levels_of(text: &str, named: (&str, &str, &str)) -> Value {
    let block = block_named(text, named);
    let line = block
        .lines()
        .find(|line| line.starts_with("levels = "))
        .expect("a levels line");
    let inside = line.trim_start_matches("levels = [").trim_end_matches(']');
    Value::Array(
        inside
            .split(", ")
            .map(|level| Value::String(level.trim_matches('"').to_owned()))
            .collect(),
    )
}

/// TE-1 (`lock_rebase_json_keeps_its_keys`): `--json` prints one document
/// for exits 0 and 1, none for 2, every key present in order (`from`,
/// `to`, `counts`, `rows`, `commit`, `unchanged`, `refusal`), absent
/// `null`; each row `change`, `node`, `relation`, `file`, `kind`,
/// `symbol`, `item_kind`, `levels`, `reason` (a dropped row's `levels`
/// `null`, a rebased row's `reason` `null`).
#[test]
fn lock_rebase_json_keeps_its_keys() {
    const KEYS: [&str; 7] = [
        "from",
        "to",
        "counts",
        "rows",
        "commit",
        "unchanged",
        "refusal",
    ];
    const ROW_KEYS: [&str; 9] = [
        "change",
        "node",
        "relation",
        "file",
        "kind",
        "symbol",
        "item_kind",
        "levels",
        "reason",
    ];
    let keys_in_order = |document: &Value, printed: &str, context: &str| {
        let mut keys: Vec<&str> = document
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        let mut want = KEYS.to_vec();
        want.sort_unstable();
        assert_eq!(keys, want, "{context}: {document}");
        let at: Vec<usize> = KEYS
            .iter()
            .map(|key| printed.find(&format!("\"{key}\":")).expect(key))
            .collect();
        assert!(
            at.windows(2).all(|pair| pair[0] < pair[1]),
            "{context}: {printed}"
        );
    };

    // Exit 0, a commit.
    let repo = LockRepo::format_1_golden("lr-json");
    let run = repo.spec_pty(&repo.root, &["lock", "rebase", "--json"], Some("y"));
    run.code(0);
    assert!(run.asked, "{}", run.show());
    let document = common::one_json_document(&run.stdout)
        .unwrap_or_else(|problem| panic!("{problem}: {}", run.show()));
    keys_in_order(&document, &run.stdout, "exit 0");
    assert_eq!(
        document["from"],
        json!({"format": 1, "code_recipe": CODE_RECIPE_FORMAT_1})
    );
    assert_eq!(
        document["to"],
        json!({"format": 2, "code_recipe": CODE_RECIPE})
    );
    assert_eq!(document["counts"], json!({"rebased": 9, "dropped": 2}));
    assert_eq!(document["commit"], json!(repo.head()));
    assert_eq!(document["unchanged"], json!(false));
    assert_eq!(document["refusal"], Value::Null);
    let rows = document["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 11);
    // Each row's keys in order as printed (a parsed `Value` sorts them).
    let raw = run.stdout.trim_end();
    let from = raw.find("\"rows\":[").expect("the rows");
    let printed_rows: Vec<String> = raw[from..]
        .split("{\"change\":")
        .skip(1)
        .map(|piece| format!("{{\"change\":{piece}"))
        .collect();
    assert_eq!(printed_rows.len(), 11, "{raw}");
    let lock = repo.lock_text();
    for ((row, line), printed) in rows.iter().zip(golden_rows().lines()).zip(&printed_rows) {
        let at: Vec<usize> = ROW_KEYS
            .iter()
            .map(|key| printed.find(&format!("\"{key}\":")).expect(key))
            .collect();
        assert!(at.windows(2).all(|pair| pair[0] < pair[1]), "{printed}");
        assert_eq!(
            row.as_object().expect("a row").len(),
            ROW_KEYS.len(),
            "{printed}"
        );
        let fields: Vec<&str> = line.split(" | ").collect();
        assert_eq!(row["change"], fields[0], "{printed}");
        assert_eq!(row["node"], fields[1], "{printed}");
        assert_eq!(row["relation"], fields[2], "{printed}");
        assert_eq!(row["file"], fields[3], "{printed}");
        let what: Vec<&str> = fields[4].splitn(2, ' ').collect();
        assert_eq!(row["kind"], what[0], "{printed}");
        let symbol = row["symbol"].as_str().expect("a symbol");
        match row["item_kind"].as_str() {
            Some(item_kind) => assert_eq!(what[1], format!("{symbol} {item_kind}"), "{printed}"),
            None => assert_eq!(what[1], symbol, "{printed}"),
        }
        if fields[0] == "dropped" {
            assert_eq!(row["levels"], Value::Null, "{printed}");
            assert_eq!(row["reason"], fields[5], "{printed}");
        } else {
            assert_eq!(row["reason"], Value::Null, "{printed}");
            assert_eq!(
                row["levels"],
                levels_of(&lock, (fields[1], fields[2], symbol)),
                "{printed}"
            );
        }
    }

    // Exit 1: a dirty lock; one document, the refusal, `commit` null.
    let repo = LockRepo::format_1_golden("lr-json-refused");
    repo.append("spec.lock", "\n");
    let run = repo.spec_pty(&repo.root, &["lock", "rebase", "--json"], None);
    run.code(1);
    assert!(!run.asked, "{}", run.show());
    let document = common::one_json_document(&run.stdout)
        .unwrap_or_else(|problem| panic!("{problem}: {}", run.show()));
    keys_in_order(&document, &run.stdout, "exit 1");
    assert!(document["refusal"].is_string(), "{document}");
    assert_eq!(document["commit"], Value::Null);
    assert_eq!(document["unchanged"], json!(false));

    // Exit 0, nothing to change.
    let repo = LockRepo::with_lock("lr-json-nothing", &golden_text());
    let run = repo.spec_pty(&repo.root, &["lock", "rebase", "--json"], None);
    run.code(0);
    assert!(!run.asked, "{}", run.show());
    let document = common::one_json_document(&run.stdout)
        .unwrap_or_else(|problem| panic!("{problem}: {}", run.show()));
    keys_in_order(&document, &run.stdout, "nothing to change");
    assert_eq!(document["unchanged"], json!(true));
    assert_eq!(document["commit"], Value::Null);
    assert_eq!(document["refusal"], Value::Null);

    // Exit 2: no document.
    let repo = LockRepo::with_lock("lr-json-cannot", &Variant::OtherNorm.text(&golden_text()));
    let run = repo.spec_pty(&repo.root, &["lock", "rebase", "--json"], None);
    run.code(2);
    assert!(run.stdout.is_empty(), "{}", run.show());
}

// ------------------------------------------------------------------ TE-2

/// TE-2: the migration on a pseudo-terminal: the format-1 golden committed
/// -> `spec verify` exits 2 naming the step -> `spec lock rebase`, `y` ->
/// one commit passing the four checks (parent the old `HEAD`, the lock
/// alone, the rebase's trailers, the confirmed bytes) -> verify's rows
/// (keys, states, categories) as over accept's golden but for
/// `lock.commit`, exit 2 both (code-a's Cannot rows) -> `spec lock accept`
/// of `PIECE-COUNTER`, `LAW-BOUND`, `GOAL-01`: `nothing to change`;
/// `cap-check/PASS-01`: exit 1.
#[test]
fn the_migration_on_a_terminal() {
    let repo = LockRepo::format_1_golden("lr-migration");
    let before = repo.spec_piped(&repo.root, &["verify"]);
    before.code(2);
    assert!(
        before
            .stdout
            .lines()
            .any(|line| line == format!("cannot: {}", Variant::Format1.message())),
        "{}",
        before.show()
    );
    assert!(before.stdout.contains("run spec lock rebase on a terminal"));

    let head = repo.head();
    let run = repo.spec_pty(&repo.root, &["lock", "rebase"], Some("y"));
    run.code(0);
    assert!(run.asked, "{}", run.show());
    let commit = repo.head();
    assert_eq!(run.stdout, format!("{}committed {commit}\n", golden_rows()));
    assert_eq!(repo.parents(&commit), [head]);
    assert_eq!(repo.changed(&commit), ["spec.lock"]);
    assert_eq!(raw_message(&repo, &commit), rebase_message());
    assert_eq!(
        repo.show_blob(&commit, "spec.lock"),
        rebased_golden_text().into_bytes()
    );

    let rebased = verify(&repo, &[]);
    let accepted = verify(&verify_common::golden("lr-migration-accepted"), &[]);
    assert_eq!((rebased.code(), accepted.code()), (2, 2));
    assert_eq!(rebased.json["counts"], accepted.json["counts"]);
    assert_eq!(
        rows_without_lock_commit(&rebased),
        rows_without_lock_commit(&accepted)
    );

    for reference in ["PIECE-COUNTER", "LAW-BOUND", "GOAL-01"] {
        let run = repo.spec_pty(&repo.root, &["lock", "accept", reference], None);
        run.code(0);
        assert!(!run.asked, "{reference}: {}", run.show());
        assert!(
            run.stdout.contains(NOTHING_TO_CHANGE),
            "{reference}: {}",
            run.show()
        );
    }
    let pass = repo.spec_pty(&repo.root, &["lock", "accept", "cap-check/PASS-01"], None);
    pass.code(1);
    assert!(!pass.asked, "{}", pass.show());
    assert_eq!(repo.head(), commit, "no accept committed");
    assert_eq!(repo.lock_text(), rebased_golden_text());
}

// ------------------------------------------------------------------ TE-3

/// TE-3: a simulated grammar bump: accept's golden committed under another
/// grammar's `code_recipe` -> verify exits 2 naming the step -> the
/// rebase, under the note (not this build's: no cross-check), gives
/// `code-a.lock` byte for byte, 9 rebased.
#[test]
fn a_grammar_bump_rebases_to_accepts_golden() {
    let golden = golden_text();
    let repo = LockRepo::with_lock("lr-bump", &Variant::OtherCode.text(&golden));
    let run = verify(&repo, &[]);
    assert_eq!(run.code(), 2);
    assert_eq!(
        causes(&run).first().map(String::as_str),
        Some(Variant::OtherCode.message().as_str())
    );
    let head = repo.head();
    let attempt = repo.rebase(true);
    attempt.committed("a grammar bump");
    assert_eq!(repo.lock_text(), golden);
    assert_eq!(attempt.notes(), [not_this_builds_note(Variant::OTHER_CODE)]);
    assert_eq!(
        attempt.question(),
        question(2, Variant::OTHER_CODE, 9, 0, &head, &repo.root)
    );
    let rebased: String = golden_rows()
        .lines()
        .filter(|line| line.starts_with("rebased | "))
        .map(|line| format!("{line}\n"))
        .collect();
    assert_eq!(attempt.rows(), rebased);
}

// ------------------------------------------------------------------ TE-6

/// TE-6: `N` (the library's `false`; on a terminal `n`, an empty line,
/// `no`) leaves `HEAD`, refs, the index, `git status` and every file as
/// they were: exit 1 after the one question.
#[test]
fn answering_no_writes_nothing() {
    let repo = LockRepo::format_1_golden("lr-no");
    let before = repo.state();
    let attempt = repo.rebase(false);
    attempt.refused("not y");
    assert_eq!(attempt.prompts.len(), 1);
    assert_eq!(repo.state(), before);
    for typed in ["n", "", "no"] {
        let run = repo.spec_pty(&repo.root, &["lock", "rebase"], Some(typed));
        run.code(1);
        assert!(run.asked, "{typed:?}: {}", run.show());
        assert!(!run.stdout.contains("committed"), "{}", run.show());
        assert_eq!(repo.state(), before, "{typed:?}");
    }
}

// ------------------------------------------------------------------ TE-8

/// What one rebase on a terminal printed, `--json` or not: stdout, stderr
/// (the worktree `<worktree>`, the old `HEAD` `<head>`, the commit made
/// `<commit>`), the lock's bytes.
type Printed = (String, String, Vec<u8>);

fn printed(repo: &LockRepo, cwd: &Path, json: bool) -> Printed {
    let head = repo.head();
    let mut args = vec!["lock", "rebase"];
    if json {
        args.push("--json");
    }
    let run = repo.spec_pty(cwd, &args, Some("y"));
    run.code(0);
    let commit = repo.head();
    assert_ne!(commit, head, "{}", run.show());
    let normalize = |text: &str| {
        text.replace(&repo.root.display().to_string(), "<worktree>")
            .replace(&commit, "<commit>")
            .replace(&head, "<head>")
    };
    (
        normalize(&run.stdout),
        normalize(&run.stderr),
        repo.lock().expect("a lock"),
    )
}

/// A repository of `fixtures/code-a` at `<scratch>/<dir>` with `lock`
/// committed on the fixture's commit.
fn repo_at(label: &str, dir: &str, lock: &str) -> LockRepo {
    let repo = LockRepo::of(label, "code-a", dir, &[]);
    repo.write("spec.lock", lock);
    repo.commit_all("the lock");
    repo
}

/// TE-8 (lock-levels.md TE-2's twin): the rebase run from two roots, from
/// another cwd, under `core.autocrlf=true` (the code, the documents and
/// the lock CRLF in the worktree) and over the input's entries reversed
/// gives byte-identical locks, stdout, stderr and JSON; the same history
/// the same commit.
#[test]
fn rebase_is_byte_identical_everywhere() {
    let format_1 = golden_format_1_text();
    let both = |make: &dyn Fn(&str) -> (LockRepo, Option<String>)| -> (Printed, Printed, String) {
        let (text, cwd) = make("text");
        let at = cwd.map_or_else(|| text.root.clone(), |sub| text.root.join(sub));
        let printed_text = printed(&text, &at, false);
        let commit = text.head();
        let (json, cwd) = make("json");
        let at = cwd.map_or_else(|| json.root.clone(), |sub| json.root.join(sub));
        let printed_json = printed(&json, &at, true);
        (printed_text, printed_json, commit)
    };

    let reference = both(&|kind| {
        (
            repo_at(&format!("lr-det-ref-{kind}"), "repo", &format_1),
            None,
        )
    });
    assert_eq!(reference.0.2, rebased_golden_text().into_bytes());
    assert_eq!(reference.1.2, reference.0.2);

    let mut failures = Vec::new();
    let mut compare = |label: &str, other: (Printed, Printed, String), same_commit: bool| {
        if other.0 != reference.0 {
            failures.push(format!(
                "{label}: text run\n{:#?}\nvs\n{:#?}",
                other.0, reference.0
            ));
        }
        if other.1 != reference.1 {
            failures.push(format!(
                "{label}: JSON run\n{:#?}\nvs\n{:#?}",
                other.1, reference.1
            ));
        }
        if same_commit && other.2 != reference.2 {
            failures.push(format!("{label}: commit {} vs {}", other.2, reference.2));
        }
    };

    compare(
        "two roots",
        both(&|kind| {
            (
                repo_at(
                    &format!("lr-det-root-{kind}"),
                    "elsewhere/deeper",
                    &format_1,
                ),
                None,
            )
        }),
        true,
    );
    compare(
        "another cwd",
        both(&|kind| {
            (
                repo_at(&format!("lr-det-cwd-{kind}"), "repo", &format_1),
                Some("crates/tally".to_owned()),
            )
        }),
        true,
    );
    compare(
        "core.autocrlf=true",
        both(&|kind| {
            let repo = repo_at(&format!("lr-det-crlf-{kind}"), "repo", &format_1);
            repo.git(&["config", "core.autocrlf", "true"]);
            for path in [
                COUNTER,
                LIB,
                BOUNDS,
                LIMITS,
                "docs/spec/counter.md",
                "docs/records/GOAL-01.md",
                "spec.lock",
            ] {
                fs::remove_file(repo.root.join(path)).expect("remove");
                repo.git(&["checkout", "--", path]);
            }
            assert!(
                repo.read("spec.lock").contains("\r\n"),
                "the scenario: a CRLF lock in the worktree"
            );
            assert!(repo.porcelain().is_empty(), "clean under autocrlf");
            (repo, None)
        }),
        true,
    );
    let reversed_input = reversed(&format_1);
    assert_ne!(reversed_input, format_1);
    compare(
        "reversed entries",
        both(&|kind| {
            (
                repo_at(&format!("lr-det-rev-{kind}"), "repo", &reversed_input),
                None,
            )
        }),
        false,
    );
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

// ------------------------------------------------------- the files read

/// The rebased golden is the repository's: under `fixtures/`, named apart
/// from `spec.lock`; and `node_blocks` sees its 9 entries by node.
#[test]
fn the_rebased_golden_lives_in_fixtures() {
    let file = rebased_golden_file();
    assert!(file.starts_with(common::repository_root().join("fixtures")));
    assert_ne!(
        file.file_name().and_then(|name| name.to_str()),
        Some("spec.lock")
    );
    let text = rebased_golden_text();
    let count = |node: &str| node_blocks(&text, node).len();
    assert_eq!(
        (count("GOAL-01"), count("LAW-BOUND"), count("PIECE-COUNTER")),
        (1, 4, 4)
    );
    let (preamble, chunked) = chunks(&text);
    assert_eq!(joined(&preamble, &chunked), text, "chunks round-trip");
}
