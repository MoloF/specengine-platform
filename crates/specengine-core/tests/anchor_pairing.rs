//! docs/features/anchor-states.md RD-1 ("The pairing", "Core API"): the
//! pure pairing of lock entries whose key no marker gives any more with
//! marker bindings that have no entry, over fingerprint strings built here
//! by hand. Per group (an opaque key), over the prints whose `body_bytes`
//! reach the caller's threshold: C0(e) the bindings with an equal `body`,
//! else those of e's file with an equal `skeleton`; C(e) narrowed to the
//! one with an equal `quote` only when exactly one is left; R(b) the
//! entries whose C holds b, body matches outranking skeleton matches, then
//! the same quote rule; MOVED iff C(e) = {b} and R(b) = {e}, else
//! AMBIGUOUS (candidates C(e), ascending) when C(e) is not empty, else
//! LOST.
//!
//! Core cannot read the code crate's triviality constant ("Fixtures"):
//! every test passes its own threshold. Each named test says the mutation
//! that turns it red (`M:`).

use specengine_core::anchor::{Follow, Print, pair};

/// The threshold most tests pass: their own, not the code crate's.
const T: u32 = 40;

/// Thresholds the boundary tests sweep: none of them is special.
const SWEEP: [u32; 4] = [1, T, 97, 300];

/// One print as written here: its group, file, the three fingerprints and
/// its `body_bytes`.
#[derive(Debug, Clone, Copy)]
struct P {
    group: &'static str,
    file: &'static str,
    body: &'static str,
    skeleton: &'static str,
    quote: &'static str,
    bytes: u32,
}

/// A non-trivial print of group `g` (`bytes` well over [`T`]).
fn p(
    group: &'static str,
    file: &'static str,
    body: &'static str,
    skeleton: &'static str,
    quote: &'static str,
) -> P {
    P {
        group,
        file,
        body,
        skeleton,
        quote,
        bytes: T + 60,
    }
}

impl P {
    fn sized(self, bytes: u32) -> Self {
        Self { bytes, ..self }
    }
}

fn prints(list: &[P]) -> Vec<Print<'static, &'static str>> {
    list.iter()
        .map(|print| Print {
            group: print.group,
            file: print.file,
            body: print.body,
            skeleton: print.skeleton,
            quote: print.quote,
            body_bytes: print.bytes,
        })
        .collect()
}

/// `pair` over `entries` and `bindings` at threshold `t`, checked to give
/// one follow per entry.
fn run(entries: &[P], bindings: &[P], t: u32) -> Vec<Follow> {
    let follows = pair(&prints(entries), &prints(bindings), t);
    assert_eq!(
        follows.len(),
        entries.len(),
        "one follow per entry: {follows:?}"
    );
    follows
}

const A: &str = "src/a.rs";
const B: &str = "src/b.rs";
const G: &str = "N-1 implements item function_item";

// ------------------------------------------------------------ shape

/// "Core API": one `Follow` per entry, in the entries' order; no binding:
/// every entry `Lost`; no entry: nothing. M: follows in binding order.
#[test]
fn one_follow_per_entry_in_order() {
    assert_eq!(
        run(&[], &[p(G, A, "b1", "s1", "q1")], T),
        Vec::<Follow>::new()
    );
    assert_eq!(
        run(
            &[p(G, A, "b1", "s1", "q1"), p(G, A, "b2", "s2", "q2")],
            &[],
            T
        ),
        vec![Follow::Lost, Follow::Lost]
    );
    // Entries in one order, their matches in the other.
    assert_eq!(
        run(
            &[
                p(G, A, "b1", "s1", "q1"),
                p(G, A, "nothing", "nothing", "q9"),
                p(G, A, "b2", "s2", "q2"),
            ],
            &[p(G, B, "b2", "x2", "y2"), p(G, B, "b1", "x1", "y1")],
            T
        ),
        vec![Follow::Moved(1), Follow::Lost, Follow::Moved(0)]
    );
}

// --------------------------------------------------------------- M1

/// M1 (uniqueness checked one way: only C(e) = {b}): two entries of one
/// group whose bodies equal the one binding's, no quote equal: each C(e)
/// is {b}, but R(b) holds both, so neither moves; each is AMBIGUOUS with
/// that one candidate. The other way round, one entry and two body-equal
/// bindings: AMBIGUOUS with both. When one entry's quote equals the
/// binding's, R(b) narrows to it: it moves, the other stays AMBIGUOUS.
#[test]
fn contested_candidate_moves_nothing() {
    let binding = p(G, B, "same body", "sk", "q binding");
    assert_eq!(
        run(
            &[
                p(G, A, "same body", "sk", "q first"),
                p(G, A, "same body", "sk", "q second"),
            ],
            &[binding],
            T
        ),
        vec![Follow::Ambiguous(vec![0]), Follow::Ambiguous(vec![0])],
        "two entries, one binding"
    );
    assert_eq!(
        run(
            &[p(G, A, "same body", "sk", "q entry")],
            &[
                p(G, B, "same body", "sk", "q one"),
                p(G, A, "same body", "sk", "q two"),
            ],
            T
        ),
        vec![Follow::Ambiguous(vec![0, 1])],
        "one entry, two bindings"
    );
    assert_eq!(
        run(
            &[
                p(G, A, "same body", "sk", "q first"),
                p(G, A, "same body", "sk", "q binding"),
            ],
            &[binding],
            T
        ),
        vec![Follow::Ambiguous(vec![0]), Follow::Moved(0)],
        "the binding's quote picks one entry"
    );
}

// --------------------------------------------------------------- M2

/// M2 (`<=` for `<`): a body of exactly `t` bytes on both sides is not
/// trivial: followed by `body`, and by `skeleton` in the entry's file, at
/// every threshold of the sweep.
#[test]
fn body_at_the_threshold_moves() {
    for t in SWEEP {
        let entry = p(G, A, "body", "sk", "q").sized(t);
        assert_eq!(
            run(&[entry], &[p(G, B, "body", "other", "q2").sized(t)], t),
            vec![Follow::Moved(0)],
            "t = {t}: by body"
        );
        assert_eq!(
            run(&[entry], &[p(G, A, "edited body", "sk", "q2").sized(t)], t),
            vec![Follow::Moved(0)],
            "t = {t}: by skeleton"
        );
    }
}

/// M2's other side: a body of `t - 1` bytes on either side never takes
/// part, the entry `Lost`, at every threshold of the sweep.
#[test]
fn body_under_the_threshold_is_never_followed() {
    for t in SWEEP {
        let under = t - 1;
        for (entry, binding, context) in [
            (under, under, "both"),
            (under, t, "the entry"),
            (t, under, "the binding"),
        ] {
            assert_eq!(
                run(
                    &[p(G, A, "body", "sk", "q").sized(entry)],
                    &[p(G, B, "body", "sk", "q").sized(binding)],
                    t
                ),
                vec![Follow::Lost],
                "t = {t}: {context} under the threshold, by body"
            );
            assert_eq!(
                run(
                    &[p(G, A, "body", "sk", "q").sized(entry)],
                    &[p(G, A, "edited", "sk", "q").sized(binding)],
                    t
                ),
                vec![Follow::Lost],
                "t = {t}: {context} under the threshold, by skeleton"
            );
        }
    }
}

// --------------------------------------------------------------- M3

/// M3 (the skeleton matched across files): a binding of another file with
/// the entry's skeleton (and quote) but another body is never a
/// candidate: `Lost`; beside one of the entry's own file it is ignored,
/// that one followed.
#[test]
fn skeleton_step_stays_in_the_entry_file() {
    let entry = p(G, A, "old body", "skeleton", "quote");
    let elsewhere = p(G, B, "new body", "skeleton", "quote");
    assert_eq!(
        run(&[entry], &[elsewhere], T),
        vec![Follow::Lost],
        "another file only"
    );
    assert_eq!(
        run(
            &[entry],
            &[
                elsewhere,
                p(G, A, "edited body", "skeleton", "another quote")
            ],
            T
        ),
        vec![Follow::Moved(1)],
        "the entry's file wins"
    );
}

// --------------------------------------------------------------- M4

/// M4 (a candidate taken while several remain after the quote): two
/// entries and two bindings, every body, skeleton and quote equal: both
/// AMBIGUOUS with both candidates; one entry before two quote-equal
/// bindings alike.
#[test]
fn twins_with_equal_quotes_stay_ambiguous() {
    let twin = p(G, A, "twin body", "twin skeleton", "twin quote");
    assert_eq!(
        run(&[twin, twin], &[twin, twin], T),
        vec![Follow::Ambiguous(vec![0, 1]), Follow::Ambiguous(vec![0, 1])],
        "two and two"
    );
    assert_eq!(
        run(&[twin], &[twin, twin], T),
        vec![Follow::Ambiguous(vec![0, 1])],
        "one and two"
    );
}

/// Step 2 replaces C0(e) only when exactly one candidate has the entry's
/// quote: two of three quote-equal leave C(e) = C0(e), all three the
/// candidates.
#[test]
fn quote_narrows_only_to_one() {
    let entry = p(G, A, "body", "sk", "quote");
    assert_eq!(
        run(
            &[entry],
            &[
                p(G, A, "body", "sk", "quote"),
                p(G, B, "body", "sk", "another"),
                p(G, B, "body", "sk", "quote"),
            ],
            T
        ),
        vec![Follow::Ambiguous(vec![0, 1, 2])]
    );
    assert_eq!(
        run(
            &[entry],
            &[
                p(G, A, "body", "sk", "another"),
                p(G, B, "body", "sk", "quote"),
                p(G, B, "body", "sk", "a third"),
            ],
            T
        ),
        vec![Follow::Moved(1)],
        "exactly one left"
    );
}

// ---------------------------------------------------------- unmutated

/// A body match outranks a skeleton match, both ways. An entry with a
/// body-equal binding in another file and a skeleton- and quote-equal one
/// in its own: followed by `body`. The R(b) rule: a binding body-equal to
/// one entry and skeleton-equal (same file) to another whose quote equals
/// the binding's: R(b) keeps the body match before the quote is read, so
/// the body-equal entry moves and the other stays AMBIGUOUS.
#[test]
fn body_match_wins_over_skeleton() {
    assert_eq!(
        run(
            &[p(G, A, "body", "skeleton", "quote")],
            &[
                p(G, B, "body", "other skeleton", "other quote"),
                p(G, A, "edited", "skeleton", "quote"),
            ],
            T
        ),
        vec![Follow::Moved(0)],
        "the entry's side"
    );
    assert_eq!(
        run(
            &[
                p(G, A, "body", "skeleton", "first quote"),
                p(G, A, "another body", "skeleton", "binding quote"),
            ],
            &[p(G, A, "body", "skeleton", "binding quote")],
            T
        ),
        vec![Follow::Moved(0), Follow::Ambiguous(vec![0])],
        "the binding's side (R(b))"
    );
    // The same with the entries given the other way round.
    assert_eq!(
        run(
            &[
                p(G, A, "another body", "skeleton", "binding quote"),
                p(G, A, "body", "skeleton", "first quote"),
            ],
            &[p(G, A, "body", "skeleton", "binding quote")],
            T
        ),
        vec![Follow::Ambiguous(vec![0]), Follow::Moved(0)],
        "the binding's side, reversed"
    );
}

/// Twins (equal bodies) with distinct quotes: each entry's quote picks
/// its binding, both move, crossed indexes kept.
#[test]
fn twins_with_distinct_quotes_each_move() {
    assert_eq!(
        run(
            &[
                p(G, A, "twin", "sk", "quote one"),
                p(G, A, "twin", "sk", "quote two"),
            ],
            &[
                p(G, A, "twin", "sk", "quote two"),
                p(G, A, "twin", "sk", "quote one"),
            ],
            T
        ),
        vec![Follow::Moved(1), Follow::Moved(0)]
    );
}

/// A trivial binding is never a candidate: alone it leaves the entry
/// `Lost`; beside a non-trivial body-equal one it does not make the entry
/// AMBIGUOUS. A trivial entry never contests a binding.
#[test]
fn trivial_candidate_is_never_followed() {
    let entry = p(G, A, "body", "sk", "quote");
    let trivial = p(G, B, "body", "sk", "quote").sized(T - 1);
    assert_eq!(run(&[entry], &[trivial], T), vec![Follow::Lost]);
    assert_eq!(
        run(&[entry], &[trivial, p(G, B, "body", "sk", "other")], T),
        vec![Follow::Moved(1)],
        "the trivial one is no candidate"
    );
    assert_eq!(
        run(
            &[entry, p(G, A, "body", "sk", "quote").sized(T - 1)],
            &[p(G, B, "body", "sk", "other")],
            T
        ),
        vec![Follow::Moved(0), Follow::Lost],
        "a trivial entry contests nothing"
    );
}

/// Candidates share the entry's group key: a body-equal binding of
/// another group is none (`Lost`); four prints of one body over two
/// groups pair within each group. The key is opaque: any `Ord` type.
#[test]
fn candidates_share_the_group_key() {
    const OTHER: &str = "N-2 implements item function_item";
    assert_eq!(
        run(
            &[p(G, A, "body", "sk", "q")],
            &[p(OTHER, A, "body", "sk", "q")],
            T
        ),
        vec![Follow::Lost]
    );
    assert_eq!(
        run(
            &[p(G, A, "body", "sk", "q"), p(OTHER, A, "body", "sk", "q")],
            &[p(OTHER, B, "body", "sk", "q"), p(G, B, "body", "sk", "q")],
            T
        ),
        vec![Follow::Moved(1), Follow::Moved(0)]
    );
    // A tuple key, differing in one part only.
    let print = |group: (u8, &'static str), body: &'static str| Print {
        group,
        file: A,
        body,
        skeleton: "sk",
        quote: "q",
        body_bytes: T,
    };
    assert_eq!(
        pair(
            &[
                print((1, "verifies"), "body"),
                print((1, "implements"), "body")
            ],
            &[
                print((1, "implements"), "body"),
                print((2, "verifies"), "body")
            ],
            T
        ),
        vec![Follow::Lost, Follow::Moved(0)]
    );
}

// -------------------------------------------------------- determinism

/// Every permutation of `n` indexes.
fn permutations(n: usize) -> Vec<Vec<usize>> {
    if n == 0 {
        return vec![Vec::new()];
    }
    let mut out = Vec::new();
    for rest in permutations(n - 1) {
        for at in 0..=rest.len() {
            let mut next = rest.clone();
            next.insert(at, n - 1);
            out.push(next);
        }
    }
    out
}

/// `follow` over bindings given in the order `order` (`order[j]` the
/// original index of the j-th binding given), in original indexes.
fn mapped_back(follow: &Follow, order: &[usize]) -> Follow {
    match follow {
        Follow::Moved(at) => Follow::Moved(order[*at]),
        Follow::Ambiguous(list) => {
            let mut back: Vec<usize> = list.iter().map(|at| order[*at]).collect();
            assert!(
                list.windows(2).all(|pair| pair[0] < pair[1]),
                "candidates ascending: {list:?}"
            );
            back.sort_unstable();
            Follow::Ambiguous(back)
        }
        Follow::Lost => Follow::Lost,
    }
}

/// One permutation case: its label, three entries, three bindings, the
/// follows expected in the given order.
type Case = (&'static str, [P; 3], [P; 3], [Follow; 3]);

/// "Deterministic": every permutation of a 3 + 3 input (both sides, 36
/// orders), the follows mapped back to the original indexes, equals the
/// expected. Two inputs: one where every entry moves (a body pair, twins
/// split by quote, a skeleton step), one with the R(b) rule, a candidate
/// list and a lost entry. M: a follow decided by input order (the first
/// candidate, the first entry to claim a binding).
#[test]
fn pairing_ignores_input_order() {
    let cases: [Case; 2] = [
        (
            "every entry moves",
            [
                p(G, A, "twin", "sk twin", "quote one"),
                p(G, A, "twin", "sk twin", "quote two"),
                p(G, A, "old", "sk step", "quote three"),
            ],
            [
                p(G, B, "twin", "sk twin", "quote two"),
                p(G, A, "new", "sk step", "quote moved"),
                p(G, A, "twin", "sk twin", "quote one"),
            ],
            [Follow::Moved(2), Follow::Moved(0), Follow::Moved(1)],
        ),
        (
            "R(b), candidates, lost",
            [
                p(G, A, "body", "sk", "entry quote"),
                p(G, A, "edited", "sk", "binding quote"),
                p(G, A, "unmatched", "sk unmatched", "q"),
            ],
            [
                p(G, A, "body", "sk", "binding quote"),
                p(G, A, "unrelated", "sk unrelated", "q"),
                p(G, B, "body", "sk", "far quote"),
            ],
            [
                Follow::Ambiguous(vec![0, 2]),
                Follow::Ambiguous(vec![0]),
                Follow::Lost,
            ],
        ),
    ];
    let orders = permutations(3);
    assert_eq!(orders.len(), 6);
    for (label, entries, bindings, expected) in cases {
        for entry_order in &orders {
            for binding_order in &orders {
                let given_entries: Vec<P> = entry_order.iter().map(|at| entries[*at]).collect();
                let given_bindings: Vec<P> = binding_order.iter().map(|at| bindings[*at]).collect();
                let follows = run(&given_entries, &given_bindings, T);
                let mut back = vec![Follow::Lost; 3];
                for (given, follow) in follows.iter().enumerate() {
                    back[entry_order[given]] = mapped_back(follow, binding_order);
                }
                assert_eq!(
                    back, expected,
                    "{label}: entries {entry_order:?}, bindings {binding_order:?}"
                );
            }
        }
    }
}
