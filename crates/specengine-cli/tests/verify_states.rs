//! docs/features/spec-verify.md TE-1, TE-2, RD-2's CLI-level named tests
//! and RD-10's golden: `spec verify` over scratch repositories of
//! `fixtures/code-a` with slice 3's golden lock committed, one scenario per
//! "States" row and category, each pinning its row's JSON (every key) and
//! its text line, and naming the mutation that turns it red (`M:`).
//!
//! code-a carries every binding problem the canon names on purpose, so the
//! base itself holds rows of most states; a scenario edits the worktree
//! (never commits unless it says) and reads the rows it changes.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;
mod lock_common;
mod verify_common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use lock_common::LockRepo;
use serde_json::{Value, json};
use verify_common::{
    BOUNDS, BROKEN, COUNTER, COUNTER_MD, GOAL_MD, LIB, LIMITS, Says, Verify, assert_row, golden,
    golden_lock, item, keyless, lock_entry, module, ron, verify,
};

/// A golden repository, `edit` applied to its worktree, verified plain.
fn scenario(label: &str, edit: impl FnOnce(&LockRepo)) -> (LockRepo, Verify) {
    let repo = golden(label);
    edit(&repo);
    let run = verify(&repo, &[]);
    (repo, run)
}

/// Says with no entry and no comparison.
fn says<'a>(
    state: &'a str,
    category: Option<&'a str>,
    detail: Option<&'a str>,
    markers: &'a [(usize, Option<u32>)],
    node_rev: Option<u32>,
) -> Says<'a> {
    Says {
        state,
        category,
        detail,
        pre_existing: false,
        markers,
        node_rev,
        lock: Value::Null,
        compared: None,
    }
}

/// `hint: <head>: …` printed exactly once.
fn has_hint(run: &Verify, head: &str) -> String {
    let prefix = format!("hint: {head}: ");
    let hits: Vec<&str> = run
        .lines()
        .into_iter()
        .filter(|line| line.starts_with(&prefix))
        .collect();
    assert_eq!(hits.len(), 1, "one {prefix:?}:\n{}", run.text.show());
    hits[0].to_owned()
}

/// A golden row: its key, state, category, markers `(line, rev)` and the
/// node's revision.
type GoldenRow = (
    verify_common::Key<'static>,
    &'static str,
    Option<&'static str>,
    Vec<(usize, Option<u32>)>,
    u32,
);

/// The eleven keys of the golden lock, with what each says over the
/// unchanged fixture.
fn golden_rows() -> Vec<GoldenRow> {
    let fmt = "crates/tally::counter::<Tally as fmt::Display>::fmt";
    vec![
        (
            item(
                "GOAL-01",
                "implements",
                COUNTER,
                "crates/tally::counter::Tally::value",
                "function_item",
            ),
            "ok",
            None,
            vec![(29, Some(1))],
            1,
        ),
        (
            item(
                "GOAL-01",
                "verifies",
                COUNTER,
                "crates/tally::counter::inner::limit",
                "function_item",
            ),
            "bad_marker",
            Some("bad_levels"),
            vec![(77, None)],
            1,
        ),
        (
            item("LAW-BOUND", "implements", COUNTER, fmt, "function_item"),
            "ok",
            None,
            vec![(48, Some(2))],
            2,
        ),
        (
            item(
                "LAW-BOUND",
                "verifies",
                BOUNDS,
                "crates/tally::test:bounds::stops_at_cap",
                "function_item",
            ),
            "ok",
            None,
            vec![(4, Some(2))],
            2,
        ),
        (
            item(
                "LAW-BOUND",
                "configures",
                COUNTER,
                "crates/tally::counter::CAP",
                "const_item",
            ),
            "ok",
            None,
            vec![(8, Some(2))],
            2,
        ),
        (
            ron("LAW-BOUND", "configures", LIMITS, "root.cap"),
            "ok",
            None,
            vec![(4, Some(2))],
            2,
        ),
        (
            item(
                "PIECE-COUNTER",
                "implements",
                COUNTER,
                "crates/tally::counter::Tally",
                "struct_item",
            ),
            "ok",
            None,
            vec![(13, Some(3))],
            3,
        ),
        (
            module("PIECE-COUNTER", "implements", LIB, "crates/tally"),
            "ok",
            None,
            vec![(2, Some(3))],
            3,
        ),
        (
            module(
                "PIECE-COUNTER",
                "verifies",
                BOUNDS,
                "crates/tally::test:bounds",
            ),
            "ok",
            None,
            vec![(1, None)],
            3,
        ),
        (
            ron("PIECE-COUNTER", "configures", LIMITS, "root"),
            "ok",
            None,
            vec![(2, Some(3))],
            3,
        ),
        (
            item(
                "cap-check/PASS-01",
                "verifies",
                BOUNDS,
                "crates/tally::test:bounds::steps_by_one",
                "function_item",
            ),
            "bad_marker",
            Some("bad_levels"),
            vec![(14, None)],
            1,
        ),
    ]
}

// ------------------------------------------------------------- RD-10 golden

/// RD-10 (seam: keys by accept's own builder), orchestrator's decision on
/// deviation 1: the golden lock over the fixture it was accepted from gives
/// one row per entry, nine `ok` by blob and the two markers with invalid
/// level lists (`counter.rs:77` `[path, path]`, `bounds.rs:14` `[Sig]`)
/// `bad_marker bad_levels`; no entry reads `marker_gone` or `unlocked`
/// beside it. M: keys built apart from accept's (the cited token, not the
/// canonical ID).
#[test]
fn golden_lock_verifies_ok_but_bad_levels() {
    let repo = golden("verify-golden");
    let run = verify(&repo, &[]);
    let text = fs::read_to_string(verify_common::golden_file()).expect("golden");
    let expected = golden_rows();
    assert_eq!(run.lock_rows().len(), expected.len(), "{}", run.text.show());
    for (key, state, category, markers, rev) in &expected {
        assert_row(
            &run,
            *key,
            &Says {
                state,
                category: *category,
                detail: None,
                pre_existing: false,
                markers,
                node_rev: Some(*rev),
                lock: lock_entry(&text, *key),
                compared: (*state == "ok").then_some("blob"),
            },
            None,
        );
    }
    assert_eq!(run.json["cascade"], json!({"by_blob": 9, "by_ast": 0}));
    // No entry is shadowed by a row of its own markers keyed apart.
    assert!(
        run.rows_where(|row| row["category"] == "marker_gone"
            || (row["state"] == "unlocked"
                && ["GOAL-01", "LAW-BOUND", "PIECE-COUNTER", "cap-check/PASS-01"]
                    .contains(&row["node"].as_str().unwrap_or_default())
                && row["relation"] != "implements"))
            .is_empty(),
        "{}",
        run.text.show()
    );
    // The fixture's deliberate problems: 39 rows, exit 2 in observe. Since
    // docs/features/symbol-discriminator.md (TE-1) the two adjacent
    // `impl Tally` blocks are two keys: one `cannot` row became two `fix`
    // rows (`predated`, `bad_marker bad_rev`).
    assert_eq!(
        run.json["counts"],
        json!({"rows": 39, "ok": 9, "fix": 19, "check": 5, "cannot": 6, "pre_existing": 0, "skipped": 2})
    );
    assert_eq!(run.code(), 2, "{}", run.text.show());
    assert_eq!(
        run.summary(),
        "verify: cannot-verify [observe]: 19 fix, 5 check, 6 cannot, 9 ok; 9 by blob, 0 by ast; 2 skipped"
    );
}

// ------------------------------------------------------------ cannot_verify

/// TE-1 `cannot_verify item_error` on a keyless marker: `broken.rs`'s
/// marker above an `fn` with a parse error. M: a keyless `cannot_verify`
/// marker judged as `unlocked` (rule 7 dropped).
#[test]
fn keyless_item_error_cannot_verify() {
    let repo = golden("verify-item-error");
    let run = verify(&repo, &[]);
    assert_row(
        &run,
        keyless("GOAL-01", "implements", BROKEN),
        &says(
            "cannot_verify",
            Some("item_error"),
            Some("other"),
            &[(4, Some(1))],
            Some(1),
        ),
        Some("cannot_verify other"),
    );
    has_hint(&run, "cannot_verify item_error");
}

/// TE-1 `cannot_verify item_error` of an entry (accept's `keep_broken`): a
/// locked item made unparseable keeps its marker, now keyless; the entry
/// reads `cannot_verify` with the hash's first category, never
/// `broken_binding`. M: `keep_broken` read as `marker_gone`.
#[test]
fn locked_item_made_unparseable_cannot_verify() {
    let (_repo, run) = scenario("verify-keep-broken", |repo| {
        repo.replace(
            COUNTER,
            "        self.count\n    }\n\n    // @implements GOAL-01@\n",
            "        self.count +\n    }\n\n    // @implements GOAL-01@\n",
        );
    });
    let key = item(
        "GOAL-01",
        "implements",
        COUNTER,
        "crates/tally::counter::Tally::value",
        "function_item",
    );
    assert_row(
        &run,
        key,
        &Says {
            lock: golden_lock(key),
            ..says(
                "cannot_verify",
                Some("item_error"),
                Some("missing"),
                &[],
                Some(1),
            )
        },
        None,
    );
    assert_row(
        &run,
        keyless("GOAL-01", "implements", COUNTER),
        &says(
            "cannot_verify",
            Some("item_error"),
            Some("missing"),
            &[(29, Some(1))],
            Some(1),
        ),
        Some("cannot_verify missing"),
    );
    assert_eq!(run.code(), 2);
}

/// TE-1 `cannot_verify item_error nesting_too_deep`: a RON marker inside
/// containers nested past the reader's depth. M: a RON keyless binding
/// judged as `unanchored`.
#[test]
fn ron_nested_too_deep_cannot_verify() {
    let depth = 520;
    let deep = format!(
        "Deep(\n    deep: {}\n        1, // @configures LAW-BOUND@2\n    {},\n)\n",
        "[".repeat(depth),
        "]".repeat(depth)
    );
    let (_repo, run) = scenario("verify-ron-deep", |repo| repo.write("data/deep.ron", deep));
    assert_row(
        &run,
        keyless("LAW-BOUND", "configures", "data/deep.ron"),
        &says(
            "cannot_verify",
            Some("item_error"),
            Some("nesting_too_deep"),
            &[(3, Some(2))],
            Some(2),
        ),
        Some("cannot_verify nesting_too_deep"),
    );
}

/// The two files of a cross-file pair: one module path,
/// `crates/tally::pair` (`docs/canon/symbol-names.md` "Ambiguity":
/// `src/x.rs` beside `src/x/mod.rs`).
const PAIR_FILES: [&str; 2] = ["crates/tally/src/pair.rs", "crates/tally/src/pair/mod.rs"];

/// Each file of the pair: one marked `fn twin`.
const PAIR_SOURCE: &str = "// @implements LAW-STEP\npub fn twin() {}\n";

/// TE-1 `cannot_verify ambiguous_symbol` (`path_attribute`, `unrooted`,
/// `duplicate`, a RON duplicate too): shown, never resolved, listed in
/// `delta.bindings.ambiguous`. The fixture holds all but the item
/// `duplicate`, which since docs/features/symbol-discriminator.md (TE-1)
/// is a cross-file pair only (written here: [`PAIR_FILES`]); the adjacent
/// `impl Tally` blocks are [`adjacent_impls_are_two_rows`]. M: an
/// ambiguity ignored (the row judged as bound).
#[test]
fn ambiguous_symbols_cannot_verify() {
    let (_repo, run) = scenario("verify-ambiguous", |repo| {
        for file in PAIR_FILES {
            repo.write(file, PAIR_SOURCE);
        }
    });
    let cases = [
        (
            module("GOAL-01", "implements", "tools/loose.rs", "."),
            "unrooted",
            vec![(2, None)],
            1,
        ),
        (
            item(
                "LAW-BOUND",
                "implements",
                "crates/tally/src/extra/moved.rs",
                "crates/tally::extra::moved::shift",
                "function_item",
            ),
            "path_attribute",
            vec![(3, Some(1))],
            2,
        ),
        (
            item(
                "LAW-STEP",
                "implements",
                PAIR_FILES[0],
                "crates/tally::pair::twin",
                "function_item",
            ),
            "duplicate",
            vec![(1, None)],
            1,
        ),
        (
            item(
                "LAW-STEP",
                "implements",
                PAIR_FILES[1],
                "crates/tally::pair::twin",
                "function_item",
            ),
            "duplicate",
            vec![(1, None)],
            1,
        ),
        (
            ron("LAW-STEP", "configures", LIMITS, "root.step"),
            "duplicate",
            vec![(6, None)],
            1,
        ),
        (
            item(
                "PREM-001",
                "implements",
                "tools/loose.rs",
                ".::seed",
                "function_item",
            ),
            "unrooted",
            vec![(5, Some(1))],
            1,
        ),
    ];
    for (key, reason, markers, rev) in &cases {
        assert_row(
            &run,
            *key,
            &says(
                "cannot_verify",
                Some("ambiguous_symbol"),
                Some(reason),
                markers,
                Some(*rev),
            ),
            None,
        );
    }
    let ambiguous = run.json["delta"]["bindings"]["ambiguous"]
        .as_array()
        .expect("ambiguous")
        .clone();
    let expected: Vec<Value> = cases
        .iter()
        .map(|(key, ..)| {
            json!({"node": key.node, "relation": key.relation, "file": key.file,
                   "kind": key.kind, "symbol": key.symbol, "item_kind": key.item_kind})
        })
        .collect();
    assert_eq!(ambiguous, expected);
    has_hint(&run, "cannot_verify ambiguous_symbol");
}

/// docs/features/symbol-discriminator.md TE-1: the two adjacent
/// `impl Tally` blocks of `counter.rs`, one `cannot_verify ambiguous_symbol
/// duplicate` row before, are two keys, `impl Tally #1` (its marker
/// `LAW-BOUND@3` above the node's rev 2: `predated`) and `impl Tally #2`
/// (`LAW-BOUND@3a`: `bad_marker bad_rev`); neither is listed as ambiguous,
/// and no row keys the undiscriminated `impl Tally`. M: in-file twins kept
/// ambiguous.
#[test]
fn adjacent_impls_are_two_rows() {
    let repo = golden("verify-adjacent-impls");
    let run = verify(&repo, &[]);
    let first = item(
        "LAW-BOUND",
        "implements",
        COUNTER,
        "crates/tally::counter::impl Tally #1",
        "impl_item",
    );
    let second = item(
        "LAW-BOUND",
        "implements",
        COUNTER,
        "crates/tally::counter::impl Tally #2",
        "impl_item",
    );
    assert_row(
        &run,
        first,
        &says("predated", None, None, &[(22, Some(3))], Some(2)),
        None,
    );
    assert_row(
        &run,
        second,
        &says("bad_marker", Some("bad_rev"), None, &[(45, None)], Some(2)),
        None,
    );
    assert!(
        run.rows_where(|row| row["symbol"] == "crates/tally::counter::impl Tally")
            .is_empty(),
        "{}",
        run.text.show()
    );
    let ambiguous = run.json["delta"]["bindings"]["ambiguous"]
        .as_array()
        .expect("ambiguous")
        .clone();
    assert!(
        ambiguous.iter().all(|key| !key["symbol"]
            .as_str()
            .unwrap_or_default()
            .starts_with("crates/tally::counter::impl Tally")),
        "{ambiguous:?}"
    );
}

/// TE-1 `cannot_verify ambiguous_id`: a bare feature-scoped ID two
/// features define. The entry the marker left reads `marker_gone`. M: the
/// first node picked.
#[test]
fn ambiguous_id_cannot_verify() {
    let (_repo, run) = scenario("verify-ambiguous-id", |repo| {
        repo.write(
            "docs/features/step-check.md",
            "---\nclass: spec\nstatus: draft\nscope: [crates/tally]\n---\n\n# Step check\n\n## Acceptance criteria\n\n### One step adds one {#PASS-01}\n\nStep once from zero: it reads 1.\n",
        );
        repo.replace(
            BOUNDS,
            "@verifies cap-check/PASS-01 [Sig]",
            "@verifies PASS-01",
        );
    });
    let symbol = "crates/tally::test:bounds::steps_by_one";
    assert_row(
        &run,
        item("PASS-01", "verifies", BOUNDS, symbol, "function_item"),
        &says(
            "cannot_verify",
            Some("ambiguous_id"),
            None,
            &[(14, None)],
            None,
        ),
        None,
    );
    let entry = item(
        "cap-check/PASS-01",
        "verifies",
        BOUNDS,
        symbol,
        "function_item",
    );
    assert_row(
        &run,
        entry,
        &Says {
            lock: golden_lock(entry),
            ..says("broken_binding", Some("marker_gone"), None, &[], Some(1))
        },
        None,
    );
    has_hint(&run, "cannot_verify ambiguous_id");
}

/// TE-1 `cannot_verify rev_unknown`: `LAW-ODD`'s `rev=0`. M: an unreadable
/// revision read as 1.
#[test]
fn unreadable_revision_cannot_verify() {
    let repo = golden("verify-rev-unknown");
    let run = verify(&repo, &[]);
    assert_row(
        &run,
        item(
            "LAW-ODD",
            "implements",
            COUNTER,
            "crates/tally::counter::Limit::max",
            "function_signature_item",
        ),
        &says(
            "cannot_verify",
            Some("rev_unknown"),
            None,
            &[(56, Some(1))],
            None,
        ),
        None,
    );
    has_hint(&run, "cannot_verify rev_unknown");
}

/// TE-1 `cannot_verify hash_null`: an unparsed reference in GOAL-01's
/// front matter nulls its meaning hash; its entries cannot be compared.
/// M: a null hash compared as unequal (`spec_ahead`).
#[test]
fn null_meaning_hash_cannot_verify() {
    let (_repo, run) = scenario("verify-hash-null", |repo| {
        repo.replace(
            GOAL_MD,
            "id: GOAL-01\n",
            "id: GOAL-01\nparent: GOAL-02 and GOAL-03\n",
        );
    });
    let key = item(
        "GOAL-01",
        "implements",
        COUNTER,
        "crates/tally::counter::Tally::value",
        "function_item",
    );
    assert_row(
        &run,
        key,
        &Says {
            lock: golden_lock(key),
            ..says(
                "cannot_verify",
                Some("hash_null"),
                None,
                &[(29, Some(1))],
                Some(1),
            )
        },
        None,
    );
    has_hint(&run, "cannot_verify hash_null");
    assert_eq!(run.code(), 2);
}

/// TE-1 `cannot_verify unreadable outside_code` and `unreadable
/// unreadable`: the entries' file on disk but excluded from `[code]`; then
/// its directory not even listable. M: a file outside `[code]` read as
/// `file_gone`.
#[test]
fn files_outside_code_or_unreadable_cannot_verify() {
    let exclude = |repo: &LockRepo| {
        repo.replace(
            "specengine.toml",
            "exclude = [\"crates/*/tests/fixtures/**\"]",
            "exclude = [\"crates/*/tests/**\"]",
        );
    };
    let (repo, run) = scenario("verify-outside", exclude);
    let rows = |run: &Verify, detail: &str| {
        for (key, rev) in [
            (
                item(
                    "LAW-BOUND",
                    "verifies",
                    BOUNDS,
                    "crates/tally::test:bounds::stops_at_cap",
                    "function_item",
                ),
                2,
            ),
            (
                module(
                    "PIECE-COUNTER",
                    "verifies",
                    BOUNDS,
                    "crates/tally::test:bounds",
                ),
                3,
            ),
            (
                item(
                    "cap-check/PASS-01",
                    "verifies",
                    BOUNDS,
                    "crates/tally::test:bounds::steps_by_one",
                    "function_item",
                ),
                1,
            ),
        ] {
            assert_row(
                run,
                key,
                &Says {
                    lock: golden_lock(key),
                    ..says(
                        "cannot_verify",
                        Some("unreadable"),
                        Some(detail),
                        &[],
                        Some(rev),
                    )
                },
                None,
            );
        }
    };
    rows(&run, "outside_code");
    has_hint(&run, "cannot_verify unreadable");
    let tests = repo.root.join("crates/tally/tests");
    fs::set_permissions(&tests, fs::Permissions::from_mode(0o000)).expect("chmod");
    let locked = verify(&repo, &[]);
    fs::set_permissions(&tests, fs::Permissions::from_mode(0o755)).expect("chmod back");
    rows(&locked, "unreadable");
    assert_eq!(locked.code(), 2);
}

// ----------------------------------------------------------------- predated

/// TE-1 `predated`: a marker naming a revision past the node's, with an
/// entry (raised on `CAP`) and without one (the fixture's `inner`). M: a
/// marker ahead of its node ignored (the row judged `ok`).
#[test]
fn marker_ahead_of_its_node_is_predated() {
    let (_repo, run) = scenario("verify-predated", |repo| {
        repo.replace(
            COUNTER,
            "@configures LAW-BOUND@2",
            "@configures LAW-BOUND@3",
        );
    });
    let key = item(
        "LAW-BOUND",
        "configures",
        COUNTER,
        "crates/tally::counter::CAP",
        "const_item",
    );
    assert_row(
        &run,
        key,
        &Says {
            lock: golden_lock(key),
            ..says("predated", None, None, &[(8, Some(3))], Some(2))
        },
        None,
    );
    assert_row(
        &run,
        module(
            "LAW-STEP",
            "implements",
            COUNTER,
            "crates/tally::counter::inner",
        ),
        &says("predated", None, None, &[(74, Some(2))], Some(1)),
        None,
    );
    has_hint(&run, "predated");
}

// --------------------------------------------------------------- bad_marker

/// TE-1 `bad_marker dangling`: IDs naming no node key on their ID part as
/// written, the `@rev` left out (`GOAL-99@2` keys on `GOAL-99`). M: a
/// dangling marker judged `unlocked`; key on the token.
#[test]
fn dangling_marker_is_bad_marker() {
    let repo = golden("verify-dangling");
    let run = verify(&repo, &[]);
    assert_row(
        &run,
        keyless("ZZZ-1", "implements", LIB),
        &says("bad_marker", Some("dangling"), None, &[(16, None)], None),
        Some("unanchored not_an_item"),
    );
    assert_row(
        &run,
        keyless("GOAL-99", "implements", LIB),
        &says("bad_marker", Some("dangling"), None, &[(17, Some(2))], None),
        Some("unanchored not_an_item"),
    );
    has_hint(&run, "bad_marker dangling");
}

/// TE-1 `bad_marker homoglyph`: a Cyrillic letter in a locked item's
/// marker, keyed on its ID part without `@1`; the entry it left reads
/// `marker_gone`. M: a look-alike ID resolved to the Latin node.
#[test]
fn homoglyph_marker_is_bad_marker() {
    let (_repo, run) = scenario("verify-homoglyph", |repo| {
        repo.replace(
            COUNTER,
            "@implements AIM-01@1",
            "@implements G\u{41E}AL-01@1",
        );
    });
    let symbol = "crates/tally::counter::Tally::value";
    assert_row(
        &run,
        item(
            "G\u{41E}AL-01",
            "implements",
            COUNTER,
            symbol,
            "function_item",
        ),
        &says(
            "bad_marker",
            Some("homoglyph"),
            None,
            &[(29, Some(1))],
            None,
        ),
        None,
    );
    let entry = item("GOAL-01", "implements", COUNTER, symbol, "function_item");
    assert_row(
        &run,
        entry,
        &Says {
            lock: golden_lock(entry),
            ..says("broken_binding", Some("marker_gone"), None, &[], Some(1))
        },
        None,
    );
    has_hint(&run, "bad_marker homoglyph");
}

/// TE-1 `bad_marker bad_rev`: `@` not followed by 1-9 digits ending the
/// token (the fixture's `GOAL-01@` and `GOAL-01@1234567890`); one keyless
/// row holds both markers of its key. M: a malformed revision read as
/// unversioned.
#[test]
fn malformed_revision_is_bad_marker() {
    let repo = golden("verify-bad-rev");
    let run = verify(&repo, &[]);
    assert_row(
        &run,
        item(
            "GOAL-01",
            "implements",
            COUNTER,
            "crates/tally::counter::Tally::reset",
            "function_item",
        ),
        &says("bad_marker", Some("bad_rev"), None, &[(41, None)], Some(1)),
        None,
    );
    let shared: Vec<&Value> = run.rows_where(|row| {
        row["node"] == "GOAL-01"
            && row["file"] == COUNTER
            && row["kind"].is_null()
            && row["category"] == "bad_rev"
    });
    assert_eq!(
        shared,
        vec![&verify_common::row_json(
            keyless("GOAL-01", "implements", COUNTER),
            &says(
                "bad_marker",
                Some("bad_rev"),
                None,
                &[(4, None), (34, None)],
                Some(1)
            ),
        )]
    );
    run.has_line("fix | bad_marker bad_rev | GOAL-01 | implements | crates/tally/src/counter.rs | unanchored not_an_item");
    has_hint(&run, "bad_marker bad_rev");
}

/// TE-1 `bad_marker unanchored <category>`: every category of the fixture
/// (`in_body`, `in_macro`, `not_an_item`, `no_item`, `no_value`), each a
/// keyless row printed as accept prints it. M: an unanchored marker
/// skipped.
#[test]
fn unanchored_markers_are_bad_marker() {
    let repo = golden("verify-unanchored");
    let run = verify(&repo, &[]);
    for (node, relation, file, category, markers, rev) in [
        (
            "GOAL-01",
            "implements",
            COUNTER,
            "in_body",
            vec![(25, None)],
            1,
        ),
        (
            "GOAL-01",
            "implements",
            COUNTER,
            "in_macro",
            vec![(67, None)],
            1,
        ),
        (
            "GOAL-01",
            "implements",
            LIB,
            "no_item",
            vec![(21, Some(1))],
            1,
        ),
        (
            "GOAL-01",
            "implements",
            LIB,
            "not_an_item",
            vec![(4, None)],
            1,
        ),
        (
            "GOAL-01",
            "configures",
            LIMITS,
            "no_value",
            vec![(8, None)],
            1,
        ),
        (
            "LAW-BOUND",
            "implements",
            COUNTER,
            "not_an_item",
            vec![(61, None)],
            2,
        ),
        (
            "PIECE-COUNTER",
            "implements",
            COUNTER,
            "in_body",
            vec![(84, Some(3))],
            3,
        ),
        (
            "PREM-001",
            "assumes",
            COUNTER,
            "no_item",
            vec![(37, None)],
            1,
        ),
    ] {
        assert_row(
            &run,
            keyless(node, relation, file),
            &says(
                "bad_marker",
                Some("unanchored"),
                Some(category),
                &markers,
                Some(rev),
            ),
            Some(&format!("unanchored {category}")),
        );
    }
    has_hint(&run, "bad_marker unanchored");
}

// ----------------------------------------------------------- broken_binding

/// TE-1 `broken_binding node_gone`, iteration 2's decision (the States
/// order stays): LAW-BOUND renamed, its leftover `LAW-BOUND@2` markers key
/// on `LAW-BOUND` and share the rows of its entries, which read
/// `bad_marker dangling` (the entry kept, `node_rev` null); the Display
/// impl's entry, whose marker cites `PIECE-COUNTER#LAW-BOUND`, reads
/// `node_gone`. With the leftover markers removed every entry reads
/// `broken_binding node_gone`, all four in `delta.bindings.lost`. M: a gone
/// node read as `unlocked`; key on the token.
#[test]
fn renamed_node_leftover_markers_are_bad_marker_then_node_gone() {
    let (repo, run) = scenario("verify-node-gone", |repo| {
        repo.replace(COUNTER_MD, "#LAW-BOUND rev=2", "#LAW-LIMIT rev=2");
    });
    let fmt = item(
        "LAW-BOUND",
        "implements",
        COUNTER,
        "crates/tally::counter::<Tally as fmt::Display>::fmt",
        "function_item",
    );
    let leftovers = [
        (
            item(
                "LAW-BOUND",
                "verifies",
                BOUNDS,
                "crates/tally::test:bounds::stops_at_cap",
                "function_item",
            ),
            4,
        ),
        (
            item(
                "LAW-BOUND",
                "configures",
                COUNTER,
                "crates/tally::counter::CAP",
                "const_item",
            ),
            8,
        ),
        (ron("LAW-BOUND", "configures", LIMITS, "root.cap"), 4),
    ];
    for (key, line) in leftovers {
        assert_row(
            &run,
            key,
            &Says {
                lock: golden_lock(key),
                ..says(
                    "bad_marker",
                    Some("dangling"),
                    None,
                    &[(line, Some(2))],
                    None,
                )
            },
            None,
        );
    }
    assert_row(
        &run,
        fmt,
        &Says {
            lock: golden_lock(fmt),
            ..says("broken_binding", Some("node_gone"), None, &[], None)
        },
        None,
    );
    assert_row(
        &run,
        item(
            "PIECE-COUNTER#LAW-BOUND",
            "implements",
            COUNTER,
            "crates/tally::counter::<Tally as fmt::Display>::fmt",
            "function_item",
        ),
        &says("bad_marker", Some("dangling"), None, &[(48, Some(2))], None),
        None,
    );
    let lost = run.json["delta"]["bindings"]["lost"]
        .as_array()
        .expect("lost");
    assert_eq!(lost.len(), 1, "{lost:?}");

    // The leftover markers removed: every entry `node_gone`.
    repo.replace(COUNTER, " // @configures LAW-BOUND@2", "");
    repo.replace(LIMITS, " // @configures LAW-BOUND@2", "");
    repo.replace(
        BOUNDS,
        "// @verifies LAW-BOUND@2 [body] mutation: drop the cap\n",
        "",
    );
    let gone = verify(&repo, &[]);
    for key in [fmt, leftovers[0].0, leftovers[1].0, leftovers[2].0] {
        assert_row(
            &gone,
            key,
            &Says {
                lock: golden_lock(key),
                ..says("broken_binding", Some("node_gone"), None, &[], None)
            },
            None,
        );
    }
    let lost = gone.json["delta"]["bindings"]["lost"]
        .as_array()
        .expect("lost");
    assert_eq!(lost.len(), 4, "{lost:?}");
    let hint = has_hint(&gone, "broken_binding node_gone");
    assert!(hint.contains("commit first"), "{hint}");
}

/// Iteration 2 ("Command and rows"): one unresolved ID cited at two
/// revisions on one item is one row, keyed on the ID part, both markers
/// in it. M: key on the token.
#[test]
fn one_unresolved_id_at_two_revisions_is_one_row() {
    let (_repo, run) = scenario("verify-two-revs", |repo| {
        repo.replace(
            COUNTER,
            "pub fn total(items: &[u32]) -> u32 {",
            "// @implements ZZZ-9@2\n// @implements ZZZ-9@3\npub fn total(items: &[u32]) -> u32 {",
        );
    });
    assert_row(
        &run,
        item(
            "ZZZ-9",
            "implements",
            COUNTER,
            "crates/tally::counter::total",
            "function_item",
        ),
        &says(
            "bad_marker",
            Some("dangling"),
            None,
            &[(83, Some(2)), (84, Some(3))],
            None,
        ),
        None,
    );
    assert!(
        run.rows_where(|row| row["node"]
            .as_str()
            .is_some_and(|node| node.starts_with("ZZZ-9@")))
            .is_empty(),
        "{}",
        run.text.show()
    );
}

/// The two `conflict` steps (iteration 2, the code's exact strings;
/// reworded by docs/features/rev-discipline.md RD-8: no proposal raises
/// `rev` after Q7, "raise its `rev` by hand, commit, then accept; or
/// `--editorial`"; "set the marker to its revision"; the editorial one,
/// shared with `spec_ahead same_revision`, naming `spec bump` since
/// docs/features/rev-rule.md RD-4).
const EDITORIAL_STEP: &str = "the node's text changed at its locked revision: raise its `rev` by hand (`spec bump` prints the line while the edit is uncommitted), commit, then accept; or `--editorial` for an editorial change (`spec lock accept <ID> --editorial`); accept refuses text changed at a revision";
const RAISE_STEP: &str = "at a revision the lock does not hold, the marker not raised: check the code against the node, set the marker to its revision, commit, then `spec lock accept <ID>` on a terminal";

/// Iteration 2, the `conflict` hint by R against the entry's revision: R
/// = `rev_L` gives the editorial step; R > `rev_L` with the marker not
/// raised gives the raise step; both present give one `hint: conflict:`
/// line, the steps joined by `; `. M: the old single hint.
#[test]
fn conflict_hint_follows_the_revision() {
    let same = |repo: &LockRepo| {
        repo.replace(
            COUNTER_MD,
            "The count never exceeds the cap.",
            "The count never exceeds the cap, ever.",
        );
        repo.replace(COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
    };
    let raised = |repo: &LockRepo| {
        repo.replace(COUNTER_MD, "rev: 3\n", "rev: 4\n");
        repo.replace(
            COUNTER_MD,
            "A count that grows one step",
            "A count that grows by one step",
        );
        repo.replace(COUNTER, "    count: u32,\n}", "    count: u64,\n}");
    };
    let conflicts = |run: &Verify| -> Vec<(String, Value, Value)> {
        run.rows_where(|row| row["state"] == "conflict")
            .iter()
            .map(|row| {
                (
                    row["node"].as_str().unwrap_or_default().to_owned(),
                    row["node_rev"].clone(),
                    row["lock"]["rev"].clone(),
                )
            })
            .collect()
    };

    let (_repo, at_same) = scenario("verify-conflict-same", same);
    assert_eq!(
        conflicts(&at_same),
        [("LAW-BOUND".to_owned(), json!(2), json!(2))]
    );
    assert_eq!(
        has_hint(&at_same, "conflict"),
        format!("hint: conflict: {EDITORIAL_STEP}")
    );

    let (_repo, at_raised) = scenario("verify-conflict-raised", raised);
    assert_eq!(
        conflicts(&at_raised),
        [("PIECE-COUNTER".to_owned(), json!(4), json!(3))]
    );
    assert_eq!(
        has_hint(&at_raised, "conflict"),
        format!("hint: conflict: {RAISE_STEP}")
    );

    let (_repo, both) = scenario("verify-conflict-both", |repo| {
        same(repo);
        raised(repo);
    });
    assert_eq!(conflicts(&both).len(), 2, "{}", both.text.show());
    assert_eq!(
        has_hint(&both, "conflict"),
        format!("hint: conflict: {EDITORIAL_STEP}; {RAISE_STEP}")
    );
    // `spec_ahead same_revision` keeps the editorial step alone.
    assert_eq!(
        has_hint(&both, "spec_ahead same_revision"),
        format!("hint: spec_ahead same_revision: {EDITORIAL_STEP}")
    );
}

/// TE-1 `broken_binding file_gone`: the RON file deleted. M: a deleted file
/// read as `unreadable`.
#[test]
fn deleted_file_is_file_gone() {
    let (_repo, run) = scenario("verify-file-gone", |repo| {
        fs::remove_file(repo.root.join(LIMITS)).expect("remove");
    });
    for (key, rev) in [
        (ron("LAW-BOUND", "configures", LIMITS, "root.cap"), 2),
        (ron("PIECE-COUNTER", "configures", LIMITS, "root"), 3),
    ] {
        assert_row(
            &run,
            key,
            &Says {
                lock: golden_lock(key),
                ..says("broken_binding", Some("file_gone"), None, &[], Some(rev))
            },
            None,
        );
    }
    let hint = has_hint(&run, "broken_binding file_gone");
    assert!(hint.contains("spec lock accept <ID>"), "{hint}");
}

/// TE-1 `broken_binding symbol_gone`: the bound `fn` renamed, no parse
/// error; its marker now binds the new name, `unlocked`. M: an absent
/// symbol read as `marker_gone`.
#[test]
fn renamed_symbol_is_symbol_gone() {
    let (_repo, run) = scenario("verify-symbol-gone", |repo| {
        repo.replace(BOUNDS, "fn stops_at_cap", "fn stops_at_the_cap");
    });
    let entry = item(
        "LAW-BOUND",
        "verifies",
        BOUNDS,
        "crates/tally::test:bounds::stops_at_cap",
        "function_item",
    );
    assert_row(
        &run,
        entry,
        &Says {
            lock: golden_lock(entry),
            ..says("broken_binding", Some("symbol_gone"), None, &[], Some(2))
        },
        None,
    );
    assert_row(
        &run,
        item(
            "LAW-BOUND",
            "verifies",
            BOUNDS,
            "crates/tally::test:bounds::stops_at_the_cap",
            "function_item",
        ),
        &says("unlocked", None, None, &[(4, Some(2))], Some(2)),
        None,
    );
    has_hint(&run, "broken_binding symbol_gone");
    has_hint(&run, "unlocked");
}

/// TE-1 `broken_binding marker_gone`: the markers of a locked item and a
/// locked RON value removed. M: an entry without a marker judged `ok`.
#[test]
fn removed_marker_is_marker_gone() {
    let (_repo, run) = scenario("verify-marker-gone", |repo| {
        repo.replace(COUNTER, " // @configures LAW-BOUND@2", "");
        repo.replace(LIMITS, " // @configures LAW-BOUND@2", "");
    });
    for key in [
        item(
            "LAW-BOUND",
            "configures",
            COUNTER,
            "crates/tally::counter::CAP",
            "const_item",
        ),
        ron("LAW-BOUND", "configures", LIMITS, "root.cap"),
    ] {
        assert_row(
            &run,
            key,
            &Says {
                lock: golden_lock(key),
                ..says("broken_binding", Some("marker_gone"), None, &[], Some(2))
            },
            None,
        );
    }
    has_hint(&run, "broken_binding marker_gone");
}

// ----------------------------------------------------------------- unlocked

/// TE-1 `unlocked`: markers with no entry, the fixture's. M: a row without
/// an entry judged `ok`.
#[test]
fn marker_without_an_entry_is_unlocked() {
    let repo = golden("verify-unlocked");
    let run = verify(&repo, &[]);
    for (key, line) in [
        (
            item(
                "LAW-STEP",
                "implements",
                COUNTER,
                "crates/tally::counter::Tally::step",
                "function_item",
            ),
            23,
        ),
        (
            item(
                "LAW-STEP",
                "implements",
                BROKEN,
                "crates/tally::broken::intact",
                "function_item",
            ),
            11,
        ),
        (
            item(
                "PREM-001",
                "implements",
                "crates/tally/src/main.rs",
                "crates/tally::bin:tally-kit::main",
                "function_item",
            ),
            1,
        ),
    ] {
        assert_row(
            &run,
            key,
            &says("unlocked", None, None, &[(line, Some(1))], Some(1)),
            None,
        );
    }
    let hint = has_hint(&run, "unlocked");
    assert!(hint.contains("spec lock accept <ID>"), "{hint}");
}

/// TE-1 `unlocked key_problem` (Q6): a file whose path `spec.lock` cannot
/// hold (a backslash; an LF), never hashed; the hint names the field and
/// never sends the owner to accept it. M: a key `key_problem` refuses
/// judged as plain `unlocked`.
#[test]
fn unlockable_key_is_unlocked_key_problem() {
    for (label, name, shown) in [
        ("verify-key-backslash", "we\\ird", "we\\ird"),
        ("verify-key-lf", "we\nird", "we ird"),
    ] {
        let (_repo, run) = scenario(label, |repo| {
            repo.write(
                &format!("crates/tally/src/{name}.rs"),
                "// @implements LAW-STEP\npub fn odd() {}\n",
            );
        });
        let file = format!("crates/tally/src/{name}.rs");
        let symbol = format!("crates/tally::{name}::odd");
        let key = item("LAW-STEP", "implements", &file, &symbol, "function_item");
        let row = run.row("LAW-STEP", "implements", &file, Some(&symbol));
        assert_eq!(
            row,
            &verify_common::row_json(
                key,
                &says("unlocked", None, Some("key_problem"), &[(1, None)], Some(1))
            )
        );
        run.has_line(&format!(
            "check | unlocked key_problem | LAW-STEP | implements | crates/tally/src/{shown}.rs | item crates/tally::{shown}::odd function_item"
        ));
        let hint = has_hint(&run, "unlocked key_problem");
        assert!(hint.contains("`file`"), "{hint}");
        assert!(!hint.contains("accept <ID>"), "{hint}");
    }
}

// --------------------------------------------------------------- spec_ahead

/// RD-2 / TE-1 `spec_ahead marker_behind`: a locked item's marker lowered
/// below the node's revision, spec and code unchanged. M: lagging with
/// equal hashes judged `ok`.
#[test]
fn lagging_marker_is_spec_ahead() {
    let (_repo, run) = scenario("verify-lagging", |repo| {
        repo.replace(
            COUNTER,
            "@configures LAW-BOUND@2",
            "@configures LAW-BOUND@1",
        );
    });
    let key = item(
        "LAW-BOUND",
        "configures",
        COUNTER,
        "crates/tally::counter::CAP",
        "const_item",
    );
    assert_row(
        &run,
        key,
        &Says {
            lock: golden_lock(key),
            compared: Some("ast"),
            ..says(
                "spec_ahead",
                Some("marker_behind"),
                None,
                &[(8, Some(1))],
                Some(2),
            )
        },
        None,
    );
    has_hint(&run, "spec_ahead marker_behind");
}

/// TE-1 `spec_ahead spec_changed` and `marker_behind`: PIECE-COUNTER's
/// `rev` raised with its text, code unchanged: the unversioned marker's
/// row `spec_changed`, the `@3` markers' `marker_behind`. M: a raised
/// revision without a raised marker judged `ok`.
#[test]
fn raised_revision_without_raised_markers_is_spec_ahead() {
    let (_repo, run) = scenario("verify-spec-changed", |repo| {
        repo.replace(COUNTER_MD, "rev: 3\n", "rev: 4\n");
        repo.replace(
            COUNTER_MD,
            "A count that grows one step",
            "A count that grows by one step",
        );
    });
    let key = module(
        "PIECE-COUNTER",
        "verifies",
        BOUNDS,
        "crates/tally::test:bounds",
    );
    assert_row(
        &run,
        key,
        &Says {
            lock: golden_lock(key),
            compared: Some("blob"),
            ..says(
                "spec_ahead",
                Some("spec_changed"),
                None,
                &[(1, None)],
                Some(4),
            )
        },
        None,
    );
    for (key, line) in [
        (
            item(
                "PIECE-COUNTER",
                "implements",
                COUNTER,
                "crates/tally::counter::Tally",
                "struct_item",
            ),
            13,
        ),
        (
            module("PIECE-COUNTER", "implements", LIB, "crates/tally"),
            2,
        ),
        (ron("PIECE-COUNTER", "configures", LIMITS, "root"), 2),
    ] {
        assert_row(
            &run,
            key,
            &Says {
                lock: golden_lock(key),
                compared: Some("blob"),
                ..says(
                    "spec_ahead",
                    Some("marker_behind"),
                    None,
                    &[(line, Some(3))],
                    Some(4),
                )
            },
            None,
        );
    }
    has_hint(&run, "spec_ahead spec_changed");
}

/// TE-1 `spec_ahead same_revision`: LAW-BOUND's text edited at its locked
/// revision; the hint names `--editorial`. M: text changed at R judged
/// `ok` (the revision alone compared).
#[test]
fn text_changed_at_its_revision_is_spec_ahead_same_revision() {
    let (_repo, run) = scenario("verify-same-revision", |repo| {
        repo.replace(
            COUNTER_MD,
            "The count never exceeds the cap.",
            "The count never exceeds the cap, ever.",
        );
    });
    for (key, line) in [
        (
            item(
                "LAW-BOUND",
                "implements",
                COUNTER,
                "crates/tally::counter::<Tally as fmt::Display>::fmt",
                "function_item",
            ),
            48,
        ),
        (
            item(
                "LAW-BOUND",
                "verifies",
                BOUNDS,
                "crates/tally::test:bounds::stops_at_cap",
                "function_item",
            ),
            4,
        ),
        (
            item(
                "LAW-BOUND",
                "configures",
                COUNTER,
                "crates/tally::counter::CAP",
                "const_item",
            ),
            8,
        ),
        (ron("LAW-BOUND", "configures", LIMITS, "root.cap"), 4),
    ] {
        assert_row(
            &run,
            key,
            &Says {
                lock: golden_lock(key),
                compared: Some("blob"),
                ..says(
                    "spec_ahead",
                    Some("same_revision"),
                    None,
                    &[(line, Some(2))],
                    Some(2),
                )
            },
            None,
        );
    }
    let hint = has_hint(&run, "spec_ahead same_revision");
    assert!(hint.contains("--editorial"), "{hint}");
}

// --------------------------------------------------------------- code_ahead

/// TE-1 `code_ahead code_changed`: the locked `CAP` edited (by its hash)
/// and the locked RON value edited (by its blob); `delta.bindings.stale`
/// lists both. M: a changed item judged `ok`.
#[test]
fn changed_code_is_code_ahead() {
    let (_repo, run) = scenario("verify-code-changed", |repo| {
        repo.replace(COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
        repo.replace(LIMITS, "cap: 10,", "cap: 12,");
    });
    for (key, compared) in [
        (
            item(
                "LAW-BOUND",
                "configures",
                COUNTER,
                "crates/tally::counter::CAP",
                "const_item",
            ),
            "ast",
        ),
        (ron("LAW-BOUND", "configures", LIMITS, "root.cap"), "blob"),
    ] {
        assert_row(
            &run,
            key,
            &Says {
                lock: golden_lock(key),
                compared: Some(compared),
                ..says(
                    "code_ahead",
                    Some("code_changed"),
                    None,
                    &[(if compared == "ast" { 8 } else { 4 }, Some(2))],
                    Some(2),
                )
            },
            None,
        );
    }
    // The other entries of the changed files: hashed, unchanged.
    for key in [
        item(
            "GOAL-01",
            "implements",
            COUNTER,
            "crates/tally::counter::Tally::value",
            "function_item",
        ),
        item(
            "PIECE-COUNTER",
            "implements",
            COUNTER,
            "crates/tally::counter::Tally",
            "struct_item",
        ),
    ] {
        let row = run.row(key.node, key.relation, key.file, key.symbol);
        assert_eq!(
            (row["state"].as_str(), row["compared"].as_str()),
            (Some("ok"), Some("ast"))
        );
    }
    // The RON root entry: compared by its blob alone, changed.
    let root = run.row("PIECE-COUNTER", "configures", LIMITS, Some("root"));
    assert_eq!(root["state"], "code_ahead");
    let stale = run.json["delta"]["bindings"]["stale"]
        .as_array()
        .expect("stale");
    assert_eq!(stale.len(), 3, "{stale:?}");
    let hint = has_hint(&run, "code_ahead code_changed");
    assert!(hint.contains("spec lock accept <ID>"), "{hint}");
}

/// RD-2 / TE-1 `code_ahead raised`: LAW-BOUND's text changed with its `rev`
/// raised to 3 and `CAP`'s marker raised to name it, the code changed too:
/// the owner's raise is no drift. M: `conflict` when a marker names R.
#[test]
fn raised_marker_with_both_changed_is_code_ahead() {
    let (_repo, run) = scenario("verify-raised", |repo| {
        repo.replace(COUNTER_MD, "#LAW-BOUND rev=2", "#LAW-BOUND rev=3");
        repo.replace(
            COUNTER_MD,
            "The count never exceeds the cap.",
            "The count never exceeds the cap, ever.",
        );
        repo.replace(
            COUNTER,
            "@configures LAW-BOUND@2",
            "@configures LAW-BOUND@3",
        );
        repo.replace(COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
    });
    let key = item(
        "LAW-BOUND",
        "configures",
        COUNTER,
        "crates/tally::counter::CAP",
        "const_item",
    );
    assert_row(
        &run,
        key,
        &Says {
            lock: golden_lock(key),
            compared: Some("ast"),
            ..says("code_ahead", Some("raised"), None, &[(8, Some(3))], Some(3))
        },
        None,
    );
    has_hint(&run, "code_ahead raised");
}

// ----------------------------------------------------------------- conflict

/// TE-1 `conflict`: LAW-BOUND's text edited at its revision and `CAP`
/// edited, no raise; the hint names `--editorial` or a raising proposal.
/// M: both changes judged `code_ahead`.
#[test]
fn spec_and_code_changed_is_conflict() {
    let (_repo, run) = scenario("verify-conflict", |repo| {
        repo.replace(
            COUNTER_MD,
            "The count never exceeds the cap.",
            "The count never exceeds the cap, ever.",
        );
        repo.replace(COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
    });
    let key = item(
        "LAW-BOUND",
        "configures",
        COUNTER,
        "crates/tally::counter::CAP",
        "const_item",
    );
    assert_row(
        &run,
        key,
        &Says {
            lock: golden_lock(key),
            compared: Some("ast"),
            ..says("conflict", None, None, &[(8, Some(2))], Some(2))
        },
        None,
    );
    let hint = has_hint(&run, "conflict");
    assert!(hint.contains("--editorial"), "{hint}");
    let stale = run.json["delta"]["bindings"]["stale"]
        .as_array()
        .expect("stale");
    assert!(stale.contains(
        &json!({"node": "LAW-BOUND", "relation": "configures", "file": COUNTER,
        "kind": "item", "symbol": "crates/tally::counter::CAP", "item_kind": "const_item"})
    ));
}

// -------------------------------------------------------------- drift hint

/// docs/features/drift-queue.md RD-18 (`docs/features/drift-queue.md`
/// "report_drift and texts"): when a row is `code_ahead`, `spec_ahead`,
/// `conflict` or `broken_binding`, verify prints once, after every other
/// hint, `hint: drift: commit first; spec propose drift queues HEAD's
/// drift for the owner`; a tree whose findings are none of those (the
/// golden lock with `PIECE-COUNTER`'s lagging marker raised to its
/// revision: only `cannot_verify`, `bad_marker`, `predated`, `unlocked`
/// left) prints no such hint. M: the hint missing, or printed before
/// another hint.
#[test]
fn drift_hint_follows_the_other_hints() {
    const DRIFT_HINT: &str =
        "hint: drift: commit first; spec propose drift queues HEAD's drift for the owner";
    for (label, edit) in [
        ("verify-drift-hint-golden", None),
        (
            "verify-drift-hint-code",
            Some(("CAP: u32 = 10;", "CAP: u32 = 12;")),
        ),
    ] {
        let (_repo, run) = scenario(label, |repo| {
            if let Some((from, to)) = edit {
                repo.replace(COUNTER, from, to);
            }
        });
        let hints: Vec<&str> = run
            .lines()
            .into_iter()
            .filter(|line| line.starts_with("hint: "))
            .collect();
        assert_eq!(
            hints.iter().filter(|line| **line == DRIFT_HINT).count(),
            1,
            "{label}: once:\n{}",
            run.text.show()
        );
        assert_eq!(hints.last(), Some(&DRIFT_HINT), "{label}: after the others");
        assert!(hints.len() > 1, "{label}: other hints too");
    }
    // No drifting state: no drift hint.
    let (_repo, run) = scenario("verify-drift-hint-none", |repo| {
        repo.replace(
            COUNTER,
            "@implements PIECE-COUNTER@2",
            "@implements PIECE-COUNTER@3",
        );
    });
    let drifting = run.rows_where(|row| {
        ["code_ahead", "spec_ahead", "conflict", "broken_binding"]
            .contains(&row["state"].as_str().unwrap_or_default())
    });
    assert!(drifting.is_empty(), "the scenario: {drifting:?}");
    assert!(
        !run.lines().contains(&DRIFT_HINT),
        "no drift hint:\n{}",
        run.text.show()
    );
}

/// RD-18 (iteration 2: the hint's condition is `drift::raises(state,
/// detail, false)`, its text unchanged), per state: over the tree with no
/// drifting state (`PIECE-COUNTER`'s marker raised to its revision), each
/// trigger state alone (`code_ahead`: `CAP` changed; `spec_ahead`:
/// `LAW-BOUND`'s text changed at its revision; `conflict`: both, which
/// leaves `LAW-BOUND`'s other bindings `spec_ahead`, and alone on
/// verify-ecs, `PERK-REGEN`'s text and its one system changed;
/// `broken_binding`: `data/limits.ron` deleted) prints the hint once,
/// last; the tree itself, whose rows hold every non-trigger state
/// (`cannot_verify`, `bad_marker`, `predated`, `unlocked`, `ok`), prints
/// none. M: the condition as `raises(…, true)` (a named raise: `unlocked`
/// rows hint too); a trigger state dropped from it.
#[test]
fn drift_hint_for_each_trigger_state_alone() {
    const DRIFT_HINT: &str =
        "hint: drift: commit first; spec propose drift queues HEAD's drift for the owner";
    const TRIGGERS: [&str; 4] = ["code_ahead", "spec_ahead", "conflict", "broken_binding"];
    let none = |repo: &LockRepo| {
        repo.replace(
            COUNTER,
            "@implements PIECE-COUNTER@2",
            "@implements PIECE-COUNTER@3",
        );
    };
    let cap = |repo: &LockRepo| repo.replace(COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
    let text = |repo: &LockRepo| {
        repo.replace(
            COUNTER_MD,
            "The count never exceeds the cap.",
            "The count never exceeds the cap, ever.",
        );
    };
    let states = |run: &Verify| -> std::collections::BTreeSet<String> {
        run.rows()
            .into_iter()
            .map(|row| row["state"].as_str().unwrap_or_default().to_owned())
            .collect()
    };
    let (_repo, base) = scenario("verify-hint-each-none", none);
    let present = states(&base);
    for state in ["cannot_verify", "bad_marker", "predated", "unlocked", "ok"] {
        assert!(present.contains(state), "{state} in the base: {present:?}");
    }
    assert!(
        TRIGGERS.iter().all(|state| !present.contains(*state)),
        "{present:?}"
    );
    assert!(!base.lines().contains(&DRIFT_HINT), "{}", base.text.show());
    /// A trigger state and the edit that makes it.
    type Edit<'a> = (&'a str, &'a dyn Fn(&LockRepo));
    let edits: [Edit<'_>; 4] = [
        ("code_ahead", &cap),
        ("spec_ahead", &text),
        ("conflict", &|repo: &LockRepo| {
            cap(repo);
            text(repo);
        }),
        ("broken_binding", &|repo: &LockRepo| {
            fs::remove_file(repo.root.join(LIMITS)).expect("remove");
        }),
    ];
    for (trigger, edit) in edits {
        let (_repo, run) = scenario(&format!("verify-hint-each-{trigger}"), |repo| {
            none(repo);
            edit(repo);
        });
        let triggered: Vec<String> = states(&run)
            .into_iter()
            .filter(|state| TRIGGERS.contains(&state.as_str()))
            .collect();
        // code-a's `LAW-BOUND` binds four items: its text and `CAP`
        // changed leave the other three `spec_ahead` (`conflict` alone is
        // the ECS case below).
        let want: &[&str] = if trigger == "conflict" {
            &["conflict", "spec_ahead"]
        } else {
            &[trigger]
        };
        assert_eq!(triggered, want, "{trigger}");
        let hints: Vec<&str> = run
            .lines()
            .into_iter()
            .filter(|line| line.starts_with("hint: "))
            .collect();
        assert_eq!(
            hints.iter().filter(|line| **line == DRIFT_HINT).count(),
            1,
            "{trigger}: once:\n{}",
            run.text.show()
        );
        assert_eq!(hints.last(), Some(&DRIFT_HINT), "{trigger}: last");
    }
    // `conflict` alone, the second genre: verify-ecs (an ECS game crate,
    // its four nodes accepted), `PERK-REGEN`'s text changed at its
    // revision and its one system's body changed.
    let repo = verify_common::ecs("verify-hint-each-ecs-conflict");
    repo.replace(
        verify_common::STAMINA_MD,
        "while the runner rests.",
        "while the runner rests, always.",
    );
    repo.replace(
        verify_common::STAMINA,
        "(stamina.current + rate * dt).min(stamina.max)",
        "(stamina.current + rate * dt * 2.0).min(stamina.max)",
    );
    let run = verify(&repo, &[]);
    let triggered: Vec<String> = states(&run)
        .into_iter()
        .filter(|state| TRIGGERS.contains(&state.as_str()))
        .collect();
    assert_eq!(
        triggered,
        ["conflict"],
        "conflict alone: {}",
        run.text.show()
    );
    let hints: Vec<&str> = run
        .lines()
        .into_iter()
        .filter(|line| line.starts_with("hint: "))
        .collect();
    assert_eq!(
        hints.iter().filter(|line| **line == DRIFT_HINT).count(),
        1,
        "{}",
        run.text.show()
    );
    assert_eq!(hints.last(), Some(&DRIFT_HINT), "conflict alone: last");
}

// -------------------------------------------------------------------- queue

/// RD-2: a `queue` ID (`PR-7`) and an `other_project` ID (`other:GOAL-01`)
/// are skipped and counted, never rows. M: queue IDs judged.
#[test]
fn queue_ids_are_skipped() {
    let repo = golden("verify-queue");
    let run = verify(&repo, &[]);
    assert_eq!(run.json["counts"]["skipped"], 2);
    assert!(
        run.rows_where(|row| {
            let node = row["node"].as_str().unwrap_or_default();
            node.starts_with("PR-") || node.contains(':')
        })
        .is_empty(),
        "{}",
        run.text.show()
    );
    assert!(run.summary().ends_with("; 2 skipped"), "{}", run.summary());
    // One more of each: counted, still no row.
    let (_repo, more) = scenario("verify-queue-more", |repo| {
        repo.append(
            LIB,
            "\n// @implements PR-12\n// @implements other:LAW-STEP\npub fn more() {}\n",
        );
    });
    assert_eq!(more.json["counts"]["skipped"], 4);
    assert_eq!(more.rows().len(), run.rows().len());
}

// --------------------------------------------------------------------- TE-2

/// The fixture's text files (`.rs`, `.ron`, `.md`, the config) rewritten
/// with CRLF line ends.
fn to_crlf(repo: &LockRepo) {
    let snapshot = common::snapshot(&repo.root);
    for (path, bytes) in snapshot {
        let Some(bytes) = bytes else { continue };
        if path.starts_with(".git/") || path == ".git" {
            continue;
        }
        if !(path.ends_with(".rs")
            || path.ends_with(".ron")
            || path.ends_with(".md")
            || path.ends_with(".toml"))
        {
            continue;
        }
        let text = String::from_utf8(bytes).expect("UTF-8 text");
        repo.write(&path, text.replace("\r\n", "\n").replace('\n', "\r\n"));
    }
}

/// TE-2: code-a committed with CRLF line ends, the golden's four nodes
/// accepted over it: the same hashes as the LF golden (only `file_blob`
/// and `commit` differ), the same states, lines and counters; then the
/// TE-1 scenarios that edit code or spec give the same states as LF. M: a
/// CRLF line end counted in a hash or a line.
#[test]
fn crlf_tree_verifies_as_lf() {
    let repo = LockRepo::uncommitted("verify-crlf");
    repo.git.init(&repo.root);
    to_crlf(&repo);
    repo.commit_all("the fixture, CRLF");
    for reference in ["PIECE-COUNTER", "LAW-BOUND", "cap-check/PASS-01", "GOAL-01"] {
        repo.accept_ok(reference);
    }
    let lock = repo.lock_text();
    let strip = |text: &str| {
        text.lines()
            .filter(|line| !line.starts_with("file_blob = ") && !line.starts_with("commit = "))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let golden_text = fs::read_to_string(verify_common::golden_file()).expect("golden");
    assert_eq!(
        strip(&lock),
        strip(&golden_text),
        "the same hashes over CRLF"
    );

    let base = verify(&repo, &[]);
    for (key, state, category, markers, rev) in golden_rows() {
        assert_row(
            &base,
            key,
            &Says {
                state,
                category,
                detail: None,
                pre_existing: false,
                markers: &markers,
                node_rev: Some(rev),
                lock: lock_entry(&lock, key),
                compared: (state == "ok").then_some("blob"),
            },
            None,
        );
    }
    // 39 since docs/features/symbol-discriminator.md (TE-1): the adjacent
    // `impl Tally` blocks are two keys.
    assert_eq!(base.json["counts"]["rows"], 39);
    assert_eq!(base.json["cascade"], json!({"by_blob": 9, "by_ast": 0}));

    // Scenarios over CRLF: a comment edit, a body edit, a text edit at the
    // revision, a removed marker.
    let cap = item(
        "LAW-BOUND",
        "configures",
        COUNTER,
        "crates/tally::counter::CAP",
        "const_item",
    );
    let state_of = |run: &Verify, key: verify_common::Key<'_>| {
        let row = run.row(key.node, key.relation, key.file, key.symbol);
        (
            row["state"].as_str().map(str::to_owned),
            row["category"].as_str().map(str::to_owned),
            row["compared"].as_str().map(str::to_owned),
            row["markers"].clone(),
        )
    };
    repo.replace(
        COUNTER,
        "/// The largest count; see LAW-BOUND.",
        "/// The largest count of all; see LAW-BOUND.",
    );
    let comment = verify(&repo, &[]);
    assert_eq!(
        state_of(&comment, cap),
        (
            Some("ok".into()),
            None,
            Some("ast".into()),
            json!([{"line": 8, "rev": 2}])
        )
    );
    repo.replace(COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
    let body = verify(&repo, &[]);
    assert_eq!(
        state_of(&body, cap),
        (
            Some("code_ahead".into()),
            Some("code_changed".into()),
            Some("ast".into()),
            json!([{"line": 8, "rev": 2}])
        )
    );
    repo.replace(COUNTER, "CAP: u32 = 12;", "CAP: u32 = 10;");
    repo.replace(
        COUNTER_MD,
        "The count never exceeds the cap.",
        "The count never exceeds the cap, ever.",
    );
    let text = verify(&repo, &[]);
    assert_eq!(
        state_of(&text, cap),
        (
            Some("spec_ahead".into()),
            Some("same_revision".into()),
            Some("ast".into()),
            json!([{"line": 8, "rev": 2}])
        )
    );
    repo.replace(
        COUNTER_MD,
        "The count never exceeds the cap, ever.",
        "The count never exceeds the cap.",
    );
    repo.replace(COUNTER, " // @configures LAW-BOUND@2", "");
    let gone = verify(&repo, &[]);
    assert_eq!(
        state_of(&gone, cap),
        (
            Some("broken_binding".into()),
            Some("marker_gone".into()),
            None,
            json!([])
        )
    );
}

/// TE-2: the LF golden lock over the fixture converted to CRLF and
/// committed: every item hashed equal (`ok` by `ast`); a module or RON
/// entry, compared by its blob alone, reads `code_ahead`; the spec side
/// unchanged. M: a CRLF item judged changed.
#[test]
fn lf_lock_over_a_crlf_tree_hashes_items_equal() {
    let repo = golden("verify-crlf-after");
    to_crlf(&repo);
    repo.commit_all("CRLF");
    let run = verify(&repo, &[]);
    for (key, state, category, _, _) in golden_rows() {
        let row = run.row(key.node, key.relation, key.file, key.symbol);
        let expected = match (state, key.kind) {
            ("ok", Some("item")) => ("ok", None, Some("ast")),
            ("ok", _) => ("code_ahead", Some("code_changed"), Some("blob")),
            (other, _) => (other, category, None),
        };
        assert_eq!(
            (
                row["state"].as_str().unwrap_or_default(),
                row["category"].as_str(),
                row["compared"].as_str()
            ),
            expected,
            "{key:?}"
        );
    }
    assert_eq!(run.json["cascade"], json!({"by_blob": 4, "by_ast": 5}));
}
