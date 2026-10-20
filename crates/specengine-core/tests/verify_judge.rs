//! docs/features/spec-verify.md RD-1, RD-2, RD-9 and RD-10 (a): the pure
//! half of `spec verify` in core, over facts built here by hand: the
//! `[verify]` table, the judge's "States" order, the verdict of "Verdict and
//! exit", Pre-existing, the row key's order, and the seam that the judge is
//! neutral to where its facts came from. Each test names the mutation that
//! turns it red (`M:`).
//!
//! docs/features/lock-levels-verify.md RD-5 ("Level-aware verify"): the
//! pure level comparison (C = (L_lock union L_mark) minus `path`, step
//! 4's changed levels in canonical order) and the judge's mapping of steps
//! 2-4 to a category and a detail, over level names and digest strings
//! alone; M6 (`sig` and `body` compared whatever the levels) turns
//! `sig_marker_ignores_a_body_edit` red.

use std::collections::{BTreeMap, BTreeSet};

use specengine_core::ProjectConfig;
use specengine_core::check::{CHECK_CODES, Mode};
use specengine_core::lock::{EntryKey, LockKind, LockLevel, LockRelation, Verified, parse_lock};
use specengine_core::verify::{
    Class, CodeFact, Compared, Comparison, Finding, Judgement, LevelDigests, MarkerFact, MarkerRev,
    NodeFact, RowBinding, RowFacts, RowKey, RowLevels, State, Unbound, Verdict, VerifyConfig,
    changed_levels, is_pre_existing, judge_row, verdict, verify_from_toml,
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

/// A format-2 entry of `N-1`'s item at `rev` vouching for `hash`, its
/// level digests made of `digit`: read from the lock's text, so the facts
/// name no field of the code side (docs/features/lock-levels.md
/// "Format 2"; the judge reads the entry's revision and spec hash alone).
fn entry_with(rev: u32, hash: &str, digit: char) -> Verified {
    let digest = format!("b3:{}", digit.to_string().repeat(64));
    let text = format!(
        "format = 2\nnorm_recipe = \"specengine-norm/v1\"\ncode_recipe = \"specengine-hash/v3 tree-sitter-rust/0.24.2 abi/15\"\n\n\
         [[binding]]\nnode = \"N-1\"\nrev = {rev}\nspec_hash = \"{hash}\"\nrelation = \"implements\"\n\
         file = \"src/a.rs\"\nkind = \"item\"\nsymbol = \"a::f\"\nitem_kind = \"function_item\"\n\
         levels = [\"sig\", \"body\"]\nsig_hash = \"{digest}\"\nbody_hash = \"{digest}\"\n\
         deps_hash = \"{digest}\"\nskeleton_hash = \"{digest}\"\nquote_hash = \"{digest}\"\n\
         body_bytes = 12\nfile_blob = \"{}\"\ncommit = \"{}\"\nverified_by = \"owner\"\n\
         verified_at = \"2026-10-08\"\n",
        "a".repeat(40),
        "b".repeat(40)
    );
    let mut lock = parse_lock(&text, None).unwrap_or_else(|error| panic!("{error}\n{text}"));
    lock.entries.pop_first().expect("one entry").1
}

fn entry(rev: u32, hash: &str) -> Verified {
    entry_with(rev, hash, '1')
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

/// Level names as a set.
fn set(names: &[&str]) -> BTreeSet<LockLevel> {
    names
        .iter()
        .map(|name| LockLevel::from_name(name).unwrap_or_else(|| panic!("no level {name}")))
        .collect()
}

/// L_lock and L_mark by their names.
fn row_levels(lock: &[&str], markers: &[&str]) -> RowLevels {
    RowLevels {
        lock: set(lock),
        markers: set(markers),
    }
}

/// The default on both sides: `sig`, `body`.
fn same() -> RowLevels {
    row_levels(&["sig", "body"], &["sig", "body"])
}

/// Step 2, the blob equal, the levels the same: unchanged by `blob`.
fn by_blob() -> CodeFact {
    CodeFact::Bound(Comparison::Blob(same()))
}

/// Step 4 over the default levels, `changed` the levels that differ.
fn by_digests(changed: &[LockLevel]) -> CodeFact {
    CodeFact::Bound(Comparison::Digests {
        levels: same(),
        changed: changed.to_vec(),
    })
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
                ..locked(by_blob())
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
                ..locked(by_blob())
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
                ..locked(by_blob())
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
        ..locked(by_blob())
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
    let equal = by_digests(&[]);
    let changed = by_digests(&[LockLevel::Body]);
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
        // code changed, spec unchanged: the changed levels the detail
        (
            with(found(2, H1), vec![MarkerRev::Written(2)], &changed),
            named(State::CodeAhead, Some("code_changed"), Some("body")),
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
/// date, blob or hash) moves a state. Since docs/features/lock-levels-verify.md
/// the one change both sources can carry is the levels' (`levels_changed`:
/// L_lock differs from L_mark, by step 2 or step 4 alike). M: facts from
/// another source change a state.
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
                    let levels = if changed {
                        row_levels(&["sig", "body"], &["sig"])
                    } else {
                        same()
                    };
                    CodeFact::Bound(match by {
                        Compared::Blob => Comparison::Blob(levels),
                        Compared::Ast => Comparison::Digests {
                            levels,
                            changed: vec![],
                        },
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
                // Every level digest another (a fresh parse), then the
                // fields only a source sets.
                *entry = Verified {
                    rev: entry.rev,
                    spec_hash: entry.spec_hash.clone(),
                    ..entry_with(1, H1, '2')
                };
                entry.commit = "c".repeat(64);
                entry.verified_at = "2030-01-01".to_owned();
                entry.file_blob = "d".repeat(64);
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

// ------------------------------------------- lock-levels-verify RD-5

const S1: &str = "b3:5151515151515151515151515151515151515151515151515151515151515151";
const S2: &str = "b3:5252525252525252525252525252525252525252525252525252525252525252";
const B1: &str = "b3:b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1";
const B2: &str = "b3:b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";
const D1: &str = "b3:d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1";
const D2: &str = "b3:d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2";

/// `(sig, body, deps)` as one side's digests.
fn digests<'a>((sig, body, deps): (&'a str, &'a str, &'a str)) -> LevelDigests<'a> {
    LevelDigests { sig, body, deps }
}

/// What a judged row says: state, category, detail and `compared`.
type Said = (State, Option<String>, Option<String>, Option<Compared>);

fn said(
    state: State,
    category: Option<&str>,
    detail: Option<&str>,
    compared: Option<Compared>,
) -> Said {
    (
        state,
        category.map(str::to_owned),
        detail.map(str::to_owned),
        compared,
    )
}

fn judged(code: CodeFact) -> Said {
    let judged = judge_row(&locked(code));
    (
        judged.state,
        judged.category,
        judged.detail,
        judged.compared,
    )
}

/// Step 4 over level names and digest strings alone (docs/features/
/// lock-levels-verify.md "Level-aware verify"): the pure comparison of C's
/// digests (`changed_levels`), then the judge over its outcome, the spec
/// side unchanged (the row `locked`: revision 2, `H1`, a marker `@2`).
fn step_4(
    lock: &[&str],
    markers: &[&str],
    entry: (&str, &str, &str),
    now: (&str, &str, &str),
) -> Said {
    let levels = row_levels(lock, markers);
    let changed = changed_levels(&levels, &digests(entry), &digests(now));
    judged(CodeFact::Bound(Comparison::Digests { levels, changed }))
}

/// RD-5, M6: a `[sig]` marker, locked `[sig]`, over a body edit: C is
/// `{sig}`, the body's digest is never compared, the row `ok` by `ast`;
/// a signature edit still drifts it. M6: `sig` and `body` compared
/// whatever the levels (the row `code_changed body`).
#[test]
fn sig_marker_ignores_a_body_edit() {
    let sig = &["sig"][..];
    assert_eq!(row_levels(sig, sig).compared(), set(sig));
    assert_eq!(
        changed_levels(
            &row_levels(sig, sig),
            &digests((S1, B1, D1)),
            &digests((S1, B2, D2))
        ),
        []
    );
    assert_eq!(
        step_4(sig, sig, (S1, B1, D1), (S1, B2, D1)),
        said(State::Ok, None, None, Some(Compared::Ast))
    );
    assert_eq!(
        step_4(sig, sig, (S1, B1, D1), (S2, B2, D1)),
        said(
            State::CodeAhead,
            Some("code_changed"),
            Some("sig"),
            Some(Compared::Ast)
        )
    );
}

/// RD-5 (Q5): a marker narrowed to `[sig]` while the lock holds `sig,
/// body`: C is the union, so a body edit is `code_changed body` until
/// accept records the narrower set. M: L_mark alone compared (the edit
/// hidden).
#[test]
fn narrowed_marker_with_a_body_edit_is_code_changed() {
    let lock = &["sig", "body"][..];
    let narrowed = &["sig"][..];
    assert_eq!(row_levels(lock, narrowed).compared(), set(lock));
    assert_eq!(
        step_4(lock, narrowed, (S1, B1, D1), (S1, B2, D1)),
        said(
            State::CodeAhead,
            Some("code_changed"),
            Some("body"),
            Some(Compared::Ast)
        )
    );
    // No edit: the sets differ, every digest equal: `levels_changed`,
    // L_lock -> L_mark, each whole.
    assert_eq!(
        step_4(lock, narrowed, (S1, B1, D1), (S1, B1, D1)),
        said(
            State::CodeAhead,
            Some("levels_changed"),
            Some("sig,body->sig"),
            Some(Compared::Ast)
        )
    );
}

/// RD-5 (Q5, Q6): a marker widened to `sig, body` over a `[sig]` entry,
/// no edit: the entry holds every level's digest, none differs, the sets
/// do: `code_ahead levels_changed sig->sig,body`, class Check. A body edit
/// under the widened marker is `code_changed body` (the entry's recorded
/// `body` digest compared). M: a widened marker read `ok`.
#[test]
fn widened_marker_is_levels_changed() {
    let lock = &["sig"][..];
    let widened = &["sig", "body"][..];
    let row = step_4(lock, widened, (S1, B1, D1), (S1, B1, D1));
    assert_eq!(
        row,
        said(
            State::CodeAhead,
            Some("levels_changed"),
            Some("sig->sig,body"),
            Some(Compared::Ast)
        )
    );
    assert_eq!(row.0.class(), Some(Class::Check));
    assert_eq!(
        step_4(lock, widened, (S1, B1, D1), (S1, B2, D1)),
        said(
            State::CodeAhead,
            Some("code_changed"),
            Some("body"),
            Some(Compared::Ast)
        )
    );
}

/// RD-5 (Q42): both sides `[path]`: C is empty, nothing compared whatever
/// the digests: the producer's step 3 (`Comparison::PathOnly`) reads `ok`,
/// `compared` null. M: a `[path]` binding compared (`code_changed`), or
/// given a `compared`.
#[test]
fn path_only_compares_nothing() {
    let path = &["path"][..];
    let levels = row_levels(path, path);
    assert!(levels.compared().is_empty());
    assert!(!levels.differ());
    assert_eq!(
        changed_levels(&levels, &digests((S1, B1, D1)), &digests((S2, B2, D2))),
        []
    );
    assert_eq!(
        judged(CodeFact::Bound(Comparison::PathOnly)),
        said(State::Ok, None, None, None)
    );
    // `path` beside other levels is never compared, but it is named in a
    // `levels_changed` detail.
    assert_eq!(
        row_levels(&["path", "sig"], &["sig"]).compared(),
        set(&["sig"])
    );
    assert_eq!(
        step_4(&["path", "sig"], &["sig"], (S1, B1, D1), (S1, B2, D2)),
        said(
            State::CodeAhead,
            Some("levels_changed"),
            Some("path,sig->sig"),
            Some(Compared::Ast)
        )
    );
}

/// RD-5 (step 2): the blob equal to `file_blob`, nothing parsed: the same
/// sets read `ok` by `blob`; L_lock and L_mark apart (an ID resolving to
/// the node anew) `code_ahead levels_changed sig,body->sig` by `blob`. M:
/// a `levels_changed` row needing a digest.
#[test]
fn blob_equal_with_other_levels_is_levels_changed_by_blob() {
    assert_eq!(
        judged(by_blob()),
        said(State::Ok, None, None, Some(Compared::Blob))
    );
    assert_eq!(
        judged(CodeFact::Bound(Comparison::Blob(row_levels(
            &["sig", "body"],
            &["sig"]
        )))),
        said(
            State::CodeAhead,
            Some("levels_changed"),
            Some("sig,body->sig"),
            Some(Compared::Blob)
        )
    );
}

/// RD-5: step 4's `changed` in canonical order (`path`, `sig`, `body`,
/// `deps`), `,`-joined in the detail; `deps` only when in C; a RON value
/// (`sig` = `body` = its value hash, `deps` the empty set's: Q46) changes
/// `sig,body`, so a `[deps]` RON binding never drifts. M: `changed` in
/// another order; `deps` compared outside C.
#[test]
fn changed_levels_follow_the_canonical_order() {
    let all = &["sig", "body", "deps"][..];
    assert_eq!(
        changed_levels(
            &row_levels(all, all),
            &digests((S1, B1, D1)),
            &digests((S2, B1, D2))
        ),
        [LockLevel::Sig, LockLevel::Deps]
    );
    assert_eq!(
        step_4(all, all, (S1, B1, D1), (S1, B2, D2)).2.as_deref(),
        Some("body,deps")
    );
    // A call added (body and deps changed) under the default: `body`.
    assert_eq!(
        step_4(
            &["sig", "body"],
            &["sig", "body"],
            (S1, B1, D1),
            (S1, B2, D2)
        )
        .2
        .as_deref(),
        Some("body")
    );
    // Two markers on one key, `[sig]` and `[deps]`: C `sig, deps`.
    assert_eq!(
        step_4(
            &["sig", "deps"],
            &["sig", "deps"],
            (S1, B1, D1),
            (S1, B2, D2)
        )
        .2
        .as_deref(),
        Some("deps")
    );
    // A RON value: sig = body = the value hash.
    let (v1, v2, empty) = (S1, S2, D1);
    assert_eq!(
        step_4(
            &["sig", "body"],
            &["sig", "body"],
            (v1, v1, empty),
            (v2, v2, empty)
        )
        .2
        .as_deref(),
        Some("sig,body")
    );
    assert_eq!(
        step_4(&["deps"], &["deps"], (v1, v1, empty), (v2, v2, empty)),
        said(State::Ok, None, None, Some(Compared::Ast))
    );
}

/// RD-5 with 4a's matrix: a level change is "code changed": the spec
/// changed and raised reads `code_ahead raised`, changed and not raised
/// `conflict`, neither carrying a detail; lagging with a level change is
/// not lagging's row. M: `levels_changed` beside a spec change read
/// `spec_ahead`.
#[test]
fn level_changes_follow_the_comparison_table() {
    let narrowed = CodeFact::Bound(Comparison::Digests {
        levels: row_levels(&["sig", "body"], &["sig"]),
        changed: vec![],
    });
    let body = by_digests(&[LockLevel::Body]);
    let with = |node: NodeFact, rev: MarkerRev, code: &CodeFact| {
        let judged = judge_row(&RowFacts {
            node,
            markers: vec![marker(rev)],
            ..locked(code.clone())
        });
        (
            judged.state,
            judged.category,
            judged.detail,
            judged.compared,
        )
    };
    for code in [&narrowed, &body] {
        assert_eq!(
            with(found(3, H2), MarkerRev::Written(3), code),
            said(State::CodeAhead, Some("raised"), None, Some(Compared::Ast))
        );
        assert_eq!(
            with(found(2, H2), MarkerRev::Written(2), code),
            said(State::Conflict, None, None, Some(Compared::Ast))
        );
        assert_eq!(
            with(found(3, H2), MarkerRev::Written(2), code),
            said(State::Conflict, None, None, Some(Compared::Ast))
        );
    }
}

/// RD-5 ("Pre-existing compares the category only"): a row whose detail
/// moved (`code_changed body` at `HEAD`, `code_changed body,deps` now) is
/// still Pre-existing; another category (`levels_changed`) is not. M: the
/// detail compared.
#[test]
fn a_new_detail_is_not_introduced() {
    let judged = |category: &str, detail: &str| Judgement {
        state: State::CodeAhead,
        category: Some(category.to_owned()),
        detail: Some(detail.to_owned()),
        compared: Some(Compared::Ast),
    };
    let mut head = BTreeMap::new();
    head.insert(key(), judged("code_changed", "body"));
    assert!(is_pre_existing(
        &key(),
        &judged("code_changed", "body,deps"),
        &head
    ));
    assert!(!is_pre_existing(
        &key(),
        &judged("levels_changed", "sig,body->sig"),
        &head
    ));
}
