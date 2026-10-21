//! docs/features/impact-since.md "Core comparer" (7b-2, Q-G, Q-H, RD-6):
//! the pure comparer `specengine_core::symbol_delta::compare(old, new)`
//! over records built here by hand, digests opaque strings compared for
//! equality only.
//!
//! - Key `(file, qpath, item_kind)`; records sharing a key on a side pair
//!   with the other side's in (line, given order), the surplus added or
//!   removed.
//! - `added` indexes `new`, `removed` indexes `old`, `changed` holds
//!   `(old, new)`: a pair whose `sig` or `body` differs, or with a `None`
//!   on either side (never read as unchanged).
//! - Each list by (file bytes, line, qpath, item_kind), a pair's line the
//!   new side's; then the input position. The input order never changes
//!   which records come out, nor their order.

use specengine_core::symbol_delta::{Compared, SymbolRecord, compare};

/// A record with both digests.
fn r(file: &str, qpath: &str, kind: &str, line: usize, sig: &str, body: &str) -> SymbolRecord {
    SymbolRecord {
        file: file.to_owned(),
        qpath: qpath.to_owned(),
        item_kind: kind.to_owned(),
        line,
        sig: Some(sig.to_owned()),
        body: Some(body.to_owned()),
    }
}

/// A function item of `file` at `line`, digests `s`, `b`.
fn f(file: &str, qpath: &str, line: usize, s: &str, b: &str) -> SymbolRecord {
    r(file, qpath, "function_item", line, s, b)
}

/// What `compared` names, as records: `(added, removed, changed)`.
type Named = (
    Vec<SymbolRecord>,
    Vec<SymbolRecord>,
    Vec<(SymbolRecord, SymbolRecord)>,
);

fn named(compared: &Compared, old: &[SymbolRecord], new: &[SymbolRecord]) -> Named {
    (
        compared.added.iter().map(|&at| new[at].clone()).collect(),
        compared.removed.iter().map(|&at| old[at].clone()).collect(),
        compared
            .changed
            .iter()
            .map(|&(was, is)| (old[was].clone(), new[is].clone()))
            .collect(),
    )
}

/// A deterministic permutation of `items` (a fixed multiplier modulo a
/// prime walk), never the identity for more than two items.
fn shuffled<T: Clone>(items: &[T], seed: usize) -> Vec<T> {
    let n = items.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&at| (at * 7919 + seed * 104_729) % 1_000_003);
    order.reverse();
    order.into_iter().map(|at| items[at].clone()).collect()
}

#[test]
fn equal_sides_compare_empty() {
    let side = vec![
        f("a.rs", "p::a", 1, "s1", "b1"),
        f("a.rs", "p::b", 5, "s2", "b2"),
        r("data/x.ron", "data/x.ron", "file", 1, "h", "h"),
    ];
    let compared = compare(&side, &side);
    assert_eq!(compared, Compared::default());
    assert!(compared.is_empty());
    assert!(compare(&[], &[]).is_empty());
}

/// A changed `sig` or `body` is `changed`; a moved line alone is not
/// (the key holds no line).
#[test]
fn sig_or_body_differing_is_changed_a_line_is_not() {
    let old = vec![
        f("a.rs", "p::sig", 1, "s", "b"),
        f("a.rs", "p::body", 5, "s", "b"),
        f("a.rs", "p::moved", 9, "s", "b"),
        f("a.rs", "p::both", 14, "s", "b"),
    ];
    let new = vec![
        f("a.rs", "p::sig", 1, "S", "b"),
        f("a.rs", "p::body", 5, "s", "B"),
        f("a.rs", "p::moved", 40, "s", "b"),
        f("a.rs", "p::both", 14, "S", "B"),
    ];
    let compared = compare(&old, &new);
    assert_eq!(compared.added, Vec::<usize>::new());
    assert_eq!(compared.removed, Vec::<usize>::new());
    assert_eq!(compared.changed, vec![(0, 0), (1, 1), (3, 3)]);
    assert!(!compared.is_empty());
}

/// A digest that could not be made (`None`) on either side, or both, is
/// `changed` (never read as unchanged); a `None` beside a made digest
/// too.
#[test]
fn an_unhashed_side_is_changed() {
    let mut none_old = f("a.rs", "p::x", 1, "s", "b");
    none_old.sig = None;
    let mut none_new = f("a.rs", "p::y", 3, "s", "b");
    none_new.body = None;
    let mut both = f("a.rs", "p::z", 5, "s", "b");
    both.sig = None;
    both.body = None;
    let old = vec![
        none_old.clone(),
        f("a.rs", "p::y", 3, "s", "b"),
        both.clone(),
    ];
    let new = vec![f("a.rs", "p::x", 1, "s", "b"), none_new, both];
    assert_eq!(compare(&old, &new).changed, vec![(0, 0), (1, 1), (2, 2)]);
    // Equal `None`s are no proof either.
    assert_eq!(
        compare(
            std::slice::from_ref(&none_old),
            std::slice::from_ref(&none_old)
        )
        .changed,
        vec![(0, 0)]
    );
}

/// `added` indexes `new` (the new side's line), `removed` indexes `old`
/// (the old side's).
#[test]
fn added_and_removed_index_their_own_side() {
    let old = vec![
        f("a.rs", "p::kept", 1, "s", "b"),
        f("a.rs", "p::gone", 7, "s", "b"),
    ];
    let new = vec![
        f("a.rs", "p::fresh", 2, "s", "b"),
        f("a.rs", "p::kept", 1, "s", "b"),
    ];
    let compared = compare(&old, &new);
    assert_eq!(compared.added, vec![0]);
    assert_eq!(compared.removed, vec![1]);
    assert!(compared.changed.is_empty());
}

/// The key is `(file, qpath, item_kind)`: the same qpath in another file
/// (a module moved) or of another kind is removed plus added, whatever
/// the digests.
#[test]
fn the_key_holds_file_qpath_and_kind() {
    let old = vec![
        f("src/a.rs", "p::item", 1, "s", "b"),
        r("src/b.rs", "p::shape", "struct_item", 3, "s", "b"),
    ];
    let new = vec![
        f("src/moved/a.rs", "p::item", 1, "s", "b"),
        r("src/b.rs", "p::shape", "enum_item", 3, "s", "b"),
    ];
    let compared = compare(&old, &new);
    let (added, removed, changed) = named(&compared, &old, &new);
    assert_eq!(added, vec![new[1].clone(), new[0].clone()]);
    assert_eq!(removed, vec![old[0].clone(), old[1].clone()]);
    assert!(changed.is_empty());
}

/// Records sharing a key pair in (line, given order): the first old twin
/// with the first new twin by line, the surplus added (or removed); given
/// out of line order, the line decides; on one line, the given order.
#[test]
fn twins_pair_in_line_then_given_order() {
    let k = |line: usize, b: &str| f("a.rs", "p::impl T", line, "s", b);
    // Old twins given out of line order.
    let old = vec![k(20, "second"), k(10, "first")];
    let new = vec![k(11, "first"), k(30, "third"), k(21, "SECOND")];
    let compared = compare(&old, &new);
    // (old line 10, new line 11) equal; (old 20, new 21) changed; new 30
    // surplus.
    assert_eq!(compared.changed, vec![(0, 2)]);
    assert_eq!(compared.added, vec![1]);
    assert!(compared.removed.is_empty());
    // The surplus removed when the old side holds more.
    let compared = compare(&new, &old);
    assert_eq!(compared.removed, vec![1]);
    assert_eq!(compared.changed, vec![(2, 0)]);
    // On one line, the given order pairs them.
    let old = vec![k(5, "x"), k(5, "y")];
    let new = vec![k(5, "x"), k(5, "Y")];
    assert_eq!(compare(&old, &new).changed, vec![(1, 1)]);
}

/// Each list by file bytes (`B` before `a`, `a-b` before `a/b`), line
/// (numeric: 9 before 10), qpath, item_kind; a changed pair by the new
/// side's line.
#[test]
fn lists_are_in_file_line_qpath_kind_order() {
    let new = vec![
        f("a/b.rs", "p::z", 1, "s", "b"),
        f("a-b.rs", "p::z", 1, "s", "b"),
        f("a.rs", "p::late", 10, "s", "b"),
        f("a.rs", "p::early", 9, "s", "b"),
        r("a.rs", "p::same", "struct_item", 4, "s", "b"),
        r("a.rs", "p::same", "impl_item", 4, "s", "b"),
        f("a.rs", "p::b", 4, "s", "b"),
        f("B.rs", "p::z", 1, "s", "b"),
    ];
    let compared = compare(&[], &new);
    let (added, _, _) = named(&compared, &[], &new);
    let order: Vec<(String, usize, String, String)> = added
        .iter()
        .map(|item| {
            (
                item.file.clone(),
                item.line,
                item.qpath.clone(),
                item.item_kind.clone(),
            )
        })
        .collect();
    let want: Vec<(String, usize, String, String)> = [
        ("B.rs", 1, "p::z", "function_item"),
        ("a-b.rs", 1, "p::z", "function_item"),
        ("a.rs", 4, "p::b", "function_item"),
        ("a.rs", 4, "p::same", "impl_item"),
        ("a.rs", 4, "p::same", "struct_item"),
        ("a.rs", 9, "p::early", "function_item"),
        ("a.rs", 10, "p::late", "function_item"),
        ("a/b.rs", 1, "p::z", "function_item"),
    ]
    .iter()
    .map(|(file, line, qpath, kind)| {
        (
            (*file).to_owned(),
            *line,
            (*qpath).to_owned(),
            (*kind).to_owned(),
        )
    })
    .collect();
    assert_eq!(order, want);
    // The same order for `removed`.
    let compared = compare(&new, &[]);
    let (_, removed, _) = named(&compared, &new, &[]);
    assert_eq!(removed, added);

    // A changed pair sits at the new side's line.
    let old = vec![
        f("a.rs", "p::one", 50, "s", "b"),
        f("a.rs", "p::two", 1, "s", "b"),
    ];
    let new = vec![
        f("a.rs", "p::one", 2, "s", "B"),
        f("a.rs", "p::two", 60, "s", "B"),
    ];
    assert_eq!(compare(&old, &new).changed, vec![(0, 0), (1, 1)]);
}

/// Shuffled input, equal output: over a mixed pair of sides (changed,
/// added, removed, twins on distinct lines, several files, a whole-file
/// record), every permutation names the same records in the same order.
#[test]
fn input_order_never_matters() {
    let old = vec![
        f("src/a.rs", "p::a::one", 3, "s", "b"),
        f("src/a.rs", "p::a::two", 9, "s", "b"),
        f("src/a.rs", "p::a::impl T", 12, "s", "b"),
        f("src/a.rs", "p::a::impl T", 30, "s", "b"),
        f("src/b.rs", "p::b::gone", 1, "s", "b"),
        r("data/x.ron", "data/x.ron", "file", 1, "h1", "h1"),
        f("src/c.rs", "p::c::kept", 4, "s", "b"),
    ];
    let new = vec![
        f("src/a.rs", "p::a::one", 3, "s", "B"),
        f("src/a.rs", "p::a::two", 10, "s", "b"),
        f("src/a.rs", "p::a::impl T", 12, "s", "b"),
        f("src/a.rs", "p::a::impl T", 31, "S", "b"),
        f("src/a.rs", "p::a::impl T", 40, "s", "b"),
        r("data/x.ron", "data/x.ron", "file", 1, "h2", "h2"),
        f("src/c.rs", "p::c::kept", 4, "s", "b"),
        f("src/d.rs", "p::d::fresh", 2, "s", "b"),
    ];
    let base = named(&compare(&old, &new), &old, &new);
    assert_eq!(base.0.len(), 2, "added: {base:#?}");
    assert_eq!(base.1.len(), 1, "removed: {base:#?}");
    assert_eq!(base.2.len(), 3, "changed: {base:#?}");
    for seed in 0..12 {
        let old_s = shuffled(&old, seed);
        let new_s = shuffled(&new, seed + 5);
        let mut old_r = old.clone();
        old_r.reverse();
        let mut new_r = new.clone();
        new_r.reverse();
        assert_eq!(
            named(&compare(&old_s, &new_s), &old_s, &new_s),
            base,
            "seed {seed}"
        );
        assert_eq!(
            named(&compare(&old_r, &new_r), &old_r, &new_r),
            base,
            "reversed"
        );
    }
}

/// Every index is in range and each record is named at most once.
#[test]
fn indexes_are_in_range_and_unique() {
    let old: Vec<SymbolRecord> = (0..200)
        .map(|n| {
            f(
                &format!("f{}.rs", n % 7),
                &format!("p::i{}", n % 13),
                n,
                "s",
                &format!("b{}", n % 3),
            )
        })
        .collect();
    let new: Vec<SymbolRecord> = (0..230)
        .map(|n| {
            f(
                &format!("f{}.rs", n % 7),
                &format!("p::i{}", n % 11),
                n,
                "s",
                &format!("b{}", n % 5),
            )
        })
        .collect();
    let compared = compare(&old, &new);
    let mut olds: Vec<usize> = compared.removed.clone();
    olds.extend(compared.changed.iter().map(|&(was, _)| was));
    let mut news: Vec<usize> = compared.added.clone();
    news.extend(compared.changed.iter().map(|&(_, is)| is));
    for (seen, len, side) in [(&mut olds, old.len(), "old"), (&mut news, new.len(), "new")] {
        assert!(seen.iter().all(|&at| at < len), "{side}: in range");
        let count = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), count, "{side}: each once");
    }
    // Every record not named is paired equal with one of the other side.
    let unnamed_old = old.len() - olds.len();
    let unnamed_new = new.len() - news.len();
    assert_eq!(unnamed_old, unnamed_new, "equal pairs, one of each side");
}
