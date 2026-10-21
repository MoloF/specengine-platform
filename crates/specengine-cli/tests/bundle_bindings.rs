//! docs/features/bundle-bindings.md (7a-2): the bundle's layers 5
//! (`bindings`, Bindings) and 9 (`tests`, Tests) and the task package's
//! `bindings`, each row 7a-1's (docs/canon/code-reads.md
//! "The binding row") with its `sync` as `spec verify --node` judges it.
//! RD-1 to RD-7
//! (the CLI half; each test names the mutation that turns it red), TE-1
//! (the differential over every corpus), TE-2 (determinism), TE-3 (scale:
//! the 1 000-file leg here, the 10 000-file leg and the latency manual) and
//! TE-4 (`fixtures/spec-c` unchanged).
//!
//! **The oracle is `spec verify --node`** (Q-B), as in `show_bindings.rs`:
//! on one state, `spec bundle REF… --budget N` (text, then `--json`, stdin
//! null) and, for every node of a set, `spec verify --node <REF> --json`.
//! Layer 5's set is each REF's holders and every section within them (as
//! `spec show REF --json` lists them, checked against the parser); layer
//! 9's is that set and every layer-4 candidate, placed or left out. Layer
//! 5 holds the union of the set's verify rows whose relation is not
//! `verifies`, layer 9 the `verifies` rows of its set, each in plain
//! verify's row order; a row's key, `sync` (verify's `state`), `category`,
//! `detail` and `anchor` are the verify row's; its `line`, `end_line`,
//! `signature` and `layer` the code index's stored symbol of that key.
//! The package's `bindings` are the rows of its targets, `affected_nodes`
//! and their sections, every relation, at most 64.
//!
//! **The bytes before 7a-2** (the rule "WHEN `[code]` is absent …"):
//! `bundle_bindings_before.json` beside this file holds, per case, the
//! BLAKE3 of what the `spec` binary of 6d66f0a (7a-1 shipped, this slice not
//! started) printed for it: bundles of `spec-a`, `spec-b`, `spec-c` and
//! `code-a` without `[code]`, every ID and document at two budgets, and
//! briefs of planned tasks. [`record_the_bytes_before_bundle_bindings`]
//! wrote it (`SPECENGINE_BEFORE_SPEC` naming that binary); it is never
//! re-recorded from a later binary.
//!
//! Fixtures reused, no new one: `code-a` with the format-2 golden lock
//! (`verify_common::golden`), `anchor-states` (its 18 scenarios),
//! `verify-tests` (a kiln library, no lock), `verify-ecs` (an ECS game
//! crate with RON data), `spec-a`, `spec-b`, `spec-c`; edits are written
//! into scratch copies. Each repository has its own `HOME`. Non-Latin
//! characters in Rust sources are Unicode escapes (ADR-0024).
//!
//! docs/features/tested-reads.md (8b-2) RD-6 `bundle_minimum_ignores_tested`
//! over the evidence kiln (`tested_kiln::evidence_kiln`) and its
//! evidence-free twin: the `tested | ` lines live in the body only.

#![cfg(unix)]

mod anchor_common;
mod bindings_common;
mod code_index_common;
mod common;
mod lock_common;
mod tested_common;
mod tested_kiln;
mod verify_common;
mod verify_tests_common;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bindings_common::*;
use common::{copy_dir, fixture, repository_root};
use lock_common::LockRepo;
use serde_json::{Value, json};
use specengine_core::tokens_est;
use verify_common::{
    BOUNDS, COUNTER, COUNTER_MD, GOAL_MD, MOVEMENT, STAMINA, clone_dir, ecs, golden, golden_file,
    golden_format_1,
};

// ------------------------------------------------------------ the bundle

/// The row checks of one placed entry of a code layer: the 14 row keys
/// and `tokens_est`, the estimate of its form as printed.
fn assert_placed_keys(read: &Read) {
    for (key, _, _) in CODE_LAYERS {
        for row in read.layer(key) {
            let mut keys: Vec<&str> = row
                .as_object()
                .unwrap_or_else(|| panic!("{}: a row object: {row}", read.what))
                .keys()
                .map(String::as_str)
                .collect();
            keys.sort_unstable();
            let mut wanted: Vec<&str> = ROW_KEYS.to_vec();
            wanted.push("tokens_est");
            wanted.sort_unstable();
            assert_eq!(keys, wanted, "{}: a placed row's 15 keys: {row}", read.what);
            assert!(
                row["tokens_est"].as_u64().is_some_and(|tokens| tokens > 0),
                "{}: a placed row's estimate: {row}",
                read.what
            );
        }
        if let Some(keys) = first_object_keys(&read.json_run.stdout, key) {
            let mut wanted: Vec<String> = ROW_KEYS.iter().map(|key| (*key).to_owned()).collect();
            wanted.push("tokens_est".to_owned());
            assert_eq!(keys, wanted, "{}: {key}'s row keys in order", read.what);
        }
    }
}

/// The bundle's two sets: layer 5's (each REF's holders and their
/// sections), layer 9's (those and every layer-4 candidate of the bundle,
/// placed or in the tail). A bundle whose `more` hides candidates cannot
/// say them: it must hold none.
fn bundle_sets(
    cwd: &Path,
    refs: &[&str],
    read: &Read,
    vars: &[(OsString, OsString)],
) -> (Vec<String>, Vec<String>) {
    let five = holder_set(cwd, refs, vars);
    let document = read.json();
    assert_eq!(
        document["more"],
        json!(0),
        "premise: {}: every candidate named",
        read.what
    );
    let mut nine = five.clone();
    let criteria = read
        .layer("criteria")
        .into_iter()
        .chain(
            read.tail()
                .into_iter()
                .filter(|entry| entry["layer"] == "criteria"),
        )
        .map(|item| reference_of(cwd, text(&item["path"]), text(&item["name"])));
    for reference in criteria {
        if !nine.contains(&reference) {
            nine.push(reference);
        }
    }
    (five, nine)
}

/// TE-1 and RD-3 over one state: each REF group's bundle at `--budget`
/// u32::MAX against `verify --node` of every node of its sets, row for row
/// in plain verify's order ([`judged_view`]); every row located as the code
/// index stores it; each code layer's section the rows' forms
/// ([`row_form`]), an empty layer no heading; the rest of the body and the
/// JSON the bundle of the same state without `[code]`; no tail, no note.
/// Every placed row, for the corpora's coverage.
fn bundle_differential(repo: &LockRepo, slug: &str, groups: &[&[&str]]) -> Vec<Value> {
    let vars = vars_of(repo);
    let mut oracle = Oracle::new(repo);
    let mut reads = Vec::new();
    for refs in groups {
        let read = bundle_of(repo, refs, ALL);
        read.answered();
        assert_eq!(
            read.binding_notes(),
            Vec::<String>::new(),
            "{}: a sound state, no bindings note\n{}",
            read.what,
            read.show()
        );
        assert_placed_keys(&read);
        reads.push((refs, read));
    }
    let plains: Vec<Read> = without_code(&repo.root, || {
        groups
            .iter()
            .map(|refs| bundle_of(repo, refs, ALL))
            .collect()
    });
    let stored = stored_symbols(&repo.home, slug);
    let mut kept = Vec::new();
    for ((refs, read), plain) in reads.iter().zip(&plains) {
        plain.answered();
        let body = read.body();
        assert_eq!(
            without_code_layers(&body),
            plain.body(),
            "{}: the body but its code layers is the bundle without [code]",
            read.what
        );
        assert_eq!(
            spec_part_json(&read.json()),
            spec_part_json(&plain.json()),
            "{}: the JSON but its code layers is the bundle without [code]",
            read.what
        );
        assert_eq!(read.tail(), Vec::<Value>::new(), "{}: no tail", read.what);
        let (five, nine) = bundle_sets(&repo.root, refs, read, &vars);
        for (key, heading, verifies) in CODE_LAYERS {
            let set = if verifies { &nine } else { &five };
            let want: Vec<Expected> = oracle
                .set_rows(repo, set)
                .into_iter()
                .filter(|expected| (expected.row["relation"] == "verifies") == verifies)
                .collect();
            let got = read.layer(key);
            let shown: Vec<Value> = got.iter().map(shown_view).collect();
            let judged: Vec<Value> = want
                .iter()
                .map(|expected| judged_view(&expected.row))
                .collect();
            assert_eq!(
                shown,
                judged,
                "{}: layer {key} over {set:?}: rows as verify --node judges them, in its order\n{}",
                read.what,
                rows_diff(&judged, &shown)
            );
            let mut forms = Vec::new();
            for (row, expected) in got.iter().zip(&want) {
                assert_eq!(
                    shown_located(row),
                    located_view(row, &expected.row["markers"], &stored),
                    "{}: {row}: located as the code index stores it",
                    read.what
                );
                if row["kind"].is_string() {
                    assert_eq!(expected.binding, binding_of(row), "premise: verify's text");
                }
                let form = row_form(row, &expected.binding);
                assert_eq!(
                    row["tokens_est"],
                    json!(tokens_est(&form)),
                    "{}: {row}: its estimate is its form's",
                    read.what
                );
                forms.push(form);
            }
            match section(&body, heading) {
                Some(printed) => assert_eq!(
                    printed,
                    forms.join(ROW_GAP),
                    "{}: the {heading} section prints its rows",
                    read.what
                ),
                None => assert!(
                    got.is_empty(),
                    "{}: rows without a {heading} heading",
                    read.what
                ),
            }
            if got.is_empty() {
                assert!(
                    section(&body, heading).is_none(),
                    "{}: an empty layer prints no heading",
                    read.what
                );
            }
            kept.extend(got);
        }
    }
    kept
}

// ------------------------------------------------------------- the corpora

/// code-a's REF groups: every node by one or more forms, two REFs at once,
/// a REF within another (merged).
const GOLDEN_GROUPS: [&[&str]; 16] = [
    &["PIECE-COUNTER"],
    &[COUNTER_MD],
    &["LAW-BOUND"],
    &["PIECE-COUNTER#LAW-BOUND"],
    &["LAW-STEP"],
    &["LAW-ODD"],
    &["GOAL-01"],
    &["AIM-01"],
    &[GOAL_MD],
    &["PREM-001"],
    &["cap-check/PASS-01"],
    &["PIECE-LIMITS"],
    &["LAW-SOURCE"],
    &["LAW-DEFAULT"],
    &["GOAL-01", "LAW-STEP"],
    &["PIECE-COUNTER", "LAW-BOUND"],
];

/// TE-1 (a): the golden with [`drift`], [`halves`], [`nested`], a criterion
/// and a verifying record of GOAL-01; then a second state: a bound file
/// deleted, one not UTF-8, one excluded from `[code]`, a level list
/// narrowed.
fn corpus_golden() -> Vec<Value> {
    let repo = golden("bb-te1-golden");
    drift(&repo);
    halves(&repo);
    nested(&repo);
    criterion(&repo, 0);
    verifying_record(&repo);
    let mut kept = bundle_differential(&repo, CODE_A, &GOLDEN_GROUPS);
    assert!(
        kept.iter()
            .any(|row| row["node"] == "cap-check/PASS-01" && row["relation"] == "verifies"),
        "premise: a criterion's test in GOAL-01's layer 9"
    );
    assert!(
        kept.iter().any(|row| row["node"] == "PREM-002"),
        "premise: a verifying record's test in GOAL-01's layer 9"
    );

    // Every stored level digest tampered: the bundles still equal `verify
    // --node`, an `ok` row compared by `ast` among them, and the tampered
    // columns survive the bundles' no-op refresh.
    repo.spec_piped(&repo.root, &["index"]).code(0);
    let db = code_index_common::db(&repo.home, CODE_A);
    let fake = format!("b3:{}", "e".repeat(64));
    let sets: Vec<String> = ["sig", "body", "deps", "skeleton", "quote"]
        .iter()
        .map(|level| {
            format!("{level}_hash = CASE WHEN {level}_hash IS NULL THEN NULL ELSE '{fake}' END")
        })
        .collect();
    code_index_common::execute(&db, &format!("UPDATE symbols SET {}", sets.join(", ")));
    let tampered = || -> u64 {
        code_index_common::query(
            &db,
            &format!("SELECT count(*) AS n FROM symbols WHERE body_hash = '{fake}'"),
        )[0]["n"]
            .as_u64()
            .expect("a count")
    };
    let before = tampered();
    assert!(before > 0, "premise: digests to tamper");
    let digests = bundle_differential(&repo, CODE_A, &GOLDEN_GROUPS[..4]);
    assert!(
        ["PIECE-COUNTER", "LAW-BOUND", "LAW-STEP"]
            .iter()
            .flat_map(|id| verify_rows(&repo, id))
            .any(|row| row["compared"] == "ast" && row["state"] == "ok"),
        "premise: an `ok` row compared by `ast`, which a stored digest would turn"
    );
    assert!(digests.iter().any(|row| row["sync"] == "ok"));
    assert_eq!(tampered(), before, "premise: the tampered columns kept");
    kept.extend(digests);

    fs::remove_file(repo.root.join(BOUNDS)).expect("the test file deleted");
    let mut bytes = fs::read(repo.root.join(verify_common::LIB)).expect("lib.rs");
    bytes.extend_from_slice(b"// \xff\n");
    repo.write(verify_common::LIB, bytes);
    repo.replace(
        "specengine.toml",
        "exclude = [\"crates/*/tests/fixtures/**\"]",
        "exclude = [\"crates/*/tests/fixtures/**\", \"data/limits.ron\"]",
    );
    repo.replace(
        COUNTER,
        "@implements PIECE-COUNTER@3 [sig, body]",
        "@implements PIECE-COUNTER@3 [body]",
    );
    kept.extend(bundle_differential(&repo, CODE_A, &GOLDEN_GROUPS));
    kept
}

/// TE-1 (b): `fixtures/anchor-states`, its baseline, then its 18
/// scenarios applied together: both documents, every section, the
/// records.
fn corpus_anchor_states() -> Vec<Value> {
    let repo = anchor_common::base("bb-te1-anchors");
    let mut refs: Vec<&str> = anchor_common::NODES.to_vec();
    refs.extend([
        "KITE-DECOY",
        "KITE-FLIGHT",
        "KITE-RIGGING",
        "WISP-03",
        "WISP-04",
        "docs/spec/kites.md",
    ]);
    let groups: Vec<&[&str]> = refs.iter().map(std::slice::from_ref).collect();
    let mut kept = bundle_differential(&repo, ANCHORS, &groups);
    for name in anchor_common::SCENARIOS {
        anchor_common::change(&repo, name);
    }
    let renamed: Vec<&str> = refs
        .iter()
        .map(|id| if *id == "WISP-01" { "WISP-02" } else { id })
        .chain(["WISP-01", "docs/spec/rigging.md"])
        .collect();
    let groups: Vec<&[&str]> = renamed.iter().map(std::slice::from_ref).collect();
    let more = bundle_differential(&repo, ANCHORS, &groups);
    assert!(
        more.iter()
            .any(|row| row["sync"] == "ok" && row["anchor"]["state"] == "moved"),
        "premise: a moved `ok` row"
    );
    kept.extend(more);
    kept
}

/// TE-1 (c): `fixtures/verify-tests` (no lock): the document by ID and
/// path, each section, `CONE-01` and its legacy alias, a feature criterion.
fn corpus_kiln() -> Vec<Value> {
    let repo = LockRepo::of("bb-te1-kiln", "verify-tests", "repo", &[]);
    bundle_differential(
        &repo,
        KILN,
        &[
            &["KILN-FIRING"],
            &["docs/spec/kiln.md"],
            &["KILN-RAMP"],
            &["KILN-FIRING#KILN-HOLD"],
            &["KILN-COOL"],
            &["CONE-01"],
            &["PYRO-01"],
            &["CONE-02"],
            &["CONE-03"],
            &["glaze-check/GLZ-01"],
            &["docs/features/glaze-check.md"],
        ],
    )
}

/// TE-1 (d): `verify_common::ecs` (an ECS game crate, a RON file) with a
/// body edited, a RON value edited and a marker raised past its node.
fn corpus_ecs() -> Vec<Value> {
    let repo = ecs("bb-te1-ecs");
    repo.replace(
        STAMINA,
        "(stamina.current - cost * dt).max(0.0)",
        "(stamina.current - cost * dt * 2.0).max(0.0)",
    );
    repo.replace(MOVEMENT, "regen_per_second: 2.5,", "regen_per_second: 3.0,");
    repo.replace(
        STAMINA,
        "// @implements PERK-REGEN@1\n",
        "// @implements PERK-REGEN@2\n",
    );
    bundle_differential(
        &repo,
        GAME,
        &[
            &["PERK-STAMINA"],
            &["docs/spec/stamina.md"],
            &["PERK-REGEN"],
            &["PERK-DRAIN"],
            &["TUNE-01"],
            &["TUNE-02"],
            &["TUNE-03"],
            &["PERK-MOVE"],
            &["docs/spec/movement.md"],
            &["PERK-SPRINT"],
            &["PERK-SPRINT-COST"],
            &["PERK-WALK"],
            &["PERK-STAMINA", "PERK-MOVE"],
        ],
    )
}

/// One corpus: every row its bundles place.
type Corpus = fn() -> Vec<Value>;

/// Runs each corpus on a thread of its own; every row they place.
fn every_corpus(corpora: &[(&str, Corpus)]) -> Vec<Value> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = corpora
            .iter()
            .map(|(name, corpus)| (*name, scope.spawn(corpus)))
            .collect();
        let mut kept = Vec::new();
        let mut failed = Vec::new();
        for (name, handle) in handles {
            match handle.join() {
                Ok(rows) => kept.extend(rows),
                Err(_) => failed.push(name),
            }
        }
        assert!(failed.is_empty(), "corpora failed (above): {failed:?}");
        kept
    })
}

/// TE-1 and RD-3: every ID'd node of each corpus (code-a's golden twice
/// over, the 18 anchor scenarios, the kiln library without a lock, the ECS
/// game with RON data), by every REF form, each corpus on a thread of its
/// own: each bundle's layers 5 and 9 equal `verify --node` over their sets
/// ([`bundle_differential`]). Across them every state and anchor state,
/// the categories and details the corpora give, an item row with its
/// signature, a RON row with its marker's line only, a keyless row. M: a
/// second state path (`sync` from the marker's `@rev` or the index's
/// level digests); a neighbour's or a `verifies` row in layer 5.
#[test]
fn bundle_sync_equals_verify_node() {
    let kept = every_corpus(&[
        ("golden", corpus_golden),
        ("anchor-states", corpus_anchor_states),
        ("verify-tests", corpus_kiln),
        ("verify-ecs", corpus_ecs),
    ]);
    assert_eq!(
        values_of(&kept, |row| &row["sync"]),
        owned(&STATES),
        "every state placed"
    );
    assert_eq!(
        values_of(&kept, |row| &row["anchor"]["state"]),
        owned(&ANCHOR_STATES),
        "every anchor state placed"
    );
    let categories = values_of(&kept, |row| &row["category"]);
    for category in [
        "item_error",
        "ambiguous_symbol",
        "rev_unknown",
        "unreadable",
        "hidden_by_error",
        "homoglyph",
        "bad_rev",
        "unanchored",
        "bad_levels",
        "file_gone",
        "symbol_gone",
        "marker_gone",
        "marker_behind",
        "code_changed",
        "levels_changed",
    ] {
        assert!(
            categories.contains(category),
            "{category} placed: {categories:?}"
        );
    }
    for kind in ["item", "module", "ron"] {
        assert!(
            kept.iter().any(|row| row["kind"] == kind),
            "a {kind} row placed"
        );
    }
    assert!(
        kept.iter().any(|row| row["kind"].is_null()),
        "a keyless row placed"
    );
    assert!(
        kept.iter().any(|row| row["kind"] == "item"
            && row["line"].is_u64()
            && row["end_line"].is_u64()
            && row["signature"].is_string()),
        "an item row located with its signature"
    );
    assert!(
        kept.iter().any(|row| row["kind"] == "ron"
            && row["line"].is_u64()
            && row["end_line"].is_null()
            && row["signature"].is_null()),
        "a RON row with its marker's line only"
    );
    for relation in ["implements", "configures", "verifies"] {
        assert!(
            kept.iter().any(|row| row["relation"] == relation),
            "a {relation} row placed"
        );
    }
}

// ------------------------------------------------------------------ RD-1

/// A crate of `count` unrelated `.rs` files under `crates/filler`, every
/// other one's function bound to `node`.
fn filler(repo: &LockRepo, count: usize, node: &str) {
    repo.write(
        "crates/filler/Cargo.toml",
        "[package]\nname = \"filler\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );
    let mut lib = String::new();
    for at in 0..count {
        lib.push_str(&format!("pub mod part_{at:05};\n"));
        let marker = if at % 2 == 0 {
            format!("// @implements {node}\n")
        } else {
            String::new()
        };
        repo.write(
            &format!("crates/filler/src/part_{at:05}.rs"),
            format!(
                "/// Part {at}.\n{marker}pub fn value_{at:05}(input: u32) -> u32 {{\n    input.wrapping_mul({at}).wrapping_add(1)\n}}\n"
            ),
        );
    }
    repo.write("crates/filler/src/lib.rs", lib);
}

/// The judged views of `ids`' verify rows: what a bundle's rows of them
/// must stay.
fn judged_views(repo: &LockRepo, ids: &[&str]) -> BTreeMap<String, Vec<Value>> {
    ids.iter()
        .map(|id| {
            (
                (*id).to_owned(),
                verify_rows(repo, id).iter().map(judged_view).collect(),
            )
        })
        .collect()
}

/// The REF groups RD-1 and TE-2 read, each at the default budget and at
/// u32::MAX.
const STEADY_GROUPS: [&[&str]; 4] = [
    &["GOAL-01"],
    &["PIECE-COUNTER"],
    &["LAW-BOUND"],
    &["GOAL-01", "LAW-STEP"],
];

/// RD-1 (constant cost) and TE-3's first leg: over the golden with a
/// criterion of GOAL-01, each bundle of [`STEADY_GROUPS`] (2 000 and
/// u32::MAX) is byte-identical (text, JSON, stderr, exit) after 1 000
/// unrelated code files are written (500 bound to PREM-001, a node no
/// bundle here holds) and PREM-001 is bound once more in a target's own
/// file, `counter.rs`; the premise: those bindings are rows of PREM-001,
/// and the bundles' nodes are judged as before. M: rows taken by the
/// target's file.
#[test]
fn bundle_is_unchanged_by_unrelated_code() {
    let repo = golden("bb-rd1");
    criterion(&repo, 0);
    repo.commit_all("a criterion of GOAL-01");
    let ids = [
        "GOAL-01",
        "PIECE-COUNTER",
        "LAW-BOUND",
        "LAW-STEP",
        "LAW-ODD",
        "cap-check/PASS-01",
    ];
    let reads = || -> Vec<Read> {
        STEADY_GROUPS
            .iter()
            .flat_map(|refs| ["2000", ALL].map(|budget| bundle_of(&repo, refs, budget)))
            .collect()
    };
    let before = reads();
    for read in &before {
        read.answered();
        assert!(
            !read.layer("bindings").is_empty() && !read.layer("tests").is_empty()
                || read.json()["budget"] == 2000,
            "premise: {} places rows in both layers\n{}",
            read.what,
            read.show()
        );
    }
    let judged = judged_views(&repo, &ids);
    let premise_rows = verify_rows(&repo, "PREM-001").len();

    repo.append(
        COUNTER,
        "\n// @implements PREM-001\npub fn premise_spare() -> u32 {\n    2\n}\n",
    );
    let premise = verify_rows(&repo, "PREM-001");
    assert_eq!(
        premise.len(),
        premise_rows + 1,
        "premise: a new row of PREM-001"
    );
    assert!(
        premise
            .iter()
            .any(|row| row["file"] == COUNTER
                && row["symbol"] == "crates/tally::counter::premise_spare"),
        "premise: a PREM-001 row in counter.rs"
    );
    filler(&repo, 1_000, "PREM-001");
    let document = judged_with(&repo.root, "PREM-001", &vars_of(&repo)).document;
    assert_eq!(
        list(&document["rows"]).len() as u64 + document["omitted"]["rows"].as_u64().unwrap_or(0),
        (premise_rows + 501) as u64,
        "premise: 500 more rows of PREM-001 (verify --node cuts its JSON)"
    );
    // The premise is asserted after the bundles, so a bundle that takes
    // rows by file (its seam shared with `verify --node`) is reported as
    // the bundle's change; the message says whether the premise held.
    let premise_held = judged_views(&repo, &ids) == judged;
    for (read, again) in before.iter().zip(reads()) {
        assert_eq!(
            again.bytes(),
            read.bytes(),
            "{}: unchanged by unrelated code (premise, the nodes judged as before: \
             {premise_held})\n{}",
            read.what,
            first_difference(&again, read)
        );
    }
    assert!(
        premise_held,
        "premise: the bundles' nodes are judged as before"
    );
}

// ------------------------------------------------------------------ RD-2

/// GOAL-01 depends on PREM-001: PREM-001 is a layer-7 neighbour of
/// GOAL-01's bundle.
fn neighbour(repo: &LockRepo) {
    repo.replace(
        GOAL_MD,
        "reviewed: 2026-10-10\n---",
        "reviewed: 2026-10-10\nlinks:\n  depends_on: [PREM-001]\n---",
    );
}

/// The rows of a layer of `read` whose `node` is `node`.
fn rows_of(read: &Read, layer: &str, node: &str) -> Vec<Value> {
    read.layer(layer)
        .into_iter()
        .filter(|row| row["node"] == node)
        .collect()
}

/// RD-2, layer 5: over the golden with [`drift`], [`halves`], [`nested`],
/// a criterion, a verifying record and a neighbour of GOAL-01, each
/// bundle's layer 5 holds its targets' and their sections' rows, never a
/// `verifies` row, never a candidate's (PREM-001, a neighbour; PASS-01 and
/// PREM-002, criteria; PIECE-COUNTER, LAW-BOUND's ancestor), never a
/// sibling's or the comment that only cites a target. GOAL-01 (by ID,
/// alias and path, alike) keeps its lock-only `marker_gone` row, its alias
/// marker's and its look-alike's; LAW-BOUND its lock-only `symbol_gone`
/// row and its look-alike's; PIECE-COUNTER its sections'; LAW-SOURCE
/// LAW-DEFAULT's, LAW-DEFAULT never LAW-SOURCE's. M: a neighbour's or a
/// `verifies` row in layer 5; a lock-only row dropped.
#[test]
fn bindings_layer_holds_target_rows_only() {
    let repo = golden("bb-rd2-five");
    drift(&repo);
    halves(&repo);
    nested(&repo);
    criterion(&repo, 0);
    verifying_record(&repo);
    neighbour(&repo);
    let lock_only = |id: &str, category: &str| -> Value {
        let rows = verify_rows(&repo, id);
        let row = rows
            .iter()
            .find(|row| {
                row["markers"] == json!([]) && !row["lock"].is_null() && row["category"] == category
            })
            .unwrap_or_else(|| panic!("premise: {id}'s lock-only {category} row: {rows:#?}"));
        judged_view(row)
    };
    {
        let read = &bundle_of(&repo, &["GOAL-01"], ALL);
        read.answered();
        let five = read.layer("bindings");
        assert!(
            five.iter().all(|row| row["relation"] != "verifies"),
            "{}: no verifies row in layer 5\n{}",
            read.what,
            read.show()
        );
        assert_eq!(
            read.layer("neighbours")
                .iter()
                .map(|item| text(&item["name"]).to_owned())
                .collect::<Vec<_>>(),
            ["PREM-001"],
            "premise: PREM-001 a neighbour\n{}",
            read.show()
        );
        assert!(
            !verify_rows(&repo, "PREM-001").is_empty(),
            "premise: the neighbour has rows"
        );
        assert_eq!(
            nodes_of(&five),
            owned(&["GOAL-01", LOOK_ALIKE]),
            "{}: GOAL-01's rows only, never a candidate's\n{}",
            read.what,
            read.show()
        );
        let shown: Vec<Value> = five.iter().map(shown_view).collect();
        assert!(
            shown.contains(&lock_only("GOAL-01", "marker_gone")),
            "{}: GOAL-01's lock-only row\n{}",
            read.what,
            read.show()
        );
        assert!(
            five.iter()
                .any(|row| row["symbol"] == "crates/tally::counter::spare"
                    && row["node"] == "GOAL-01"
                    && row["sync"] == "unlocked"),
            "{}: the alias marker's row, keyed GOAL-01",
            read.what
        );
        assert!(
            five.iter()
                .any(|row| row["node"] == LOOK_ALIKE && row["category"] == "homoglyph"),
            "{}: the look-alike's row",
            read.what
        );
        let nine = read.layer("tests");
        assert!(
            nine.iter().all(|row| row["relation"] == "verifies"),
            "{}: layer 9 holds verifies rows only",
            read.what
        );
        assert!(
            rows_of(read, "tests", "PREM-001").is_empty(),
            "{}: never the neighbour's test",
            read.what
        );
    }
    let goal = bundle_of(&repo, &["GOAL-01"], ALL).json();
    for other in ["AIM-01", GOAL_MD] {
        let read = bundle_of(&repo, &[other], ALL);
        for (key, _, _) in CODE_LAYERS {
            assert_eq!(
                read.json()["layers"][key],
                goal["layers"][key],
                "{other}: GOAL-01's {key}"
            );
        }
    }

    let law = bundle_of(&repo, &["LAW-BOUND"], ALL);
    law.answered();
    let five = law.layer("bindings");
    let shown: Vec<Value> = five.iter().map(shown_view).collect();
    assert!(
        shown.contains(&lock_only("LAW-BOUND", "symbol_gone")),
        "LAW-BOUND's lock-only row\n{}",
        law.show()
    );
    assert_eq!(
        nodes_of(&five),
        owned(&["LAW-BOUND", LAW_LOOK_ALIKE]),
        "never the parent's or a sibling's\n{}",
        law.show()
    );
    assert!(
        five.iter().all(|row| row["file"] != NESTED),
        "never the mention\n{}",
        law.show()
    );
    assert!(
        law.layer("tests")
            .iter()
            .any(|row| row["node"] == "LAW-BOUND" && row["file"] == BOUNDS),
        "LAW-BOUND's own test in layer 9\n{}",
        law.show()
    );

    let piece = bundle_of(&repo, &["PIECE-COUNTER"], ALL);
    let nodes = nodes_of(&piece.layer("bindings"));
    assert_eq!(
        nodes,
        owned(&[
            "PIECE-COUNTER",
            "LAW-BOUND",
            "LAW-STEP",
            "LAW-ODD",
            LAW_LOOK_ALIKE,
        ]),
        "the document holds its sections' rows, nobody else's\n{}",
        piece.show()
    );
    for (reference, wanted) in [
        (
            "PIECE-LIMITS",
            &["PIECE-LIMITS", "LAW-SOURCE", "LAW-DEFAULT"][..],
        ),
        (
            "docs/spec/limits.md",
            &["PIECE-LIMITS", "LAW-SOURCE", "LAW-DEFAULT"],
        ),
        ("LAW-SOURCE", &["LAW-SOURCE", "LAW-DEFAULT"]),
        ("LAW-DEFAULT", &["LAW-DEFAULT"]),
    ] {
        let read = bundle_of(&repo, &[reference], ALL);
        assert_eq!(
            nodes_of(&read.layer("bindings")),
            owned(wanted),
            "{reference}: its own and its sections' rows only\n{}",
            read.show()
        );
    }
    bundle_differential(
        &repo,
        CODE_A,
        &[
            &["GOAL-01"],
            &["LAW-BOUND"],
            &["PIECE-COUNTER"],
            &["LAW-SOURCE"],
        ],
    );
}

/// RD-2, layer 9: over the golden with a large criterion of GOAL-01
/// (`cap-check/PASS-01`, its text mentions GOAL-01) and a record that
/// `verifies` GOAL-01 (`PREM-002`), GOAL-01's layer 9 holds GOAL-01's own
/// `verifies` row and both criteria's tests, layer 5 none of the criteria's
/// rows; at the budget one token below the one that places PASS-01, PASS-01
/// is left out (a `criteria` tail entry) and its test is still placed in
/// layer 9 ("placed or not"). M: layer 9's set without the criteria, or
/// without the criteria left out.
#[test]
fn tests_layer_reaches_criteria_tests() {
    let repo = golden("bb-rd2-nine");
    criterion(&repo, 40);
    verifying_record(&repo);
    let read = bundle_of(&repo, &["GOAL-01"], ALL);
    read.answered();
    let criteria: Vec<String> = read
        .layer("criteria")
        .iter()
        .map(|item| text(&item["name"]).to_owned())
        .collect();
    assert_eq!(
        criteria,
        ["PASS-01", "PREM-002"],
        "premise: both criteria\n{}",
        read.show()
    );
    for node in ["GOAL-01", "cap-check/PASS-01", "PREM-002"] {
        assert!(
            !rows_of(&read, "tests", node).is_empty(),
            "{node}'s test in layer 9\n{}",
            read.show()
        );
        if node != "GOAL-01" {
            assert!(
                rows_of(&read, "bindings", node).is_empty(),
                "{node}: a criterion's rows never in layer 5"
            );
        }
    }
    bundle_differential(&repo, CODE_A, &[&["GOAL-01"]]);

    let placed = |budget: u64| -> bool {
        bundle_of(&repo, &["GOAL-01"], &budget.to_string())
            .layer("criteria")
            .iter()
            .any(|item| item["name"] == "PASS-01")
    };
    let (mut low, mut high) = (u64::from(minimum_of(&repo, &["GOAL-01"])), 20_000u64);
    assert!(
        !placed(low) && placed(high),
        "premise: PASS-01 placed only with room"
    );
    while high - low > 1 {
        let middle = (low + high) / 2;
        if placed(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    let left_out = bundle_of(&repo, &["GOAL-01"], &(high - 1).to_string());
    left_out.answered();
    assert!(
        left_out
            .tail()
            .iter()
            .any(|entry| entry["name"] == "PASS-01" && entry["layer"] == "criteria"),
        "premise: PASS-01 left out at {}\n{}",
        high - 1,
        left_out.show()
    );
    assert!(
        !rows_of(&left_out, "tests", "cap-check/PASS-01").is_empty(),
        "a criterion left out still brings its test\n{}",
        left_out.show()
    );
}

// ------------------------------------------------------------------ RD-4

/// The minimum a `--budget 1` bundle names (`… minimum of <n> tokens …`).
fn minimum_of(repo: &LockRepo, refs: &[&str]) -> u32 {
    minimum_in(&repo.root, refs, &vars_of(repo))
}

fn minimum_in(cwd: &Path, refs: &[&str], vars: &[(OsString, OsString)]) -> u32 {
    let mut args = vec!["bundle"];
    args.extend_from_slice(refs);
    args.extend_from_slice(&["--budget", "1"]);
    let run = run_with(cwd, &args, vars);
    assert_eq!(run.code, 2, "a budget below the minimum\n{}", run.show());
    run.stderr
        .split("minimum of ")
        .nth(1)
        .and_then(|tail| tail.split(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no minimum named: {}", run.show()))
}

/// RD-4, the order: over the golden with a large criterion, a verifying
/// record and a neighbour of GOAL-01, the body's headings run Targets,
/// Criteria, Bindings, Neighbours, Tests (canon order 1-4, 5, 6-8, 9) and
/// the JSON's layers keep their nine keys in print order; at the least
/// budget that places PASS-01 (the first candidate, large), a row is left
/// out: the criterion took the room before any row. M: rows before layer 2
/// (in print or in fit order).
#[test]
fn bindings_follow_criteria() {
    let repo = golden("bb-rd4-order");
    criterion(&repo, 40);
    verifying_record(&repo);
    neighbour(&repo);
    let read = bundle_of(&repo, &["GOAL-01"], ALL);
    read.answered();
    assert_eq!(
        headings(&read.body()),
        ["Targets", "Criteria", "Bindings", "Neighbours", "Tests"],
        "the canon order\n{}",
        read.show()
    );
    let raw = &read.json_run.stdout;
    let open = raw.find("\"layers\":{").expect("layers") + "\"layers\":".len();
    assert_eq!(
        raw_order(raw, open),
        common::bundle::LAYERS,
        "the layers' keys in print order"
    );

    let placed = |budget: u64| -> Read { bundle_of(&repo, &["GOAL-01"], &budget.to_string()) };
    let has_pass = |read: &Read| {
        read.layer("criteria")
            .iter()
            .any(|item| item["name"] == "PASS-01")
    };
    let (mut low, mut high) = (u64::from(minimum_of(&repo, &["GOAL-01"])), 20_000u64);
    assert!(!has_pass(&placed(low)) && has_pass(&placed(high)));
    while high - low > 1 {
        let middle = (low + high) / 2;
        if has_pass(&placed(middle)) {
            high = middle;
        } else {
            low = middle;
        }
    }
    let tight = placed(high);
    let rows = read.layer("bindings").len() + read.layer("tests").len();
    let placed_rows = tight.layer("bindings").len() + tight.layer("tests").len();
    assert!(
        placed_rows < rows,
        "at {high}, PASS-01 placed before the rows: {placed_rows} of {rows} rows placed\n{}",
        tight.show()
    );
    let left: Vec<Value> = tight
        .tail()
        .into_iter()
        .filter(|entry| entry["layer"] == "bindings" || entry["layer"] == "tests")
        .collect();
    assert!(
        !left.is_empty() || tight.json()["more"].as_u64() > Some(0),
        "rows left out after the criterion\n{}",
        tight.show()
    );
}

/// RD-4, the frame: GOAL-01 has no candidate and no child section, so
/// without `[code]` its frame has no reserve; with `[code]` its rows count
/// in the reserve's n: the minimum grows, and at the minimum the body is
/// the frame, its `- <n> more` counting every row (layers 5 and 9), with
/// one token less exit 2. M: the reserve without rows.
#[test]
fn frame_reserve_counts_binding_rows() {
    let repo = golden("bb-rd4-frame");
    let all = bundle_of(&repo, &["GOAL-01"], ALL);
    all.answered();
    let rows = all.layer("bindings").len() + all.layer("tests").len();
    assert!(rows > 0, "premise: GOAL-01's rows\n{}", all.show());
    let without = without_code(&repo.root, || {
        let read = bundle_of(&repo, &["GOAL-01"], ALL);
        (minimum_of(&repo, &["GOAL-01"]), read)
    });
    assert_eq!(
        headings(&without.1.body()),
        ["Targets"],
        "premise: no candidate, no reserve"
    );
    let low = minimum_of(&repo, &["GOAL-01"]);
    assert!(
        low > without.0,
        "the rows count in the reserve: minimum {low} with [code], {} without",
        without.0
    );
    let frame = bundle_of(&repo, &["GOAL-01"], &low.to_string());
    frame.answered();
    let body = frame.body();
    assert!(
        body.ends_with(&format!("\n## Not included\n- {rows} more\n")),
        "the frame's reserve counts the {rows} rows\n{}",
        frame.show()
    );
    assert_eq!(
        frame.json()["tokens"],
        json!(tokens_est(&body)),
        "the minimum is the frame's estimate"
    );
    assert!(u64::from(tokens_est(&body)) <= u64::from(low));
    let below = run_with(
        &repo.root,
        &["bundle", "GOAL-01", "--budget", &(low - 1).to_string()],
        &vars_of(&repo),
    );
    assert_eq!(below.code, 2, "below the minimum\n{}", below.show());
}

// ------------------------------------------------------------------ RD-5

/// The file of the bytes before 7a-2, beside this test.
fn before_file() -> PathBuf {
    repository_root().join("crates/specengine-cli/tests/bundle_bindings_before.json")
}

/// `b3:` and the BLAKE3 of `text`.
fn b3(text: &str) -> String {
    specengine_store::b3_hash(text.as_bytes())
}

/// The REFs pinned for `fixture_name`: every ID of its `expected.json`
/// (files and sections) and every document path; code-a's
/// [`GOLDEN_GROUPS`] REFs.
fn before_refs(fixture_name: &str) -> Vec<String> {
    if fixture_name == "code-a" {
        let mut refs: Vec<String> = Vec::new();
        for group in GOLDEN_GROUPS {
            for reference in group {
                if !refs.iter().any(|known| known == reference) {
                    refs.push((*reference).to_owned());
                }
            }
        }
        return refs;
    }
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(fixture(fixture_name).join("expected.json")).expect("expected.json"),
    )
    .expect("JSON");
    let mut refs: Vec<String> = Vec::new();
    for (path, file) in expected["files"].as_object().expect("files") {
        refs.push(path.clone());
        refs.extend(file["id"].as_str().map(str::to_owned));
        for section in list(&file["sections"]) {
            refs.extend(section["id"].as_str().map(str::to_owned));
        }
    }
    refs
}

/// The planned task pinned for `fixture_name`: its nodes, criteria and
/// affected nodes.
fn before_task(
    fixture_name: &str,
) -> (
    &'static [&'static str],
    &'static [&'static str],
    &'static [&'static str],
) {
    match fixture_name {
        "spec-a" => (
            &["MEC-STAMINA"],
            &["stamina-tuning/AC-07", "Holds in play."],
            &["RULE-STAM-REGEN"],
        ),
        "spec-b" => (&["REQ-001"], &["Checked by hand."], &["CMD-SYNC"]),
        "spec-c" => (&["JRN-UNLOCK"], &["Checked by hand."], &["FEE-RIDE"]),
        _ => (
            &["GOAL-01", "PIECE-COUNTER"],
            &["cap-check/PASS-01"],
            &["LAW-STEP", "PIECE-LIMITS"],
        ),
    }
}

/// Every case of the bytes before 7a-2, as `program` prints them: per
/// fixture (`code-a` without `[code]`, committed), each REF's bundle at the
/// default budget and at 10 000 (text, JSON, stderr, exit), then a planned
/// task's brief (made by `program`: its package's times are its own, the
/// brief prints none). With `check`, each answer is also checked to carry
/// no code layer: no Bindings or Tests heading, layers 5 and 9 `[]`, no
/// `bindings` note, no Bindings section in the brief.
fn before_cases(program: &Path, check: bool) -> Vec<Value> {
    let mut cases = Vec::new();
    for fixture_name in ["spec-a", "spec-b", "spec-c", "code-a"] {
        let repo = LockRepo::of(
            &format!("bb-before-{fixture_name}"),
            fixture_name,
            "repo",
            &[],
        );
        if fixture_name == "code-a" {
            let config = repo.read("specengine.toml");
            repo.write("specengine.toml", without_code_table(&config));
            repo.commit_all("no [code]");
        }
        let vars = vars_of(&repo);
        for reference in before_refs(fixture_name) {
            for budget in [None, Some("10000")] {
                let mut args = vec!["bundle", reference.as_str()];
                if let Some(budget) = budget {
                    args.extend(["--budget", budget]);
                }
                let read = read_program(program, &repo.root, &args, &vars);
                if check {
                    assert!(
                        !read.text.stderr.contains("bindings"),
                        "{fixture_name}: {}: no bindings note\n{}",
                        read.what,
                        read.show()
                    );
                    if read.code() == 0 {
                        let body = read.body();
                        assert!(
                            section(&body, "Bindings").is_none()
                                && section(&body, "Tests").is_none(),
                            "{fixture_name}: {}: no code heading\n{}",
                            read.what,
                            read.show()
                        );
                        for (key, _, _) in CODE_LAYERS {
                            assert_eq!(read.json()["layers"][key], json!([]), "{}", read.what);
                        }
                    }
                }
                cases.push(json!({
                    "case": format!("{fixture_name}: {}", args.join(" ")),
                    "exit": read.code(),
                    "text": b3(&read.text.stdout),
                    "json": b3(&read.json_run.stdout),
                    "stderr": b3(&read.text.stderr),
                }));
            }
        }
        let (nodes, criteria, affected) = before_task(fixture_name);
        let plan = repo.scratch.join("plan.md");
        fs::write(&plan, "1. Read the nodes.\n2. Check them.\n").expect("the plan");
        let mut args = vec!["task", "new"];
        for node in nodes {
            args.extend(["--nodes", node]);
        }
        args.extend([
            "--title",
            "Check the scope",
            "--goal",
            "Read the task's nodes.",
        ]);
        let made = run_program(program, &repo.root, &args, &vars);
        assert_eq!(made.code, 0, "{fixture_name}: task new\n{}", made.show());
        let plan_path = plan.to_str().expect("a UTF-8 path").to_owned();
        let mut args = vec!["task", "plan", "T-0001", "--plan-file", plan_path.as_str()];
        for item in criteria {
            args.extend(["--criterion", item]);
        }
        for node in affected {
            args.extend(["--affected", node]);
        }
        let planned = run_program(program, &repo.root, &args, &vars);
        assert_eq!(
            planned.code,
            0,
            "{fixture_name}: task plan\n{}",
            planned.show()
        );
        let brief = run_program(program, &repo.root, &["task", "show", "T-0001"], &vars);
        if check {
            assert_eq!(brief.code, 0, "{fixture_name}: task show\n{}", brief.show());
            assert!(
                brief_block(&brief.stdout, "Bindings").is_none(),
                "{fixture_name}: no Bindings section without [code]\n{}",
                brief.show()
            );
            assert!(
                !brief.stderr.contains("bindings"),
                "{fixture_name}: no bindings note\n{}",
                brief.show()
            );
            let package = run_program(
                program,
                &repo.root,
                &["task", "show", "T-0001", "--json"],
                &vars,
            )
            .json();
            assert_eq!(
                package["bindings"],
                json!([]),
                "{fixture_name}: `bindings` []"
            );
            assert!(
                list(&package["notes"])
                    .iter()
                    .all(|note| !text(note).starts_with("bindings")),
                "{fixture_name}: no bindings note in the package"
            );
        }
        cases.push(json!({
            "case": format!("{fixture_name}: task show T-0001 (planned)"),
            "exit": brief.code,
            "text": b3(&brief.stdout),
            "stderr": b3(&brief.stderr),
        }));
    }
    cases
}

/// Writes the bytes before 7a-2 from the binary `SPECENGINE_BEFORE_SPEC`
/// names (6d66f0a's `spec`, built before this slice). Run once, by hand:
/// `SPECENGINE_BEFORE_SPEC=<binary> cargo nextest run -p specengine-cli
/// --test bundle_bindings record_the_bytes_before --run-ignored
/// ignored-only`. Never re-recorded from a later binary.
#[test]
#[ignore = "manual: records the bytes before 7a-2 from 6d66f0a's binary"]
fn record_the_bytes_before_bundle_bindings() {
    let program = PathBuf::from(
        std::env::var_os("SPECENGINE_BEFORE_SPEC").expect("SPECENGINE_BEFORE_SPEC: the binary"),
    );
    let cases = before_cases(&program, false);
    let again = before_cases(&program, false);
    assert_eq!(cases, again, "the binary's answers are stable");
    let mut out = String::from(
        "{\"recorded_with\": \"spec at 6d66f0a (7a-1 shipped, 7a-2 not started)\",\n\"cases\": [\n",
    );
    let lines: Vec<String> = cases.iter().map(Value::to_string).collect();
    out.push_str(&lines.join(",\n"));
    out.push_str("\n]}\n");
    fs::write(before_file(), out).expect("the record");
}

/// RD-5 and TE-4: without `[code]` a bundle and a brief are the bytes
/// before 7a-2: over `spec-a`, `spec-b` (two conventions), `spec-c` (a
/// non-Rust project) and `code-a` with its `[code]` table taken out, every
/// pinned case (each ID and document's bundle at two budgets: text, JSON,
/// stderr, exit; a planned task's brief) prints what 6d66f0a's binary
/// printed ([`before_cases`]), with no code heading, no `bindings` note,
/// layers 5 and 9 `[]`, the package's `bindings` `[]`. M: an empty heading
/// or a note.
#[test]
fn no_code_table_bundle_is_unchanged() {
    let record: Value = serde_json::from_str(
        &fs::read_to_string(before_file()).expect("bundle_bindings_before.json"),
    )
    .expect("the record parses");
    let want: BTreeMap<String, Value> = list(&record["cases"])
        .into_iter()
        .map(|case| (text(&case["case"]).to_owned(), case))
        .collect();
    let got: BTreeMap<String, Value> = before_cases(Path::new(common::SPEC), true)
        .into_iter()
        .map(|case| (text(&case["case"]).to_owned(), case))
        .collect();
    assert!(
        want.len() > 150,
        "premise: the record's cases: {}",
        want.len()
    );
    let names: Vec<&String> = want.keys().collect();
    assert_eq!(names, got.keys().collect::<Vec<_>>(), "the same cases");
    let changed: Vec<String> = want
        .iter()
        .filter(|(name, case)| got[*name] != **case)
        .map(|(name, case)| format!("{name}: before {case}, now {}", got[name]))
        .collect();
    assert!(
        changed.is_empty(),
        "{} case(s) changed without [code]:\n{}",
        changed.len(),
        changed.join("\n")
    );
}

// ------------------------------------------------------------------ RD-6

/// The first run cause `verify --node ID` names in `cwd` with `vars`.
fn first_cause(cwd: &Path, id: &str, vars: &[(OsString, OsString)]) -> String {
    let document = judged_with(cwd, id, vars).document;
    let causes = list(&document["cannot"]);
    assert!(
        !causes.is_empty(),
        "premise: verify --node {id} has a run cause here: {document}"
    );
    text(&causes[0]).to_owned()
}

/// What a degraded read places.
#[derive(Clone, Copy)]
enum Degraded<'a> {
    /// No row (a `[code]` error, a walk gap).
    NoRows,
    /// These keys listed, never judged (a lock or git cause).
    Listed(&'a [Value]),
}

/// The keys a listing gives for judged `rows` (no pairing): each row's
/// key, and a moved row's entry's key beside it.
fn listed_keys(rows: &[Value]) -> Vec<Value> {
    let mut keys = Vec::new();
    for row in rows {
        keys.push(row.clone());
        if row["anchor"]["state"] == "moved" {
            let from = &row["anchor"]["from"];
            keys.push(json!({
                "node": row["node"],
                "relation": row["relation"],
                "file": from["file"],
                "kind": from["kind"],
                "symbol": from["symbol"],
                "item_kind": from["item_kind"],
            }));
        }
    }
    keys
}

/// Rows never judged: `sync`, `category`, `detail`, `anchor` `null`.
fn assert_unjudged(rows: &[Value], context: &str) {
    for row in rows {
        for unjudged in ["sync", "category", "detail", "anchor"] {
            assert_eq!(
                row[unjudged],
                Value::Null,
                "{context}: {unjudged} null, never judged: {row}"
            );
        }
    }
}

/// RD-6's bundle: `spec bundle refs --budget u32::MAX` from `cwd` with
/// `vars`: exit 0; stderr and the JSON's notes the bundle's without
/// `[code]` and one note `bindings: <cause>` after them; with no row, every
/// byte of stdout the bundle's without `[code]`; with keys listed, the
/// body but its code layers the bundle's without `[code]`, the layers'
/// keys `keys` (by relation), never judged, each printed in its form with
/// `sync -`.
fn assert_bundle_degraded(
    cwd: &Path,
    refs: &[&str],
    vars: &[(OsString, OsString)],
    cause: &str,
    shows: Degraded<'_>,
    context: &str,
) {
    let read = bundle_in(cwd, refs, ALL, vars);
    let plain = without_code(cwd, || bundle_in(cwd, refs, ALL, vars));
    let context = format!("{context}: {}", read.what);
    assert_eq!(read.code(), 0, "{context}: exit 0\n{}", read.show());
    plain.answered();
    assert_eq!(
        read.text.stderr,
        format!("{}note: bindings: {cause}\n", plain.text.stderr),
        "{context}: the bundle's stderr and one note\n{}",
        read.show()
    );
    let mut notes = list(&plain.json()["notes"]);
    notes.push(json!(format!("bindings: {cause}")));
    assert_eq!(
        list(&read.json()["notes"]),
        notes,
        "{context}: the JSON's notes"
    );
    match shows {
        Degraded::NoRows => {
            assert_eq!(
                read.text.stdout, plain.text.stdout,
                "{context}: the text is the bundle's without [code]"
            );
            let mut want = plain.json();
            want["notes"] = json!(notes);
            assert_eq!(
                read.json(),
                want,
                "{context}: the JSON is the bundle's without [code]"
            );
        }
        Degraded::Listed(keys) => {
            let body = read.body();
            assert_eq!(
                without_code_layers(&body),
                plain.body(),
                "{context}: the body but its code layers"
            );
            assert_eq!(
                spec_part_json(&read.json()),
                spec_part_json(&plain.json()),
                "{context}: the JSON but its code layers"
            );
            assert_eq!(read.tail(), Vec::<Value>::new(), "{context}: all placed");
            for (key, heading, verifies) in CODE_LAYERS {
                let rows = read.layer(key);
                let want: Vec<Value> = keys
                    .iter()
                    .filter(|row| (row["relation"] == "verifies") == verifies)
                    .cloned()
                    .collect();
                assert_eq!(
                    keys_of(&rows),
                    keys_of(&want),
                    "{context}: layer {key}'s keys listed\n{}",
                    read.show()
                );
                assert_unjudged(&rows, &context);
                match section(&body, heading) {
                    Some(printed) => assert_eq!(
                        printed,
                        section_forms(&printed, &rows).join(ROW_GAP),
                        "{context}: {heading}"
                    ),
                    None => assert!(rows.is_empty(), "{context}: {heading} rows printed"),
                }
            }
        }
    }
}

/// RD-6's package: `spec task show <id>` from `cwd` with `vars`: the exit,
/// the brief but its Bindings section and the package but its `bindings`
/// those without `[code]`; one `note: bindings: <cause>` on stderr and once
/// in the package's notes; `bindings` `[]` with no Bindings section, or
/// the listed keys never judged.
fn assert_package_degraded(
    cwd: &Path,
    id: &str,
    vars: &[(OsString, OsString)],
    cause: &str,
    shows: Degraded<'_>,
    context: &str,
) {
    let read = task_show_in(cwd, id, vars);
    let plain = without_code(cwd, || task_show_in(cwd, id, vars));
    if plain.code() != 0 {
        // `task show` refuses here on its own (no git): the code layer
        // adds nothing, the exit the command's.
        assert_eq!(
            read.bytes(),
            plain.bytes(),
            "{context}: task show's own refusal, unchanged\n{}",
            first_difference(&read, &plain)
        );
        return;
    }
    assert_eq!(
        read.code(),
        0,
        "{context}: task show answers\n{}",
        read.show()
    );
    let note = format!("note: bindings: {cause}");
    assert_eq!(
        read.binding_notes(),
        std::slice::from_ref(&note),
        "{context}: one note\n{}",
        read.show()
    );
    let lines: Vec<&str> = read
        .text
        .stderr
        .lines()
        .filter(|line| *line != note)
        .collect();
    assert_eq!(
        lines,
        plain.text.stderr.lines().collect::<Vec<_>>(),
        "{context}: stderr but the note"
    );
    let package = read.json();
    assert_eq!(
        list(&package["notes"])
            .iter()
            .filter(|known| text(known) == format!("bindings: {cause}"))
            .count(),
        1,
        "{context}: the note once in the package\n{package}"
    );
    assert_eq!(
        package_spec_part(&package),
        package_spec_part(&plain.json()),
        "{context}: the package but its bindings"
    );
    assert_eq!(
        brief_spec_part(&read.text.stdout),
        brief_spec_part(&plain.text.stdout),
        "{context}: the brief but its Bindings section"
    );
    match shows {
        Degraded::NoRows => {
            assert_eq!(package["bindings"], json!([]), "{context}: no row");
            assert!(
                brief_block(&read.text.stdout, "Bindings").is_none(),
                "{context}: no Bindings section\n{}",
                read.show()
            );
        }
        Degraded::Listed(keys) => {
            let rows = list(&package["bindings"]);
            assert_eq!(keys_of(&rows), keys_of(keys), "{context}: the keys listed");
            assert_unjudged(&rows, context);
            assert_eq!(
                brief_block(&read.text.stdout, "Bindings"),
                Some(brief_bindings(&rows)),
                "{context}: the listed rows in the brief"
            );
        }
    }
}

/// A `git` that fails every call, first on `PATH`.
fn failing_git(dir: &Path) -> OsString {
    fs::create_dir_all(dir).expect("the shim's directory");
    let shim = dir.join("git");
    fs::write(
        &shim,
        "#!/bin/sh\necho 'fatal: refused by the test' >&2\nexit 128\n",
    )
    .expect("the shim");
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("chmod the shim");
    dir.as_os_str().to_owned()
}

/// A copy of `base` in a merge whose `spec.lock` conflicts.
fn unmerged_copy(base: &LockRepo, dir: &str) -> PathBuf {
    let root = clone_dir(base, dir);
    let git = |args: &[&str]| base.git.git(&root, args);
    git(&["checkout", "-q", "-b", "side"]);
    let entry = "verified_at = \"2026-10-08\"\n\n[[binding]]\nnode = \"PERK-REGEN\"";
    let lock = fs::read_to_string(root.join("spec.lock")).expect("the lock");
    assert!(lock.contains(entry), "premise: the entry");
    fs::write(
        root.join("spec.lock"),
        lock.replace(entry, &entry.replace("2026-10-08", "2026-10-09")),
    )
    .expect("side's lock");
    base.git.add_all(&root);
    base.git.commit(&root, "side lock");
    git(&["checkout", "-q", "main"]);
    fs::write(
        root.join("spec.lock"),
        lock.replace(entry, &entry.replace("2026-10-08", "2026-10-07")),
    )
    .expect("main's lock");
    base.git.add_all(&root);
    base.git.commit(&root, "main lock");
    let merge = base
        .git
        .git_output(&root, &["merge", "-q", "--no-edit", "side"], &[]);
    assert!(!merge.status.success(), "the scenario: spec.lock conflicts");
    root
}

/// The verify rows of `ids` in `cwd` with `vars`, whatever the causes.
fn rows_in(cwd: &Path, ids: &[&str], vars: &[(OsString, OsString)]) -> Vec<Value> {
    ids.iter()
        .flat_map(|id| list(&judged_with(cwd, id, vars).document["rows"]))
        .collect()
}

/// RD-6: a bundle and a package never fail on the code layer. Over the
/// golden with [`drift`] and a criterion of GOAL-01, bundles of GOAL-01,
/// PIECE-COUNTER and LAW-BOUND, and a planned task on GOAL-01 (LAW-STEP
/// affected): a refused `[code]` key, an unknown `frameworks` name and a
/// walk gap place no row (the bundle's every byte the bundle's without
/// `[code]` but the note); no git on `PATH`, a git that fails, a root in no
/// worktree (bundle), a lock not UTF-8 and a format-1 lock list the keys,
/// never judged; over the ECS game an unmerged `spec.lock` lists the
/// markers' keys. Each read exit 0, one `note: bindings: <cause>`, the
/// cause `verify --node`'s first `cannot:` line there. M: exit 2
/// propagated.
#[test]
fn bundle_survives_a_broken_code_layer() {
    let repo = golden("bb-degrade");
    drift(&repo);
    criterion(&repo, 0);
    let vars = vars_of(&repo);
    let root = repo.root.clone();
    let config = repo.read("specengine.toml");
    let task = verify_common::new_task(&repo, &["GOAL-01"]);
    verify_common::plan_task(&repo, &task, &["cap-check/PASS-01"], &["LAW-STEP"]);
    let groups: [(&[&str], &[&str]); 3] = [
        (&["GOAL-01"], &["GOAL-01", "cap-check/PASS-01"]),
        (
            &["PIECE-COUNTER"],
            &["PIECE-COUNTER", "LAW-BOUND", "LAW-STEP", "LAW-ODD"],
        ),
        (&["LAW-BOUND"], &["LAW-BOUND"]),
    ];
    // Layer 9 of GOAL-01 holds PASS-01's test only; layer 5 none of its rows.
    let bundle_keys = |rows: &[Value], refs: &[&str]| -> Vec<Value> {
        rows.iter()
            .filter(|row| {
                refs[0] != "GOAL-01"
                    || row["node"] != "cap-check/PASS-01"
                    || row["relation"] == "verifies"
            })
            .cloned()
            .collect()
    };
    let package_ids = ["GOAL-01", "LAW-STEP"];
    let healthy: Vec<(Vec<Value>, Vec<Value>)> = groups
        .iter()
        .map(|(refs, set)| {
            let rows = rows_in(&root, set, &vars);
            (bundle_keys(&listed_keys(&rows), refs), rows)
        })
        .collect();
    let package_listed = listed_keys(&rows_in(&root, &package_ids, &vars));
    assert!(
        healthy.iter().all(|(keys, _)| !keys.is_empty()),
        "premise: rows to list"
    );

    // No row: a refused `[code]` key, an unknown framework, a walk gap.
    let roots = "roots   = [\"crates\", \"data\", \"tools/loose.rs\"]";
    let spare = "crates/tally/src/spare.rs";
    for (context, edit) in [
        (
            "a refused [code] key",
            Some(format!("{roots}\nlanguages = [\"rust\"]")),
        ),
        (
            "an unknown framework",
            Some(format!("{roots}\nframeworks = [\"no-such-framework\"]")),
        ),
        ("a walk gap", None),
    ] {
        let _mode = match &edit {
            Some(edit) => {
                repo.replace("specengine.toml", roots, edit);
                None
            }
            None => {
                repo.write(spare, "pub fn spare_part() {}\n");
                Some(Mode::set(repo.root.join(spare), 0o000))
            }
        };
        let cause = first_cause(&root, "GOAL-01", &vars);
        for (refs, _) in &groups {
            assert_bundle_degraded(&root, refs, &vars, &cause, Degraded::NoRows, context);
        }
        assert_package_degraded(&root, &task, &vars, &cause, Degraded::NoRows, context);
        drop(_mode);
        repo.write("specengine.toml", &config);
        let _ = fs::remove_file(repo.root.join(spare));
    }

    // Listed, never judged: git missing or failing, no worktree.
    let nowhere = repo.scratch.dir("no-git-here");
    let no_git = with_var(&vars, "PATH", nowhere.as_os_str());
    let bad_git = with_var(&vars, "PATH", &failing_git(&repo.scratch.join("bad-git")));
    for (vars, context) in [(&no_git, "no git"), (&bad_git, "a git failure")] {
        let cause = first_cause(&root, "GOAL-01", vars);
        for ((refs, _), (keys, _)) in groups.iter().zip(&healthy) {
            assert_bundle_degraded(&root, refs, vars, &cause, Degraded::Listed(keys), context);
        }
        assert_package_degraded(
            &root,
            &task,
            vars,
            &cause,
            Degraded::Listed(&package_listed),
            context,
        );
    }
    let loose = repo.scratch.join("loose");
    copy_dir(&repo.root, &loose);
    fs::remove_dir_all(loose.join(".git")).expect("no .git");
    let loose = fs::canonicalize(&loose).expect("the loose copy");
    let cause = first_cause(&loose, "GOAL-01", &vars);
    for ((refs, _), (keys, _)) in groups.iter().zip(&healthy) {
        assert_bundle_degraded(
            &loose,
            refs,
            &vars,
            &cause,
            Degraded::Listed(keys),
            "no worktree",
        );
    }

    // A lock not UTF-8: the markers' keys alone.
    let lock = repo.lock().expect("the lock");
    let bare = clone_dir(&repo, "bare");
    fs::remove_file(bare.join("spec.lock")).expect("no lock");
    let markers_only: Vec<Vec<Value>> = groups
        .iter()
        .map(|(refs, set)| bundle_keys(&rows_in(&bare, set, &vars), refs))
        .collect();
    let package_markers = rows_in(&bare, &package_ids, &vars);
    repo.write("spec.lock", b"\xff\xfe".as_slice());
    let cause = first_cause(&root, "GOAL-01", &vars);
    for ((refs, _), keys) in groups.iter().zip(&markers_only) {
        assert_bundle_degraded(
            &root,
            refs,
            &vars,
            &cause,
            Degraded::Listed(keys),
            "a lock not UTF-8",
        );
    }
    assert_package_degraded(
        &root,
        &task,
        &vars,
        &cause,
        Degraded::Listed(&package_markers),
        "a lock not UTF-8",
    );
    repo.write("spec.lock", lock);

    // A format-1 lock: its entries' keys too.
    let old = golden_format_1("bb-degrade-format-1");
    drift(&old);
    criterion(&old, 0);
    let old_vars = vars_of(&old);
    let cause = first_cause(&old.root, "GOAL-01", &old_vars);
    for ((refs, _), (keys, _)) in groups.iter().zip(&healthy) {
        assert_bundle_degraded(
            &old.root,
            refs,
            &old_vars,
            &cause,
            Degraded::Listed(keys),
            "a format-1 lock",
        );
    }

    // An unmerged `spec.lock` over the ECS game: the markers' keys.
    let game = ecs("bb-degrade-unmerged");
    let game_vars = vars_of(&game);
    let bare = clone_dir(&game, "bare");
    fs::remove_file(bare.join("spec.lock")).expect("no lock");
    let merged = unmerged_copy(&game, "unmerged");
    assert_eq!(
        first_cause(&merged, "PERK-REGEN", &game_vars),
        "spec.lock is unmerged: run spec lock regen",
        "premise: cause 5"
    );
    for (refs, set) in [
        (&["PERK-REGEN"][..], &["PERK-REGEN"][..]),
        (&["TUNE-01"], &["TUNE-01"]),
    ] {
        let rows = rows_in(&bare, set, &game_vars);
        assert!(!rows.is_empty(), "premise: {set:?}'s markers");
        assert_bundle_degraded(
            &merged,
            refs,
            &game_vars,
            "spec.lock is unmerged: run spec lock regen",
            Degraded::Listed(&rows),
            "an unmerged lock",
        );
    }
}

// ------------------------------------------------------------------ TE-2

/// Copies `from` (the worktree and `.git`) to `to`, every entry created in
/// reverse name order.
fn copy_reversed(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("target");
    let mut entries: Vec<_> = fs::read_dir(from)
        .expect("directory")
        .map(|entry| entry.expect("entry"))
        .collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    for entry in entries {
        let target = to.join(entry.file_name());
        let kind = entry.file_type().expect("type");
        if kind.is_dir() {
            copy_reversed(&entry.path(), &target);
        } else if kind.is_file() {
            fs::write(&target, fs::read(entry.path()).expect("read")).expect("write");
            let mode = fs::metadata(entry.path()).expect("meta").permissions();
            fs::set_permissions(&target, mode).expect("mode");
        }
    }
}

/// The golden at `<scratch>/<dir>` with [`drift`], [`halves`] and a
/// criterion of GOAL-01.
fn golden_at(label: &str, dir: &str) -> LockRepo {
    let repo = LockRepo::of(label, "code-a", dir, &[]);
    repo.write(
        "spec.lock",
        fs::read(golden_file()).expect("fixtures/spec-lock/code-a.lock"),
    );
    repo.commit_all("the lock");
    drift(&repo);
    halves(&repo);
    criterion(&repo, 0);
    repo
}

/// The code files of a worktree (`.rs`, `.ron`), `.git` aside.
fn code_paths(root: &Path) -> Vec<String> {
    common::snapshot(root)
        .into_iter()
        .filter(|(path, bytes)| {
            bytes.is_some()
                && !path.starts_with(".git")
                && (path.ends_with(".rs") || path.ends_with(".ron"))
        })
        .map(|(path, _)| path)
        .collect()
}

/// TE-2: over the golden with [`drift`], [`halves`] and a criterion, each
/// bundle (text, JSON, stderr, exit; at 2 000 and u32::MAX) is
/// byte-identical repeated, from another root with its own `HOME`, from
/// another cwd, from a copy made in reverse creation order and from a
/// checkout of every committed code file CRLF; a planned task's `task
/// show` (brief and package) is byte-identical repeated, from the other
/// root (its own queue, the same clock) and from another cwd. M: rows in
/// walk or rowid order; a stored absolute path; a line read with its CR.
#[test]
fn bundle_and_package_are_byte_identical_across_roots_order_and_crlf() {
    let first = golden_at("bb-det-a", "repo");
    let second = golden_at("bb-det-b", "elsewhere/deeper");
    assert_eq!(second.head(), first.head(), "the same commits");
    let reversed = first.scratch.join("reversed");
    copy_reversed(&first.root, &reversed);
    let reversed = fs::canonicalize(&reversed).expect("the reversed copy");

    let crlf = clone_dir(&first, "crlf");
    first.git.git(&crlf, &["config", "core.autocrlf", "true"]);
    let committed: BTreeSet<String> = first
        .git_text(&["ls-files"])
        .lines()
        .map(str::to_owned)
        .collect();
    let dirty: BTreeSet<String> = first
        .git(&["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .split(|byte| *byte == 0)
        .filter(|entry| entry.len() > 3)
        .map(|entry| String::from_utf8_lossy(&entry[3..]).into_owned())
        .collect();
    let mut crlf_files = 0;
    for path in code_paths(&crlf) {
        if !committed.contains(&path) || dirty.contains(&path) {
            continue;
        }
        fs::remove_file(crlf.join(&path)).expect("remove");
        first.git.git(&crlf, &["checkout", "--", &path]);
        if fs::read(crlf.join(&path))
            .expect("checked out")
            .windows(2)
            .any(|pair| pair == b"\r\n")
        {
            crlf_files += 1;
        }
    }
    assert!(crlf_files > 0, "premise: CRLF code files in the worktree");

    let vars = vars_of(&first);
    let other_vars = vars_of(&second);
    let deeper = second.root.join("crates").join("tally");
    let mut groups: Vec<&[&str]> = STEADY_GROUPS.to_vec();
    groups.extend([&["PIECE-LIMITS"][..], &["cap-check/PASS-01"]]);
    for refs in groups {
        for budget in ["2000", ALL] {
            let reference = bundle_of(&first, refs, budget);
            reference.answered();
            let runs = [
                ("a repeat run", bundle_of(&first, refs, budget)),
                ("another root", bundle_of(&second, refs, budget)),
                ("another cwd", bundle_in(&deeper, refs, budget, &other_vars)),
                (
                    "a reverse creation order",
                    bundle_in(&reversed, refs, budget, &vars),
                ),
                ("a CRLF checkout", bundle_in(&crlf, refs, budget, &vars)),
            ];
            for (context, run) in runs {
                assert_eq!(
                    run.bytes(),
                    reference.bytes(),
                    "{refs:?} at {budget}: {context}\n{}",
                    first_difference(&run, &reference)
                );
            }
        }
    }

    let tasks: Vec<String> = [&first, &second]
        .iter()
        .map(|repo| {
            let task = verify_common::new_task(repo, &["GOAL-01"]);
            verify_common::plan_task(repo, &task, &["cap-check/PASS-01"], &["LAW-STEP"]);
            task
        })
        .collect();
    assert_eq!(tasks[0], tasks[1], "premise: one ID");
    let reference = task_show_in(&first.root, &tasks[0], &vars);
    reference.answered();
    assert!(
        !list(&reference.json()["bindings"]).is_empty(),
        "premise: the package's rows"
    );
    for (context, run) in [
        ("a repeat run", task_show_in(&first.root, &tasks[0], &vars)),
        (
            "another root",
            task_show_in(&second.root, &tasks[1], &other_vars),
        ),
        ("another cwd", task_show_in(&deeper, &tasks[1], &other_vars)),
    ] {
        assert_eq!(
            run.bytes(),
            reference.bytes(),
            "task show: {context}\n{}",
            first_difference(&run, &reference)
        );
    }
}

// ------------------------------------------------------------------ TE-3

/// The median of five timings of `call`.
fn median(mut call: impl FnMut() -> Duration) -> Duration {
    let mut times: Vec<Duration> = (0..5).map(|_| call()).collect();
    times.sort();
    times[2]
}

/// TE-3, manual (`#[ignore]`), release: the golden with a criterion and a
/// task on GOAL-01, then 10 000 unrelated `.rs` files (5 000 bound to
/// PREM-001): each bundle of [`STEADY_GROUPS`] and the task's `task show`
/// byte-identical; the medians of five warm runs of `bundle GOAL-01`,
/// `bundle PIECE-COUNTER --budget 10000`, `task show`, `show GOAL-01
/// --bindings` and plain `verify` printed (`--no-capture`) for the spec's
/// "Implementation"; the bundle at most plain verify plus the bundle
/// without `[code]`, `task show` under 1 s.
///
/// `cargo nextest run -p specengine-cli --release --test bundle_bindings
/// bundle_bindings_cost --run-ignored ignored-only --no-capture`
#[test]
#[ignore = "manual (TE-3): release, figures recorded in the spec"]
fn bundle_bindings_cost() {
    let repo = golden("bb-cost");
    criterion(&repo, 0);
    repo.commit_all("a criterion");
    let vars = vars_of(&repo);
    let task = verify_common::new_task(&repo, &["GOAL-01"]);
    let reads = || -> Vec<Read> {
        STEADY_GROUPS
            .iter()
            .flat_map(|refs| ["2000", ALL].map(|budget| bundle_of(&repo, refs, budget)))
            .chain([task_show_in(&repo.root, &task, &vars)])
            .collect()
    };
    let before = reads();
    filler(&repo, 10_000, "PREM-001");
    repo.commit_all("10 000 unrelated files");
    repo.spec_piped(&repo.root, &["index"]).code(0);
    for (read, again) in before.iter().zip(reads()) {
        assert_eq!(again.bytes(), read.bytes(), "{}: unchanged", read.what);
    }
    let time = |args: &[&str]| {
        median(|| {
            let started = Instant::now();
            let run = repo.spec_piped(&repo.root, args);
            let took = started.elapsed();
            // An answer: plain verify reports code-a's `cannot_verify`
            // rows with exit 2; the reads answer with exit 0.
            let answered = if args[0] == "verify" {
                [0, 1, 2].contains(&run.code)
            } else {
                run.code == 0
            };
            assert!(answered && !run.stdout.is_empty(), "{}", run.show());
            took
        })
    };
    let bundle = time(&["bundle", "GOAL-01"]);
    let document = time(&["bundle", "PIECE-COUNTER", "--budget", "10000"]);
    let show = time(&["task", "show", &task]);
    let node = time(&["show", "GOAL-01", "--bindings"]);
    let whole = time(&["verify"]);
    let config = repo.read("specengine.toml");
    repo.write("specengine.toml", without_code_table(&config));
    let plain = time(&["bundle", "GOAL-01"]);
    repo.write("specengine.toml", &config);
    eprintln!(
        "TE-3: code-a + 10 000 unrelated .rs files, warm indexes, median of 5: bundle GOAL-01 {} ms (without [code] {} ms), bundle PIECE-COUNTER --budget 10000 {} ms, task show {} ms, show GOAL-01 --bindings {} ms, plain verify {} ms",
        bundle.as_millis(),
        plain.as_millis(),
        document.as_millis(),
        show.as_millis(),
        node.as_millis(),
        whole.as_millis()
    );
    // docs/features/bundle-bindings.md TE-3: the bundle with rows at most
    // plain verify plus the bundle without `[code]`.
    assert!(
        bundle <= whole + plain,
        "bundle {bundle:?} against plain verify {whole:?} + the bundle without [code] {plain:?}"
    );
    assert!(
        show < Duration::from_secs(1),
        "task show: {show:?} against 1 s"
    );
}

/// TE-5, the owner's pilots by hand (`#[ignore]`, read-only): with
/// `SPECENGINE_PILOT_DIR` naming a pilot's root and `SPECENGINE_PILOT_REFS`
/// its REFs (comma-separated), a scratch `HOME`: each `spec bundle REF`,
/// warm (median of five), under 1 s; only the scratch data directory is
/// written (the pilot's files and `git status` as before). Release:
/// `SPECENGINE_PILOT_DIR=… SPECENGINE_PILOT_REFS=… cargo nextest run -p
/// specengine-cli --release --test bundle_bindings bundle_bindings_pilot
/// --run-ignored ignored-only --no-capture`.
#[test]
#[ignore = "manual (TE-5): the owner's pilots, read-only"]
fn bundle_bindings_pilot() {
    let root =
        PathBuf::from(std::env::var_os("SPECENGINE_PILOT_DIR").expect("SPECENGINE_PILOT_DIR"));
    let refs = std::env::var("SPECENGINE_PILOT_REFS").expect("SPECENGINE_PILOT_REFS");
    let scratch = common::Scratch::new("bb-pilot");
    let home = scratch.home("h");
    let path = std::env::var_os("PATH").unwrap_or_default();
    let vars: Vec<(OsString, OsString)> = vec![
        ("HOME".into(), home.into_os_string()),
        ("PATH".into(), path),
    ];
    let before = common::snapshot(&root);
    for reference in refs.split(',').map(str::trim).filter(|r| !r.is_empty()) {
        let first = run_with(&root, &["bundle", reference], &vars);
        assert_eq!(first.code, 0, "{}", first.show());
        let took = median(|| {
            let started = Instant::now();
            let run = run_with(&root, &["bundle", reference], &vars);
            assert_eq!(run.code, 0, "{}", run.show());
            started.elapsed()
        });
        eprintln!(
            "TE-5 pilot: bundle {reference} warm {} ms",
            took.as_millis()
        );
        assert!(took < Duration::from_secs(1), "{reference}: {took:?}");
    }
    assert!(common::snapshot(&root) == before, "the pilot is unchanged");
}

// ------------------------------------------------------- the ceiling, the tail

/// The ceiling and the tail: LAW-BOUND bound by 600 marked tests and 100
/// functions (each row about 190 characters) passes 40 000 characters at
/// `--budget` u32::MAX: the body stays within 40 000 characters; each
/// layer's placed rows are LAW-BOUND's, judged, each once, its estimate its
/// form's, the first of them `verify --node`'s first rows of that layer
/// in order (as far as its cut JSON lists them); every row not placed is a
/// tail entry (docs/features/bundle-bindings.md "Layers 5 and 9",
/// **Left out**: its six keys in order, `title` `<relation> LAW-BOUND`,
/// printed `- <name> | <title> | <n> tokens`) or counted in `- <k> more`,
/// each row once. M: the JSON holds every row; a row cut; rows past the
/// ceiling.
#[test]
fn rows_past_the_ceiling_are_left_out_whole() {
    let repo = golden("bb-ceiling");
    let mut tests = String::new();
    for at in 0..600 {
        tests.push_str(&format!(
            "// @verifies LAW-BOUND\n#[test]\nfn holds_the_cap_in_case_{at:04}() {{}}\n\n"
        ));
    }
    repo.write("crates/tally/tests/many.rs", tests);
    let mut code = String::new();
    for at in 0..100 {
        code.push_str(&format!(
            "// @implements LAW-BOUND\npub fn bounded_step_number_{at:04}(count: u32, cap: u32) -> u32 {{\n    count.min(cap)\n}}\n\n"
        ));
    }
    repo.write("crates/tally/src/many.rs", code);
    let read = bundle_of(&repo, &["LAW-BOUND"], ALL);
    read.answered();
    let body = read.body();
    assert!(
        body.chars().count() <= CAP,
        "the body within the ceiling: {} chars",
        body.chars().count()
    );
    assert_placed_keys(&read);
    let judged = judged_with(&repo.root, "LAW-BOUND", &vars_of(&repo));
    let listed = list(&judged.document["rows"]);
    let total = listed.len()
        + judged.document["omitted"]["rows"]
            .as_u64()
            .expect("omitted") as usize;
    assert!(total > 700, "premise: {total} rows");
    let mut placed_keys: BTreeSet<String> = BTreeSet::new();
    let mut placed = 0;
    let mut compared = 0;
    for (key, heading, verifies) in CODE_LAYERS {
        let rows = read.layer(key);
        let first: Vec<Value> = listed
            .iter()
            .filter(|row| (row["relation"] == "verifies") == verifies)
            .map(judged_view)
            .collect();
        let shown: Vec<Value> = rows.iter().map(shown_view).collect();
        let common = first.len().min(shown.len());
        compared += common;
        assert_eq!(
            shown[..common],
            first[..common],
            "{key}: the first placed rows are verify's first, in order"
        );
        for row in &rows {
            assert_eq!(row["node"], "LAW-BOUND", "{key}: LAW-BOUND's rows: {row}");
            assert!(row["sync"].is_string(), "{key}: judged: {row}");
            assert!(placed_keys.insert(key_of(row)), "{key}: a row once: {row}");
        }
        let printed = section(&body, heading).expect("the layer's section");
        let forms = section_forms(&printed, &rows);
        assert_eq!(printed, forms.join(ROW_GAP), "{key}: whole rows printed");
        for (row, form) in rows.iter().zip(&forms) {
            assert_eq!(
                row["tokens_est"],
                json!(tokens_est(form)),
                "{row}: its form's estimate"
            );
        }
        placed += rows.len();
    }
    assert!(placed < total, "premise: {placed} of {total} placed");
    assert!(
        compared > 20,
        "premise: rows compared with verify's: {compared}"
    );
    let tail = read.tail();
    if let Some(keys) = first_object_keys(&read.json_run.stdout, "tail") {
        assert_eq!(keys, TAIL_KEYS, "a tail entry's keys in order");
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for entry in &tail {
        let layer = text(&entry["layer"]);
        let relation = if layer == "tests" {
            "verifies"
        } else {
            "implements"
        };
        assert!(
            layer == "tests" || layer == "bindings",
            "a row's entry: {entry}"
        );
        assert!(
            entry["title"] == format!("{relation} LAW-BOUND")
                || entry["title"] == "configures LAW-BOUND",
            "{entry}: titled `<relation> <node>`"
        );
        assert!(
            entry["line"].as_u64().is_some_and(|line| line >= 1),
            "{entry}"
        );
        assert!(
            entry["tokens_est"].as_u64().is_some_and(|n| n > 0),
            "{entry}"
        );
        assert!(
            seen.insert(format!(
                "{}|{}|{}",
                entry["name"], entry["path"], entry["layer"]
            )),
            "an entry once: {entry}"
        );
        let placed_here = read.layer(layer).iter().any(|row| {
            row["file"] == entry["path"]
                && row["symbol"].as_str().unwrap_or_else(|| text(&row["file"]))
                    == text(&entry["name"])
        });
        assert!(!placed_here, "{entry}: a row not placed");
    }
    let more = read.json()["more"].as_u64().expect("more") as usize;
    assert_eq!(
        placed + tail.len() + more,
        total,
        "each row once: placed, listed or counted"
    );
    let lines: Vec<String> = tail
        .iter()
        .map(|entry| {
            format!(
                "- {} | {} | {} tokens",
                text(&entry["name"]),
                text(&entry["title"]),
                entry["tokens_est"]
            )
        })
        .chain((more > 0).then(|| format!("- {more} more")))
        .collect();
    assert_eq!(
        section(&body, "Not included")
            .map(|lines| lines.lines().map(str::to_owned).collect::<Vec<_>>()),
        Some(lines),
        "the not-included lines"
    );
}

// ------------------------------------------------- docs/features/tested-reads.md

/// A bundle's JSON without the keys a body line moves (`body`, `tokens`,
/// `chars`, `bytes`, `bundle_hash`).
fn without_body_keys(document: &Value) -> Value {
    let mut copy = document.clone();
    for key in ["body", "tokens", "chars", "bytes", "bundle_hash"] {
        copy.as_object_mut().expect("an object").remove(key);
    }
    copy
}

/// The header line each `tested | ` line of `body` sits under (the last
/// line before it that is not a tested line).
fn tested_headers(body: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = body.lines().collect();
    let mut out = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        if line.starts_with("tested | ") {
            let header = lines[..at]
                .iter()
                .rev()
                .find(|above| !above.starts_with("tested | "))
                .expect("a line above");
            out.push(((*header).to_owned(), (*line).to_owned()));
        }
    }
    out
}

/// docs/features/tested-reads.md RD-6 **M6** (a `tested` line in the
/// header form): over the evidence kiln and its evidence-free twin, in
/// states whose lines differ in length (`verified`, `stale spec_changed`,
/// `unverifiable spec_ahead`), for KILN-HOLD (GLZ-01 its criterion), the
/// document KILN-FIRING and CONE-01 with KILN-RAMP: the minimum is the
/// twin's, at it the body is the twin's byte for byte (the frame carries no
/// line) and one token less exits 2 for both; with room, each line sits
/// right under a target's (text or outline) or a placed criterion's
/// header, before its text, and names that node or one within it, one
/// line per placed node holding a block; with room for everything the
/// JSON's keys and every item value are the twin's (only `body`, `tokens`,
/// `chars`, `bytes`, `bundle_hash` move), and `bundle_hash` moves with the
/// state where the body differs only by the lines.
#[test]
fn bundle_minimum_ignores_tested() {
    let with = tested_kiln::evidence_kiln("bb-tested-with");
    let without = tested_kiln::evidence_kiln("bb-tested-without");
    tested_kiln::without_evidence(&without);
    let (with_vars, without_vars) = (vars_of(&with.repo), vars_of(&without.repo));
    let groups: [&[&str]; 3] = [&["KILN-HOLD"], &["KILN-FIRING"], &["CONE-01", "KILN-RAMP"]];
    let mut outlined = false;
    for state in [
        tested_kiln::EvidenceState::Verified,
        tested_kiln::EvidenceState::SpecChanged,
        tested_kiln::EvidenceState::Unverifiable,
    ] {
        state.write(&with);
        state.write(&without);
        for refs in groups {
            let context = format!("{state:?} {refs:?}");
            let low = minimum_in(with.root(), refs, &with_vars);
            assert_eq!(
                low,
                minimum_in(without.root(), refs, &without_vars),
                "{context}: the minimum is the twin's"
            );
            let at_low = bundle_in(with.root(), refs, &low.to_string(), &with_vars);
            at_low.answered();
            let twin = bundle_in(without.root(), refs, &low.to_string(), &without_vars);
            assert_eq!(
                at_low.bytes(),
                twin.bytes(),
                "{context}: at the minimum, the twin's bytes\n{}",
                at_low.show()
            );
            assert!(
                !at_low.text.stdout.contains("tested | "),
                "{context}: the frame carries no line"
            );
            let below = (low - 1).to_string();
            for (root, vars) in [(with.root(), &with_vars), (without.root(), &without_vars)] {
                let mut args = vec!["bundle"];
                args.extend_from_slice(refs);
                args.extend_from_slice(&["--budget", &below]);
                let run = run_with(root, &args, vars);
                assert_eq!(run.code, 2, "{context}: below the minimum\n{}", run.show());
            }

            // Every budget from the minimum up to the whole: the lines sit
            // under placed text or outline targets and placed criteria.
            let mut budgets: Vec<u32> = (0..12).map(|step| low + step * 40).collect();
            budgets.push(10_000);
            for budget in budgets {
                let read = bundle_in(with.root(), refs, &budget.to_string(), &with_vars);
                read.answered();
                let document = read.json();
                if budget == 10_000 {
                    // With room for every item: a line moves no item.
                    let twin = bundle_in(without.root(), refs, "10000", &without_vars);
                    assert_eq!(
                        without_body_keys(&document),
                        without_body_keys(&twin.json()),
                        "{context}: keys and item values are the twin's"
                    );
                }
                assert!(
                    !read.json_run.stdout.contains("\"tested\":"),
                    "{context}: no JSON key"
                );
                let mut headed: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
                for target in read.layer("targets") {
                    let form = text(&target["form"]).to_owned();
                    if form == "outline" {
                        outlined = true;
                    }
                    if form == "header" {
                        continue;
                    }
                    let name = text(&target["name"]).to_owned();
                    let set: BTreeSet<String> =
                        holder_set(with.root(), &[name.as_str()], &with_vars)
                            .into_iter()
                            .collect();
                    headed.insert(name, set);
                }
                for criterion in read.layer("criteria") {
                    let name = text(&criterion["name"]).to_owned();
                    headed.insert(
                        name.clone(),
                        BTreeSet::from([reference_of(
                            with.root(),
                            text(&criterion["path"]),
                            &name,
                        )]),
                    );
                }
                for (header, line) in tested_headers(&read.body()) {
                    let node = line.rsplit(" | ").next().expect("a node").to_owned();
                    let owner = headed
                        .iter()
                        .find(|(name, _)| header.starts_with(&format!("{name} | ")))
                        .unwrap_or_else(|| {
                            panic!(
                                "{context} at {budget}: {line:?} under {header:?}, not a placed text, outline or criterion header\n{}",
                                read.show()
                            )
                        });
                    assert!(
                        owner.1.contains(&node),
                        "{context} at {budget}: {line:?} under {}'s header names a node of it",
                        owner.0
                    );
                }
                let lines = tested_common::read_tested_lines(&read.body());
                let nodes: BTreeSet<String> = lines
                    .iter()
                    .map(|line| line.rsplit(" | ").next().expect("a node").to_owned())
                    .collect();
                let want: BTreeSet<String> = headed
                    .values()
                    .flatten()
                    .filter(|node| {
                        ["CONE-01", "KILN-HOLD", "KILN-RAMP", "glaze-check/GLZ-01"]
                            .contains(&node.as_str())
                    })
                    .cloned()
                    .collect();
                assert_eq!(
                    nodes, want,
                    "{context} at {budget}: a line per placed node with a block"
                );
                assert_eq!(lines.len(), nodes.len(), "{context}: each once");
            }
        }
        state.undo(&with);
        state.undo(&without);
    }
    assert!(outlined, "premise: an outline target met on the way");

    // `bundle_hash` moves with the state where only the lines differ.
    let hash = |kiln: &verify_tests_common::Kiln, vars: &[(OsString, OsString)]| {
        bundle_in(kiln.root(), &["KILN-HOLD"], ALL, vars).json()["bundle_hash"].clone()
    };
    let calm = (hash(&with, &with_vars), hash(&without, &without_vars));
    tested_kiln::EvidenceState::CodeChanged.write(&with);
    tested_kiln::EvidenceState::CodeChanged.write(&without);
    let moved = (hash(&with, &with_vars), hash(&without, &without_vars));
    assert_eq!(moved.1, calm.1, "premise: the twin's body does not move");
    assert_ne!(moved.0, calm.0, "`bundle_hash` moves with the state");
}
