//! docs/features/impact-since.md (7b-2) "verify --changed": `spec verify
//! --changed --json`, `--task` or not, fills `delta.symbols` with the
//! comparer's `{added, removed, changed, renamed}` against `HEAD`'s code
//! side, all code, bound or not, uncapped, `renamed` `null`; `null` in
//! plain, `--staged`, `--node` and under a run cause; the text, verdict,
//! counts and exit never move with it. RD-5 (M6 and the uncounted ones),
//! over scratch git copies of `fixtures/impact/` with the edits written
//! here; `impact_since.rs` holds RD-6 (`impact --since HEAD`'s delta is
//! this one).

#![cfg(unix)]

mod common;
mod lock_common;
mod verify_common;

use std::collections::BTreeSet;
use std::fs;

use common::git::Sandbox;
use common::{Scratch, data_dir, snapshot};
use lock_common::LockRepo;
use serde_json::{Value, json};
use specengine_store::{ProposalQueue as _, SqliteQueue};
use verify_common::{MODES, Verify, new_task, set_mode};

/// The fixture.
const FIXTURE: &str = "impact";

const BOARD: &str = "crates/ferry/src/board.rs";
const CROSSING: &str = "crates/ferry/src/crossing.rs";
const FARES: &str = "crates/ferry/src/fares.rs";
const FARES_RON: &str = "data/fares.ron";

/// `delta.symbols`' keys.
const DELTA_KEYS: [&str; 4] = ["added", "removed", "changed", "renamed"];

/// The queue's event count (none without a queue).
fn events(repo: &LockRepo) -> usize {
    let db = data_dir(&repo.home).join("ferry-lines.db");
    if !db.exists() {
        return 0;
    }
    SqliteQueue::open(&db, "ferry-lines")
        .expect("the queue opens")
        .events()
        .expect("events")
        .len()
}

/// `spec verify <extra>` text, then `--json`, from the root: both exit
/// alike; each run leaves every byte under the root (`.git` included) and
/// the queue's events as they were (`verify_common::verify` refuses a
/// queue with any event: the tasks here make some).
fn verify(repo: &LockRepo, extra: &[&str]) -> Verify {
    let mut args = vec!["verify"];
    args.extend_from_slice(extra);
    let before = (snapshot(&repo.root), events(repo));
    let text = repo.spec_piped(&repo.root, &args);
    assert_eq!(
        (snapshot(&repo.root), events(repo)),
        before,
        "{args:?} wrote"
    );
    args.push("--json");
    let json_run = repo.spec_piped(&repo.root, &args);
    assert_eq!(
        (snapshot(&repo.root), events(repo)),
        before,
        "{args:?} wrote"
    );
    assert_eq!(
        text.code,
        json_run.code,
        "text and JSON exit alike\n{}\n{}",
        text.show(),
        json_run.show()
    );
    let json = json_run.json();
    Verify {
        text,
        json_run,
        json,
    }
}

/// `fixtures/impact` at `<scratch>/repo`, `git init` and nothing committed.
fn unborn(label: &str) -> LockRepo {
    let scratch = Scratch::new(label);
    let git = Sandbox::new(scratch.path());
    let root = scratch.copy(FIXTURE, "repo");
    fs::remove_file(root.join("expected.json")).expect("expected.json");
    git.init(&root);
    let home = scratch.home("h");
    LockRepo {
        scratch,
        git,
        root,
        home,
    }
}

/// The fixture committed, then on disk: `late_fare`'s body edited (bound
/// to FARE-02), an untracked unbound file, `fares::rows` removed, a RON
/// value edited.
fn edited(label: &str) -> LockRepo {
    let repo = LockRepo::of(label, FIXTURE, "repo", &[]);
    edit(&repo);
    repo
}

fn edit(repo: &LockRepo) {
    repo.replace(FARES, "    crossings * 2\n", "    crossings * 4\n");
    repo.replace(FARES, "\npub fn rows() -> u32 {\n    2\n}\n", "");
    repo.write(
        "crates/ferry/src/spare.rs",
        "pub fn spare() -> u32 {\n    1\n}\n",
    );
    repo.replace(FARES_RON, "    late: 2,", "    late: 5,");
}

/// [`edit`]'s delta against `HEAD`.
fn edited_delta() -> Value {
    json!({
        "added": [
            {"file": "crates/ferry/src/spare.rs", "symbol": "crates/ferry::spare::spare",
             "item_kind": "function_item", "line": 1},
        ],
        "removed": [
            {"file": FARES, "symbol": "crates/ferry::fares::rows",
             "item_kind": "function_item", "line": 13},
        ],
        "changed": [
            {"file": FARES, "symbol": "crates/ferry::fares::late_fare",
             "item_kind": "function_item", "line": 9},
            {"file": FARES_RON, "symbol": FARES_RON, "item_kind": "file", "line": 1},
        ],
        "renamed": null,
    })
}

/// The keys of `delta.symbols` as printed, in order.
fn delta_keys(run: &Verify) -> Vec<String> {
    let raw = &run.json_run.stdout;
    let needle = "\"symbols\":{";
    let at = raw.find(needle).expect("delta.symbols is an object") + needle.len() - 1;
    let bytes = raw.as_bytes();
    let (mut depth, mut keys, mut in_string, mut escaped, mut start) =
        (0usize, Vec::new(), false, false, 0);
    for (index, &byte) in bytes.iter().enumerate().skip(at) {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
                if depth == 1 && bytes.get(index + 1) == Some(&b':') {
                    keys.push(raw[start..index].to_owned());
                }
            }
            continue;
        }
        match byte {
            b'"' => {
                in_string = true;
                start = index + 1;
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

/// A JSON document without `delta.symbols`.
fn without_symbols(document: &Value) -> Value {
    let mut document = document.clone();
    document["delta"]["symbols"] = Value::Null;
    document
}

// ------------------------------------------------------------------ RD-5

/// RD-5 (docs/features/impact-since.md "verify --changed"): with code
/// edited on disk (a bound body, an item removed, an untracked file, a RON
/// value), `delta.symbols` is `null` in plain verify, plain with `--task`,
/// `--staged` (the edits staged too) and `--node`, and under a run cause
/// (an unreadable `spec.lock`: `rows` `[]`); `--changed` fills it (the
/// contrast). M6: filled in plain.
#[test]
fn plain_verify_has_no_symbol_delta() {
    let repo = edited("vd-plain");
    let task = new_task(&repo, &["FARE-02"]);
    for extra in [
        &[][..],
        &["--task", task.as_str()],
        &["--node", "FARE-02"],
        &["--node", "TIDE-WINDOW"],
    ] {
        let run = verify(&repo, extra);
        assert_eq!(
            run.json["delta"]["symbols"],
            Value::Null,
            "{extra:?}: delta.symbols null\n{}",
            run.text.show()
        );
    }
    let changed = verify(&repo, &["--changed"]);
    assert_eq!(
        changed.json["delta"]["symbols"],
        edited_delta(),
        "premise: --changed fills it"
    );

    repo.git(&["add", "-A"]);
    let staged = verify(&repo, &["--staged"]);
    assert_eq!(staged.json["delta"]["symbols"], Value::Null, "--staged");

    let broken = LockRepo::of("vd-cause", FIXTURE, "repo", &[]);
    edit(&broken);
    fs::write(broken.root.join("spec.lock"), "garbage = [\n").expect("a broken lock");
    let cause = verify(&broken, &["--changed"]);
    assert_eq!(cause.code(), 2, "{}", cause.text.show());
    assert!(
        !cause.json["cannot"].as_array().expect("cannot").is_empty(),
        "premise: a run cause\n{}",
        cause.text.show()
    );
    assert_eq!(cause.json["rows"], json!([]), "a run cause: rows []");
    assert_eq!(
        cause.json["delta"]["symbols"],
        Value::Null,
        "a run cause: delta.symbols null"
    );
}

/// RD-5 (docs/features/impact-since.md "verify --changed"): `--changed
/// --json`, without and with `--task` (whose task names one unrelated
/// node: unfiltered, all code, bound or not), sets `delta.symbols` to the
/// edits' delta, keys `added, removed, changed, renamed` in order; with
/// nothing edited, three `[]`.
#[test]
fn changed_verify_fills_the_symbol_delta() {
    let repo = LockRepo::of("vd-changed", FIXTURE, "repo", &[]);
    let clean = verify(&repo, &["--changed"]);
    assert_eq!(
        clean.json["delta"]["symbols"],
        json!({"added": [], "removed": [], "changed": [], "renamed": null}),
        "all equal: three []"
    );
    edit(&repo);
    let task = new_task(&repo, &["NOTE-001"]);
    for extra in [&["--changed"][..], &["--changed", "--task", task.as_str()]] {
        let run = verify(&repo, extra);
        assert_eq!(run.json["delta"]["symbols"], edited_delta(), "{extra:?}");
        assert_eq!(delta_keys(&run), DELTA_KEYS, "{extra:?}: keys in order");
    }
}

/// RD-5 (docs/features/impact-since.md "verify --changed"): before the
/// first commit, `--changed --json`'s `delta.symbols` holds every record
/// of the disk `added`: each item `spec symbols --file` lists for every
/// `.rs` file (its qpath, kind, start line), each RON file as one record
/// (`file`, line 1), ordered by file bytes, line, qpath, item kind;
/// `removed` and `changed` `[]`.
#[test]
fn unborn_head_adds_every_item() {
    let repo = unborn("vd-unborn");
    let run = verify(&repo, &["--changed"]);
    let symbols = &run.json["delta"]["symbols"];
    // (file bytes, line, qpath, kind): the comparer's order; the item.
    type Keyed = (Vec<u8>, u64, Vec<u8>, Vec<u8>, Value);
    let mut want: Vec<Keyed> = Vec::new();
    for path in common::paths_under(&repo.root) {
        if path.starts_with(".git") {
            continue;
        }
        if path.ends_with(".ron") {
            want.push((
                path.as_bytes().to_vec(),
                1,
                path.as_bytes().to_vec(),
                b"file".to_vec(),
                json!({"file": path, "symbol": path, "item_kind": "file", "line": 1}),
            ));
        } else if path.ends_with(".rs") {
            let listed = repo.spec_piped(&repo.root, &["symbols", "--file", &path, "--json"]);
            assert_eq!(listed.code, 0, "{}", listed.show());
            let listed = listed.json();
            for symbol in listed["symbols"].as_array().expect("symbols") {
                let qpath = symbol["qpath"].as_str().expect("a qpath").to_owned();
                let kind = symbol["item_kind"].as_str().expect("a kind").to_owned();
                let line = symbol["start_line"].as_u64().expect("a line");
                want.push((
                    path.as_bytes().to_vec(),
                    line,
                    qpath.as_bytes().to_vec(),
                    kind.as_bytes().to_vec(),
                    json!({"file": path, "symbol": qpath, "item_kind": kind, "line": line}),
                ));
            }
        }
    }
    want.sort_by(|a, b| (&a.0, a.1, &a.2, &a.3).cmp(&(&b.0, b.1, &b.2, &b.3)));
    assert!(want.len() > 15, "premise: the fixture's items");
    let want: Vec<Value> = want.into_iter().map(|entry| entry.4).collect();
    assert_eq!(
        symbols["added"],
        Value::Array(want),
        "every item added\n{}",
        run.json_run.stdout
    );
    assert_eq!(symbols["removed"], json!([]));
    assert_eq!(symbols["changed"], json!([]));
    assert_eq!(symbols["renamed"], Value::Null);
}

/// RD-5 (docs/features/impact-since.md "verify --changed", RM-0031): a
/// function renamed with its body and marker kept, and a file moved, are
/// removed plus added; `renamed` stays `null`.
#[test]
fn renamed_stays_null() {
    let repo = LockRepo::of("vd-renamed", FIXTURE, "repo", &[]);
    repo.accept_ok("FARE-02");
    repo.replace(
        FARES,
        "pub fn late_fare(crossings: u32) -> u32 {",
        "pub fn night_fare(crossings: u32) -> u32 {",
    );
    let board = repo.read(BOARD);
    fs::remove_file(repo.root.join(BOARD)).expect("board.rs moved");
    repo.write("crates/ferry/src/display.rs", board);
    let run = verify(&repo, &["--changed"]);
    let symbols = &run.json["delta"]["symbols"];
    assert_eq!(symbols["renamed"], Value::Null, "renamed null");
    let names = |key: &str| -> BTreeSet<String> {
        symbols[key]
            .as_array()
            .expect("a list")
            .iter()
            .map(|item| item["symbol"].as_str().unwrap_or_default().to_owned())
            .collect()
    };
    assert_eq!(
        names("removed"),
        [
            "crates/ferry::board::reach",
            "crates/ferry::board::rows",
            "crates/ferry::fares::late_fare",
        ]
        .map(str::to_owned)
        .into_iter()
        .collect::<BTreeSet<_>>(),
        "{symbols}"
    );
    assert_eq!(
        names("added"),
        [
            "crates/ferry::display::reach",
            "crates/ferry::display::rows",
            "crates/ferry::fares::night_fare",
        ]
        .map(str::to_owned)
        .into_iter()
        .collect::<BTreeSet<_>>(),
        "{symbols}"
    );
    assert_eq!(symbols["changed"], json!([]), "{symbols}");
}

/// RD-5 (docs/features/impact-since.md "verify --changed"): with FARE-02
/// accepted and its body drifted (a finding), then unbound code edited on
/// top (`dock_time`'s body, which only a mention names; an untracked file;
/// a RON file without a marker), every `[verify] mode`'s `--changed` run,
/// with and without `--task`, keeps its exit, text (stdout and stderr) and
/// JSON but `delta.symbols` byte for byte; only `delta.symbols` grows.
/// Uncounted M: the verdict moving with the delta.
#[test]
fn symbol_delta_changes_no_verdict() {
    let repo = LockRepo::of("vd-verdict", FIXTURE, "repo", &[]);
    repo.accept_ok("FARE-02");
    repo.replace(FARES, "    crossings * 2\n", "    crossings * 6\n");
    let task = new_task(&repo, &["FARE-02"]);
    let forms: [Vec<&str>; 2] = [vec!["--changed"], vec!["--changed", "--task", &task]];
    let mut before = Vec::new();
    for mode in MODES {
        set_mode(&repo.root, mode);
        for form in &forms {
            before.push(verify(&repo, form));
        }
        repo.git(&["checkout", "-q", "--", "specengine.toml"]);
    }
    assert!(
        before
            .iter()
            .any(|run| run.json["counts"]["fix"].as_u64() > Some(0) && run.code() == 1),
        "premise: a finding that blocks under enforce"
    );
    repo.replace(
        CROSSING,
        "pub fn dock_time() -> u32 {\n    3\n}",
        "pub fn dock_time() -> u32 {\n    30\n}",
    );
    repo.write(
        "crates/ferry/src/spare.rs",
        "pub fn spare() -> u32 {\n    1\n}\n",
    );
    repo.write("data/spare.ron", "Spare(\n    count: 1,\n)\n");
    let mut at = 0;
    for mode in MODES {
        set_mode(&repo.root, mode);
        for form in &forms {
            let old = &before[at];
            at += 1;
            let new = verify(&repo, form);
            let what = format!("{mode} {form:?}");
            assert_eq!(new.code(), old.code(), "{what}: the exit");
            assert_eq!(new.text.stdout, old.text.stdout, "{what}: the text");
            assert_eq!(new.text.stderr, old.text.stderr, "{what}: stderr");
            assert_eq!(
                without_symbols(&new.json),
                without_symbols(&old.json),
                "{what}: the JSON but delta.symbols"
            );
            assert_eq!(new.json["verdict"], old.json["verdict"], "{what}: verdict");
            let grown = |run: &Verify| {
                ["added", "changed"]
                    .iter()
                    .map(|key| {
                        run.json["delta"]["symbols"][key]
                            .as_array()
                            .map_or(0, Vec::len)
                    })
                    .sum::<usize>()
            };
            assert!(
                grown(&new) > grown(old),
                "{what}: premise, the delta grew\n{}",
                new.json["delta"]["symbols"]
            );
        }
        repo.git(&["checkout", "-q", "--", "specengine.toml"]);
    }
}

/// The owner's decision on docs/features/impact-since.md "verify --changed"
/// (the developer's D1): `delta.symbols` is computed for the JSON alone.
/// Through the library, `verify_as(.., VerifyFormat::Json)` (as `verify`)
/// serialises the edits' delta, `VerifyFormat::Text` a `null` one (no comparer
/// run, so no item hashed), the rest of the outcome and the exit equal; the
/// binary's `--json` the filled one. M: the comparer run for a text run (the
/// cost the decision avoids).
#[test]
fn text_verify_computes_no_symbol_delta() {
    use specengine_cli::{
        Globals, VerifiedTree, VerifyFormat, VerifyRequest, verify as verify_json, verify_as,
    };
    let repo = edited("vd-text");
    let request = VerifyRequest {
        tree: VerifiedTree::Changed,
        task: None,
        node: None,
        symbol: None,
        git: repo.git_env(&repo.root),
    };
    let env = repo.env(&repo.root);
    let json_run = verify_json(&env, &Globals::default(), &request).expect("verify --changed");
    let text_run = verify_as(&env, &Globals::default(), &request, VerifyFormat::Text)
        .expect("verify --changed, text");
    let json_doc = serde_json::to_value(&json_run).expect("the JSON outcome");
    let text_doc = serde_json::to_value(&text_run).expect("the text outcome");
    assert_eq!(
        json_doc["delta"]["symbols"],
        edited_delta(),
        "--json: filled"
    );
    assert_eq!(
        text_doc["delta"]["symbols"],
        Value::Null,
        "a text run computes no delta"
    );
    assert_eq!(without_symbols(&json_doc), without_symbols(&text_doc));
    assert_eq!(json_run.exit(), text_run.exit(), "the exit");
    let binary = verify(&repo, &["--changed"]);
    assert_eq!(
        binary.json["delta"]["symbols"],
        edited_delta(),
        "the binary's --json"
    );
}
