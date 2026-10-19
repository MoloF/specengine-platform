//! docs/features/spec-verify.md TE-3 and RD-3 (AC-14): what cannot be
//! verified exits 2 in every mode, plain and with `--changed`, the report
//! printed (rows, `cannot:` lines, summary), never `ok`. Over
//! `fixtures/verify-ecs` (every binding sound, its nodes accepted), so the
//! exit is the scenario's alone; the hidden symbol over code-a's golden.
//! TE-3's own mutations: an `is_extra()` filter (an `ERROR` node read as a
//! comment, the broken item hashed); `cannot_verify` -> `ok`.

mod common;
mod lock_common;
mod verify_common;

use lock_common::LockRepo;
use verify_common::{
    BOUNDS, COUNTER, MODES, STAMINA, Verify, ecs, golden, golden_lock, item, set_mode, verify,
};

/// Every mode, plain and `--changed`: exit 2, `cannot-verify`, the report
/// printed with `check` the scenario's rows; the config restored after.
fn exits_2_everywhere(repo: &LockRepo, check: impl Fn(&Verify, &str)) {
    let config = repo.read("specengine.toml");
    for mode in MODES {
        repo.write("specengine.toml", &config);
        set_mode(&repo.root, mode);
        for extra in [&[][..], &["--changed"][..]] {
            let context = format!("{mode} {extra:?}");
            let run = verify(repo, extra);
            assert_eq!(run.code(), 2, "{context}:\n{}", run.text.show());
            assert_eq!(run.verdict(), "cannot-verify", "{context}");
            // Plain, `enforce-introduced` is judged as `enforce`; the mode
            // shown is the one the verdict used, a run cause or not.
            let judged = if extra.is_empty() && mode == "enforce-introduced" {
                "enforce"
            } else {
                mode
            };
            assert_eq!(run.json["mode"], judged, "{context}");
            assert!(
                run.summary()
                    .starts_with(&format!("verify: cannot-verify [{judged}]: ")),
                "{context}: {}",
                run.summary()
            );
            assert!(
                run.rows_where(|row| row["state"] == "ok"
                    && row["class"].is_null()
                    && row["compared"].is_null())
                    .is_empty(),
                "{context}: an ok row never compared"
            );
            check(&run, &context);
        }
    }
    repo.write("specengine.toml", &config);
}

/// The `regen_system` row's state, category and detail.
fn regen(run: &Verify) -> (String, String, String) {
    let row = run.row(
        "PERK-REGEN",
        "implements",
        STAMINA,
        Some("crates/game::stamina::regen_system"),
    );
    (
        row["state"].as_str().unwrap_or_default().to_owned(),
        row["category"].as_str().unwrap_or_default().to_owned(),
        row["detail"].as_str().unwrap_or_default().to_owned(),
    )
}

/// RD-3 / TE-3: a locked `fn` made unparseable: its entry and its keyless
/// marker `cannot_verify item_error`, exit 2 in every mode with the rows
/// printed. M: exit 0 under `observe`.
#[test]
fn unparseable_bound_item_exits_2_in_every_mode() {
    let repo = ecs("verify-unparseable");
    repo.replace(
        STAMINA,
        "(stamina.current + rate * dt)",
        "(stamina.current + rate * )",
    );
    exits_2_everywhere(&repo, |run, context| {
        assert_eq!(
            regen(run),
            ("cannot_verify".into(), "item_error".into(), "other".into()),
            "{context}"
        );
        assert_eq!(run.json["counts"]["cannot"], 2, "{context}");
        run.has_line("cannot | cannot_verify item_error other | PERK-REGEN | implements | crates/game/src/stamina.rs | item crates/game::stamina::regen_system function_item");
        run.has_line("cannot | cannot_verify item_error other | PERK-REGEN | implements | crates/game/src/stamina.rs | cannot_verify other");
    });
}

/// TE-3, Rules: two different broken bound items are never compared: each
/// reads `cannot_verify` with its own first category, `compared` null.
#[test]
fn two_broken_items_are_never_compared() {
    let repo = ecs("verify-two-broken");
    repo.replace(
        STAMINA,
        "(stamina.current + rate * dt)",
        "(stamina.current + rate * )",
    );
    repo.replace(
        STAMINA,
        "(stamina.current - cost * dt)",
        "(stamina.current - cost dt",
    );
    exits_2_everywhere(&repo, |run, context| {
        let drain = run.row(
            "PERK-DRAIN",
            "implements",
            STAMINA,
            Some("crates/game::stamina::drain_system"),
        );
        assert_eq!(
            (
                &drain["state"],
                &drain["category"],
                &drain["detail"],
                &drain["compared"]
            ),
            (
                &"cannot_verify".into(),
                &"item_error".into(),
                &"missing".into(),
                &serde_json::Value::Null
            ),
            "{context}"
        );
        assert_eq!(regen(run).0, "cannot_verify", "{context}");
        assert_eq!(run.json["counts"]["cannot"], 4, "{context}");
    });
}

/// docs/features/lock-levels.md TE-4 (AC-14 on the format-2 golden): two
/// different bound items of code-a broken at once, `GOAL-01`'s
/// `Tally::value` (`self.count +`) and `LAW-BOUND`'s `stops_at_cap`
/// (`tally.step(;`): each entry reads `cannot_verify item_error` with its
/// own first category, never compared (`compared` null), its marker now
/// keyless; exit 2 in every mode, plain and `--changed`. M: a broken item
/// compared against the entry's `sig_hash`/`body_hash`.
#[test]
fn two_broken_golden_items_are_never_compared() {
    let repo = golden("verify-two-broken-golden");
    repo.replace(
        COUNTER,
        "        self.count\n    }\n\n    // @implements GOAL-01@\n",
        "        self.count +\n    }\n\n    // @implements GOAL-01@\n",
    );
    repo.replace(
        BOUNDS,
        "    for _ in 0..11 {\n        tally.step();",
        "    for _ in 0..11 {\n        tally.step(;",
    );
    let value = item(
        "GOAL-01",
        "implements",
        COUNTER,
        "crates/tally::counter::Tally::value",
        "function_item",
    );
    let stops = item(
        "LAW-BOUND",
        "verifies",
        BOUNDS,
        "crates/tally::test:bounds::stops_at_cap",
        "function_item",
    );
    exits_2_everywhere(&repo, |run, context| {
        let mut details = Vec::new();
        for key in [value, stops] {
            let row = run.row(key.node, key.relation, key.file, key.symbol);
            assert_eq!(
                (&row["state"], &row["category"], &row["compared"]),
                (
                    &"cannot_verify".into(),
                    &"item_error".into(),
                    &serde_json::Value::Null
                ),
                "{context} {key:?}"
            );
            assert_eq!(row["lock"], golden_lock(key), "{context} {key:?}");
            assert_eq!(row["markers"], serde_json::json!([]), "{context} {key:?}");
            details.push(row["detail"].clone());
        }
        assert_eq!(details[0], "missing", "{context}");
        assert!(details[1].is_string(), "{context}: {details:?}");
    });
}

/// RD-3 / TE-3: an earlier unclosed item (`struct Oops {`) hides the bound
/// `fn` behind an orphan parse error: its entry `cannot_verify
/// hidden_by_error`, exit 2 in every mode. M: `broken_binding` behind an
/// orphan error.
#[test]
fn symbol_hidden_by_a_parse_error_cannot_verify() {
    let repo = golden("verify-hidden");
    repo.replace(
        BOUNDS,
        "// @verifies LAW-BOUND@2",
        "struct Oops {\n// @verifies LAW-BOUND@2",
    );
    let key = item(
        "LAW-BOUND",
        "verifies",
        BOUNDS,
        "crates/tally::test:bounds::stops_at_cap",
        "function_item",
    );
    exits_2_everywhere(&repo, |run, context| {
        let row = run.row(key.node, key.relation, key.file, key.symbol);
        assert_eq!(
            row,
            &verify_common::row_json(
                key,
                &verify_common::Says {
                    state: "cannot_verify",
                    category: Some("hidden_by_error"),
                    detail: None,
                    pre_existing: false,
                    markers: &[],
                    node_rev: Some(2),
                    lock: golden_lock(key),
                    compared: None,
                }
            ),
            "{context}"
        );
        run.has_line("cannot | cannot_verify hidden_by_error | LAW-BOUND | verifies | crates/tally/tests/bounds.rs | item crates/tally::test:bounds::stops_at_cap function_item");
    });
}

/// TE-3, Rules: an error in an unbound neighbour leaves a bound
/// error-free item judged as in a clean file: `ok` by its hash; a body
/// edit beside it `code_ahead`. (The assumption TE-3 pins.)
#[test]
fn error_in_an_unbound_neighbour_leaves_the_bound_item_judged() {
    let repo = ecs("verify-neighbour");
    repo.append(STAMINA, "\npub fn broken_helper() -> f32 {\n    1.0 +\n}\n");
    let run = verify(&repo, &[]);
    assert_eq!(run.code(), 0, "{}", run.text.show());
    assert_eq!(run.verdict(), "clean");
    let row = |run: &Verify| {
        let row = run.row(
            "PERK-REGEN",
            "implements",
            STAMINA,
            Some("crates/game::stamina::regen_system"),
        );
        (row["state"].clone(), row["compared"].clone())
    };
    assert_eq!(row(&run), ("ok".into(), "ast".into()));
    assert_eq!(run.json["cascade"]["by_ast"], 3);
    repo.replace(
        STAMINA,
        "(stamina.current + rate * dt)",
        "(stamina.current + 2.0 * rate * dt)",
    );
    let edited = verify(&repo, &[]);
    assert_eq!(row(&edited), ("code_ahead".into(), "ast".into()));
    assert_eq!(edited.code(), 0);
}

/// RD-3 / TE-3: a bound file made not UTF-8: every entry of it
/// `cannot_verify unreadable not_utf8`, never gone; exit 2 in every mode.
/// M: non-UTF-8 as gone.
#[test]
fn unreadable_bound_file_cannot_verify() {
    let repo = ecs("verify-not-utf8");
    let mut bytes = repo.read(STAMINA).into_bytes();
    bytes.extend_from_slice(b"\n// \xff\n");
    repo.write(STAMINA, bytes);
    exits_2_everywhere(&repo, |run, context| {
        for (node, symbol) in [
            ("PERK-REGEN", "crates/game::stamina::regen_system"),
            ("PERK-DRAIN", "crates/game::stamina::drain_system"),
        ] {
            let row = run.row(node, "implements", STAMINA, Some(symbol));
            assert_eq!(
                (
                    &row["state"],
                    &row["category"],
                    &row["detail"],
                    &row["markers"]
                ),
                (
                    &"cannot_verify".into(),
                    &"unreadable".into(),
                    &"not_utf8".into(),
                    &serde_json::json!([])
                ),
                "{context} {node}"
            );
        }
        let row = run.row(
            "TUNE-01",
            "configures",
            STAMINA,
            Some("crates/game::stamina::BASE_REGEN"),
        );
        assert_eq!(row["detail"], "not_utf8", "{context}");
        assert!(
            run.rows_where(|row| row["category"] == "file_gone")
                .is_empty()
        );
        assert_eq!(run.json["counts"]["cannot"], 3, "{context}");
        assert_eq!(run.json["delta"]["bindings"]["lost"], serde_json::json!([]));
    });
}

/// RD-3 / TE-3: a lock written with another `code_recipe`: run cause 6
/// naming both recipes, exit 2 in every mode, no row judged, the report
/// printed. M: recipe unchecked.
#[test]
fn lock_of_another_recipe_exits_2() {
    let repo = ecs("verify-recipe");
    repo.replace(
        "spec.lock",
        "tree-sitter-rust/0.24.2",
        "tree-sitter-rust/0.24.1",
    );
    exits_2_everywhere(&repo, |run, context| {
        let cannot = run.json["cannot"].as_array().expect("cannot");
        assert_eq!(cannot.len(), 1, "{context}: {cannot:?}");
        let cause = cannot[0].as_str().expect("a cause");
        assert!(cause.starts_with("spec.lock:4: "), "{context}: {cause}");
        assert!(
            cause.contains("tree-sitter-rust/0.24.1") && cause.contains("tree-sitter-rust/0.24.2"),
            "{context}: both recipes named: {cause}"
        );
        assert!(run.rows().is_empty(), "{context}");
        run.has_line(&format!("cannot: {cause}"));
    });
}
