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

mod anchor_common;
mod common;
mod lock_common;
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
    for key in ["task", "node", "symbol", "omitted"] {
        assert_eq!(json[key], Value::Null, "`{key}` under --changed");
    }
    let whole = verify(&repo, &[]);
    assert_eq!(raw_order(&whole.json_run.stdout, 0), TOP_KEYS);
    for key in ["task", "node", "symbol", "omitted"] {
        assert_eq!(whole.json[key], Value::Null, "plain's `{key}`");
    }
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
