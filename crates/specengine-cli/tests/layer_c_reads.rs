//! docs/features/layer-c-reads.md (11a-3): `spec symbols` shows the layer
//! C snapshot 11a-2's `spec resolve` stored — a `c` line per item target
//! after `  sig`, a `layer c:` line after the summary, the JSON keys
//! `c_state`, `moniker`, `agrees` and `layer_c` — and finds an item by its
//! shown moniker (the tier `moniker`, after `exact`).
//!
//! Every snapshot here is stored by 11a-2's Perl fake (`layer_c_common`)
//! replaying the golden `fixtures/layer-c/ra-mini.jsonl`, or a reply built
//! from it or from the code index (`Reply`), over a scratch copy of the
//! workspace-excluded `fixtures/ra-mini` (the fixture is only read) or of
//! `fixtures/code-bevy` (an ECS-style farm game with `[code] frameworks`:
//! roles; ADR-0008's second project); `specengine-ra` is never built. What
//! a read must show is computed by the oracle `layer_c_common::expected_c`
//! (canon `layer-c.md` "Join" restated over the code index's rows), not by
//! the code under test; `resolve`'s own counts are the tally's reference.
//!
//! - TE-1 `rows_tally_to_resolve_counts` (M6, inline modules dropped).
//! - TE-2 `symbols_text_is_unchanged_without_a_snapshot` (M1, a `c` line
//!   while absent): HEAD e266128's bytes, `fixtures/layer-c/symbols-head.json`.
//! - TE-3 `stale_snapshot_marks_every_c_line` (M2, the state unread),
//!   `changed_file_rows_are_unseen` (M3, no BLAKE3 check).
//! - TE-4 `moniker_query_ranks_after_exact` (M4, the tier after `suffix`),
//!   `shown_collision_finds_one_item` (M5, raw matching).
//! - TE-5 `c_line_follows_sig_before_roles`.
//! - TE-8 `symbols_with_a_snapshot_are_byte_identical_across_roots_and_order`
//!   (the plan of the moniker statement: the store's `src/symbols.rs` tests).
//! - TE-9 `layer_c_reads_cost` (`#[ignore]`, release).
//! - "Rules and edge cases": `gaps_show_their_states` (partial, a failed
//!   run keeps the snapshot), `c_lines_count_toward_the_output_cap`,
//!   `reads_leave_the_snapshot_and_the_worktree_untouched` (limits),
//!   `a_limited_query_keeps_its_targets_and_c_lines` (the limit after the
//!   merge over the narrowed pick), `an_unknown_stored_state_degrades_the_read`
//!   (a stored word this build does not know: a warning, no exit 2).
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod code_index_common;
mod common;
mod layer_c_common;
mod symbol_reads_common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use specengine_cli::OUTPUT_CAP_CHARS;

use common::{Run, snapshot, write};
use layer_c_common::{
    C_PREFIX, CLine, CONFIG, FILES, GENERATED, LAYER_C_FRESH, LAYER_C_KEYS, LAYER_C_STALE, Lab,
    Reply, Step, a_rows, c_lines_by_target, c_of_json, counts, expected_c, golden_head,
    golden_text, head_symbols, shown_targets,
};
use symbol_reads_common::{SYMBOL_KEYS, SYMBOLS_KEYS, assert_key_order};

/// The golden's one collision, shown (the joined rows' discriminators).
const COLLIDING: [(&str, &str); 2] = [
    ("arena::impl Health #1", "arena impl#[Health] #1"),
    ("arena::impl Health #2", "arena impl#[Health] #2"),
];

/// Its raw moniker.
const COLLIDING_RAW: &str = "arena impl#[Health]";

/// The golden's moniker of `arena::systems::regenerate` (arena/src/systems.rs).
const REGENERATE: &str = "arena systems/regenerate().";

/// The golden's moniker of `arena::Health` (arena/src/lib.rs).
const HEALTH: &str = "arena Health#";

/// The summary line of a `--file` or QUERY read over the lab's six files.
fn summary(shown: usize, total: usize) -> String {
    format!("symbols: {shown} of {total}; code files 6 (.rs 6, .ron 0), skipped 0")
}

/// The fake replays `reply`; `spec --json resolve` exit 0. Its counts and
/// `by_state`.
fn resolve_reply(lab: &Lab, reply: &str) -> (BTreeMap<String, u64>, Value) {
    lab.fake.clear();
    lab.fake.script(&[], reply);
    let run = lab.resolve_json();
    run.code(0);
    (counts(&run), run.json()["by_state"].clone())
}

/// `spec symbols --file` of each of the six files, text then JSON, both exit 0.
fn file_reads(lab: &Lab) -> Vec<(String, Run, Run)> {
    FILES
        .iter()
        .map(|path| {
            let text = lab.run(&["symbols", "--file", path]);
            text.code(0);
            let json = lab.run(&["--json", "symbols", "--file", path]);
            json.code(0);
            ((*path).to_owned(), text, json)
        })
        .collect()
}

/// `text` with every line break of a field read as `one_line` reads it.
fn one_line(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\n', '\r'], " ")
}

/// The text and the JSON of one read say the same: each target's `c` line
/// is its three keys (`stale` from `state`; a moniker as `one_line` shows
/// it), a module's and a RON path's keys are `null`, the last line is the
/// `layer c:` line of `state` and `layer_c.state` is `state` (`None`: no
/// snapshot, no line, `layer_c` `null`); every key in the spec's order.
fn assert_text_is_json(text: &Run, json: &Run, state: Option<&str>, context: &str) {
    let (targets, tail) = shown_targets(&text.stdout);
    let document = json.json();
    let listed = document["symbols"].as_array().expect("symbols");
    assert_eq!(targets.len(), listed.len(), "{context}: the same targets");
    let stale = state == Some("stale");
    for (shown, target) in targets.iter().zip(listed) {
        let from_text = shown.c_line().map(CLine::parse);
        let from_json = c_of_json(target, stale).map(|mut line| {
            line.moniker = line.moniker.as_deref().map(one_line);
            line
        });
        assert_eq!(from_text, from_json, "{context}: {}", shown.line);
        if target["target"] != json!("item") {
            for key in ["c_state", "moniker", "agrees"] {
                assert_eq!(target[key], Value::Null, "{context}: {key} on {target}");
            }
        }
        if target["c_state"] != json!("moniker") {
            assert_eq!(target["moniker"], Value::Null, "{context}: {target}");
        }
    }
    assert_key_order(&json.stdout, "$", &SYMBOLS_KEYS, 1, context);
    assert_key_order(
        &json.stdout,
        "$.symbols[]",
        &SYMBOL_KEYS,
        listed.len(),
        context,
    );
    match state {
        None => {
            assert_eq!(document.get("layer_c"), Some(&Value::Null), "{context}");
            assert!(
                !tail.iter().any(|line| line.starts_with("layer c:")),
                "{context}: {tail:?}"
            );
        }
        Some(state) => {
            assert_eq!(document["layer_c"]["state"], json!(state), "{context}");
            assert_key_order(&json.stdout, "$.layer_c", &LAYER_C_KEYS, 1, context);
            let want = if stale { LAYER_C_STALE } else { LAYER_C_FRESH };
            assert_eq!(
                tail.last().map(String::as_str),
                Some(want),
                "{context}: the last line\n{}",
                text.stdout
            );
            assert_eq!(
                tail.iter()
                    .filter(|line| line.starts_with("layer c:"))
                    .count(),
                1,
                "{context}: one `layer c:` line"
            );
        }
    }
}

/// `head`, a JSON document of HEAD's, with docs/features/layer-c-reads.md's
/// four keys `null`.
fn head_json_with_null_keys(head: &str) -> Value {
    let mut value: Value = serde_json::from_str(head).expect("HEAD's JSON");
    value["layer_c"] = Value::Null;
    if let Some(targets) = value["symbols"].as_array_mut() {
        for target in targets {
            for key in ["c_state", "moniker", "agrees"] {
                target[key] = Value::Null;
            }
        }
    }
    value
}

/// Every form of [`HEAD_SYMBOLS`](layer_c_common::HEAD_SYMBOLS) read in `lab`
/// as HEAD read it: exit and stderr equal; a text's stdout byte for byte, a
/// JSON document HEAD's with the four keys `null`, in the spec's order.
fn assert_heads_bytes(lab: &Lab, context: &str) {
    let head = head_symbols();
    assert!(head.len() >= 40, "every form captured: {}", head.len());
    let mut differ = Vec::new();
    for captured in &head {
        let args: Vec<&str> = captured.args.iter().map(String::as_str).collect();
        let run = lab.run(&args);
        let json = args.first() == Some(&"--json");
        let same_stdout = if json && !captured.stdout.is_empty() {
            common::one_json_document(&run.stdout).ok()
                == Some(head_json_with_null_keys(&captured.stdout))
        } else {
            run.stdout == captured.stdout
        };
        if same_stdout && json && !run.stdout.is_empty() {
            let at = format!("{context}: spec {args:?}");
            assert_key_order(&run.stdout, "$", &SYMBOLS_KEYS, 1, &at);
            let targets = run.json()["symbols"].as_array().map_or(0, Vec::len);
            assert_key_order(&run.stdout, "$.symbols[]", &SYMBOL_KEYS, targets, &at);
        }
        if run.code != captured.exit || run.stderr != captured.stderr || !same_stdout {
            differ.push(format!(
                "spec {args:?}: HEAD exit {}\n--- HEAD stdout\n{}--- HEAD stderr\n{}--- now\n{}",
                captured.exit,
                captured.stdout,
                captured.stderr,
                run.show()
            ));
        }
    }
    assert!(
        differ.is_empty(),
        "{context}: {} form(s) differ from HEAD's bytes:\n{}",
        differ.len(),
        differ.join("\n")
    );
}

/// The lab's config without its `[code]` table.
fn config_without_code() -> String {
    CONFIG
        .split("[code]")
        .next()
        .expect("the config before [code]")
        .to_owned()
}

/// `text` appended to the lab's `relative`.
fn append(lab: &Lab, relative: &str, text: &str) {
    let path = lab.root.join(relative);
    let mut bytes = fs::read(&path).expect("read");
    bytes.extend_from_slice(text.as_bytes());
    fs::write(&path, bytes).expect("write");
}

// ----------------------------------------------------------------- TE-1

/// TE-1. M6: inline modules dropped in the reads. After the golden replay
/// the six files' `c` lines tally to `resolve`'s own counts — joined 29,
/// unmatched 0, agrees 26, disagrees 0, `by_state` state for state — each
/// line the oracle's state and shown moniker; the one collision shows
/// twice, `arena impl#[Health] #1` and `#2`, on `arena::impl Health #1`
/// and `#2`. The text and the JSON agree; `layer_c` is the stored stamp;
/// exit 1 keeps `layer_c` `null`, as `code`.
#[test]
fn rows_tally_to_resolve_counts() {
    let lab = Lab::ra_mini("lcr-tally");
    let (want, by_state) = resolve_reply(&lab, &golden_text());
    assert_eq!(
        (
            want["joined"],
            want["unmatched"],
            want["c_only"],
            want["agrees"],
            want["disagrees"],
            want["collisions"]
        ),
        (29, 0, 0, 26, 0, 1),
        "the golden's counts (canon layer-c.md \"Output and exits\")"
    );
    let oracle = expected_c(&a_rows(&lab.db()), &Reply::golden(), &[]);
    let mut states: BTreeMap<String, u64> = BTreeMap::new();
    let (mut joined, mut agrees, mut disagrees) = (0u64, 0u64, 0u64);
    let mut shown_collisions = Vec::new();
    let mut lines = 0;
    for (path, text, json) in file_reads(&lab) {
        let context = format!("symbols --file {path}");
        assert_text_is_json(&text, &json, Some("fresh"), &context);
        for (key, line) in c_lines_by_target(&text.stdout) {
            let line =
                line.unwrap_or_else(|| panic!("{context}: an item without a `c` line: {key:?}"));
            lines += 1;
            assert!(!line.stale, "{context}: fresh: {line:?}");
            let (state, moniker) = oracle
                .get(&key)
                .unwrap_or_else(|| panic!("{context}: no layer A row {key:?}"));
            assert_eq!(
                (&line.state, &line.moniker),
                (state, moniker),
                "{context}: {key:?}"
            );
            *states.entry(line.state.clone()).or_default() += 1;
            if !["unmatched", "unseen"].contains(&line.state.as_str()) {
                joined += 1;
            }
            match line.agrees {
                Some(true) => agrees += 1,
                Some(false) => disagrees += 1,
                None => assert!(
                    line.state != "moniker" || path == "arena/src/stray.rs",
                    "{context}: a joined moniker row of a one-target file has a word: {key:?}"
                ),
            }
            if line.moniker.as_deref().is_some_and(|m| m.contains(" #")) {
                shown_collisions.push((key.1.clone(), line.moniker.clone().expect("a moniker")));
            }
        }
    }
    assert_eq!(lines, 29, "a `c` line per item of the six files");
    assert_eq!(
        (
            joined,
            states.get("unmatched").copied().unwrap_or(0),
            agrees,
            disagrees
        ),
        (
            want["joined"],
            want["unmatched"],
            want["agrees"],
            want["disagrees"]
        ),
        "the `c` lines tally to resolve's counts: {states:?}"
    );
    for (state, count) in by_state.as_object().expect("by_state") {
        assert_eq!(
            states.get(state).copied().unwrap_or(0),
            count.as_u64().expect("a count"),
            "`c {state}` lines against resolve's by_state {by_state}"
        );
    }
    shown_collisions.sort();
    assert_eq!(
        shown_collisions,
        COLLIDING
            .iter()
            .map(|(qpath, shown)| ((*qpath).to_owned(), (*shown).to_owned()))
            .collect::<Vec<_>>(),
        "one collision, shown twice as `<raw> #<v>`"
    );

    let document = lab
        .run(&["--json", "symbols", "--file", "arena/src/lib.rs"])
        .json();
    assert_eq!(
        document["layer_c"],
        json!({
            "state": "fresh", "protocol": 1, "ra_ap": "0.0.352", "host": "masked",
            "toolchains": ["masked"], "degraded": false, "workspaces": 1,
            "files": 6, "items": 29,
        }),
        "`layer_c`: the state and code_resolve's columns"
    );
    let nothing = lab.run(&["--json", "symbols", "--node", "VIT-01"]);
    nothing.code(1);
    let nothing = nothing.json();
    assert_eq!(nothing["code"], Value::Null, "{nothing}");
    assert_eq!(
        nothing.get("layer_c"),
        Some(&Value::Null),
        "exit 1: `layer_c` null, as `code`: {nothing}"
    );
}

// ----------------------------------------------------------------- TE-2

/// TE-2. M1: a `c` line while absent. Never resolved, every form's text
/// (QUERY, `--file` of the six, `--at`, `--node` at exit 1, a PATH outside
/// the walk at exit 2) is HEAD e266128's bytes, stderr and exit too, and
/// each JSON document HEAD's with the four new keys `null`; again after a
/// stored snapshot is dropped by `clear_code` (a `spec index` without
/// `[code]`, the table then restored).
#[test]
fn symbols_text_is_unchanged_without_a_snapshot() {
    let lab = Lab::ra_mini("lcr-absent");
    assert_heads_bytes(&lab, "never resolved");

    lab.store_golden();
    let shown = lab.run(&["symbols", "--file", "arena/src/lib.rs"]);
    assert!(
        shown.stdout.lines().any(|line| line.starts_with(C_PREFIX)),
        "a stored snapshot shows:\n{}",
        shown.stdout
    );
    write(&lab.root, "specengine.toml", config_without_code());
    lab.index();
    assert!(
        !lab.layer_c()
            .lines()
            .any(|line| line.starts_with("code_resolve\t")),
        "clear_code drops the snapshot:\n{}",
        lab.layer_c()
    );
    write(&lab.root, "specengine.toml", CONFIG);
    assert_heads_bytes(&lab, "dropped by clear_code");
    assert_eq!(
        lab.fake.invocations().len(),
        1,
        "only the one resolve started the fake"
    );
}

// ----------------------------------------------------------------- TE-3

/// TE-3. M2: the state unread. With `Cargo.lock` edited after the golden
/// replay, every read's lines are the fresh ones with each `c` line ending
/// `| stale` and the last line `layer c: stale (ra_ap 0.0.352); spec
/// resolve refreshes it`; `layer_c.state` `stale`, its other values kept.
/// The spec's "Data" example, byte for byte; the tier still answers.
#[test]
fn stale_snapshot_marks_every_c_line() {
    let lab = Lab::ra_mini("lcr-stale");
    lab.store_golden();
    let fresh = file_reads(&lab);
    append(&lab, "Cargo.lock", "# edited after the run\n");
    let stale = file_reads(&lab);
    let mut marked = 0;
    for ((path, was, was_json), (_, now, now_json)) in fresh.iter().zip(&stale) {
        let context = format!("symbols --file {path}");
        assert_text_is_json(now, now_json, Some("stale"), &context);
        let want: Vec<String> = was
            .stdout
            .lines()
            .map(|line| {
                if line.starts_with(C_PREFIX) {
                    marked += 1;
                    format!("{line} | stale")
                } else if line == LAYER_C_FRESH {
                    LAYER_C_STALE.to_owned()
                } else {
                    line.to_owned()
                }
            })
            .collect();
        assert_eq!(
            now.stdout.lines().collect::<Vec<_>>(),
            want,
            "{context}: the fresh lines, marked stale"
        );
        let (mut was_c, mut now_c) = (
            was_json.json()["layer_c"].clone(),
            now_json.json()["layer_c"].clone(),
        );
        was_c["state"] = Value::Null;
        now_c["state"] = Value::Null;
        assert_eq!(now_c, was_c, "{context}: the stamp's values kept");
    }
    assert_eq!(marked, 29, "every `c` line");

    let example = lab.run(&["symbols", COLLIDING[1].1]);
    example.code(0);
    assert_eq!(
        example.stdout,
        "arena::impl Health #2 | impl_item | arena/src/lib.rs:23-27 | layer a | mentions 0 | moniker\n  sig impl Health\n  c moniker | arena impl#[Health] #2 | agrees | stale\nsymbols: 1 of 1; code files 6 (.rs 6, .ron 0), skipped 0\nlayer c: stale (ra_ap 0.0.352); spec resolve refreshes it\n",
        "docs/features/layer-c-reads.md \"Data\""
    );
    let found = lab.run(&["symbols", HEALTH]);
    found.code(0);
    let (targets, tail) = shown_targets(&found.stdout);
    assert_eq!(
        targets
            .iter()
            .map(|target| (target.qpath(), target.tier()))
            .collect::<Vec<_>>(),
        [("arena::Health", Some("moniker"))],
        "a stale snapshot answers the tier (R9)\n{}",
        found.stdout
    );
    assert_eq!(tail, [summary(1, 1), LAYER_C_STALE.to_owned()]);
}

/// TE-3. M3: no BLAKE3 check. A byte appended to `arena/src/systems.rs`
/// after the golden replay: its rows show `  c unseen | stale` (JSON
/// `unseen`, `moniker` and `agrees` `null`) and its moniker is found no
/// more, while the other files keep their values, marked stale; a file
/// walked after the run is `unseen` too.
#[test]
fn changed_file_rows_are_unseen() {
    let lab = Lab::ra_mini("lcr-unseen");
    lab.store_golden();
    let before = lab.run(&["symbols", REGENERATE]);
    before.code(0);
    let (targets, _) = shown_targets(&before.stdout);
    assert_eq!(
        targets
            .iter()
            .map(|target| (target.qpath(), target.tier()))
            .collect::<Vec<_>>(),
        [("arena::systems::regenerate", Some("moniker"))],
        "found before the edit\n{}",
        before.stdout
    );

    append(&lab, "arena/src/systems.rs", "\n");
    let oracle = expected_c(
        &a_rows(&lab.db()),
        &Reply::golden(),
        &["arena/src/systems.rs"],
    );
    for (path, text, json) in file_reads(&lab) {
        let context = format!("symbols --file {path}");
        assert_text_is_json(&text, &json, Some("stale"), &context);
        for (key, line) in c_lines_by_target(&text.stdout) {
            let line = line.unwrap_or_else(|| panic!("{context}: no `c` line: {key:?}"));
            assert!(line.stale, "{context}: {line:?}");
            let (state, moniker) = &oracle[&key];
            assert_eq!(
                (&line.state, &line.moniker),
                (state, moniker),
                "{context}: {key:?}"
            );
            if path == "arena/src/systems.rs" {
                assert_eq!(line.render(), "  c unseen | stale", "{context}: {key:?}");
            } else {
                assert_ne!(
                    line.state, "unseen",
                    "{context}: an unchanged file: {key:?}"
                );
            }
        }
        if path == "arena/src/systems.rs" {
            for target in json.json()["symbols"].as_array().expect("symbols") {
                assert_eq!(
                    (&target["c_state"], &target["moniker"], &target["agrees"]),
                    (&json!("unseen"), &Value::Null, &Value::Null),
                    "{context}: {target}"
                );
            }
        }
    }
    let gone = lab.run(&["symbols", REGENERATE]);
    gone.code(0);
    assert_eq!(
        gone.stdout,
        format!("{}\n{LAYER_C_STALE}\n", summary(0, 0)),
        "an unseen file's moniker is not found"
    );
    let kept = lab.run(&["symbols", HEALTH]);
    assert!(
        kept.stdout.starts_with("arena::Health | struct_item | arena/src/lib.rs:12-15 | layer a | mentions 0 | moniker\n"),
        "an unchanged file's moniker is\n{}",
        kept.stdout
    );

    write(&lab.root, "arena/src/extra.rs", "pub fn extra() {}\n");
    let extra = lab.run(&["symbols", "--file", "arena/src/extra.rs"]);
    extra.code(0);
    let (targets, tail) = shown_targets(&extra.stdout);
    assert_eq!(targets.len(), 1, "{}", extra.stdout);
    assert_eq!(
        targets[0].c_line(),
        Some("  c unseen | stale"),
        "a file the run never saw"
    );
    assert_eq!(tail.last().map(String::as_str), Some(LAYER_C_STALE));
}

// ----------------------------------------------------------------- TE-4

/// The golden with hand-written monikers in arena/src/lib.rs, each still a
/// `moniker` row: `arena::Phase` holds `arena::Health` (another item's
/// qpath), `arena::Tick` holds `Health` (a suffix of another), `arena::
/// Health::full` holds `full` (its own suffix and name), `arena::Roster`
/// holds `arena::Roster` (its own qpath); `arena::tests::
/// labels_every_component` and `arena::systems::Regeneration` share
/// `arena shared#` across two files (no collision).
fn ranked_reply() -> Reply {
    let mut reply = Reply::golden();
    for (start, moniker) in [
        (696, "arena::Health"),
        (551, "Health"),
        (471, "full"),
        (810, "arena::Roster"),
        (1240, "arena shared#"),
    ] {
        reply.item_mut("arena/src/lib.rs", start).moniker = Some(moniker.to_owned());
    }
    reply.item_mut("arena/src/systems.rs", 28).moniker = Some("arena shared#".to_owned());
    reply
}

/// `(qpath, tier)` of each target `spec symbols QUERY` lists, and its tail.
fn ranked(lab: &Lab, args: &[&str]) -> (Vec<(String, String)>, Vec<String>) {
    let run = lab.run(args);
    run.code(0);
    let (targets, tail) = shown_targets(&run.stdout);
    (
        targets
            .iter()
            .map(|target| {
                (
                    target.qpath().to_owned(),
                    target.tier().unwrap_or("-").to_owned(),
                )
            })
            .collect(),
        tail,
    )
}

/// A QUERY read and the `(qpath, tier)` it must list.
type Ranked<'a> = (&'a [&'a str], Vec<(String, String)>);

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(qpath, tier)| ((*qpath).to_owned(), (*tier).to_owned()))
        .collect()
}

/// TE-4. M4: the tier after `suffix`. Over [`ranked_reply`], fresh and
/// then stale (`Cargo.lock` edited): `arena::Health` lists the exact item,
/// then the one whose moniker it is; `Health` the moniker's item before
/// the suffix's; an item met by moniker and suffix shows once, at
/// `moniker`, one met by its qpath and moniker once, at `exact`; within the
/// tier qpath order across files; the limit cuts after the merge. JSON
/// `tier` is `moniker`.
#[test]
fn moniker_query_ranks_after_exact() {
    let lab = Lab::ra_mini("lcr-ranks");
    resolve_reply(&lab, &ranked_reply().render());
    let want: [Ranked<'_>; 6] = [
        (
            &["symbols", "arena::Health"],
            pairs(&[("arena::Health", "exact"), ("arena::Phase", "moniker")]),
        ),
        (
            &["symbols", "Health"],
            pairs(&[("arena::Tick", "moniker"), ("arena::Health", "suffix")]),
        ),
        (
            &["symbols", "full"],
            pairs(&[("arena::Health::full", "moniker")]),
        ),
        (
            &["symbols", "arena::Roster"],
            pairs(&[("arena::Roster", "exact")]),
        ),
        (
            &["symbols", "arena shared#"],
            pairs(&[
                ("arena::systems::Regeneration", "moniker"),
                ("arena::tests::labels_every_component", "moniker"),
            ]),
        ),
        (
            &["symbols", "--limit", "1", "Health"],
            pairs(&[("arena::Tick", "moniker")]),
        ),
    ];
    for (state, layer_c) in [("fresh", LAYER_C_FRESH), ("stale", LAYER_C_STALE)] {
        if state == "stale" {
            append(&lab, "Cargo.lock", "# edited after the run\n");
        }
        for (args, listed) in &want {
            let (got, tail) = ranked(&lab, args);
            assert_eq!(&got, listed, "{state}: spec {args:?}");
            assert_eq!(
                tail.last().map(String::as_str),
                Some(layer_c),
                "{state}: {args:?}"
            );
        }
        let (_, tail) = ranked(&lab, &["symbols", "--limit", "1", "Health"]);
        assert_eq!(tail[0], summary(1, 2), "{state}: the limit after the merge");
        let json = lab.run(&["--json", "symbols", "Health"]);
        json.code(0);
        let document = json.json();
        assert_eq!(
            document["symbols"][0]["tier"],
            json!("moniker"),
            "{document}"
        );
        assert_eq!(
            document["symbols"][0]["moniker"],
            json!("Health"),
            "{document}"
        );
        assert_eq!(
            document["symbols"][1]["tier"],
            json!("suffix"),
            "{document}"
        );
        assert_eq!(document["layer_c"]["state"], json!(state));
    }
}

/// TE-4. M5: matching the raw moniker. After the golden replay the shown
/// collision form `arena impl#[Health] #2` finds exactly `arena::impl
/// Health #2` (`#1` its twin), tier `moniker`; the raw `arena
/// impl#[Health]` and an unknown value find nothing (Q3). With the
/// collision values ordinals (two rows of no discriminator sharing a
/// moniker), `<raw> #2` finds the later one only.
#[test]
fn shown_collision_finds_one_item() {
    let lab = Lab::ra_mini("lcr-collision");
    lab.store_golden();
    for (qpath, shown) in COLLIDING {
        let (got, _) = ranked(&lab, &["symbols", shown]);
        assert_eq!(got, pairs(&[(qpath, "moniker")]), "spec symbols {shown:?}");
        let document = lab.run(&["--json", "symbols", shown]).json();
        assert_eq!(document["total"], json!(1), "{document}");
        assert_eq!(
            document["symbols"][0]["moniker"],
            json!(shown),
            "{document}"
        );
        assert_eq!(
            document["symbols"][0]["tier"],
            json!("moniker"),
            "{document}"
        );
    }
    for nothing in [
        COLLIDING_RAW,
        "arena impl#[Health] #3",
        "arena impl#[Health] #",
    ] {
        let run = lab.run(&["symbols", nothing]);
        run.code(0);
        assert_eq!(
            run.stdout,
            format!("{}\n{LAYER_C_FRESH}\n", summary(0, 0)),
            "spec symbols {nothing:?} finds nothing (Q3)"
        );
    }

    let mut reply = Reply::golden();
    for start in [696, 740] {
        reply.item_mut("arena/src/lib.rs", start).moniker = Some("arena twin.".to_owned());
    }
    resolve_reply(&lab, &reply.render());
    for (qpath, shown) in [
        ("arena::Phase", "arena twin. #1"),
        ("arena::MAX_ENTITIES", "arena twin. #2"),
    ] {
        let (got, _) = ranked(&lab, &["symbols", shown]);
        assert_eq!(got, pairs(&[(qpath, "moniker")]), "spec symbols {shown:?}");
    }
    let (got, _) = ranked(&lab, &["symbols", "arena twin."]);
    assert!(got.is_empty(), "the raw form finds nothing: {got:?}");
    let text = lab.run(&["symbols", "--file", "arena/src/lib.rs"]);
    let lines: BTreeMap<String, Option<CLine>> = c_lines_by_target(&text.stdout)
        .into_iter()
        .map(|(key, line)| (key.1, line))
        .collect();
    assert_eq!(
        lines["arena::MAX_ENTITIES"]
            .as_ref()
            .and_then(|line| line.moniker.clone()),
        Some("arena twin. #2".to_owned()),
        "ordinals by start when the rows hold no discriminator"
    );
}

// ----------------------------------------------------------------- TE-5

/// TE-5. Over code-bevy (`[code] frameworks = ["bevy"]`, a module and a
/// RON marker of `CHORE-02` written in) with every row joined: each item
/// target's lines are `  sig`, its `  c` line, its role lines, then its
/// bindings; a module and a RON path show no `c` line, their keys `null`.
#[test]
fn c_line_follows_sig_before_roles() {
    let lab = Lab::fixture("lcr-bevy", "code-bevy", "code-bevy");
    for (path, from, to) in [
        (
            "crates/farm/src/clock.rs",
            "//! The clock: what turns over every tick.\n",
            "//! The clock: what turns over every tick.\n//! @implements CHORE-02\n",
        ),
        (
            "assets/farm.ron",
            "    growth_per_tick: 1,\n",
            "    growth_per_tick: 1, // @configures CHORE-02\n",
        ),
    ] {
        common::replace(&lab.root, path, from, to);
    }
    lab.index();
    let files = lab.requested_files();
    let reply = Reply::from_index(&lab.db(), &files);
    lab.store_reply(&reply.render());
    let mut reads: Vec<Vec<String>> = files
        .iter()
        .map(|path| vec!["symbols".to_owned(), "--file".to_owned(), path.clone()])
        .collect();
    reads.push(vec!["symbols".into(), "--node".into(), "CHORE-02".into()]);
    reads.push(vec!["symbols".into(), "--node".into(), "ALM-GROWTH".into()]);
    let (mut with_roles, mut with_bindings, mut bare) = (0, 0, BTreeSet::new());
    for read in &reads {
        let args: Vec<&str> = read.iter().map(String::as_str).collect();
        let text = lab.run(&args);
        text.code(0);
        let mut json_args = vec!["--json"];
        json_args.extend(&args);
        let json = lab.run(&json_args);
        json.code(0);
        let context = format!("spec {args:?}");
        assert_text_is_json(&text, &json, Some("fresh"), &context);
        let (targets, _) = shown_targets(&text.stdout);
        for target in &targets {
            if matches!(target.kind(), "module" | "ron") {
                assert_eq!(target.c_line(), None, "{context}: {target:?}");
                bare.insert(target.kind().to_owned());
                continue;
            }
            assert!(
                target.under.len() >= 2
                    && target.under[0].starts_with("  sig ")
                    && target.under[1].starts_with(C_PREFIX),
                "{context}: `sig`, then `c`: {target:?}"
            );
            let rest = &target.under[2..];
            let roles = rest
                .iter()
                .take_while(|line| line.starts_with("  role "))
                .count();
            assert!(
                rest[roles..]
                    .iter()
                    .all(|line| !line.starts_with("  role ") && !line.starts_with(C_PREFIX)),
                "{context}: the roles before the bindings, one `c` line: {target:?}"
            );
            if roles > 0 {
                with_roles += 1;
            }
            if rest.len() > roles {
                with_bindings += 1;
            }
        }
    }
    assert!(
        with_roles > 0 && with_bindings > 0,
        "roles {with_roles}, bindings {with_bindings}"
    );
    assert_eq!(
        bare,
        BTreeSet::from(["module".to_owned(), "ron".to_owned()]),
        "a module and a RON target met"
    );
}

// ----------------------------------------------------------------- TE-8

/// The reads TE-8 compares: HEAD's forms and the moniker forms.
fn compared_reads() -> Vec<Vec<String>> {
    let mut reads: Vec<Vec<String>> = head_symbols().into_iter().map(|run| run.args).collect();
    for query in [
        HEALTH,
        REGENERATE,
        COLLIDING[0].1,
        COLLIDING[1].1,
        COLLIDING_RAW,
    ] {
        reads.push(vec!["symbols".into(), query.into()]);
        reads.push(vec!["--json".into(), "symbols".into(), query.into()]);
    }
    reads
}

/// TE-8. M: an order by rowid, a stored absolute path. Two scratch roots,
/// the second created in reverse name order, each with its own `HOME` and
/// the golden replayed: every read, text and JSON, byte for byte the same,
/// and the same again; fresh, stale (`Cargo.lock` edited) and with a file
/// changed (`unseen`).
#[test]
fn symbols_with_a_snapshot_are_byte_identical_across_roots_and_order() {
    let one = Lab::ra_mini("lcr-twice-a");
    let two = Lab::ra_mini_reversed("lcr-twice-b");
    one.store_golden();
    two.store_golden();
    assert_eq!(one.layer_c(), two.layer_c(), "the stored rows");
    for step in ["fresh", "stale", "unseen"] {
        match step {
            "stale" => {
                append(&one, "Cargo.lock", "# edited\n");
                append(&two, "Cargo.lock", "# edited\n");
            }
            "unseen" => {
                append(&one, "arena/src/systems.rs", "\n");
                append(&two, "arena/src/systems.rs", "\n");
            }
            _ => {}
        }
        let mut shown = 0;
        for read in compared_reads() {
            let args: Vec<&str> = read.iter().map(String::as_str).collect();
            let a = one.run(&args);
            let b = two.run(&args);
            let again = one.run(&args);
            let context = format!("{step}: spec {args:?}");
            assert_eq!(
                (a.code, &a.stdout, &a.stderr),
                (b.code, &b.stdout, &b.stderr),
                "{context}: two roots"
            );
            assert_eq!(
                (a.code, &a.stdout, &a.stderr),
                (again.code, &again.stdout, &again.stderr),
                "{context}: again"
            );
            if a.stdout.contains("\nlayer c: ") || a.stdout.contains("\"layer_c\":{") {
                shown += 1;
            }
        }
        assert!(shown > 40, "{step}: the snapshot shows in {shown} reads");
    }
}

// ------------------------------------------------- rules and edge cases

/// "Rules and edge cases", partial. M: a gap shown as another state. A
/// complete reply with gaps: `local` and `none` rows, a row whose end
/// moved (its A row `unmatched`, `c_only` 1), the golden's `unresolved`
/// and `not_loaded`, a degraded workspace among three, two toolchains and
/// a moniker holding a line break (shown through `one_line`). Each `c`
/// line is the oracle's, the tally is resolve's, `layer_c` holds the
/// stamp. Then a run that ends before `done` fails: the reads show the
/// same snapshot, byte for byte.
#[test]
fn gaps_show_their_states() {
    let lab = Lab::ra_mini("lcr-gaps");
    let mut reply = Reply::golden();
    reply.steps = vec![
        Step::at(1, "."),
        Step {
            toolchain: Some("1.98.1".to_owned()),
            ..Step::at(2, "arena-derive")
        },
        Step {
            toolchain: None,
            degraded: true,
            ..Step::at(3, "tools")
        },
    ];
    for (start, state) in [(740, "local"), (776, "none")] {
        let item = reply.item_mut("arena/src/lib.rs", start);
        item.state = state.to_owned();
        item.moniker = None;
    }
    reply.item_mut("arena/src/lib.rs", 810).end += 1;
    reply.item_mut("arena/src/lib.rs", 909).moniker = Some("arena doubled\r\n_regen().".to_owned());
    let (want, by_state) = resolve_reply(&lab, &reply.render());
    assert_eq!(
        (want["joined"], want["unmatched"], want["c_only"]),
        (28, 1, 1),
        "{by_state}"
    );
    let oracle = expected_c(&a_rows(&lab.db()), &reply, &[]);
    let mut states: BTreeMap<String, u64> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let mut texts = Vec::new();
    for (path, text, json) in file_reads(&lab) {
        let context = format!("symbols --file {path}");
        assert_text_is_json(&text, &json, Some("fresh"), &context);
        for (key, line) in c_lines_by_target(&text.stdout) {
            let line = line.unwrap_or_else(|| panic!("{context}: no `c` line: {key:?}"));
            let (state, moniker) = &oracle[&key];
            assert_eq!(
                (&line.state, &line.moniker),
                (state, &moniker.as_deref().map(one_line)),
                "{context}: {key:?}"
            );
            if line.state != "moniker" {
                assert_eq!(line.agrees, None, "{context}: no word: {line:?}");
            }
            seen.insert(line.state.clone());
            *states.entry(line.state.clone()).or_default() += 1;
        }
        let document = json.json();
        assert_eq!(
            document["layer_c"],
            json!({
                "state": "fresh", "protocol": 1, "ra_ap": "0.0.352", "host": "masked",
                "toolchains": ["1.98.1", "masked"], "degraded": true, "workspaces": 3,
                "files": 6, "items": 29,
            }),
            "{context}"
        );
        if path == "arena/src/lib.rs" {
            let doubled = document["symbols"]
                .as_array()
                .expect("symbols")
                .iter()
                .find(|target| target["qpath"] == json!("arena::doubled_regen"))
                .expect("arena::doubled_regen")
                .clone();
            assert_eq!(
                doubled["moniker"],
                json!("arena doubled\r\n_regen()."),
                "JSON keeps the raw text"
            );
            assert!(
                text.stdout
                    .contains("\n  c moniker | arena doubled _regen()."),
                "the text through one_line:\n{}",
                text.stdout
            );
        }
        texts.push(text.stdout);
    }
    assert_eq!(
        seen,
        [
            "local",
            "moniker",
            "none",
            "not_loaded",
            "unmatched",
            "unresolved"
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>(),
        "every gap of a complete snapshot"
    );
    assert_eq!(states["unmatched"], want["unmatched"]);
    let joined: u64 = states
        .iter()
        .filter(|(state, _)| state.as_str() != "unmatched")
        .map(|(_, count)| count)
        .sum();
    assert_eq!(joined, want["joined"]);

    lab.fake.clear();
    lab.fake.script(&[], &golden_head(5));
    let failed = lab.resolve();
    failed.code(2);
    let again: Vec<String> = file_reads(&lab)
        .into_iter()
        .map(|(_, text, _)| text.stdout)
        .collect();
    assert_eq!(
        again, texts,
        "a failed run keeps the snapshot the reads show"
    );
}

/// "Data", `c` line: counted toward `OUTPUT_CAP_CHARS`. M: the `c` lines
/// left out of the cap. A scratch file of 700 functions, every row joined:
/// with the snapshot fewer whole targets fit, the text before the tail
/// stays within the cap, the tail counts the rest, the JSON holds the
/// printed ones.
#[test]
fn c_lines_count_toward_the_output_cap() {
    let lab = Lab::ra_mini("lcr-cap");
    let mut many = String::new();
    for at in 0..700 {
        many.push_str(&format!("pub fn many_{at:04}() {{}}\n"));
    }
    write(&lab.root, "arena/src/many.rs", &many);
    let args = ["symbols", "--file", "arena/src/many.rs"];
    let without = lab.run(&args);
    without.code(0);
    let (shown_without, _) = shown_targets(&without.stdout);
    assert!(
        shown_without.len() < 700,
        "the cap cuts the file: {}",
        shown_without.len()
    );
    lab.index();
    let reply = Reply::from_index(&lab.db(), &lab.requested_files());
    lab.store_reply(&reply.render());
    let with = lab.run(&args);
    with.code(0);
    let (targets, tail) = shown_targets(&with.stdout);
    assert!(
        targets.iter().all(|target| target.c_line().is_some()),
        "every shown item has its `c` line"
    );
    assert!(
        targets.len() < shown_without.len(),
        "fewer fit: {} with, {} without",
        targets.len(),
        shown_without.len()
    );
    let used: usize = targets
        .iter()
        .map(|target| {
            target.line.chars().count()
                + 1
                + target
                    .under
                    .iter()
                    .map(|line| line.chars().count() + 1)
                    .sum::<usize>()
        })
        .sum();
    assert!(
        used <= OUTPUT_CAP_CHARS,
        "{used} characters before the tail"
    );
    let omitted = 700 - targets.len();
    assert_eq!(
        tail[0],
        format!("[truncated: {omitted} symbols not shown]"),
        "{tail:?}"
    );
    let document = lab
        .run(&["--json", "symbols", "--file", "arena/src/many.rs"])
        .json();
    assert_eq!(
        document["symbols"].as_array().map(Vec::len),
        Some(targets.len())
    );
    assert_eq!(document["omitted"]["symbols"], json!(omitted));
    assert!(
        with.stdout.contains(GENERATED),
        "the monikers show:\n{}",
        &with.stdout[..400.min(with.stdout.len())]
    );
}

/// "Rules and edge cases", limits. M: a read writing the snapshot or the
/// tree, or spawning. With the golden stored, fresh and then stale, every
/// form of HEAD's capture and the moniker forms exit as HEAD's did; the
/// snapshot's rows, the worktree's bytes and the fake's one start stay.
#[test]
fn reads_leave_the_snapshot_and_the_worktree_untouched() {
    let lab = Lab::ra_mini("lcr-untouched");
    let stored = lab.store_golden();
    let head: BTreeMap<Vec<String>, i32> = head_symbols()
        .into_iter()
        .map(|run| (run.args, run.exit))
        .collect();
    for step in ["fresh", "stale"] {
        if step == "stale" {
            append(&lab, "Cargo.lock", "# edited\n");
        }
        let tree = snapshot(&lab.root);
        for read in compared_reads() {
            let args: Vec<&str> = read.iter().map(String::as_str).collect();
            let run = lab.run(&args);
            if let Some(exit) = head.get(&read) {
                assert_eq!(
                    run.code,
                    *exit,
                    "{step}: spec {args:?} keeps HEAD's exit\n{}",
                    run.show()
                );
            } else {
                run.code(0);
            }
        }
        assert_eq!(snapshot(&lab.root), tree, "{step}: the worktree untouched");
        assert_eq!(lab.layer_c(), stored, "{step}: the snapshot untouched");
        assert_eq!(
            lab.fake.invocations().len(),
            1,
            "{step}: no read starts specengine-ra"
        );
    }
}

/// docs/features/layer-c-reads.md "Tier moniker": the limit after the merge,
/// over the narrowed read (iteration 2: the picked files are the found
/// ones and the first `limit` hits'). M: a shown target's file left out of
/// the pick (its `c` line `unseen`), or the cut before the merge. A scratch
/// file adds three `tick` items and `arena::spawn_seed` holds the moniker
/// `tick`: `spec symbols tick` lists the moniker's item first, then the
/// suffix hits across three files, every `c` line the oracle's; with each
/// `--limit` k from 1 to the total the text is the whole answer's first k
/// targets line for line, the summary `k of <total>`, the JSON the whole
/// one's first k targets, `omitted.symbols` 0.
#[test]
fn a_limited_query_keeps_its_targets_and_c_lines() {
    let lab = Lab::ra_mini("lcr-limit");
    write(
        &lab.root,
        "arena/src/more.rs",
        "pub mod a {\n    pub fn tick() {}\n}\n\npub mod b {\n    pub fn tick() {}\n}\n\npub fn tick() {}\n",
    );
    lab.index();
    let mut reply = Reply::from_index(&lab.db(), &lab.requested_files());
    let seed = a_rows(&lab.db())
        .into_iter()
        .find(|row| row.qpath == "arena::spawn_seed")
        .expect("arena::spawn_seed");
    reply.item_mut("arena/src/lib.rs", seed.start_byte).moniker = Some("tick".to_owned());
    resolve_reply(&lab, &reply.render());
    let oracle = expected_c(&a_rows(&lab.db()), &reply, &[]);

    let whole = lab.run(&["symbols", "--limit", "200", "tick"]);
    whole.code(0);
    let (all, _) = shown_targets(&whole.stdout);
    let total = all.len();
    assert!(
        total >= 5,
        "the moniker's item and the suffix hits:\n{}",
        whole.stdout
    );
    assert_eq!(
        (all[0].qpath(), all[0].tier()),
        ("arena::spawn_seed", Some("moniker")),
        "{}",
        whole.stdout
    );
    let files: BTreeSet<String> = all
        .iter()
        .filter_map(|target| target.span().map(|(path, _, _)| path))
        .collect();
    assert!(files.len() >= 3, "hits across files: {files:?}");
    for (key, line) in c_lines_by_target(&whole.stdout) {
        let line = line.unwrap_or_else(|| panic!("no `c` line: {key:?}"));
        let (state, moniker) = &oracle[&key];
        assert_eq!((&line.state, &line.moniker), (state, moniker), "{key:?}");
    }
    let whole_json = lab
        .run(&["--json", "symbols", "--limit", "200", "tick"])
        .json();
    for k in 1..=total {
        let limit = k.to_string();
        let run = lab.run(&["symbols", "--limit", &limit, "tick"]);
        run.code(0);
        let (shown, tail) = shown_targets(&run.stdout);
        assert_eq!(
            shown,
            all[..k].to_vec(),
            "--limit {k}: the whole answer's first {k}"
        );
        assert_eq!(
            tail,
            [
                format!("symbols: {k} of {total}; code files 7 (.rs 7, .ron 0), skipped 0"),
                LAYER_C_FRESH.to_owned()
            ],
            "--limit {k}"
        );
        let json = lab
            .run(&["--json", "symbols", "--limit", &limit, "tick"])
            .json();
        assert_eq!(
            json["symbols"].as_array().map(|targets| targets.to_vec()),
            whole_json["symbols"]
                .as_array()
                .map(|targets| targets[..k].to_vec()),
            "--limit {k}: JSON"
        );
        assert_eq!(
            (&json["total"], &json["omitted"]["symbols"]),
            (&json!(total), &json!(0))
        );
    }
}

/// The stored states the reads must refuse to show, one at a time, and the
/// warning each gives (`escape_controls`: a control as `\u{XX}`).
const UNKNOWN_STATES: [(&str, &str); 2] = [
    ("'bogus'", "bogus"),
    ("'bo' || char(27) || 'gus'", "bo\\u{1b}gus"),
];

/// docs/features/layer-c-reads.md (iteration 2): a stored state this build
/// does not know degrades the read. M: exit 2, a `c` line of the word, a
/// second warning. With the golden stored and one `code_monikers` row of
/// arena/src/lib.rs edited in the scratch data directory, `spec symbols
/// --file arena/src/lib.rs` and `spec symbols Health`, text and JSON: exit
/// 0, stdout HEAD's (no `c` line, no `layer c:` line; `layer_c` and the
/// items' keys `null`), stderr the one line `warning: the layer C snapshot
/// holds an unknown state `<x>`; run `spec resolve``, a control escaped;
/// `--file arena/src/systems.rs` (no unknown row picked) still shows the
/// snapshot, fresh, with no warning.
#[test]
fn an_unknown_stored_state_degrades_the_read() {
    let lab = Lab::ra_mini("lcr-unknown");
    lab.store_golden();
    let head = head_symbols();
    for (sql, shown) in UNKNOWN_STATES {
        code_index_common::execute(
            &lab.db(),
            &format!(
                "UPDATE code_monikers SET state = {sql} WHERE path = 'arena/src/lib.rs' AND start = 278"
            ),
        );
        let warning = format!(
            "warning: the layer C snapshot holds an unknown state `{shown}`; run `spec resolve`\n"
        );
        for captured in head.iter().filter(|captured| {
            let args: Vec<&str> = captured.args.iter().map(String::as_str).collect();
            matches!(
                args.as_slice(),
                ["symbols", "--file", "arena/src/lib.rs"]
                    | ["--json", "symbols", "--file", "arena/src/lib.rs"]
                    | ["symbols", "Health"]
                    | ["--json", "symbols", "Health"]
            )
        }) {
            let args: Vec<&str> = captured.args.iter().map(String::as_str).collect();
            let run = lab.run(&args);
            let context = format!("{shown}: spec {args:?}");
            assert_eq!(run.code, 0, "{context}: the exit kept\n{}", run.show());
            assert_eq!(run.stderr, warning, "{context}: one warning line");
            if args[0] == "--json" {
                assert_eq!(
                    run.json(),
                    head_json_with_null_keys(&captured.stdout),
                    "{context}: `layer_c` and the keys null"
                );
            } else {
                assert_eq!(run.stdout, captured.stdout, "{context}: HEAD's bytes");
            }
        }
        // A read whose picked file holds no unknown row shows the snapshot.
        let untouched = lab.run(&["symbols", "--file", "arena/src/systems.rs"]);
        let untouched_json = lab.run(&["--json", "symbols", "--file", "arena/src/systems.rs"]);
        for run in [&untouched, &untouched_json] {
            run.code(0);
            assert_eq!(run.stderr, "", "{shown}: no warning\n{}", run.show());
        }
        assert_text_is_json(
            &untouched,
            &untouched_json,
            Some("fresh"),
            &format!("{shown}: symbols --file arena/src/systems.rs"),
        );
        let lines = c_lines_by_target(&untouched.stdout);
        assert_eq!(lines.len(), 4, "{}", untouched.stdout);
        assert!(
            lines
                .iter()
                .all(|(_, line)| line.as_ref().is_some_and(|line| line.state == "moniker")),
            "{shown}: systems.rs keeps its `c` lines\n{}",
            untouched.stdout
        );
    }
}

// ----------------------------------------------------------------- TE-9

/// The median of five warm runs of `args` (one run first, untimed).
fn median_of_five(lab: &Lab, args: &[&str]) -> Duration {
    lab.run(args).code(0);
    let mut times: Vec<Duration> = (0..5)
        .map(|_| {
            let at = Instant::now();
            lab.run(args).code(0);
            at.elapsed()
        })
        .collect();
    times.sort();
    times[2]
}

/// TE-9. Release, warm, the median of five, over the lab's ra-mini: `spec
/// symbols --file arena/src/lib.rs` and `spec symbols Health` without and
/// with the golden's snapshot; the snapshot adds at most 10 ms to each.
/// The figures print to stderr.
#[test]
#[ignore = "TE-9, the release figure: cargo nextest run --release -p specengine-cli --test layer_c_reads --run-ignored only layer_c_reads_cost --no-capture"]
fn layer_c_reads_cost() {
    if cfg!(debug_assertions) {
        panic!("TE-9 measures the release build (`--release`)");
    }
    let lab = Lab::ra_mini("lcr-cost");
    lab.index();
    let forms: [&[&str]; 2] = [
        &["symbols", "--file", "arena/src/lib.rs"],
        &["symbols", "Health"],
    ];
    let without: Vec<Duration> = forms
        .iter()
        .map(|args| median_of_five(&lab, args))
        .collect();
    lab.store_golden();
    let with: Vec<Duration> = forms
        .iter()
        .map(|args| median_of_five(&lab, args))
        .collect();
    let mut over = Vec::new();
    for ((args, was), now) in forms.iter().zip(&without).zip(&with) {
        let added = now.saturating_sub(*was);
        eprintln!(
            "TE-9 spec {}: without {:.1} ms, with {:.1} ms, added {:.1} ms",
            args.join(" "),
            was.as_secs_f64() * 1000.0,
            now.as_secs_f64() * 1000.0,
            added.as_secs_f64() * 1000.0
        );
        if added > Duration::from_millis(10) {
            over.push(args.join(" "));
        }
    }
    assert!(over.is_empty(), "a snapshot adds over 10 ms: {over:?}");
}
