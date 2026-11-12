//! docs/features/intake-similar.md (RM-0059) "Acceptance criteria", the
//! CLI's half: the `spec` binary over scratch git copies of
//! `fixtures/spec-a` (`common::proposal::Pair`: a main worktree, a linked
//! one on `t1`, a scratch `HOME`), every proposal raised in `t1` unless
//! said. The names the tests add (a lantern's oil, an old lamp, a night
//! watch, sixty-five twin lamps) are invented here.
//!
//! - IS1 `is1_*`: the "Text" example (`endurance.md`, `title: STAMINA`,
//!   `--keyword drained`) is not stored: exit 0, `id` null, `similar[0]`
//!   the entry of "The entry", the note first; no row, event or
//!   reservation; with `--distinct-from MEC-STAMINA` it stores `PR-0001`
//!   with a byte-equal `similar`. M: the title probe skipped.
//! - IS2 `is2_*`: a live create and a live amendment titled "Lantern oil"
//!   gate a create titled "lantern  OIL"; a reject frees it. M: the
//!   live-create probe dropped; amendments not read.
//! - IS3 `is3_*`: an `aliases:` entry equal to the title gates. M: aliases
//!   not probed.
//! - IS4 `is4_*`: search and neighbour entries advise and the create
//!   stores. M: search entries gating.
//! - IS5 `is5_*`: path order over rowid order, the dry run, the real call
//!   and a copy at another root and `HOME` byte-equal, the section form's
//!   target and a Tier 3 document never listed. M: order by rowid; the
//!   target not excluded.
//! - IS6 `is6_*`, the CLI's part: clap refuses the flags on the other
//!   kinds; every other answer and `spec review` carry `similar: null`, the
//!   review text no line; each broken cap exits 2 naming the field and its
//!   index before anything is read; sixty-five gating entries list 64 under
//!   the unnameable note.
//! - IS7 `is7_*`: the stored row equals the row of the same create stored
//!   with no entry; its intake columns `NULL`; queue schema 12 and the
//!   backup's format and keys as before. M: `distinct_from` stored.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use common::git::Sandbox;
use common::proposal::{Pair, printed};
use common::{Run, Scratch, read_text, write};
use serde_json::{Value, json};
use specengine_store::{ProposalQueue as _, QUEUE_SCHEMA_VERSION, Reservation};

/// The note of a create a gating entry keeps unstored.
const NOT_STORED: &str = "not stored: a similar entry gates; propose an update of it, or name \
                          each gating entry in `distinct_from` (--distinct-from) to store it \
                          anyway";

/// A dry run's head line.
const DRY_RUN: &str = "dry run: nothing stored";

/// The "Text" example's file.
const ENDURANCE: &str = "docs/spec/movement/endurance.md";

/// MEC-STAMINA's file.
const STAMINA: &str = "docs/spec/movement/stamina.md";

/// "The entry"'s example, as `similar[0]` of the "Text" example.
const STAMINA_ENTRY: &str = "{\"id\":\"MEC-STAMINA\",\"path\":\"docs/spec/movement/stamina.md\",\
                             \"title\":\"Stamina\",\"kind\":\"mechanic\",\"source\":\"title\",\
                             \"reason\":\"title equal to new `MEC-ENDURANCE`'s\",\"gates\":true}";

/// The "Text" example's MEC-STAMINA line.
const STAMINA_LINE: &str = "similar: MEC-STAMINA | title | gates | mechanic | \
                            docs/spec/movement/stamina.md | Stamina | title equal to new \
                            `MEC-ENDURANCE`'s";

/// The "Text" example's answer on spec-a, whole.
const IS1_TEXT: &str = "not stored: a similar entry gates; propose an update of it, or name \
each gating entry in `distinct_from` (--distinct-from) to store it anyway
introduced: 0
similar: MEC-STAMINA | title | gates | mechanic | docs/spec/movement/stamina.md | Stamina | \
title equal to new `MEC-ENDURANCE`'s
similar: Q-032 | search | advises | question | docs/records/Q/Q-032.md | Does the lantern \
drain stamina? | search rank 2 on `stamina`, `drained`
similar: R-12 | search | advises | requirement | docs/records/R/R-12.md | Stamina regenerates \
only at rest | search rank 3 on `stamina`, `drained`
similar: docs/features/stamina-tuning.md | search | advises | - | \
docs/features/stamina-tuning.md | Stamina tuning | search rank 4 on `stamina`, `drained`
similar: DOM-MOVEMENT | search | advises | domain | docs/spec/movement/README.md | Movement | \
search rank 5 on `stamina`, `drained`
similar: MEC-SPRINT | search | advises | mechanic | docs/spec/movement/sprint.md | Sprint | \
search rank 6 on `stamina`, `drained`
";

/// The rationale of every create here.
const RATIONALE: &str = "A new node.";

/// A spec-a canon document: `id`, the title as `title:` and as its H1, the
/// parent, the links block (empty for none) and one paragraph.
fn canon(id: &str, title: &str, parent: &str, links: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\ntier: 2\ntitle: {title}\nparent: {parent}\nstatus: \
         accepted\nowner: owner\nreviewed: 2026-09-20\n{links}---\n\n# {title}\n\nA body of \
         plain words.\n"
    )
}

/// "Text"'s example: `endurance.md`, `id: MEC-ENDURANCE`, `title:
/// STAMINA`, `# STAMINA`, `depends_on: [MEC-SPRINT]`, `derived_from:
/// [R-12]`.
fn endurance() -> String {
    canon(
        "MEC-ENDURANCE",
        "STAMINA",
        "DOM-MOVEMENT",
        "links:\n  depends_on: [MEC-SPRINT]\n  derived_from: [R-12]\n",
    )
}

/// `spec propose create PATH --text-file - --rationale R --author-role
/// writer [extra...]` in `cwd`, `text` on stdin.
fn create(pair: &Pair, cwd: &Path, target: &str, text: &str, extra: &[&str]) -> Run {
    let mut args = vec![
        "propose",
        "create",
        target,
        "--text-file",
        "-",
        "--rationale",
        RATIONALE,
        "--author-role",
        "writer",
    ];
    args.extend_from_slice(extra);
    pair.spec_piped(cwd, &args, text.as_bytes())
}

/// [`create`] with `--json` added: the document.
fn create_json(pair: &Pair, cwd: &Path, target: &str, text: &str, extra: &[&str]) -> (Value, Run) {
    let mut args = extra.to_vec();
    args.push("--json");
    let run = create(pair, cwd, target, text, &args);
    run.code(0);
    (run.json(), run)
}

/// The raw bytes of a document's `similar`: between `"similar":` and
/// `,"notes":`, the key that follows it.
fn raw_similar(stdout: &str) -> String {
    let start = stdout
        .find("\"similar\":")
        .unwrap_or_else(|| panic!("no similar key: {stdout}"))
        + "\"similar\":".len();
    let end = stdout[start..]
        .find(",\"notes\":")
        .unwrap_or_else(|| panic!("`notes` follows `similar`: {stdout}"))
        + start;
    stdout[start..end].to_owned()
}

/// The `similar:` lines of a text answer.
fn similar_lines(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|line| line.starts_with("similar: "))
        .map(str::to_owned)
        .collect()
}

/// The entries of a document's `similar` list.
fn entries(document: &Value) -> Vec<Value> {
    document["similar"]
        .as_array()
        .unwrap_or_else(|| panic!("a list: {document}"))
        .clone()
}

/// `(source, id or path)` of each entry.
fn sources(document: &Value) -> Vec<(String, String)> {
    entries(document)
        .iter()
        .map(|entry| {
            let name = entry["id"]
                .as_str()
                .or_else(|| entry["path"].as_str())
                .expect("an id or a path")
                .to_owned();
            (entry["source"].as_str().expect("a source").to_owned(), name)
        })
        .collect()
}

/// An entry's text line, as "Text" writes it.
fn line_of(entry: &Value) -> String {
    let or_dash = |value: &Value| value.as_str().unwrap_or("-").to_owned();
    format!(
        "similar: {} | {} | {} | {} | {} | {} | {}",
        entry["id"]
            .as_str()
            .or_else(|| entry["path"].as_str())
            .expect("a name"),
        entry["source"].as_str().expect("a source"),
        if entry["gates"] == json!(true) {
            "gates"
        } else {
            "advises"
        },
        or_dash(&entry["kind"]),
        entry["path"].as_str().expect("a path"),
        or_dash(&entry["title"]),
        entry["reason"].as_str().expect("a reason"),
    )
}

/// What a not-stored call must leave: the rows, the events, the
/// reservations (none without a queue).
fn queue_state(pair: &Pair) -> (usize, Option<i64>, Vec<Reservation>) {
    if !pair.db().exists() {
        return (0, None, Vec::new());
    }
    let events = pair.events();
    (
        pair.proposals().len(),
        events.last().map(|event| event.seq),
        pair.queue().reserved().expect("reserved"),
    )
}

/// Writes `files` in `pair`'s linked worktree and commits them there.
fn committed(pair: &Pair, files: &[(&str, String)], message: &str) {
    for (path, text) in files {
        write(&pair.linked, path, text);
    }
    pair.commit_all(&pair.linked, message);
}

// ------------------------------------------------------------------ IS1

/// IS1: the "Text" example's create exits 0, stores nothing (`id` null, no
/// row, event or reservation) and lists "The entry"'s example first, its
/// note first in `notes` and as the head line; a create of the same ID is
/// then not refused as reserved: with `--distinct-from MEC-STAMINA` it
/// stores `PR-0001`, its `similar` byte-equal to the first call's. M: the
/// title probe skipped.
#[test]
fn is1_an_equal_title_gates_until_distinct_from_names_it() {
    let pair = Pair::new("is1", "spec-a");
    let cwd = pair.linked.clone();
    let text = endurance();
    let before = queue_state(&pair);

    let run = create(&pair, &cwd, ENDURANCE, &text, &["--keyword", "drained"]);
    run.code(0);
    let lines: Vec<&str> = run.stdout.lines().collect();
    assert_eq!(lines.first(), Some(&NOT_STORED), "{}", run.show());
    assert_eq!(lines.get(1), Some(&"introduced: 0"), "{}", run.show());
    assert_eq!(lines.get(2), Some(&STAMINA_LINE), "{}", run.show());
    // The whole answer, the search rows as this run fixes them (each reason
    // checked by hand: the five best of `stamina` OR `drained` after
    // MEC-STAMINA, which the title listed and which keeps rank 1).
    assert_eq!(run.stdout, IS1_TEXT, "{}", run.show());
    assert_eq!(run.stderr, "", "{}", run.show());
    assert!(!run.stdout.contains("PR-"), "{}", run.show());
    assert_eq!(queue_state(&pair), before, "nothing stored, nothing held");

    let (document, raw) = create_json(&pair, &cwd, ENDURANCE, &text, &["--keyword", "drained"]);
    assert!(
        raw.stdout
            .contains(&format!("\"similar\":[{STAMINA_ENTRY}")),
        "similar[0] is the example, keys in order: {}",
        raw.stdout
    );
    assert_eq!(
        entries(&document)[0],
        serde_json::from_str::<Value>(STAMINA_ENTRY).unwrap()
    );
    assert_eq!(document["notes"][0], json!(NOT_STORED), "{document}");
    for key in [
        "id",
        "status",
        "base_commit",
        "preview",
        "revisions",
        "created_at",
        "updated_at",
    ] {
        assert_eq!(document[key], Value::Null, "{key}: {document}");
    }
    assert_eq!(document["kind"], json!("create"));
    assert_eq!(document["target_id"], json!("MEC-ENDURANCE"));
    assert_eq!(document["distinct_from"], json!([]), "the row holds none");
    // Every entry once, one text line each, in the list's order; MEC-STAMINA
    // the one that gates (it shares two targets too, but is listed by title).
    let listed = entries(&document);
    assert_eq!(
        listed.iter().map(line_of).collect::<Vec<_>>(),
        similar_lines(&run.stdout),
        "{}",
        run.stdout
    );
    assert_eq!(
        listed
            .iter()
            .filter(|entry| entry["gates"] == json!(true))
            .count(),
        1,
        "{document}"
    );
    assert!(
        listed[1..]
            .iter()
            .all(|entry| entry["source"] == json!("search") && entry["gates"] == json!(false)),
        "the rest search advice: {document}"
    );
    assert!(listed.len() <= 6, "at most 5 search entries: {document}");
    assert_eq!(queue_state(&pair), before, "nothing stored, nothing held");

    // A rehearsal naming it lists the same, byte for byte.
    let rehearsal = create(
        &pair,
        &cwd,
        ENDURANCE,
        &text,
        &[
            "--keyword",
            "drained",
            "--distinct-from",
            "MEC-STAMINA",
            "--dry-run",
        ],
    );
    rehearsal.code(0);
    assert_eq!(rehearsal.stdout.lines().next(), Some(DRY_RUN));
    assert_eq!(
        similar_lines(&rehearsal.stdout),
        similar_lines(&run.stdout),
        "{}",
        rehearsal.show()
    );

    let (stored, stored_run) = create_json(
        &pair,
        &cwd,
        ENDURANCE,
        &text,
        &["--keyword", "drained", "--distinct-from", "MEC-STAMINA"],
    );
    assert_eq!(stored["id"], json!("PR-0001"), "{stored}");
    assert_eq!(stored["status"], json!("open"));
    assert_eq!(raw_similar(&stored_run.stdout), raw_similar(&raw.stdout));
    assert!(
        !stored["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|note| note.as_str().is_some_and(|n| n.starts_with("not stored"))),
        "{stored}"
    );
    assert_eq!(pair.proposals().len(), 1);
    assert_eq!(pair.proposal("PR-0001").target_id, "MEC-ENDURANCE");
}

// ------------------------------------------------------------------ IS2

/// A spec-a mechanic under `DOM-GAME` titled `title`.
fn lamp_part(id: &str, title: &str) -> String {
    canon(id, title, "DOM-GAME", "")
}

const OIL: &str = "docs/spec/keep/oil.md";
const FUEL: &str = "docs/spec/keep/fuel.md";

/// IS2: a live create titled "Lantern oil" gates a create titled "lantern
/// OIL" (two spaces), listed as `{id: "PR-0001", source: "queue", gates:
/// true}` with its reason, and nothing is stored; once `PR-0001` is
/// rejected the create stores with no queue entry. M: the live-create probe
/// dropped.
#[test]
fn is2_a_live_create_gates_until_it_is_rejected() {
    let pair = Pair::new("is2-create", "spec-a");
    let cwd = pair.linked.clone();
    let first = create(
        &pair,
        &cwd,
        OIL,
        &lamp_part("MEC-LANTERN-OIL", "Lantern oil"),
        &[],
    );
    first.code(0);
    assert_eq!(
        first.stdout.lines().next(),
        Some("PR-0001"),
        "{}",
        first.show()
    );

    let fuel = lamp_part("MEC-LAMP-FUEL", "lantern  OIL");
    let before = queue_state(&pair);
    let (document, raw) = create_json(&pair, &cwd, FUEL, &fuel, &[]);
    assert_eq!(document["id"], Value::Null, "{document}");
    assert_eq!(document["notes"][0], json!(NOT_STORED), "{document}");
    let queued: Vec<&Value> = document["similar"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["source"] == json!("queue"))
        .collect();
    assert_eq!(
        queued,
        [&json!({
            "id": "PR-0001", "path": OIL, "title": "Lantern oil", "kind": "mechanic",
            "source": "queue",
            "reason": "`PR-0001` (open), a live create, adds `MEC-LANTERN-OIL` titled as new \
                       `MEC-LAMP-FUEL`",
            "gates": true})],
        "{}",
        raw.stdout
    );
    assert_eq!(queue_state(&pair), before, "nothing stored");

    let (outcome, _) = pair.reject_answer(&pair.main, "PR-0001", "x", true);
    outcome.expect("PR-0001 rejected");
    let (stored, _) = create_json(&pair, &cwd, FUEL, &fuel, &[]);
    assert_eq!(stored["id"], json!("PR-0002"), "{stored}");
    assert!(
        sources(&stored).iter().all(|(source, _)| source != "queue"),
        "a rejected create lists nothing: {stored}"
    );
}

/// IS2, the amendment: a live amendment whose new file is titled "Lantern
/// oil" gates the create alike, named by its path (its document has no
/// ID). M: amendments not read.
#[test]
fn is2_a_live_amendment_gates_alike() {
    let pair = Pair::new("is2-amendment", "spec-a");
    let cwd = pair.linked.clone();
    let amendment = "---\nclass: spec\nstatus: draft\nscope: [keep]\nlinks:\n  amends: \
                     [RULE-STAM-REGEN]\n---\n\n# Lantern oil\n\n## Burn time {#AC-01}\n\nOne \
                     flask burns a night.\n";
    let path = "docs/features/lantern-oil.md";
    let run = pair.spec_piped(
        &cwd,
        &[
            "propose",
            "amendment",
            path,
            "--text-file",
            "-",
            "--rationale",
            RATIONALE,
            "--author-role",
            "writer",
        ],
        amendment.as_bytes(),
    );
    run.code(0);
    assert_eq!(run.stdout.lines().next(), Some("PR-0001"), "{}", run.show());

    let before = queue_state(&pair);
    let fuel = lamp_part("MEC-LAMP-FUEL", "lantern  OIL");
    let (document, raw) = create_json(&pair, &cwd, FUEL, &fuel, &[]);
    assert_eq!(document["id"], Value::Null, "{document}");
    let queued: Vec<&Value> = document["similar"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["source"] == json!("queue"))
        .collect();
    assert_eq!(
        queued,
        [&json!({
            "id": "PR-0001", "path": path, "title": "Lantern oil", "kind": null,
            "source": "queue",
            "reason": "`PR-0001` (open), a live amendment, adds `docs/features/lantern-oil.md` \
                       titled as new `MEC-LAMP-FUEL`",
            "gates": true})],
        "{}",
        raw.stdout
    );
    assert_eq!(queue_state(&pair), before, "nothing stored");
    let (stored, _) = create_json(&pair, &cwd, FUEL, &fuel, &["--distinct-from", "PR-0001"]);
    assert_eq!(stored["id"], json!("PR-0002"), "{stored}");
}

/// A spec-a mechanic whose front-matter title holds a U+0000 (YAML's
/// `"\0"` escape), its H1 the same words without it.
fn nul_titled(id: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\ntier: 2\ntitle: \"Lan\\0tern oil\"\nparent: DOM-GAME\n\
         status: accepted\nowner: owner\nreviewed: 2026-09-20\n---\n\n# Lantern oil\n\nA body \
         of plain words.\n"
    )
}

/// IS1 and IS2, iteration 2 of the review: a U+0000 in a new node's title
/// splits its search terms (`lan`, `tern`, `oil`) instead of ending the FTS5
/// string (exit 2 before): the create stores, its `similar` search advice
/// alone; a second create of that title is kept unstored by the first, a
/// live create, its text line writing the title `Lan\u{0}tern oil`. M: the
/// U+0000 split removed (`unterminated string`).
#[test]
fn is1_a_nul_in_a_title_splits_its_terms_and_the_create_stores() {
    let pair = Pair::new("is1-nul", "spec-a");
    let cwd = pair.linked.clone();
    let (document, raw) = create_json(&pair, &cwd, OIL, &nul_titled("MEC-LANTERN-OIL"), &[]);
    assert_eq!(document["id"], json!("PR-0001"), "{}", raw.show());
    let listed = entries(&document);
    assert!(!listed.is_empty(), "`lan` finds the lantern: {document}");
    for (rank, entry) in listed.iter().enumerate() {
        assert_eq!(entry["source"], json!("search"), "{entry}");
        assert_eq!(entry["gates"], json!(false), "{entry}");
        assert_eq!(
            entry["reason"],
            json!(format!("search rank {} on `lan`, `tern`, `oil`", rank + 1)),
            "{entry}"
        );
    }

    let second = nul_titled("MEC-LAMP-OIL");
    let path = "docs/spec/keep/lamp-oil.md";
    let (document, raw) = create_json(&pair, &cwd, path, &second, &[]);
    assert_eq!(document["id"], Value::Null, "{}", raw.show());
    assert_eq!(
        entries(&document)[0],
        json!({"id": "PR-0001", "path": OIL, "title": "Lan\u{0}tern oil", "kind": "mechanic",
            "source": "queue",
            "reason": "`PR-0001` (open), a live create, adds `MEC-LANTERN-OIL` titled as new \
                       `MEC-LAMP-OIL`",
            "gates": true}),
        "{}",
        raw.stdout
    );
    let text = create(&pair, &cwd, path, &second, &[]);
    text.code(0);
    assert_eq!(
        text.stdout.lines().next(),
        Some(NOT_STORED),
        "{}",
        text.show()
    );
    let line = similar_lines(&text.stdout)
        .into_iter()
        .find(|line| line.starts_with("similar: PR-0001 | queue | gates |"))
        .unwrap_or_else(|| panic!("the queue line: {}", text.show()));
    assert!(
        line.contains(" | Lan\\u{0}tern oil | "),
        "the title escaped: {line}"
    );
    assert!(!line.contains('\0'), "{line:?}");
    assert_eq!(pair.proposals().len(), 1);
}

// ------------------------------------------------------------------ IS3

/// IS3: a committed document with `aliases: ["Old lamp"]`; a create titled
/// "Old Lamp" lists it with `source: alias`, gating, and is not stored. M:
/// aliases not probed.
#[test]
fn is3_an_alias_equal_to_the_title_gates() {
    let pair = Pair::new("is3", "spec-a");
    let cwd = pair.linked.clone();
    let lamp = canon("MEC-LAMP", "Lamp", "DOM-GAME", "aliases: [\"Old lamp\"]\n");
    committed(&pair, &[("docs/spec/keep/lamp.md", lamp)], "a lamp");
    let before = queue_state(&pair);
    let (document, raw) = create_json(
        &pair,
        &cwd,
        "docs/spec/keep/old-lamp.md",
        &lamp_part("MEC-OLD-LAMP", "Old Lamp"),
        &[],
    );
    assert_eq!(document["id"], Value::Null, "{document}");
    assert_eq!(document["notes"][0], json!(NOT_STORED));
    assert_eq!(
        entries(&document)[0],
        json!({"id": "MEC-LAMP", "path": "docs/spec/keep/lamp.md", "title": "Lamp",
            "kind": "mechanic", "source": "alias",
            "reason": "alias `Old lamp` equal to new `MEC-OLD-LAMP`'s title", "gates": true}),
        "{}",
        raw.stdout
    );
    assert_eq!(
        entries(&document)
            .iter()
            .filter(|entry| entry["gates"] == json!(true))
            .count(),
        1,
        "{document}"
    );
    assert_eq!(queue_state(&pair), before, "nothing stored");
}

// ------------------------------------------------------------------ IS4

/// IS4: a create titled "Stamina regeneration curve" with `--keyword
/// drained` lists only search entries, at most five, none gating, and
/// stores with no `distinct_from`; a mechanic titled "Endurance pool"
/// sharing MEC-STAMINA's two targets lists it as a neighbour naming both,
/// and stores. M: search entries gating.
#[test]
fn is4_search_and_neighbour_entries_advise_and_the_create_stores() {
    let pair = Pair::new("is4", "spec-a");
    let cwd = pair.linked.clone();
    let curve = canon(
        "MEC-REGEN-CURVE",
        "Stamina regeneration curve",
        "DOM-MOVEMENT",
        "",
    );
    let (document, raw) = create_json(
        &pair,
        &cwd,
        "docs/spec/movement/curve.md",
        &curve,
        &["--keyword", "drained"],
    );
    assert_eq!(document["id"], json!("PR-0001"), "{}", raw.show());
    let listed = entries(&document);
    assert!(!listed.is_empty(), "search finds stamina: {document}");
    assert!(listed.len() <= 5, "{document}");
    for entry in &listed {
        assert_eq!(entry["source"], json!("search"), "{entry}");
        assert_eq!(entry["gates"], json!(false), "{entry}");
        let reason = entry["reason"].as_str().unwrap();
        assert!(reason.starts_with("search rank "), "{reason}");
    }
    // Ranks are 1.. in list order.
    for (rank, entry) in listed.iter().enumerate() {
        let reason = entry["reason"].as_str().unwrap();
        assert!(
            reason.starts_with(&format!("search rank {} on `", rank + 1)),
            "{reason}"
        );
    }
    assert!(
        !document["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|note| note.as_str().is_some_and(|n| n.starts_with("not stored"))),
        "{document}"
    );

    let pool = canon(
        "MEC-ENDURANCE-POOL",
        "Endurance pool",
        "DOM-MOVEMENT",
        "links:\n  depends_on: [MEC-SPRINT]\n  derived_from: [R-12]\n",
    );
    let (document, raw) = create_json(&pair, &cwd, "docs/spec/movement/pool.md", &pool, &[]);
    assert_eq!(document["id"], json!("PR-0002"), "{}", raw.show());
    let neighbours: Vec<Value> = entries(&document)
        .into_iter()
        .filter(|entry| entry["source"] == json!("neighbours"))
        .collect();
    assert_eq!(neighbours.len(), 1, "{document}");
    let entry = &neighbours[0];
    assert_eq!(entry["id"], json!("MEC-STAMINA"));
    assert_eq!(entry["path"], json!(STAMINA));
    assert_eq!(entry["kind"], json!("mechanic"));
    assert_eq!(entry["gates"], json!(false));
    let reason = entry["reason"].as_str().unwrap();
    assert!(
        reason.starts_with("same kind `mechanic`, 2 link targets shared: "),
        "{reason}"
    );
    assert!(
        reason.contains("`MEC-SPRINT`") && reason.contains("`R-12`"),
        "{reason}"
    );
    assert!(
        entries(&document)
            .iter()
            .all(|entry| entry["gates"] == json!(false)),
        "{document}"
    );
}

// ------------------------------------------------------------------ IS5

/// The night watch: two documents sharing a title and a Tier 3 one (a
/// shipped feature) holding it too.
const NIGHT_WATCH: &str = "Night watch";

fn night_watch_files() -> Vec<(&'static str, String)> {
    vec![
        (
            "docs/spec/a.md",
            canon("DOM-ALPHA", NIGHT_WATCH, "DOM-GAME", ""),
        ),
        (
            "docs/spec/b.md",
            canon("DOM-BETA", NIGHT_WATCH, "DOM-GAME", ""),
        ),
        (
            "docs/features/night-watch.md",
            format!(
                "---\nclass: spec\nstatus: shipped\nscope: [keep]\n---\n\n# {NIGHT_WATCH}\n\n\
                 The watch was tuned.\n"
            ),
        ),
    ]
}

/// The night watch's pair: the files committed in `t1`, indexed, then
/// `a.md` edited so its rows are the newer ones.
fn night_watch(label: &str) -> Pair {
    let pair = Pair::new(label, "spec-a");
    committed(&pair, &night_watch_files(), "the night watch");
    pair.spec_piped(&pair.linked, &["index"], b"").code(0);
    let a = read_text(&pair.linked, "docs/spec/a.md");
    write(
        &pair.linked,
        "docs/spec/a.md",
        a.replace("A body of plain words.", "A body of plain words, edited."),
    );
    pair.spec_piped(&pair.linked, &["index"], b"").code(0);
    pair
}

/// The create of IS5: a new document titled "Night watch".
fn watch() -> String {
    canon("DOM-WATCH", NIGHT_WATCH, "DOM-GAME", "")
}

const WATCH: &str = "docs/spec/watch.md";

/// IS5: path order (`a.md`, then `b.md`) though `a.md`'s rows are newer;
/// the Tier 3 document never listed; the dry run, the real call (not
/// stored) and a copy at another root and `HOME` give byte-equal
/// `similar`, JSON and text; stored once both are named. M: order by rowid.
#[test]
fn is5_path_order_and_one_list_for_one_state() {
    let pair = night_watch("is5");
    let cwd = pair.linked.clone();
    let text = watch();
    let rehearsed = create(&pair, &cwd, WATCH, &text, &["--dry-run"]);
    rehearsed.code(0);
    assert_eq!(rehearsed.stdout.lines().next(), Some(DRY_RUN));
    let (rehearsed_json, rehearsed_raw) = create_json(&pair, &cwd, WATCH, &text, &["--dry-run"]);
    let real = create(&pair, &cwd, WATCH, &text, &[]);
    real.code(0);
    assert_eq!(
        real.stdout.lines().next(),
        Some(NOT_STORED),
        "{}",
        real.show()
    );
    let (real_json, real_raw) = create_json(&pair, &cwd, WATCH, &text, &[]);

    let titled: Vec<(String, String)> = sources(&real_json)
        .into_iter()
        .filter(|(source, _)| source == "title")
        .collect();
    assert_eq!(
        titled,
        [
            ("title".to_owned(), "DOM-ALPHA".to_owned()),
            ("title".to_owned(), "DOM-BETA".to_owned())
        ],
        "{real_json}"
    );
    assert_eq!(
        entries(&real_json)[..2]
            .iter()
            .map(|entry| entry["path"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        ["docs/spec/a.md", "docs/spec/b.md"]
    );
    assert!(
        entries(&real_json)
            .iter()
            .all(|entry| entry["path"] != json!("docs/features/night-watch.md")),
        "a Tier 3 document is never an entry: {real_json}"
    );
    assert_eq!(
        raw_similar(&rehearsed_raw.stdout),
        raw_similar(&real_raw.stdout)
    );
    assert_eq!(rehearsed_json["similar"], real_json["similar"]);
    assert_eq!(
        similar_lines(&rehearsed.stdout),
        similar_lines(&real.stdout)
    );
    assert_eq!(
        rehearsed_json["notes"],
        json!([DRY_RUN, NOT_STORED]),
        "the dry run names the gate after its own note"
    );

    // Another root, another HOME: the same list, byte for byte.
    let other = night_watch("is5-other");
    assert_ne!(other.linked, pair.linked);
    assert_ne!(other.home, pair.home);
    let elsewhere = create(&other, &other.linked, WATCH, &text, &[]);
    elsewhere.code(0);
    assert_eq!(
        similar_lines(&elsewhere.stdout),
        similar_lines(&real.stdout)
    );
    let (_, elsewhere_raw) = create_json(&other, &other.linked, WATCH, &text, &[]);
    assert_eq!(
        raw_similar(&elsewhere_raw.stdout),
        raw_similar(&real_raw.stdout)
    );

    // Both named: stored, the list unchanged.
    let (stored, stored_raw) = create_json(
        &pair,
        &cwd,
        WATCH,
        &text,
        &[
            "--distinct-from",
            "DOM-ALPHA",
            "--distinct-from",
            "DOM-BETA",
        ],
    );
    assert_eq!(stored["id"], json!("PR-0001"), "{stored}");
    assert_eq!(
        raw_similar(&stored_raw.stdout),
        raw_similar(&real_raw.stdout)
    );
}

/// IS5, the section form: new sections titled as the target node (and the
/// target holding the search's words) never list the target, so the create
/// stores. M: the target not excluded.
#[test]
fn is5_a_section_form_create_never_lists_its_target() {
    let pair = Pair::new("is5-sections", "spec-a");
    let cwd = pair.linked.clone();
    let (hash, span) = pair.span(&cwd, "MEC-STAMINA");
    let text = format!("{span}\n## Stamina {{#RULE-STAM-CAP}}\n- Capped at 100 units.\n");
    let (document, raw) = create_json(
        &pair,
        &cwd,
        "MEC-STAMINA",
        &text,
        &["--base", &hash, "--keyword", "drained"],
    );
    assert_eq!(document["id"], json!("PR-0001"), "{}", raw.show());
    assert!(
        entries(&document)
            .iter()
            .all(|entry| entry["id"] != json!("MEC-STAMINA")),
        "the target is never an entry: {document}"
    );
}

// ------------------------------------------------------------------ IS6

/// IS6: the other kinds take neither flag (clap's exit 2, nothing stored).
#[test]
fn is6_the_other_kinds_take_neither_flag() {
    let pair = Pair::new("is6-flags", "spec-a");
    let cwd = pair.linked.clone();
    let (hash, span) = pair.span(&cwd, "EDGE-STAM-ZERO");
    let text = span.replace("immediately", "at once");
    let amendment = "---\nclass: spec\nstatus: draft\nscope: [keep]\nlinks:\n  amends: \
                     [RULE-STAM-REGEN]\n---\n\n# Rest\n\n## Wait {#AC-01}\n\nA wait.\n";
    for flag in [["--distinct-from", "MEC-STAMINA"], ["--keyword", "drained"]] {
        let mut runs = Vec::new();
        let mut update = vec![
            "propose",
            "update",
            "EDGE-STAM-ZERO",
            "--base",
            &hash,
            "--text-file",
            "-",
            "--rationale",
            RATIONALE,
        ];
        update.extend_from_slice(&flag);
        runs.push(pair.spec_piped(&cwd, &update, text.as_bytes()));
        let mut interpretation = vec![
            "propose",
            "interpretation",
            "EDGE-STAM-ZERO",
            "--base",
            &hash,
            "--decision",
            "DEC-0023",
            "--how",
            "At once.",
            "--why",
            "Rest.",
            "--text-file",
            "-",
            "--rationale",
            RATIONALE,
        ];
        interpretation.extend_from_slice(&flag);
        runs.push(pair.spec_piped(&cwd, &interpretation, text.as_bytes()));
        let mut amend = vec![
            "propose",
            "amendment",
            "docs/features/rest.md",
            "--text-file",
            "-",
            "--rationale",
            RATIONALE,
        ];
        amend.extend_from_slice(&flag);
        runs.push(pair.spec_piped(&cwd, &amend, amendment.as_bytes()));
        for run in runs {
            assert_eq!(run.code, 2, "{:?}: {}", run.args, run.show());
            assert!(run.stderr.contains(flag[0]), "{}", run.show());
            assert_eq!(run.stdout, "", "{}", run.show());
        }
    }
    assert!(pair.proposals().is_empty());
}

/// IS6: an update's, an interpretation's and an amendment's answers, a
/// review (text and JSON), an approve and a reject carry `similar: null`;
/// the review text has no `similar` line; a stored create's review too.
#[test]
fn is6_every_other_document_carries_similar_null() {
    let pair = Pair::new("is6-null", "spec-a");
    let cwd = pair.linked.clone();
    let (hash, span) = pair.span(&cwd, "EDGE-STAM-ZERO");
    let update = pair.spec_piped(
        &cwd,
        &[
            "propose",
            "update",
            "EDGE-STAM-ZERO",
            "--base",
            &hash,
            "--text-file",
            "-",
            "--rationale",
            RATIONALE,
            "--json",
        ],
        span.replace("immediately", "at once").as_bytes(),
    );
    update.code(0);
    assert_eq!(update.json()["id"], json!("PR-0001"));
    assert_eq!(update.json()["similar"], Value::Null, "{}", update.stdout);
    assert!(
        update
            .stdout
            .contains("\"why\":null,\"similar\":null,\"notes\":")
    );

    let (regen_hash, regen) = pair.span(&cwd, "RULE-STAM-REGEN");
    let interpretation = pair.spec_piped(
        &cwd,
        &[
            "propose",
            "interpretation",
            "RULE-STAM-REGEN",
            "--base",
            &regen_hash,
            "--decision",
            "DEC-0023",
            "--how",
            "Rest is any speed under 40 % of walking.",
            "--why",
            "DEC-0023 says rest; the rule names no speed.",
            "--text-file",
            "-",
            "--rationale",
            RATIONALE,
            "--author-role",
            "spec-author",
            "--json",
        ],
        regen
            .replace(
                "Base rate 10 units/s,",
                "Base rate 10 units/s at rest (under 40 % of walking),",
            )
            .as_bytes(),
    );
    interpretation.code(0);
    assert_eq!(interpretation.json()["id"], json!("PR-0002"));
    assert_eq!(interpretation.json()["similar"], Value::Null);

    let amendment = pair.spec_piped(
        &cwd,
        &[
            "propose",
            "amendment",
            "docs/features/rest.md",
            "--text-file",
            "-",
            "--rationale",
            RATIONALE,
            "--json",
        ],
        b"---\nclass: spec\nstatus: draft\nscope: [keep]\nlinks:\n  amends: \
          [EDGE-SPRINT-EMPTY]\n---\n\n# Rest\n\n## Wait {#AC-01}\n\nA wait.\n",
    );
    amendment.code(0);
    assert_eq!(amendment.json()["id"], json!("PR-0003"));
    assert_eq!(amendment.json()["similar"], Value::Null);

    let (created, _) = create_json(
        &pair,
        &cwd,
        ENDURANCE,
        &endurance(),
        &["--distinct-from", "MEC-STAMINA"],
    );
    assert_eq!(created["id"], json!("PR-0004"));
    assert!(created["similar"].is_array());

    for id in ["PR-0001", "PR-0002", "PR-0003", "PR-0004"] {
        let review = pair.spec_piped(&pair.main, &["review", id, "--json"], b"");
        review.code(0);
        assert_eq!(
            review.json()["similar"],
            Value::Null,
            "{id}: {}",
            review.stdout
        );
        let brief = pair.spec_piped(&pair.main, &["review", id, "--brief", "--json"], b"");
        brief.code(0);
        assert_eq!(
            brief.json()["similar"],
            Value::Null,
            "{id}: {}",
            brief.stdout
        );
        let text = pair.spec_piped(&pair.main, &["review", id], b"");
        text.code(0);
        assert!(
            !text.stdout.lines().any(|line| line.starts_with("similar")),
            "{id}: {}",
            text.stdout
        );
    }

    let approved = pair.approve_ok(&pair.main, "PR-0001");
    let (text, raw) = printed(&approved);
    assert!(
        raw.contains("\"why\":null,\"similar\":null,\"notes\":"),
        "{raw}"
    );
    assert!(
        !text.lines().any(|line| line.starts_with("similar")),
        "{text}"
    );
    let (rejected, _) = pair.reject_answer(&pair.main, "PR-0004", "x", true);
    let (text, raw) = printed(&rejected.expect("rejected"));
    assert!(
        raw.contains("\"why\":null,\"similar\":null,\"notes\":"),
        "{raw}"
    );
    assert!(
        !text.lines().any(|line| line.starts_with("similar")),
        "{text}"
    );
}

/// IS6: each broken cap exits 2 with `spec: <field>[<i>]: <problem>`, no
/// document and nothing read past the form (no data directory is made);
/// the caps themselves are accepted.
#[test]
fn is6_each_broken_cap_exits_2_naming_the_field_and_its_index() {
    let pair = Pair::new("is6-caps", "spec-a");
    let cwd = pair.linked.clone();
    let text = endurance();
    let seventeen: Vec<String> = (0..17).map(|n| format!("word{n:02}")).collect();
    let sixty_five: Vec<String> = (0..65).map(|n| format!("MEC-TWIN-{n:02}")).collect();
    let long = "X".repeat(257);
    let mut cases: Vec<(&str, Vec<String>, &str)> = vec![
        (
            "a two-character keyword",
            vec!["--keyword".into(), "ab".into()],
            "spec: keywords[0]: ",
        ),
        (
            "a keyword with a space",
            vec!["--keyword".into(), "two words".into()],
            "spec: keywords[0]: ",
        ),
        (
            "a keyword twice",
            vec![
                "--keyword".into(),
                "drained".into(),
                "--keyword".into(),
                "drained".into(),
            ],
            "spec: keywords[1]: ",
        ),
        (
            "a 257-byte distinct_from",
            vec!["--distinct-from".into(), long.clone()],
            "spec: distinct_from[0]: ",
        ),
    ];
    let mut flags = Vec::new();
    for word in &seventeen {
        flags.push("--keyword".to_owned());
        flags.push(word.clone());
    }
    cases.push(("17 keywords", flags, "spec: keywords[16]: "));
    let mut flags = Vec::new();
    for name in &sixty_five {
        flags.push("--distinct-from".to_owned());
        flags.push(name.clone());
    }
    cases.push(("65 distinct_from", flags, "spec: distinct_from[64]: "));
    let mut problems = Vec::new();
    for (label, flags, head) in &cases {
        let extra: Vec<&str> = flags.iter().map(String::as_str).collect();
        let run = create(&pair, &cwd, ENDURANCE, &text, &extra);
        assert_eq!(run.code, 2, "{label}: {}", run.show());
        assert_eq!(run.stdout, "", "{label}: no document");
        let line = run.stderr.lines().next().unwrap_or_default();
        if !line.starts_with(head) {
            problems.push(format!("{label}: want {head:?}..., got {line:?}"));
        }
        assert!(
            !pair.db().exists(),
            "{label}: nothing read past the form: {}",
            run.show()
        );
    }
    // At the caps: 16 keywords of 3 and 64 bytes, 64 names of 256 bytes.
    let mut at_caps: Vec<String> = Vec::new();
    for n in 0..15 {
        at_caps.push("--keyword".into());
        at_caps.push(format!("word{n:02}"));
    }
    at_caps.push("--keyword".into());
    at_caps.push("abc".into());
    for n in 0..63 {
        at_caps.push("--distinct-from".into());
        at_caps.push(format!("MEC-TWIN-{n:02}"));
    }
    at_caps.push("--distinct-from".into());
    at_caps.push("Y".repeat(256));
    at_caps.push("--dry-run".into());
    let extra: Vec<&str> = at_caps.iter().map(String::as_str).collect();
    let run = create(&pair, &cwd, ENDURANCE, &text, &extra);
    run.code(0);
    assert_eq!(run.stdout.lines().next(), Some(DRY_RUN), "{}", run.show());
    let wide = format!("{}x", "k".repeat(64));
    let run = create(&pair, &cwd, ENDURANCE, &text, &["--keyword", &wide]);
    assert_eq!(run.code, 2, "a 65-byte keyword: {}", run.show());
    assert!(
        run.stderr.starts_with("spec: keywords[0]: "),
        "{}",
        run.show()
    );
    assert_eq!(problems, Vec::<String>::new(), "the field and its index");
}

/// IS6: sixty-five documents titled "Twin lamp" gate a create of that
/// title: 64 entries listed, by path, under the unnameable note, and
/// naming all 64 still stores nothing.
#[test]
fn is6_sixty_five_gating_entries_list_64_under_the_unnameable_note() {
    let pair = Pair::new("is6-twins", "spec-a");
    let cwd = pair.linked.clone();
    let files: Vec<(String, String)> = (1..=65)
        .map(|n| {
            (
                format!("docs/spec/twins/{n:02}.md"),
                canon(&format!("MEC-TWIN-{n:02}"), "Twin lamp", "DOM-GAME", ""),
            )
        })
        .collect();
    let borrowed: Vec<(&str, String)> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.clone()))
        .collect();
    committed(&pair, &borrowed, "sixty-five twins");
    let text = canon("MEC-TWIN-NEW", "Twin lamp", "DOM-GAME", "");
    let unnameable = "not stored: 65 similar entries gate and `distinct_from` names at most 64, \
                      so this create cannot be stored (a known limit)";
    let (document, raw) = create_json(&pair, &cwd, "docs/spec/twins/new.md", &text, &[]);
    assert_eq!(document["id"], Value::Null, "{document}");
    assert_eq!(document["notes"][0], json!(unnameable), "{document}");
    let gating: Vec<String> = entries(&document)
        .iter()
        .filter(|entry| entry["gates"] == json!(true))
        .map(|entry| entry["id"].as_str().unwrap().to_owned())
        .collect();
    let want: Vec<String> = (1..=64).map(|n| format!("MEC-TWIN-{n:02}")).collect();
    assert_eq!(gating, want, "{}", raw.stdout);
    let text_run = create(&pair, &cwd, "docs/spec/twins/new.md", &text, &[]);
    text_run.code(0);
    assert_eq!(text_run.stdout.lines().next(), Some(unnameable));

    let mut named = Vec::new();
    for id in &want {
        named.push("--distinct-from");
        named.push(id.as_str());
    }
    let (document, _) = create_json(&pair, &cwd, "docs/spec/twins/new.md", &text, &named);
    assert_eq!(document["id"], Value::Null, "{document}");
    assert_eq!(document["notes"][0], json!(unnameable));
    assert!(pair.proposals().is_empty());
}

// ------------------------------------------------------------------ IS7

/// The columns of a stored row, by name, every value as the `sqlite3`
/// client's `quote()` prints it.
fn row(pair: &Pair, id: &str) -> BTreeMap<String, String> {
    let columns: Vec<String> = pair
        .sql("SELECT name FROM pragma_table_info('proposals') ORDER BY cid;")
        .lines()
        .map(str::to_owned)
        .collect();
    assert!(columns.len() > 40, "{columns:?}");
    columns
        .iter()
        .map(|column| {
            let value = pair.sql(&format!(
                "SELECT quote(\"{column}\") FROM proposals WHERE id = '{id}';"
            ));
            (column.clone(), value.trim_end_matches('\n').to_owned())
        })
        .collect()
}

/// A [`Pair`] built at exactly `at` (the IS7 twins share their paths, so
/// their rows' `worktree` and `git_common_dir` agree).
fn pair_at(at: &Path) -> Pair {
    let scratch = Scratch::at(at);
    let git = Sandbox::new(scratch.path());
    let main = scratch.copy("spec-a", "main");
    git.init(&main);
    git.add_all(&main);
    git.commit(&main, "the fixture");
    let linked = scratch.join("t1");
    git.git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "t1",
            linked.to_str().expect("a UTF-8 scratch path"),
        ],
    );
    let linked = fs::canonicalize(linked).expect("the linked worktree");
    let home = scratch.home("h");
    Pair {
        scratch,
        git,
        main,
        linked,
        home,
        slug: "lantern-keep".to_owned(),
    }
}

/// The backup's proposal line's keys, as the format of before writes them
/// (48: the table's columns).
const BACKUP_KEYS: [&str; 50] = [
    "id",
    "project",
    "kind",
    "status",
    "target_id",
    "target_path",
    "git_common_dir",
    "worktree",
    "root_rel",
    "branch",
    "base_commit",
    "base_hash",
    "base_text",
    "new_text",
    "patch_hash",
    "rationale",
    "author",
    "diagnostics",
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
    "working_answer",
    "price_of_other",
    "evidence",
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
    "drift",
    "drift_hash",
    "card_id",
    "why",
    "gist",
    "matters",
];

/// The intake columns a create leaves empty.
const INTAKE: [&str; 10] = [
    "severity",
    "gap_type",
    "summary",
    "working_answer",
    "price_of_other",
    "evidence",
    "options",
    "recommendation",
    "distinct_from",
    "linked",
];

/// IS7: the "Text" example stored with `--distinct-from` and two
/// `--keyword`s, and the same create stored at the same paths once the
/// corpus node is renamed (no entry), give rows equal column by column but
/// `id` and the times; the intake columns `NULL`; queue schema 12; `spec
/// export state` writes format 6 with the 48 keys of before. M:
/// `distinct_from` stored.
#[test]
fn is7_nothing_new_is_stored() {
    // 13 since docs/features/plain-questions.md PQ8.
    assert_eq!(QUEUE_SCHEMA_VERSION, 14);
    let at = std::env::temp_dir().join(format!("specengine-cli-is7-{}", std::process::id()));
    let named = {
        let pair = pair_at(&at);
        let (document, _) = create_json(
            &pair,
            &pair.linked,
            ENDURANCE,
            &endurance(),
            &[
                "--distinct-from",
                "MEC-STAMINA",
                "--keyword",
                "drained",
                "--keyword",
                "rested",
            ],
        );
        assert_eq!(document["id"], json!("PR-0001"), "{document}");
        assert!(
            sources(&document).contains(&("title".to_owned(), "MEC-STAMINA".to_owned())),
            "{document}"
        );
        let row = row(&pair, "PR-0001");
        for column in INTAKE {
            assert_eq!(row[column], "NULL", "{column}: {row:?}");
        }
        assert_eq!(row["target_ids"], "'[\"MEC-ENDURANCE\"]'");
        let out = pair.scratch.join("state.jsonl");
        let export = pair.spec_piped(
            &pair.main,
            &["export", "state", "--out", out.to_str().unwrap()],
            b"",
        );
        export.code(0);
        let state = fs::read_to_string(&out).expect("the backup");
        let mut lines = state.lines();
        let header: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
        // Format 7, schema 13 since docs/features/plain-questions.md PQ8, 14
        // since docs/features/collision-pass.md D6.
        assert_eq!(header["format"], json!(7), "{header}");
        assert_eq!(header["queue_schema"], json!(14), "{header}");
        let line: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
        let keys: Vec<&str> = line["proposals"]
            .as_object()
            .expect("a proposal line")
            .keys()
            .map(String::as_str)
            .collect();
        let mut want = BACKUP_KEYS.to_vec();
        want.sort_unstable();
        let mut got = keys.clone();
        got.sort_unstable();
        assert_eq!(got, want, "{line}");
        assert_eq!(line["proposals"]["distinct_from"], Value::Null);
        let columns = pair.sql("SELECT count(*) FROM pragma_table_info('proposals');");
        // 50 since docs/features/plain-questions.md PQ8 (its two, none of
        // this slice's).
        assert_eq!(columns.trim(), "50", "no column added");
        row
    };
    let plain = {
        let pair = pair_at(&at);
        let stamina = read_text(&pair.linked, STAMINA);
        write(
            &pair.linked,
            STAMINA,
            stamina
                .replace("title: Stamina\n", "title: Stamina meter\n")
                .replace("# Stamina\n", "# Stamina meter\n"),
        );
        let (document, _) = create_json(&pair, &pair.linked, ENDURANCE, &endurance(), &[]);
        assert_eq!(document["id"], json!("PR-0001"), "{document}");
        assert!(
            sources(&document)
                .iter()
                .all(|(source, _)| source != "title"),
            "no entry gates: {document}"
        );
        row(&pair, "PR-0001")
    };
    let _ = fs::remove_dir_all(&at);
    assert_eq!(
        named.keys().collect::<Vec<_>>(),
        plain.keys().collect::<Vec<_>>()
    );
    let differing: Vec<&String> = named
        .keys()
        .filter(|column| !["id", "created_at", "updated_at"].contains(&column.as_str()))
        .filter(|column| named[*column] != plain[*column])
        .collect();
    assert_eq!(
        differing,
        Vec::<&String>::new(),
        "columns differ: {named:?}\n{plain:?}"
    );
}

/// The fixture's root, for a reader of this file: every test copies it.
#[allow(dead_code)]
fn fixture_root() -> PathBuf {
    common::fixture("spec-a")
}
