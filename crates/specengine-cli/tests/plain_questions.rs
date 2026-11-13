//! docs/features/plain-questions.md (RM-0088, the question half)
//! "Acceptance criteria", the CLI's half: the `spec` binary over scratch git
//! copies of `fixtures/spec-a` (`common::proposal::Pair`: a main worktree, a
//! linked one on `t1`, a scratch `HOME`), every item raised in `t1`; the
//! library where the terminal is the subject (`approve_with` with a consent
//! callback, `round_answer` with a scripted terminal, `import_state`).
//!
//! The "Data" example as the spec writes it is not stored on spec-a: its two
//! targets are answered by the accepted DEC-0023, a hit. Stored, it names
//! that hit (`--distinct-from DEC-0023`), as every "Data" item here does.
//!
//! - PQ1 `pq1_*`: the example stores `PR-0001`; `spec review --json` returns
//!   `gist`, `matters`, both options and `recommendation` 0 byte-equal to
//!   the input, 53 keys, the two after `summary`; the text has `gist:` and
//!   `matters:` after `summary:` and the options as a discrepancy's;
//!   `--options` from a file stores the same. M: `matters` not stored.
//! - PQ2 `pq2_*`: each cap exits 1 with `spec: <line>`, the `--json`
//!   document its note, nothing stored, no event; a malformed `--options`
//!   exits 2 naming the flag. M: 4 options accepted.
//! - PQ3 `pq3_*`: a question without the fields answers byte-equal to HEAD's
//!   build (text and JSON), its four columns `NULL`, the review's `gist`
//!   and `matters` null; a decision's or a discrepancy's `--input` naming
//!   `gist` or `matters` exits 2; clap refuses the flags on the other
//!   kinds. M: `gist` required.
//! - PQ4 `pq4_*`: `approve --option 1` records option 1 (title, effect,
//!   cost, the options listed, `{"option":1}`); no flag the working answer,
//!   `--answer T` T, each with the options listed; `--option 2` exits 1
//!   naming `0-1`; an option-less question's and a decision's `--option 0`
//!   exit 2 with the new line; a discrepancy without `--option` keeps its
//!   line. M: `--option` on a question ignored.
//! - PQ6 `pq6_*`: the fields hold IDs, paths, `::`, U+202E and CRLF
//!   byte-equal through JSON, escaped in text; the note names the first of
//!   `MEC-STAMINA`, `docs/spec/x.md` and `stamina_regen::tick`, once, last,
//!   stored or not, never on a refusal; none for `stamina` and `ZZ-99`. M:
//!   IDs stripped from the stored gist; the note refusing.
//! - PQ7 `pq7_*`: `spec round new` prints the "Data" sheet; `PR-0001: 1`
//!   stages option 1, `ok` the working answer, `no` `--answer no`; an
//!   option-less question still refuses `1`; the JSON item 21 keys. M: the
//!   sheet refusing a number on a question.
//! - PQ8 `pq8_*`: `spec export state` writes format 7, schema 13, 50
//!   columns, `gist` and `matters` last; a format 6 dump (this build's
//!   export with the two columns out and the header 6/12) imports both
//!   `NULL` and re-exports equal but for the header and the two nulls; a
//!   format 7 header at schema 12, a format 7 row without `matters`, a
//!   format 6 row with `gist`: exit 2 at the line; schema 14 is refused. M:
//!   the format 6 import path dropped.
//! - PQ10, the CLI's half: `spec inbox` gains `gist` (13 keys, the text's
//!   last column the gist), `spec card show`'s items `gist` and `matters`
//!   (18 keys, the `items:` line the gist).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::decision::{flags, option};
use common::proposal::{DECIDER, Pair, cannot, json_of, printed, refused};
use common::{Run, data_dir, read_text};
use serde_json::{Value, json};
use specengine_cli::{
    Env, Exit, ExportStateRequest, Globals, ImportStateRequest, Outcome, RoundAnswerRequest,
    RoundNewRequest, RoundTerminal, STATE_FORMAT, export_state, import_state, render_json,
    render_text, round_answer, round_new, utc_now,
};
use specengine_store::{Choice, PROPOSAL_COLUMNS, QUEUE_SCHEMA_VERSION, Stage};

/// The "Data" example's fields.
const TEXT: &str =
    "Does RULE-STAM-REGEN's delay restart when stamina_regen() is called mid-sprint?";
const WORKING: &str = "yes, the 1.5 s delay restarts";
const PRICE: &str = "R-28 rebalanced";
const GIST: &str = "Should resting restart the wait before stamina refills?";
const MATTERS: &str = "Sprint tuning and two tasks wait on it.";

/// The "Data" example's stdin.
const OPTIONS: &str = "[{\"label\": \"Restart the wait\", \"effect\": \"Short rests do not \
                       refill\", \"price\": \"none: as built\"},\n {\"label\": \"Keep the wait \
                       running\", \"effect\": \"Tapping rest refills faster\", \"price\": \
                       \"R-28 rebalanced\"}]\n";

/// The accepted decision answering both of the example's targets.
const HIT_LINE: &str =
    "hit: DEC-0023 | accepted | docs/records/DEC/DEC-0023.md | Regeneration waits for rest";

/// The example's options as `spec review` lists them.
const OPTION_LINES: &str = "options: 2\n  [0] Restart the wait | Short rests do not refill | \
                            none: as built (recommended)\n  [1] Keep the wait running | \
                            Tapping rest refills faster | R-28 rebalanced\nrecommendation: 0\n";

/// The review document's keys: RM-0059's 51 with `gist` and `matters`
/// after `summary`; 54 since docs/features/collision-pairs.md PP8 (`pair`
/// after `evidence`).
const REVIEW_KEYS: [&str; 54] = [
    "id",
    "project",
    "kind",
    "status",
    "target_id",
    "target_path",
    "worktree",
    "branch",
    "base_commit",
    "base_hash",
    "base_text",
    "new_text",
    "patch_hash",
    "rationale",
    "author",
    "diagnostics",
    "diff",
    "preview",
    "conflict",
    "revisions",
    "drift",
    "decided_by",
    "decided_at",
    "decision_note",
    "applied_commit",
    "created_at",
    "updated_at",
    "target_ids",
    "severity",
    "gap_type",
    "summary",
    "gist",
    "matters",
    "working_answer",
    "price_of_other",
    "evidence",
    "pair",
    "options",
    "recommendation",
    "distinct_from",
    "linked",
    "record_id",
    "record_path",
    "record_title",
    "record_text",
    "choice",
    "task_id",
    "staged",
    "staged_at",
    "interpretation",
    "card_id",
    "why",
    "similar",
    "notes",
];

/// A round item's keys: `gist` and `matters` after `summary`; 22 since
/// docs/features/collision-pairs.md PP8 (`pair` after `evidence`).
const ROUND_ITEM_KEYS: [&str; 22] = [
    "id",
    "kind",
    "severity",
    "gap_type",
    "summary",
    "gist",
    "matters",
    "working_answer",
    "price_of_other",
    "options",
    "recommendation",
    "evidence",
    "pair",
    "targets",
    "tasks",
    "worktree",
    "branch",
    "author",
    "staged",
    "staged_at",
    "created_at",
    "updated_at",
];

/// An inbox entry's 13 keys: `gist` after `summary`.
const INBOX_KEYS: [&str; 13] = [
    "id",
    "kind",
    "status",
    "target_id",
    "target_ids",
    "branch",
    "created_at",
    "rationale",
    "severity",
    "summary",
    "gist",
    "record_id",
    "staged_at",
];

/// A card item's 18 keys: `gist` and `matters` after `summary`.
const CARD_ITEM_KEYS: [&str; 18] = [
    "id",
    "kind",
    "status",
    "summary",
    "gist",
    "matters",
    "working_answer",
    "why",
    "author",
    "created_at",
    "updated_at",
    "staged_at",
    "decided_by",
    "decided_at",
    "decision_note",
    "record_id",
    "record_path",
    "record_title",
];

// ---------------------------------------------------------------- helpers

/// The example's arguments after `spec propose question`, as the spec
/// writes them (`--options -`).
fn example_args() -> Vec<&'static str> {
    vec![
        "MEC-STAMINA",
        "RULE-STAM-REGEN",
        "--text",
        TEXT,
        "--working-answer",
        WORKING,
        "--price-of-other",
        PRICE,
        "--gist",
        GIST,
        "--matters",
        MATTERS,
        "--options",
        "-",
        "--recommendation",
        "0",
        "--author-role",
        "developer",
    ]
}

/// `spec propose question <args>` in `t1`, `stdin` piped.
fn ask(pair: &Pair, args: &[&str], stdin: &[u8]) -> Run {
    let mut all = vec!["propose", "question"];
    all.extend_from_slice(args);
    pair.spec_piped(&pair.linked, &all, stdin)
}

/// [`ask`] that must store: the new ID (stdout's first line).
fn stored(pair: &Pair, args: &[&str], stdin: &[u8]) -> String {
    let run = ask(pair, args, stdin);
    run.code(0);
    let id = run.stdout.lines().next().unwrap_or_default().to_owned();
    assert!(id.starts_with("PR-"), "not stored: {}", run.show());
    id
}

/// The example stored, its hit named.
fn example(pair: &Pair) -> String {
    let mut args = example_args();
    args.extend(["--distinct-from", "DEC-0023"]);
    stored(pair, &args, OPTIONS.as_bytes())
}

/// A question of `text` on `target` with options `[a, b]` and the given
/// flags beside, its hits named.
fn priced(pair: &Pair, target: &str, text: &str, extra: &[&str]) -> String {
    let mut args = vec![
        target,
        "--text",
        text,
        "--working-answer",
        WORKING,
        "--price-of-other",
        PRICE,
        "--options",
        "-",
        "--author-role",
        "developer",
    ];
    args.extend_from_slice(extra);
    let run = ask(pair, &args, OPTIONS.as_bytes());
    run.code(0);
    if run.stdout.starts_with("PR-") {
        return run.stdout.lines().next().unwrap().to_owned();
    }
    let json = ask(
        pair,
        &[args.as_slice(), &["--json"]].concat(),
        OPTIONS.as_bytes(),
    )
    .json();
    let mut named = args.clone();
    let hits: Vec<String> = json["hits"]
        .as_array()
        .expect("hits")
        .iter()
        .map(|hit| hit["id"].as_str().expect("an id").to_owned())
        .collect();
    for hit in &hits {
        named.extend(["--distinct-from", hit.as_str()]);
    }
    stored(pair, &named, OPTIONS.as_bytes())
}

/// `spec <args>` in `t1`, stdin empty, exit 0: stdout.
fn spec_ok(pair: &Pair, args: &[&str]) -> String {
    let run = pair.spec_piped(&pair.linked, args, b"");
    run.code(0);
    run.stdout
}

fn review_json(pair: &Pair, id: &str) -> Value {
    json_of(&spec_ok(pair, &["review", id, "--json"]))
}

/// `spec review ID --json`'s keys, in order.
fn review_keys(pair: &Pair, id: &str) -> Vec<String> {
    Ordered::parse(&spec_ok(pair, &["review", id, "--json"]))
        .keys()
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn review_text(pair: &Pair, id: &str) -> String {
    spec_ok(pair, &["review", id])
}

/// `SELECT <columns> FROM proposals WHERE id = <id>` through `sqlite3`,
/// each `NULL` printed as `<null>`.
fn columns(pair: &Pair, id: &str, names: &[&str]) -> String {
    let shown: Vec<String> = names
        .iter()
        .map(|name| format!("ifnull({name}, '<null>')"))
        .collect();
    pair.sql(&format!(
        "SELECT {} FROM proposals WHERE id = '{id}';",
        shown.join(", ")
    ))
}

/// A JSON value whose objects keep their keys in document order (the
/// tests' `serde_json` keeps no order).
#[derive(Debug)]
enum Ordered {
    Object(Vec<(String, Ordered)>),
    Array(Vec<Ordered>),
    Scalar,
}

impl<'de> serde::Deserialize<'de> for Ordered {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Ordered;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("JSON")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Ordered, A::Error> {
                let mut members = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, Ordered>()? {
                    members.push((key, value));
                }
                Ok(Ordered::Object(members))
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Ordered, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element::<Ordered>()? {
                    items.push(item);
                }
                Ok(Ordered::Array(items))
            }

            fn visit_bool<E>(self, _: bool) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }

            fn visit_i64<E>(self, _: i64) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }

            fn visit_u64<E>(self, _: u64) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }

            fn visit_f64<E>(self, _: f64) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }

            fn visit_str<E>(self, _: &str) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }

            fn visit_unit<E>(self) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

impl Ordered {
    fn parse(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_else(|error| panic!("{error}: {text}"))
    }

    /// The object's keys, in document order.
    fn keys(&self) -> Vec<&str> {
        match self {
            Self::Object(members) => members.iter().map(|(key, _)| key.as_str()).collect(),
            other => panic!("no object: {other:?}"),
        }
    }

    fn get(&self, key: &str) -> &Self {
        match self {
            Self::Object(members) => members
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value)
                .unwrap_or_else(|| panic!("no {key}")),
            other => panic!("no object: {other:?}"),
        }
    }

    fn items(&self) -> &[Self] {
        match self {
            Self::Array(items) => items,
            other => panic!("no array: {other:?}"),
        }
    }
}

/// The number of stored proposals and events (0 with no queue).
fn stored_counts(pair: &Pair) -> (usize, usize) {
    if !pair.db().exists() {
        return (0, 0);
    }
    (pair.proposals().len(), pair.events().len())
}

// ------------------------------------------------------------------ PQ1

/// PQ1: the example as the spec writes it is not stored (DEC-0023 hits);
/// with the hit named it stores `PR-0001`; the review returns the four
/// fields byte-equal to the input in 53 keys, the text its lines in order.
/// M: `matters` not stored.
#[test]
fn pq1_the_example_round_trips() {
    let pair = Pair::new("pq-pq1", "spec-a");
    let run = ask(&pair, &example_args(), OPTIONS.as_bytes());
    run.code(0);
    assert_eq!(
        run.stdout,
        format!("{HIT_LINE}\nnot stored: name every hit in `distinct_from` to store it anyway\n"),
        "the example as written meets DEC-0023"
    );
    assert_eq!(stored_counts(&pair).0, 0);

    let id = example(&pair);
    assert_eq!(id, "PR-0001");
    let json = review_json(&pair, &id);
    assert_eq!(review_keys(&pair, &id), REVIEW_KEYS);
    assert_eq!(json["kind"], json!("question"));
    assert_eq!(json["gist"], json!(GIST));
    assert_eq!(json["matters"], json!(MATTERS));
    let given: Value = serde_json::from_str(OPTIONS).expect("the example's options");
    assert_eq!(json["options"], given);
    assert_eq!(json["recommendation"], json!(0));
    assert_eq!(json["summary"], json!(TEXT));
    assert_eq!(json["working_answer"], json!(WORKING));
    assert_eq!(json["price_of_other"], json!(PRICE));
    assert_eq!(json["evidence"], json!([]));
    assert_eq!(json["gap_type"], Value::Null);
    assert_eq!(
        json["target_ids"],
        json!(["MEC-STAMINA", "RULE-STAM-REGEN"])
    );

    let text = review_text(&pair, &id);
    let lines = format!(
        "\nsummary: {TEXT}\ngist: {GIST}\nmatters: {MATTERS}\nworking_answer: {WORKING}\n\
         price_of_other: {PRICE}\nevidence: 0\n{OPTION_LINES}"
    );
    assert!(text.contains(&lines), "{lines:?} in:\n{text}");

    // The row as stored: the texts raw, the options as JSON.
    assert_eq!(
        columns(&pair, &id, &["gist", "matters", "recommendation"]),
        format!("{GIST}|{MATTERS}|0\n")
    );
    let options: Value =
        serde_json::from_str(columns(&pair, &id, &["options"]).trim_end()).expect("JSON options");
    assert_eq!(options, given);

    // `--options F`: the same options from a file outside the worktree.
    let file = pair.scratch.dir("in").join("options.json");
    fs::write(&file, OPTIONS).unwrap();
    let mut args = example_args();
    let at = args.iter().position(|arg| *arg == "-").unwrap();
    let path = file.to_str().unwrap().to_owned();
    args[at] = path.as_str();
    args[3] = "Does the wait restart on a short rest?";
    args.extend(["--distinct-from", "DEC-0023"]);
    let second = stored(&pair, &args, b"");
    assert_eq!(review_json(&pair, &second)["options"], given);
    assert_eq!(review_json(&pair, &second)["recommendation"], json!(0));
}

// ------------------------------------------------------------------ PQ2

/// The example's arguments with `--options -` fed `options` and `edit`
/// applied: exit 1, `spec: <line>` alone on stderr, stdout empty; with
/// `--json` the document's note is the line; nothing stored, no event.
fn refused_with(pair: &Pair, label: &str, args: &[&str], options: &str, line: &str) {
    let before = stored_counts(pair);
    let run = ask(pair, args, options.as_bytes());
    assert_eq!(run.code, 1, "{label}: {}", run.show());
    assert_eq!(run.stderr, format!("spec: {line}\n"), "{label}");
    assert_eq!(run.stdout, "", "{label}");
    let mut json_args = args.to_vec();
    json_args.push("--json");
    let run = ask(pair, &json_args, options.as_bytes());
    assert_eq!(run.code, 1, "{label}: {}", run.show());
    assert_eq!(
        run.json(),
        json!({"id": null, "created": false, "hits": [], "related": [], "linked": null,
            "diagnostics": [], "notes": [line]}),
        "{label}"
    );
    assert_eq!(stored_counts(pair), before, "{label}: nothing stored");
}

/// `count` options, each valid.
fn options_of(count: usize) -> String {
    let items: Vec<Value> = (0..count)
        .map(|index| json!({"label": format!("Way {index}"), "effect": "e", "price": "p"}))
        .collect();
    Value::Array(items).to_string()
}

/// The example's arguments with `flag`'s value replaced (or removed with
/// `None`).
fn with_flag<'a>(flag: &str, value: Option<&'a str>) -> Vec<&'a str> {
    let mut args = example_args();
    let at = args
        .iter()
        .position(|arg| *arg == flag)
        .unwrap_or_else(|| panic!("no {flag}"));
    match value {
        Some(value) => args[at + 1] = value,
        None => {
            args.drain(at..at + 2);
        }
    }
    args.extend(["--distinct-from", "DEC-0023"]);
    args
}

/// PQ2: each cap exits 1 with its line, stores nothing. M: 4 options
/// accepted.
#[test]
fn pq2_each_cap_exits_1_with_its_line() {
    let pair = Pair::new("pq-pq2", "spec-a");
    let long_gist = "g".repeat(161);
    let long_matters = "m".repeat(513);
    let long_label = json!([{"label": "l".repeat(129), "effect": "e", "price": "p"},
        {"label": "b", "effect": "e", "price": "p"}])
    .to_string();
    let cases: Vec<(&str, Vec<&str>, String, &str)> = vec![
        (
            "4 options",
            with_flag("--recommendation", Some("0")),
            options_of(4),
            "options: 4 option(s); give 2 to 3, each priced",
        ),
        (
            "1 option",
            with_flag("--recommendation", Some("0")),
            options_of(1),
            "options: 1 option(s); give 2 to 3, each priced",
        ),
        (
            "[]",
            with_flag("--recommendation", None),
            "[]".to_owned(),
            "options: 0 option(s); give 2 to 3, each priced",
        ),
        (
            "a 161-byte gist",
            with_flag("--gist", Some(&long_gist)),
            OPTIONS.to_owned(),
            "gist: 161 bytes; at most 160",
        ),
        (
            "a gist holding LF",
            with_flag("--gist", Some("Should resting\nrestart the wait?")),
            OPTIONS.to_owned(),
            "gist: holds a line break; give one line",
        ),
        (
            "a blank gist",
            with_flag("--gist", Some("   ")),
            OPTIONS.to_owned(),
            "gist: blank: give a value",
        ),
        (
            "a 513-byte matters",
            with_flag("--matters", Some(&long_matters)),
            OPTIONS.to_owned(),
            "matters: 513 bytes; at most 512",
        ),
        (
            "--recommendation 2 with 2 options",
            with_flag("--recommendation", Some("2")),
            OPTIONS.to_owned(),
            "recommendation: 2 is no index into the 2 options (0 to 1)",
        ),
        (
            "a recommendation without options",
            with_flag("--options", None),
            String::new(),
            "recommendation: given without options",
        ),
        (
            "a 129-byte label",
            with_flag("--recommendation", Some("0")),
            long_label,
            "options[0].label: 129 bytes; at most 128",
        ),
    ];
    for (label, args, options, line) in &cases {
        refused_with(&pair, label, args, options, line);
    }
    assert_eq!(stored_counts(&pair), (0, 0), "no queue row, no event");
    // The same item within its caps stores PR-0001: no ID was taken.
    assert_eq!(example(&pair), "PR-0001");
}

/// PQ2: a malformed `--options` (stdin or a file) exits 2 naming the flag,
/// stdout empty, nothing stored; a missing file too.
#[test]
fn pq2_a_malformed_options_document_exits_2() {
    let pair = Pair::new("pq-pq2-bad", "spec-a");
    let mut big = OPTIONS.as_bytes().to_vec();
    big.extend(std::iter::repeat_n(b' ', 8 << 20));
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("not UTF-8", b"[{\"label\": \"\xff\"}]".to_vec()),
        ("not JSON", b"[{".to_vec()),
        (
            "an object",
            b"{\"label\": \"a\", \"effect\": \"e\", \"price\": \"p\"}".to_vec(),
        ),
        (
            "no price",
            b"[{\"label\": \"a\", \"effect\": \"e\"}, {\"label\": \"b\", \"effect\": \"e\", \
              \"price\": \"p\"}]"
                .to_vec(),
        ),
        (
            "an unknown key",
            b"[{\"label\": \"a\", \"effect\": \"e\", \"price\": \"p\", \"why\": \"w\"}, \
              {\"label\": \"b\", \"effect\": \"e\", \"price\": \"p\"}]"
                .to_vec(),
        ),
        ("a number", b"7".to_vec()),
        ("over 8 MiB", big),
    ];
    let mut args = example_args();
    args.extend(["--distinct-from", "DEC-0023"]);
    let dir = pair.scratch.dir("bad");
    for (name, bytes) in &cases {
        let run = ask(&pair, &args, bytes);
        assert_eq!(run.code, 2, "{name}: {}", run.show());
        assert!(
            run.stderr.starts_with("spec: --options "),
            "{name}: {}",
            run.stderr
        );
        assert_eq!(run.stdout, "", "{name}");
        let file = dir.join(format!("{}.json", name.replace(' ', "-")));
        fs::write(&file, bytes).unwrap();
        let mut from_file = args.clone();
        let at = from_file.iter().position(|arg| *arg == "-").unwrap();
        let path = file.to_str().unwrap().to_owned();
        from_file[at] = path.as_str();
        let run = ask(&pair, &from_file, b"");
        assert_eq!(run.code, 2, "{name} (file): {}", run.show());
        assert!(
            run.stderr.starts_with("spec: --options "),
            "{name}: {}",
            run.stderr
        );
    }
    let mut missing = args.clone();
    let at = missing.iter().position(|arg| *arg == "-").unwrap();
    let absent = dir.join("absent.json");
    let absent = absent.to_str().unwrap().to_owned();
    missing[at] = absent.as_str();
    let run = ask(&pair, &missing, b"");
    assert_eq!(run.code, 2, "a missing file: {}", run.show());
    assert_eq!(stored_counts(&pair), (0, 0));
}

// ------------------------------------------------------------------ PQ3

/// PQ3: a question with none of the fields answers byte-equal to the
/// build before this slice (recorded at 4d52ec3c: text and JSON), stores
/// the four columns `NULL`, and reviews with `gist` and `matters` null
/// (`gist: -`, `matters: -`). M: `gist` required.
#[test]
fn pq3_nothing_given_nothing_changed() {
    let pair = Pair::new("pq-pq3", "spec-a");
    let args = [
        "MEC-STAMINA",
        "RULE-STAM-REGEN",
        "--text",
        TEXT,
        "--working-answer",
        WORKING,
        "--price-of-other",
        PRICE,
        "--distinct-from",
        "DEC-0023",
        "--author-role",
        "developer",
    ];
    let run = ask(&pair, &args, b"");
    run.code(0);
    assert_eq!(run.stdout, format!("PR-0001\n{HIT_LINE}\n"));
    assert_eq!(run.stderr, "");
    assert_eq!(
        columns(
            &pair,
            "PR-0001",
            &["gist", "matters", "options", "recommendation"]
        ),
        "<null>|<null>|<null>|<null>\n"
    );
    let json = review_json(&pair, "PR-0001");
    assert_eq!(review_keys(&pair, "PR-0001"), REVIEW_KEYS);
    assert_eq!(json["gist"], Value::Null);
    assert_eq!(json["matters"], Value::Null);
    assert_eq!(json["options"], json!([]));
    assert_eq!(json["recommendation"], Value::Null);
    let text = review_text(&pair, "PR-0001");
    assert!(
        text.contains(&format!(
            "\nsummary: {TEXT}\ngist: -\nmatters: -\nworking_answer: {WORKING}\n"
        )),
        "{text}"
    );
    assert!(text.contains("\noptions: 0\nrecommendation: -\n"), "{text}");

    // The JSON answer as before: no note, the seven keys.
    let mut json_args = args.to_vec();
    json_args[3] = "Does the wait restart at zero stamina?";
    json_args.push("--json");
    let run = ask(&pair, &json_args, b"");
    run.code(0);
    assert_eq!(
        run.json(),
        json!({"id": "PR-0002", "created": true, "hits": [{"id": "DEC-0023",
            "source": "corpus", "status": "accepted", "path": "docs/records/DEC/DEC-0023.md",
            "answer": "Regeneration waits for rest", "record": "DEC-0023"}],
            "related": [{"id": "PR-0001", "source": "queue", "status": "open", "path": null,
                "answer": null, "record": null}],
            "linked": null, "diagnostics": [], "notes": []})
    );
    // A gist alone (no options) stores, and no note for plain words.
    let mut gist_args = args.to_vec();
    gist_args[3] = "Does the wait restart after a fall?";
    gist_args.extend(["--gist", GIST, "--json"]);
    let run = ask(&pair, &gist_args, b"");
    run.code(0);
    assert_eq!(run.json()["notes"], json!([]));
    assert_eq!(
        columns(
            &pair,
            "PR-0003",
            &["gist", "matters", "options", "recommendation"]
        ),
        format!("{GIST}|<null>|<null>|<null>\n")
    );
}

/// PQ3, "Rules": a decision or a discrepancy is never given the fields:
/// `--input` naming `gist` or `matters` exits 2 (an unknown field); clap
/// refuses `--gist` on `propose discrepancy`; nothing stored.
#[test]
fn pq3_a_decision_or_a_discrepancy_takes_no_gist() {
    let pair = Pair::new("pq-pq3-kinds", "spec-a");
    let decision = json!({"node_ids": ["EDGE-STAM-ZERO"], "text": "Which oil?",
        "working_answer": "Whale oil.", "price_of_other": "One scene."});
    let discrepancy = json!({"node_ids": ["EDGE-STAM-ZERO"], "summary": "S",
        "gap_type": "partial", "severity": "low",
        "evidence": [{"file": "src/a.rs", "observed": "o", "documented": "d"}],
        "options": [{"label": "a", "effect": "e", "price": "p"},
            {"label": "b", "effect": "e", "price": "p"}], "recommendation": 1});
    for (kind, document) in [("decision", decision), ("discrepancy", discrepancy)] {
        for field in ["gist", "matters"] {
            let mut given = document.clone();
            given[field] = json!("A line.");
            let run = pair.spec_piped(
                &pair.linked,
                &[
                    "propose",
                    kind,
                    "--input",
                    "-",
                    "--author-role",
                    "developer",
                ],
                given.to_string().as_bytes(),
            );
            assert_eq!(run.code, 2, "{kind} {field}: {}", run.show());
            assert!(run.stderr.contains(field), "{kind} {field}: {}", run.stderr);
        }
        let run = pair.spec_piped(
            &pair.linked,
            &[
                "propose",
                kind,
                "--input",
                "-",
                "--gist",
                "A line.",
                "--author-role",
                "developer",
            ],
            document.to_string().as_bytes(),
        );
        assert_eq!(run.code, 2, "{kind} --gist: {}", run.show());
    }
    assert_eq!(stored_counts(&pair), (0, 0));
}

// ------------------------------------------------------------------ PQ4

/// A record's body from `# <title>` to `Note:`, as spec-a's template
/// places the slots.
fn record_body(
    title: &str,
    choice: &str,
    effect: &str,
    cost: &str,
    summary: &str,
    options: &str,
) -> String {
    format!(
        "# {title}\n\nChoice:\n\n{choice}\n\nEffect:\n\n{effect}\n\nCost:\n\n{cost}\n\n\
         Summary:\n\n{summary}\n\nOptions:\n\n{options}\n\nEvidence:\n\n\n\nNote:\n\n"
    )
}

/// The example's options as a record's `{{options}}` lists them.
const OPTIONS_LISTED: &str = "- Restart the wait | Short rests do not refill | none: as built\n\
                              - Keep the wait running | Tapping rest refills faster | R-28 \
                              rebalanced";

/// PQ4: `--option 1` records option 1; no flag the working answer;
/// `--answer T` T; the options listed in each. M: `--option` on a question
/// ignored (the working answer recorded).
#[test]
fn pq4_a_question_with_options_takes_an_option_its_working_answer_or_an_answer() {
    let pair = Pair::new("pq-pq4", "spec-a");
    let id = example(&pair);
    let (outcome, question) = pair.decide_ok(&pair.main, &id, &option(1), None);
    let path = "docs/records/DEC/DEC-0024.md";
    let record = read_text(&pair.linked, path);
    assert!(
        record.contains("\ntitle: \"Keep the wait running\"\n"),
        "{record}"
    );
    assert!(
        record.contains(&record_body(
            "Keep the wait running",
            "Keep the wait running",
            "Tapping rest refills faster",
            PRICE,
            TEXT,
            OPTIONS_LISTED,
        )),
        "{record}"
    );
    assert!(record.ends_with(&format!("Decided by:\n\n{DECIDER}\n")));
    assert!(question.contains(&format!("apply {id} as DEC-0024 (option 1) on t1")));
    assert_eq!(
        json_of(&printed(&outcome).1)["choice"],
        json!({"option": 1})
    );
    let kept = pair.proposal(&id).record.expect("the record stored");
    assert_eq!(kept.choice, Choice::Option(1));
    assert_eq!(kept.title, "Keep the wait running");

    let working = priced(&pair, "MEC-SPRINT", "Does a sprint restart the wait?", &[]);
    pair.decide_ok(&pair.main, &working, &flags(None, None, None), None);
    let record = read_text(&pair.linked, "docs/records/DEC/DEC-0025.md");
    assert!(
        record.contains(&record_body(
            WORKING,
            WORKING,
            "",
            "",
            "Does a sprint restart the wait?",
            OPTIONS_LISTED,
        )),
        "{record}"
    );
    assert_eq!(
        pair.proposal(&working).record.unwrap().choice,
        Choice::WorkingAnswer
    );

    let answered = priced(&pair, "MEC-SPRINT", "Does a jump restart the wait?", &[]);
    pair.decide_ok(
        &pair.main,
        &answered,
        &flags(None, Some("No, it keeps running."), None),
        None,
    );
    let record = read_text(&pair.linked, "docs/records/DEC/DEC-0026.md");
    assert!(
        record.contains(&record_body(
            "No, it keeps running.",
            "No, it keeps running.",
            "",
            PRICE,
            "Does a jump restart the wait?",
            OPTIONS_LISTED,
        )),
        "{record}"
    );
    assert_eq!(
        pair.proposal(&answered).record.unwrap().choice,
        Choice::Answer("No, it keeps running.".to_owned())
    );
}

/// PQ4: `--option 2` of two exits 1 naming `0-1`; an option-less question's
/// and a decision's `--option 0` exit 2 with the new line; a discrepancy
/// without `--option` keeps today's line; nothing written.
#[test]
fn pq4_the_flags_are_judged_by_the_items_fields() {
    let pair = Pair::new("pq-pq4-flags", "spec-a");
    let priced = priced(&pair, "MEC-SPRINT", "Does a sprint restart the wait?", &[]);
    let bare = stored(
        &pair,
        &[
            "EDGE-STAM-ZERO",
            "--text",
            "Does the lamp stay lit?",
            "--working-answer",
            "yes",
            "--price-of-other",
            "a rule",
            "--author-role",
            "developer",
        ],
        b"",
    );
    let decision = pair.spec_piped(
        &pair.linked,
        &[
            "propose",
            "decision",
            "--input",
            "-",
            "--author-role",
            "developer",
        ],
        json!({"node_ids": ["RULE-SPRINT-COST"], "text": "Which oil burns?",
            "working_answer": "Whale oil.", "price_of_other": "One scene."})
        .to_string()
        .as_bytes(),
    );
    decision.code(0);
    let decision = decision.stdout.lines().next().unwrap().to_owned();
    let report = pair.spec_piped(
        &pair.linked,
        &[
            "propose",
            "discrepancy",
            "--input",
            "-",
            "--author-role",
            "developer",
        ],
        json!({"node_ids": ["EDGE-SPRINT-EMPTY"], "summary": "The sprint ends late.",
            "gap_type": "partial", "severity": "low",
            "evidence": [{"file": "src/a.rs", "observed": "o", "documented": "d"}],
            "options": [{"label": "a", "effect": "e", "price": "p"},
                {"label": "b", "effect": "e", "price": "p"}], "recommendation": 1})
        .to_string()
        .as_bytes(),
    );
    report.code(0);
    let report = report.stdout.lines().next().unwrap().to_owned();
    let before = pair.state();
    let events = pair.events().len();

    let (outcome, questions) = pair.decide(&pair.main, &priced, &option(2), None, true);
    assert_eq!(
        refused(&outcome, "option 2"),
        format!("--option 2: `{priced}` has options 0-1; nothing changed")
    );
    assert!(questions.is_empty());
    // Iteration 2: both flags on a question that takes either.
    let (outcome, questions) = pair.decide(
        &pair.main,
        &priced,
        &flags(Some(1), Some("yes"), None),
        None,
        true,
    );
    assert_eq!(
        cannot(&outcome, "option and answer"),
        format!(
            "spec: `--option` and `--answer` each name the owner's choice, and `{priced}` takes \
             one: give either; nothing changed"
        )
    );
    assert!(questions.is_empty());
    for (id, kind) in [(&bare, "a question"), (&decision, "a decision")] {
        let (outcome, questions) = pair.decide(&pair.main, id, &option(0), None, true);
        assert_eq!(
            cannot(&outcome, id),
            format!(
                "spec: `--option` names an option, and `{id}` is {kind} with none: its working \
                 answer, or `--answer T`, answers it; nothing changed"
            )
        );
        assert!(questions.is_empty());
    }
    let (outcome, _) = pair.decide(&pair.main, &report, &flags(None, None, None), None, true);
    assert_eq!(
        cannot(&outcome, &report),
        format!(
            "spec: `{report}` is a discrepancy: name the owner's choice with `--option N` (0-1); \
             nothing changed"
        )
    );
    let (outcome, _) = pair.decide(
        &pair.main,
        &report,
        &flags(None, Some("yes"), None),
        None,
        true,
    );
    assert_eq!(
        cannot(&outcome, &report),
        format!(
            "spec: `--answer` answers a question, and `{report}` is a discrepancy: name the \
             owner's choice with `--option N` (0-1); nothing changed"
        )
    );
    assert_eq!(pair.state(), before, "nothing written");
    assert_eq!(pair.events().len(), events, "no event");
    // `spec approve --help` names a question's options.
    let help = pair.spec_piped(&pair.linked, &["approve", "--help"], b"");
    help.code(0);
    assert!(
        help.stdout.contains(
            "A discrepancy or a question with options: the owner's chosen option, from 0"
        ),
        "{}",
        help.stdout
    );
}

// ------------------------------------------------------------------ PQ6

/// `--json` answer of the example with `gist` and the given extras, a new
/// text each time (no queue hit), stdin the example's options.
fn noted(pair: &Pair, text: &str, gist: &str, extra: &[&str]) -> Value {
    let mut args = with_flag("--gist", Some(gist));
    // `with_flag` names DEC-0023; only `extra` may.
    args.truncate(args.len() - 2);
    args[3] = text;
    args.extend_from_slice(extra);
    args.push("--json");
    let run = ask(pair, &args, OPTIONS.as_bytes());
    run.code(0);
    run.json()
}

/// The note naming `token`.
fn note(token: &str) -> String {
    format!(
        "gist: `{token}` reads as code to the owner: say it in plain words; IDs belong in \
         node_ids"
    )
}

/// PQ6: the note names the first flagged token, once, last; none for plain
/// words or an unresolvable ID; the item stores either way. M: the note
/// refusing.
#[test]
fn pq6_the_note_names_the_first_token_that_reads_as_code() {
    let pair = Pair::new("pq-pq6-note", "spec-a");
    let cases: [(&str, &str, Option<&str>); 9] = [
        (
            "Does the stamina wait restart?",
            "Does MEC-STAMINA restart its wait?",
            Some("MEC-STAMINA"),
        ),
        (
            "Does the doc say rest restarts it?",
            "Does docs/spec/x.md say rest restarts it?",
            Some("docs/spec/x.md"),
        ),
        (
            "Does a tick restart it?",
            "Does stamina_regen::tick restart the wait?",
            Some("stamina_regen::tick"),
        ),
        (
            "Does an upper-case path count?",
            "Is README.MD the place?",
            Some("README.MD"),
        ),
        (
            "Does a slug-qualified ID read as code?",
            "Does stamina-tuning/AC-07 restart it?",
            Some("stamina-tuning/AC-07"),
        ),
        (
            "Do two codes give one note?",
            "Does docs/spec/x.md agree with MEC-STAMINA?",
            Some("docs/spec/x.md"),
        ),
        (
            "Does stamina refill at rest?",
            "Does stamina refill at rest?",
            None,
        ),
        (
            "Does an unknown ID read as code?",
            "Does ZZ-99 refill at rest?",
            None,
        ),
        (
            "Does a look-alike read as code?",
            "Does MEC-STAMIN\u{410} refill at rest?",
            None,
        ),
    ];
    for (index, (text, gist, flagged)) in cases.iter().enumerate() {
        let json = noted(&pair, text, gist, &["--distinct-from", "DEC-0023"]);
        assert_eq!(json["created"], json!(true), "{gist}: {json}");
        let notes = json["notes"].as_array().expect("notes").clone();
        let ours: Vec<&Value> = notes
            .iter()
            .filter(|note| note.as_str().is_some_and(|text| text.starts_with("gist: ")))
            .collect();
        match flagged {
            Some(token) => {
                assert_eq!(ours, [&json!(note(token))], "{gist}: one note");
                assert_eq!(notes.last(), Some(&json!(note(token))), "{gist}: last");
            }
            None => assert!(ours.is_empty(), "{gist}: {notes:?}"),
        }
        // Stored as given: the gist never rewritten.
        let id = format!("PR-{:04}", index + 1);
        assert_eq!(json["id"], json!(id));
        assert_eq!(review_json(&pair, &id)["gist"], json!(gist));
    }
    // Rule 2 trims punctuation before the `.md` test: one note, naming the
    // path.
    let json = noted(
        &pair,
        "Is the rule in the doc?",
        "Is the rule in (docs/spec/x.md)?",
        &["--distinct-from", "DEC-0023"],
    );
    let ours: Vec<String> = json["notes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .filter(|note| note.starts_with("gist: "))
        .map(str::to_owned)
        .collect();
    assert_eq!(ours.len(), 1, "{json}");
    assert!(ours[0].contains("docs/spec/x.md"), "{ours:?}");
    // Not stored (DEC-0023 hits): the note still comes, after the hit's.
    let json = noted(
        &pair,
        "Does MEC-STAMINA wait at all?",
        "Does MEC-STAMINA wait at all?",
        &[],
    );
    assert_eq!(json["created"], json!(false));
    assert_eq!(
        json["notes"].as_array().unwrap().last(),
        Some(&json!(note("MEC-STAMINA")))
    );
    let text_args = {
        let mut args = with_flag("--gist", Some("Does MEC-STAMINA wait at all?"));
        args.retain(|arg| *arg != "--distinct-from" && *arg != "DEC-0023");
        args[3] = "Does MEC-STAMINA wait at all?";
        args
    };
    // In text the note goes to stderr as notes do (`note: `), last; the
    // hit and `not stored:` to stdout.
    let run = ask(&pair, &text_args, OPTIONS.as_bytes());
    run.code(0);
    assert!(
        run.stderr
            .ends_with(&format!("note: {}\n", note("MEC-STAMINA"))),
        "{}",
        run.show()
    );
    assert!(
        run.stdout.starts_with(&format!("{HIT_LINE}\n")),
        "{}",
        run.show()
    );
    assert!(
        run.stdout
            .ends_with("not stored: name every hit in `distinct_from` to store it anyway\n"),
        "{}",
        run.show()
    );
    assert!(!run.stdout.contains("reads as code"), "{}", run.show());
    // Never on a refusal: the refusal's line is the one note.
    let mut refused_args = with_flag("--gist", Some("Does MEC-STAMINA wait?"));
    refused_args.push("--json");
    let run = ask(&pair, &refused_args, options_of(4).as_bytes());
    assert_eq!(run.code, 1);
    assert_eq!(
        run.json()["notes"],
        json!(["options: 4 option(s); give 2 to 3, each priced"])
    );
}

/// PQ6, "The plain-words note" rule 3 (the orchestrator's ruling of
/// 2026-11-12: "an ID, alias or `slug/ID`" flags an alias): spec-a's alias
/// `TERM-tired` (of `TERM-exhausted`) and its legacy ID `QST-031` (of
/// `Q-031`) read as code, the note naming each as given after the trim
/// (`QST-031?` named `QST-031`); a look-alike of an alias gives none.
#[test]
fn pq6_an_alias_in_the_gist_reads_as_code() {
    let pair = Pair::new("pq-pq6-alias", "spec-a");
    for (text, gist, flagged) in [
        (
            "Does an alias read as code?",
            "Is TERM-tired the word?",
            Some("TERM-tired"),
        ),
        (
            "Does a legacy ID read as code?",
            "Is it settled by QST-031?",
            Some("QST-031"),
        ),
        (
            "Does a look-alike alias read as code?",
            "Is TERM-tir\u{435}d the word?",
            None,
        ),
        (
            "Does a record ID read as code?",
            "Is Q-031 right?",
            Some("Q-031"),
        ),
        (
            "Does a bracketed ID read as code?",
            "Is (TERM-exhausted) right?",
            Some("TERM-exhausted"),
        ),
        (
            "Does an unknown legacy ID read as code?",
            "Is QST-999 right?",
            None,
        ),
        (
            "Does an unknown ID among words read as code?",
            "Is ZZ-99 or stamina right?",
            None,
        ),
        (
            "Does a project-prefixed ID read as code?",
            "Is project:MEC-STAMINA right?",
            None,
        ),
    ] {
        let json = noted(&pair, text, gist, &["--distinct-from", "DEC-0023"]);
        assert_eq!(json["created"], json!(true), "{gist}: {json}");
        let ours: Vec<&Value> = json["notes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|note| note.as_str().is_some_and(|text| text.starts_with("gist: ")))
            .collect();
        match flagged {
            Some(token) => assert_eq!(ours, [&json!(note(token))], "{gist}: {json}"),
            None => assert!(ours.is_empty(), "{gist}: {json}"),
        }
    }
}

/// PQ6: agent text is data: a gist, a `matters` and labels holding
/// `MEC-STAMINA`, `docs/spec/x.md`, `a::b`, and the gist and `matters`
/// U+202E and (in `matters`) CRLF, round-trip byte-equal through JSON, and
/// the review's and the inbox's text escape the controls. M: IDs stripped
/// from the stored gist. An option's fields carry no control since
/// docs/features/release-hardening.md "Acceptance criteria" RH-R1 refuses
/// it at intake (a record never carries one); the gist and `matters` keep
/// their own rules.
#[test]
fn pq6_the_fields_are_verbatim_in_json_and_escaped_in_text() {
    let pair = Pair::new("pq-pq6-verbatim", "spec-a");
    let gist = "Does MEC-STAMINA in docs/spec/x.md call a::b \u{202e}now?";
    let matters = "Sprint\u{202e} tuning\r\nand docs/spec/x.md wait on MEC-STAMINA.";
    let labels = ["MEC-STAMINA a::b", "docs/spec/x.md"];
    let options = json!([{"label": labels[0], "effect": "e", "price": "p"},
        {"label": labels[1], "effect": "e", "price": "p"}]);
    let mut args = with_flag("--gist", Some(gist));
    let at = args.iter().position(|arg| *arg == "--matters").unwrap();
    args[at + 1] = matters;
    args.push("--json");
    let run = ask(&pair, &args, options.to_string().as_bytes());
    run.code(0);
    assert_eq!(run.json()["id"], json!("PR-0001"));
    let json = review_json(&pair, "PR-0001");
    assert_eq!(json["gist"], json!(gist));
    assert_eq!(json["matters"], json!(matters));
    assert_eq!(json["options"], options);
    assert_eq!(
        columns(&pair, "PR-0001", &["hex(gist)", "hex(matters)"]),
        format!("{}|{}\n", hex(gist.as_bytes()), hex(matters.as_bytes()))
    );
    let text = review_text(&pair, "PR-0001");
    assert!(
        !text.contains('\u{202e}') && !text.contains('\r'),
        "no raw control: {text:?}"
    );
    assert!(
        text.contains("\n  [0] MEC-STAMINA a::b | e | p (recommended)\n"),
        "{text}"
    );
    assert!(
        text.contains("\ngist: Does MEC-STAMINA in docs/spec/x.md call a::b \\u{202e}now?\n"),
        "{text}"
    );
    assert!(
        // One line in text, as `summary:` and `working_answer:` are: the line
        // break folded to a space, U+202E escaped.
        text.contains(
            "\nmatters: Sprint\\u{202e} tuning and docs/spec/x.md wait on MEC-STAMINA.\n"
        ),
        "{text}"
    );
    let inbox = spec_ok(&pair, &["inbox"]);
    assert!(!inbox.contains('\u{202e}'), "{inbox:?}");
    assert!(
        inbox.ends_with(" | normal: Does MEC-STAMINA in docs/spec/x.md call a::b \\u{202e}now?\n"),
        "{inbox}"
    );
}

/// Upper-case hexadecimal of `bytes`, as SQLite's `hex()` prints it.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}

// ------------------------------------------------------------------ PQ7

/// The owner's terminal of a library round: every question answered with
/// the end of input (the stages kept, nothing applied).
#[derive(Default)]
struct Silent {
    reports: Vec<String>,
}

impl RoundTerminal for Silent {
    fn report(&mut self, text: &str) {
        self.reports.push(text.to_owned());
    }

    fn ask(&mut self, _question: &str) -> Option<bool> {
        None
    }

    fn tell(&mut self, _line: &str) {}
}

fn env(pair: &Pair) -> Env {
    pair.env(&pair.linked)
}

/// `spec round new`'s text and JSON (parsed and as printed), from `t1`.
fn sheet(pair: &Pair) -> (String, Value, String) {
    let sheet = round_new(
        &env(pair),
        &Globals::default(),
        &RoundNewRequest {
            limit: None,
            git: pair.git_env(&pair.linked),
        },
    )
    .unwrap_or_else(|error| panic!("round new: {error}"));
    let outcome = Outcome::RoundSheet(Box::new(sheet));
    let json = render_json(&outcome);
    (render_text(&outcome), json_of(&json), json)
}

/// `spec round answer FILE` (the file `project: lantern-keep` then `line`)
/// from `t1` on a silent terminal: its report.
fn answer(pair: &Pair, name: &str, line: &str) -> String {
    let file = pair.scratch.dir("answers").join(name);
    fs::write(&file, format!("project: lantern-keep\n{line}\n")).unwrap();
    let mut terminal = Silent::default();
    round_answer(
        &env(pair),
        &Globals::default(),
        &RoundAnswerRequest {
            file: Some(file),
            now: utc_now(),
            git: pair.git_env(&pair.linked),
        },
        &mut terminal,
    )
    .unwrap_or_else(|error| panic!("round answer: {error}"));
    assert_eq!(terminal.reports.len(), 1, "{:?}", terminal.reports);
    terminal.reports.remove(0)
}

fn staged(pair: &Pair, id: &str) -> Option<Stage> {
    pair.proposal(id).staged.map(|staged| staged.stage)
}

fn approve_stage(option: Option<u64>, answer: Option<&str>) -> Stage {
    Stage::Approve {
        option,
        answer: answer.map(str::to_owned),
        canon: None,
        editorial: false,
        note: None,
        span_hash: None,
    }
}

/// The lines from `## <id> - ` through `<id>:`.
fn block<'a>(text: &'a str, id: &str) -> &'a str {
    let start = text
        .find(&format!("## {id} - "))
        .unwrap_or_else(|| panic!("no {id} in:\n{text}"));
    let slot = format!("\n{id}:\n");
    let end = text[start..].find(&slot).expect("its slot") + start + slot.len();
    &text[start..end]
}

/// PQ7: the sheet prints the "Data" block for `PR-0001` (its ID for
/// `PR-0007`), an option-less question as before; the JSON item 21 keys;
/// `1` stages option 1, `ok` the working answer, `no` `--answer no`; an
/// option-less question refuses `1`. M: the sheet refusing a number on a
/// question.
#[test]
fn pq7_the_sheet_leads_with_the_gist_and_takes_a_number() {
    let pair = Pair::new("pq-pq7", "spec-a");
    let id = example(&pair);
    let bare = stored(
        &pair,
        &[
            "EDGE-STAM-ZERO",
            "--text",
            "Does the lamp stay lit?",
            "--working-answer",
            "yes",
            "--price-of-other",
            "a rule",
            "--author-role",
            "developer",
        ],
        b"",
    );
    let (text, json, raw) = sheet(&pair);
    assert_eq!(
        block(&text, &id),
        format!(
            "## {id} - question, normal\n\nGist: {GIST}\n\nWhy it matters:\n  {MATTERS}\n\n\
             As asked:\n  {TEXT}\n\nOptions:\n  - [0] Restart the wait | Short rests do not \
             refill | none: as built (recommended)\n  - [1] Keep the wait running | Tapping rest \
             refills faster | R-28 rebalanced\n\nWorking answer:\n  {WORKING}\n\nAnother answer \
             costs:\n  {PRICE}\n\nConcerns: Stamina (MEC-STAMINA); Regeneration \
             (RULE-STAM-REGEN)\n\n{id}:\n"
        )
    );
    assert_eq!(
        block(&text, &bare),
        format!(
            "## {bare} - question, normal\n\n  Does the lamp stay lit?\n\nWorking answer:\n  \
             yes\n\nAnother answer costs:\n  a rule\n\nConcerns: Depletion (EDGE-STAM-ZERO)\n\n\
             {bare}:\n"
        ),
        "no gist: as before"
    );
    let items = json["items"].as_array().expect("items");
    let raw = Ordered::parse(&raw);
    assert_eq!(raw.get("items").items().len(), 2);
    for item in raw.get("items").items() {
        assert_eq!(item.keys(), ROUND_ITEM_KEYS, "{item:?}");
    }
    let item = items.iter().find(|item| item["id"] == json!(id)).unwrap();
    assert_eq!(item["gist"], json!(GIST));
    assert_eq!(item["matters"], json!(MATTERS));
    assert_eq!(item["recommendation"], json!(0));
    assert_eq!(
        item["options"],
        serde_json::from_str::<Value>(OPTIONS).unwrap()
    );
    let other = items.iter().find(|item| item["id"] == json!(bare)).unwrap();
    assert_eq!(
        (&other["gist"], &other["matters"]),
        (&Value::Null, &Value::Null)
    );

    let report = answer(&pair, "one.md", &format!("{id}: 1"));
    assert!(report.contains(&format!("{id} line 2: staged")), "{report}");
    assert_eq!(staged(&pair, &id), Some(approve_stage(Some(1), None)));
    answer(&pair, "ok.md", &format!("{id}: ok"));
    assert_eq!(staged(&pair, &id), Some(approve_stage(None, None)));
    answer(&pair, "no.md", &format!("{id}: no"));
    assert_eq!(staged(&pair, &id), Some(approve_stage(None, Some("no"))));
    let report = answer(&pair, "five.md", &format!("{id}: 5"));
    assert!(
        report.contains(&format!(
            "refused: option 5: `{id}` has options 0-1; nothing staged"
        )),
        "{report}"
    );
    assert_eq!(
        staged(&pair, &id),
        Some(approve_stage(None, Some("no"))),
        "kept"
    );

    let report = answer(&pair, "bare.md", &format!("{bare}: 1"));
    assert!(
        report.contains(&format!(
            "refused: `{bare}` has no options: `ok` takes its working answer, other text is your \
             answer; nothing staged"
        )),
        "{report}"
    );
    assert_eq!(staged(&pair, &bare), None);
}

// ------------------------------------------------------------------ PQ8

fn env_at(home: &Path, cwd: &Path) -> Env {
    Env {
        cwd: cwd.to_path_buf(),
        home: Some(home.as_os_str().to_owned()),
        xdg_data_home: None,
    }
}

/// `spec export state --out <file>` with `home`'s queue, from `t1`: the
/// dump's lines.
fn export(pair: &Pair, home: &Path, file: &Path) -> Vec<String> {
    export_state(
        &env_at(home, &pair.linked),
        &Globals::default(),
        &ExportStateRequest {
            out: Some(file.to_path_buf()),
            now: utc_now(),
            git: pair.git_env(&pair.linked),
        },
    )
    .unwrap_or_else(|error| panic!("export state: {error}"));
    let text = fs::read_to_string(file).expect("the dump");
    text.strip_suffix('\n')
        .expect("ends with LF")
        .split('\n')
        .map(str::to_owned)
        .collect()
}

/// `spec import-state <file>` into `home`'s queue, consent yes: `Ok` with
/// the questions asked, else the exit-2 message.
fn import(pair: &Pair, home: &Path, file: &Path) -> Result<usize, String> {
    let mut asked = 0;
    let mut consent = |_: &str| {
        asked += 1;
        true
    };
    let outcome = import_state(
        &env_at(home, &pair.linked),
        &Globals::default(),
        &ImportStateRequest {
            file: file.to_path_buf(),
        },
        &mut consent,
    );
    match outcome {
        Ok(outcome) => {
            assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
            Ok(asked)
        }
        Err(error) => {
            assert_eq!(error.exit, Exit::CannotRun, "{error}");
            Err(error.message)
        }
    }
}

/// A queue holding the example, an option-less question, a decision with a
/// `why`, a discrepancy and a card's question.
fn mixed_queue(pair: &Pair) {
    example(pair);
    stored(
        pair,
        &[
            "EDGE-STAM-ZERO",
            "--text",
            "Does the lamp stay lit?",
            "--working-answer",
            "yes",
            "--price-of-other",
            "a rule",
            "--author-role",
            "developer",
        ],
        b"",
    );
    for (kind, document) in [
        (
            "decision",
            json!({"node_ids": ["RULE-SPRINT-COST"], "text": "Which oil burns?",
                "working_answer": "Whale oil.", "price_of_other": "One scene.",
                "why": "It burns longest."}),
        ),
        (
            "discrepancy",
            json!({"node_ids": ["EDGE-SPRINT-EMPTY"], "summary": "The sprint ends late.",
                "gap_type": "partial", "severity": "low",
                "evidence": [{"file": "src/a.rs", "observed": "o", "documented": "d"}],
                "options": [{"label": "a", "effect": "e", "price": "p"},
                    {"label": "b", "effect": "e", "price": "p"}], "recommendation": 1}),
        ),
    ] {
        let run = pair.spec_piped(
            &pair.linked,
            &[
                "propose",
                kind,
                "--input",
                "-",
                "--author-role",
                "developer",
            ],
            document.to_string().as_bytes(),
        );
        run.code(0);
    }
    spec_ok(pair, &["card", "new", "--title", "Lamp room"]);
    let mut args = vec![
        "MEC-SPRINT",
        "--text",
        "Is the lamp lit at dawn?",
        "--card",
        "C-0001",
        "--gist",
        "Is the lamp lit at dawn?",
    ];
    args.extend([
        "--working-answer",
        "yes",
        "--price-of-other",
        "a scene",
        "--author-role",
        "developer",
    ]);
    stored(pair, &args, b"");
}

/// `line`, a `{"proposals":{…}}` row, without its last two columns: the
/// row a format 6 dump holds; asserts they are `gist` and `matters`.
fn without_plain_columns(line: &str) -> String {
    let at = line
        .rfind(",\"gist\":")
        .unwrap_or_else(|| panic!("a gist column: {line}"));
    assert!(line[at..].contains(",\"matters\":"), "{line}");
    format!("{}}}}}", &line[..at])
}

/// PQ8: `spec export state` writes format 7, schema 13, 50 columns, `gist`
/// and `matters` last; export, import fresh and export again byte-equal; a
/// format 6 dump imports both `NULL` and re-exports as format 7 equal but
/// for the header and the two nulls. M: the format 6 import path dropped.
#[test]
fn pq8_format_7_carries_the_two_columns_and_format_6_imports_them_null() {
    assert_eq!(STATE_FORMAT, 7);
    assert_eq!(QUEUE_SCHEMA_VERSION, 14);
    assert_eq!(PROPOSAL_COLUMNS.len(), 50);
    let pair = Pair::new("pq-pq8", "spec-a");
    mixed_queue(&pair);
    let dumps = pair.scratch.dir("dumps");
    let lines = export(&pair, &pair.home, &dumps.join("seven.jsonl"));
    assert!(
        lines[0].starts_with("{\"format\":7,\"queue_schema\":14,\"project\":\"lantern-keep\","),
        "{}",
        lines[0]
    );
    assert_eq!(
        Ordered::parse(&lines[0]).keys(),
        [
            "format",
            "queue_schema",
            "project",
            "proposals",
            "tasks",
            "runs",
            "cards",
            "events"
        ]
    );
    let rows: Vec<usize> = (0..lines.len())
        .filter(|index| lines[*index].starts_with("{\"proposals\":"))
        .collect();
    assert_eq!(rows.len(), 5);
    for index in &rows {
        assert_eq!(
            Ordered::parse(&lines[*index]).get("proposals").keys(),
            PROPOSAL_COLUMNS,
            "{}",
            lines[*index]
        );
    }
    let first: Value = serde_json::from_str(&lines[rows[0]]).unwrap();
    assert_eq!(first["proposals"]["gist"], json!(GIST));
    assert_eq!(first["proposals"]["matters"], json!(MATTERS));
    assert!(lines[rows[0]].ends_with(&format!(
        ",\"gist\":{},\"matters\":{}}}}}",
        json!(GIST),
        json!(MATTERS)
    )));
    let decision: Value = serde_json::from_str(&lines[rows[2]]).unwrap();
    assert_eq!(decision["proposals"]["kind"], json!("decision"));
    assert_eq!(decision["proposals"]["gist"], Value::Null);

    // Seven round-trips byte-equal.
    let fresh = pair.scratch.home("fresh");
    assert_eq!(import(&pair, &fresh, &dumps.join("seven.jsonl")), Ok(1));
    let again = export(&pair, &fresh, &dumps.join("again.jsonl"));
    assert_eq!(again, lines, "byte-identical");

    // A format 6 dump: the header 6/12, rows of 48 columns (the example's
    // gist and matters out, as schema 12 held none).
    let mut six = lines.clone();
    six[0] = six[0].replacen(
        "\"format\":7,\"queue_schema\":14,",
        "\"format\":6,\"queue_schema\":12,",
        1,
    );
    for index in &rows {
        six[*index] = without_plain_columns(&lines[*index]);
        assert_eq!(
            Ordered::parse(&six[*index]).get("proposals").keys(),
            PROPOSAL_COLUMNS[..48],
            "{}",
            six[*index]
        );
    }
    let six_file = dumps.join("six.jsonl");
    fs::write(&six_file, format!("{}\n", six.join("\n"))).unwrap();
    let older = pair.scratch.home("older");
    assert_eq!(import(&pair, &older, &six_file), Ok(1));
    let db = data_dir(&older).join("lantern-keep.db");
    let rows_sql = std::process::Command::new(common::proposal::sqlite3())
        .arg(&db)
        .arg("SELECT count(*) FROM proposals WHERE gist IS NULL AND matters IS NULL;")
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&rows_sql.stdout), "5\n");
    let reexported = export(&pair, &older, &dumps.join("six-again.jsonl"));
    assert_eq!(reexported[0], lines[0], "the header format 7, schema 14");
    for index in 1..lines.len() {
        let want = if rows.contains(&index) {
            format!(
                "{},\"gist\":null,\"matters\":null}}}}",
                &six[index][..six[index].len() - 2]
            )
        } else {
            lines[index].clone()
        };
        assert_eq!(reexported[index], want, "line {}", index + 1);
    }
}

/// PQ8: refused at the line, exit 2, nothing written, no prompt: a format 7
/// header at schema 12 (naming 13), a format 7 row without `matters`, a
/// format 6 row with `gist`, format 8; a queue at schema 14 is refused by
/// `spec inbox` naming both schemas.
#[test]
fn pq8_a_mismatched_dump_or_a_newer_queue_is_refused() {
    let pair = Pair::new("pq-pq8-refused", "spec-a");
    mixed_queue(&pair);
    let dumps = pair.scratch.dir("dumps");
    let lines = export(&pair, &pair.home, &dumps.join("seven.jsonl"));
    let row = (0..lines.len())
        .find(|index| lines[*index].starts_with("{\"proposals\":"))
        .unwrap();
    let mut header_12 = lines.clone();
    header_12[0] = header_12[0].replacen("\"queue_schema\":14,", "\"queue_schema\":12,", 1);
    let mut no_matters = lines.clone();
    let at = no_matters[row].rfind(",\"matters\":").unwrap();
    no_matters[row] = format!("{}}}}}", &no_matters[row][..at]);
    let mut six_with_gist = lines.clone();
    six_with_gist[0] = six_with_gist[0].replacen(
        "\"format\":7,\"queue_schema\":14,",
        "\"format\":6,\"queue_schema\":12,",
        1,
    );
    for (index, line) in lines.iter().enumerate().skip(1) {
        if line.starts_with("{\"proposals\":") && index != row {
            six_with_gist[index] = without_plain_columns(line);
        }
    }
    let mut eight = lines.clone();
    eight[0] = eight[0].replacen("\"format\":7,", "\"format\":8,", 1);
    for (name, bad, line, says) in [
        (
            "header-12",
            header_12,
            1,
            "(it holds queue schemas 13 to 14)",
        ),
        ("no-matters", no_matters, row + 1, "matters"),
        ("six-with-gist", six_with_gist, row + 1, "gist"),
        ("eight", eight, 1, "upgrade SpecEngine"),
    ] {
        let file = dumps.join(format!("{name}.jsonl"));
        fs::write(&file, format!("{}\n", bad.join("\n"))).unwrap();
        let fresh: PathBuf = pair.scratch.home(&format!("fresh-{name}"));
        let message = import(&pair, &fresh, &file).expect_err(name);
        assert!(
            message.starts_with(&format!("{}:{line}: ", file.display())),
            "{name}: {message}"
        );
        assert!(message.contains(says), "{name}: {message}");
        assert!(!data_dir(&fresh).exists(), "{name}: nothing written");
    }

    // A queue a newer build stepped: refused, left at 14.
    pair.sql("PRAGMA user_version = 15;");
    let run = pair.spec_piped(&pair.linked, &["inbox"], b"");
    assert_eq!(run.code, 2, "{}", run.show());
    assert!(
        run.stderr
            .contains("schema version 15, this build knows 14"),
        "{}",
        run.stderr
    );
    assert_eq!(pair.sql("PRAGMA user_version;").trim(), "15");
}

// ----------------------------------------------------------------- PQ10

/// PQ10, the CLI's half: `spec inbox` entries 13 keys with `gist` after
/// `summary`, the text's last column the gist else the summary's first
/// line; `spec card show` items 18 keys with `gist` and `matters`, its
/// `items:` line the gist else the summary.
#[test]
fn pq10_the_inbox_and_a_cards_items_carry_the_gist() {
    let pair = Pair::new("pq-pq10", "spec-a");
    let id = example(&pair);
    spec_ok(&pair, &["card", "new", "--title", "Lamp room"]);
    let card_item = stored(
        &pair,
        &[
            "MEC-SPRINT",
            "--text",
            "Does a sprint\nrestart the wait?",
            "--working-answer",
            "yes",
            "--price-of-other",
            "a scene",
            "--gist",
            "Should a sprint restart the wait?",
            "--matters",
            "One task waits.",
            "--card",
            "C-0001",
            "--author-role",
            "developer",
        ],
        b"",
    );
    let bare = stored(
        &pair,
        &[
            "EDGE-STAM-ZERO",
            "--text",
            "Does the lamp\nstay lit?",
            "--working-answer",
            "yes",
            "--price-of-other",
            "a rule",
            "--card",
            "C-0001",
            "--author-role",
            "developer",
        ],
        b"",
    );
    let raw = spec_ok(&pair, &["inbox", "--json"]);
    let json = json_of(&raw);
    let entries = json["proposals"].as_array().expect("entries");
    assert_eq!(entries.len(), 3);
    for entry in Ordered::parse(&raw).get("proposals").items() {
        assert_eq!(entry.keys(), INBOX_KEYS, "{entry:?}");
    }
    assert_eq!(entries[0]["gist"], json!(GIST));
    assert_eq!(entries[2]["gist"], Value::Null);
    let text = spec_ok(&pair, &["inbox"]);
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].starts_with(&format!("{id} | question | open | MEC-STAMINA | t1 | ")));
    assert!(lines[0].ends_with(&format!(" | normal: {GIST}")), "{text}");
    assert!(
        lines[1].ends_with(" | normal: Should a sprint restart the wait?"),
        "{text}"
    );
    assert!(lines[2].ends_with(" | normal: Does the lamp"), "{text}");

    let raw = spec_ok(&pair, &["card", "show", "C-0001", "--json"]);
    let card = json_of(&raw);
    let items = card["items"].as_array().expect("items");
    assert_eq!(items.len(), 2);
    for item in Ordered::parse(&raw).get("items").items() {
        assert_eq!(item.keys(), CARD_ITEM_KEYS, "{item:?}");
    }
    assert_eq!(items[0]["id"], json!(card_item));
    assert_eq!(items[0]["gist"], json!("Should a sprint restart the wait?"));
    assert_eq!(items[0]["matters"], json!("One task waits."));
    assert_eq!(
        (&items[1]["gist"], &items[1]["matters"]),
        (&Value::Null, &Value::Null)
    );
    let shown = spec_ok(&pair, &["card", "show", "C-0001"]);
    let line = shown
        .lines()
        .find(|line| line.starts_with("items: "))
        .expect("an items line");
    assert!(
        line.starts_with(&format!(
            "items: {card_item} | question | open | Should a sprint restart the wait?"
        )),
        "{shown}"
    );
    assert!(
        shown.contains(&format!("{bare} | question | open | Does the lamp")),
        "{shown}"
    );
}
