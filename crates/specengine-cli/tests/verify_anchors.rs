//! docs/features/anchor-states.md TE-1, TE-2, TE-4 and the named RD-2 and
//! RD-3 tests: a binding renamed or moved with its marker is paired with
//! its vanished lock entry inside verify's judge, by fingerprints, one to
//! one, per group (node, relation, kind, item kind); a MOVED pair is one
//! row at the binding's key judged against the entry, an AMBIGUOUS entry
//! lists its candidates, every other entry row without a binding is
//! `lost`; nothing is written.
//!
//! Over `fixtures/anchor-states` (a kite game crate, a planner binary, RON
//! tuning data; `anchor_common`): every node but `KITE-DECOY` accepted
//! through the library (format 2), committed, then one or more of TE-1's
//! scenarios applied on disk. Each TE-1 test pins every row's key, state,
//! category, detail, class and anchor against the fixture's
//! `expected.json` (authored from the spec's table, never from the
//! command's output), the text's sub-lines under moved and ambiguous
//! rows, and TE-4: `spec.lock`'s bytes, `git status --porcelain=v1 -z
//! --untracked-files=all`, `HEAD`, the refs and the index unchanged by the
//! two runs (`verify_common::verify` checks every byte of the worktree and
//! the queue besides). Each named test says the mutation that turns it red
//! (`M:`).

#![cfg(unix)]

mod anchor_common;
mod common;
mod drift_common;
mod lock_common;
mod verify_common;

use std::collections::{BTreeMap, BTreeSet};

use anchor_common::{
    CLONES, FIXTURE, LAG, NODES, RENAME, SCENARIOS, SKETCH, SPOOL, TETHER, ambiguous, base,
    base_with, body_bytes, change, exact, expected, expected_rows, key_json, key_of, lost, moved,
    place, place_text, prints, scenario, view_as,
};
use common::fixture;
use drift_common::{proposals, raise, raised};
use lock_common::{LockRepo, item_code, node_blocks};
use serde_json::{Value, json};
use specengine_code::TRIVIAL_BODY_BYTES;
use specengine_core::drift::{HashedEntry, HashedRow, MovedFrom, drift_hash_input};
use specengine_core::verify::{MarkerRev, State};
use verify_common::{ROW_KEYS, Verify, set_mode, verify};

const SLUG: &str = "anchor-states";

/// The printed line of a non-`ok` row (`spec-verify-output.md` "Text").
fn row_line(row: &Value) -> String {
    let text = |field: &str| row[field].as_str().map(str::to_owned);
    let mut state = text("state").expect("state");
    for part in [text("category"), text("detail")].into_iter().flatten() {
        state.push(' ');
        state.push_str(&part);
    }
    let mut binding = format!(
        "{} {}",
        text("kind").expect("a keyed row"),
        text("symbol").expect("a keyed row")
    );
    if let Some(item_kind) = text("item_kind") {
        binding.push(' ');
        binding.push_str(&item_kind);
    }
    format!(
        "{}{} | {state} | {} | {} | {} | {binding}",
        if row["pre_existing"] == true {
            "pre-existing "
        } else {
            ""
        },
        text("class").expect("a printed row has a class"),
        text("node").expect("node"),
        text("relation").expect("relation"),
        text("file").expect("file"),
    )
}

/// The lines "Text" gives under a row: `  moved from <place>` under a
/// non-`ok` moved row, one `  candidate <place>` per candidate under an
/// ambiguous row, none else.
fn sub_lines(row: &Value) -> Vec<String> {
    let anchor = &row["anchor"];
    match anchor["state"].as_str() {
        Some("moved") if row["state"] != "ok" => {
            vec![format!("  moved from {}", place_text(&anchor["from"]))]
        }
        Some("ambiguous") => anchor["candidates"]
            .as_array()
            .expect("candidates")
            .iter()
            .map(|candidate| format!("  candidate {}", place_text(candidate)))
            .collect(),
        _ => Vec::new(),
    }
}

/// Every row of `run` is one of `wanted` and every one of `wanted` a row
/// (by key), each saying what `wanted` says; a non-`ok` row prints its
/// line followed by exactly its sub-lines, an `ok` row prints nothing.
fn assert_rows(run: &Verify, wanted: &[Value], context: &str) {
    let mut found: BTreeMap<String, &Value> = BTreeMap::new();
    for row in run.rows() {
        assert!(
            found.insert(key_of(row), row).is_none(),
            "{context}: two rows {}",
            key_of(row)
        );
    }
    let mut want: BTreeMap<String, &Value> = BTreeMap::new();
    for row in wanted {
        assert!(
            want.insert(key_of(row), row).is_none(),
            "{context}: expected.json names {} twice",
            key_of(row)
        );
    }
    let mut problems = Vec::new();
    for key in found.keys().filter(|key| !want.contains_key(*key)) {
        problems.push(format!("unexpected row {key}: {}", found[key]));
    }
    for key in want.keys().filter(|key| !found.contains_key(*key)) {
        problems.push(format!("missing row {key}"));
    }
    for (key, expected) in &want {
        let Some(row) = found.get(key) else { continue };
        let seen = view_as(row, expected);
        if &seen != *expected {
            problems.push(format!(
                "row {key}:\n  expected {expected}\n  found    {seen}"
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "{context}:\n{}\n{}",
        problems.join("\n"),
        run.text.show()
    );
    let lines = run.lines();
    for row in run.rows() {
        let line = row_line_or_none(row);
        let Some(line) = line else {
            let tail = format!(" | {} | {} |", row["node"], row["relation"]);
            assert!(
                !lines
                    .iter()
                    .any(|printed| printed.ends_with(&key_of(row)) && printed.contains(&tail)),
                "{context}: an ok row printed"
            );
            continue;
        };
        let at: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, printed)| **printed == line)
            .map(|(at, _)| at)
            .collect();
        assert_eq!(
            at.len(),
            1,
            "{context}: the line {line:?} once\n{}",
            run.text.show()
        );
        let subs = sub_lines(row);
        let printed: Vec<String> = lines[at[0] + 1..]
            .iter()
            .take_while(|printed| printed.starts_with("  "))
            .map(|printed| (*printed).to_owned())
            .collect();
        assert_eq!(
            printed,
            subs,
            "{context}: the lines under {line:?}\n{}",
            run.text.show()
        );
    }
}

/// [`row_line`] of a printed row; `None` for an `ok` row.
fn row_line_or_none(row: &Value) -> Option<String> {
    (row["state"] != "ok").then(|| row_line(row))
}

/// TE-4: what one scenario's runs must leave as they found it.
fn assert_read_only(repo: &LockRepo, before: &lock_common::RepoState, lock: &[u8], context: &str) {
    let after = repo.state();
    assert_eq!(after.head, before.head, "{context}: HEAD");
    assert_eq!(after.refs, before.refs, "{context}: the refs");
    assert_eq!(after.status, before.status, "{context}: git status");
    assert_eq!(after.index, before.index, "{context}: the index");
    assert_eq!(after.files, before.files, "{context}: the worktree");
    assert_eq!(
        repo.lock().expect("spec.lock"),
        lock,
        "{context}: spec.lock's bytes"
    );
}

/// The base with `names` applied, judged plain (text and JSON), checked
/// against `expected.json` and TE-4.
fn judged(label: &str, names: &[&str]) -> (LockRepo, Verify) {
    let repo = base(label);
    for name in names {
        change(&repo, name);
    }
    let before = repo.state();
    let lock = repo.lock().expect("spec.lock");
    let run = verify(&repo, &[]);
    assert_read_only(&repo, &before, &lock, &format!("{names:?}"));
    assert_rows(&run, &expected_rows(names), &format!("{names:?}"));
    (repo, run)
}

/// The one row of `node` whose symbol is `symbol`.
fn row_of<'a>(run: &'a Verify, node: &str, symbol: &str) -> &'a Value {
    let found = run.rows_where(|row| row["node"] == node && row["symbol"] == symbol);
    assert_eq!(found.len(), 1, "{node} {symbol}\n{}", run.text.show());
    found[0]
}

/// The `lock` object verify gives the entry of `node` whose block holds
/// `symbol` in `lock`'s text.
fn entry_lock(lock: &str, node: &str, symbol: &str) -> Value {
    let wanted = format!("\nsymbol = \"{symbol}\"\n");
    let found: Vec<String> = node_blocks(lock, node)
        .into_iter()
        .filter(|block| block.contains(&wanted))
        .collect();
    assert_eq!(found.len(), 1, "{node} {symbol} in:\n{lock}");
    let field = |name: &str| {
        found[0]
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{name} = ")))
            .unwrap_or_else(|| panic!("no {name}"))
            .trim_matches('"')
            .to_owned()
    };
    json!({
        "rev": field("rev").parse::<u32>().expect("a revision"),
        "commit": field("commit"),
        "verified_at": field("verified_at"),
    })
}

// -------------------------------------------------------------- shape

/// TE-1's table and `expected.json` agree: the 18 scenarios in the
/// table's order, each replacing only nodes the base accepts, the
/// baseline one row per key, every baseline row `ok` and `exact` but the
/// never-accepted `KITE-DECOY` (`unlocked`, null). Nothing in the fixture
/// or its expectations holds the threshold's number.
#[test]
fn every_scenario_has_its_expectations() {
    let document = expected();
    let names: Vec<&str> = document["scenarios"]
        .as_array()
        .expect("scenarios")
        .iter()
        .map(|scenario| scenario["name"].as_str().expect("a name"))
        .collect();
    assert_eq!(names, SCENARIOS);
    let accepted: BTreeSet<&str> = NODES.iter().copied().collect();
    for name in SCENARIOS {
        for node in scenario(name)["nodes"].as_array().expect("nodes") {
            assert!(
                accepted.contains(node.as_str().expect("a node")),
                "{name}: {node} is accepted by the base"
            );
        }
    }
    let baseline = document["baseline"].as_array().expect("baseline");
    let keys: BTreeSet<String> = baseline.iter().map(key_of).collect();
    assert_eq!(keys.len(), baseline.len(), "one baseline row per key");
    for row in baseline {
        let says = (
            row["state"].as_str().expect("state"),
            row["category"].as_str(),
            row["anchor"].clone(),
        );
        let want = match row["node"].as_str().expect("node") {
            "KITE-DECOY" => ("unlocked", None, Value::Null),
            _ => ("ok", None, exact()),
        };
        assert_eq!(says, want, "{}", key_of(row));
    }
    let number = TRIVIAL_BODY_BYTES.to_string();
    for (path, bytes) in common::snapshot(&fixture(FIXTURE)) {
        let Some(bytes) = bytes else { continue };
        let text = String::from_utf8(bytes).expect("UTF-8");
        assert!(!text.contains(&number), "{path} holds the threshold");
    }
}

/// The base itself: every row as `expected.json`'s baseline, the lock of
/// format 2 holding every accepted node, the run read-only.
#[test]
fn base_verifies_as_its_baseline() {
    let (repo, run) = judged("anchor-base", &[]);
    let lock = repo.lock_text();
    assert!(lock.contains("\nformat = 2\n"), "{lock}");
    for node in NODES {
        assert!(!node_blocks(&lock, node).is_empty(), "{node} locked");
    }
    assert!(node_blocks(&lock, "KITE-DECOY").is_empty());
    assert!(!run.summary().contains("moved"), "{}", run.summary());
}

/// RD-1's premise on real code: every body a scenario follows is at or
/// over the threshold, through `item_levels` (the decoy's too, so only
/// its group keeps it apart); the sized pair one under and at it.
#[test]
fn followed_bodies_clear_the_threshold() {
    let repo = base("anchor-bodies");
    let followed: [(&str, &str, &str); 17] = [
        (RENAME, "pub fn lift_force(", "function_item"),
        (
            "crates/sky/src/decoy.rs",
            "pub fn lift_echo(",
            "function_item",
        ),
        (anchor_common::CARRY, "pub fn drag_force(", "function_item"),
        (SKETCH, "pub fn tail_angle(", "function_item"),
        (anchor_common::STRAY, "pub fn gust_peak(", "function_item"),
        (anchor_common::TWINS, "pub fn pull_north(", "function_item"),
        (anchor_common::TWINS, "pub fn pull_south(", "function_item"),
        (CLONES, "pub fn gust_left(", "function_item"),
        (CLONES, "pub fn gust_right(", "function_item"),
        (anchor_common::CONTEST, "pub fn reel_low(", "function_item"),
        (anchor_common::CONTEST, "pub fn reel_high(", "function_item"),
        (anchor_common::MARKER, "pub fn spin_rate(", "function_item"),
        (anchor_common::RIG, "impl Rig {", "impl_item"),
        (TETHER, "pub fn line_tension(", "function_item"),
        (anchor_common::TOOL_MAIN, "fn plan_route(", "function_item"),
        (anchor_common::TOOL_MAIN, "fn plan_rest(", "function_item"),
        (LAG, "pub fn reel_speed(", "function_item"),
    ];
    let mut under = Vec::new();
    for (file, needle, kind) in followed {
        let bytes = body_bytes(&repo.read(file), needle, kind);
        if bytes < TRIVIAL_BODY_BYTES {
            under.push(format!("{file} {needle}: {bytes} body bytes"));
        }
    }
    assert!(
        under.is_empty(),
        "under the threshold:\n{}",
        under.join("\n")
    );
    let sized = repo.read(anchor_common::SIZED);
    assert_eq!(
        body_bytes(&sized, "pub fn trim_short(", "function_item") + 1,
        TRIVIAL_BODY_BYTES
    );
    assert_eq!(
        body_bytes(&sized, "pub fn trim_exact(", "function_item"),
        TRIVIAL_BODY_BYTES
    );
    // The file module's body: the lock's `body_bytes` for it.
    let lock = repo.lock_text();
    let banner = node_blocks(&lock, "KITE-BANNER");
    let bytes: u32 = banner[0]
        .lines()
        .find_map(|line| line.strip_prefix("body_bytes = "))
        .expect("body_bytes")
        .parse()
        .expect("a number");
    assert!(bytes >= TRIVIAL_BODY_BYTES, "the banner module: {bytes}");
}

/// Risks, S15: the rename tests left unedited stay green only while their
/// bodies stay trivial; each is under the threshold through
/// `item_levels`: `verify_states.rs` `renamed_symbol_is_symbol_gone` and
/// `drift_raise.rs` `renamed_symbol_raises_symbol_gone` (code-a's
/// `stops_at_cap`), `symbol_twins.rs`
/// `twin_added_after_lock_breaks_the_entry` (code-a's `Tally::value`) and
/// `ecs_system_cfg_twins_lock_and_verify_clean` (verify-ecs'
/// `regen_system`).
#[test]
fn existing_rename_bodies_stay_trivial() {
    let read = |name: &str, path: &str| {
        std::fs::read_to_string(fixture(name).join(path)).expect("a fixture file")
    };
    let items = [
        (
            read("code-a", "crates/tally/tests/bounds.rs"),
            "fn stops_at_cap(",
        ),
        (
            read("code-a", "crates/tally/src/counter.rs"),
            "pub fn value(&self)",
        ),
        (
            read("verify-ecs", "crates/game/src/stamina.rs"),
            "pub fn regen_system(",
        ),
    ];
    for (source, needle) in items {
        let line = anchor_common::line_of(&source, needle);
        let bytes = item_code(source.as_bytes(), line, "function_item", &["sig", "body"])
            .fingerprints
            .expect("fingerprints")
            .2;
        assert!(
            bytes < TRIVIAL_BODY_BYTES,
            "{needle}: {bytes} body bytes would be followed"
        );
    }
}

// ----------------------------------------------------- TE-1 and RD-2

/// RD-2, M5 (e's row kept beside the merged one): a function renamed
/// within its file is one row at its new key, `ok`, `moved` from the old
/// key; the old key makes no row, so `counts.rows` is the base's; its
/// `lock` is the old entry's, its markers the new key's; `compared` `ast`,
/// nothing changed; the summary `<n> ok (1 moved)`; the moved hint once,
/// after the per-state hints.
#[test]
fn moved_binding_is_one_row() {
    let name = "rename within the file";
    let (repo, run) = judged("anchor-rename", &[name]);
    let base_rows = expected_rows(&[]).len();
    assert_eq!(run.json["counts"]["rows"], json!(base_rows));
    assert_eq!(run.rows().len(), base_rows);
    let row = row_of(&run, "KITE-RENAME", "crates/sky::rename::lift_power");
    assert_eq!(
        row["lock"],
        entry_lock(
            &repo.lock_text(),
            "KITE-RENAME",
            "crates/sky::rename::lift_force"
        )
    );
    let marker_line = anchor_common::line_of(&repo.read(RENAME), "// @implements KITE-RENAME@1");
    assert_eq!(row["markers"], json!([{"line": marker_line, "rev": 1}]));
    assert_eq!(row["pre_existing"], false);
    assert!(
        run.rows_where(|row| row["symbol"] == "crates/sky::rename::lift_force")
            .is_empty()
    );
    let ok = run.json["counts"]["ok"].as_u64().expect("ok");
    assert!(
        run.summary().contains(&format!(" {ok} ok (1 moved);")),
        "{}",
        run.summary()
    );
    let hints: Vec<&str> = run
        .lines()
        .into_iter()
        .filter(|line| line.starts_with("hint: "))
        .collect();
    let moved_at = hints
        .iter()
        .position(|hint| *hint == MOVED_HINT)
        .unwrap_or_else(|| panic!("no moved hint in {hints:?}"));
    assert_eq!(hints.iter().filter(|hint| **hint == MOVED_HINT).count(), 1);
    // Nothing here raises a drift item (the decoy was never locked): the
    // moved hint is the last; its place before the drift hint is the
    // goldens'.
    assert_eq!(moved_at + 1, hints.len(), "{hints:?}");
    assert!(!hints.contains(&DRIFT_HINT), "{hints:?}");
}

/// TE-1: a function moved to another file (the old file kept) is followed
/// by `body`: `ok`, `moved`.
#[test]
fn moved_to_another_file_is_ok() {
    judged("anchor-carry", &["move to another file, the old file kept"]);
}

/// RD-2: renamed with a local renamed (the skeleton step, same file):
/// the merged row `code_ahead code_changed body`, `moved`, `levels.changed`
/// `["body"]`, its `moved from` line under it.
#[test]
fn renamed_and_edited_is_code_ahead() {
    let name = "rename plus a local renamed (skeleton step)";
    let before = base("anchor-sketch-premise");
    let old = prints(&before.read(SKETCH), "pub fn tail_angle(", "function_item");
    change(&before, name);
    let new = prints(&before.read(SKETCH), "pub fn tail_pitch(", "function_item");
    // The premise: another body, the same skeleton (and quote), one file.
    assert_ne!(old.0, new.0, "the body changed");
    assert_eq!(old.1, new.1, "the skeleton kept");
    assert!(old.3 >= TRIVIAL_BODY_BYTES && new.3 >= TRIVIAL_BODY_BYTES);
    drop(before);
    judged("anchor-sketch", &[name]);
}

/// TE-1: moved across files and edited: neither body nor skeleton (other
/// file) matches: the entry `broken_binding symbol_gone` `lost`, the new
/// key `unlocked`, null.
#[test]
fn moved_and_edited_across_files_is_lost() {
    judged("anchor-stray", &["move across files plus an edit"]);
}

/// TE-1 and RD-1's M2 on real code: of two renamed functions with bodies
/// one byte under and at `TRIVIAL_BODY_BYTES` (sized through
/// `item_levels`), only the one at it is followed.
#[test]
fn threshold_bodies_follow_only_from_the_threshold() {
    judged("anchor-trim", &["bodies of T-1 and T body_bytes, renamed"]);
}

/// TE-1: twins of one body whose quotes differ, both renamed: each
/// follows its own, both `ok`, `moved`.
#[test]
fn twins_with_distinct_quotes_each_follow() {
    judged("anchor-pair", &["twins, distinct quotes, both renamed"]);
}

/// TE-1: twins of one body and one quote, both renamed: both entries
/// `broken_binding` `ambiguous` with both candidates in key order (Fix
/// rows, a `candidate` line each), both new keys `unlocked`, null.
#[test]
fn twins_with_equal_quotes_list_their_candidates() {
    let (_, run) = judged("anchor-clone", &["twins, equal quotes, both renamed"]);
    assert_eq!(run.json["counts"]["fix"], 2, "{}", run.text.show());
}

/// TE-1: two entries of one body, one binding of it left, no quote equal:
/// both entries `ambiguous` with that one candidate, the binding
/// `unlocked`.
#[test]
fn contested_candidate_follows_neither() {
    judged("anchor-contest", &["a contested candidate"]);
}

/// RD-2's never-followed cell `marker_gone`: a marker moved to a
/// neighbour of the same body leaves its item: `broken_binding
/// marker_gone` `lost`, the neighbour `unlocked`, null. M: `marker_gone`
/// in E.
#[test]
fn moved_marker_is_never_followed() {
    judged("anchor-mark", &["a marker moved, the old item stays"]);
}

/// TE-1: a locked `impl` gaining an unmarked in-file twin is renamed
/// `impl Rig #1`; followed by `body`: `ok`, `moved`; the twin is no
/// candidate and makes no row.
#[test]
fn impl_gaining_a_twin_follows_by_body() {
    let (_, run) = judged(
        "anchor-rig",
        &["a locked impl gaining an unmarked in-file twin"],
    );
    assert!(
        run.rows_where(|row| row["symbol"] == "crates/sky::rig::impl Rig #2")
            .is_empty()
    );
}

/// TE-1: an unmarked twin inserted above two locked twins of one node:
/// never paired; the old `#1` now names the unmarked twin
/// (`broken_binding marker_gone`, `lost`), the old `#2` is compared with
/// the twin now there (`code_ahead code_changed body`, `exact`), `#3`
/// `unlocked`, null.
#[test]
fn ordinal_shift_is_never_paired() {
    judged("anchor-gauge", &["an ordinal shift"]);
}

/// RD-2's never-followed cell RON: a RON path renamed: `broken_binding
/// marker_gone` `lost`, the new path `unlocked`, null.
#[test]
fn ron_path_rename_is_never_followed() {
    judged("anchor-wind", &["a RON path renamed"]);
}

/// TE-1: a `//!` file renamed: its module followed by `body`, `ok`,
/// `moved`, kind `module`.
#[test]
fn file_module_rename_is_followed() {
    judged("anchor-banner", &["a //! file renamed"]);
}

/// RD-2, M6 (`path` not added): a `[path]` binding moved to another file
/// reads `code_ahead code_changed path`, `moved`, `compared` `ast`,
/// `levels.changed` `["path"]`, its `moved from` line under it.
#[test]
fn path_marker_reads_a_move_as_code_changed() {
    let (_, run) = judged("anchor-tether", &["a [path] binding moved"]);
    let row = row_of(&run, "KITE-TETHER", "crates/sky::spool::line_tension");
    assert_eq!(row["file"], SPOOL);
    assert_eq!(
        run.json["delta"]["bindings"]["stale"],
        json!([key_json(row)])
    );
}

/// TE-1: a library added beside `src/main.rs` renames every item of the
/// binary (`crates/tool::bin:tool::…`, `code-identity.md` "Known
/// limits"): every bound one followed, `ok`, `moved`.
#[test]
fn manifest_rename_follows_every_item() {
    let (_, run) = judged(
        "anchor-tool",
        &["a rename by the manifest only (a lib added)"],
    );
    assert!(
        run.summary().contains(" ok (2 moved);"),
        "{}",
        run.summary()
    );
}

/// TE-1: a lagging marker renamed: the merged row still lags,
/// `spec_ahead marker_behind`, `moved`, its `moved from` line under it.
#[test]
fn lagging_marker_follows_as_spec_ahead() {
    judged("anchor-lag", &["a lagging marker, renamed"]);
}

/// RD-2's never-followed cell `node_gone` (Q20): a record's ID renamed to
/// an alias: the entry `broken_binding node_gone` `lost`, the marker's key
/// under the new ID `unlocked`, null.
#[test]
fn renamed_node_is_never_followed() {
    judged("anchor-alias", &["an ID renamed to an alias"]);
}

/// RD-2's never-followed cell `hidden_by_error`: the locked function
/// renamed in a file with an orphan parse error: the entry `cannot_verify
/// hidden_by_error`, null (AC-14: never `ok`, never followed), the copy
/// `unlocked`, null; exit 2 in every mode.
#[test]
fn hidden_symbol_is_never_followed() {
    let (repo, run) = judged(
        "anchor-fog",
        &["a symbol gone from a file with an orphan parse error"],
    );
    assert_eq!(run.code(), 2, "{}", run.text.show());
    assert_eq!(run.verdict(), "cannot-verify");
    set_mode(&repo.root, "enforce");
    let strict = verify(&repo, &[]);
    assert_eq!(strict.code(), 2, "{}", strict.text.show());
}

/// RD-2: a `[path]` binding whose body is edited in place stays `ok`,
/// `exact`: C is empty, nothing compared (`compared` null,
/// `levels.changed` null); untouched it is `ok` by `blob`.
#[test]
fn exact_path_binding_stays_ok() {
    let repo = base("anchor-path-exact");
    let untouched = verify(&repo, &[]);
    let symbol = "crates/sky::tether::line_tension";
    let row = row_of(&untouched, "KITE-TETHER", symbol);
    assert_eq!(
        (&row["state"], &row["compared"], &row["anchor"]),
        (&json!("ok"), &json!("blob"), &exact())
    );
    repo.replace(
        TETHER,
        "let stretch = length * 0.03125 + weight;",
        "let stretch = length * 0.25 + weight * 2.0;",
    );
    let edited = verify(&repo, &[]);
    let row = row_of(&edited, "KITE-TETHER", symbol);
    assert_eq!(
        (
            &row["state"],
            &row["compared"],
            &row["anchor"],
            &row["levels"]
        ),
        (
            &json!("ok"),
            &Value::Null,
            &exact(),
            &json!({"lock": ["path"], "markers": ["path"], "changed": null})
        ),
        "{}",
        edited.text.show()
    );
}

// --------------------------------------------------------------- RD-3

/// The moved hint ("Output"): once, after the per-state hints, before the
/// drift hint.
const MOVED_HINT: &str = "hint: moved: commit, then spec lock accept <ID> on a terminal locks the new key and drops the old";

/// The drift hint (`spec-verify-output.md` "Hints"), last.
const DRIFT_HINT: &str =
    "hint: drift: commit first; spec propose drift queues HEAD's drift for the owner";

/// RD-3: `delta.bindings.moved` lists each moved row as `{"from": <the
/// entry's six key fields>, "to": <the row's>}`, in key order, a moved
/// `code_ahead` row in `stale` too; `[]` with no move. M: `moved` left
/// null, or an `ok` row left out.
#[test]
fn delta_lists_moved_pairs() {
    let (_, run) = judged(
        "anchor-delta-moved",
        &[
            "rename within the file",
            "rename plus a local renamed (skeleton step)",
            "a //! file renamed",
        ],
    );
    let pair = |node: &str, from: Value, to: Value| {
        json!({
            "from": {"node": node, "relation": "implements", "file": from["file"],
                     "kind": from["kind"], "symbol": from["symbol"], "item_kind": from["item_kind"]},
            "to": {"node": node, "relation": "implements", "file": to["file"],
                   "kind": to["kind"], "symbol": to["symbol"], "item_kind": to["item_kind"]},
        })
    };
    let item = |file: &str, symbol: &str| place(file, "item", symbol, Some("function_item"));
    let banner = |file: &str, symbol: &str| place(file, "module", symbol, None);
    assert_eq!(
        run.json["delta"]["bindings"]["moved"],
        json!([
            pair(
                "KITE-BANNER",
                banner("crates/sky/src/banner.rs", "crates/sky::banner"),
                banner(anchor_common::PENNANT, "crates/sky::pennant"),
            ),
            pair(
                "KITE-RENAME",
                item(RENAME, "crates/sky::rename::lift_force"),
                item(RENAME, "crates/sky::rename::lift_power"),
            ),
            pair(
                "KITE-SKETCH",
                item(SKETCH, "crates/sky::skeleton::tail_angle"),
                item(SKETCH, "crates/sky::skeleton::tail_pitch"),
            ),
        ]),
        "{}",
        run.json_run.stdout
    );
    let sketch = row_of(&run, "KITE-SKETCH", "crates/sky::skeleton::tail_pitch");
    assert_eq!(
        run.json["delta"]["bindings"]["stale"],
        json!([key_json(sketch)])
    );
    assert_eq!(run.json["delta"]["bindings"]["ambiguous"], json!([]));
    assert_eq!(run.json["delta"]["bindings"]["lost"], json!([]));
    assert!(
        run.summary().contains(" ok (2 moved);"),
        "{}",
        run.summary()
    );

    let (_, still) = judged("anchor-delta-none", &[]);
    assert_eq!(still.json["delta"]["bindings"]["moved"], json!([]));
}

/// RD-3: `moved`, `ambiguous` and `lost` are disjoint: `ambiguous` holds
/// the rows of `ambiguous` anchors (and a `cannot_verify ambiguous_*` row,
/// none here), `lost` the `broken_binding` rows that are not, `moved` the
/// merged rows, whose entries make no row. M: an ambiguous row in `lost`
/// too, or a moved entry's old key listed.
#[test]
fn delta_lists_are_disjoint() {
    let (_, run) = judged(
        "anchor-delta-disjoint",
        &[
            "rename within the file",
            "twins, equal quotes, both renamed",
            "a marker moved, the old item stays",
            "a RON path renamed",
        ],
    );
    let bindings = &run.json["delta"]["bindings"];
    let keys = |list: &str| -> BTreeSet<String> {
        bindings[list]
            .as_array()
            .unwrap_or_else(|| panic!("{list} is a list"))
            .iter()
            .map(key_of)
            .collect()
    };
    let moved_to: BTreeSet<String> = bindings["moved"]
        .as_array()
        .expect("moved")
        .iter()
        .map(|pair| key_of(&pair["to"]))
        .collect();
    let moved_from: BTreeSet<String> = bindings["moved"]
        .as_array()
        .expect("moved")
        .iter()
        .map(|pair| key_of(&pair["from"]))
        .collect();
    let rows_with = |anchor: &str| -> BTreeSet<String> {
        run.rows_where(|row| row["anchor"]["state"] == anchor)
            .into_iter()
            .map(key_of)
            .collect()
    };
    assert_eq!(moved_to, rows_with("moved"));
    assert_eq!(keys("ambiguous"), rows_with("ambiguous"));
    let broken: BTreeSet<String> = run
        .rows_where(|row| row["state"] == "broken_binding" && row["anchor"]["state"] != "ambiguous")
        .into_iter()
        .map(key_of)
        .collect();
    assert_eq!(keys("lost"), broken);
    assert_eq!(keys("lost").len(), 2, "spin_rate, root.gust_cap");
    assert_eq!(keys("ambiguous").len(), 2, "gust_left, gust_right");
    for (a, b) in [
        ("moved", "ambiguous"),
        ("moved", "lost"),
        ("ambiguous", "lost"),
    ] {
        let first = if a == "moved" {
            moved_to.clone()
        } else {
            keys(a)
        };
        assert!(
            first.is_disjoint(&keys(b)),
            "{a} and {b} share a key: {first:?} {:?}",
            keys(b)
        );
    }
    let all_rows: BTreeSet<String> = run.rows().into_iter().map(key_of).collect();
    assert!(
        moved_from.is_disjoint(&all_rows),
        "a moved entry's old key makes no row: {moved_from:?}"
    );
}

/// RD-3's goldens over three scenarios at once (a pure rename, a rename
/// with an edit, equal-quote twins): the whole text report (the moved and
/// candidate sub-lines, the hints with `moved` before `drift`, the
/// summary's ` (1 moved)`), and the JSON: every row's 17 keys in order,
/// `anchor` after `levels`, the moved `code_ahead` row's tail byte for
/// byte as "Output" shows it, an ambiguous row's anchor.
#[test]
fn text_and_json_goldens() {
    let (repo, run) = judged(
        "anchor-goldens",
        &[
            "rename within the file",
            "rename plus a local renamed (skeleton step)",
            "twins, equal quotes, both renamed",
        ],
    );
    let clones = |name: &str| format!("item crates/sky::clones::{name} function_item");
    let candidates = [
        format!("  candidate {CLONES} | {}", clones("gust_east")),
        format!("  candidate {CLONES} | {}", clones("gust_west")),
    ];
    let mut want: Vec<String> = vec![
        format!(
            "check | unlocked | KITE-CLONE | implements | {CLONES} | {}",
            clones("gust_east")
        ),
        format!(
            "fix | broken_binding symbol_gone | KITE-CLONE | implements | {CLONES} | {}",
            clones("gust_left")
        ),
    ];
    want.extend(candidates.iter().cloned());
    want.push(format!(
        "fix | broken_binding symbol_gone | KITE-CLONE | implements | {CLONES} | {}",
        clones("gust_right")
    ));
    want.extend(candidates.iter().cloned());
    want.extend([
        format!(
            "check | unlocked | KITE-CLONE | implements | {CLONES} | {}",
            clones("gust_west")
        ),
        "check | unlocked | KITE-DECOY | implements | crates/sky/src/decoy.rs | item crates/sky::decoy::lift_echo function_item".to_owned(),
        format!(
            "check | code_ahead code_changed body | KITE-SKETCH | implements | {SKETCH} | item crates/sky::skeleton::tail_pitch function_item"
        ),
        format!(
            "  moved from {SKETCH} | item crates/sky::skeleton::tail_angle function_item"
        ),
        "hint: broken_binding symbol_gone: commit, then `spec lock accept <ID>` on a terminal"
            .to_owned(),
        "hint: unlocked: commit, then `spec lock accept <ID>` on a terminal".to_owned(),
        "hint: code_ahead code_changed: commit, then `spec lock accept <ID>` on a terminal"
            .to_owned(),
        MOVED_HINT.to_owned(),
        DRIFT_HINT.to_owned(),
        // 28 rows: 26 of the base (25 `ok`), the lift's and the sketch's
        // one each, the clones' four; by blob the 21 rows of unchanged
        // files with an entry, by ast the two merged rows.
        "verify: observed [observe]: 2 fix, 4 check, 0 cannot, 22 ok (1 moved); 21 by blob, 2 by ast; 0 skipped".to_owned(),
    ]);
    assert_eq!(run.lines(), want, "{}", run.text.show());
    assert_eq!(run.text.stderr, "", "no note");

    // JSON: 17 keys per row, in order, `anchor` last.
    let raw = &run.json_run.stdout;
    for row in run.rows() {
        let keys: Vec<&str> = row
            .as_object()
            .expect("a row")
            .keys()
            .map(String::as_str)
            .collect();
        let mut sorted_keys = ROW_KEYS.to_vec();
        sorted_keys.sort_unstable();
        let mut seen = keys.clone();
        seen.sort_unstable();
        assert_eq!(seen, sorted_keys, "{row}");
    }
    assert_eq!(ROW_KEYS.last(), Some(&"anchor"));
    let lock = repo.lock_text();
    let entry = entry_lock(&lock, "KITE-SKETCH", "crates/sky::skeleton::tail_angle");
    let marker_line = anchor_common::line_of(&repo.read(SKETCH), "// @implements KITE-SKETCH@1");
    let sketch_tail = format!(
        "{{\"node\":\"KITE-SKETCH\",\"relation\":\"implements\",\"file\":\"{SKETCH}\",\"kind\":\"item\",\
         \"symbol\":\"crates/sky::skeleton::tail_pitch\",\"item_kind\":\"function_item\",\
         \"state\":\"code_ahead\",\"category\":\"code_changed\",\"detail\":\"body\",\"class\":\"check\",\
         \"pre_existing\":false,\"markers\":[{{\"line\":{marker_line},\"rev\":1}}],\"node_rev\":1,\
         \"lock\":{{\"rev\":{},\"commit\":{},\"verified_at\":{}}},\"compared\":\"ast\",\
         \"levels\":{{\"lock\":[\"sig\",\"body\"],\"markers\":[\"sig\",\"body\"],\"changed\":[\"body\"]}},\
         \"anchor\":{{\"state\":\"moved\",\"from\":{{\"file\":\"{SKETCH}\",\"kind\":\"item\",\
         \"symbol\":\"crates/sky::skeleton::tail_angle\",\"item_kind\":\"function_item\"}},\
         \"candidates\":null}}}}",
        entry["rev"], entry["commit"], entry["verified_at"]
    );
    assert!(
        raw.contains(&sketch_tail),
        "the moved row byte for byte:\n{sketch_tail}\nin:\n{raw}"
    );
    let left = row_of(&run, "KITE-CLONE", "crates/sky::clones::gust_left");
    assert_eq!(
        left["anchor"],
        ambiguous(vec![
            place(
                CLONES,
                "item",
                "crates/sky::clones::gust_east",
                Some("function_item")
            ),
            place(
                CLONES,
                "item",
                "crates/sky::clones::gust_west",
                Some("function_item")
            ),
        ])
    );
    let ambiguous_tail = format!(
        "\"anchor\":{{\"state\":\"ambiguous\",\"from\":null,\"candidates\":[\
         {{\"file\":\"{CLONES}\",\"kind\":\"item\",\"symbol\":\"crates/sky::clones::gust_east\",\"item_kind\":\"function_item\"}},\
         {{\"file\":\"{CLONES}\",\"kind\":\"item\",\"symbol\":\"crates/sky::clones::gust_west\",\"item_kind\":\"function_item\"}}]}}"
    );
    assert!(
        raw.contains(&ambiguous_tail),
        "{ambiguous_tail}\nin:\n{raw}"
    );
    let renamed = row_of(&run, "KITE-RENAME", "crates/sky::rename::lift_power");
    assert_eq!(
        renamed["anchor"],
        moved(place(
            RENAME,
            "item",
            "crates/sky::rename::lift_force",
            Some("function_item")
        ))
    );
    let steady = row_of(&run, "KITE-STEADY", "crates/sky::steady::hold_steady");
    assert_eq!(steady["anchor"], exact());
    let decoy = row_of(&run, "KITE-DECOY", "crates/sky::decoy::lift_echo");
    assert_eq!(decoy["anchor"], Value::Null);
    let _ = lost();
}

// --------------------------------------------------------------- TE-2

/// The base under `enforce-introduced` (committed before the lock).
fn introduced(label: &str) -> LockRepo {
    base_with(label, "repo", |repo| {
        set_mode(&repo.root, "enforce-introduced");
    })
}

/// TE-2 (AC-15), the disk: a pure rename, uncommitted, under
/// `enforce-introduced`: `spec verify --changed` exits 0 (before 5c: 1,
/// a Fix `broken_binding symbol_gone` introduced), its row `ok` `moved`;
/// the plain run, judged as `enforce`, exits 0 too.
#[test]
fn pure_rename_passes_changed_under_enforce_introduced() {
    let repo = introduced("anchor-te2-changed");
    change(&repo, "rename within the file");
    for extra in [&["--changed"][..], &[][..]] {
        let run = verify(&repo, extra);
        assert_eq!(run.code(), 0, "{extra:?}\n{}", run.text.show());
        assert_eq!(run.json["counts"]["fix"], 0, "{extra:?}");
        let row = row_of(&run, "KITE-RENAME", "crates/sky::rename::lift_power");
        assert_eq!(
            (&row["state"], &row["anchor"]["state"]),
            (&json!("ok"), &json!("moved")),
            "{extra:?}"
        );
    }
}

/// TE-2 (AC-15), a commit: a pure rename committed: `spec propose drift`
/// (MCP `report_drift` runs the same command) raises nothing for its node,
/// named or swept (the base drifts nowhere else: nothing stored);
/// the moved row is in `delta.bindings.moved` on every run (Q21), under
/// `--changed` too.
#[test]
fn committed_pure_rename_raises_nothing() {
    let repo = introduced("anchor-te2-raise");
    change(&repo, "rename within the file");
    repo.commit_all("lift renamed");
    let named = raised(&repo, &["KITE-RENAME"]);
    assert!(named.created().is_empty(), "{}", named.text());
    assert!(proposals(&repo, SLUG).is_empty(), "nothing stored");
    let swept = raise(&repo, &[]);
    swept.ok("the sweep");
    assert!(swept.created().is_empty(), "{}", swept.text());
    assert!(proposals(&repo, SLUG).is_empty(), "nothing stored");
    let changed = verify(&repo, &["--changed"]);
    assert_eq!(changed.code(), 0, "{}", changed.text.show());
    let moved = changed.json["delta"]["bindings"]["moved"]
        .as_array()
        .expect("moved")
        .clone();
    assert_eq!(moved.len(), 1, "{moved:?}");
    assert_eq!(moved[0]["to"]["symbol"], "crates/sky::rename::lift_power");
}

/// TE-2: a rename plus an edit committed: the moved `code_ahead` row is
/// Pre-existing under `--changed` and under `--staged` (`HEAD` pairs the
/// same way), so `enforce-introduced` exits 0; an ambiguous Fix committed
/// alike is Pre-existing too.
#[test]
fn committed_rename_with_an_edit_is_pre_existing() {
    let repo = introduced("anchor-te2-pre");
    change(&repo, "rename plus a local renamed (skeleton step)");
    change(&repo, "a contested candidate");
    repo.commit_all("tail renamed and edited, a reel removed");
    for flag in ["--changed", "--staged"] {
        let run = verify(&repo, &[flag]);
        assert_eq!(run.code(), 0, "{flag}\n{}", run.text.show());
        let row = row_of(&run, "KITE-SKETCH", "crates/sky::skeleton::tail_pitch");
        assert_eq!(
            (&row["state"], &row["anchor"]["state"], &row["pre_existing"]),
            (&json!("code_ahead"), &json!("moved"), &json!(true)),
            "{flag}"
        );
        for symbol in ["reel_low", "reel_high"] {
            let row = row_of(
                &run,
                "KITE-CONTEST",
                &format!("crates/sky::contest::{symbol}"),
            );
            assert_eq!(
                (&row["state"], &row["anchor"]["state"], &row["pre_existing"]),
                (&json!("broken_binding"), &json!("ambiguous"), &json!(true)),
                "{flag} {symbol}"
            );
        }
        assert!(
            run.lines().iter().any(|line| line
                .starts_with("pre-existing check | code_ahead code_changed body | KITE-SKETCH |")),
            "{flag}\n{}",
            run.text.show()
        );
    }
}

// ------------------------------------------------- drift evidence (RD-2)

/// The `drift_hash` of `KITE-TETHER`'s item when its one row is the
/// moved `[path]` binding (`code_ahead code_changed path`), built by hand
/// from the lock at `HEAD` (the entry's facts: "`lock_*` are e's") with
/// `code` = `code` (place 17) and place 18 = `moved_from`.
fn tether_hash(repo: &LockRepo, code: Value, moved_from: Value) -> String {
    let lock = repo.lock_text();
    let blocks = node_blocks(&lock, "KITE-TETHER");
    assert_eq!(blocks.len(), 1, "{lock}");
    let block = &blocks[0];
    let field = |name: &str| {
        block
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{name} = ")))
            .unwrap_or_else(|| panic!("no {name}"))
            .trim_matches('"')
            .to_owned()
    };
    let lock_code: Vec<&str> = block
        .lines()
        .filter(|line| {
            ["levels = ", "sig_hash = ", "body_hash = ", "deps_hash = "]
                .iter()
                .any(|prefix| line.starts_with(prefix))
        })
        .collect();
    assert_eq!(lock_code.len(), 4, "{block}");
    let lock_code = lock_code.join("\n");
    let spec_hash = field("spec_hash");
    let file_blob = field("file_blob");
    let norm = repo.norm_hash("KITE-TETHER");
    let row = HashedRow {
        node: "KITE-TETHER",
        relation: "implements",
        file: SPOOL,
        kind: Some("item"),
        symbol: Some("crates/sky::spool::line_tension"),
        item_kind: Some("function_item"),
        state: State::CodeAhead,
        category: Some("code_changed"),
        detail: Some("path"),
        marker_revs: vec![MarkerRev::Written(1)],
        entry: Some(HashedEntry {
            rev: field("rev").parse().expect("a revision"),
            spec_hash: &spec_hash,
            code: Some(&lock_code),
            file_blob: &file_blob,
        }),
        rev: Some(1),
        norm_hash: Some(&norm),
        code: None,
        moved_from: None,
    };
    let input = drift_hash_input(&[row]);
    let (domain, rows) = input.split_once('\n').expect("the domain line");
    let mut rows: Value = serde_json::from_str(rows).expect("the rows' JSON");
    assert_eq!(rows[0].as_array().expect("a row").len(), 18, "{input}");
    rows[0][16] = code;
    rows[0][17] = moved_from;
    specengine_store::b3_hash(format!("{domain}\n{rows}").as_bytes())
}

/// RD-2 (S7, Q8): a moved `[path]` binding committed and raised: its
/// drift `code` (place 17) is null, never `""`, the move carried by place
/// 18 alone (`[file, kind, symbol, item_kind]` of the entry). The hash is
/// rebuilt by hand; core's own render of the same row with `moved_from`
/// gives it too. M: `path` added to `changed` before the code fact is
/// built (`Some("")`).
#[test]
fn path_change_adds_no_code_fact() {
    let repo = base("anchor-path-code");
    change(&repo, "a [path] binding moved");
    repo.commit_all("tether moved");
    let raised = raised(&repo, &["KITE-TETHER"]);
    let created = raised.created();
    assert_eq!(created.len(), 1, "{}", raised.text());
    let item = drift_common::proposal(&repo, SLUG, &created[0].0);
    let stored = item.drift.expect("a drift item").hash;
    let from = json!([
        TETHER,
        "item",
        "crates/sky::tether::line_tension",
        "function_item"
    ]);
    let want = tether_hash(&repo, Value::Null, from.clone());
    assert_eq!(stored, want, "code null, moved_from the entry's place");
    assert_ne!(
        stored,
        tether_hash(&repo, json!(""), from),
        "never an empty code fact"
    );
    assert_ne!(
        stored,
        tether_hash(&repo, Value::Null, Value::Null),
        "place 18 carries the move"
    );
    // Core renders `moved_from` so.
    let lock = repo.lock_text();
    let block = &node_blocks(&lock, "KITE-TETHER")[0];
    let field = |name: &str| {
        block
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{name} = ")))
            .unwrap_or_else(|| panic!("no {name}"))
            .trim_matches('"')
            .to_owned()
    };
    let lock_code = block
        .lines()
        .filter(|line| {
            ["levels = ", "sig_hash = ", "body_hash = ", "deps_hash = "]
                .iter()
                .any(|prefix| line.starts_with(prefix))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let (spec_hash, file_blob) = (field("spec_hash"), field("file_blob"));
    let norm = repo.norm_hash("KITE-TETHER");
    let core = HashedRow {
        node: "KITE-TETHER",
        relation: "implements",
        file: SPOOL,
        kind: Some("item"),
        symbol: Some("crates/sky::spool::line_tension"),
        item_kind: Some("function_item"),
        state: State::CodeAhead,
        category: Some("code_changed"),
        detail: Some("path"),
        marker_revs: vec![MarkerRev::Written(1)],
        entry: Some(HashedEntry {
            rev: field("rev").parse().expect("a revision"),
            spec_hash: &spec_hash,
            code: Some(&lock_code),
            file_blob: &file_blob,
        }),
        rev: Some(1),
        norm_hash: Some(&norm),
        code: None,
        moved_from: Some(MovedFrom {
            file: TETHER,
            kind: "item",
            symbol: "crates/sky::tether::line_tension",
            item_kind: Some("function_item"),
        }),
    };
    assert_eq!(
        specengine_store::b3_hash(drift_hash_input(&[core]).as_bytes()),
        want
    );
    // The stored row is verify's, its anchor kept as evidence (S9).
    let rows = &item_rows(&repo, &created[0].0);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(
        rows[0]["anchor"],
        moved(place(
            TETHER,
            "item",
            "crates/sky::tether::line_tension",
            Some("function_item")
        ))
    );
    assert_eq!(rows[0]["levels"]["changed"], json!(["path"]));
}

/// An item's stored rows, as `spec verify --json` wrote them.
fn item_rows(repo: &LockRepo, id: &str) -> Vec<Value> {
    drift_common::proposal(repo, SLUG, id)
        .drift
        .expect("drift")
        .stored_rows
        .iter()
        .map(|row| serde_json::to_value(row).expect("a row"))
        .collect()
}

// --------------------------------------------------------------- RD-4

/// RD-4's guard (S13: no runtime counter exists): the pairing hashes B in
/// the cascade's one `made()`, so `verify_rows.rs` holds one `.parse(`
/// outside its `#[cfg(test)]` modules, a static scan as `code_refs_walk.rs`
/// `one_parse_per_rederived_file`. M: B parsed apart from the cascade.
#[test]
fn verify_rows_parses_each_file_once() {
    let file = "crates/specengine-cli/src/verify_rows.rs";
    let whole = std::fs::read_to_string(common::repository_root().join(file)).expect(file);
    let scanned = whole
        .find("\n#[cfg(test)]\nmod ")
        .map_or(whole.as_str(), |at| &whole[..at]);
    assert!(
        scanned.contains("fn judge_side("),
        "{file}: the judge is scanned"
    );
    assert_eq!(scanned.matches(".parse(").count(), 1, "{file}: one parse");
}

/// RD-4 (canon `spec-verify-anchors`, "The pairing": a trivial body on
/// either side never takes part): a trivial E draws no binding into B, so
/// a side whose lone entries are all trivial hashes nothing beyond the
/// cascade's. No runtime counter exists (S13) and a trivial E reads
/// `lost` whether or not its group's bindings were hashed
/// (`threshold_bodies_follow_only_from_the_threshold`), so the guard is a
/// static scan of `judge_side`: the keys it gives `select_bindings` are
/// built with the `TRIVIAL_BODY_BYTES` filter on E's prints. M: the
/// trivial-E filter dropped.
#[test]
fn trivial_entries_select_no_binding() {
    let file = "crates/specengine-cli/src/verify_rows.rs";
    let whole = std::fs::read_to_string(common::repository_root().join(file)).expect(file);
    let scanned = whole
        .find("\n#[cfg(test)]\nmod ")
        .map_or(whole.as_str(), |at| &whole[..at]);
    let judge = &scanned[scanned.find("fn judge_side(").expect("judge_side")..];
    let call = judge
        .find("select_bindings(&")
        .expect("judge_side selects B");
    let argument: String = judge[call + "select_bindings(&".len()..]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    let built_at = judge[..call]
        .rfind(&format!("let {argument}"))
        .unwrap_or_else(|| panic!("`{argument}` is built in judge_side"));
    let built = &judge[built_at..call];
    let statement = &built[..built.find(";\n").expect("one statement")];
    assert!(
        statement.contains(".body_bytes >= TRIVIAL_BODY_BYTES"),
        "{file}: the keys B is drawn from skip trivial entries:\n{statement}"
    );
}

// ------------------------------------------------------- known limits

/// canon `spec-verify-anchors`, "The pairing" (a binding whose file's tree
/// has an `ERROR` or `MISSING` node is never a candidate: its place there is
/// the parser's recovery): an unclosed `impl Lift {` inserted above the
/// locked, non-trivial `lift_force` (its marker unchanged) re-parents it to
/// `Lift::lift_force`, the function itself error-free. Never followed: the
/// entry `crates/sky::rename::lift_force` reads `broken_binding
/// symbol_gone`, `lost`; the marker's new key `unlocked`, null; under
/// `observe` exit 0, nothing `cannot_verify`. M: the broken-file skip
/// dropped (the pair read `ok`, `moved`).
#[test]
fn unclosed_impl_above_a_locked_fn_is_never_followed() {
    let repo = base("anchor-unclosed-impl");
    repo.replace(
        RENAME,
        "// @implements KITE-RENAME@1\n",
        "impl Lift {\n// @implements KITE-RENAME@1\n",
    );
    let before = repo.state();
    let lock = repo.lock().expect("spec.lock");
    let run = verify(&repo, &[]);
    assert_read_only(&repo, &before, &lock, "unclosed impl");
    let rows = run.rows_where(|row| row["node"] == "KITE-RENAME");
    let said: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!([
                row["symbol"],
                row["item_kind"],
                row["state"],
                row["category"],
                row["anchor"]
            ])
        })
        .collect();
    assert_eq!(
        said,
        [
            json!([
                "crates/sky::rename::Lift::lift_force",
                "function_item",
                "unlocked",
                null,
                null
            ]),
            json!([
                "crates/sky::rename::lift_force",
                "function_item",
                "broken_binding",
                "symbol_gone",
                lost()
            ]),
        ],
        "{}",
        run.text.show()
    );
    assert_eq!(run.code(), 0, "{}", run.text.show());
    assert_eq!(
        run.summary(),
        "verify: observed [observe]: 1 fix, 2 check, 0 cannot, 24 ok; 24 by blob, 0 by ast; 0 skipped",
        "{}",
        run.text.show()
    );
}
