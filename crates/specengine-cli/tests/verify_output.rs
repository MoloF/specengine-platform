//! docs/features/spec-verify.md TE-6, TE-7, TE-9, RD-7 and RD-8: the
//! output is byte-identical from two roots, another cwd and a copy made in
//! reverse creation order; the JSON document has every key ("Output"), no
//! timing; `spec verify` writes nothing under the root (the worktree,
//! `.git`, `spec.lock` and its time stamp) and no queue row or event; and
//! `spec check` is unchanged by drift, this repository's hook untouched.

mod common;
mod lock_common;
mod verify_common;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use common::{data_dir, fixture, repository_root, snapshot};
use lock_common::LockRepo;
use serde_json::Value;
use specengine_store::{ProposalFilter, ProposalQueue as _, SqliteQueue};
use verify_common::{
    COUNTER, COUNTER_MD, ROW_KEYS, STAMINA, TOP_KEYS, Verify, ecs, golden, golden_file, set_mode,
    verify, verify_at,
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
/// repeat run too; rows in key order. M: walk order (an order not the
/// key's, a hash order).
#[test]
fn verify_is_byte_identical_across_roots_and_order() {
    let first = golden("verify-det-a");
    drift(&first);
    let second = LockRepo::of("verify-det-b", "code-a", "elsewhere/deeper", &[]);
    lock_it(&second);
    drift(&second);
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
    assert_eq!(third.head(), first.head(), "the same commits");

    for extra in [&[][..], &["--changed"][..]] {
        let reference = verify(&first, extra);
        assert!(
            !reference.text.stderr.is_empty(),
            "the scenario prints notes"
        );
        assert_key_order(&reference);
        let runs = [
            ("a repeat run", verify(&first, extra)),
            ("another root", verify(&second, extra)),
            (
                "another cwd",
                verify_at(&second, &second.root.join("crates").join("tally"), extra),
            ),
            ("reverse creation order", verify(&third, extra)),
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
    assert_eq!(json["delta"]["symbols"], Value::Null);
    assert_eq!(json["delta"]["tests_reaching"], Value::Null);
    assert_eq!(
        object_keys(&json["delta"]["bindings"]),
        sorted(&["moved", "ambiguous", "lost", "stale"])
    );
    assert_eq!(json["delta"]["bindings"]["moved"], Value::Null);
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
    }
    assert_eq!(
        seen.keys().copied().collect::<Vec<_>>(),
        ["keyless", "lock-only", "locked", "unlocked"],
        "every shape of row is present"
    );
    // The first row's keys in "Output"'s order.
    let rows_at = raw.find("\"rows\":[{").expect("rows") + "\"rows\":[".len();
    assert_eq!(raw_order(raw, rows_at), ROW_KEYS);
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
