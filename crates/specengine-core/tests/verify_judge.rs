//! docs/features/spec-verify.md RD-1, RD-2, RD-9 and RD-10 (a): the pure
//! half of `spec verify` in core, over facts built here by hand: the
//! `[verify]` table, the judge's "States" order, the verdict of "Verdict and
//! exit", Pre-existing, the row key's order, and the seam that the judge is
//! neutral to where its facts came from. Each test names the mutation that
//! turns it red (`M:`).

use std::collections::BTreeMap;

use specengine_core::ProjectConfig;
use specengine_core::check::{CHECK_CODES, Mode};
use specengine_core::lock::{EntryKey, LockKind, LockRelation, Verified};
use specengine_core::verify::{
    Class, CodeFact, Compared, Comparison, Finding, Judgement, MarkerFact, MarkerRev, NodeFact,
    RowBinding, RowFacts, RowKey, State, Unbound, Verdict, VerifyConfig, is_pre_existing,
    judge_row, verdict, verify_from_toml,
};

const H1: &str = "b3:1111111111111111111111111111111111111111111111111111111111111111";
const H2: &str = "b3:2222222222222222222222222222222222222222222222222222222222222222";

fn key() -> RowKey {
    RowKey {
        node: "N-1".to_owned(),
        relation: LockRelation::Implements,
        file: "src/a.rs".to_owned(),
        binding: RowBinding::Keyed {
            kind: LockKind::Item,
            symbol: "a::f".to_owned(),
            item_kind: Some("function_item".to_owned()),
        },
    }
}

fn entry(rev: u32, hash: &str) -> Verified {
    Verified {
        rev,
        spec_hash: hash.to_owned(),
        ast_hash: Some(H1.to_owned()),
        file_blob: "a".repeat(40),
        commit: "b".repeat(40),
        verified_at: "2026-10-08".to_owned(),
    }
}

fn marker(rev: MarkerRev) -> MarkerFact {
    MarkerFact {
        line: 7,
        rev,
        bad_levels: false,
    }
}

fn found(rev: u32, hash: &str) -> NodeFact {
    NodeFact::Found {
        rev: Some(rev),
        hash: Some(hash.to_owned()),
    }
}

/// A locked row at revision 2 with `H1`, its marker `@2`, its code `code`.
fn locked(code: CodeFact) -> RowFacts {
    RowFacts {
        key: key(),
        node: found(2, H1),
        entry: Some(entry(2, H1)),
        markers: vec![marker(MarkerRev::Written(2))],
        code,
    }
}

fn state(facts: &RowFacts) -> (State, Option<String>, Option<String>) {
    let judged = judge_row(facts);
    (judged.state, judged.category, judged.detail)
}

fn named(
    state: State,
    category: Option<&str>,
    detail: Option<&str>,
) -> (State, Option<String>, Option<String>) {
    (
        state,
        category.map(str::to_owned),
        detail.map(str::to_owned),
    )
}

// --------------------------------------------------------------------- RD-1

/// RD-1: no `[verify]` table, an empty one and the default all read
/// `observe`; each mode is read by its name. M: default `enforce`.
#[test]
fn verify_mode_defaults_to_observe() {
    assert_eq!(VerifyConfig::default().mode, Mode::Observe);
    for text in ["", "[project]\nslug = \"x\"\n", "[verify]\n"] {
        assert_eq!(
            verify_from_toml(text).expect("readable").mode,
            Mode::Observe,
            "{text:?}"
        );
    }
    for mode in Mode::ALL {
        let text = format!("[verify]\nmode = \"{}\"\n", mode.as_str());
        assert_eq!(VerifyConfig::from_toml(&text).expect("readable").mode, mode);
    }
}

/// RD-1, flipped by docs/features/verify-tests.md (8a-1) RD-1: `[verify]`
/// is closed: `mode`, `tests` and `timeout` read, nothing named "not
/// built"; another key, a wrong type and a wrong value refused at their
/// line, a string `tests` with the argv-list text, a `timeout` out of
/// 1..86 400 naming the key. M: `tests` as a string accepted.
#[test]
fn verify_unknown_key_is_refused() {
    for text in [
        "[project]\nslug = \"x\"\n\n[verify]\ntests = [\"scripts/run-test\", \"{test}\"]\n",
        "[verify]\nmode = \"observe\"\ntimeout = 30\ntests = [\"scripts/run-test\", \"{qpath}\"]\n",
    ] {
        verify_from_toml(text).unwrap_or_else(|error| panic!("{text:?}: {error:?}"));
    }
    let cases = [
        (
            "[project]\nslug = \"x\"\n\n[verify]\ntests = \"scripts/run-test {test}\"\n",
            5,
            "[verify] tests must be an argv list, e.g. [\"scripts/run-test\", \"{test}\"]",
        ),
        ("[verify]\nmode = \"observe\"\ntimeout = 0\n", 3, "timeout"),
        (
            "[verify]\nstrict = true\n",
            2,
            "`[verify]` has no key `strict`",
        ),
        ("[verify]\nmode = 1\n", 2, "`[verify] mode` is a integer"),
        (
            "[verify]\nmode = \"strict\"\n",
            2,
            "`[verify] mode` `strict` is not",
        ),
    ];
    for (text, line, words) in cases {
        let error = verify_from_toml(text).expect_err(text);
        assert_eq!(error.line, Some(line), "{text:?}: {error:?}");
        assert!(error.message.contains(words), "{text:?}: {}", error.message);
        assert!(
            !error.message.contains("is not built"),
            "{text:?}: {}",
            error.message
        );
        assert!(
            error
                .at("specengine.toml")
                .starts_with(&format!("specengine.toml:{line}: ")),
            "{}",
            error.at("specengine.toml")
        );
    }
}

/// RD-1: every other reader of the config only name-checks `[verify]`: a
/// config with a `[verify]` table (a valid one and one verify itself
/// refuses) is a valid project config. M: `verify` not in `RawFile`.
#[test]
fn verify_table_is_name_checked_by_the_project_config() {
    for table in ["[verify]\nmode = \"enforce\"\n", "[verify]\ntests = 1\n"] {
        let text = format!("[project]\nslug = \"x\"\n\n{table}");
        ProjectConfig::from_toml(&text).unwrap_or_else(|error| panic!("{text:?}: {error:?}"));
    }
}

// --------------------------------------------------------------------- RD-2

/// RD-2: every fact "States" makes `cannot_verify` gives it, whatever else
/// holds (an entry or none, markers raised or lagging, the code equal),
/// and a `cannot_verify` row makes the verdict `cannot-verify` in every
/// mode, beside any other finding. M: `cannot_verify` as `ok`.
#[test]
fn cannot_verify_is_never_ok() {
    let cannot: Vec<(RowFacts, &str, Option<&str>)> = vec![
        (
            locked(CodeFact::ItemError("missing".into())),
            "item_error",
            Some("missing"),
        ),
        (
            locked(CodeFact::Unbound(Unbound::ItemError("other".into()))),
            "item_error",
            Some("other"),
        ),
        (
            locked(CodeFact::Ambiguous("duplicate".into())),
            "ambiguous_symbol",
            Some("duplicate"),
        ),
        (
            RowFacts {
                node: NodeFact::Ambiguous,
                ..locked(CodeFact::Bound(Comparison::Equal(Compared::Blob)))
            },
            "ambiguous_id",
            None,
        ),
        (
            RowFacts {
                node: NodeFact::Found {
                    rev: None,
                    hash: Some(H1.into()),
                },
                ..locked(CodeFact::Bound(Comparison::Equal(Compared::Blob)))
            },
            "rev_unknown",
            None,
        ),
        (
            RowFacts {
                node: NodeFact::Found {
                    rev: Some(2),
                    hash: None,
                },
                ..locked(CodeFact::Bound(Comparison::Equal(Compared::Blob)))
            },
            "hash_null",
            None,
        ),
        (
            locked(CodeFact::Unbound(Unbound::Unreadable("not_utf8".into()))),
            "unreadable",
            Some("not_utf8"),
        ),
        (
            locked(CodeFact::Unbound(Unbound::HiddenByError)),
            "hidden_by_error",
            None,
        ),
        (
            locked(CodeFact::Bound(Comparison::CannotVerify(
                "nesting_too_deep".into(),
            ))),
            "item_error",
            Some("nesting_too_deep"),
        ),
    ];
    for (facts, category, detail) in &cannot {
        for markers in [
            vec![],
            vec![marker(MarkerRev::Written(2))],
            vec![marker(MarkerRev::Written(1))],
            vec![marker(MarkerRev::Written(9))],
            vec![marker(MarkerRev::Bad)],
            vec![MarkerFact {
                bad_levels: true,
                ..marker(MarkerRev::Unversioned)
            }],
        ] {
            let facts = RowFacts {
                markers,
                ..facts.clone()
            };
            assert_eq!(
                state(&facts),
                named(State::CannotVerify, Some(category), *detail),
                "{facts:?}"
            );
            assert_eq!(judge_row(&facts).state.class(), Some(Class::Cannot));
        }
    }
    // An entry whose code was never compared is never `ok`.
    assert_eq!(
        judge_row(&locked(CodeFact::Bound(Comparison::NotHashed))).state,
        State::CannotVerify
    );
    for mode in Mode::ALL {
        for pre_existing in [false, true] {
            let rows = [
                Finding {
                    state: State::Ok,
                    pre_existing: false,
                },
                Finding {
                    state: State::Conflict,
                    pre_existing: false,
                },
                Finding {
                    state: State::CannotVerify,
                    pre_existing,
                },
                Finding {
                    state: State::Unlocked,
                    pre_existing: false,
                },
            ];
            assert_eq!(
                verdict(mode, false, rows),
                Verdict::CannotVerify,
                "{mode:?}"
            );
        }
        assert_eq!(
            verdict(mode, true, []),
            Verdict::CannotVerify,
            "a run cause, {mode:?}"
        );
    }
}

/// RD-2: the order of "States": `cannot_verify` before `predated` before
/// `bad_marker` before `broken_binding` before `unlocked`. M: a later state
/// tried first.
#[test]
fn states_apply_in_the_tables_order() {
    // predated beats bad_marker (a bad rev beside a raised marker).
    let facts = RowFacts {
        markers: vec![marker(MarkerRev::Written(3)), marker(MarkerRev::Bad)],
        ..locked(CodeFact::Unbound(Unbound::MarkerGone))
    };
    assert_eq!(state(&facts), named(State::Predated, None, None));
    // bad_marker beats broken_binding.
    let facts = RowFacts {
        markers: vec![marker(MarkerRev::Bad)],
        ..locked(CodeFact::Unbound(Unbound::SymbolGone))
    };
    assert_eq!(
        state(&facts),
        named(State::BadMarker, Some("bad_rev"), None)
    );
    // dangling and homoglyph IDs.
    for (node, category) in [
        (NodeFact::Dangling, "dangling"),
        (NodeFact::Homoglyph, "homoglyph"),
    ] {
        let facts = RowFacts {
            node,
            entry: None,
            ..locked(CodeFact::Bound(Comparison::NotHashed))
        };
        assert_eq!(state(&facts), named(State::BadMarker, Some(category), None));
    }
    // unanchored, bad levels.
    let facts = RowFacts {
        entry: None,
        ..locked(CodeFact::Unanchored("in_body".into()))
    };
    assert_eq!(
        state(&facts),
        named(State::BadMarker, Some("unanchored"), Some("in_body"))
    );
    let facts = RowFacts {
        markers: vec![MarkerFact {
            bad_levels: true,
            ..marker(MarkerRev::Written(2))
        }],
        ..locked(CodeFact::Bound(Comparison::Equal(Compared::Blob)))
    };
    assert_eq!(
        state(&facts),
        named(State::BadMarker, Some("bad_levels"), None)
    );
    // broken_binding: each category; gone beats the comparison.
    for (unbound, category) in [
        (Unbound::FileGone, "file_gone"),
        (Unbound::SymbolGone, "symbol_gone"),
        (Unbound::MarkerGone, "marker_gone"),
    ] {
        let facts = RowFacts {
            markers: vec![],
            ..locked(CodeFact::Unbound(unbound))
        };
        assert_eq!(
            state(&facts),
            named(State::BrokenBinding, Some(category), None)
        );
    }
    let facts = RowFacts {
        node: NodeFact::Gone,
        markers: vec![],
        ..locked(CodeFact::Unbound(Unbound::MarkerGone))
    };
    assert_eq!(
        state(&facts),
        named(State::BrokenBinding, Some("node_gone"), None)
    );
    // unlocked: no entry, not lagging; key_problem whatever the entry.
    let facts = RowFacts {
        entry: None,
        ..locked(CodeFact::Bound(Comparison::NotHashed))
    };
    assert_eq!(state(&facts), named(State::Unlocked, None, None));
    let mut facts = RowFacts {
        entry: None,
        ..locked(CodeFact::Bound(Comparison::NotHashed))
    };
    facts.key.file = "src/a\nb.rs".to_owned();
    assert_eq!(
        state(&facts),
        named(State::Unlocked, None, Some("key_problem"))
    );
}

/// RD-2: the comparison rows of "States", each from its facts; a raised
/// marker with both sides changed is `code_ahead raised`, a lagging one
/// with equal hashes `spec_ahead marker_behind`. M: `conflict` when a
/// marker names R; lagging with equal hashes `ok`.
#[test]
fn comparison_states_follow_the_table() {
    let equal = CodeFact::Bound(Comparison::Equal(Compared::Ast));
    let changed = CodeFact::Bound(Comparison::Changed(Compared::Ast));
    let with = |node: NodeFact, markers: Vec<MarkerRev>, code: &CodeFact| RowFacts {
        node,
        markers: markers.into_iter().map(marker).collect(),
        ..locked(code.clone())
    };
    let cases = [
        // neither changed
        (
            with(found(2, H1), vec![MarkerRev::Written(2)], &equal),
            named(State::Ok, None, None),
        ),
        (
            with(found(2, H1), vec![MarkerRev::Unversioned], &equal),
            named(State::Ok, None, None),
        ),
        // lagging, code unchanged, hashes equal
        (
            with(found(2, H1), vec![MarkerRev::Written(1)], &equal),
            named(State::SpecAhead, Some("marker_behind"), None),
        ),
        // spec changed at the same revision, code unchanged
        (
            with(found(2, H2), vec![MarkerRev::Written(2)], &equal),
            named(State::SpecAhead, Some("same_revision"), None),
        ),
        // spec raised, the marker not, code unchanged
        (
            with(found(3, H2), vec![MarkerRev::Unversioned], &equal),
            named(State::SpecAhead, Some("spec_changed"), None),
        ),
        // code changed, spec unchanged
        (
            with(found(2, H1), vec![MarkerRev::Written(2)], &changed),
            named(State::CodeAhead, Some("code_changed"), None),
        ),
        // spec raised and a marker names R: code unchanged, then changed
        (
            with(found(3, H2), vec![MarkerRev::Written(3)], &equal),
            named(State::CodeAhead, Some("raised"), None),
        ),
        (
            with(found(3, H2), vec![MarkerRev::Written(3)], &changed),
            named(State::CodeAhead, Some("raised"), None),
        ),
        // spec and code changed, no raise
        (
            with(found(2, H2), vec![MarkerRev::Written(2)], &changed),
            named(State::Conflict, None, None),
        ),
        (
            with(found(3, H2), vec![MarkerRev::Unversioned], &changed),
            named(State::Conflict, None, None),
        ),
        // a lagging marker with the code changed: not lagging's row
        (
            with(found(3, H2), vec![MarkerRev::Written(2)], &changed),
            named(State::Conflict, None, None),
        ),
    ];
    for (facts, expected) in cases {
        assert_eq!(state(&facts), expected, "{facts:?}");
        assert_eq!(judge_row(&facts).compared, Some(Compared::Ast), "{facts:?}");
    }
    // No entry: lagging is `spec_ahead marker_behind`, else `unlocked`; never compared.
    let unlocked = RowFacts {
        entry: None,
        ..with(
            found(3, H1),
            vec![MarkerRev::Written(2)],
            &CodeFact::Bound(Comparison::NotHashed),
        )
    };
    assert_eq!(
        state(&unlocked),
        named(State::SpecAhead, Some("marker_behind"), None)
    );
    assert_eq!(judge_row(&unlocked).compared, None);
}

// --------------------------------------------------------------- RD-10 (a)

/// RD-10 (a): the judge is pure over per-key facts from any source: the
/// same facts give the same state whether the code was compared by the
/// blob (4b's index OIDs) or by the item's hash (7a's indexed hash), and
/// no field the source alone sets (the marker's line, the entry's commit,
/// date, blob or hash) moves a state. M: facts from another source change
/// a state.
#[test]
fn judge_is_source_neutral() {
    let nodes = [found(2, H1), found(2, H2), found(3, H2), found(1, H1)];
    let markers = [
        vec![],
        vec![MarkerRev::Unversioned],
        vec![MarkerRev::Written(1)],
        vec![MarkerRev::Written(2)],
        vec![MarkerRev::Written(3)],
        vec![MarkerRev::Written(2), MarkerRev::Written(3)],
    ];
    let mut seen = 0;
    for node in &nodes {
        for marks in &markers {
            for changed in [false, true] {
                let code = |by: Compared| {
                    CodeFact::Bound(if changed {
                        Comparison::Changed(by)
                    } else {
                        Comparison::Equal(by)
                    })
                };
                let base = RowFacts {
                    node: node.clone(),
                    markers: marks.iter().copied().map(marker).collect(),
                    ..locked(code(Compared::Blob))
                };
                let by_blob = judge_row(&base);
                let by_ast = judge_row(&RowFacts {
                    code: code(Compared::Ast),
                    ..base.clone()
                });
                assert_eq!(
                    (by_blob.state, &by_blob.category, &by_blob.detail),
                    (by_ast.state, &by_ast.category, &by_ast.detail),
                    "{base:?}"
                );
                // `compared` names the source only when the comparison
                // decided the state (a predated row never compares).
                assert_eq!(
                    (by_blob.compared, by_ast.compared),
                    match by_blob.compared {
                        None => (None, None),
                        Some(_) => (Some(Compared::Blob), Some(Compared::Ast)),
                    },
                    "{base:?}"
                );
                // Fields only a source sets.
                let mut moved = base.clone();
                for fact in &mut moved.markers {
                    fact.line += 100;
                }
                let entry = moved.entry.as_mut().expect("an entry");
                entry.commit = "c".repeat(64);
                entry.verified_at = "2030-01-01".to_owned();
                entry.file_blob = "d".repeat(64);
                entry.ast_hash = Some(H2.to_owned());
                assert_eq!(judge_row(&moved), by_blob, "{moved:?}");
                seen += 1;
            }
        }
    }
    assert_eq!(seen, 48);
}

// -------------------------------------------------------------- the verdict

/// "Verdict and exit": `clean` only with every row `ok`; Check rows never
/// block; `observe` never blocks; `enforce` blocks on any Fix row;
/// `enforce-introduced` on one not Pre-existing. M: a Check blocking; a
/// Pre-existing Fix blocking under `enforce-introduced`; exit 1 under
/// `observe`.
#[test]
fn verdict_follows_the_mode_table() {
    let f = |state, pre_existing| Finding {
        state,
        pre_existing,
    };
    let fix_states = [
        State::Predated,
        State::BadMarker,
        State::BrokenBinding,
        State::Conflict,
    ];
    let check_states = [State::Unlocked, State::SpecAhead, State::CodeAhead];
    for mode in Mode::ALL {
        assert_eq!(verdict(mode, false, []), Verdict::Clean);
        assert_eq!(verdict(mode, false, [f(State::Ok, false)]), Verdict::Clean);
        for state in check_states {
            for pre in [false, true] {
                assert_eq!(
                    verdict(mode, false, [f(state, pre), f(State::Ok, false)]),
                    Verdict::Observed,
                    "{mode:?} {state:?}"
                );
            }
        }
        for state in fix_states {
            let introduced = verdict(mode, false, [f(State::Unlocked, false), f(state, false)]);
            let pre_existing = verdict(mode, false, [f(State::Unlocked, false), f(state, true)]);
            let expected = match mode {
                Mode::Observe => (Verdict::Observed, Verdict::Observed),
                Mode::EnforceIntroduced => (Verdict::Blocked, Verdict::Observed),
                Mode::Enforce => (Verdict::Blocked, Verdict::Blocked),
            };
            assert_eq!((introduced, pre_existing), expected, "{mode:?} {state:?}");
        }
    }
    let classes: BTreeMap<State, Option<Class>> = [
        State::CannotVerify,
        State::Predated,
        State::BadMarker,
        State::BrokenBinding,
        State::Unlocked,
        State::SpecAhead,
        State::CodeAhead,
        State::Conflict,
        State::Ok,
    ]
    .into_iter()
    .map(|state| (state, state.class()))
    .collect();
    assert_eq!(
        classes.values().copied().collect::<Vec<_>>(),
        [
            Some(Class::Cannot),
            Some(Class::Fix),
            Some(Class::Fix),
            Some(Class::Fix),
            Some(Class::Check),
            Some(Class::Check),
            Some(Class::Check),
            Some(Class::Fix),
            None
        ]
    );
    assert_eq!(
        [
            Verdict::Clean,
            Verdict::Observed,
            Verdict::Blocked,
            Verdict::CannotVerify
        ]
        .map(Verdict::as_str),
        ["clean", "observed", "blocked", "cannot-verify"]
    );
}

/// "Base": a row is Pre-existing when `HEAD` gives the same key, state and
/// category; an `ok` row never; another category or no row at `HEAD`: not.
#[test]
fn pre_existing_needs_the_same_state_and_category() {
    let judged = |state, category: Option<&str>| Judgement {
        state,
        category: category.map(str::to_owned),
        detail: None,
        compared: None,
    };
    let now = judged(State::BrokenBinding, Some("marker_gone"));
    let mut head = BTreeMap::new();
    assert!(!is_pre_existing(&key(), &now, &head));
    head.insert(key(), judged(State::BrokenBinding, Some("symbol_gone")));
    assert!(!is_pre_existing(&key(), &now, &head));
    head.insert(key(), judged(State::BrokenBinding, Some("marker_gone")));
    assert!(is_pre_existing(&key(), &now, &head));
    let mut other = key();
    other.file = "src/b.rs".to_owned();
    assert!(!is_pre_existing(&other, &now, &head));
    head.insert(key(), judged(State::Ok, None));
    assert!(!is_pre_existing(&key(), &judged(State::Ok, None), &head));
}

/// "Command and rows": rows in `EntryKey` order (node, relation, file,
/// kind, symbol, item kind by bytes, null first), a keyless binding before
/// every kind of its node, relation and file.
#[test]
fn row_keys_order_as_lock_keys_keyless_first() {
    let keyed = |node: &str, relation, file: &str, kind, symbol: &str, item_kind: Option<&str>| {
        RowKey::of_entry(&EntryKey {
            node: node.to_owned(),
            relation,
            file: file.to_owned(),
            kind,
            symbol: symbol.to_owned(),
            item_kind: item_kind.map(str::to_owned),
        })
    };
    let keyless = |node: &str, file: &str, state: &str, category: &str| RowKey {
        node: node.to_owned(),
        relation: LockRelation::Implements,
        file: file.to_owned(),
        binding: RowBinding::Keyless {
            state: state.to_owned(),
            category: category.to_owned(),
        },
    };
    let expected = vec![
        keyless("A-1", "src/a.rs", "cannot_verify", "other"),
        keyless("A-1", "src/a.rs", "unanchored", "in_body"),
        keyless("A-1", "src/a.rs", "unanchored", "not_an_item"),
        keyed(
            "A-1",
            LockRelation::Implements,
            "src/a.rs",
            LockKind::Item,
            "a::f",
            Some("function_item"),
        ),
        keyed(
            "A-1",
            LockRelation::Implements,
            "src/a.rs",
            LockKind::Module,
            "a",
            None,
        ),
        keyed(
            "A-1",
            LockRelation::Implements,
            "src/b.rs",
            LockKind::Item,
            "b::g",
            Some("const_item"),
        ),
        keyed(
            "A-1",
            LockRelation::Verifies,
            "src/a.rs",
            LockKind::Item,
            "a::f",
            Some("function_item"),
        ),
        keyed(
            "A-1",
            LockRelation::Configures,
            "d.ron",
            LockKind::Ron,
            "root",
            None,
        ),
        keyed(
            "A-1",
            LockRelation::Assumes,
            "src/a.rs",
            LockKind::Item,
            "a::f",
            Some("function_item"),
        ),
        keyed(
            "B-1",
            LockRelation::Implements,
            "src/a.rs",
            LockKind::Item,
            "a::f",
            Some("function_item"),
        ),
        keyed(
            "a/B-1",
            LockRelation::Implements,
            "src/a.rs",
            LockKind::Item,
            "a::f",
            Some("function_item"),
        ),
    ];
    let mut shuffled = expected.clone();
    shuffled.reverse();
    shuffled.sort();
    assert_eq!(shuffled, expected);
    for key in &expected {
        if let RowBinding::Keyed { .. } = key.binding {
            assert_eq!(
                RowKey::of_entry(&key.entry_key().expect("a lock key")),
                *key
            );
        } else {
            assert!(key.entry_key().is_none());
        }
    }
}

// --------------------------------------------------------------------- RD-9

/// RD-9: verify adds no check code: `CHECK_CODES` stayed 35; slice 4b-2
/// adds the rule `rev-not-raised` as the 36th (docs/features/rev-rule.md
/// RD-2), no other. M: a verify code added to the check.
#[test]
fn check_codes_stay_35() {
    assert_eq!(CHECK_CODES.len(), 36, "{CHECK_CODES:?}");
    assert_eq!(
        CHECK_CODES.last(),
        Some(&"rev-not-raised"),
        "{CHECK_CODES:?}"
    );
    assert!(
        CHECK_CODES.iter().all(|code| !code.contains("verify")),
        "{CHECK_CODES:?}"
    );
}
