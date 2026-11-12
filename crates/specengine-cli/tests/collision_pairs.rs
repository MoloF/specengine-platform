//! docs/features/collision-pairs.md (RM-0088, the pair half)
//! "Acceptance criteria", the CLI's half: the `spec` binary over scratch git copies of
//! `fixtures/spec-a` (`common::proposal::Pair`: a main worktree, a linked
//! one on `t1`, a scratch `HOME`), every item raised and read in `t1`.
//!
//! "The example" as the spec writes it is not stored on spec-a: its target
//! `RULE-STAM-REGEN` is answered by the accepted DEC-0023, a hit. Stored, it
//! names that hit (`distinct_from`), as every item here names its hits.
//!
//! - PP1 `pp1_*`: the example's `spec review PR --json` and `--brief --json`
//!   hold the spec's `pair`, right after `evidence`, in 54 keys; the
//!   `spec round new --json` item the same object, after `evidence`, in 22
//!   keys. M: evidence grouped by position, not `file` (an item whose
//!   evidence lists stamina before sprint); `left` and `right` swapped.
//! - PP2 `pp2_*`: `pair` is `null` with a third target (`MEC-SPRINT`), one
//!   target, `gap_type` `missing`, for a question on the two targets and for
//!   an update, in the review and the round alike. M: served for any
//!   two-target discrepancy.
//! - PP3 `pp3_*`: `MEC-STAMINA` and `RULE-STAM-REGEN` share `stamina.md`:
//!   evidence 1 under both, `other` `[0, 2]`; the example after
//!   `RULE-STAM-REGEN`'s section is removed: `right` its ID, `title` and
//!   `path` null, no evidence, `other` `[1, 2]`; after `RULE-SPRINT-COST`'s:
//!   `left` keeps the stored `target_path` and its evidence. M: `null` for a
//!   gone side; a shared file's evidence under `left` alone.
//! - PP4 `pp4_*`: retitling `RULE-SPRINT-COST` "Sprint cost" shows on the
//!   next review; the queue's dump and last event `seq` unchanged by it;
//!   `QUEUE_SCHEMA_VERSION` 14 and `STATE_FORMAT` 7, `collision-pass`'s. M:
//!   the title taken from the row stored at intake.
//! - PP5 `pp5_*`: `spec review PR` prints the four lines right after the
//!   evidence lines, before `options:`; a null pair's text has no `pair`
//!   line, and the example's text without its four lines is, line for line,
//!   its `missing` twin's but for the ID, `gap_type` and the clock. M: a
//!   `pair: null` line printed.
//!
//! - The orchestrator's two rulings, pinned (recorded at ship):
//!   `acts_*`: a propose (an update's review document) answers `pair`
//!   null, an approve and a reject of a pair item answer `pair` null, and
//!   the decided items' reviews still serve their pair (D1: any status);
//!   `unknown_*`: an index that cannot be read (its database read-only
//!   under a changed file) leaves `pair` null with the note `pair unknown:
//!   <error>`, exit 0, never a gone side.
//!
//! The seam of PP6 (target resolutions counted) is the crate's
//! `#[cfg(test)]` `pair_tests.rs`; the MCP and daemon halves of PP1 are
//! `specengine-mcp`'s and `specengine-http`'s `collision_pairs.rs`; the pure
//! rule core's `tests/pair.rs`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::fs;

use common::decision::option;
use common::proposal::{Pair as Repo, json_of, printed};
use common::{Run, data_dir};
use serde_json::{Value, json};
use specengine_cli::STATE_FORMAT;
use specengine_store::QUEUE_SCHEMA_VERSION;

const SPRINT: &str = "docs/spec/movement/sprint.md";
const STAMINA: &str = "docs/spec/movement/stamina.md";

/// The review document's 54 keys: plain-questions' 53 with `pair` after
/// `evidence`.
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

/// A round item's 22 keys: plain-questions' 21 with `pair` after
/// `evidence`.
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

/// The example's evidence, in its order.
fn evidence() -> Value {
    json!([
        {"file": SPRINT, "lines": "18-20", "observed": "Sprinting pauses regeneration",
            "documented": "Cost is drained per second"},
        {"file": STAMINA, "lines": "21-23", "observed": "Regeneration never pauses",
            "documented": "Refill starts after 1.5 s"},
        {"file": "src/stamina.rs", "qpath": "stamina::regen", "lines": "40-52",
            "observed": "regen runs during sprint", "documented": "a pause is specified"}
    ])
}

/// The example's `--input` document over `targets`, `gap_type` `gap`.
fn input(targets: &[&str], gap: &str, summary: &str, evidence: Value) -> Value {
    json!({"node_ids": targets, "gap_type": gap, "severity": "normal", "summary": summary,
        "evidence": evidence,
        "options": [{"label": "Pause while sprinting", "effect": "Sprint drains faster",
                "price": "Regeneration rewritten"},
            {"label": "Never pause", "effect": "Sprint lasts longer",
                "price": "Sprint cost rewritten"}],
        "recommendation": 0})
}

/// The example's summary.
const SUMMARY: &str = "Sprint cost and regeneration disagree on when stamina refills";

/// `spec <args>` in `t1`, stdin `input`, exit 0.
fn spec(repo: &Repo, args: &[&str], input: &[u8]) -> Run {
    let run = repo.spec_piped(&repo.linked, args, input);
    run.code(0);
    run
}

/// `spec propose discrepancy --input -` of `document` in `t1`: stored, its
/// hits named in `distinct_from` when the first try lists any. The new ID.
fn report(repo: &Repo, document: &Value) -> String {
    let args = [
        "propose",
        "discrepancy",
        "--input",
        "-",
        "--author-role",
        "developer",
    ];
    let first = spec(repo, &args, document.to_string().as_bytes());
    if first.stdout.starts_with("PR-") {
        return first.stdout.lines().next().unwrap().to_owned();
    }
    let listed = spec(
        repo,
        &[args.as_slice(), &["--json"]].concat(),
        document.to_string().as_bytes(),
    )
    .json();
    let hits: Vec<Value> = listed["hits"]
        .as_array()
        .unwrap_or_else(|| panic!("hits: {listed}"))
        .iter()
        .map(|hit| hit["id"].clone())
        .collect();
    assert!(!hits.is_empty(), "not stored and no hit: {}", first.show());
    let mut named = document.clone();
    named["distinct_from"] = Value::Array(hits);
    let run = spec(repo, &args, named.to_string().as_bytes());
    let id = run.stdout.lines().next().unwrap_or_default().to_owned();
    assert!(id.starts_with("PR-"), "not stored: {}", run.show());
    id
}

/// The example stored.
fn example(repo: &Repo) -> String {
    report(
        repo,
        &input(
            &["RULE-SPRINT-COST", "RULE-STAM-REGEN"],
            "contradicts",
            SUMMARY,
            evidence(),
        ),
    )
}

/// `spec review ID --json` (plus `extra`), as printed.
fn review_raw(repo: &Repo, id: &str, extra: &[&str]) -> String {
    let mut args = vec!["review", id, "--json"];
    args.extend_from_slice(extra);
    spec(repo, &args, b"").stdout
}

fn review(repo: &Repo, id: &str) -> Value {
    json_of(&review_raw(repo, id, &[]))
}

fn review_text(repo: &Repo, id: &str) -> String {
    spec(repo, &["review", id], b"").stdout
}

/// `spec --json round new` in `t1`, as printed.
fn round_raw(repo: &Repo) -> String {
    spec(repo, &["--json", "round", "new"], b"").stdout
}

/// The round item of `id`.
fn round_item(round: &Value, id: &str) -> Value {
    round["items"]
        .as_array()
        .unwrap_or_else(|| panic!("items: {round}"))
        .iter()
        .find(|item| item["id"] == json!(id))
        .unwrap_or_else(|| panic!("no {id} in {round}"))
        .clone()
}

/// The spec's pair for the example.
fn data_pair() -> Value {
    json!({"left": {"id": "RULE-SPRINT-COST", "title": "Cost", "path": SPRINT, "evidence": [0]},
        "right": {"id": "RULE-STAM-REGEN", "title": "Regeneration", "path": STAMINA,
            "evidence": [1]},
        "other": [2]})
}

/// Removes the section headed `heading` (through the line before the next
/// `## ` heading) from `path` in `t1` and commits it.
fn remove_section(repo: &Repo, path: &str, heading: &str) {
    let file = repo.linked.join(path);
    let text = fs::read_to_string(&file).expect("the document");
    let start = text
        .find(heading)
        .unwrap_or_else(|| panic!("{heading} in {path}"));
    let rest = &text[start + heading.len()..];
    let end = rest
        .find("\n## ")
        .map_or(text.len(), |at| start + heading.len() + at + 1);
    let edited = format!("{}{}", &text[..start], &text[end..]);
    assert!(!edited.contains(heading));
    fs::write(&file, edited).expect("write");
    repo.commit_all(&repo.linked, "a section removed");
}

// ------------------------------------------------------------- ordered keys

/// A JSON value whose objects keep their keys in document order (the
/// tests' `serde_json` keeps none).
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
        serde_json::from_str(text.trim_end()).unwrap_or_else(|error| panic!("{error}: {text}"))
    }

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

/// Asserts a served pair object's keys in order: `left, right, other`, each
/// side `id, title, path, evidence`.
fn assert_pair_keys(pair: &Ordered, context: &str) {
    assert_eq!(pair.keys(), ["left", "right", "other"], "{context}");
    for side in ["left", "right"] {
        assert_eq!(
            pair.get(side).keys(),
            ["id", "title", "path", "evidence"],
            "{context}: {side}"
        );
    }
}

// ------------------------------------------------------------------ PP1

/// PP1: the example's review (`--json`, `--brief --json`) holds the spec's
/// `pair` right after `evidence`, 54 keys; the round's item the same object
/// after `evidence`, 22 keys.
#[test]
fn pp1_the_example_serves_its_pair_in_the_review_and_the_round() {
    let repo = Repo::new("cpair-pp1", "spec-a");
    let id = example(&repo);
    let raw = review_raw(&repo, &id, &[]);
    let ordered = Ordered::parse(&raw);
    assert_eq!(ordered.keys(), REVIEW_KEYS, "{raw}");
    assert_pair_keys(ordered.get("pair"), "the review");
    let document = json_of(&raw);
    assert_eq!(document["pair"], data_pair(), "{raw}");
    let files: Vec<&Value> = document["evidence"]
        .as_array()
        .expect("evidence")
        .iter()
        .map(|item| &item["file"])
        .collect();
    assert_eq!(
        files,
        [&json!(SPRINT), &json!(STAMINA), &json!("src/stamina.rs")],
        "evidence as stored, in order"
    );
    let brief = json_of(&review_raw(&repo, &id, &["--brief"]));
    assert_eq!(brief["pair"], data_pair(), "--brief alike");
    // A repeat is byte-equal.
    assert_eq!(review_raw(&repo, &id, &[]), raw);

    let round_text = round_raw(&repo);
    let round = json_of(&round_text);
    let ordered = Ordered::parse(&round_text);
    let items = ordered.get("items").items();
    assert_eq!(items.len(), 1, "{round_text}");
    assert_eq!(items[0].keys(), ROUND_ITEM_KEYS, "{round_text}");
    assert_pair_keys(items[0].get("pair"), "the round");
    assert_eq!(round_item(&round, &id)["pair"], data_pair());
}

/// PP1: the sides follow `file`, not position: the evidence listed stamina
/// first gives `left [1]`, `right [0]`; the targets stored the other way
/// round swap the sides whole. M: evidence grouped by position; `left` and
/// `right` swapped.
#[test]
fn pp1_the_sides_follow_the_file_and_the_stored_order() {
    let repo = Repo::new("cpair-pp1-order", "spec-a");
    let mut reordered = evidence();
    reordered.as_array_mut().unwrap().swap(0, 1);
    let flipped = report(
        &repo,
        &input(
            &["RULE-SPRINT-COST", "RULE-STAM-REGEN"],
            "contradicts",
            "Stamina refill and sprint cost disagree, the stamina quote first",
            reordered,
        ),
    );
    let pair = &review(&repo, &flipped)["pair"];
    assert_eq!(
        (
            &pair["left"]["evidence"],
            &pair["right"]["evidence"],
            &pair["other"]
        ),
        (&json!([1]), &json!([0]), &json!([2])),
        "{pair}"
    );
    let swapped = report(
        &repo,
        &input(
            &["RULE-STAM-REGEN", "RULE-SPRINT-COST"],
            "contradicts",
            "Regeneration and sprint cost disagree, regeneration named first",
            evidence(),
        ),
    );
    let pair = review(&repo, &swapped)["pair"].clone();
    let want = data_pair();
    assert_eq!(
        pair,
        json!({"left": want["right"], "right": want["left"], "other": [2]}),
        "{pair}"
    );
    let round = json_of(&round_raw(&repo));
    assert_eq!(round_item(&round, &swapped)["pair"], pair);
}

// ------------------------------------------------------------------ PP2

/// PP2: `pair` is `null` for the example with a third target, with one
/// target, with `gap_type` `missing`, for a question on the two targets and
/// for an update, in the review and the round alike. M: served for any
/// two-target discrepancy.
#[test]
fn pp2_every_other_item_serves_null() {
    let repo = Repo::new("cpair-pp2", "spec-a");
    let example = example(&repo);
    let three = report(
        &repo,
        &input(
            &["RULE-SPRINT-COST", "RULE-STAM-REGEN", "MEC-SPRINT"],
            "contradicts",
            "Three documents disagree on when stamina refills",
            evidence(),
        ),
    );
    let one = report(
        &repo,
        &input(
            &["RULE-SPRINT-COST"],
            "contradicts",
            "Sprint cost contradicts the build",
            evidence(),
        ),
    );
    let missing = report(
        &repo,
        &input(
            &["RULE-SPRINT-COST", "RULE-STAM-REGEN"],
            "missing",
            "Neither document says when stamina refills mid-sprint",
            evidence(),
        ),
    );
    let ask = |extra: &[&str]| -> Run {
        let mut args = vec![
            "propose",
            "question",
            "RULE-SPRINT-COST",
            "RULE-STAM-REGEN",
            "--text",
            "Does sprinting pause regeneration?",
            "--working-answer",
            "yes",
            "--price-of-other",
            "a rebalance",
            "--author-role",
            "developer",
        ];
        args.extend_from_slice(extra);
        spec(&repo, &args, b"")
    };
    let first = ask(&[]);
    let question = if first.stdout.starts_with("PR-") {
        first.stdout.lines().next().unwrap().to_owned()
    } else {
        let hits = ask(&["--json"]).json()["hits"].clone();
        let mut named: Vec<String> = Vec::new();
        for hit in hits.as_array().expect("hits") {
            named.push("--distinct-from".to_owned());
            named.push(hit["id"].as_str().expect("an id").to_owned());
        }
        let named: Vec<&str> = named.iter().map(String::as_str).collect();
        let run = ask(&named);
        run.stdout.lines().next().unwrap().to_owned()
    };
    assert!(question.starts_with("PR-"), "{question}");
    let update = repo.propose_edit(
        &repo.linked,
        "RULE-SPRINT-COST",
        "## Cost",
        "## Sprint price",
    );
    let round = json_of(&round_raw(&repo));
    for (label, id) in [
        ("three targets", &three),
        ("one target", &one),
        ("missing", &missing),
        ("a question", &question),
        ("an update", &update),
    ] {
        let raw = review_raw(&repo, id, &[]);
        let ordered = Ordered::parse(&raw);
        assert_eq!(ordered.keys(), REVIEW_KEYS, "{label}: {raw}");
        assert_eq!(json_of(&raw)["pair"], Value::Null, "{label}");
        let brief = json_of(&review_raw(&repo, id, &["--brief"]));
        assert_eq!(brief["pair"], Value::Null, "{label}: --brief");
        if let Some(item) = round["items"]
            .as_array()
            .expect("items")
            .iter()
            .find(|item| item["id"] == json!(id))
        {
            assert_eq!(item["pair"], Value::Null, "{label}: the round");
        } else {
            assert_eq!(label, "an update", "{label}: on the round");
        }
    }
    assert_eq!(review(&repo, &example)["pair"], data_pair());
    assert_eq!(round_item(&round, &example)["pair"], data_pair());
}

// ------------------------------------------------------------------ PP3

/// PP3: `MEC-STAMINA` and `RULE-STAM-REGEN` share `stamina.md`: evidence 1
/// under both, `other` `[0, 2]`. M: a shared file's evidence under `left`
/// alone.
#[test]
fn pp3_one_file_on_both_sides() {
    let repo = Repo::new("cpair-pp3-shared", "spec-a");
    let id = report(
        &repo,
        &input(
            &["MEC-STAMINA", "RULE-STAM-REGEN"],
            "contradicts",
            "The stamina page and its regeneration rule disagree",
            evidence(),
        ),
    );
    let want = json!({
        "left": {"id": "MEC-STAMINA", "title": "Stamina", "path": STAMINA, "evidence": [1]},
        "right": {"id": "RULE-STAM-REGEN", "title": "Regeneration", "path": STAMINA,
            "evidence": [1]},
        "other": [0, 2]});
    assert_eq!(review(&repo, &id)["pair"], want);
    assert_eq!(
        round_item(&json_of(&round_raw(&repo)), &id)["pair"],
        want,
        "the round"
    );
}

/// PP3: the example after `RULE-STAM-REGEN`'s section is removed: `right`
/// its ID, `title` and `path` null, no evidence, `other` `[1, 2]`, in the
/// review and the round. M: `null` for a gone side.
#[test]
fn pp3_a_gone_second_target_keeps_its_side() {
    let repo = Repo::new("cpair-pp3-gone", "spec-a");
    let id = example(&repo);
    remove_section(&repo, STAMINA, "## Regeneration {#RULE-STAM-REGEN}");
    let want = json!({
        "left": {"id": "RULE-SPRINT-COST", "title": "Cost", "path": SPRINT, "evidence": [0]},
        "right": {"id": "RULE-STAM-REGEN", "title": null, "path": null, "evidence": []},
        "other": [1, 2]});
    let raw = review_raw(&repo, &id, &[]);
    assert_eq!(json_of(&raw)["pair"], want, "{raw}");
    assert_eq!(round_item(&json_of(&round_raw(&repo)), &id)["pair"], want);
    let text = review_text(&repo, &id);
    assert!(
        text.contains(
            "\npair:\n  left RULE-SPRINT-COST | Cost | docs/spec/movement/sprint.md | evidence \
             0\n  right RULE-STAM-REGEN | - | - | evidence -\n  other | evidence 1, 2\noptions:"
        ),
        "{text}"
    );
}

/// PP3 ("Rules"): the first target gone keeps the stored `target_path`, so
/// its evidence still groups under it, `title` null.
#[test]
fn pp3_a_gone_first_target_keeps_its_stored_path() {
    let repo = Repo::new("cpair-pp3-first", "spec-a");
    let id = example(&repo);
    remove_section(&repo, SPRINT, "## Cost {#RULE-SPRINT-COST rev=3}");
    let pair = review(&repo, &id)["pair"].clone();
    assert_eq!(
        pair["left"],
        json!({"id": "RULE-SPRINT-COST", "title": null, "path": SPRINT, "evidence": [0]}),
        "{pair}"
    );
    assert_eq!(pair["right"], data_pair()["right"]);
    assert_eq!(pair["other"], json!([2]));
}

// ------------------------------------------------------------------ PP4

/// PP4: retitling `RULE-SPRINT-COST` "Sprint cost" changes `left.title` on
/// the next review, the queue's dump and last event `seq` unchanged by the
/// read; the schema and state format are `collision-pass`'s. M: the title
/// taken from the row stored at intake.
#[test]
fn pp4_a_retitle_shows_on_the_next_read_and_writes_nothing() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 14, "collision-pass's schema");
    assert_eq!(STATE_FORMAT, 7, "collision-pass's state format");
    let repo = Repo::new("cpair-pp4", "spec-a");
    let id = example(&repo);
    assert_eq!(review(&repo, &id)["pair"], data_pair());
    let file = repo.linked.join(SPRINT);
    let text = fs::read_to_string(&file).expect("sprint");
    fs::write(
        &file,
        text.replacen(
            "## Cost {#RULE-SPRINT-COST rev=3}",
            "## Sprint cost {#RULE-SPRINT-COST rev=3}",
            1,
        ),
    )
    .expect("retitle");
    let dump = repo.queue().dump().expect("dump");
    let last_seq = repo.events().last().map(|event| event.seq);
    let pair = review(&repo, &id)["pair"].clone();
    assert_eq!(pair["left"]["title"], json!("Sprint cost"), "{pair}");
    assert_eq!(pair["left"]["evidence"], json!([0]));
    assert_eq!(pair["right"], data_pair()["right"]);
    let round = json_of(&round_raw(&repo));
    assert_eq!(
        round_item(&round, &id)["pair"]["left"]["title"],
        json!("Sprint cost")
    );
    assert_eq!(repo.queue().dump().expect("dump"), dump, "nothing stored");
    assert_eq!(
        repo.events().last().map(|event| event.seq),
        last_seq,
        "no event"
    );
}

// ------------------------------------------------------------------ PP5

/// The four pair lines of the spec's text.
const PAIR_LINES: &str = "pair:\n  left RULE-SPRINT-COST | Cost | docs/spec/movement/sprint.md | \
                          evidence 0\n  right RULE-STAM-REGEN | Regeneration | \
                          docs/spec/movement/stamina.md | evidence 1\n  other | evidence 2\n";

/// The example's three evidence lines as the text prints them.
const EVIDENCE_LINES: &str = "evidence: 3\n  docs/spec/movement/sprint.md:18-20 | Sprinting \
                              pauses regeneration | Cost is drained per second\n  \
                              docs/spec/movement/stamina.md:21-23 | Regeneration never pauses \
                              | Refill starts after 1.5 s\n  src/stamina.rs:40-52 \
                              stamina::regen | regen runs during sprint | a pause is \
                              specified\n";

/// PP5: the four lines right after the evidence lines, before `options:`; a
/// null pair prints no `pair` line, and the example's text without its four
/// lines is its `missing` twin's but for the ID, `gap_type` and the clock.
/// M: a `pair: null` line printed.
#[test]
fn pp5_the_text_prints_four_lines_after_the_evidence() {
    let repo = Repo::new("cpair-pp5", "spec-a");
    let id = example(&repo);
    let text = review_text(&repo, &id);
    assert!(
        text.contains(&format!("\n{EVIDENCE_LINES}{PAIR_LINES}options: 2\n")),
        "{text}"
    );
    assert_eq!(text.matches("pair:").count(), 1, "{text}");
    let brief = spec(&repo, &["review", &id, "--brief"], b"").stdout;
    assert!(
        brief.contains(&format!("\n{PAIR_LINES}options: 2\n")),
        "{brief}"
    );

    let twin = report(
        &repo,
        &input(
            &["RULE-SPRINT-COST", "RULE-STAM-REGEN"],
            "missing",
            SUMMARY,
            evidence(),
        ),
    );
    let three = report(
        &repo,
        &input(
            &["RULE-SPRINT-COST", "RULE-STAM-REGEN", "MEC-SPRINT"],
            "contradicts",
            "Three documents disagree on when stamina refills",
            evidence(),
        ),
    );
    for other in [&twin, &three] {
        let text = review_text(&repo, other);
        assert!(
            !text
                .lines()
                .any(|line| line.trim_start().starts_with("pair")),
            "{other}: no pair line: {text}"
        );
        assert!(
            text.contains(&format!("\n{EVIDENCE_LINES}options: 2\n")),
            "{other}: options right after the evidence: {text}"
        );
    }
    // Line for line, the example less its four lines is its twin.
    let normalise = |text: &str, id: &str| -> Vec<String> {
        text.lines()
            .filter(|line| {
                !line.starts_with("created_at: ")
                    && !line.starts_with("updated_at: ")
                    && !line.starts_with("distinct_from: ")
                    && !line.starts_with("  DEC-")
                    && !line.starts_with("  PR-")
            })
            .map(|line| {
                line.replace(id, "PR-X")
                    .replace("gap_type: contradicts", "gap_type: missing")
            })
            .collect()
    };
    let without = text.replacen(PAIR_LINES, "", 1);
    assert_eq!(
        normalise(&without, &id),
        normalise(&review_text(&repo, &twin), &twin)
    );
}

// ----------------------------------------------------- the rulings, pinned

/// The orchestrator's ruling: only `spec review` and the round fill `pair`.
/// An update's propose answers its review document with `pair` null; an
/// approve and a reject of a pair item answer `null`; a decided item's
/// review still serves its pair (D1: any status).
#[test]
fn acts_answer_no_pair() {
    let repo = Repo::new("cpair-acts", "spec-a");
    let decided = example(&repo);
    let rejected = report(
        &repo,
        &input(
            &["MEC-STAMINA", "RULE-STAM-REGEN"],
            "contradicts",
            "The stamina page and its regeneration rule disagree",
            evidence(),
        ),
    );
    let (span_hash, text) = repo.span(&repo.linked, "MEC-SPRINT");
    let proposed = repo
        .propose(
            &repo.linked,
            "MEC-SPRINT",
            &span_hash,
            &text.replacen("# Sprint", "# Sprinting", 1),
        )
        .unwrap_or_else(|error| panic!("propose: {error}"));
    let (_, proposed_json) = printed(&proposed);
    assert_eq!(json_of(&proposed_json)["pair"], Value::Null, "propose");

    let (approved, _) = repo.decide_ok(&repo.main, &decided, &option(0), None);
    assert!(approved.document.pair.is_none(), "approve: {approved:?}");
    assert_eq!(json_of(&printed(&approved).1)["pair"], Value::Null);
    let (outcome, questions) = repo.reject_answer(&repo.linked, &rejected, "asked before", true);
    let outcome = outcome.unwrap_or_else(|error| panic!("reject: {error}"));
    assert_eq!(questions.len(), 1, "{questions:?}");
    assert!(outcome.document.pair.is_none(), "reject: {outcome:?}");
    assert_eq!(json_of(&printed(&outcome).1)["pair"], Value::Null);

    let after = review(&repo, &decided);
    assert_ne!(after["status"], json!("open"), "{after}");
    assert_eq!(after["pair"], data_pair(), "a decided item's review");
    let after = review(&repo, &rejected);
    assert_eq!(after["status"], json!("rejected"));
    assert_eq!(after["pair"]["other"], json!([0, 2]));
}

/// Sets every file of the project's database (the db, its `-wal` and
/// `-shm`) to `mode`.
fn database_mode(repo: &Repo, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let dir = data_dir(&repo.home);
    let name = format!("{}.db", repo.slug);
    for entry in fs::read_dir(&dir).expect("the data directory") {
        let entry = entry.expect("an entry");
        if entry.file_name().to_string_lossy().starts_with(&name) {
            fs::set_permissions(entry.path(), fs::Permissions::from_mode(mode))
                .expect("permissions");
        }
    }
}

/// The orchestrator's ruling: an index that cannot be read leaves `pair`
/// null with the note `pair unknown: <error>`, exit 0, never a gone side;
/// writable again, the pair is served.
#[test]
fn unknown_an_unreadable_index_serves_null_with_a_note() {
    let repo = Repo::new("cpair-unknown", "spec-a");
    let id = example(&repo);
    assert_eq!(review(&repo, &id)["pair"], data_pair());
    let file = repo.linked.join(SPRINT);
    let text = fs::read_to_string(&file).expect("sprint");
    fs::write(&file, format!("{text}\nA line the index must take.\n")).expect("an edit");
    database_mode(&repo, 0o444);
    let run = repo.spec_piped(&repo.linked, &["review", &id, "--json"], b"");
    database_mode(&repo, 0o644);
    run.code(0);
    let document = run.json();
    assert_eq!(document["pair"], Value::Null, "{}", run.show());
    let notes: Vec<&str> = document["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .filter_map(Value::as_str)
        .filter(|note| note.starts_with("pair unknown: "))
        .collect();
    assert_eq!(notes.len(), 1, "{}", run.show());
    assert!(!notes[0].contains('\n'), "one line: {}", notes[0]);
    assert_eq!(review(&repo, &id)["pair"], data_pair(), "writable again");
}
