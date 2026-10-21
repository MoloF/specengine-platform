//! docs/features/spec-verify.md TE-6, TE-7, TE-9, RD-7 and RD-8: the
//! output is byte-identical from two roots, another cwd and a copy made in
//! reverse creation order; the JSON document has every key ("Output"), no
//! timing; `spec verify` writes nothing under the root (the worktree,
//! `.git`, `spec.lock` and its time stamp) and no queue row or event; and
//! `spec check` is unchanged by drift, this repository's hook untouched.
//! docs/features/lock-levels-verify.md TE-7: the determinism gains a
//! levels scenario (a narrowed marker over a body edit, a `levels_changed`
//! row) over the lock's entries reversed; every row's `levels` object
//! keyed and ordered as **JSON** gives it. docs/features/anchor-states.md
//! TE-3: the determinism gains a moved and an ambiguous scenario over
//! `fixtures/anchor-states` (two roots, the lock's entries reversed, a CRLF
//! checkout under `core.autocrlf=true`); every row's `anchor` keyed as
//! **JSON** gives it, `delta.bindings.moved` `[]` with no move.
//! docs/features/show-bindings.md RD-6 ("delta.tests_reaching"):
//! `tests_reaching_lists_bound_tests` over the modes; `verify_json_every_key_present`
//! with `tests_reaching` a list of six-field keys.
//! docs/features/tested-evidence.md (8b-1) RD-4 ("Plain verify"): the
//! `tested` key and lines over a format-3 lock never change an exit, a
//! verdict, a count, a class, a hint or a row; null in the base modes and
//! under a run cause; an evidence-free run prints 5b's bytes
//! (`verify_tested_before.json`, recorded with the binary before 8b-1).

mod anchor_common;
mod common;
mod lock_common;
mod tested_common;
mod verify_common;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use common::{data_dir, fixture, repository_root, snapshot};
use lock_common::LockRepo;
use serde_json::{Value, json};
use specengine_store::{ProposalFilter, ProposalQueue as _, SqliteQueue};
use verify_common::{
    COUNTER, COUNTER_MD, LEVELS_KEYS, ROW_KEYS, STAMINA, TOP_KEYS, Verify, clone_dir, ecs, golden,
    golden_file, new_task, set_mode, verify, verify_at,
};

/// Drift of every kind over the golden, and a note: `CAP` and the RON cap
/// edited, LAW-BOUND's text edited, a marker removed, a non-UTF-8 file
/// (a note), `enforce-introduced` (a note when plain).
fn drift(repo: &LockRepo) {
    repo.replace(COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
    repo.replace("data/limits.ron", "cap: 10,", "cap: 12,");
    repo.replace(
        COUNTER_MD,
        "The count never exceeds the cap.",
        "The count never exceeds the cap, ever.",
    );
    repo.replace(
        "crates/tally/tests/bounds.rs",
        "//! @verifies PIECE-COUNTER\n",
        "",
    );
    repo.write("crates/tally/src/odd.rs", b"// \xff\n".as_slice());
    set_mode(&repo.root, "enforce-introduced");
}

/// docs/features/lock-levels-verify.md TE-7's levels scenario over the
/// golden: `PIECE-COUNTER`'s `Tally` marker narrowed from `[sig, body]` to
/// `[sig]` with its body edited (`code_ahead code_changed body`, the union
/// with the lock's levels compared), and `GOAL-01`'s `Tally::value`
/// marker widened from `[body]` to `[body, deps]`, its code unchanged
/// (`code_ahead levels_changed body->body,deps`).
fn levels_drift(repo: &LockRepo) {
    repo.replace(
        COUNTER,
        "// @implements PIECE-COUNTER@3 [sig, body]",
        "// @implements PIECE-COUNTER@3 [sig]",
    );
    repo.replace(COUNTER, "    count: u32,\n}", "    count: u64,\n}");
    repo.replace(
        COUNTER,
        "// @implements AIM-01@1 [body] mutation",
        "// @implements AIM-01@1 [body, deps] mutation",
    );
}

/// The golden's lock written over `repo` (at its first commit) and
/// committed as `the lock`.
fn lock_it(repo: &LockRepo) {
    repo.write("spec.lock", fs::read(golden_file()).expect("golden"));
    repo.commit_all("the lock");
}

/// Every printed byte of a run: text stdout and stderr, JSON stdout and
/// stderr, the exit.
fn bytes(run: &Verify) -> (i32, String, String, String, String) {
    (
        run.code(),
        run.text.stdout.clone(),
        run.text.stderr.clone(),
        run.json_run.stdout.clone(),
        run.json_run.stderr.clone(),
    )
}

/// The rank of a relation in key order.
fn relation_rank(relation: &str) -> usize {
    ["implements", "verifies", "configures", "assumes"]
        .iter()
        .position(|known| *known == relation)
        .unwrap_or_else(|| panic!("no relation {relation}"))
}

/// The rows' keys sort as "Command and rows" says: `EntryKey`'s order,
/// null first.
fn assert_key_order(run: &Verify) {
    type SortKey = (
        Vec<u8>,
        usize,
        Vec<u8>,
        Option<Vec<u8>>,
        Option<Vec<u8>>,
        Option<Vec<u8>>,
    );
    let key = |row: &Value| -> SortKey {
        let bytes = |field: &str| row[field].as_str().map(|text| text.as_bytes().to_vec());
        (
            bytes("node").unwrap_or_default(),
            relation_rank(row["relation"].as_str().unwrap_or_default()),
            bytes("file").unwrap_or_default(),
            bytes("kind"),
            bytes("symbol"),
            bytes("item_kind"),
        )
    };
    let keys: Vec<SortKey> = run.rows().into_iter().map(key).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    // Keyless rows of one place order by state and category: not part of
    // the six fields; only keyed rows must be strictly ordered.
    assert_eq!(keys, sorted, "rows in key order");
}

/// TE-6 / RD-7: two roots (`repo`, `elsewhere/deeper`), another cwd (a
/// subdirectory) and a copy made in reverse creation order give the same
/// bytes on stdout and stderr, text and JSON, plain and `--changed`; a
/// repeat run too; rows in key order. docs/features/lock-levels-verify.md
/// TE-7: with [`levels_drift`] applied, and over a copy whose committed
/// `spec.lock` holds the same entries reversed, byte-identical too. M:
/// walk order (an order not the key's, a hash order); the lock's entry
/// order read into the union or the `levels` object.
#[test]
fn verify_is_byte_identical_across_roots_and_order() {
    let first = golden("verify-det-a");
    drift(&first);
    levels_drift(&first);
    let second = LockRepo::of("verify-det-b", "code-a", "elsewhere/deeper", &[]);
    lock_it(&second);
    drift(&second);
    levels_drift(&second);
    assert_eq!(second.head(), first.head(), "the same commits");

    let third = LockRepo::uncommitted("verify-det-c");
    fs::remove_dir_all(&third.root).expect("clear");
    let mut files: Vec<_> = snapshot(&fixture("code-a"))
        .into_iter()
        .filter(|(path, bytes)| bytes.is_some() && path != "expected.json")
        .collect();
    files.reverse();
    for (path, bytes) in files {
        third.write(&path, bytes.expect("a file"));
    }
    third.git.init(&third.root);
    third.commit_all("the fixture");
    lock_it(&third);
    drift(&third);
    levels_drift(&third);
    assert_eq!(third.head(), first.head(), "the same commits");

    // The lock's entries reversed, committed alone over a copy of `first`.
    let reversed = clone_dir(&first, "reversed-lock");
    let lock = fs::read_to_string(reversed.join("spec.lock")).expect("the lock");
    let backwards = lock_common::rebase::reversed(&lock);
    assert_ne!(backwards, lock, "the premise: the order differs");
    fs::write(reversed.join("spec.lock"), &backwards).expect("the reversed lock");
    first.git.git(&reversed, &["add", "spec.lock"]);
    first.git.commit(&reversed, "the lock reversed");

    for extra in [&[][..], &["--changed"][..]] {
        let reference = verify(&first, extra);
        assert!(
            !reference.text.stderr.is_empty(),
            "the scenario prints notes"
        );
        assert_key_order(&reference);
        // The levels scenario's premise: a narrowed marker's body edit and
        // a widened marker, as "Level-aware verify" step 4 gives them.
        let tally = reference.row(
            "PIECE-COUNTER",
            "implements",
            COUNTER,
            Some("crates/tally::counter::Tally"),
        );
        assert_eq!(
            (&tally["state"], &tally["category"], &tally["detail"]),
            (
                &Value::from("code_ahead"),
                &Value::from("code_changed"),
                &Value::from("body")
            ),
            "{}",
            reference.text.show()
        );
        assert_eq!(
            tally["levels"],
            serde_json::json!({"lock": ["sig", "body"], "markers": ["sig"], "changed": ["body"]})
        );
        let value = reference.row(
            "GOAL-01",
            "implements",
            COUNTER,
            Some("crates/tally::counter::Tally::value"),
        );
        assert_eq!(
            (&value["state"], &value["category"], &value["detail"]),
            (
                &Value::from("code_ahead"),
                &Value::from("levels_changed"),
                &Value::from("body->body,deps")
            ),
            "{}",
            reference.text.show()
        );
        assert_eq!(
            value["levels"],
            serde_json::json!({"lock": ["body"], "markers": ["body", "deps"], "changed": []})
        );
        let runs = [
            ("a repeat run", verify(&first, extra)),
            ("another root", verify(&second, extra)),
            (
                "another cwd",
                verify_at(&second, &second.root.join("crates").join("tally"), extra),
            ),
            ("reverse creation order", verify(&third, extra)),
            (
                "the lock's entries reversed",
                verify_at(&first, &reversed, extra),
            ),
        ];
        for (context, run) in runs {
            assert_eq!(bytes(&run), bytes(&reference), "{context} {extra:?}");
        }
        let root_text = first.root.display().to_string();
        assert!(
            !reference.text.stdout.contains(&root_text)
                && !reference.text.stderr.contains(&root_text)
                && !reference.json_run.stdout.contains(&root_text),
            "no absolute path printed"
        );
    }
    anchors_are_byte_identical();
}

/// TE-3's scenarios: a pure rename (`ok`, moved), equal-quote twins
/// renamed (two ambiguous Fix rows), a rename with an edit (`code_ahead`,
/// moved, its `moved from` line).
const ANCHOR_SCENARIOS: [&str; 3] = [
    "rename within the file",
    "twins, equal quotes, both renamed",
    "rename plus a local renamed (skeleton step)",
];

/// The files [`ANCHOR_SCENARIOS`] change; nothing else is locked in them.
const ANCHOR_FILES: [&str; 3] = [
    anchor_common::RENAME,
    anchor_common::CLONES,
    anchor_common::SKETCH,
];

/// `fixtures/anchor-states`' base at `<scratch>/<dir>`, [`ANCHOR_SCENARIOS`]
/// applied and committed (the lock not accepted again).
fn anchor_scenario(label: &str, dir: &str) -> LockRepo {
    let repo = anchor_common::base_with(label, dir, |_| {});
    for name in ANCHOR_SCENARIOS {
        anchor_common::change(&repo, name);
    }
    repo.commit_all("moved and ambiguous");
    repo
}

/// docs/features/anchor-states.md TE-3: over [`anchor_scenario`], a repeat
/// run, another root (`elsewhere/deeper`), another cwd, the lock's entries
/// reversed and a CRLF checkout of the changed files under
/// `core.autocrlf=true` give the same bytes, text and JSON, plain and
/// `--changed`. M: a follow decided by walk or entry order, a fingerprint
/// read with its CR.
fn anchors_are_byte_identical() {
    let first = anchor_scenario("verify-det-anchor-a", "repo");
    let second = anchor_scenario("verify-det-anchor-b", "elsewhere/deeper");
    assert_eq!(second.head(), first.head(), "the same commits");

    let reversed = clone_dir(&first, "reversed-anchor-lock");
    let lock = fs::read_to_string(reversed.join("spec.lock")).expect("the lock");
    let backwards = lock_common::rebase::reversed(&lock);
    assert_ne!(backwards, lock, "the premise: the order differs");
    fs::write(reversed.join("spec.lock"), &backwards).expect("the reversed lock");
    first.git.git(&reversed, &["add", "spec.lock"]);
    first.git.commit(&reversed, "the lock reversed");

    let crlf = clone_dir(&first, "crlf-anchors");
    first.git.git(&crlf, &["config", "core.autocrlf", "true"]);
    for path in ANCHOR_FILES {
        fs::remove_file(crlf.join(path)).expect("remove");
        first.git.git(&crlf, &["checkout", "--", path]);
        assert!(
            fs::read(crlf.join(path))
                .expect("checked out")
                .windows(2)
                .any(|pair| pair == b"\r\n"),
            "CRLF in the worktree: {path}"
        );
    }
    assert!(
        first
            .git
            .git(
                &crlf,
                &["status", "--porcelain=v1", "-z", "--untracked-files=all"]
            )
            .is_empty(),
        "clean under autocrlf"
    );

    for extra in [&[][..], &["--changed"][..]] {
        let reference = verify(&first, extra);
        assert_key_order(&reference);
        let anchors: Vec<&Value> = reference
            .rows()
            .into_iter()
            .map(|row| &row["anchor"]["state"])
            .collect();
        assert_eq!(
            anchors.iter().filter(|state| **state == "moved").count(),
            2,
            "the premise: two moved rows {extra:?}\n{}",
            reference.text.show()
        );
        assert_eq!(
            anchors
                .iter()
                .filter(|state| **state == "ambiguous")
                .count(),
            2,
            "the premise: two ambiguous rows {extra:?}\n{}",
            reference.text.show()
        );
        assert!(
            reference
                .lines()
                .iter()
                .any(|line| line.starts_with("  moved from "))
                && reference
                    .lines()
                    .iter()
                    .any(|line| line.starts_with("  candidate ")),
            "{}",
            reference.text.show()
        );
        // Every anchor object keyed `state`, `from`, `candidates`; every
        // place `file`, `kind`, `symbol`, `item_kind`; in that order.
        let raw = &reference.json_run.stdout;
        let objects = reference
            .rows()
            .into_iter()
            .filter(|row| row["anchor"].is_object())
            .count();
        let opened: Vec<usize> = raw
            .match_indices("\"anchor\":{")
            .map(|(at, text)| at + text.len() - 1)
            .collect();
        assert_eq!(opened.len(), objects, "{raw}");
        for open in opened {
            assert_eq!(raw_order(raw, open), ["state", "from", "candidates"]);
        }
        for (at, text) in raw.match_indices("\"from\":{\"file\"") {
            let open = at + "\"from\":".len();
            assert!(text.ends_with("file\""));
            assert_eq!(
                raw_order(raw, open),
                ["file", "kind", "symbol", "item_kind"]
            );
        }
        let runs = [
            ("a repeat run", verify(&first, extra)),
            ("another root", verify(&second, extra)),
            (
                "another cwd",
                verify_at(&second, &second.root.join("crates").join("sky"), extra),
            ),
            (
                "the lock's entries reversed",
                verify_at(&first, &reversed, extra),
            ),
            ("a CRLF checkout", verify_at(&first, &crlf, extra)),
        ];
        for (context, run) in runs {
            assert_eq!(
                bytes(&run),
                bytes(&reference),
                "anchors: {context} {extra:?}"
            );
        }
    }
}

/// Every key of an object, recursively, as `path.key`.
fn keys_of(value: &Value, path: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                let here = format!("{path}.{key}");
                out.push(here.clone());
                keys_of(inner, &here, out);
            }
        }
        Value::Array(items) => {
            for inner in items {
                keys_of(inner, &format!("{path}[]"), out);
            }
        }
        _ => {}
    }
}

/// RD-7: the JSON document holds no timing: no `ms`, no time, duration or
/// elapsed key anywhere; repeat runs are byte-identical. M: an `ms` key.
#[test]
fn verify_json_has_no_timing() {
    let repo = golden("verify-no-ms");
    drift(&repo);
    for extra in [&[][..], &["--changed"][..]] {
        let run = verify(&repo, extra);
        let mut keys = Vec::new();
        keys_of(&run.json, "", &mut keys);
        let timing: Vec<&String> = keys
            .iter()
            .filter(|key| {
                let last = key.rsplit('.').next().unwrap_or_default();
                last == "ms"
                    || last.ends_with("_ms")
                    || ["time", "elapsed", "duration", "took", "secs"]
                        .iter()
                        .any(|word| last.contains(word))
            })
            .collect();
        assert!(timing.is_empty(), "{extra:?}: {timing:?}");
        assert_eq!(bytes(&verify(&repo, extra)), bytes(&run), "{extra:?}");
    }
}

/// The keys of the JSON object opening at byte `open` of `raw`, in the
/// order printed (depth 1 of that object only).
fn raw_order(raw: &str, open: usize) -> Vec<String> {
    let bytes = raw.as_bytes();
    assert_eq!(bytes[open], b'{', "an object opens at {open}");
    let (mut depth, mut in_string, mut escaped, mut start) = (0usize, false, false, 0);
    let mut keys = Vec::new();
    for (at, &byte) in bytes.iter().enumerate().skip(open) {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
                if depth == 1 && bytes.get(at + 1) == Some(&b':') {
                    keys.push(raw[start..at].to_owned());
                }
            }
            continue;
        }
        match byte {
            b'"' => {
                in_string = true;
                start = at + 1;
            }
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
    }
    keys
}

/// The byte where the value of `"key":` opens (its first occurrence).
fn value_at(raw: &str, key: &str) -> usize {
    let needle = format!("\"{key}\":");
    raw.find(&needle).unwrap_or_else(|| panic!("{key} printed")) + needle.len()
}

/// RD-7: every key of "Output" present, absent values `null`, in order:
/// the top level, `counts`, `cascade`, every row (a lock-only, a keyless,
/// an unlocked, a locked row), `lock`, `markers`, `delta` and its
/// `bindings`; uncapped. M: a key dropped.
#[test]
fn verify_json_every_key_present() {
    let repo = golden("verify-keys");
    drift(&repo);
    let run = verify(&repo, &["--changed"]);
    let json = &run.json;
    let raw = &run.json_run.stdout;
    assert_eq!(raw_order(raw, 0), TOP_KEYS);
    // docs/canon/spec-verify-output.md "JSON": `node`, `symbol`, `omitted`
    // `null` in every mode but `--node` (verify-node), plain too.
    for key in ["task", "node", "symbol", "omitted", "tested"] {
        assert_eq!(json[key], Value::Null, "`{key}` under --changed");
    }
    let whole = verify(&repo, &[]);
    assert_eq!(raw_order(&whole.json_run.stdout, 0), TOP_KEYS);
    for key in ["task", "node", "symbol", "omitted"] {
        assert_eq!(whole.json[key], Value::Null, "plain's `{key}`");
    }
    // docs/features/tested-evidence.md "Plain verify": no block, `[]`.
    assert_eq!(whole.json["tested"], json!([]), "plain's `tested`");
    let object_keys = |value: &Value| -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().expect("object").keys().cloned().collect();
        keys.sort();
        keys
    };
    let sorted = |keys: &[&str]| -> Vec<String> {
        let mut keys: Vec<String> = keys.iter().map(|key| (*key).to_owned()).collect();
        keys.sort();
        keys
    };
    assert_eq!(
        object_keys(&json["counts"]),
        sorted(&[
            "rows",
            "ok",
            "fix",
            "check",
            "cannot",
            "pre_existing",
            "skipped"
        ])
    );
    assert_eq!(
        raw_order(raw, value_at(raw, "counts")),
        [
            "rows",
            "ok",
            "fix",
            "check",
            "cannot",
            "pre_existing",
            "skipped"
        ]
    );
    assert_eq!(
        object_keys(&json["cascade"]),
        sorted(&["by_blob", "by_ast"])
    );
    assert_eq!(
        object_keys(&json["delta"]),
        sorted(&["symbols", "bindings", "affected_nodes", "tests_reaching"])
    );
    // docs/features/impact-since.md "verify --changed": under `--changed`
    // the comparer's `{added, removed, changed, renamed}` (`renamed`
    // `null`); `null` in plain verify.
    assert_eq!(
        object_keys(&json["delta"]["symbols"]),
        sorted(&["added", "removed", "changed", "renamed"])
    );
    assert_eq!(json["delta"]["symbols"]["renamed"], Value::Null);
    for list in ["added", "removed", "changed"] {
        for item in json["delta"]["symbols"][list].as_array().expect("a list") {
            assert_eq!(
                object_keys(item),
                sorted(&["file", "symbol", "item_kind", "line"]),
                "{list}"
            );
        }
    }
    assert_eq!(whole.json["delta"]["symbols"], Value::Null, "plain's");
    // docs/features/show-bindings.md "delta.tests_reaching": a list of
    // six-field keys under `--changed` (and plain), never `null`.
    for document in [json, &whole.json] {
        let reaching = document["delta"]["tests_reaching"]
            .as_array()
            .expect("tests_reaching, a list");
        assert!(
            !reaching.is_empty(),
            "premise: a test reaches a drifted node"
        );
        for key in reaching {
            assert_eq!(
                object_keys(key),
                sorted(&["node", "relation", "file", "kind", "symbol", "item_kind"]),
                "a six-field key: {key}"
            );
            assert_eq!(key["relation"], "verifies", "{key}");
        }
    }
    assert_eq!(
        object_keys(&json["delta"]["bindings"]),
        sorted(&["moved", "ambiguous", "lost", "stale"])
    );
    // docs/features/anchor-states.md "Output": `[]` when nothing moved,
    // else `{"from", "to"}` pairs of six key fields each.
    assert_eq!(json["delta"]["bindings"]["moved"], serde_json::json!([]));
    for list in ["ambiguous", "lost", "stale"] {
        for key in json["delta"]["bindings"][list].as_array().expect("a list") {
            assert_eq!(
                object_keys(key),
                sorted(&["node", "relation", "file", "kind", "symbol", "item_kind"]),
                "{list}"
            );
        }
    }
    assert_eq!(
        json["counts"]["rows"].as_u64(),
        Some(run.rows().len() as u64),
        "uncapped"
    );
    let mut seen = BTreeMap::new();
    for row in run.rows() {
        assert_eq!(object_keys(row), sorted(&ROW_KEYS), "{row}");
        let kind = match (
            row["lock"].is_null(),
            row["kind"].is_null(),
            row["markers"].as_array().map(Vec::len),
        ) {
            (false, _, Some(0)) => "lock-only",
            (false, ..) => "locked",
            (true, true, _) => "keyless",
            (true, false, _) => "unlocked",
        };
        seen.insert(kind, row.clone());
        if !row["lock"].is_null() {
            assert_eq!(
                object_keys(&row["lock"]),
                sorted(&["rev", "commit", "verified_at"])
            );
        }
        for marker in row["markers"].as_array().expect("markers") {
            assert_eq!(object_keys(marker), sorted(&["line", "rev"]));
        }
        // docs/features/lock-levels-verify.md **JSON**: `levels` null
        // without an entry or a bound marker; else its three keys, the
        // level lists in canonical order, `changed` a list exactly when
        // the digests were compared (`ast`).
        let levels = &row["levels"];
        if row["lock"].is_null() || row["markers"].as_array().is_some_and(Vec::is_empty) {
            assert!(levels.is_null(), "{row}");
        }
        if !levels.is_null() {
            assert_eq!(object_keys(levels), sorted(&LEVELS_KEYS), "{row}");
            for list in ["lock", "markers"] {
                let names: Vec<&str> = levels[list]
                    .as_array()
                    .unwrap_or_else(|| panic!("`levels.{list}` is a list: {row}"))
                    .iter()
                    .map(|name| name.as_str().expect("a level name"))
                    .collect();
                let rank = |name: &&str| {
                    ["path", "sig", "body", "deps"]
                        .iter()
                        .position(|known| known == name)
                        .unwrap_or_else(|| panic!("no level {name}: {row}"))
                };
                assert!(
                    !names.is_empty()
                        && names.windows(2).all(|pair| rank(&pair[0]) < rank(&pair[1])),
                    "`levels.{list}` in canonical order: {row}"
                );
            }
            assert_eq!(
                levels["changed"].is_array(),
                row["compared"] == "ast",
                "`changed` a list iff compared by ast: {row}"
            );
        }
    }
    assert_eq!(
        seen.keys().copied().collect::<Vec<_>>(),
        ["keyless", "lock-only", "locked", "unlocked"],
        "every shape of row is present"
    );
    // The first row's keys in "Output"'s order.
    let rows_at = raw.find("\"rows\":[{").expect("rows") + "\"rows\":[".len();
    assert_eq!(raw_order(raw, rows_at), ROW_KEYS);
    // The first `levels` object's keys in **JSON**'s order.
    let levels_at = raw.find("\"levels\":{").expect("a levels object") + "\"levels\":".len();
    assert_eq!(raw_order(raw, levels_at), LEVELS_KEYS);
    assert_eq!(
        raw_order(raw, value_at(raw, "delta")),
        ["symbols", "bindings", "affected_nodes", "tests_reaching"]
    );
    assert_eq!(
        raw_order(raw, value_at(raw, "bindings")),
        ["moved", "ambiguous", "lost", "stale"]
    );
    assert!(json["notes"].is_array() && json["cannot"].is_array());
}

/// A row's six key fields, as `delta`'s keys hold them.
fn six(row: &Value) -> Value {
    json!({
        "node": row["node"],
        "relation": row["relation"],
        "file": row["file"],
        "kind": row["kind"],
        "symbol": row["symbol"],
        "item_kind": row["item_kind"],
    })
}

/// "delta.tests_reaching" over `rows` (the run's, before any filter):
/// the distinct keys, in row order, of each `verifies` row whose node is
/// in `affected`, or names a node in `links` (a node, the node its
/// resolved `verifies` link targets) whose target is in `affected`.
fn reaching(rows: &[Value], affected: &Value, links: &[(&str, &str)]) -> Vec<Value> {
    let affected: Vec<&str> = affected
        .as_array()
        .expect("affected_nodes")
        .iter()
        .map(|node| node.as_str().expect("a node"))
        .collect();
    let mut keys: Vec<Value> = Vec::new();
    for row in rows.iter().filter(|row| row["relation"] == "verifies") {
        let node = row["node"].as_str().unwrap_or_default();
        let linked = links
            .iter()
            .any(|(from, to)| *from == node && affected.contains(to));
        let key = six(row);
        if (affected.contains(&node) || linked) && !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
}

/// The golden's records of RD-6: `GOAL-02` holds a resolved `verifies`
/// link to LAW-BOUND (a criterion section cannot hold one: a section has no
/// front-matter, `spec-check-links.md` Q-I, so the linked node is a
/// document); `GOAL-04` holds none. Each is bound by one test.
const REACH_TESTS: &str = "crates/tally/tests/reach.rs";

fn reach_records(repo: &LockRepo) {
    repo.write(
        "docs/records/GOAL-02.md",
        "---\nclass: canon\nid: GOAL-02\nowner: owner\nreviewed: 2026-10-10\nlinks:\n  verifies: [LAW-BOUND]\n---\n\n# The cap is proven\n\nA test steps past the cap and reads it.\n",
    );
    repo.write(
        "docs/records/GOAL-04.md",
        "---\nclass: canon\nid: GOAL-04\nowner: owner\nreviewed: 2026-10-10\n---\n\n# The count is readable\n\nA caller reads the count at any time.\n",
    );
    repo.write(
        REACH_TESTS,
        "// @verifies GOAL-02\n#[test]\nfn reaches_through_the_link() {}\n\n// @verifies GOAL-04\n#[test]\nfn reaches_nothing_affected() {}\n",
    );
    repo.commit_all("two records and their tests");
    repo.accept_ok("GOAL-02");
    repo.accept_ok("GOAL-04");
}

/// docs/features/show-bindings.md RD-6 over the golden with
/// [`reach_records`] (both records' rows `ok`, so neither is affected),
/// LAW-BOUND drifted (its text edited, uncommitted): plain, `--changed`
/// and `--staged` list, in row order, the keys of the `verifies` rows whose
/// node is affected or links (`verifies`) into an affected node: LAW-BOUND's
/// own test and GOAL-02's, never GOAL-04's; `--task` on LAW-BOUND lists
/// GOAL-02's test though its row is outside the task's set (computed
/// before the filter); `--node` gives `null`; a run cause (a bad mode) and
/// a clean state (the ECS game) give `[]`. M6: `--task` computed after its
/// filter. Variants: `null` in plain; an unaffected node's test listed.
#[test]
fn tests_reaching_lists_bound_tests() {
    let repo = golden("verify-reaching");
    reach_records(&repo);
    let links = repo.spec_piped(&repo.root, &["show", "GOAL-02", "--links"]);
    assert!(
        links
            .stdout
            .lines()
            .any(|line| line.starts_with("  out verifies LAW-BOUND ")),
        "premise: GOAL-02's resolved verifies link\n{}",
        links.show()
    );
    let link = [("GOAL-02", "LAW-BOUND")];
    let key_of_test = |run: &Verify, node: &str| -> Value {
        six(run
            .rows()
            .into_iter()
            .find(|row| row["node"] == node && row["relation"] == "verifies")
            .unwrap_or_else(|| panic!("premise: {node}'s test row")))
    };
    let accepted = verify(&repo, &[]);
    for node in ["GOAL-02", "GOAL-04"] {
        assert!(
            accepted
                .rows()
                .iter()
                .filter(|row| row["node"] == node)
                .all(|row| row["state"] == "ok"),
            "premise: {node}'s rows ok"
        );
    }
    let linked = key_of_test(&accepted, "GOAL-02");
    let unlinked = key_of_test(&accepted, "GOAL-04");

    repo.replace(
        COUNTER_MD,
        "The count never exceeds the cap.",
        "The count never exceeds the cap, ever.",
    );
    let check =
        |run: &Verify, context: &str| {
            let document = &run.json;
            let reached = document["delta"]["tests_reaching"]
                .as_array()
                .unwrap_or_else(|| panic!("{context}: tests_reaching, a list: {document}"))
                .clone();
            let rows: Vec<Value> = run.rows().into_iter().cloned().collect();
            assert_eq!(
                reached,
                reaching(&rows, &document["delta"]["affected_nodes"], &link),
                "{context}: the reaching keys in row order"
            );
            assert!(
                reached.contains(&linked),
                "{context}: GOAL-02's test, by its link"
            );
            assert!(!reached.contains(&unlinked), "{context}: never GOAL-04's");
            assert!(
                reached.iter().any(|key| key["node"] == "LAW-BOUND"
                    && key["file"] == "crates/tally/tests/bounds.rs"),
                "{context}: LAW-BOUND's own test"
            );
        };
    let plain = verify(&repo, &[]);
    check(&plain, "plain");
    check(&verify(&repo, &["--changed"]), "--changed");
    repo.git(&["add", COUNTER_MD]);
    check(&verify(&repo, &["--staged"]), "--staged");

    let node = verify(&repo, &["--node", "LAW-BOUND"]);
    assert_eq!(
        node.json["delta"]["tests_reaching"],
        Value::Null,
        "--node: null"
    );

    let config = repo.read("specengine.toml");
    set_mode(&repo.root, "strict");
    let cause = verify(&repo, &[]);
    assert_eq!(cause.json["rows"], json!([]), "premise: a run cause");
    assert_eq!(
        cause.json["delta"]["tests_reaching"],
        json!([]),
        "a run cause: []"
    );
    repo.write("specengine.toml", config);

    let task = new_task(&repo, &["LAW-BOUND"]);
    // The task's event is in the queue now: run it as the binary alone
    // (`verify()` pins a queue without events).
    let scoped = repo
        .spec_piped(&repo.root, &["verify", "--task", &task, "--json"])
        .json();
    assert!(
        scoped["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .all(|row| row["node"] != "GOAL-02"),
        "premise: GOAL-02's row is outside the task's set"
    );
    let reached = scoped["delta"]["tests_reaching"]
        .as_array()
        .expect("--task: a list")
        .clone();
    let all: Vec<Value> = plain.rows().into_iter().cloned().collect();
    assert_eq!(
        reached,
        reaching(&all, &scoped["delta"]["affected_nodes"], &link),
        "--task: over the run's rows before its filter"
    );
    assert!(
        reached.contains(&linked),
        "--task: GOAL-02's test, outside the set"
    );

    let clean = ecs("verify-reaching-clean");
    clean.write(
        "crates/game/tests/regen.rs",
        "// @verifies PERK-REGEN\n#[test]\nfn regen_restores() {}\n",
    );
    clean.commit_all("a test of the regen");
    clean.accept_ok("PERK-REGEN");
    let run = verify(&clean, &[]);
    assert_eq!(
        run.json["delta"]["affected_nodes"],
        json!([]),
        "premise: clean"
    );
    assert!(
        run.rows().iter().any(|row| row["relation"] == "verifies"),
        "premise: a test row, not reaching"
    );
    assert_eq!(
        run.json["delta"]["tests_reaching"],
        json!([]),
        "nothing affected: []"
    );
}

/// The bytes of every file under `root`, `.git` included.
fn everything(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    snapshot(root)
}

/// A scenario's edit of a fresh golden repository.
type Edit = Box<dyn Fn(&LockRepo)>;

/// RD-8 / TE-7: `spec verify`, plain and `--changed`, text and JSON, over
/// a drifted tree, an unborn-free merge state and a base: every byte under
/// the root (the worktree, `.git` with its index and objects) unchanged,
/// then `git status --porcelain=v1 -z --untracked-files=all`, `HEAD` and
/// the refs as before. M: a file under the root.
#[test]
fn verify_leaves_the_worktree_untouched() {
    let scenarios: Vec<(&str, Edit)> = vec![
        ("golden", Box::new(|_| {})),
        ("drift", Box::new(drift)),
        (
            "a broken item",
            Box::new(|repo: &LockRepo| {
                repo.replace(
                    COUNTER,
                    "        self.count\n    }\n\n    // @implements GOAL-01@\n",
                    "        self.count +\n    }\n\n    // @implements GOAL-01@\n",
                );
            }),
        ),
        (
            "an invalid lock",
            Box::new(|repo: &LockRepo| repo.write("spec.lock", "format = 9\n")),
        ),
    ];
    for (n, (context, edit)) in scenarios.iter().enumerate() {
        let repo = golden(&format!("verify-untouched-{n}"));
        edit(&repo);
        let state = repo.state();
        let before = everything(&repo.root);
        for extra in [&[][..], &["--changed"][..]] {
            verify(&repo, extra);
        }
        let after = everything(&repo.root);
        let changed: Vec<&String> = before
            .keys()
            .chain(after.keys())
            .filter(|path| before.get(*path) != after.get(*path))
            .collect();
        assert!(
            changed.is_empty(),
            "{context}: written under the root: {changed:?}"
        );
        assert_eq!(repo.state(), state, "{context}");
    }
}

/// RD-8 / TE-7: `spec.lock` is never written: its bytes, its modification
/// time and its mode stay, over a tree whose drift a rewrite would
/// "refresh". M: the lock rewritten.
#[test]
fn verify_never_writes_the_lock() {
    let repo = golden("verify-lock-untouched");
    drift(&repo);
    let path = repo.root.join("spec.lock");
    let past = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    fs::File::options()
        .write(true)
        .open(&path)
        .expect("the lock")
        .set_modified(past)
        .expect("set mtime");
    let bytes = fs::read(&path).expect("the lock");
    let meta = fs::metadata(&path).expect("meta");
    for extra in [&[][..], &["--changed"][..]] {
        let run = verify(&repo, extra);
        assert!(!run.rows().is_empty());
    }
    let after = fs::metadata(&path).expect("meta");
    assert_eq!(fs::read(&path).expect("the lock"), bytes);
    assert_eq!(after.modified().expect("mtime"), past);
    assert_eq!(after.len(), meta.len());
    assert_eq!(
        std::os::unix::fs::MetadataExt::ino(&after),
        std::os::unix::fs::MetadataExt::ino(&meta),
        "the same file, not a replacement"
    );
    assert_eq!(
        repo.git_text(&["diff", "--name-only", "HEAD", "--", "spec.lock"]),
        ""
    );
}

/// RD-8 / TE-7: verify writes the data directory's index only: the queue
/// holds no proposal and no event after runs over drift of every kind,
/// plain and `--changed`. M: an event.
#[test]
fn verify_writes_no_queue_row() {
    for (label, repo) in [
        ("code-a", golden("verify-queue-golden")),
        ("verify-ecs", ecs("verify-queue-ecs")),
    ] {
        if label == "code-a" {
            drift(&repo);
        } else {
            repo.replace(STAMINA, "// @implements PERK-DRAIN\n", "");
        }
        for extra in [&[][..], &["--changed"][..]] {
            verify(&repo, extra);
        }
        let db = data_dir(&repo.home).join(format!("{label}.db"));
        assert!(
            db.exists(),
            "{label}: the index refresh made the data directory"
        );
        let queue = SqliteQueue::open(&db, label).expect("the queue opens");
        assert!(
            queue
                .list(&ProposalFilter::default())
                .expect("list")
                .is_empty(),
            "{label}: a proposal"
        );
        assert!(
            queue.events().expect("events").is_empty(),
            "{label}: an event"
        );
    }
}

/// TE-9: `spec check` prints the same bytes and exits alike with drift of
/// every kind present (code and lock drift beside an unchanged corpus) as
/// without it; verify adds nothing to it.
#[test]
fn spec_check_is_unchanged_by_drift() {
    let repo = golden("verify-check-same");
    let check = |repo: &LockRepo| {
        let text = repo.spec_piped(&repo.root, &["check"]);
        let json = repo.spec_piped(&repo.root, &["check", "--json"]);
        (text.code, text.stdout, text.stderr, json.code, json.stdout)
    };
    let before = check(&repo);
    repo.replace(COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
    repo.replace("data/limits.ron", "cap: 10,", "cap: 12,");
    repo.replace(
        "crates/tally/tests/bounds.rs",
        "//! @verifies PIECE-COUNTER\n",
        "",
    );
    let lock = repo.read("spec.lock");
    repo.write(
        "spec.lock",
        lock.replace(
            "verified_at = \"2026-10-08\"",
            "verified_at = \"2026-10-01\"",
        ),
    );
    let config = repo.read("specengine.toml");
    repo.write(
        "specengine.toml",
        format!("{config}\n[verify]\nmode = \"enforce\"\n"),
    );
    let drifted = verify(&repo, &[]);
    assert_eq!(drifted.code(), 2, "drift present");
    assert_eq!(check(&repo), before);
}

/// TE-9: this repository's pre-commit hook runs `spec check` only, never
/// `spec verify` (Q7: a project opts its own hook in).
#[test]
fn repository_hook_runs_check_only() {
    let hook = fs::read_to_string(repository_root().join(".githooks").join("pre-commit"))
        .expect(".githooks/pre-commit");
    assert!(hook.contains("-- check --staged --root ."), "{hook}");
    let commands: Vec<&str> = hook
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect();
    assert!(
        commands.iter().all(|line| !line.contains("verify")),
        "{commands:?}"
    );
}

// ------------------------------------------- tested-evidence (8b-1) RD-4

use tested_common::ecs_with_blocks;

/// `repo` with `spec.lock` replaced by `text`, committed as `message`
/// (nothing committed when the bytes and the config are as they were).
fn relock(repo: &LockRepo, text: &str, message: &str) {
    repo.write("spec.lock", text);
    if !repo.porcelain().is_empty() {
        repo.commit_all(message);
    }
}

/// The text of `with` is the text of `without` with `with`'s `tested`
/// lines just before the summary, nothing else changed; the JSON documents
/// equal but for `tested`; the exits alike.
fn same_but_tested(with: &Verify, without: &Verify, context: &str) {
    assert_eq!(with.code(), without.code(), "{context}: exit");
    assert_eq!(
        tested_common::without_tested_key(&with.json),
        tested_common::without_tested_key(&without.json),
        "{context}: every key but `tested`"
    );
    assert_eq!(
        tested_common::without_tested_lines(&with.text.stdout),
        without.text.stdout,
        "{context}: the text but the `tested` lines"
    );
    let lines = with.lines();
    let tested = tested_common::tested_lines(with);
    let at = lines.len() - 1 - tested.len();
    assert_eq!(
        lines[at..lines.len() - 1].to_vec(),
        tested.iter().map(String::as_str).collect::<Vec<_>>(),
        "{context}: the `tested` lines last before the summary:\n{}",
        with.text.stdout
    );
    assert_eq!(with.text.stderr, without.text.stderr, "{context}: stderr");
}

/// RD-4 **M4** (a block changing exit, verdict, counts, class or hints):
/// over two corpora of different nature (the ECS game, clean; `code-a`'s
/// library with every binding problem, blocked or observed) and in every
/// mode (`observe`, `enforce`), plain verify and `--node` over a lock with
/// blocks in every state print what they print over the same entries with
/// no block, but for `tested`: the exit, the verdict, the counts, every
/// row and class, the cascade, the hints and notes; the text but the
/// `tested | …` lines.
#[test]
fn tested_never_changes_verify_exit() {
    let (repo, text, commit) = ecs_with_blocks("vo-tested-exit-ecs");
    let with_blocks = repo.lock_text();
    let golden_repo = golden("vo-tested-exit-golden");
    let golden_text = golden_repo.lock_text();
    let golden_blocks: Vec<tested_common::Block> = ["GOAL-01", "LAW-BOUND", "PIECE-COUNTER"]
        .iter()
        .map(|node| tested_common::vouching_block(&golden_text, node, &commit))
        .chain([tested_common::made_up_block("GONE-01", 1, '9', &commit)])
        .collect();
    let golden_with = tested_common::with_blocks_added(&golden_text, &golden_blocks);
    for (corpus, repo, without, with, node) in [
        (
            "ecs",
            &repo,
            text.as_str(),
            with_blocks.as_str(),
            "PERK-STAMINA",
        ),
        (
            "code-a",
            &golden_repo,
            golden_text.as_str(),
            golden_with.as_str(),
            "LAW-BOUND",
        ),
    ] {
        for mode in [None, Some("enforce")] {
            if let Some(mode) = mode {
                set_mode(&repo.root, mode);
            }
            for extra in [&[][..], &["--node", node][..]] {
                relock(repo, without, "no block");
                let plain = verify(repo, extra);
                relock(repo, with, "blocks");
                let tested = verify(repo, extra);
                let context = format!("{corpus} {mode:?} {extra:?}");
                assert!(
                    !tested_common::tested(&tested).is_empty(),
                    "{context}: blocks listed"
                );
                assert_eq!(plain.json["tested"], json!([]), "{context}");
                same_but_tested(&tested, &plain, &context);
            }
        }
    }
}

/// RD-4 ("Plain verify", **JSON**, text): `tested` is the 11th of 14 keys,
/// between `delta` and `cannot`; plain lists every block by `node` bytes,
/// each `{node, state, category, rev, node_rev, commit, verified_at}` in
/// that order, `rev`, `commit`, `verified_at` the block's, `node_rev` the
/// node's revision now (`null` gone); the text a `tested | <state>
/// [<category>] | <node>` line per entry not `verified`, by node bytes,
/// after the hints and before the summary. M: a `verified` entry printed,
/// another order, a key missing.
#[test]
fn plain_verify_lists_every_block() {
    let (repo, _, commit) = ecs_with_blocks("vo-tested-plain");
    let run = verify(&repo, &[]);
    assert_eq!(raw_order(&run.json_run.stdout, 0), TOP_KEYS);
    let entry = |node: &str, state: &str, category: Value, node_rev: Value| {
        json!({
            "node": node,
            "state": state,
            "category": category,
            "rev": if node == "GONE-01" { json!(1) } else { node_rev.clone() },
            "node_rev": node_rev,
            "commit": commit,
            "verified_at": tested_common::DAY,
        })
    };
    let rev = |node: &str| -> Value {
        let row = run
            .rows()
            .into_iter()
            .find(|row| row["node"] == node)
            .unwrap_or_else(|| panic!("a row of {node}"))
            .clone();
        row["node_rev"].clone()
    };
    assert_eq!(
        run.json["tested"],
        json!([
            entry("GONE-01", "node_gone", Value::Null, Value::Null),
            entry(
                "PERK-DRAIN",
                "stale",
                json!("code_changed"),
                rev("PERK-DRAIN")
            ),
            entry(
                "PERK-REGEN",
                "stale",
                json!("spec_changed"),
                rev("PERK-REGEN")
            ),
            entry("PERK-STAMINA", "verified", Value::Null, rev("PERK-STAMINA")),
        ])
    );
    let tested_at = value_at(&run.json_run.stdout, "tested");
    assert_eq!(&run.json_run.stdout[tested_at..=tested_at], "[");
    assert_eq!(
        raw_order(&run.json_run.stdout, tested_at + 1),
        tested_common::TESTED_KEYS,
        "an entry's keys in order"
    );
    assert_eq!(
        tested_common::tested_lines(&run),
        [
            "tested | node_gone | GONE-01",
            "tested | stale code_changed | PERK-DRAIN",
            "tested | stale spec_changed | PERK-REGEN",
        ]
    );
    let lines = run.lines();
    assert_eq!(
        lines[lines.len() - 4..lines.len() - 1],
        [
            "tested | node_gone | GONE-01",
            "tested | stale code_changed | PERK-DRAIN",
            "tested | stale spec_changed | PERK-REGEN",
        ],
        "before the summary"
    );
}

/// RD-4 (Q-B; M: a non-null value under `--changed` or `--staged`, or
/// under a run cause): `tested` is `null` and no `tested |` line prints
/// under `--changed`, under `--staged` and under any run cause (here a
/// config error, cause 1), the lock holding blocks; nothing is hashed for
/// them; plain over the same lock lists them.
#[test]
fn base_modes_tested_is_null() {
    let (repo, _, _) = ecs_with_blocks("vo-tested-null");
    assert_eq!(tested_common::tested(&verify(&repo, &[])).len(), 4);
    repo.replace(STAMINA, "2.5", "3.5");
    for extra in [&["--changed"][..], &["--staged"][..]] {
        if extra == ["--staged"] {
            repo.git(&["add", "-A"]);
        }
        let run = verify(&repo, extra);
        assert_eq!(run.json["tested"], Value::Null, "{extra:?}");
        assert!(tested_common::tested_lines(&run).is_empty(), "{extra:?}");
    }
    set_mode(&repo.root, "bogus");
    for extra in [&[][..], &["--node", "PERK-STAMINA"][..]] {
        let run = verify(&repo, extra);
        assert_eq!(run.code(), 2, "a run cause: {}", run.text.show());
        assert!(!run.json["cannot"].as_array().expect("cannot").is_empty());
        assert_eq!(run.json["tested"], Value::Null, "a run cause {extra:?}");
        assert!(tested_common::tested_lines(&run).is_empty());
    }
}

/// The record of the bytes before 8b-1, beside this test.
fn tested_before_file() -> std::path::PathBuf {
    repository_root().join("crates/specengine-cli/tests/verify_tested_before.json")
}

/// The evidence-free cases: `(case, repository, args)`, each repository a
/// lock without a block: `code-a`'s golden plain, `--node`, `--changed`
/// over the drift scenario; the ECS corpus plain and `--node`.
fn evidence_free_cases() -> Vec<(String, LockRepo, Vec<&'static str>)> {
    let mut out = Vec::new();
    for (case, args) in [
        ("code-a plain", vec!["verify"]),
        ("code-a --node", vec!["verify", "--node", "PIECE-COUNTER"]),
        ("code-a --changed", vec!["verify", "--changed"]),
    ] {
        let repo = golden(&format!("vo-before-{}", out.len()));
        if case.ends_with("--changed") {
            drift(&repo);
        }
        out.push((case.to_owned(), repo, args));
    }
    for (case, args) in [
        ("ecs plain", vec!["verify"]),
        ("ecs --node", vec!["verify", "--node", "PERK-STAMINA"]),
    ] {
        out.push((
            case.to_owned(),
            ecs(&format!("vo-before-{}", out.len())),
            args,
        ));
    }
    out
}

/// The b3 of each case's text and JSON stdout and its exit, by
/// `program`; `strip` the `tested` key off the JSON first.
fn before_record(program: &Path, strip: bool) -> Vec<Value> {
    let mut out = Vec::new();
    for (case, repo, args) in evidence_free_cases() {
        let (code, text) = tested_common::spec_program(program, &repo, &repo.root, &args);
        let mut json_args = args.clone();
        json_args.push("--json");
        let (json_code, json) = tested_common::spec_program(program, &repo, &repo.root, &json_args);
        assert_eq!(code, json_code, "{case}");
        let json = if strip {
            tested_common::json_without_tested(&json)
        } else {
            json
        };
        out.push(json!({
            "case": case,
            "exit": code,
            "text": specengine_store::b3_hash(text.as_bytes()),
            "json": specengine_store::b3_hash(json.as_bytes()),
        }));
    }
    out
}

/// Records [`tested_before_file`] with `SPECENGINE_BEFORE_SPEC`, the `spec`
/// binary of 3c9fe6f (7b-2 shipped, 8b-1 not started); never re-recorded
/// from a later binary.
#[test]
#[ignore = "records the bytes before 8b-1: SPECENGINE_BEFORE_SPEC names that binary"]
fn record_the_bytes_before_tested_evidence() {
    let program = std::path::PathBuf::from(
        std::env::var_os("SPECENGINE_BEFORE_SPEC").expect("SPECENGINE_BEFORE_SPEC: the binary"),
    );
    let cases = before_record(&program, false);
    assert_eq!(cases, before_record(&program, false), "stable answers");
    let lines: Vec<String> = cases.iter().map(Value::to_string).collect();
    fs::write(
        tested_before_file(),
        format!(
            "{{\"recorded_with\": \"spec at 3c9fe6f (7b-2 shipped, 8b-1 not started)\",\n\"cases\": [\n{}\n]}}\n",
            lines.join(",\n")
        ),
    )
    .expect("the record");
}

/// RD-4 **evidence_free_verify_text_unchanged** ("Rules": WHEN no block
/// exists, verify's stdout SHALL equal 5b's; M: an evidence-free run's
/// text changed): over locks without a block, plain verify, `--node` and
/// `--changed` print the bytes the binary before 8b-1 printed (its
/// record), the exit alike; the JSON gains only `"tested":[]` (`null`
/// under `--changed`).
#[test]
fn evidence_free_verify_text_unchanged() {
    let record: Value = serde_json::from_str(
        &fs::read_to_string(tested_before_file()).expect("verify_tested_before.json"),
    )
    .expect("the record parses");
    let program = Path::new(common::SPEC);
    assert_eq!(
        Value::Array(before_record(program, true)),
        record["cases"],
        "the bytes before 8b-1"
    );
}

/// `spec verify <args>` (text, then `--json`) in `repo` with a `git`
/// first on `PATH` that refuses `hash-object` (exit 128) and runs the
/// sandbox's git for everything else.
fn verify_with_failing_hash(repo: &LockRepo, args: &[&str]) -> (i32, String, String, Value) {
    let bin = repo.scratch.dir("failing-git");
    let real = repo.git.git_program().display().to_string();
    let wrapper = bin.join("git");
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  if [ \"$arg\" = hash-object ]; then\n    echo 'fatal: hash-object refused by the test' >&2\n    exit 128\n  fi\ndone\nexec '{real}' \"$@\"\n"
        ),
    )
    .expect("the wrapper");
    let mut permissions = fs::metadata(&wrapper).expect("the wrapper").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    fs::set_permissions(&wrapper, permissions).expect("executable");
    let run = |extra: Option<&str>| {
        let mut command = std::process::Command::new(common::SPEC);
        command
            .env_clear()
            .current_dir(&repo.root)
            .arg("verify")
            .args(args)
            .args(extra)
            .stdin(std::process::Stdio::null());
        for (name, value) in repo.git.vars() {
            if name == "PATH" {
                let mut path = std::ffi::OsString::from(bin.as_os_str());
                path.push(":");
                path.push(&value);
                command.env(name, path);
            } else if name != "HOME" {
                command.env(name, value);
            }
        }
        command.env("HOME", &repo.home);
        command.output().expect("spec verify")
    };
    let text = run(None);
    let json = run(Some("--json"));
    assert_eq!(
        text.status.code(),
        json.status.code(),
        "text and JSON exit alike"
    );
    let document: Value = serde_json::from_slice(&json.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&json.stdout)));
    (
        text.status.code().unwrap_or(-1),
        String::from_utf8(text.stdout).expect("UTF-8"),
        String::from_utf8(text.stderr).expect("UTF-8"),
        document,
    )
}

/// 8b-1 iteration 2 (owner-approved): a failure while hashing a tested
/// node's bound code (git refusing `hash-object`; here over a lock of
/// blocks and no entry, so only the evidence's read hashes) never changes
/// verify's exit, verdict, counts or rows, plain or `--node`: `tested` is
/// `null` with one note `tested: <cause>` (on stderr too), no `tested |`
/// line; the same corpus without the blocks gives the rest alike. M: the
/// failure a run cause (exit 2), or a block judged without its hash.
#[test]
fn tested_hash_failure_never_changes_verify_exit() {
    let repo = LockRepo::of("vo-tested-hash-failure", "verify-tests", "repo", &[]);
    let text = tested_common::lock_with(
        &[],
        &[tested_common::made_up_block(
            "KILN-RAMP",
            1,
            '7',
            &repo.head(),
        )],
    );
    repo.write("spec.lock", &text);
    repo.commit_all("evidence alone");
    let plain = LockRepo::of("vo-tested-hash-failure-none", "verify-tests", "repo", &[]);
    for args in [&[][..], &["--node", "KILN-RAMP"][..]] {
        let (with_code, with_text, with_stderr, with) = verify_with_failing_hash(&repo, args);
        let (code, without_text, _, without) = verify_with_failing_hash(&plain, args);
        assert_ne!(
            code, 2,
            "{args:?}: premise: verify hashes nothing without blocks:\n{without_text}"
        );
        assert_eq!(with_code, code, "{args:?}: the exit\n{with_text}\n{with}");
        assert_eq!(with["tested"], Value::Null, "{args:?}: {with}");
        let notes: Vec<&str> = with["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .filter_map(Value::as_str)
            .filter(|note| note.starts_with("tested: "))
            .collect();
        assert_eq!(notes.len(), 1, "{args:?}: one note: {with}");
        assert_eq!(
            with_stderr
                .lines()
                .filter(|line| *line == format!("note: {}", notes[0]))
                .count(),
            1,
            "{args:?}: the note on stderr:\n{with_stderr}"
        );
        assert!(!with_text.contains("\ntested | "), "{args:?}: {with_text}");
        for key in ["verdict", "counts", "rows", "cannot"] {
            assert_eq!(with[key], without[key], "{args:?}: {key}");
        }
    }
}
