//! docs/features/card-questions.md, the CLI half (RM-0097, ADR-0077): the
//! `spec` binary on scratch git repositories of `fixtures/studio-lamp` (a
//! lighthouse game: five trades' flows, each returning to `sent-back`, no
//! `[decision_records]`) and of `fixtures/spec-a` (`[decision_records]`,
//! its template gaining the `{{why}}` lines in the copy only).
//!
//! - S2: `spec propose question|discrepancy|decision … --card C` stores
//!   `card_id`, a card in any state (open, accepted, cancelled); a card no
//!   one made is exit 1, the intake document's last note, no event, no ID
//!   taken; `C-07` exit 1, a look-alike exit 2; `[ids]` claiming `C` exit 2
//!   with `--card` alone. M: the existence check after the insert; a closed
//!   card refused.
//! - S3: a decision's `why` stored and served (`review`, its text line); a
//!   question's `null`; `why` or `card_id` in a discrepancy's `--input` exit
//!   2; the record's `{{why}}` slot holds a decision's why under its working
//!   answer alone, empty under `--answer`, for a question and a decision
//!   without one; in the front-matter a template error. M: `why` filled
//!   under `--answer`; `why` accepted on a question.
//! - S4: a `card_id` on an update, `X-7` on a question, a `why` on a
//!   question: `review` exits 2 naming the column, `inbox` skips the row
//!   with a note; a departing accept's discrepancy names its card; stage,
//!   unstage, a reopen, approve with a record and reject leave `card_id`;
//!   a cancelled card's item is approved as before; a correction card
//!   starts with no item. M: `departure_intake` without the card; a
//!   correction listing its original's items.
//! - L1: the three raises' `--help` list `--card C` with "Data"'s line;
//!   `review --json` ends `interpretation, card_id, why, notes`; `inbox`
//!   lists a carded item as its twin. M: the review document without
//!   `card_id`.
//! - L4, the library half: a stage under `as` logs `proposal.staged`
//!   ending `"as":"ana"`; with none the payload is today's three keys;
//!   `spec approve` confirms the stage as before.
//! - M1: two probes green before and after the slice, each with a name no
//!   build knows (`colour`): an unknown template slot is exit 2 at its line,
//!   before the prompt; an unknown `--input` key of a decision is exit 2
//!   naming it. What a 0.35 binary does with this slice's `{{why}}` and
//!   `why`.
//!
//! K1 (the card read) is `cards.rs`; S5 (backup format 5) `queue_state.rs`.
//! Every expected value below is worked out from the spec, never read back
//! from a run.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::fmt;
use std::fs;
use std::io::{Read as _, Write as _};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Instant;

use common::decision::flags;
use common::proposal::{DECIDER, Pair, cannot, refused};
use common::{RUN_TIMEOUT, Run, SPEC, read_text, with_ids_line, write};
use serde::de::{Deserialize, Deserializer, IgnoredAny, MapAccess, Visitor};
use serde_json::{Value, json};
use specengine_cli::{Exit, Globals, StageBody, StageRequest, UnstageRequest, stage, unstage};

/// The lighthouse game, with return columns and no records.
const LAMP: &str = "studio-lamp";

/// spec-a's decision of kind-decision.md "Data", on `RULE-STAM-REGEN`.
const TEXT: &str = "Does regeneration wait for rest?";
const WORKING: &str = "Yes: it starts 1.5 s after the last stamina use.";
const PRICE: &str = "Without the wait R-28 is rebalanced.";
/// "Data"'s `why`.
const WHY: &str = "Two playtests read instant regeneration as a bug.";

/// "Data"'s question of the card door, on studio-lamp.
const BELL: &str = "Which bell rings at dusk?";
const BELL_ANSWER: &str = "The harbour bell, as in chapter one.";
const BELL_PRICE: &str = "Two scenes rewritten.";

/// "Data"'s decision of the card read, on studio-lamp.
const OIL: &str = "Does the lamp burn oil?";
const OIL_ANSWER: &str = "Yes: whale oil, as the keeper's log says.";
const OIL_PRICE: &str = "The lamp room is redrawn.";

/// `--card`'s help ("Data", "Raising with a card"), as clap prints a
/// one-line help: its final period dropped, as every flag's here.
const CARD_HELP: &str = "The card (C-0001) this is raised for: one of this queue's, in any state";

// ------------------------------------------------------------- helpers

/// A JSON object's keys, in the order written.
struct Keys(Vec<String>);

impl<'de> Deserialize<'de> for Keys {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Seen;
        impl<'de> Visitor<'de> for Seen {
            type Value = Keys;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON object")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Keys, M::Error> {
                let mut keys = Vec::new();
                while let Some((key, IgnoredAny)) = map.next_entry::<String, IgnoredAny>()? {
                    keys.push(key);
                }
                Ok(Keys(keys))
            }
        }
        deserializer.deserialize_map(Seen)
    }
}

fn keys_of(json: &str) -> Vec<String> {
    serde_json::from_str::<Keys>(json)
        .unwrap_or_else(|error| panic!("{error}: {json}"))
        .0
}

/// `spec args` in the main worktree: an empty pipe on stdin.
fn run(pair: &Pair, args: &[&str]) -> Run {
    pair.spec_piped(&pair.main, args, b"")
}

/// `spec --json args` in `cwd` with `input` on stdin.
fn json_run(pair: &Pair, cwd: &Path, args: &[&str], input: &[u8]) -> Run {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    pair.spec_piped(cwd, &all, input)
}

/// `spec --json args` in the main worktree, exit 0: the document.
fn doc(pair: &Pair, args: &[&str]) -> Value {
    let done = json_run(pair, &pair.main, args, b"");
    done.code(0);
    done.json()
}

/// `spec card new --title <title> <flags>`: the card's ID.
fn make(pair: &Pair, title: &str, flags: &[&str]) -> String {
    let mut args = vec!["card", "new", "--title", title];
    args.extend_from_slice(flags);
    doc(pair, &args)["id"]
        .as_str()
        .expect("a card's ID")
        .to_owned()
}

/// `spec --json propose question <targets> --text <text> …` in `cwd`, a
/// developer's, the working answer and price of "Data", then `extra`.
fn ask_in(pair: &Pair, cwd: &Path, targets: &[&str], text: &str, extra: &[&str]) -> Run {
    let mut args = vec!["propose", "question"];
    args.extend_from_slice(targets);
    args.extend_from_slice(&[
        "--text",
        text,
        "--working-answer",
        BELL_ANSWER,
        "--price-of-other",
        BELL_PRICE,
        "--author-role",
        "developer",
    ]);
    args.extend_from_slice(extra);
    json_run(pair, cwd, &args, b"")
}

/// [`ask_in`] in the main worktree.
fn ask(pair: &Pair, targets: &[&str], text: &str, extra: &[&str]) -> Run {
    ask_in(pair, &pair.main, targets, text, extra)
}

/// The names `distinct_from` takes for every hit of an intake document.
fn hit_names(document: &Value) -> Vec<String> {
    document["hits"]
        .as_array()
        .expect("hits")
        .iter()
        .map(|hit| {
            hit["id"]
                .as_str()
                .or_else(|| hit["path"].as_str())
                .expect("a hit's name")
                .to_owned()
        })
        .collect()
}

/// The stored ID of an intake run: exit 0, `created`.
fn stored(done: &Run) -> String {
    done.code(0);
    let document = done.json();
    assert_eq!(document["created"], json!(true), "not stored: {document}");
    document["id"].as_str().expect("an ID").to_owned()
}

/// A question asked in `cwd` and stored: when it hits, asked again naming
/// every hit in `--distinct-from`.
fn ask_stored(pair: &Pair, cwd: &Path, targets: &[&str], text: &str, extra: &[&str]) -> String {
    let first = ask_in(pair, cwd, targets, text, extra);
    first.code(0);
    let document = first.json();
    if document["created"] == json!(true) {
        return document["id"].as_str().expect("an ID").to_owned();
    }
    let names = hit_names(&document);
    let mut again: Vec<&str> = extra.to_vec();
    for name in &names {
        again.extend_from_slice(&["--distinct-from", name]);
    }
    stored(&ask_in(pair, cwd, targets, text, &again))
}

/// `spec --json propose decision --input - --author-role developer
/// <extra>` in `cwd` with `document` on stdin.
fn decide(pair: &Pair, cwd: &Path, document: &Value, extra: &[&str]) -> Run {
    let mut args = vec![
        "propose",
        "decision",
        "--input",
        "-",
        "--author-role",
        "developer",
    ];
    args.extend_from_slice(extra);
    json_run(pair, cwd, &args, document.to_string().as_bytes())
}

/// A decision raised in `cwd` and stored: when it hits, raised again
/// naming every hit in `distinct_from`.
fn decide_stored(pair: &Pair, cwd: &Path, document: &Value, extra: &[&str]) -> String {
    let first = decide(pair, cwd, document, extra);
    first.code(0);
    let answer = first.json();
    if answer["created"] == json!(true) {
        return answer["id"].as_str().expect("an ID").to_owned();
    }
    let mut named = document.clone();
    let mut names: Vec<String> = document["distinct_from"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|name| name.as_str().expect("a name").to_owned())
                .collect()
        })
        .unwrap_or_default();
    names.extend(hit_names(&answer));
    named["distinct_from"] = json!(names);
    stored(&decide(pair, cwd, &named, extra))
}

/// `spec --json propose discrepancy --input - --author-role developer
/// <extra>` in `cwd` with `document` on stdin.
fn report(pair: &Pair, cwd: &Path, document: &Value, extra: &[&str]) -> Run {
    let mut args = vec![
        "propose",
        "discrepancy",
        "--input",
        "-",
        "--author-role",
        "developer",
    ];
    args.extend_from_slice(extra);
    json_run(pair, cwd, &args, document.to_string().as_bytes())
}

/// A decision's `--input` document on `RULE-STAM-REGEN`, its working
/// answer and price, nothing else (spec-a).
fn decision_input() -> Value {
    json!({
        "node_ids": ["RULE-STAM-REGEN"], "text": TEXT, "working_answer": WORKING,
        "price_of_other": PRICE, "severity": null, "proposed_patch": null,
        "distinct_from": ["DEC-0023"]
    })
}

/// "Data"'s decision of the card read, on `SCN-STORM` (studio-lamp).
fn lamp_decision() -> Value {
    json!({
        "node_ids": ["SCN-STORM"], "text": OIL, "working_answer": OIL_ANSWER,
        "price_of_other": OIL_PRICE
    })
}

/// A discrepancy on `node` with two options, the first recommended.
fn report_input(node: &str, summary: &str) -> Value {
    json!({
        "node_ids": [node], "summary": summary, "gap_type": "contradicts", "severity": "normal",
        "evidence": [{"file": "docs/spec/scn-storm.md", "qpath": null, "lines": "12-14",
            "observed": "the lamp lights before the mirrors turn",
            "documented": "the lamp lights once all four face the water"}],
        "options": [{"label": "scene", "effect": "reorder the scene", "price": "one scene"},
            {"label": "spec", "effect": "light it first", "price": "a rule"}],
        "recommendation": 0
    })
}

/// `spec --json propose decision` in `cwd`: an exit-2 refusal of the input
/// naming `named`, stdout empty.
fn input_refused(done: &Run, named: &str, context: &str) {
    done.code(2);
    assert_eq!(done.stdout, "", "{context}");
    assert!(done.stderr.contains(named), "{context}: {}", done.stderr);
}

/// `spec --json review id`: the review document.
fn review(pair: &Pair, id: &str) -> Value {
    doc(pair, &["review", id])
}

/// `quote(<column>)` of `id`'s row, as `sqlite3` prints it.
fn column(pair: &Pair, id: &str, column: &str) -> String {
    pair.sql(&format!(
        "SELECT quote({column}) FROM proposals WHERE id = '{id}';"
    ))
    .trim()
    .to_owned()
}

/// The rows of the project's queue (`0` with no database yet).
fn proposal_count(pair: &Pair) -> usize {
    if !pair.db().exists() {
        return 0;
    }
    pair.sql("select count(*) from proposals")
        .trim()
        .parse()
        .expect("a count")
}

/// The number of stored events (`0` with no database yet).
fn event_count(pair: &Pair) -> usize {
    pair.events().len()
}

/// The last note of an intake document.
fn last_note(document: &Value) -> String {
    document["notes"]
        .as_array()
        .and_then(|notes| notes.last())
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("no note: {document}"))
        .to_owned()
}

/// The library's stage of `body` (JSON, its `updated_at` added) on `id`
/// from `cwd`, `actor` as a local daemon passes the body's `as`.
fn stage_json(
    pair: &Pair,
    cwd: &Path,
    id: &str,
    fields: &str,
    actor: Option<&str>,
) -> specengine_cli::StageOutcome {
    let updated_at = pair.proposal(id).updated_at;
    let body = format!("{{{fields},\"updated_at\":\"{updated_at}\"}}");
    let body =
        StageBody::from_json(body.as_bytes()).unwrap_or_else(|error| panic!("{body}: {error}"));
    stage(
        &pair.env(cwd),
        &Globals::default(),
        &StageRequest {
            id: id.to_owned(),
            body,
            now: "2026-11-11T09:30:00Z".to_owned(),
            git: pair.git_env(cwd),
            actor: actor.map(str::to_owned),
        },
    )
    .unwrap_or_else(|error| panic!("stage {id}: {error}"))
}

/// The library's unstage of `id` from `cwd`.
fn unstage_now(pair: &Pair, cwd: &Path, id: &str) -> specengine_cli::StageOutcome {
    unstage(
        &pair.env(cwd),
        &Globals::default(),
        &UnstageRequest {
            id: id.to_owned(),
            now: "2026-11-11T09:31:00Z".to_owned(),
            git: pair.git_env(cwd),
            actor: None,
        },
    )
    .unwrap_or_else(|error| panic!("unstage {id}: {error}"))
}

/// The raw payload of the last event of `kind` naming `id`.
fn last_payload(pair: &Pair, kind: &str, id: &str) -> String {
    pair.sql(&format!(
        "SELECT payload FROM events WHERE type = '{kind}' AND json_extract(payload, '$.id') = \
         '{id}' ORDER BY seq DESC LIMIT 1;"
    ))
    .trim_end_matches('\n')
    .to_owned()
}

/// spec-a's template in the linked worktree with "Data"'s lines after
/// `{{cost}}`'s block (`Why:`, a blank line, `{{why}}`), committed there.
fn with_why_slot(pair: &Pair) {
    let template = read_text(&pair.linked, "templates/decision.md");
    let block = "Cost:\n\n{{cost}}\n";
    assert!(template.contains(block), "{template}");
    write(
        &pair.linked,
        "templates/decision.md",
        template.replacen(block, "Cost:\n\n{{cost}}\n\nWhy:\n\n{{why}}\n", 1),
    );
    pair.commit_all(&pair.linked, "The why slot.");
}

/// spec-a's record `id` in the linked worktree.
fn record(pair: &Pair, id: &str) -> String {
    read_text(&pair.linked, &format!("docs/records/DEC/{id}.md"))
}

/// `spec card accept id --as name --briefed` on a terminal, `y`: exit 0.
fn accept_briefed(pair: &Pair, id: &str, name: &str) {
    let done = pair.spec_pty(
        &pair.main,
        &["card", "accept", id, "--as", name, "--briefed"],
        Some("y"),
    );
    assert_eq!(done.code, 0, "accept {id}: {}", done.output);
}

// ------------------------------------------------------------------ S2

/// S2: on studio-lamp, `C-0001` with a target; a question, a decision and a
/// discrepancy each raised with `--card C-0001` store it (`review --json`
/// `card_id`, the column); an accepted card and a cancelled one each take
/// an item; a raise without the flag holds `NULL`, served `null`. M: a
/// closed card refused.
#[test]
fn s2_each_deciding_kind_names_its_card_in_any_state() {
    let pair = Pair::new("cq-s2-raise", LAMP);
    let card = make(
        &pair,
        "The storm scene",
        &["--target", "SCN-STORM", "--discipline", "narrative"],
    );
    assert_eq!(card, "C-0001");
    assert_eq!(
        stored(&ask(&pair, &["SCN-STORM"], BELL, &["--card", "C-0001"])),
        "PR-0001"
    );
    let reviewed = review(&pair, "PR-0001");
    assert_eq!(reviewed["card_id"], json!("C-0001"), "{reviewed}");
    assert_eq!(reviewed["why"], Value::Null, "{reviewed}");
    assert_eq!(column(&pair, "PR-0001", "card_id"), "'C-0001'");

    assert_eq!(
        stored(&decide(
            &pair,
            &pair.main,
            &lamp_decision(),
            &["--card", "C-0001"]
        )),
        "PR-0002"
    );
    let report_doc = report_input("SCN-STORM", "The storm scene lights the lamp too early.");
    assert_eq!(
        stored(&report(
            &pair,
            &pair.main,
            &report_doc,
            &["--card", "C-0001"]
        )),
        "PR-0003"
    );
    for id in ["PR-0002", "PR-0003"] {
        assert_eq!(review(&pair, id)["card_id"], json!("C-0001"), "{id}");
        assert_eq!(column(&pair, id, "card_id"), "'C-0001'", "{id}");
    }

    // An accepted card and a cancelled one each take an item.
    let accepted = make(
        &pair,
        "The arrival",
        &[
            "--target",
            "SCN-ARRIVAL",
            "--discipline",
            "narrative",
            "--reviewer",
            "Ann",
        ],
    );
    run(&pair, &["card", "move", &accepted, "--to", "ready-for-dev"]).code(0);
    accept_briefed(&pair, &accepted, "Ann");
    assert_ne!(
        doc(&pair, &["card", "show", &accepted])["acceptance"],
        Value::Null,
        "premise: accepted"
    );
    let cancelled = make(
        &pair,
        "The cove",
        &["--target", "ISL-COVE", "--discipline", "narrative"],
    );
    run(
        &pair,
        &[
            "card",
            "cancel",
            &cancelled,
            "--reason",
            "Cut from the chapter.",
        ],
    )
    .code(0);
    let on_accepted = stored(&ask(
        &pair,
        &["SCN-ARRIVAL"],
        "Who meets the keeper at the pier?",
        &["--card", &accepted],
    ));
    let on_cancelled = stored(&ask(
        &pair,
        &["ISL-COVE"],
        "Is the cove reachable at high tide?",
        &["--card", &cancelled],
    ));
    assert_eq!(
        [on_accepted.as_str(), on_cancelled.as_str()],
        ["PR-0004", "PR-0005"]
    );
    assert_eq!(column(&pair, &on_accepted, "card_id"), "'C-0002'");
    assert_eq!(column(&pair, &on_cancelled, "card_id"), "'C-0003'");

    // Without the flag: NULL, served null.
    let plain = stored(&ask(
        &pair,
        &["ISL-LAMP"],
        "How many steps has the stair?",
        &[],
    ));
    assert_eq!(column(&pair, &plain, "card_id"), "NULL");
    assert_eq!(review(&pair, &plain)["card_id"], Value::Null);
}

/// S2: `--card C-0099`, which no card holds: exit 1, the intake document
/// with `id` `null`, `created` false, its last note `card_id: no card
/// C-0099`, on stderr too; no event, the queue as it was; the next stored
/// raise takes the next free ID. A decision and a discrepancy alike. M: the
/// existence check after the insert (an ID taken).
#[test]
fn s2_a_card_no_one_made_stores_nothing_and_takes_no_id() {
    let pair = Pair::new("cq-s2-missing", LAMP);
    make(
        &pair,
        "The storm scene",
        &["--target", "SCN-STORM", "--discipline", "narrative"],
    );
    assert_eq!(
        stored(&ask(&pair, &["SCN-STORM"], BELL, &["--card", "C-0001"])),
        "PR-0001"
    );
    let events = event_count(&pair);
    let dump = pair.queue().dump().expect("dump");
    let reason = "card_id: no card C-0099";

    let pier = "Who meets the keeper at the pier?";
    let missing = ask(&pair, &["SCN-ARRIVAL"], pier, &["--card", "C-0099"]);
    missing.code(1);
    let document = missing.json();
    assert_eq!(document["id"], Value::Null, "{document}");
    assert_eq!(document["created"], json!(false), "{document}");
    assert_eq!(last_note(&document), reason);
    assert!(missing.stderr.contains(reason), "{}", missing.stderr);
    let decided = decide(&pair, &pair.main, &lamp_decision(), &["--card", "C-0099"]);
    decided.code(1);
    assert_eq!(last_note(&decided.json()), reason);
    let reported = report(
        &pair,
        &pair.main,
        &report_input("SCN-STORM", "The storm scene lights the lamp too early."),
        &["--card", "C-0099"],
    );
    reported.code(1);
    assert_eq!(last_note(&reported.json()), reason);
    assert_eq!(event_count(&pair), events, "no event");
    assert_eq!(pair.queue().dump().expect("dump"), dump, "nothing stored");

    // The next stored raise takes the next free ID.
    assert_eq!(
        stored(&ask(&pair, &["SCN-ARRIVAL"], pier, &["--card", "C-0001"])),
        "PR-0002"
    );
}

/// S2: `--card C-07` (no card ID in any script) exits 1 with "Data"'s line;
/// a look-alike (a Cyrillic first letter) exits 2 naming `C-0001`, stdout
/// empty; nothing stored by either, for each of the three raises.
#[test]
fn s2_a_card_id_off_the_grammar_exits_1_and_a_look_alike_exits_2() {
    let pair = Pair::new("cq-s2-grammar", LAMP);
    make(
        &pair,
        "The storm scene",
        &["--target", "SCN-STORM", "--discipline", "narrative"],
    );
    let off = "card_id: no card `C-07`: a card ID is `C-` and 4 or more digits, as `spec card \
               list` lists it";
    // "C-0001" with a Cyrillic capital es (U+0421) for its "C".
    let look = "\u{0421}-0001";
    let alike = format!(
        "card_id: `{look}` uses look-alike letters or digits; card IDs are Latin only: write \
         `C-0001`"
    );
    let report_doc = report_input("SCN-STORM", "The storm scene lights the lamp too early.");
    let raises = |card: &str| -> Vec<(&'static str, Run)> {
        vec![
            (
                "question",
                ask(&pair, &["SCN-STORM"], BELL, &["--card", card]),
            ),
            (
                "decision",
                decide(&pair, &pair.main, &lamp_decision(), &["--card", card]),
            ),
            (
                "discrepancy",
                report(&pair, &pair.main, &report_doc, &["--card", card]),
            ),
        ]
    };
    for (kind, done) in raises("C-07") {
        assert_eq!(done.code, 1, "{kind}: {}", done.show());
        assert!(done.stderr.contains(off), "{kind}: {}", done.stderr);
        assert_eq!(last_note(&done.json()), off, "{kind}");
    }
    for (kind, done) in raises(look) {
        assert_eq!(done.code, 2, "{kind}: {}", done.show());
        assert_eq!(done.stdout, "", "{kind}");
        assert!(done.stderr.contains(&alike), "{kind}: {}", done.stderr);
    }
    assert_eq!(proposal_count(&pair), 0, "nothing stored");
}

/// S2: `[ids]` claiming `C` exits 2 with `card_prefix_clash`'s line for
/// each raise naming `--card`, nothing stored; without the flag the raise
/// is as today.
#[test]
fn s2_ids_claiming_c_exit_2_only_with_card() {
    let pair = Pair::new("cq-s2-clash", LAMP);
    let config = read_text(&pair.main, "specengine.toml");
    write(
        &pair.main,
        "specengine.toml",
        with_ids_line(&config, "C = { kind = \"cue\", width = 4 }\n"),
    );
    let clash = "the project's `[ids]` configures the prefix `C`, which is SpecEngine's card \
                 prefix (`C-0001`): rename it before using cards";
    let report_doc = report_input("SCN-STORM", "The storm scene lights the lamp too early.");
    for (kind, done) in [
        (
            "question",
            ask(&pair, &["SCN-STORM"], BELL, &["--card", "C-0001"]),
        ),
        (
            "decision",
            decide(&pair, &pair.main, &lamp_decision(), &["--card", "C-0001"]),
        ),
        (
            "discrepancy",
            report(&pair, &pair.main, &report_doc, &["--card", "C-0001"]),
        ),
    ] {
        assert_eq!(done.code, 2, "{kind}: {}", done.show());
        assert_eq!(done.stdout, "", "{kind}");
        assert!(done.stderr.contains(clash), "{kind}: {}", done.stderr);
    }
    assert_eq!(proposal_count(&pair), 0, "nothing stored");
    assert_eq!(stored(&ask(&pair, &["SCN-STORM"], BELL, &[])), "PR-0001");
    assert_eq!(column(&pair, "PR-0001", "card_id"), "NULL");
}

// ------------------------------------------------------------------ S3

/// S3, the CLI: a decision's `--input` with `why` stores it as given
/// (`review --json`, the text line `why: …`, the column); a question's is
/// `null` (`why: -`); a blank `why`, then 2 049 bytes, exit 1 with "Data"'s
/// lines, nothing stored; 1 024 `é` (2 048 bytes) stored; `why` in a
/// discrepancy's `--input` and `card_id` in any `--input` exit 2 as unknown
/// fields. M: `why` accepted on a question.
#[test]
fn s3_a_decision_keeps_its_why_a_question_has_none_a_discrepancy_refuses_one() {
    let pair = Pair::new("cq-s3-why", "spec-a");
    let cwd = pair.linked.clone();
    let mut with_why = decision_input();
    with_why["why"] = json!(WHY);
    let decision = decide_stored(&pair, &cwd, &with_why, &[]);
    assert_eq!(decision, "PR-0001");
    let reviewed = review(&pair, &decision);
    assert_eq!(reviewed["why"], json!(WHY), "{reviewed}");
    assert_eq!(reviewed["card_id"], Value::Null, "{reviewed}");
    assert_eq!(column(&pair, &decision, "why"), format!("'{WHY}'"));
    let text = run(&pair, &["review", &decision]);
    text.code(0);
    assert!(
        text.stdout.contains(&format!("\nwhy: {WHY}\n")),
        "{}",
        text.stdout
    );
    assert!(text.stdout.contains("\ncard_id: -\n"), "{}", text.stdout);

    let question = ask_stored(
        &pair,
        &cwd,
        &["EDGE-STAM-ZERO"],
        "Is zero stamina a stop?",
        &[],
    );
    assert_eq!(review(&pair, &question)["why"], Value::Null);
    assert_eq!(column(&pair, &question, "why"), "NULL");
    let text = run(&pair, &["review", &question]);
    text.code(0);
    assert!(text.stdout.contains("\nwhy: -\n"), "{}", text.stdout);

    // The refusals of `why`, after `price_of_other`'s: exit 1, nothing.
    let count = proposal_count(&pair);
    let other = |why: Value| {
        let mut document = decision_input();
        document["text"] = json!("How long is the wait after a sprint?");
        document["why"] = why;
        document
    };
    for (why, line) in [
        (json!(" \n\t"), "why: blank: give a value"),
        (json!("a".repeat(2049)), "why: 2049 bytes; at most 2048"),
    ] {
        let done = decide(&pair, &cwd, &other(why), &[]);
        assert_eq!(done.code, 1, "{line}: {}", done.show());
        assert!(done.stderr.contains(line), "{line}: {}", done.stderr);
        assert_eq!(last_note(&done.json()), line);
        assert_eq!(proposal_count(&pair), count, "{line}: nothing stored");
    }
    // 2 048 bytes of two-byte characters: stored as given.
    let long = "\u{e9}".repeat(1024);
    assert_eq!(long.len(), 2048);
    let at_cap = decide_stored(&pair, &cwd, &other(json!(long)), &[]);
    assert_eq!(review(&pair, &at_cap)["why"], json!(long));

    // Unknown fields: `why` in a discrepancy's input, `card_id` in any.
    let count = proposal_count(&pair);
    let mut report_why = report_input("MEC-SPRINT", "Walking drains stamina in the build.");
    report_why["why"] = json!(WHY);
    input_refused(
        &report(&pair, &cwd, &report_why, &[]),
        "unknown field `why`",
        "a discrepancy's why",
    );
    let mut report_card = report_input("MEC-SPRINT", "Walking drains stamina in the build.");
    report_card["card_id"] = json!("C-0001");
    input_refused(
        &report(&pair, &cwd, &report_card, &[]),
        "unknown field `card_id`",
        "a discrepancy's card_id",
    );
    let mut decision_card = other(Value::Null);
    decision_card["card_id"] = json!("C-0001");
    input_refused(
        &decide(&pair, &cwd, &decision_card, &[]),
        "unknown field `card_id`",
        "a decision's card_id",
    );
    assert_eq!(proposal_count(&pair), count, "nothing stored");
}

/// S3, the record: spec-a's template gains "Data"'s `Why:` lines (in the
/// copy). The decision with a why, approved with no flag: `DEC-0024` holds
/// it under `Why:` (its `Cost:` empty, as a working answer's record has it,
/// kind-decision AC-R4); another decision with a why approved `--answer T`: the
/// slot empty (W2); a question approved with no flag and a decision without
/// a why: empty. M: `why` filled under `--answer`.
#[test]
fn s3_the_why_slot_holds_a_decisions_why_under_its_working_answer_alone() {
    let pair = Pair::new("cq-s3-slot", "spec-a");
    let cwd = pair.linked.clone();
    with_why_slot(&pair);
    let empty = "\n\nWhy:\n\n\n\nSummary:\n\n";

    let mut first = decision_input();
    first["why"] = json!(WHY);
    let first = decide_stored(&pair, &cwd, &first, &[]);
    pair.decide_ok(&pair.main, &first, &flags(None, None, None), None);
    let written = record(&pair, "DEC-0024");
    assert!(
        written.contains(&format!(
            "Cost:\n\n\n\nWhy:\n\n{WHY}\n\nSummary:\n\n{TEXT}\n"
        )),
        "{written}"
    );
    assert!(
        written.ends_with(&format!("Decided by:\n\n{DECIDER}\n")),
        "{written}"
    );

    let since = "The sprint test reads it so.";
    let second = json!({
        "node_ids": ["EDGE-STAM-ZERO"], "text": "Is zero stamina a stop?",
        "working_answer": "Yes.", "price_of_other": "A new rule case.", "why": since
    });
    let second = decide_stored(&pair, &cwd, &second, &[]);
    pair.decide_ok(
        &pair.main,
        &second,
        &flags(None, Some("No: it slows to a walk."), None),
        None,
    );
    let written = record(&pair, "DEC-0025");
    assert!(written.contains("No: it slows to a walk."), "{written}");
    assert!(written.contains(empty), "{written}");
    assert!(!written.contains(since), "the why stays out: {written}");

    let question = ask_stored(
        &pair,
        &cwd,
        &["MEC-SPRINT"],
        "Does a sprint end at zero?",
        &[],
    );
    pair.decide_ok(&pair.main, &question, &flags(None, None, None), None);
    let written = record(&pair, "DEC-0026");
    assert!(written.contains(BELL_ANSWER), "{written}");
    assert!(written.contains(empty), "{written}");

    let without = json!({
        "node_ids": ["EDGE-SPRINT-EMPTY"], "text": "Does an empty sprint end at once?",
        "working_answer": "Yes, at once.", "price_of_other": "A new edge case."
    });
    let without = decide_stored(&pair, &cwd, &without, &[]);
    pair.decide_ok(&pair.main, &without, &flags(None, None, None), None);
    let written = record(&pair, "DEC-0027");
    assert!(written.contains(empty), "{written}");
}

/// S3: `{{why}}` in the template's front-matter is a template error: the
/// approve exits 2 at step 3 naming its line, nothing written.
#[test]
fn s3_why_in_the_front_matter_is_a_template_error() {
    let pair = Pair::new("cq-s3-front", "spec-a");
    let template = read_text(&pair.linked, "templates/decision.md");
    let front = template.replacen("title: {{title}}\n", "title: {{title}}\nwhy: {{why}}\n", 1);
    assert_ne!(front, template);
    let line = front
        .lines()
        .position(|line| line == "why: {{why}}")
        .expect("the line")
        + 1;
    write(&pair.linked, "templates/decision.md", &front);
    pair.commit_all(&pair.linked, "The why slot in the front-matter.");
    let mut input = decision_input();
    input["why"] = json!(WHY);
    let id = decide_stored(&pair, &pair.linked, &input, &[]);
    let before = pair.state();
    let (outcome, questions) = pair.decide(&pair.main, &id, &flags(None, None, None), None, true);
    let message = cannot(&outcome, "why in the front-matter");
    assert!(
        message.contains(&format!(
            "(step 3): templates/decision.md:{line}: `{{{{why}}}}` is free text, and free text \
             goes in the body only"
        )),
        "{message}"
    );
    assert!(questions.is_empty(), "{questions:?}");
    assert_eq!(pair.state(), before, "nothing written");
}

// ------------------------------------------------------------------ S4

/// S4, corrupt rows by SQL on spec-a: `card_id` on an `update`, `X-7` on a
/// question, a `why` on a question: `spec review` exits 2 with "Data"'s
/// line; `inbox` lists the sound row alone, a note naming each broken one
/// and its column; restored, all three list again.
#[test]
fn s4_a_card_id_or_why_where_none_belongs_is_named_and_skipped() {
    let pair = Pair::new("cq-s4-corrupt", "spec-a");
    let cwd = pair.linked.clone();
    let update = pair.propose_edit(&cwd, "EDGE-STAM-ZERO", "immediately", "at once");
    let question = ask_stored(
        &pair,
        &cwd,
        &["MEC-SPRINT"],
        "Does a sprint end at zero?",
        &[],
    );
    let sound = ask_stored(
        &pair,
        &cwd,
        &["EDGE-SPRINT-EMPTY"],
        "Does an empty sprint end at once?",
        &[],
    );
    assert_eq!(
        [update.as_str(), question.as_str(), sound.as_str()],
        ["PR-0001", "PR-0002", "PR-0003"]
    );
    for (sql, broken, line, named) in [
        (
            "UPDATE proposals SET card_id = 'C-0001' WHERE id = 'PR-0001';",
            "PR-0001",
            "proposal PR-0001: the stored `card_id` cannot be read: it is set on an update, \
             which names no card",
            "card_id",
        ),
        (
            "UPDATE proposals SET card_id = 'X-7' WHERE id = 'PR-0002';",
            "PR-0002",
            "proposal PR-0002: the stored `card_id` cannot be read: `X-7` is no card ID",
            "card_id",
        ),
        (
            "UPDATE proposals SET why = 'Because.' WHERE id = 'PR-0002';",
            "PR-0002",
            "proposal PR-0002: the stored `why` cannot be read: it is set on a question, which \
             holds none",
            "why",
        ),
    ] {
        pair.sql(sql);
        let done = run(&pair, &["review", broken]);
        assert_eq!(done.code, 2, "{sql}: {}", done.show());
        assert!(done.stderr.contains(line), "{sql}: {}", done.stderr);
        let inbox = pair.inbox(&pair.main, true).expect("inbox lists the rest");
        assert!(
            inbox
                .notes
                .iter()
                .any(|note| note.contains(broken) && note.contains(&format!("`{named}`"))),
            "{sql}: {:?}",
            inbox.notes
        );
        let listed: Vec<&str> = inbox
            .proposals
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert!(!listed.contains(&broken), "{sql}: {listed:?}");
        assert!(listed.contains(&sound.as_str()), "{sql}: {listed:?}");
        pair.sql("UPDATE proposals SET card_id = NULL, why = NULL;");
        assert_eq!(pair.inbox(&pair.main, true).unwrap().proposals.len(), 3);
    }
}

/// S4, the link's life on spec-a (the default flow): a question raised for
/// `C-0001` keeps `card_id` through a stage, an unstage, a reopen (a
/// failing `pre-commit` hook: step 9) and the approve with its record; a
/// second through a reject; the card cancelled meanwhile, a third is
/// approved as before. Each read from the row after each write.
#[test]
fn s4_every_later_write_leaves_the_card_as_raised() {
    let pair = Pair::new("cq-s4-life", "spec-a");
    let cwd = pair.linked.clone();
    let card = make(&pair, "Rest regeneration", &["--target", "RULE-STAM-REGEN"]);
    assert_eq!(card, "C-0001");
    let asked = ask_stored(
        &pair,
        &cwd,
        &["EDGE-STAM-ZERO"],
        "Is zero stamina a stop?",
        &["--card", "C-0001"],
    );
    let held = |id: &str, after: &str| {
        assert_eq!(column(&pair, id, "card_id"), "'C-0001'", "after {after}");
    };
    held(&asked, "the raise");
    let staged = stage_json(&pair, &pair.main, &asked, "\"decision\":\"approve\"", None);
    assert_eq!(staged.exit(), Exit::Answered, "{staged:?}");
    held(&asked, "a stage");
    let cleared = unstage_now(&pair, &pair.main, &asked);
    assert_eq!(cleared.exit(), Exit::Answered, "{cleared:?}");
    held(&asked, "an unstage");

    // A reopen: the commit refused by a hook (step 9), the item open again.
    let hooks = pair.main.join(".git/hooks");
    fs::create_dir_all(&hooks).expect("hooks");
    let hook = hooks.join("pre-commit");
    fs::write(&hook, "#!/bin/sh\necho refused by the hook >&2\nexit 1\n").expect("hook");
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("mode");
    let (outcome, _) = pair.decide(&pair.main, &asked, &flags(None, None, None), None, true);
    refused(&outcome, "a failing hook");
    assert_eq!(column(&pair, &asked, "status"), "'open'");
    held(&asked, "a reopen");
    fs::remove_file(&hook).expect("the hook removed");
    pair.decide_ok(&pair.main, &asked, &flags(None, None, None), None);
    assert_eq!(column(&pair, &asked, "status"), "'applied'");
    held(&asked, "the approve and its record");

    let other = ask_stored(
        &pair,
        &cwd,
        &["MEC-SPRINT"],
        "Does a sprint end at zero?",
        &["--card", "C-0001"],
    );
    let (outcome, _) = pair.reject_answer(&pair.main, &other, "No: it slows to a walk.", true);
    assert_eq!(
        outcome.expect("reject").exit(),
        Exit::Answered,
        "the reject settles"
    );
    assert_eq!(column(&pair, &other, "status"), "'rejected'");
    held(&other, "a reject");

    run(&pair, &["card", "cancel", "C-0001", "--reason", "Dropped."]).code(0);
    let third = ask_stored(
        &pair,
        &cwd,
        &["EDGE-SPRINT-EMPTY"],
        "Does an empty sprint end at once?",
        &["--card", "C-0001"],
    );
    held(&third, "a raise on a cancelled card");
    pair.decide_ok(&pair.main, &third, &flags(None, None, None), None);
    assert_eq!(column(&pair, &third, "status"), "'applied'");
    held(&third, "the approve under a cancelled card");
    for id in [&asked, &other, &third] {
        assert_eq!(review(&pair, id)["card_id"], json!("C-0001"), "{id}");
    }
}

/// S4, a departing accept on studio-lamp: `spec card accept C --as Ann
/// --departs SCN-STORM …` stores its discrepancy with `card_id` the card
/// accepted; a correction card (`card new --corrects C`) starts with
/// `items: []` while the original lists its item; cancelling a card keeps
/// its items. M: `departure_intake` without the card; a correction listing
/// its original's items.
#[test]
fn s4_a_departing_accept_names_its_card_and_a_correction_starts_empty() {
    let pair = Pair::new("cq-s4-departs", LAMP);
    let card = make(
        &pair,
        "The storm scene",
        &[
            "--target",
            "SCN-STORM",
            "--discipline",
            "narrative",
            "--reviewer",
            "Ann",
        ],
    );
    run(&pair, &["card", "move", &card, "--to", "ready-for-dev"]).code(0);
    let done = pair.spec_pty(
        &pair.main,
        &[
            "card",
            "accept",
            &card,
            "--as",
            "Ann",
            "--departs",
            "SCN-STORM",
            "--summary",
            "The lamp lights before the mirrors turn.",
            "--observed",
            "the lamp lights first",
            "--documented",
            "the lamp lights once all four face the water",
        ],
        Some("y"),
    );
    assert_eq!(done.code, 0, "{}", done.output);
    assert_eq!(proposal_count(&pair), 1, "one discrepancy");
    assert_eq!(column(&pair, "PR-0001", "kind"), "'discrepancy'");
    assert_eq!(column(&pair, "PR-0001", "card_id"), format!("'{card}'"));
    let original = doc(&pair, &["card", "show", &card]);
    let ids: Vec<&str> = original["items"]
        .as_array()
        .unwrap_or_else(|| panic!("items: {original}"))
        .iter()
        .map(|item| item["id"].as_str().expect("an ID"))
        .collect();
    assert_eq!(ids, ["PR-0001"], "{original}");

    let correction = make(
        &pair,
        "Light the lamp last",
        &[
            "--target",
            "SCN-STORM",
            "--discipline",
            "narrative",
            "--corrects",
            &card,
        ],
    );
    let shown = doc(&pair, &["card", "show", &correction]);
    assert_eq!(shown["items"], json!([]), "{shown}");

    let other = make(
        &pair,
        "The arrival",
        &["--target", "SCN-ARRIVAL", "--discipline", "narrative"],
    );
    let asked = stored(&ask(
        &pair,
        &["SCN-ARRIVAL"],
        "Who meets the keeper at the pier?",
        &["--card", &other],
    ));
    run(&pair, &["card", "cancel", &other, "--reason", "Cut."]).code(0);
    let shown = doc(&pair, &["card", "show", &other]);
    assert_eq!(shown["items"][0]["id"], json!(asked), "{shown}");
    assert_eq!(column(&pair, &asked, "card_id"), format!("'{other}'"));
}

// ------------------------------------------------------------------ L1

/// L1: the three raises' `--help` list `--card <C>` with "Data"'s line;
/// `spec review PR --json`'s keys end `interpretation, card_id, why,
/// notes`; `spec inbox --json` lists a carded question exactly as its twin
/// raised without (set 2 unchanged), `created_at` aside. M: the review
/// document without `card_id`.
#[test]
fn l1_the_raises_take_card_and_review_ends_with_card_id_and_why() {
    let pair = Pair::new("cq-l1", LAMP);
    for kind in ["question", "discrepancy", "decision"] {
        let help = run(&pair, &["propose", kind, "--help"]);
        help.code(0);
        assert!(
            help.stdout.contains("--card <C>"),
            "{kind}: {}",
            help.stdout
        );
        let flat = help.stdout.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(flat.contains(CARD_HELP), "{kind}: {}", help.stdout);
    }

    make(
        &pair,
        "The storm scene",
        &["--target", "SCN-STORM", "--discipline", "narrative"],
    );
    stored(&ask(&pair, &["SCN-STORM"], BELL, &["--card", "C-0001"]));
    let raw = json_run(&pair, &pair.main, &["review", "PR-0001"], b"");
    raw.code(0);
    let keys = keys_of(raw.stdout.trim_end());
    assert_eq!(
        keys[keys.len() - 4..],
        ["interpretation", "card_id", "why", "notes"],
        "{keys:?}"
    );
    assert_eq!(keys.len(), 50, "{keys:?}");

    // The twin, on a second copy: the same question without the flag.
    let twin = Pair::new("cq-l1-twin", LAMP);
    stored(&ask(&twin, &["SCN-STORM"], BELL, &[]));
    let listed = |pair: &Pair| {
        let mut inbox = doc(pair, &["inbox"]);
        for entry in inbox["proposals"].as_array_mut().expect("proposals") {
            entry["created_at"] = Value::Null;
        }
        inbox
    };
    let carded = listed(&pair);
    assert_eq!(carded, listed(&twin));
    let entry = &carded["proposals"][0];
    let mut entry_keys: Vec<&String> = entry.as_object().expect("an entry").keys().collect();
    entry_keys.sort();
    assert_eq!(
        entry_keys,
        [
            "branch",
            "created_at",
            "id",
            "kind",
            "rationale",
            "record_id",
            "severity",
            "staged_at",
            "status",
            "summary",
            "target_id",
            "target_ids"
        ],
        "set 2 unchanged"
    );
}

// ------------------------------------------------------------------ L4

/// L4, the library half on spec-a: a stage passed `as` (`StageRequest.actor`,
/// what a local daemon passes from the body) logs `proposal.staged` ending
/// `"as":"ana"`; with none its payload is exactly `{"id","staged",
/// "staged_at"}`; `staged` holds no `as` either way; `spec approve` confirms
/// the stage as before. M: `"as":null` written; the `as` in a column.
#[test]
fn l4_a_stage_under_as_logs_it_and_without_logs_todays_payload() {
    let pair = Pair::new("cq-l4", "spec-a");
    let cwd = pair.linked.clone();
    let asked = ask_stored(
        &pair,
        &cwd,
        &["EDGE-STAM-ZERO"],
        "Is zero stamina a stop?",
        &[],
    );
    let done = stage_json(
        &pair,
        &pair.main,
        &asked,
        "\"decision\":\"approve\"",
        Some("ana"),
    );
    assert_eq!(done.exit(), Exit::Answered, "{done:?}");
    let payload = last_payload(&pair, "proposal.staged", &asked);
    assert!(payload.ends_with(",\"as\":\"ana\"}"), "{payload}");
    assert_eq!(
        keys_of(&payload),
        ["id", "staged", "staged_at", "as"],
        "{payload}"
    );
    let with_as = column(&pair, &asked, "staged");
    assert!(!with_as.contains("ana"), "no `as` in `staged`: {with_as}");

    let done = stage_json(&pair, &pair.main, &asked, "\"decision\":\"approve\"", None);
    assert_eq!(done.exit(), Exit::Answered, "{done:?}");
    let payload = last_payload(&pair, "proposal.staged", &asked);
    assert_eq!(
        keys_of(&payload),
        ["id", "staged", "staged_at"],
        "{payload}"
    );
    assert!(!payload.contains("\"as\""), "{payload}");
    assert_eq!(column(&pair, &asked, "staged"), with_as, "the same stage");

    pair.decide_ok(&pair.main, &asked, &flags(None, None, None), None);
    assert_eq!(column(&pair, &asked, "status"), "'applied'");
}

// ------------------------------------------------------------------ M1 helpers

/// `spec --json propose decision --input - --author-role developer` in
/// `cwd` with `document` on stdin.
fn propose_decision(pair: &Pair, cwd: &Path, document: &Value) -> common::Run {
    pair.spec_piped(
        cwd,
        &[
            "--json",
            "propose",
            "decision",
            "--input",
            "-",
            "--author-role",
            "developer",
        ],
        document.to_string().as_bytes(),
    )
}

/// One run of `spec` with stdin and stdout on a pseudo-terminal (`script`)
/// and stderr apart, in a file: the exit code, the terminal's text and
/// stderr. A `[y/N]` on the terminal is answered `n`, so the run never
/// hangs; whether one was seen is returned too.
struct TtyRun {
    code: i32,
    terminal: String,
    stderr: String,
    asked: bool,
}

fn spec_tty(pair: &Pair, cwd: &Path, args: &[&str]) -> TtyRun {
    let script = ["/usr/bin/script", "/bin/script"]
        .into_iter()
        .find(|path| Path::new(path).exists())
        .expect("a `script` program gives `spec` a terminal");
    let dir = pair.scratch.dir("tty");
    let err: PathBuf = dir.join(format!("stderr-{}", args.join("-").replace('/', "_")));
    let _ = fs::remove_file(&err);
    let wrapper = dir.join("stderr-apart.sh");
    let quote = |text: &str| format!("'{}'", text.replace('\'', "'\\''"));
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nexec \"$@\" 2>{}\n",
            quote(err.to_str().expect("a UTF-8 scratch path"))
        ),
    )
    .expect("the wrapper");
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).expect("executable");
    let wrapper = wrapper.to_str().expect("a UTF-8 scratch path").to_owned();
    let mut command = Command::new(script);
    if cfg!(target_os = "macos") {
        command
            .arg("-q")
            .arg("/dev/null")
            .arg(&wrapper)
            .arg(SPEC)
            .args(args);
    } else {
        let quoted: Vec<String> = [wrapper.as_str(), SPEC]
            .into_iter()
            .chain(args.iter().copied())
            .map(quote)
            .collect();
        command.arg("-qec").arg(quoted.join(" ")).arg("/dev/null");
    }
    command
        .env_clear()
        .envs(pair.git.vars())
        .env("HOME", &pair.home)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn script");
    let mut stdin = child.stdin.take().expect("stdin");
    let mut stdout = child.stdout.take().expect("stdout");
    let (sender, chunks) = mpsc::channel::<Vec<u8>>();
    let reader = std::thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        loop {
            match stdout.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    if sender.send(buffer[..read].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let mut output = Vec::new();
    let mut asked = false;
    let started = Instant::now();
    let status = loop {
        while let Ok(chunk) = chunks.try_recv() {
            output.extend_from_slice(&chunk);
        }
        if !asked && String::from_utf8_lossy(&output).contains("[y/N]") {
            asked = true;
            let _ = stdin.write_all(b"n\n");
            let _ = stdin.flush();
        }
        if let Some(status) = child.try_wait().expect("wait on script") {
            break status;
        }
        if started.elapsed() > RUN_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("spec {args:?} on a terminal ran longer than {RUN_TIMEOUT:?}; killed");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    drop(stdin);
    reader.join().expect("the reader");
    while let Ok(chunk) = chunks.try_recv() {
        output.extend_from_slice(&chunk);
    }
    let terminal = String::from_utf8(output)
        .expect("UTF-8 terminal output")
        .replace("\r\n", "\n");
    let stderr = fs::read_to_string(&err).unwrap_or_default();
    TtyRun {
        code: status
            .code()
            .expect("script exited, not killed by a signal"),
        asked: asked || terminal.contains("[y/N]"),
        terminal,
        stderr,
    }
}

// ------------------------------------------------------------------ M1

/// M1: spec-a's template gains a `Colour:` block after `{{cost}}`'s, its
/// slot `{{colour}}` in the body, committed in the item's worktree.
/// `spec approve PR-0001` on a terminal exits 2 before the prompt, stderr
/// naming `templates/decision.md:<that line>: `{{colour}}` is no slot: `
/// (approve step 3); nothing written, the item open, no record file. A
/// 0.35 binary meets `{{why}}` so. M: the slot's line taken from the
/// front-matter.
#[test]
fn m1_an_unknown_template_slot_exits_2_at_its_line() {
    let pair = Pair::new("cq-m1-slot", "spec-a");
    let raised = propose_decision(&pair, &pair.linked, &decision_input());
    assert_eq!(raised.code, 0, "{}", raised.show());
    let raised: Value = serde_json::from_str(&raised.stdout).expect("the intake document");
    assert_eq!(raised["id"], json!("PR-0001"), "{raised}");
    assert_eq!(raised["created"], json!(true), "{raised}");

    let template = read_text(&pair.linked, "templates/decision.md");
    let block = "Cost:\n\n{{cost}}\n";
    assert!(template.contains(block), "{template}");
    let unknown = template.replacen(block, "Cost:\n\n{{cost}}\n\nColour:\n\n{{colour}}\n", 1);
    let line = unknown
        .lines()
        .position(|line| line == "{{colour}}")
        .expect("the slot's line")
        + 1;
    // The slot sits in the body, well past the front-matter's 12 lines.
    assert!(line > 12, "{line}");
    write(&pair.linked, "templates/decision.md", &unknown);
    pair.commit_all(&pair.linked, "An unknown slot in the body.");
    let rows = pair.sql("select id, status, record_id from proposals order by id");

    let run = spec_tty(&pair, &pair.main, &["approve", "PR-0001"]);
    assert_eq!(
        run.code, 2,
        "terminal:\n{}\nstderr:\n{}",
        run.terminal, run.stderr
    );
    assert!(!run.asked, "refused before the prompt: {}", run.terminal);
    let named = format!("templates/decision.md:{line}: `{{{{colour}}}}` is no slot: ");
    assert!(
        run.stderr
            .starts_with(&format!("spec: `PR-0001` not applied (step 3): {named}")),
        "{}",
        run.stderr
    );
    assert_eq!(run.stderr.lines().count(), 1, "{}", run.stderr);
    assert!(run.stderr.ends_with('\n'), "{}", run.stderr);
    assert_eq!(
        pair.sql("select id, status, record_id from proposals order by id"),
        rows,
        "the item as it was"
    );
    assert_eq!(rows.trim(), "PR-0001|open|", "{rows}");
    assert!(!pair.linked.join("docs/records/DEC/DEC-0024.md").exists());
    assert!(!pair.main.join("docs/records/DEC/DEC-0024.md").exists());
}

/// M1: a decision's `--input` holding `"colour": 1` exits 2, stdout empty,
/// stderr the input's refusal naming `colour`; nothing stored. A 0.35
/// binary meets this slice's `why` so.
#[test]
fn m1_an_unknown_decision_input_key_exits_2() {
    let pair = Pair::new("cq-m1-input", "spec-a");
    let mut document = decision_input();
    document["colour"] = json!(1);
    let run = propose_decision(&pair, &pair.linked, &document);
    assert_eq!(run.code, 2, "{}", run.show());
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr
            .starts_with("spec: --input -: not a decision's arguments"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr.contains("unknown field `colour`"),
        "{}",
        run.stderr
    );
    assert_eq!(proposal_count(&pair), 0, "nothing stored");

    // Its twin without the key is stored: the key alone was refused.
    let run = propose_decision(&pair, &pair.linked, &decision_input());
    assert_eq!(run.code, 0, "{}", run.show());
    assert_eq!(proposal_count(&pair), 1);
}
