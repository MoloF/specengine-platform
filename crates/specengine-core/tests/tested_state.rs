//! docs/features/tested-evidence.md (8b-1) RD-4's core half ("The state"),
//! with "Bound hash"'s bytes and "The run's write"'s table as the core
//! models them (`specengine_core::lock`: pure, no hash computed here). The
//! expected values are built from "Data", never from the functions' own
//! output; the bound bytes' example is the spec's, line for line. Each
//! test names the mutation that turns it red (`M:`).

use std::cell::Cell;
use std::collections::BTreeSet;

use specengine_core::lock::{
    BOUND_TAG, EntryKey, Fingerprints, LevelCode, Lock, LockKind, LockLevel, LockRelation, Recipes,
    Tested, TestedChange, TestedNode, TestedRun, TestedState, TestedTest, bound_bytes,
    merge_tested, parse_lock, render_lock, tested_state,
};

const NORM: &str = "specengine-norm/v1";
const CODE: &str = "specengine-hash/v3 tree-sitter-rust/0.24.2 abi/15";

fn build() -> Recipes {
    Recipes {
        norm: NORM.to_owned(),
        code: CODE.to_owned(),
    }
}

fn hex(digit: char, len: usize) -> String {
    digit.to_string().repeat(len)
}

fn b3(digit: char) -> String {
    format!("b3:{}", hex(digit, 64))
}

/// A block at revision 3, `spec_hash` of `1`s, `bound_hash` of `2`s.
fn block() -> Tested {
    Tested {
        rev: 3,
        spec_hash: b3('1'),
        bound_hash: b3('2'),
        tests: [TestedTest {
            file: "crates/game/tests/stamina.rs".to_owned(),
            symbol: "crates/game::test:stamina::drain_is_clamped".to_owned(),
        }]
        .into_iter()
        .collect(),
        commit: hex('a', 40),
        verified_at: "2026-10-21".to_owned(),
    }
}

/// What a judge was asked: `(lockable, bound_hash)` calls.
struct Asked {
    lockable: Cell<usize>,
    bound: Cell<usize>,
}

impl Asked {
    fn new() -> Self {
        Self {
            lockable: Cell::new(0),
            bound: Cell::new(0),
        }
    }
}

/// `tested_state` of [`block`] with the node now `node`, the first binding
/// not lockable now `lockable`, the bound hash now `bound`; every call
/// counted in `asked`.
fn judge(node: TestedNode<'_>, lockable: Option<&str>, bound: &str, asked: &Asked) -> TestedState {
    tested_state::<String>(
        &block(),
        node,
        || {
            asked.lockable.set(asked.lockable.get() + 1);
            Ok(lockable.map(str::to_owned))
        },
        || {
            asked.bound.set(asked.bound.get() + 1);
            Ok(bound.to_owned())
        },
    )
    .expect("no error")
}

fn found(rev: Option<u32>, norm_hash: Option<&str>) -> TestedNode<'_> {
    TestedNode::Found { rev, norm_hash }
}

/// One case of the order: (context, node, lockable, bound now, state,
/// category, asked lockable, asked bound).
type Case<'a> = (
    &'a str,
    TestedNode<'a>,
    Option<&'a str>,
    &'a str,
    &'a str,
    Option<&'a str>,
    usize,
    usize,
);

/// RD-4 ("The state"; M: the order changed, or an input asked when an
/// earlier state applies): per block the first that applies, `node_gone`,
/// `unverifiable rev_unknown`, `unverifiable hash_null`, `unverifiable
/// <word>` of the first binding not lockable now, `stale spec_changed`,
/// `stale code_changed`, `verified`; `lockable` and the bound hash asked
/// only when no earlier state applies, each at most once; the words and
/// categories are the spec's.
#[test]
fn tested_state_takes_the_first_that_applies() {
    let spec = b3('1');
    let other = b3('9');
    let bound = b3('2');
    let moved = b3('8');
    let cases: Vec<Case<'_>> = vec![
        (
            "gone",
            TestedNode::Gone,
            Some("spec_ahead"),
            &moved,
            "node_gone",
            None,
            0,
            0,
        ),
        (
            "rev unreadable",
            found(None, Some(&other)),
            Some("spec_ahead"),
            &moved,
            "unverifiable",
            Some("rev_unknown"),
            0,
            0,
        ),
        (
            "hash null",
            found(Some(4), None),
            Some("spec_ahead"),
            &moved,
            "unverifiable",
            Some("hash_null"),
            0,
            0,
        ),
        (
            "a marker lagging",
            found(Some(4), Some(&other)),
            Some("spec_ahead"),
            &moved,
            "unverifiable",
            Some("spec_ahead"),
            1,
            0,
        ),
        (
            "bad levels",
            found(Some(3), Some(&spec)),
            Some("bad_levels"),
            &bound,
            "unverifiable",
            Some("bad_levels"),
            1,
            0,
        ),
        (
            "text edited",
            found(Some(3), Some(&other)),
            None,
            &moved,
            "stale",
            Some("spec_changed"),
            1,
            0,
        ),
        (
            "raised",
            found(Some(4), Some(&spec)),
            None,
            &moved,
            "stale",
            Some("spec_changed"),
            1,
            0,
        ),
        (
            "code edited",
            found(Some(3), Some(&spec)),
            None,
            &moved,
            "stale",
            Some("code_changed"),
            1,
            1,
        ),
        (
            "nothing changed",
            found(Some(3), Some(&spec)),
            None,
            &bound,
            "verified",
            None,
            1,
            1,
        ),
    ];
    let mut failures = Vec::new();
    for (context, node, lockable, now, state, category, lockable_asked, bound_asked) in cases {
        let asked = Asked::new();
        let got = judge(node, lockable, now, &asked);
        let seen = (
            got.as_str(),
            got.category().map(str::to_owned),
            asked.lockable.get(),
            asked.bound.get(),
        );
        let want = (
            state,
            category.map(str::to_owned),
            lockable_asked,
            bound_asked,
        );
        if seen != want {
            failures.push(format!("{context}: want {want:?}, got {seen:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// RD-4 **editing_the_node_stales_evidence** (M: `spec_hash` ignored): the
/// node's text edited without a raise (R the block's, `norm_hash` another)
/// is `stale spec_changed`, even with its code as it was.
#[test]
fn editing_the_node_stales_evidence() {
    let asked = Asked::new();
    let state = judge(found(Some(3), Some(&b3('9'))), None, &b3('2'), &asked);
    assert_eq!(state, TestedState::Stale("spec_changed"));
    assert_eq!(state.as_str(), "stale");
    assert_eq!(state.category(), Some("spec_changed"));
}

/// RD-4 **unverifiable_wins_over_stale** (M: `stale` checked before
/// `unverifiable`): raised with its markers lagging, the node's (R, H)
/// differ from the block's and a binding is `spec_ahead`: `unverifiable
/// spec_ahead`, never `stale`; the bound hash is not asked.
#[test]
fn unverifiable_wins_over_stale() {
    let asked = Asked::new();
    let state = judge(
        found(Some(4), Some(&b3('9'))),
        Some("spec_ahead"),
        &b3('8'),
        &asked,
    );
    assert_eq!(state, TestedState::Unverifiable("spec_ahead".to_owned()));
    assert_eq!(asked.bound.get(), 0, "the bound hash is not asked");
}

/// RD-4: an input's error ends the judge (no state).
#[test]
fn an_input_error_ends_the_judge() {
    let lockable = tested_state::<String>(
        &block(),
        found(Some(3), Some(&b3('1'))),
        || Err("cannot read the worktree".to_owned()),
        || panic!("asked after an error"),
    );
    assert_eq!(lockable, Err("cannot read the worktree".to_owned()));
    let bound = tested_state::<String>(
        &block(),
        found(Some(3), Some(&b3('1'))),
        || Ok(None),
        || Err("cannot parse".to_owned()),
    );
    assert_eq!(bound, Err("cannot parse".to_owned()));
}

// ---------------------------------------------------------- bound bytes

fn key(
    node: &str,
    relation: LockRelation,
    file: &str,
    kind: LockKind,
    symbol: &str,
    item_kind: Option<&str>,
) -> EntryKey {
    EntryKey {
        node: node.to_owned(),
        relation,
        file: file.to_owned(),
        kind,
        symbol: symbol.to_owned(),
        item_kind: item_kind.map(str::to_owned),
    }
}

/// A level code of `digit`'s digests at `levels`, with fingerprints when
/// `fingerprints`.
fn code(digit: char, levels: &[LockLevel], fingerprints: bool) -> LevelCode {
    LevelCode {
        levels: levels.iter().copied().collect(),
        sig_hash: format!("b3:a{}", hex(digit, 63)),
        body_hash: format!("b3:b{}", hex(digit, 63)),
        deps_hash: format!("b3:c{}", hex(digit, 63)),
        fingerprints: fingerprints.then(|| Fingerprints {
            skeleton_hash: format!("b3:d{}", hex(digit, 63)),
            quote_hash: format!("b3:e{}", hex(digit, 63)),
            body_bytes: 212,
        }),
    }
}

/// The spec's bound bytes, by hand: the tag, then per binding in key order
/// its key lines and three digest lines, then an empty line.
fn spelled(bindings: &[(&EntryKey, char)]) -> String {
    let mut out = format!("{BOUND_TAG}\n");
    for (key, digit) in bindings {
        out.push_str(&format!(
            "node = \"{}\"\nrelation = \"{}\"\nfile = \"{}\"\nkind = \"{}\"\nsymbol = \"{}\"\n",
            key.node,
            key.relation.as_str(),
            key.file,
            key.kind.as_str(),
            key.symbol
        ));
        if let Some(item_kind) = &key.item_kind {
            out.push_str(&format!("item_kind = \"{item_kind}\"\n"));
        }
        out.push_str(&format!(
            "sig_hash = \"b3:a{0}\"\nbody_hash = \"b3:b{0}\"\ndeps_hash = \"b3:c{0}\"\n\n",
            hex(*digit, 63)
        ));
    }
    out
}

/// RD-3's core half ("Bound hash"; M: a level list, a fingerprint, a
/// `file_blob` or a revision in the bytes; the order of the caller kept):
/// the tag `specengine-tested/v1`, then each binding of every relation in
/// `EntryKey` order (`implements` < `verifies` < `configures`, then file,
/// kind, symbol bytes), its key lines (`item_kind` only when present) and
/// three digests, an empty line after each, LF-ended; the same bytes
/// whatever the levels or fingerprints; a key given twice once.
#[test]
fn bound_bytes_are_the_specs() {
    assert_eq!(BOUND_TAG, "specengine-tested/v1");
    let regen = key(
        "R-12",
        LockRelation::Implements,
        "crates/game/src/stamina.rs",
        LockKind::Item,
        "crates/game::stamina::regen_system",
        Some("function_item"),
    );
    let module = key(
        "R-12",
        LockRelation::Implements,
        "crates/game/src/stamina.rs",
        LockKind::Module,
        "crates/game::stamina",
        None,
    );
    let test = key(
        "R-12",
        LockRelation::Verifies,
        "crates/game/tests/stamina.rs",
        LockKind::Item,
        "crates/game::test:stamina::drain_is_clamped",
        Some("function_item"),
    );
    let tuning = key(
        "R-12",
        LockRelation::Configures,
        "assets/data/stamina.ron",
        LockKind::Ron,
        "root.stamina.regen_per_second",
        None,
    );
    let want = spelled(&[(&regen, '1'), (&module, '2'), (&test, '3'), (&tuning, '4')]);
    let sig_body = [LockLevel::Sig, LockLevel::Body];
    let codes = [
        code('1', &sig_body, true),
        code('2', &sig_body, true),
        code('3', &sig_body, true),
        code('4', &sig_body, false),
    ];
    let given = [
        (&tuning, &codes[3]),
        (&test, &codes[2]),
        (&module, &codes[1]),
        (&regen, &codes[0]),
    ];
    assert_eq!(
        bound_bytes(given),
        want,
        "in key order, whatever the caller's"
    );

    // The spec's own example, verbatim.
    let example = key(
        "R-12",
        LockRelation::Implements,
        "crates/game/src/stamina.rs",
        LockKind::Item,
        "crates/game::stamina::regen_system",
        Some("function_item"),
    );
    let mut example_code = code('0', &sig_body, true);
    example_code.sig_hash = format!("b3:41aa{}", hex('0', 60));
    example_code.body_hash = format!("b3:07c3{}", hex('0', 60));
    example_code.deps_hash = format!("b3:d1e0{}", hex('0', 60));
    assert_eq!(
        bound_bytes([(&example, &example_code)]),
        format!(
            "specengine-tested/v1\nnode = \"R-12\"\nrelation = \"implements\"\nfile = \"crates/game/src/stamina.rs\"\nkind = \"item\"\nsymbol = \"crates/game::stamina::regen_system\"\nitem_kind = \"function_item\"\nsig_hash = \"b3:41aa{0}\"\nbody_hash = \"b3:07c3{0}\"\ndeps_hash = \"b3:d1e0{0}\"\n\n",
            hex('0', 60)
        )
    );

    // Levels and fingerprints never count.
    let narrowed = [
        (&regen, code('1', &[LockLevel::Sig], false)),
        (&module, code('2', &[LockLevel::Path], true)),
        (&test, code('3', &LockLevel::ALL, false)),
        (&tuning, code('4', &[LockLevel::Deps], false)),
    ];
    assert_eq!(
        bound_bytes(narrowed.iter().map(|(key, code)| (*key, code))),
        want,
        "levels and fingerprints are not hashed"
    );

    // A key from several markers once.
    let twice = [(&regen, &codes[0]), (&regen, &codes[0]), (&test, &codes[2])];
    assert_eq!(
        bound_bytes(twice),
        spelled(&[(&regen, '1'), (&test, '3')]),
        "a key once"
    );
    assert_eq!(bound_bytes(std::iter::empty()), format!("{BOUND_TAG}\n"));
}

// --------------------------------------------------------------- merge

fn lock_with_entry() -> Lock {
    let text = format!(
        "# spec.lock: written only by SpecEngine (spec lock, spec approve, spec verify --tests); do not edit\nformat = 2\nnorm_recipe = \"{NORM}\"\ncode_recipe = \"{CODE}\"\n\n[[binding]]\nnode = \"R-12\"\nrev = 3\nspec_hash = \"{}\"\nrelation = \"implements\"\nfile = \"crates/game/src/stamina.rs\"\nkind = \"module\"\nsymbol = \"crates/game::stamina\"\nlevels = [\"sig\", \"body\"]\nsig_hash = \"{}\"\nbody_hash = \"{}\"\ndeps_hash = \"{}\"\nskeleton_hash = \"{}\"\nquote_hash = \"{}\"\nbody_bytes = 12\nfile_blob = \"{}\"\ncommit = \"{}\"\nverified_by = \"owner\"\nverified_at = \"2026-10-08\"\n",
        b3('1'),
        b3('3'),
        b3('4'),
        b3('5'),
        b3('6'),
        b3('7'),
        hex('8', 40),
        hex('9', 40)
    );
    parse_lock(&text, Some(&build())).unwrap_or_else(|error| panic!("{error}"))
}

/// A block of `node` at revision `rev`, `bound` its bound hash, the run at
/// `commit` on `day`.
fn evidence(rev: u32, bound: char, commit: char, day: &str) -> Tested {
    Tested {
        rev,
        spec_hash: b3('1'),
        bound_hash: b3(bound),
        tests: [TestedTest {
            file: "crates/game/tests/stamina.rs".to_owned(),
            symbol: "crates/game::test:stamina::drain_is_clamped".to_owned(),
        }]
        .into_iter()
        .collect::<BTreeSet<_>>(),
        commit: hex(commit, 40),
        verified_at: day.to_owned(),
    }
}

/// RD-5's core half ("The write"'s table; M: an equal block rewritten, a
/// failed node's block kept, an entry or the recipes touched): a verified
/// node without a block `added`; with another `rev`, `spec_hash`,
/// `bound_hash` or `tests` `refreshed`; equal on those four `unchanged`,
/// its bytes kept (`commit`, `verified_at` the old run's); a failed node's
/// block `removed`; a failed node without one no row; nodes not given
/// untouched; rows by node bytes; the entries and recipes the old lock's;
/// `unchanged` iff every row is; `changed` the rest by bytes.
#[test]
fn merge_tested_follows_the_table() {
    let mut old = lock_with_entry();
    old.tested
        .insert("R-12".to_owned(), evidence(3, '2', 'a', "2026-10-20"));
    old.tested
        .insert("R-2".to_owned(), evidence(3, '2', 'a', "2026-10-20"));
    old.tested
        .insert("R-7".to_owned(), evidence(3, '2', 'a', "2026-10-20"));
    old.tested
        .insert("Z-1".to_owned(), evidence(3, '2', 'a', "2026-10-20"));
    let merged = merge_tested(
        &old,
        [
            (
                "R-2".to_owned(),
                TestedRun::Verified(evidence(3, '2', 'b', "2026-10-21")),
            ),
            (
                "R-12".to_owned(),
                TestedRun::Verified(evidence(3, '5', 'b', "2026-10-21")),
            ),
            (
                "A-1".to_owned(),
                TestedRun::Verified(evidence(1, '6', 'b', "2026-10-21")),
            ),
            ("R-7".to_owned(), TestedRun::Failed),
            ("B-1".to_owned(), TestedRun::Failed),
        ],
    );
    assert_eq!(
        merged.rows,
        vec![
            ("A-1".to_owned(), TestedChange::Added),
            ("R-12".to_owned(), TestedChange::Refreshed),
            ("R-2".to_owned(), TestedChange::Unchanged),
            ("R-7".to_owned(), TestedChange::Removed),
        ]
    );
    assert_eq!(merged.changed(), ["A-1", "R-12", "R-7"]);
    assert!(!merged.unchanged());
    assert_eq!(
        merged.lock.tested.get("R-2"),
        Some(&evidence(3, '2', 'a', "2026-10-20")),
        "unchanged: the old run's commit and day kept"
    );
    assert_eq!(
        merged.lock.tested.get("R-12"),
        Some(&evidence(3, '5', 'b', "2026-10-21"))
    );
    assert!(!merged.lock.tested.contains_key("R-7"), "removed");
    assert!(!merged.lock.tested.contains_key("B-1"), "no block made");
    assert_eq!(
        merged.lock.tested.get("Z-1"),
        old.tested.get("Z-1"),
        "a node not given untouched"
    );
    assert_eq!(merged.lock.entries, old.entries, "entries untouched");
    assert_eq!(merged.lock.recipes, old.recipes);
    assert_eq!(merged.lock.format, old.format);
    for (change, word) in [
        (TestedChange::Added, "added"),
        (TestedChange::Refreshed, "refreshed"),
        (TestedChange::Unchanged, "unchanged"),
        (TestedChange::Removed, "removed"),
    ] {
        assert_eq!(change.as_str(), word);
    }

    let same = merge_tested(
        &old,
        [(
            "R-2".to_owned(),
            TestedRun::Verified(evidence(3, '2', 'c', "2026-10-22")),
        )],
    );
    assert!(same.unchanged(), "every row unchanged");
    assert_eq!(render_lock(&same.lock), render_lock(&old), "the bytes kept");
    let none = merge_tested(&old, [("B-1".to_owned(), TestedRun::Failed)]);
    assert!(
        none.rows.is_empty() && !none.unchanged(),
        "no row: not unchanged"
    );

    // Every key of the four counts: `tests` too.
    let mut other_tests = evidence(3, '2', 'd', "2026-10-22");
    other_tests.tests.insert(TestedTest {
        file: "crates/game/tests/stamina.rs".to_owned(),
        symbol: "crates/game::test:stamina::regen_stops".to_owned(),
    });
    let rev_raised = evidence(4, '2', 'd', "2026-10-22");
    let mut text_changed = evidence(3, '2', 'd', "2026-10-22");
    text_changed.spec_hash = b3('7');
    for (context, now) in [
        ("tests", other_tests),
        ("rev", rev_raised),
        ("spec_hash", text_changed),
    ] {
        let merged = merge_tested(&old, [("R-2".to_owned(), TestedRun::Verified(now))]);
        assert_eq!(
            merged.rows,
            vec![("R-2".to_owned(), TestedChange::Refreshed)],
            "{context}"
        );
    }
}

/// RD-7's core half (Q-E: a block counts as one entry): a block vouches
/// for a node at its `rev` and `spec_hash` as an entry does, and only
/// there.
#[test]
fn a_block_vouches_like_an_entry() {
    let block = block();
    assert!(block.vouches(3, &b3('1')));
    assert!(!block.vouches(4, &b3('1')), "another revision");
    assert!(!block.vouches(3, &b3('9')), "another meaning");
    let mut lock = Lock::empty(build());
    assert!(!lock.holds("R-12"));
    lock.tested.insert("R-12".to_owned(), block);
    assert!(lock.holds("R-12"), "a block alone holds its node");
    assert_eq!(lock.format_number(), 3);
    lock.tested.clear();
    assert_eq!(lock.format_number(), 2);
}
