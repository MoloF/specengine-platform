//! docs/features/card-trace.md, the CLI half: `spec card`, `spec trace`,
//! `spec impact` through the binary on scratch git repositories of
//! `fixtures/card-press` (a print shop: five documents under `PRN-WEEKLY`,
//! `SHT-BACK` using `SHT-FRONT-HEAD`, the accepted `STY-0001` constraining
//! `SHT-FRONT`), and of `fixtures/spec-a`, `-b`, `-c` for the genre. The store
//! half is the store's `queue_trace.rs`, the tool `mcp_trace.rs`, the daemon
//! http `trace.rs`.
//!
//! - AC-01: a meaning change marks, whitespace does not, a clear removes the
//!   mark and the next meaning change restores it. M: comparing `span_hash`.
//! - AC-02: `spec approve --editorial` keeps a card clean, one `card.refreshed`
//!   each, from a worktree without the commit too; a completion refreshes; a
//!   hand edit marks. M: the apply refreshing nothing.
//! - AC-03: `confirmed` only with a card, all accepted or cancelled, one
//!   accepted, none suspect or departed; nothing written. M: confirming with
//!   no card.
//! - AC-04: `--departs`: one `open` discrepancy, the card accepted, the node
//!   `departed`; a settled item gives `confirmed`; a failure injected after
//!   the insert leaves neither. M: the insert in its own transaction.
//! - AC-05: a dedup hit never refuses the accept. M: the dedup's refusal kept.
//! - AC-06: needs correction. M: re-freezing at the clear.
//! - AC-07: `clear --group`. M: a bulk clear ignoring `parent:`.
//! - AC-08: `uses` carries no mark. M: marking through `uses`.
//! - AC-09: exit 0 whatever is listed, `spec check` and `spec gate` unchanged,
//!   `spec impact`, the re-estimate, a piped `clear`, a root with no `.git`.
//! - "Rules and edge cases", the output's forms, the scope, the cap, the
//!   genre.
//!
//! Every expected value below is worked out from the spec, never read back
//! from a run. Non-Latin characters in Rust sources are Unicode escapes
//! (ADR-0024).

#![cfg(unix)]

mod common;
mod task_common;

use std::fs;
use std::path::Path;

use common::decision::{PtyRun, flags, with_records};
use common::proposal::{CASES, LATER, Pair};
use common::{
    FIXTURES, NON_RUST, Run, Scratch, read, read_text, replace, spec_with, with_ids_line, write,
};
use serde_json::{Value, json};
use specengine_cli::{
    ApproveFlags, ApproveRequest, Env, Exit, Globals, StageBody, StageRequest, approve_editorial,
};
use specengine_core::intake::{Evidence, GapType, IntakeSeverity};
use specengine_model::CardFrozen;
use specengine_store::{NewCard, ProposalKind, ProposalStatus};
use task_common::Ordered;

const PRESS: &str = "card-press";
const WEEKLY: &str = "PRN-WEEKLY";
const PLATES: &str = "PRN-PLATES";
const FRONT: &str = "SHT-FRONT";
const HEAD: &str = "SHT-FRONT-HEAD";
const BACK: &str = "SHT-BACK";
const INNER: &str = "SHT-INNER";
const FOLD: &str = "SHT-INNER-FOLD";
const RULING: &str = "STY-0001";
const FRONT_MD: &str = "docs/spec/sht-front.md";
const BACK_MD: &str = "docs/spec/sht-back.md";
const INNER_MD: &str = "docs/spec/sht-inner.md";
const RULING_MD: &str = "docs/records/STY/STY-0001.md";

/// A change of `SHT-FRONT`'s own meaning, and a later one on top of it.
const MEANING: (&str, &str) = (
    "the masthead and the lead story",
    "the masthead, the lead story and the weather",
);
const MEANING_AGAIN: (&str, &str) = ("and the weather", "and the tide");

/// A trailing space, which `norm_hash` trims and `span_hash` keeps. An inner
/// double space would not do: the recipe keeps those.
const WHITESPACE: (&str, &str) = ("the lead story.\n", "the lead story.  \n");

/// A change of `SHT-FRONT-HEAD`'s meaning.
const HEAD_MEANING: (&str, &str) = (
    "stays the same from week to week",
    "changes with the season",
);

/// A change of `SHT-INNER`'s own meaning.
const INNER_MEANING: (&str, &str) = ("carries the long reads", "carries the short reads");

/// A change of spec-a's feature check `AC-07`.
const CHECK_FROM: &str = "first regeneration tick 1.5 s later";
const CHECK_TO: &str = "first regeneration tick 2 s later";

/// `spec trace --json`'s document, in order; `stages` since
/// docs/features/trace-views.md.
const TRACE_KEYS: [&str; 8] = [
    "ref",
    "reason",
    "notes",
    "counts",
    "stages",
    "nodes",
    "truncated",
    "omitted",
];

/// A node of it; `stage` and `distance` since docs/features/trace-views.md.
const NODE_KEYS: [&str; 9] = [
    "id", "title", "path", "stage", "distance", "rollup", "counts", "cards", "uses",
];

/// A card of a node.
const CARD_KEYS: [&str; 8] = [
    "id",
    "flow",
    "column",
    "title",
    "mark",
    "on",
    "discrepancy",
    "correction",
];

/// A `uses` row.
const USES_KEYS: [&str; 3] = ["id", "rollup", "cards"];

/// The five states' counts.
const COUNT_KEYS: [&str; 5] = ["no_card", "in_work", "suspect", "departed", "confirmed"];

/// An item's summary, observed and documented text.
const SUMMARY: &str = "The result left the brief.";
const OBSERVED: &str = "The lead runs below the fold.";
const DOCUMENTED: &str = "The lead runs above the fold.";

// ------------------------------------------------------------- helpers

/// `spec args` in the main worktree: an empty pipe on stdin, the pair's
/// scratch `HOME`.
fn run(pair: &Pair, args: &[&str]) -> Run {
    pair.spec_piped(&pair.main, args, b"")
}

/// Exit 0: stdout.
fn ok(pair: &Pair, args: &[&str]) -> String {
    let done = run(pair, args);
    done.code(0);
    done.stdout
}

/// `spec --json args`, whatever its exit.
fn json_run(pair: &Pair, args: &[&str]) -> Run {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    run(pair, &all)
}

/// `spec --json args`, exit 0: the document.
fn doc(pair: &Pair, args: &[&str]) -> Value {
    let done = json_run(pair, args);
    done.code(0);
    done.json()
}

/// A card of the `copy` flow on `targets`: its ID.
fn make(pair: &Pair, title: &str, targets: &[&str]) -> String {
    let mut args = vec!["card", "new", "--title", title, "--discipline", "copy"];
    for target in targets {
        args.extend_from_slice(&["--target", target]);
    }
    doc(pair, &args)["id"]
        .as_str()
        .expect("a card's ID")
        .to_owned()
}

/// `show --json` of `id`.
fn show(pair: &Pair, id: &str) -> Value {
    doc(pair, &["card", "show", id])
}

/// The card at `proof`: `ana` its assignee, `ben` its reviewer.
fn to_proof(pair: &Pair, id: &str) {
    ok(
        pair,
        &[
            "card",
            "assign",
            id,
            "--assignee",
            "ana",
            "--reviewer",
            "ben",
        ],
    );
    ok(pair, &["card", "move", id, "--to", "edit", "--as", "desk"]);
    ok(pair, &["card", "move", id, "--to", "proof"]);
}

/// `spec card accept id --as ben flags` on a terminal answering `y`.
fn accept(pair: &Pair, id: &str, flags: &[&str]) -> PtyRun {
    let mut args = vec!["card", "accept", id, "--as", "ben"];
    args.extend_from_slice(flags);
    pair.spec_pty(&pair.main, &args, Some("y"))
}

/// [`accept`], exit 0.
fn accept_ok(pair: &Pair, id: &str, flags: &[&str]) -> String {
    let done = accept(pair, id, flags);
    assert_eq!(done.code, 0, "accept {id} {flags:?}: {}", done.output);
    done.output
}

/// A card accepted `--briefed` on `targets`: its ID.
fn accepted(pair: &Pair, title: &str, targets: &[&str]) -> String {
    let id = make(pair, title, targets);
    to_proof(pair, &id);
    accept_ok(pair, &id, &["--briefed"]);
    id
}

/// The flags of a departure from `nodes`.
fn departing<'a>(nodes: &[&'a str]) -> Vec<&'a str> {
    let mut flags = Vec::new();
    for node in nodes {
        flags.extend_from_slice(&["--departs", node]);
    }
    flags.extend_from_slice(&[
        "--summary",
        SUMMARY,
        "--observed",
        OBSERVED,
        "--documented",
        DOCUMENTED,
    ]);
    flags
}

/// `spec card clear id --as ben flags` on a terminal answering `y`.
fn clear(pair: &Pair, id: &str, flags: &[&str]) -> PtyRun {
    let mut args = vec!["card", "clear", id];
    args.extend_from_slice(flags);
    args.extend_from_slice(&["--as", "ben"]);
    pair.spec_pty(&pair.main, &args, Some("y"))
}

/// `spec card clear id --no-effect`, exit 0.
fn clear_ok(pair: &Pair, id: &str, reason: &str) -> String {
    let done = clear(pair, id, &["--no-effect", "--reason", reason]);
    assert_eq!(done.code, 0, "clear {id}: {}", done.output);
    done.output
}

/// `(from, to)` on `target` proposed and approved in the main worktree: the
/// proposal's ID.
fn approved(pair: &Pair, target: &str, (from, to): (&str, &str)) -> String {
    let id = pair.propose_edit(&pair.main, target, from, to);
    pair.approve_ok(&pair.main, &id);
    id
}

/// `spec approve id --editorial` in the main worktree, which must apply.
fn approve_editorially(pair: &Pair, id: &str) {
    let mut consent = |_: &str| true;
    let outcome = approve_editorial(
        &pair.env(&pair.main),
        &Globals::default(),
        &ApproveRequest {
            id: id.to_owned(),
            note: None,
            now: LATER.to_owned(),
            git: pair.git_env(&pair.main),
        },
        &ApproveFlags::default(),
        &mut consent,
    )
    .unwrap_or_else(|error| panic!("approve {id} --editorial: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
}

/// [`approved`] under `--editorial`.
fn editorial(pair: &Pair, target: &str, (from, to): (&str, &str)) -> String {
    let id = pair.propose_edit(&pair.main, target, from, to);
    approve_editorially(pair, &id);
    id
}

/// `spec show`'s `norm_hash` of `id` in the main worktree.
fn norm(pair: &Pair, id: &str) -> String {
    pair.node(&pair.main, id).norm_hash.expect("a norm hash")
}

/// `SHT-FRONT`'s `within_hash` now: it holds one section.
fn front_within(pair: &Pair) -> String {
    pair.within(&[FRONT, HEAD])
}

/// `spec --json trace [reference]`, exit 0: the document.
fn trace(pair: &Pair, reference: Option<&str>) -> Value {
    trace_in(pair, &pair.main, reference)
}

/// [`trace`] in `cwd`.
fn trace_in(pair: &Pair, cwd: &Path, reference: Option<&str>) -> Value {
    let mut args = vec!["--json", "trace"];
    args.extend(reference);
    let done = pair.spec_piped(cwd, &args, b"");
    done.code(0);
    done.json()
}

/// The node `id` of a trace document.
fn node_of<'a>(document: &'a Value, id: &str) -> &'a Value {
    document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|node| node["id"] == id)
        .unwrap_or_else(|| panic!("no node {id}: {document}"))
}

/// The card `id` of a trace node.
fn card_of<'a>(node: &'a Value, id: &str) -> &'a Value {
    node["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .find(|card| card["id"] == id)
        .unwrap_or_else(|| panic!("no card {id}: {node}"))
}

/// The IDs of a trace document's nodes.
fn node_ids(document: &Value) -> Vec<String> {
    document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| node["id"].as_str().expect("an ID").to_owned())
        .collect()
}

/// `id`'s state.
fn rollup(pair: &Pair, id: &str) -> String {
    rollup_in(pair, &pair.main, id)
}

/// [`rollup`] in `cwd`.
fn rollup_in(pair: &Pair, cwd: &Path, id: &str) -> String {
    node_of(&trace_in(pair, cwd, Some(id)), id)["rollup"]
        .as_str()
        .expect("a rollup")
        .to_owned()
}

/// The line `spec trace reference` prints for the card `id`.
fn card_line(pair: &Pair, cwd: &Path, reference: &str, id: &str) -> String {
    let done = pair.spec_piped(cwd, &["trace", reference], b"");
    done.code(0);
    let head = format!("  card {id} | ");
    done.stdout
        .lines()
        .find(|line| line.starts_with(&head))
        .unwrap_or_else(|| panic!("no line of {id}: {}", done.show()))
        .to_owned()
}

/// `(type, payload)` of every `card.*` event, in `seq` order.
fn events(pair: &Pair) -> Vec<(String, Value)> {
    pair.events()
        .into_iter()
        .filter(|event| event.event_type.starts_with("card."))
        .map(|event| (event.event_type, event.payload))
        .collect()
}

/// The payloads of `kind`, in `seq` order.
fn events_of(pair: &Pair, kind: &str) -> Vec<Value> {
    events(pair)
        .into_iter()
        .filter(|(event, _)| event == kind)
        .map(|(_, payload)| payload)
        .collect()
}

/// The status with ignored files, `HEAD`, the number of commits.
fn git_side(pair: &Pair) -> (String, String, String) {
    (
        pair.git_text(&pair.main, &["status", "--porcelain", "--ignored"]),
        pair.rev(&pair.main, "HEAD"),
        pair.git_text(&pair.main, &["rev-list", "--all", "--count"]),
    )
}

/// What a refusal must leave: the cards' rows raw, every row of the queue.
fn state(pair: &Pair) -> (String, String) {
    (
        pair.sql("SELECT * FROM cards ORDER BY id;"),
        pair.queue().dump().expect("dump"),
    )
}

/// Appends `extra` to the root's config.
fn append_config(root: &Path, extra: &str) {
    let mut text = read_text(root, "specengine.toml");
    text.push_str(extra);
    write(root, "specengine.toml", text);
}

/// `count` accepted rulings past `STY-0001`, each constraining `SHT-FRONT`,
/// committed: their IDs.
fn rulings(pair: &Pair, count: usize) -> Vec<String> {
    let mut ids = Vec::new();
    for number in 2..count + 2 {
        let id = format!("STY-{number:04}");
        write(
            &pair.main,
            &format!("docs/records/STY/{id}.md"),
            format!(
                "---\nid: {id}\nclass: decision\nstatus: accepted\ndate: 2026-10-01\ncanon: \
                 docs/spec/sht-front.md#the-front-sheet\nlinks:\n  constrains: [SHT-FRONT]\n\
                 scope: [front-sheet]\n---\n\n# Ruling {number}\n\nThe front sheet keeps rule \
                 {number}.\n"
            ),
        );
        ids.push(id);
    }
    pair.commit_all(&pair.main, "the rulings");
    ids
}

// --------------------------------------------------------------- AC-01

/// "Snapshot and suspect": `new` and `edit --target` freeze each direct
/// target at `spec show`'s `norm_hash`, `also` empty; a manual card freezes
/// nothing; a target given as a path is frozen under its node's ID.
#[test]
fn ac01_new_and_edit_target_freeze_each_direct_target() {
    let pair = Pair::new("ct-freeze", PRESS);
    let id = make(&pair, "Lead story", &[FRONT, "docs/spec/sht-back.md"]);
    let card = show(&pair, &id);
    let frozen = card["snapshot"].as_array().expect("a snapshot");
    assert_eq!(frozen.len(), 2, "{card}");
    for (entry, node) in frozen.iter().zip([FRONT, BACK]) {
        assert_eq!(entry["id"], node, "{card}");
        assert_eq!(entry["norm_hash"], norm(&pair, node), "{card}");
        assert_eq!(entry["also"], json!([]), "{card}");
        assert_eq!(
            entry.as_object().expect("an entry").len(),
            5,
            "id, rev, norm_hash, within_hash, also: {entry}"
        );
    }
    assert_eq!(frozen[0]["within_hash"], front_within(&pair), "{card}");
    assert_eq!(frozen[1]["within_hash"], norm(&pair, BACK), "{card}");

    ok(&pair, &["card", "edit", &id, "--target", HEAD]);
    let card = show(&pair, &id);
    assert_eq!(card["targets"], json!([HEAD]));
    let frozen = card["snapshot"].as_array().expect("a snapshot");
    assert_eq!(
        frozen.len(),
        3,
        "the targets, then what was dropped: {card}"
    );
    assert_eq!(frozen[0]["id"], HEAD);
    assert_eq!(frozen[0]["norm_hash"], norm(&pair, HEAD));
    assert_eq!(frozen[1]["id"], FRONT, "dropped, kept after `targets`");
    assert_eq!(frozen[2]["id"], BACK, "dropped, kept after `targets`");

    // A dropped entry is inert: its node's change marks nothing.
    approved(&pair, FRONT, MEANING);
    let shown = trace(&pair, Some(HEAD));
    assert_eq!(
        card_of(node_of(&shown, HEAD), &id)["mark"],
        "open",
        "{shown}"
    );
    assert_eq!(rollup(&pair, FRONT), "in work", "the card on its section");

    let manual = make(&pair, "A manual card", &[]);
    let card = show(&pair, &manual);
    assert_eq!(card["targets"], json!([]));
    assert_eq!(card["snapshot"], Value::Null, "nothing to freeze");
}

/// "Snapshot and suspect": no edit takes a mark away: a target dropped keeps
/// its entry, inert, and finds its old hash when it returns, so two edits of
/// `--target` leave the card suspect; so does clearing the list and naming it
/// again. M: `retargeted` dropping a removed target's entry.
#[test]
fn two_edits_cannot_clear_a_mark() {
    let pair = Pair::new("ct-two-edits", PRESS);
    let id = make(&pair, "Lead story", &[FRONT]);
    let old = show(&pair, &id)["snapshot"][0].clone();
    approved(&pair, FRONT, MEANING);
    assert_eq!(rollup(&pair, FRONT), "suspect");

    ok(&pair, &["card", "edit", &id, "--target", BACK]);
    assert_eq!(rollup(&pair, FRONT), "no card", "dropped: inert");
    ok(&pair, &["card", "edit", &id, "--target", FRONT]);
    let card = show(&pair, &id);
    assert_eq!(card["snapshot"][0], old, "its old hash: {card}");
    assert_eq!(card["snapshot"][1]["id"], BACK, "the other dropped: {card}");
    assert_eq!(rollup(&pair, FRONT), "suspect", "the mark is back");

    ok(&pair, &["card", "edit", &id, "--target", "-"]);
    assert_eq!(show(&pair, &id)["targets"], json!([]));
    ok(&pair, &["card", "edit", &id, "--target", FRONT]);
    assert_eq!(rollup(&pair, FRONT), "suspect", "through an empty list");
    assert_eq!(events_of(&pair, "card.cleared"), Vec::<Value>::new());
}

/// AC-01: on `fixtures/card-press`, approving a whitespace change leaves the
/// card clean though `span_hash` moved; a meaning change marks it; a clear
/// with a reason removes the mark, one `card.cleared` naming the node with
/// its revision; the next meaning change restores it. `git status` stays
/// empty and no commit is made by a card's call. M: comparing `span_hash`
/// turns the whitespace case red.
#[test]
fn ac01_a_meaning_change_marks_whitespace_does_not_a_clear_removes_it() {
    let pair = Pair::new("ct-ac01", PRESS);
    let id = make(&pair, "Lead story", &[FRONT]);
    let line = |mark: &str| format!("  card {id} | copy/draft | {mark} | Lead story");
    assert_eq!(card_line(&pair, &pair.main, FRONT, &id), line("open"));
    assert_eq!(rollup(&pair, FRONT), "in work");

    // Whitespace: `norm_hash` equal, `span_hash` not, so the case would turn
    // red on the other hash.
    let frozen = norm(&pair, FRONT);
    let held = front_within(&pair);
    let span = pair.span(&pair.main, FRONT).0;
    approved(&pair, FRONT, WHITESPACE);
    assert_eq!(norm(&pair, FRONT), frozen, "the fixture's premise");
    assert_ne!(
        pair.span(&pair.main, FRONT).0,
        span,
        "the fixture's premise"
    );
    assert_eq!(card_line(&pair, &pair.main, FRONT, &id), line("open"));
    assert_eq!(rollup(&pair, FRONT), "in work");

    // A meaning change.
    approved(&pair, FRONT, MEANING);
    let changed = norm(&pair, FRONT);
    assert_ne!(changed, frozen);
    assert_eq!(
        card_line(&pair, &pair.main, FRONT, &id),
        line("suspect SHT-FRONT")
    );
    assert_eq!(rollup(&pair, FRONT), "suspect");
    let shown = trace(&pair, Some(FRONT));
    let card = card_of(node_of(&shown, FRONT), &id);
    assert_eq!(card["mark"], "suspect", "{card}");
    assert_eq!(card["on"], FRONT, "{card}");
    assert_eq!(card["discrepancy"], Value::Null, "{card}");
    assert_eq!(card["correction"], Value::Null, "{card}");

    // The clear.
    let git = git_side(&pair);
    let done = clear(
        &pair,
        &id,
        &["--no-effect", "--reason", "Only the weather."],
    );
    assert_eq!(done.code, 0, "{}", done.output);
    assert!(
        done.output
            .contains("clear C-0001 (Lead story) as ben: no effect? [y/N]"),
        "{}",
        done.output
    );
    assert_eq!(
        git_side(&pair),
        git,
        "a clear touches no file and no commit"
    );
    assert_eq!(card_line(&pair, &pair.main, FRONT, &id), line("open"));
    assert_eq!(rollup(&pair, FRONT), "in work");
    assert_eq!(
        events_of(&pair, "card.cleared"),
        [json!({
            "id": id,
            "outcome": "no_effect",
            "nodes": [{"id": FRONT, "rev": 2, "from": held, "to": front_within(&pair)}],
            "correction": null,
            "reason": "Only the weather.",
            "as": "ben",
        })]
    );
    let card = show(&pair, &id);
    assert_eq!(card["snapshot"][0]["norm_hash"], changed, "{card}");
    assert_eq!(card["snapshot"][0]["rev"], 2, "{card}");

    // The next meaning change restores it.
    approved(&pair, FRONT, MEANING_AGAIN);
    assert_eq!(
        card_line(&pair, &pair.main, FRONT, &id),
        line("suspect SHT-FRONT")
    );
    assert_eq!(rollup(&pair, FRONT), "suspect");
}

/// AC-01 on an accepted card: suspect like any other, cleared to `confirmed`;
/// a card moves and a suspect one is accepted as any other (ADR-0012).
#[test]
fn a_suspect_card_moves_and_is_accepted_as_any_other() {
    let pair = Pair::new("ct-moves", PRESS);
    let done = accepted(&pair, "Lead story", &[FRONT]);
    assert_eq!(rollup(&pair, FRONT), "confirmed");
    approved(&pair, FRONT, MEANING);
    assert_eq!(rollup(&pair, FRONT), "suspect");
    clear_ok(&pair, &done, "No effect.");
    assert_eq!(rollup(&pair, FRONT), "confirmed");

    let open = make(&pair, "Side story", &[INNER]);
    approved(&pair, INNER, INNER_MEANING);
    assert_eq!(rollup(&pair, INNER), "suspect");
    to_proof(&pair, &open);
    assert_eq!(rollup(&pair, INNER), "suspect", "a move clears nothing");
    accept_ok(&pair, &open, &["--briefed"]);
    assert_eq!(
        rollup(&pair, INNER),
        "confirmed",
        "an accept re-freezes the snapshot"
    );
    let card = show(&pair, &open);
    assert_eq!(
        card["snapshot"][0]["norm_hash"],
        norm(&pair, INNER),
        "{card}"
    );
    assert_eq!(card["acceptance"]["verdict"], "briefed", "{card}");
    assert_eq!(card["acceptance"]["departs"], json!([]), "{card}");
    assert_eq!(card["acceptance"]["discrepancy"], Value::Null, "{card}");
    let last = events_of(&pair, "card.accepted").pop().expect("an accept");
    assert_eq!(last["verdict"], "briefed", "{last}");
    assert_eq!(last["discrepancy"], Value::Null, "{last}");
}

/// "Clearing": exit 1 on a card not suspect, a declined prompt, a stale
/// `--revision`, a cancelled card, no such card; nothing changed each time.
#[test]
fn a_clear_of_nothing_is_refused() {
    let pair = Pair::new("ct-clear-refused", PRESS);
    let clean = make(&pair, "Clean", &[BACK]);
    let id = make(&pair, "Lead story", &[FRONT]);
    let gone = make(&pair, "Pulled", &[FRONT]);
    ok(&pair, &["card", "cancel", &gone, "--reason", "Pulled."]);
    approved(&pair, FRONT, MEANING);
    let before = state(&pair);

    for card in [clean.as_str(), gone.as_str(), "C-0099"] {
        let done = clear(&pair, card, &["--no-effect", "--reason", "Nothing."]);
        assert_eq!(done.code, 1, "{card}: {}", done.output);
        assert_eq!(state(&pair), before, "{card}");
    }
    let declined = pair.spec_pty(
        &pair.main,
        &[
            "card",
            "clear",
            &id,
            "--no-effect",
            "--reason",
            "Nothing.",
            "--as",
            "ben",
        ],
        Some("n"),
    );
    assert!(declined.asked, "{}", declined.output);
    assert_eq!(declined.code, 1, "{}", declined.output);
    assert_eq!(state(&pair), before, "a declined prompt");
    let stale = clear(
        &pair,
        &id,
        &["--no-effect", "--reason", "Nothing.", "--revision", "9"],
    );
    assert_eq!(stale.code, 1, "{}", stale.output);
    assert_eq!(state(&pair), before, "a stale revision");
}

/// "Clearing": the verdict is one of `--no-effect`, `--needs-correction`, and
/// `--as`, `--reason` are required; a group takes no effect only, no `C`
/// beside it: each exits 2, nothing changed.
#[test]
fn a_clear_takes_one_outcome() {
    let pair = Pair::new("ct-clear-usage", PRESS);
    let id = accepted(&pair, "Lead story", &[FRONT]);
    approved(&pair, FRONT, MEANING);
    let before = state(&pair);
    for args in [
        vec!["card", "clear", &id, "--reason", "R.", "--as", "ben"],
        vec![
            "card",
            "clear",
            &id,
            "--no-effect",
            "--needs-correction",
            "--reason",
            "R.",
            "--estimate",
            "3",
            "--as",
            "ben",
        ],
        vec!["card", "clear", &id, "--no-effect", "--as", "ben"],
        vec!["card", "clear", &id, "--no-effect", "--reason", "R."],
        vec![
            "card",
            "clear",
            &id,
            "--needs-correction",
            "--reason",
            "R.",
            "--as",
            "ben",
        ],
        vec![
            "card",
            "clear",
            "--group",
            WEEKLY,
            "--needs-correction",
            "--reason",
            "R.",
            "--estimate",
            "3",
            "--as",
            "ben",
        ],
        vec![
            "card",
            "clear",
            &id,
            "--group",
            WEEKLY,
            "--no-effect",
            "--reason",
            "R.",
            "--as",
            "ben",
        ],
        vec![
            "card",
            "clear",
            "--no-effect",
            "--reason",
            "R.",
            "--as",
            "ben",
        ],
    ] {
        let done = pair.spec_pty(&pair.main, &args, Some("y"));
        assert_eq!(done.code, 2, "{args:?}: {}", done.output);
        assert_eq!(state(&pair), before, "{args:?}");
    }
}

/// "Clearing": a person's act: `agent` and `local` are no one's name, a blank
/// reason says nothing: exit 1, nothing changed, the words a clear's, of a card
/// and of a group.
#[test]
fn a_clear_needs_a_person_and_a_reason() {
    let pair = Pair::new("ct-clear-person", PRESS);
    let id = make(&pair, "Lead story", &[FRONT]);
    approved(&pair, FRONT, MEANING);
    let before = state(&pair);
    for (name, reason, why) in [
        (
            "agent",
            "R.",
            "--as: `agent` clears no mark: a person answers one; nothing changed",
        ),
        (
            "local",
            "R.",
            "--as: `local` clears no mark: a person answers one; nothing changed",
        ),
        ("ben", "   ", "reason: blank; say why the mark clears"),
    ] {
        for what in [["clear", id.as_str()], ["clear", "--group"]] {
            let mut args = vec!["card", what[0], what[1]];
            if what[1] == "--group" {
                args.push(WEEKLY);
            }
            args.extend_from_slice(&["--no-effect", "--reason", reason, "--as", name]);
            let done = pair.spec_pty(&pair.main, &args, Some("y"));
            assert_eq!(done.code, 1, "{args:?}: {}", done.output);
            assert!(done.output.contains(why), "{args:?}: {}", done.output);
            assert!(
                !done.output.contains("accepts a result"),
                "an accept's words: {}",
                done.output
            );
            assert_eq!(state(&pair), before, "{args:?}");
        }
    }
}

/// "Marks read the reading root": a clear judges the root it runs in: a card
/// suspect on a branch is not suspect on `main`, and only the branch's clear
/// runs.
#[test]
fn a_clear_reads_its_reading_root() {
    let pair = Pair::new("ct-clear-root", PRESS);
    let id = make(&pair, "Lead story", &[FRONT]);
    let (from, to) = MEANING;
    replace(&pair.linked, FRONT_MD, from, to);
    pair.commit_all(&pair.linked, "on the branch");
    let before = state(&pair);
    let refused = clear(&pair, &id, &["--no-effect", "--reason", "Nothing here."]);
    assert_eq!(refused.code, 1, "{}", refused.output);
    assert_eq!(state(&pair), before);
    let done = pair.spec_pty(
        &pair.linked,
        &[
            "card",
            "clear",
            &id,
            "--no-effect",
            "--reason",
            "On the branch.",
            "--as",
            "ben",
        ],
        Some("y"),
    );
    assert_eq!(done.code, 0, "{}", done.output);
    assert_eq!(rollup_in(&pair, &pair.linked, FRONT), "in work");
    assert_eq!(
        rollup_in(&pair, &pair.main, FRONT),
        "suspect",
        "frozen at the branch's text now"
    );
}

// --------------------------------------------------------------- AC-02

/// AC-02: `spec approve --editorial` on a meaning change: each card holding
/// the old hash stays clean, one `card.refreshed` each and none for a card on
/// another node, the new hash added to `also`, the entry's own hash kept, so
/// the linked worktree, which lacks the commit, stays clean too. The revision
/// stays. M: the apply refreshing nothing.
#[test]
fn ac02_an_editorial_approve_refreshes_each_card_holding_the_old_hash() {
    let pair = Pair::new("ct-ac02", PRESS);
    let first = make(&pair, "Lead story", &[FRONT]);
    let second = make(&pair, "Side story", &[FRONT, BACK]);
    let other = make(&pair, "Long read", &[INNER]);
    let before = norm(&pair, FRONT);
    let proposal = editorial(&pair, FRONT, MEANING);
    let after = norm(&pair, FRONT);
    let vouched = front_within(&pair);
    assert_ne!(before, after, "the fixture's premise: the text changed");
    assert_eq!(pair.node(&pair.main, FRONT).rev, None, "no raise");
    assert!(
        pair.git_text(&pair.main, &["log", "-1", "--format=%B"])
            .contains("Revision: SHT-FRONT 1 (editorial)"),
        "the trailer a completion reads"
    );

    assert_eq!(
        events_of(&pair, "card.refreshed"),
        [
            json!({"id": first, "node": FRONT, "proposal": proposal}),
            json!({"id": second, "node": FRONT, "proposal": proposal}),
        ]
    );
    for id in [&first, &second] {
        let card = show(&pair, id);
        let entry = &card["snapshot"][0];
        assert_eq!(entry["id"], FRONT, "{card}");
        assert_eq!(entry["norm_hash"], before, "`also`, not a replace: {card}");
        assert_eq!(
            entry["also"],
            json!([vouched]),
            "the kind it compares: {card}"
        );
    }
    assert_eq!(
        show(&pair, &other)["snapshot"][0]["also"],
        json!([]),
        "another node's card"
    );
    for cwd in [&pair.main, &pair.linked] {
        for id in [&first, &second] {
            let line = card_line(&pair, cwd, FRONT, id);
            assert!(line.contains(" | open | "), "{cwd:?}: {line}");
        }
        assert_eq!(rollup_in(&pair, cwd, FRONT), "in work", "{cwd:?}");
    }

    // A second editorial approve: `also` grows, oldest first.
    editorial(&pair, FRONT, MEANING_AGAIN);
    let latest = front_within(&pair);
    assert_eq!(
        show(&pair, &first)["snapshot"][0]["also"],
        json!([vouched, latest])
    );
    assert_eq!(rollup(&pair, FRONT), "in work");
    assert_eq!(
        events_of(&pair, "card.refreshed").len(),
        4,
        "two cards, two applies"
    );

    // A meaning approve vouches for nothing.
    approved(&pair, FRONT, ("and the tide", "and the moon"));
    assert_eq!(rollup(&pair, FRONT), "suspect");
    assert_eq!(events_of(&pair, "card.refreshed").len(), 4);
}

/// AC-02: one change of a file over two nodes: one `card.refreshed` per entry,
/// each node's own hashes; the card clean on both.
#[test]
fn ac02_an_editorial_apply_refreshes_each_changed_node() {
    let pair = Pair::new("ct-ac02-two", PRESS);
    let id = make(&pair, "Front and masthead", &[FRONT, HEAD]);
    let (front, head) = (norm(&pair, FRONT), norm(&pair, HEAD));
    let (span, text) = pair.span(&pair.main, FRONT);
    let new = text
        .replace(MEANING.0, MEANING.1)
        .replace(HEAD_MEANING.0, HEAD_MEANING.1);
    let proposal = pair
        .propose(&pair.main, FRONT, &span, &new)
        .expect("a proposal")
        .document
        .id
        .clone()
        .expect("an ID");
    approve_editorially(&pair, &proposal);
    assert_eq!(
        events_of(&pair, "card.refreshed"),
        [
            json!({"id": id, "node": FRONT, "proposal": proposal}),
            json!({"id": id, "node": HEAD, "proposal": proposal}),
        ]
    );
    let card = show(&pair, &id);
    assert_eq!(card["snapshot"][0]["norm_hash"], front);
    assert_eq!(card["snapshot"][0]["also"], json!([front_within(&pair)]));
    assert_eq!(card["snapshot"][1]["norm_hash"], head);
    assert_eq!(card["snapshot"][1]["also"], json!([norm(&pair, HEAD)]));
    assert_eq!(rollup(&pair, FRONT), "in work");
}

/// AC-02, a staged `editorial: true` confirmed by a bare `spec approve`: the
/// owner's editorial word whichever way it came, one `card.refreshed`.
#[test]
fn ac02_a_staged_editorial_refreshes() {
    let pair = Pair::new("ct-ac02-staged", PRESS);
    let id = make(&pair, "Lead story", &[FRONT]);
    let proposal = pair.propose_edit(&pair.main, FRONT, MEANING.0, MEANING.1);
    specengine_cli::stage(
        &pair.env(&pair.main),
        &Globals::default(),
        &StageRequest {
            id: proposal.clone(),
            body: StageBody::Approve {
                option: None,
                answer: None,
                canon: None,
                editorial: Some(true),
                note: None,
                updated_at: pair.proposal(&proposal).updated_at,
            },
            now: "2026-10-05T22:00:00Z".to_owned(),
            git: pair.git_env(&pair.main),
            actor: None,
        },
    )
    .expect("the stage");
    pair.approve_ok(&pair.main, &proposal);
    assert_eq!(
        events_of(&pair, "card.refreshed"),
        [json!({"id": id, "node": FRONT, "proposal": proposal})]
    );
    assert_eq!(rollup(&pair, FRONT), "in work");
}

/// AC-02: a hand edit is vouched for by nobody: suspect, committed or not,
/// and no event.
#[test]
fn ac02_a_hand_edit_marks() {
    let pair = Pair::new("ct-ac02-hand", PRESS);
    let id = make(&pair, "Lead story", &[FRONT]);
    let (from, to) = MEANING;
    replace(&pair.main, FRONT_MD, from, to);
    assert_eq!(rollup(&pair, FRONT), "suspect", "in the working tree");
    assert_eq!(
        card_line(&pair, &pair.main, FRONT, &id),
        format!("  card {id} | copy/draft | suspect SHT-FRONT | Lead story")
    );
    pair.commit_all(&pair.main, "by hand");
    assert_eq!(rollup(&pair, FRONT), "suspect", "committed");
    assert_eq!(events_of(&pair, "card.refreshed"), Vec::<Value>::new());
}

/// "Rules": marks read the reading root: a branch editing a target marks its
/// cards there alone.
#[test]
fn a_branch_editing_a_target_marks_its_cards_there_alone() {
    let pair = Pair::new("ct-branch", PRESS);
    make(&pair, "Lead story", &[FRONT]);
    let (from, to) = MEANING;
    replace(&pair.linked, FRONT_MD, from, to);
    pair.commit_all(&pair.linked, "on the branch");
    assert_eq!(rollup_in(&pair, &pair.linked, FRONT), "suspect");
    assert_eq!(rollup_in(&pair, &pair.main, FRONT), "in work");
}

/// AC-02, "a completion reads the commit's `Revision: <ID> <rev> (editorial)`
/// trailers": an editorial apply interrupted after its commit, the row, the
/// card and the event set back: the completion, which makes no new commit,
/// refreshes as the apply did, once. A meaning apply's completion refreshes
/// nothing.
#[test]
fn ac02_a_completion_refreshes_as_the_apply_did() {
    let pair = Pair::new("ct-ac02-complete", PRESS);
    let id = make(&pair, "Lead story", &[FRONT]);
    let raw = pair.sql("SELECT snapshot FROM cards WHERE id = 'C-0001';");
    let raw = raw.trim_end_matches('\n');
    let proposal = editorial(&pair, FRONT, MEANING);
    let commit = pair.rev(&pair.main, "HEAD");
    let vouched = front_within(&pair);
    assert_eq!(events_of(&pair, "card.refreshed").len(), 1);

    pair.sql(&format!(
        "UPDATE proposals SET status = 'approved', applied_commit = NULL WHERE id = \
         '{proposal}'; UPDATE cards SET snapshot = '{raw}' WHERE id = '{id}'; DELETE FROM events \
         WHERE type = 'card.refreshed';"
    ));
    assert_eq!(rollup(&pair, FRONT), "suspect", "set back");
    pair.approve_ok(&pair.main, &proposal);
    assert_eq!(pair.rev(&pair.main, "HEAD"), commit, "no new commit");
    assert_eq!(
        events_of(&pair, "card.refreshed"),
        [json!({"id": id, "node": FRONT, "proposal": proposal})]
    );
    assert_eq!(
        show(&pair, &id)["snapshot"][0]["also"],
        json!([vouched]),
        "the completion's refresh"
    );
    assert_eq!(rollup(&pair, FRONT), "in work");

    // A meaning apply's commit carries `1 -> 2`, no `(editorial)`.
    let pair = Pair::new("ct-ac02-meaning", PRESS);
    make(&pair, "Lead story", &[FRONT]);
    let proposal = approved(&pair, FRONT, MEANING);
    pair.sql(&format!(
        "UPDATE proposals SET status = 'approved', applied_commit = NULL WHERE id = \
         '{proposal}';"
    ));
    pair.approve_ok(&pair.main, &proposal);
    assert_eq!(events_of(&pair, "card.refreshed"), Vec::<Value>::new());
    assert_eq!(rollup(&pair, FRONT), "suspect");
}

// --------------------------------------------------------------- AC-03

/// AC-03: `SHT-FRONT` walks the table's states: `no card` with none, `in
/// work` with an open one, `confirmed` with one accepted, `in work` again with
/// another open, `confirmed` when that one is cancelled, and `no card` when
/// every card is cancelled. `spec trace` writes nothing: the file's bytes,
/// `git status` and the queue stay. M: confirming with no card.
#[test]
fn ac03_confirmed_needs_one_accepted_card_and_nothing_else_open() {
    let pair = Pair::new("ct-ac03", PRESS);
    let file = read(&pair.main, FRONT_MD);
    let git = git_side(&pair);
    assert_eq!(rollup(&pair, FRONT), "no card");

    // All cancelled reads `no card`, before any card was accepted.
    let pulled = make(&pair, "Pulled", &[FRONT]);
    assert_eq!(rollup(&pair, FRONT), "in work");
    ok(&pair, &["card", "cancel", &pulled, "--reason", "Pulled."]);
    assert_eq!(rollup(&pair, FRONT), "no card", "all cancelled");
    assert_eq!(
        node_of(&trace(&pair, Some(FRONT)), FRONT)["cards"],
        json!([]),
        "a cancelled card is ignored"
    );

    let lead = make(&pair, "Lead story", &[FRONT]);
    assert_eq!(rollup(&pair, FRONT), "in work");
    to_proof(&pair, &lead);
    assert_eq!(
        rollup(&pair, FRONT),
        "in work",
        "at the last column but one"
    );
    accept_ok(&pair, &lead, &["--briefed"]);
    assert_eq!(rollup(&pair, FRONT), "confirmed");
    assert_eq!(
        card_line(&pair, &pair.main, FRONT, &lead),
        format!("  card {lead} | copy/filed | accepted | Lead story")
    );

    let side = make(&pair, "Side story", &[FRONT]);
    assert_eq!(rollup(&pair, FRONT), "in work", "a card not accepted");
    ok(&pair, &["card", "cancel", &side, "--reason", "Pulled."]);
    assert_eq!(
        rollup(&pair, FRONT),
        "confirmed",
        "all accepted or cancelled"
    );

    let before = state(&pair);
    let shown = trace(&pair, None);
    assert_eq!(node_of(&shown, FRONT)["rollup"], "confirmed");
    assert_eq!(node_of(&shown, BACK)["rollup"], "no card");
    assert_eq!(read(&pair.main, FRONT_MD), file, "the file stays as it was");
    assert_eq!(git_side(&pair), git, "nothing stored in a file");
    assert_eq!(state(&pair), before, "a read writes nothing");
}

/// AC-03, the table's order: first match wins: `departed` over `suspect` over
/// `in work` over `confirmed`, each judged on the node asked about, a card
/// with a second target `confirmed` there.
#[test]
fn ac03_the_first_row_of_the_table_wins() {
    let pair = Pair::new("ct-ac03-order", PRESS);
    let both = make(&pair, "Two sheets", &[INNER, BACK]);
    to_proof(&pair, &both);
    accept_ok(&pair, &both, &departing(&[INNER]));
    assert_eq!(rollup(&pair, INNER), "departed");
    assert_eq!(rollup(&pair, BACK), "confirmed", "it departs from one");

    let open = make(&pair, "Open", &[INNER]);
    let early = accepted(&pair, "Early", &[INNER]);
    assert_eq!(rollup(&pair, INNER), "departed", "over `in work`");
    approved(&pair, INNER, INNER_MEANING);
    assert_eq!(rollup(&pair, INNER), "departed", "over `suspect`");
    assert_eq!(rollup(&pair, BACK), "confirmed", "suspect on the other");

    pair.reject_answer(&pair.main, "PR-0001", "Keep it.", true)
        .0
        .expect("the reject");
    assert_eq!(rollup(&pair, INNER), "suspect", "over `in work`");
    clear_ok(&pair, &both, "No effect.");
    clear_ok(&pair, &open, "No effect.");
    clear_ok(&pair, &early, "No effect.");
    assert_eq!(rollup(&pair, INNER), "in work", "over `confirmed`");
    ok(&pair, &["card", "cancel", &open, "--reason", "Pulled."]);
    assert_eq!(rollup(&pair, INNER), "confirmed");
}

/// "Rollup": a node's cards are those with a target in `within(node)`: a
/// card on a section counts for its document, never the other way round.
#[test]
fn a_nodes_cards_are_those_within_it() {
    let pair = Pair::new("ct-within", PRESS);
    let id = accepted(&pair, "Masthead", &[HEAD]);
    assert_eq!(rollup(&pair, HEAD), "confirmed");
    assert_eq!(rollup(&pair, FRONT), "confirmed", "a section's card");
    let shown = trace(&pair, Some(FRONT));
    assert_eq!(card_of(node_of(&shown, FRONT), &id)["mark"], "accepted");

    make(&pair, "Front", &[FRONT]);
    assert_eq!(rollup(&pair, FRONT), "in work");
    assert_eq!(rollup(&pair, HEAD), "confirmed", "the document's card");
    approved(&pair, HEAD, HEAD_MEANING);
    assert_eq!(rollup(&pair, HEAD), "suspect");
    assert_eq!(
        rollup(&pair, FRONT),
        "suspect",
        "suspect on a section of it"
    );
}

// --------------------------------------------------------------- AC-04

/// AC-04: `--departs` on `SHT-INNER`, which no decision constrains: one
/// `open` discrepancy shaped as "Accepting" lists, the card accepted, the node
/// `departed`, `proposal.created` before `card.accepted`, git untouched;
/// `spec reject` of it gives `confirmed`.
#[test]
fn ac04_a_departure_is_one_open_discrepancy_and_an_accepted_card() {
    let pair = Pair::new("ct-ac04", PRESS);
    let id = make(&pair, "Long read", &[INNER]);
    to_proof(&pair, &id);
    let git = git_side(&pair);
    let output = accept_ok(&pair, &id, &departing(&[INNER]));
    assert!(
        output.contains("accept C-0001 (Long read) as ben? [y/N]"),
        "{output}"
    );
    assert_eq!(git_side(&pair), git, "`--departs` reads git, reads only");

    let items = pair.proposals();
    assert_eq!(items.len(), 1, "{items:?}");
    let item = &items[0];
    assert_eq!(item.id, "PR-0001");
    assert_eq!(item.kind, ProposalKind::Discrepancy);
    assert_eq!(item.status, ProposalStatus::Open);
    assert_eq!(item.target_id, INNER);
    assert_eq!(item.target_path, INNER_MD);
    assert_eq!(item.place.branch, "main");
    let intake = item.intake.as_ref().expect("an intake");
    assert_eq!(intake.target_ids, [INNER]);
    assert_eq!(intake.summary, SUMMARY);
    assert_eq!(intake.gap_type, Some(GapType::Contradicts));
    assert_eq!(intake.severity, IntakeSeverity::Normal);
    assert_eq!(
        intake.evidence,
        [Evidence {
            file: id.clone(),
            qpath: None,
            lines: None,
            observed: OBSERVED.to_owned(),
            documented: DOCUMENTED.to_owned(),
        }]
    );
    assert_eq!(intake.options.len(), 2, "{:?}", intake.options);
    for option in &intake.options {
        assert_eq!(option.price, "not priced at acceptance", "{option:?}");
    }
    assert_eq!(intake.recommendation, Some(0));
    assert_eq!(intake.distinct_from, Vec::<String>::new());

    let card = show(&pair, &id);
    assert_eq!(card["column"], "filed");
    assert_eq!(card["acceptance"]["by"], "ben");
    assert_eq!(card["acceptance"]["verdict"], "departs");
    assert_eq!(card["acceptance"]["departs"], json!([INNER]));
    assert_eq!(card["acceptance"]["discrepancy"], "PR-0001");
    assert_eq!(card["snapshot"][0]["norm_hash"], norm(&pair, INNER));

    let kinds: Vec<String> = pair
        .events()
        .into_iter()
        .map(|event| event.event_type)
        .collect();
    let tail = &kinds[kinds.len() - 2..];
    assert_eq!(tail, ["proposal.created", "card.accepted"], "{kinds:?}");
    let last = events_of(&pair, "card.accepted").pop().expect("an accept");
    assert_eq!(last["verdict"], "departs", "{last}");
    assert_eq!(last["discrepancy"], "PR-0001", "{last}");

    assert_eq!(rollup(&pair, INNER), "departed");
    assert_eq!(
        card_line(&pair, &pair.main, INNER, &id),
        format!("  card {id} | copy/filed | departed PR-0001 | Long read")
    );
    let shown = trace(&pair, Some(INNER));
    let card = card_of(node_of(&shown, INNER), &id);
    assert_eq!(card["mark"], "departed", "{card}");
    assert_eq!(card["discrepancy"], "PR-0001", "{card}");
    assert_eq!(card["on"], INNER, "{card}");

    pair.reject_answer(&pair.main, "PR-0001", "Keep it.", true)
        .0
        .expect("the reject");
    assert_eq!(rollup(&pair, INNER), "confirmed");
    assert_eq!(
        card_line(&pair, &pair.main, INNER, &id),
        format!("  card {id} | copy/filed | accepted | Long read")
    );
}

/// AC-04, the rollup's "not `applied` or `rejected`": the item decided through
/// `spec approve` settles; an `approved` one, an apply interrupted, still
/// holds. An unreadable discrepancy, and one gone from the queue, read
/// unsettled, with a note ("Rules and edge cases").
#[test]
fn only_applied_or_rejected_settles_a_discrepancy() {
    let pair = Pair::new("ct-settled", PRESS);
    let config = read_text(&pair.main, "specengine.toml");
    write(
        &pair.main,
        "specengine.toml",
        with_records(&config, "STY", "docs/records/STY", "templates/decision.md"),
    );
    write(
        &pair.main,
        "templates/decision.md",
        read(&common::fixture("spec-a"), "templates/decision.md"),
    );
    pair.commit_all(&pair.main, "the records' table");
    let id = make(&pair, "Long read", &[INNER]);
    to_proof(&pair, &id);
    accept_ok(&pair, &id, &departing(&[INNER]));
    assert_eq!(rollup(&pair, INNER), "departed");

    pair.decide_ok(
        &pair.main,
        "PR-0001",
        &flags(Some(0), None, Some(&format!("{INNER}#{FOLD}"))),
        None,
    );
    assert_eq!(pair.proposal("PR-0001").status, ProposalStatus::Applied);
    assert_eq!(rollup(&pair, INNER), "confirmed", "`applied` settles");

    pair.sql("UPDATE proposals SET status = 'approved' WHERE id = 'PR-0001';");
    assert_eq!(rollup(&pair, INNER), "departed", "`approved` does not");

    pair.sql("UPDATE proposals SET evidence = 'not json' WHERE id = 'PR-0001';");
    let shown = trace(&pair, Some(INNER));
    assert_eq!(node_of(&shown, INNER)["rollup"], "departed", "{shown}");
    assert!(
        shown["notes"].to_string().contains("PR-0001"),
        "a note names it: {shown}"
    );
    pair.sql("DELETE FROM proposals WHERE id = 'PR-0001';");
    let shown = trace(&pair, Some(INNER));
    assert_eq!(node_of(&shown, INNER)["rollup"], "departed", "{shown}");
    assert!(
        shown["notes"].to_string().contains("PR-0001"),
        "gone from the queue: {shown}"
    );
}

/// AC-04: a failure after the insert, at the card's own event, leaves
/// neither: no proposal, no event, the card as it was; with it gone the same
/// call takes the same ID. M: the insert in its own transaction.
#[test]
fn ac04_a_failure_after_the_insert_leaves_neither() {
    let pair = Pair::new("ct-ac04-fail", PRESS);
    let id = make(&pair, "Long read", &[INNER]);
    to_proof(&pair, &id);
    let cards = pair.sql("SELECT * FROM cards ORDER BY id;");
    let events = pair.sql("SELECT count(*) FROM events;");
    pair.sql(
        "CREATE TRIGGER injected BEFORE INSERT ON events WHEN NEW.type = 'card.accepted' \
         BEGIN SELECT RAISE(ABORT, 'injected'); END;",
    );
    let asked = departing(&[INNER]);
    let failed = accept(&pair, &id, &asked);
    assert_ne!(failed.code, 0, "{}", failed.output);
    assert_eq!(pair.sql("SELECT count(*) FROM proposals;").trim(), "0");
    assert_eq!(pair.sql("SELECT count(*) FROM events;"), events);
    assert_eq!(pair.sql("SELECT * FROM cards ORDER BY id;"), cards);

    pair.sql("DROP TRIGGER injected;");
    accept_ok(&pair, &id, &asked);
    assert_eq!(pair.proposals().len(), 1);
    assert_eq!(pair.proposals()[0].id, "PR-0001", "no ID burnt");
    assert_eq!(rollup(&pair, INNER), "departed");
}

/// "Accepting": one of `--briefed`, `--departs`, else exit 2 on the terminal;
/// a departure needs its three texts; each changes nothing.
#[test]
fn accept_takes_one_verdict() {
    let pair = Pair::new("ct-verdict", PRESS);
    let id = make(&pair, "Long read", &[INNER]);
    to_proof(&pair, &id);
    let before = state(&pair);
    let mut both = departing(&[INNER]);
    both.push("--briefed");
    let mut bad_gap = departing(&[INNER]);
    bad_gap.extend_from_slice(&["--gap-type", "bogus"]);
    let cases: [&[&str]; 7] = [
        &[],
        &both,
        &bad_gap,
        &["--departs", INNER, "--observed", "O.", "--documented", "D."],
        &["--departs", INNER, "--summary", "S.", "--documented", "D."],
        &["--departs", INNER, "--summary", "S.", "--observed", "O."],
        &["--briefed", "--summary", "S."],
    ];
    for flags in cases {
        let done = accept(&pair, &id, flags);
        assert_eq!(done.code, 2, "{flags:?}: {}", done.output);
        assert_eq!(state(&pair), before, "{flags:?}");
    }
}

/// "Accepting": `--departs` names direct targets: a section of a target, a
/// node no target names and any node on a manual card are refused; nothing
/// changes. A chosen `--gap-type` and a `--note` reach the item and the row.
#[test]
fn departs_names_direct_targets() {
    let pair = Pair::new("ct-direct", PRESS);
    let id = make(&pair, "Long read", &[INNER]);
    to_proof(&pair, &id);
    let manual = make(&pair, "Manual", &[]);
    to_proof(&pair, &manual);
    let before = state(&pair);
    for (card, node) in [(&id, FOLD), (&id, BACK), (&manual, BACK)] {
        let done = accept(&pair, card, &departing(&[node]));
        assert_eq!(done.code, 1, "{card} {node}: {}", done.output);
        assert_eq!(state(&pair), before, "{card} {node}");
        assert_eq!(pair.proposals().len(), 0);
    }

    let mut flags = departing(&[INNER]);
    flags.extend_from_slice(&["--gap-type", "partial", "--note", "See the fold."]);
    accept_ok(&pair, &id, &flags);
    let intake = pair.proposals().remove(0).intake.expect("an intake");
    assert_eq!(intake.gap_type, Some(GapType::Partial));
    assert_eq!(show(&pair, &id)["acceptance"]["note"], "See the fold.");
}

/// "Accepting": 1-16 direct targets: sixteen run, a seventeenth is refused.
#[test]
fn departs_names_at_most_sixteen() {
    let pair = Pair::new("ct-sixteen", PRESS);
    let ids = rulings(&pair, 17);
    let names: Vec<&str> = ids.iter().map(String::as_str).collect();
    let id = make(&pair, "Every ruling", &names);
    to_proof(&pair, &id);
    let before = state(&pair);
    let refused = accept(&pair, &id, &departing(&names));
    assert_eq!(refused.code, 1, "{}", refused.output);
    assert_eq!(state(&pair), before);
    accept_ok(&pair, &id, &departing(&names[..16]));
    let intake = pair.proposals().remove(0).intake.expect("an intake");
    assert_eq!(intake.target_ids, names[..16]);
    assert_eq!(rollup(&pair, &ids[0]), "departed");
    assert_eq!(rollup(&pair, &ids[16]), "confirmed", "the one left out");
}

/// "Accepting", the terminal as before: a declined departure and a stale
/// `--revision` store nothing; a pipe exits 2 first, naming the terminal.
#[test]
fn a_departure_takes_the_terminal_as_an_accept_does() {
    let pair = Pair::new("ct-departs-consent", PRESS);
    let id = make(&pair, "Long read", &[INNER]);
    to_proof(&pair, &id);
    let before = state(&pair);
    let flags = departing(&[INNER]);
    let mut args = vec!["card", "accept", id.as_str(), "--as", "ben"];
    args.extend_from_slice(&flags);
    let declined = pair.spec_pty(&pair.main, &args, Some("n"));
    assert!(declined.asked, "{}", declined.output);
    assert_eq!(declined.code, 1, "{}", declined.output);
    assert_eq!(state(&pair), before, "declined");

    let piped = pair.spec_piped(&pair.main, &args, b"y\n");
    piped.code(2);
    assert!(piped.stderr.contains("terminal"), "{}", piped.show());
    assert_eq!(state(&pair), before, "piped");

    args.extend_from_slice(&["--revision", "1"]);
    let stale = pair.spec_pty(&pair.main, &args, Some("y"));
    assert_eq!(stale.code, 1, "{}", stale.output);
    assert_eq!(state(&pair), before, "stale");
}

/// "Git": the item binds the place it is raised in: a linked worktree's branch
/// from there, git only read there too. Two targets in two files are one
/// item, the first's holder its path, and each node departs.
#[test]
fn a_departure_binds_its_place_and_names_each_target() {
    let pair = Pair::new("ct-departs-place", PRESS);
    let id = make(&pair, "Two sheets", &[INNER, BACK]);
    to_proof(&pair, &id);
    let status = pair.git_text(&pair.linked, &["status", "--porcelain", "--ignored"]);
    let head = pair.rev(&pair.linked, "HEAD");
    let mut args = vec!["card", "accept", id.as_str(), "--as", "ben"];
    let flags = departing(&[INNER, BACK]);
    args.extend_from_slice(&flags);
    let done = pair.spec_pty(&pair.linked, &args, Some("y"));
    assert_eq!(done.code, 0, "{}", done.output);
    assert_eq!(
        pair.git_text(&pair.linked, &["status", "--porcelain", "--ignored"]),
        status
    );
    assert_eq!(pair.rev(&pair.linked, "HEAD"), head);
    let item = pair.proposal("PR-0001");
    assert_eq!(item.place.branch, "t1");
    assert_eq!(item.target_path, INNER_MD);
    let intake = item.intake.expect("an intake");
    assert_eq!(intake.target_ids, [INNER, BACK]);
    assert_eq!(
        show(&pair, &id)["acceptance"]["departs"],
        json!([INNER, BACK])
    );
    assert_eq!(rollup(&pair, INNER), "departed");
    assert_eq!(rollup(&pair, BACK), "departed");
    assert_eq!(
        rollup(&pair, WEEKLY),
        "no card",
        "the parent holds no target"
    );
}

/// "Git": a place that cannot be bound exits 2, nothing changed: a detached
/// `HEAD`; `PR` taken by `[ids]`; each stops the departure alone: `--briefed`
/// runs.
#[test]
fn a_departure_that_cannot_bind_a_place_exits_2() {
    let pair = Pair::new("ct-departs-detached", PRESS);
    let id = make(&pair, "Long read", &[INNER]);
    to_proof(&pair, &id);
    pair.git.git(&pair.main, &["checkout", "-q", "--detach"]);
    let before = state(&pair);
    let done = accept(&pair, &id, &departing(&[INNER]));
    assert_eq!(done.code, 2, "{}", done.output);
    assert_eq!(state(&pair), before, "detached");
    accept_ok(&pair, &id, &["--briefed"]);

    let pair = Pair::new("ct-departs-clash", PRESS);
    let id = make(&pair, "Long read", &[INNER]);
    to_proof(&pair, &id);
    let config = read_text(&pair.main, "specengine.toml");
    write(
        &pair.main,
        "specengine.toml",
        with_ids_line(&config, "PR = { kind = \"press-run\", width = 4 }\n"),
    );
    pair.commit_all(&pair.main, "PR taken");
    let before = state(&pair);
    let done = accept(&pair, &id, &departing(&[INNER]));
    assert_eq!(done.code, 2, "{}", done.output);
    assert_eq!(state(&pair), before, "`PR` in [ids]");
    accept_ok(&pair, &id, &["--briefed"]);
}

// --------------------------------------------------------------- AC-05

/// AC-05: `STY-0001` constrains `SHT-FRONT`, a hit: the departing accept
/// exits 0 with no flag, the item's `distinct_from` names it, a note lists it.
/// M: the dedup's refusal kept.
#[test]
fn ac05_a_linked_decision_is_a_hit_that_refuses_nothing() {
    let pair = Pair::new("ct-ac05", PRESS);
    let id = make(&pair, "Lead story", &[FRONT]);
    to_proof(&pair, &id);
    let output = accept_ok(&pair, &id, &departing(&[FRONT]));
    assert!(output.contains(RULING), "a note lists it: {output}");
    let items = pair.proposals();
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(
        items[0].intake.as_ref().expect("an intake").distinct_from,
        [RULING]
    );
    assert_eq!(rollup(&pair, FRONT), "departed");
}

/// AC-05, a queue hit: a second departure with the same summary on the same
/// node stores a second item, naming the first; the exit stays 0 and each
/// card holds its own item.
#[test]
fn ac05_a_queue_hit_is_named_by_the_accept() {
    let pair = Pair::new("ct-ac05-queue", PRESS);
    let first = make(&pair, "First", &[INNER]);
    let second = make(&pair, "Second", &[INNER]);
    to_proof(&pair, &first);
    to_proof(&pair, &second);
    accept_ok(&pair, &first, &departing(&[INNER]));
    let output = accept_ok(&pair, &second, &departing(&[INNER]));
    assert!(output.contains("PR-0001"), "a note lists it: {output}");
    let items = pair.proposals();
    assert_eq!(items.len(), 2, "{items:?}");
    let stored = pair.proposal("PR-0002");
    assert_eq!(
        stored.intake.as_ref().expect("an intake").distinct_from,
        ["PR-0001"]
    );
    assert_eq!(show(&pair, &second)["acceptance"]["discrepancy"], "PR-0002");
}

/// "The dedup never refuses a departure": past 64 hits, `DISTINCT_MAX`, the
/// item names the first 64, the notes list them all, the exit stays 0.
#[test]
fn ac05_past_the_cap_the_item_names_the_first_sixty_four() {
    let pair = Pair::new("ct-ac05-cap", PRESS);
    let more = rulings(&pair, 69);
    let id = make(&pair, "Lead story", &[FRONT]);
    to_proof(&pair, &id);
    let output = accept_ok(&pair, &id, &departing(&[FRONT]));
    let intake = pair.proposals().remove(0).intake.expect("an intake");
    assert_eq!(intake.distinct_from.len(), 64, "{:?}", intake.distinct_from);
    assert_eq!(intake.distinct_from[0], RULING);
    for name in std::iter::once(RULING).chain(more.iter().map(String::as_str)) {
        assert!(output.contains(name), "{name}: {output}");
    }
}

// --------------------------------------------------------------- AC-06

/// The accepted card on `SHT-FRONT`, its target changed, a correction asked:
/// `(pair, original, correction)`.
fn corrected(label: &str) -> (Pair, String, String) {
    let pair = Pair::new(label, PRESS);
    let id = accepted(&pair, "Lead story", &[FRONT]);
    approved(&pair, FRONT, MEANING);
    let done = clear(
        &pair,
        &id,
        &[
            "--needs-correction",
            "--reason",
            "The lead moved.",
            "--estimate",
            "3",
        ],
    );
    assert_eq!(done.code, 0, "{}", done.output);
    (pair, id, "C-0002".to_owned())
}

/// AC-06: the clear: a new card with `corrects`, the targets, flow,
/// discipline and estimate, the default title; the original accepted, its
/// snapshot untouched, its mark held: `correcting C-0002`, the node `in work`
/// through it, one `card.cleared` naming the correction. M: re-freezing at
/// the clear.
#[test]
fn ac06_a_correction_holds_the_mark() {
    let pair = Pair::new("ct-ac06", PRESS);
    let id = accepted(&pair, "Lead story", &[FRONT]);
    let frozen = show(&pair, &id);
    let old = front_within(&pair);
    approved(&pair, FRONT, MEANING);
    let new = front_within(&pair);
    let git = git_side(&pair);
    let done = clear(
        &pair,
        &id,
        &[
            "--needs-correction",
            "--reason",
            "The lead moved.",
            "--estimate",
            "3",
        ],
    );
    assert_eq!(done.code, 0, "{}", done.output);
    assert_eq!(git_side(&pair), git);

    let original = show(&pair, &id);
    assert_eq!(original["column"], "filed");
    assert_eq!(original["acceptance"], frozen["acceptance"]);
    assert_eq!(
        original["snapshot"], frozen["snapshot"],
        "its snapshot untouched"
    );

    let fix = show(&pair, "C-0002");
    assert_eq!(fix["corrects"], id.as_str());
    assert_eq!(fix["title"], "Correct Lead story");
    assert_eq!(fix["targets"], json!([FRONT]));
    assert_eq!(fix["flow"], "copy");
    assert_eq!(fix["discipline"], "copy");
    assert_eq!(fix["estimate"], "3");
    assert_eq!(fix["column"], "draft");

    assert_eq!(rollup(&pair, FRONT), "in work", "through the correction");
    assert_eq!(
        card_line(&pair, &pair.main, FRONT, &id),
        format!("  card {id} | copy/filed | correcting C-0002 | Lead story")
    );
    // A held mark takes no second clear: the correction answers it.
    let before = state(&pair);
    let held = clear(&pair, &id, &["--no-effect", "--reason", "Never mind."]);
    assert_eq!(held.code, 1, "{}", held.output);
    assert_eq!(state(&pair), before);
    assert_eq!(
        card_line(&pair, &pair.main, FRONT, "C-0002"),
        "  card C-0002 | copy/draft | open | Correct Lead story"
    );
    let shown = trace(&pair, Some(FRONT));
    let card = card_of(node_of(&shown, FRONT), &id);
    assert_eq!(card["mark"], "correcting", "{card}");
    assert_eq!(card["correction"], "C-0002", "{card}");

    assert_eq!(
        events_of(&pair, "card.cleared"),
        [json!({
            "id": id,
            "outcome": "needs_correction",
            "nodes": [{"id": FRONT, "rev": 2, "from": old, "to": new}],
            "correction": "C-0002",
            "reason": "The lead moved.",
            "as": "ben",
        })]
    );
}

/// AC-06: the correction accepted re-freezes the original: `confirmed`, one
/// `card.cleared` of `corrected`, by either verdict.
#[test]
fn ac06_the_correction_accepted_gives_confirmed() {
    let (pair, id, fix) = corrected("ct-ac06-accepted");
    let new = norm(&pair, FRONT);
    to_proof(&pair, &fix);
    accept_ok(&pair, &fix, &["--briefed"]);
    assert_eq!(rollup(&pair, FRONT), "confirmed");
    assert_eq!(show(&pair, &id)["snapshot"][0]["norm_hash"], new);
    assert_eq!(
        card_line(&pair, &pair.main, FRONT, &id),
        format!("  card {id} | copy/filed | accepted | Lead story")
    );
    let cleared = events_of(&pair, "card.cleared");
    assert_eq!(cleared.len(), 2, "{cleared:?}");
    assert_eq!(cleared[1]["id"], id.as_str());
    assert_eq!(cleared[1]["outcome"], "corrected");
    assert_eq!(cleared[1]["correction"], fix.as_str());

    // A departing correction re-freezes as well: "Either re-freezes".
    let (pair, id, fix) = corrected("ct-ac06-departs");
    let new = norm(&pair, FRONT);
    to_proof(&pair, &fix);
    accept_ok(&pair, &fix, &departing(&[FRONT]));
    assert_eq!(show(&pair, &id)["snapshot"][0]["norm_hash"], new);
    assert_eq!(rollup(&pair, FRONT), "departed", "the correction departs");
}

/// AC-06: the correction cancelled: the original suspect again, the node
/// `suspect`; a new correction can be asked. M: re-freezing at the clear.
#[test]
fn ac06_the_correction_cancelled_gives_suspect() {
    let (pair, id, fix) = corrected("ct-ac06-cancelled");
    ok(
        &pair,
        &["card", "cancel", &fix, "--reason", "Not worth it."],
    );
    assert_eq!(rollup(&pair, FRONT), "suspect");
    assert_eq!(
        card_line(&pair, &pair.main, FRONT, &id),
        format!("  card {id} | copy/filed | suspect SHT-FRONT | Lead story")
    );
    let again = clear(
        &pair,
        &id,
        &[
            "--needs-correction",
            "--reason",
            "Once more.",
            "--estimate",
            "1",
            "--title",
            "Redo the lead",
        ],
    );
    assert_eq!(again.code, 0, "{}", again.output);
    let fix = show(&pair, "C-0003");
    assert_eq!(fix["title"], "Redo the lead", "`--title`");
    assert_eq!(rollup(&pair, FRONT), "in work");
}

/// "Needs correction, accepted cards only": an open card is refused, nothing
/// changed. A correction made by hand with `spec card new --corrects` holds
/// the mark too: "while the correction lives".
#[test]
fn needs_correction_takes_accepted_cards_only() {
    let pair = Pair::new("ct-ac06-open", PRESS);
    let id = make(&pair, "Lead story", &[FRONT]);
    approved(&pair, FRONT, MEANING);
    let before = state(&pair);
    let done = clear(
        &pair,
        &id,
        &["--needs-correction", "--reason", "R.", "--estimate", "3"],
    );
    assert_eq!(done.code, 1, "{}", done.output);
    assert_eq!(state(&pair), before);

    let pair = Pair::new("ct-ac06-hand", PRESS);
    let id = accepted(&pair, "Lead story", &[FRONT]);
    approved(&pair, FRONT, MEANING);
    assert_eq!(rollup(&pair, FRONT), "suspect");
    ok(
        &pair,
        &[
            "card",
            "new",
            "--title",
            "By hand",
            "--discipline",
            "copy",
            "--target",
            FRONT,
            "--corrects",
            &id,
        ],
    );
    assert_eq!(rollup(&pair, FRONT), "in work");
    assert_eq!(
        card_line(&pair, &pair.main, FRONT, &id),
        format!("  card {id} | copy/filed | correcting C-0002 | Lead story")
    );
}

// --------------------------------------------------------------- AC-07

/// AC-07: `clear --group PRN-WEEKLY` over cards on a document under it, a
/// section of one and an accepted card: one event per suspect card, the
/// count in the prompt; a clean card under it and a suspect one outside it
/// stay. M: a bulk clear ignoring `parent:`.
#[test]
fn ac07_a_group_clear_takes_what_the_tree_holds_and_nothing_outside() {
    let pair = Pair::new("ct-ac07", PRESS);
    let sealed = accepted(&pair, "Sealed", &[BACK]);
    let front = make(&pair, "Front", &[FRONT]);
    let head = make(&pair, "Head", &[HEAD]);
    let clean = make(&pair, "Clean", &[INNER]);
    let outside = make(&pair, "Outside", &[RULING]);
    approved(&pair, FRONT, MEANING);
    approved(&pair, HEAD, HEAD_MEANING);
    let back_text = read_text(&pair.main, BACK_MD);
    write(
        &pair.main,
        BACK_MD,
        back_text.replace("the tide table", "the moon table"),
    );
    let ruling = read_text(&pair.main, RULING_MD);
    write(
        &pair.main,
        RULING_MD,
        ruling.replace("whatever else runs", "unless a storm runs"),
    );
    pair.commit_all(&pair.main, "by hand");
    for node in [BACK, FRONT, HEAD, RULING] {
        assert_eq!(rollup(&pair, node), "suspect", "{node}");
    }

    let git = git_side(&pair);
    let done = pair.spec_pty(
        &pair.main,
        &[
            "card",
            "clear",
            "--group",
            WEEKLY,
            "--no-effect",
            "--reason",
            "A reprint.",
            "--as",
            "ben",
        ],
        Some("y"),
    );
    assert_eq!(done.code, 0, "{}", done.output);
    assert!(done.asked, "{}", done.output);
    assert!(done.output.contains('3'), "its count: {}", done.output);
    assert_eq!(git_side(&pair), git);

    let cleared = events_of(&pair, "card.cleared");
    let ids: Vec<&str> = cleared
        .iter()
        .map(|event| event["id"].as_str().expect("an ID"))
        .collect();
    assert_eq!(ids, [sealed.as_str(), &front, &head], "one event each");
    for event in &cleared {
        assert_eq!(event["outcome"], "no_effect", "{event}");
        assert_eq!(event["reason"], "A reprint.", "{event}");
        assert_eq!(event["as"], "ben", "{event}");
        assert_eq!(event["correction"], Value::Null, "{event}");
    }
    assert_eq!(rollup(&pair, BACK), "confirmed");
    assert_eq!(rollup(&pair, FRONT), "in work");
    assert_eq!(rollup(&pair, HEAD), "in work");
    assert_eq!(rollup(&pair, INNER), "in work");
    assert_eq!(rollup(&pair, RULING), "suspect", "outside the tree");
    assert_eq!(
        show(&pair, &clean)["snapshot"][0]["also"],
        json!([]),
        "a clean card is left"
    );
    assert_eq!(
        card_line(&pair, &pair.main, RULING, &outside),
        format!("  card {outside} | copy/draft | suspect STY-0001 | Outside")
    );
}

/// AC-07: a group is `within` REF and of each document under it: a leaf's
/// group takes its own and no sibling's, a section's group its own; the group
/// is one transaction: a failure at its last event leaves every card as it
/// was.
#[test]
fn ac07_a_group_is_its_refs_tree_in_one_transaction() {
    let pair = Pair::new("ct-ac07-leaf", PRESS);
    make(&pair, "Front", &[FRONT]);
    let head = make(&pair, "Head", &[HEAD]);
    let inner = make(&pair, "Inner", &[INNER]);
    approved(&pair, FRONT, MEANING);
    approved(&pair, HEAD, HEAD_MEANING);
    approved(&pair, INNER, INNER_MEANING);
    let group = |reference: &str, answer: &str| {
        pair.spec_pty(
            &pair.main,
            &[
                "card",
                "clear",
                "--group",
                reference,
                "--no-effect",
                "--reason",
                "R.",
                "--as",
                "ben",
            ],
            Some(answer),
        )
    };

    // Declined: nothing.
    let before = state(&pair);
    assert_eq!(group(FRONT, "n").code, 1);
    assert_eq!(state(&pair), before);

    // A failure past the first card.
    pair.sql(&format!(
        "CREATE TRIGGER injected BEFORE INSERT ON events WHEN NEW.type = 'card.cleared' AND \
         json_extract(NEW.payload, '$.id') = '{head}' BEGIN SELECT RAISE(ABORT, 'injected'); \
         END;"
    ));
    let cards = pair.sql("SELECT * FROM cards ORDER BY id;");
    let count = pair.sql("SELECT count(*) FROM events;");
    assert_ne!(group(WEEKLY, "y").code, 0);
    assert_eq!(pair.sql("SELECT * FROM cards ORDER BY id;"), cards);
    assert_eq!(pair.sql("SELECT count(*) FROM events;"), count);
    pair.sql("DROP TRIGGER injected;");

    let done = group(HEAD, "y");
    assert_eq!(done.code, 0, "{}", done.output);
    assert_eq!(
        events_of(&pair, "card.cleared")
            .iter()
            .map(|event| event["id"].as_str().expect("an ID").to_owned())
            .collect::<Vec<_>>(),
        std::slice::from_ref(&head),
        "a section's group"
    );
    assert_eq!(rollup(&pair, FRONT), "suspect", "the document's card stays");

    let done = group(FRONT, "y");
    assert_eq!(done.code, 0, "{}", done.output);
    assert_eq!(events_of(&pair, "card.cleared").len(), 2);
    assert_eq!(rollup(&pair, FRONT), "in work");
    assert_eq!(rollup(&pair, INNER), "suspect", "a sibling stays");
    assert_eq!(
        card_line(&pair, &pair.main, INNER, &inner),
        format!("  card {inner} | copy/draft | suspect SHT-INNER | Inner")
    );
}

/// AC-07: a group reaches its tree alone. A card clean inside and suspect only
/// outside is not cleared: exit 1, `no suspect card under`, its mark kept.
/// Suspect on both, only the in-scope entry re-freezes, the event naming it;
/// the outside mark stays, and `spec card clear C` reaches it. M: a group
/// qualifying a card on any target.
#[test]
fn ac07_a_group_reaches_its_own_entries_alone() {
    let pair = Pair::new("ct-ac07-whole", PRESS);
    let id = make(&pair, "Both sides", &[BACK, RULING]);
    let frozen = show(&pair, &id)["snapshot"].clone();
    let text = read_text(&pair.main, RULING_MD);
    write(
        &pair.main,
        RULING_MD,
        text.replace("whatever else runs", "unless a storm runs"),
    );
    pair.commit_all(&pair.main, "by hand");
    let group = |pair: &Pair| {
        pair.spec_pty(
            &pair.main,
            &[
                "card",
                "clear",
                "--group",
                WEEKLY,
                "--no-effect",
                "--reason",
                "A reprint.",
                "--as",
                "ben",
            ],
            Some("y"),
        )
    };

    let before = state(&pair);
    let done = group(&pair);
    assert_eq!(done.code, 1, "{}", done.output);
    assert!(
        done.output.contains("no suspect card under `PRN-WEEKLY`"),
        "{}",
        done.output
    );
    assert_eq!(state(&pair), before, "suspect only outside");
    assert_eq!(rollup(&pair, RULING), "suspect", "its mark kept");

    let text = read_text(&pair.main, BACK_MD);
    write(
        &pair.main,
        BACK_MD,
        text.replace("the tide table", "the moon table"),
    );
    pair.commit_all(&pair.main, "by hand");
    let done = group(&pair);
    assert_eq!(done.code, 0, "{}", done.output);
    assert!(
        done.output
            .contains("clear 1 cards of PRN-WEEKLY as ben: no effect? [y/N]"),
        "{}",
        done.output
    );
    let cleared = events_of(&pair, "card.cleared");
    assert_eq!(cleared.len(), 1, "{cleared:?}");
    let nodes = cleared[0]["nodes"].as_array().expect("nodes");
    assert_eq!(nodes.len(), 1, "{cleared:?}");
    assert_eq!(nodes[0]["id"], BACK, "the in-scope entry alone");
    let card = show(&pair, &id);
    assert_eq!(
        card["snapshot"][0]["norm_hash"],
        norm(&pair, BACK),
        "{card}"
    );
    assert_eq!(card["snapshot"][1], frozen[1], "the outside entry: {card}");
    assert_eq!(rollup(&pair, BACK), "in work");
    assert_eq!(rollup(&pair, RULING), "suspect", "the outside mark stays");

    clear_ok(&pair, &id, "The ruling too.");
    let cleared = events_of(&pair, "card.cleared");
    assert_eq!(cleared.len(), 2, "{cleared:?}");
    assert_eq!(cleared[1]["nodes"][0]["id"], RULING);
    assert_eq!(rollup(&pair, RULING), "in work");
}

// --------------------------------------------------------------- AC-08

/// AC-08: `SHT-FRONT-HEAD` changes: its card suspect, `SHT-BACK`'s, which
/// uses it, not: `spec trace` lists it under `uses`, with no mark, one level.
/// M: marking through `uses`.
#[test]
fn ac08_a_changed_definition_marks_its_card_and_lists_its_users() {
    let pair = Pair::new("ct-ac08", PRESS);
    let head = make(&pair, "Masthead", &[HEAD]);
    let back = make(&pair, "Notices", &[BACK]);
    // One level: `SHT-INNER` uses `SHT-BACK`, which uses the definition.
    let text = read_text(&pair.main, INNER_MD);
    write(
        &pair.main,
        INNER_MD,
        text.replace(
            "reviewed: 2026-10-01\n---",
            "reviewed: 2026-10-01\nlinks:\n  uses: [SHT-BACK]\n---",
        ),
    );
    pair.commit_all(&pair.main, "a second level");
    let hash = norm(&pair, BACK);
    approved(&pair, HEAD, HEAD_MEANING);
    assert_eq!(norm(&pair, BACK), hash, "the user's hash holds");

    let shown = trace(&pair, Some(HEAD));
    assert_eq!(node_ids(&shown), [HEAD]);
    let node = node_of(&shown, HEAD);
    assert_eq!(node["rollup"], "suspect");
    assert_eq!(card_of(node, &head)["mark"], "suspect");
    assert_eq!(
        node["uses"],
        json!([{"id": BACK, "rollup": "in work", "cards": 1}]),
        "one level"
    );
    let text = ok(&pair, &["trace", HEAD]);
    assert_eq!(
        text,
        format!(
            "trace {HEAD} | nodes 1, cards 1 | no card 0, in work 0, suspect 1, departed 0, \
             confirmed 0\nsuspect | {HEAD} | The masthead | {FRONT_MD} | cards 1\n  card {head} \
             | copy/draft | suspect {HEAD} | Masthead\n  uses {BACK} | in work | cards 1\n"
        )
    );

    assert_eq!(rollup(&pair, BACK), "in work");
    assert_eq!(
        card_line(&pair, &pair.main, BACK, &back),
        format!("  card {back} | copy/draft | open | Notices")
    );

    // "`spec impact D` the whole reach."
    let reach = ok(&pair, &["impact", HEAD]);
    for node in [HEAD, BACK, INNER] {
        assert!(reach.contains(node), "{node}: {reach}");
    }
}

/// "A changed definition": a definition with no card lists its users all the
/// same, `no card` and a count of 0 where a user has none.
#[test]
fn a_definition_with_no_card_lists_its_users() {
    let pair = Pair::new("ct-ac08-none", PRESS);
    approved(&pair, HEAD, HEAD_MEANING);
    let shown = trace(&pair, Some(HEAD));
    let node = node_of(&shown, HEAD);
    assert_eq!(node["rollup"], "no card");
    assert_eq!(
        node["uses"],
        json!([{"id": BACK, "rollup": "no card", "cards": 0}])
    );
}

// --------------------------------------------------------------- AC-09

/// A gate's answer on the worktree: exit, stdout, stderr. `spec gate` answers
/// within 3 s of wall clock (`GATE_DEADLINE`): a run a loaded machine starves
/// past that says `unavailable` and nothing of the cards, so it is asked again
/// (five tries; the last answer stands, and a compare shows it).
fn gate_answer(pair: &Pair) -> (i32, String, String) {
    let main = pair.main.to_str().unwrap();
    let mut done = None;
    for _ in 0..5 {
        let run = run(
            pair,
            &["gate", "--worktree", main, "--path", "src/a.rs", "--json"],
        );
        let starved = run.stdout.contains("\"reason\":\"deadline\"")
            || run.stderr.contains("no answer within");
        done = Some(run);
        if !starved {
            break;
        }
    }
    let done = done.expect("a run");
    (done.code, done.stdout, done.stderr)
}

/// AC-09: `spec trace` exits 0 on suspect and departed rows, and neither
/// `spec check` nor `spec gate` moves a byte: nothing is gated by a mark
/// (ADR-0012).
#[test]
fn ac09_trace_exits_0_and_check_and_gate_stay() {
    let pair = Pair::new("ct-ac09-gate", PRESS);
    append_config(
        &pair.main,
        "\n[zones]\ncode = [\"src/**\"]\n\n[gate]\nmode = \"selective\"\nzones = [\"code\"]\n",
    );
    write(&pair.main, "src/a.rs", "fn a() {}\n");
    pair.commit_all(&pair.main, "the gate's scratch files");
    let check = json_run(&pair, &["check"]);
    let gate = gate_answer(&pair);

    // No file moves between the two readings: the mark is the frozen hash's.
    let id = make(&pair, "Long read", &[INNER]);
    to_proof(&pair, &id);
    accept_ok(&pair, &id, &departing(&[INNER]));
    let lead = make(&pair, "Lead story", &[FRONT]);
    pair.sql(&format!(
        "UPDATE cards SET snapshot = replace(snapshot, '{}', 'b3:{}') WHERE id = '{lead}';",
        front_within(&pair),
        "0".repeat(64)
    ));
    assert_eq!(rollup(&pair, FRONT), "suspect");
    assert_eq!(rollup(&pair, INNER), "departed");

    let done = run(&pair, &["trace"]);
    done.code(0);
    let after = json_run(&pair, &["check"]);
    assert_eq!(
        (after.code, &after.stdout, &after.stderr),
        (check.code, &check.stdout, &check.stderr),
        "`spec check` unchanged"
    );
    assert_eq!(gate_answer(&pair), gate, "`spec gate` unchanged");
}

/// AC-09: `spec impact` lists the cards with a target in what it reached, by
/// number, a cancelled one and one off the reach left out; nodes, tasks and
/// the exit as they were.
#[test]
fn ac09_impact_lists_the_cards() {
    let pair = Pair::new("ct-ac09-impact", PRESS);
    let before = doc(&pair, &["impact", HEAD]);
    let nodes = ok(&pair, &["impact", HEAD])
        .lines()
        .filter(|line| line.starts_with("node "))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let head = make(&pair, "Masthead", &[HEAD]);
    let back = make(&pair, "Notices", &[BACK, INNER]);
    let gone = make(&pair, "Pulled", &[HEAD]);
    ok(&pair, &["card", "cancel", &gone, "--reason", "Pulled."]);
    make(&pair, "Off the reach", &[INNER]);
    let both = make(&pair, "Both", &[BACK, HEAD]);
    approved(&pair, HEAD, HEAD_MEANING);

    let text = ok(&pair, &["impact", HEAD]);
    let rows: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("card "))
        .collect();
    assert_eq!(
        rows,
        [
            format!("card {head} | copy/draft | suspect {HEAD} | Masthead | {HEAD}"),
            format!("card {back} | copy/draft | open | Notices | {BACK}"),
            format!("card {both} | copy/draft | suspect {HEAD} | Both | {HEAD}, {BACK}"),
        ]
    );
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("node "))
            .map(str::to_owned)
            .collect::<Vec<_>>(),
        nodes,
        "the node rows"
    );
    let after = doc(&pair, &["impact", HEAD]);
    let cards = after["cards"].as_array().expect("cards");
    assert_eq!(
        cards
            .iter()
            .map(|card| card["id"].as_str().expect("an ID"))
            .collect::<Vec<_>>(),
        [head.as_str(), back.as_str(), both.as_str()]
    );
    assert_eq!(after["nodes"], before["nodes"], "nodes do not change");
    assert_eq!(after["tasks"], before["tasks"], "tasks do not change");
    assert_eq!(before["cards"], json!([]), "none yet");
}

/// AC-09, "Re-estimating": on a card holding an estimate, `--reason` is
/// required (exit 1) and alone (exit 2), `card.reestimated`; a first estimate
/// stays `card.edited`, no reason asked.
#[test]
fn ac09_a_re_estimate_needs_a_reason() {
    let pair = Pair::new("ct-ac09-estimate", PRESS);
    let id = make(&pair, "Lead story", &[FRONT]);
    ok(&pair, &["card", "edit", &id, "--estimate", "2"]);
    let last = events(&pair).pop().expect("an event");
    assert_eq!(last.0, "card.edited", "a first estimate");
    assert_eq!(last.1["fields"], json!(["estimate"]));

    let before = state(&pair);
    let done = run(&pair, &["card", "edit", &id, "--estimate", "3"]);
    done.code(1);
    assert!(done.stderr.contains("--reason"), "{}", done.show());
    assert_eq!(state(&pair), before, "no reason");
    let done = run(
        &pair,
        &[
            "card",
            "edit",
            &id,
            "--estimate",
            "3",
            "--reason",
            "Bigger.",
            "--goal",
            "Again.",
        ],
    );
    done.code(2);
    assert_eq!(state(&pair), before, "no other flag beside it");

    ok(
        &pair,
        &[
            "card",
            "edit",
            &id,
            "--estimate",
            "3",
            "--reason",
            "Bigger.",
            "--as",
            "ana",
        ],
    );
    assert_eq!(show(&pair, &id)["estimate"], "3");
    let last = events(&pair).pop().expect("an event");
    assert_eq!(
        last,
        (
            "card.reestimated".to_owned(),
            json!({"id": id, "old": "2", "new": "3", "reason": "Bigger.", "as": "ana"})
        )
    );
}

/// "`spec impact`": cut after tasks, `truncated.cards` the cards left out, the
/// text's tail naming them; the exit stays 0.
#[test]
fn impact_cuts_its_cards() {
    let pair = Pair::new("ct-impact-cap", PRESS);
    many_cards(&pair, FRONT, 200);
    let done = run(&pair, &["impact", FRONT]);
    done.code(0);
    let (rows, tail) = done
        .stdout
        .trim_end()
        .rsplit_once('\n')
        .expect("rows and a tail");
    assert!(
        rows.chars().count() < specengine_cli::OUTPUT_CAP_CHARS,
        "everything before the tail: {} chars",
        rows.chars().count()
    );
    assert!(tail.starts_with("[truncated: "), "{tail}");
    assert!(tail.contains(" cards"), "{tail}");
    let document = doc(&pair, &["impact", FRONT]);
    let shown = document["cards"].as_array().expect("cards").len();
    let omitted = document["truncated"]["cards"].as_u64().expect("a count");
    assert!(omitted > 0, "{}", document["truncated"]);
    assert_eq!(shown + usize::try_from(omitted).unwrap(), 200);
}

/// "Re-estimating", the readings: `--reason` beside a first estimate exits 2;
/// clearing a held estimate is a change of it: its reason required, nothing
/// beside it, `card.reestimated` with `new` null, so the record keeps `old`.
#[test]
fn clearing_a_held_estimate_is_a_re_estimate() {
    let pair = Pair::new("ct-estimate-clear", PRESS);
    let id = make(&pair, "Lead story", &[FRONT]);
    let before = state(&pair);
    let done = run(
        &pair,
        &["card", "edit", &id, "--estimate", "2", "--reason", "First."],
    );
    done.code(2);
    assert_eq!(state(&pair), before, "no estimate held");
    ok(&pair, &["card", "edit", &id, "--estimate", "2"]);

    let before = state(&pair);
    let done = run(&pair, &["card", "edit", &id, "--estimate", "-"]);
    done.code(1);
    assert!(done.stderr.contains("--reason"), "{}", done.show());
    assert_eq!(state(&pair), before, "a clear says why");
    let done = run(
        &pair,
        &[
            "card",
            "edit",
            &id,
            "--estimate",
            "-",
            "--reason",
            "Dropped.",
            "--goal",
            "Again.",
        ],
    );
    done.code(2);
    assert_eq!(state(&pair), before, "no other flag beside it");

    ok(
        &pair,
        &[
            "card",
            "edit",
            &id,
            "--estimate",
            "-",
            "--reason",
            "Dropped.",
            "--as",
            "ana",
        ],
    );
    assert_eq!(show(&pair, &id)["estimate"], Value::Null);
    assert_eq!(
        events(&pair).pop().expect("an event"),
        (
            "card.reestimated".to_owned(),
            json!({"id": id, "old": "2", "new": null, "reason": "Dropped.", "as": "ana"})
        )
    );
}

/// AC-09: a piped `clear` exits 2 first: no card is read, nothing changes.
#[test]
fn ac09_a_piped_clear_exits_2() {
    let pair = Pair::new("ct-ac09-pipe", PRESS);
    let id = make(&pair, "Lead story", &[FRONT]);
    approved(&pair, FRONT, MEANING);
    let before = state(&pair);
    for args in [
        vec![
            "card",
            "clear",
            id.as_str(),
            "--no-effect",
            "--reason",
            "R.",
            "--as",
            "ben",
        ],
        vec![
            "card",
            "clear",
            "C-0099",
            "--no-effect",
            "--reason",
            "R.",
            "--as",
            "ben",
        ],
        vec![
            "card",
            "clear",
            "--group",
            WEEKLY,
            "--no-effect",
            "--reason",
            "R.",
            "--as",
            "ben",
        ],
    ] {
        let done = pair.spec_piped(&pair.main, &args, b"y\nyes\n");
        done.code(2);
        assert_eq!(done.stdout, "", "{}", done.show());
        assert!(done.stderr.contains("terminal"), "{}", done.show());
        assert_eq!(state(&pair), before, "{args:?}");
    }
}

/// AC-09: `--briefed` runs at a root with no `.git`, and so does `spec
/// trace`; `--departs` there exits 2, nothing changed.
#[test]
fn ac09_briefed_runs_with_no_git_and_departs_does_not() {
    let pair = Pair::new("ct-ac09-nogit", PRESS);
    let root = pair.scratch.copy(PRESS, "nogit");
    assert!(!root.join(".git").exists());
    let at = |args: &[&str]| {
        let done = pair.spec_piped(&root, args, b"");
        done.code(0);
        done.stdout
    };
    let make_at = |title: &str| {
        let done = pair.spec_piped(
            &root,
            &[
                "--json",
                "card",
                "new",
                "--title",
                title,
                "--discipline",
                "copy",
                "--target",
                FRONT,
            ],
            b"",
        );
        done.code(0);
        done.json()["id"].as_str().expect("an ID").to_owned()
    };
    let proof = |id: &str| {
        at(&["card", "assign", id, "--reviewer", "ben"]);
        at(&["card", "move", id, "--to", "edit", "--as", "desk"]);
        at(&["card", "move", id, "--to", "proof"]);
    };
    let first = make_at("Lead story");
    proof(&first);
    let done = pair.spec_pty(
        &root,
        &["card", "accept", &first, "--as", "ben", "--briefed"],
        Some("y"),
    );
    assert_eq!(done.code, 0, "{}", done.output);
    assert!(at(&["trace", FRONT]).starts_with(&format!("trace {FRONT} | ")));
    assert!(!root.join(".git").exists(), "no repository made");

    let second = make_at("Side story");
    proof(&second);
    let before = state(&pair);
    let mut args = vec!["card", "accept", second.as_str(), "--as", "ben"];
    let flags = departing(&[FRONT]);
    args.extend_from_slice(&flags);
    let done = pair.spec_pty(&root, &args, Some("y"));
    assert_eq!(done.code, 2, "{}", done.output);
    assert_eq!(state(&pair), before, "nothing changed");
}

// ---------------------------------------------- the output of `spec trace`

/// "`spec trace [REF]`", every form on one hand-worked board: the header's
/// counts, a row per node, a line per card, `--json`'s keys in order, the
/// counts of what a node holds by `parent:`.
#[test]
fn the_rows_of_a_tree() {
    let pair = Pair::new("ct-rows", PRESS);
    let lead = accepted(&pair, "Lead story", &[FRONT]);
    let notices = make(&pair, "Notices", &[BACK]);
    let long = make(&pair, "Long read", &[INNER]);
    make(&pair, "A manual card", &[]);
    approved(&pair, INNER, INNER_MEANING);

    let done = run(&pair, &["trace", WEEKLY]);
    done.code(0);
    assert_eq!(done.stderr, "", "{}", done.show());
    assert_eq!(
        done.stdout,
        format!(
            "trace {WEEKLY} | nodes 5, cards 3 | no card 1, in work 1, suspect 1, departed 0, \
             confirmed 1\n\
             no card | {PLATES} | The plate room | docs/spec/prn-plates.md | cards 0\n\
             no card | {WEEKLY} | The weekly run | docs/spec/prn-weekly.md | cards 0\n\
             in work | {BACK} | The back sheet | {BACK_MD} | cards 1\n\
             \x20 card {notices} | copy/draft | open | Notices\n\
             confirmed | {FRONT} | The front sheet | {FRONT_MD} | cards 1\n\
             \x20 card {lead} | copy/filed | accepted | Lead story\n\
             \x20 uses {BACK} | in work | cards 1\n\
             suspect | {INNER} | The inner sheet | {INNER_MD} | cards 1\n\
             \x20 card {long} | copy/draft | suspect {INNER} | Long read\n"
        )
    );

    let done = json_run(&pair, &["trace", WEEKLY]);
    done.code(0);
    let ordered = Ordered::of(&done.stdout);
    assert_eq!(ordered.keys(), TRACE_KEYS);
    assert_eq!(ordered.get("counts").keys(), COUNT_KEYS);
    let document = done.json();
    assert_eq!(document["ref"], WEEKLY);
    assert_eq!(document["reason"], Value::Null);
    assert_eq!(document["truncated"], false);
    assert_eq!(
        document["counts"],
        json!({"no_card": 1, "in_work": 1, "suspect": 1, "departed": 0, "confirmed": 1})
    );
    assert_eq!(node_ids(&document), [PLATES, WEEKLY, BACK, FRONT, INNER]);
    let Ordered::Array(nodes) = ordered.get("nodes") else {
        panic!("nodes is a list");
    };
    for node in nodes {
        assert_eq!(node.keys(), NODE_KEYS);
        assert_eq!(node.get("counts").keys(), COUNT_KEYS);
        let Ordered::Array(cards) = node.get("cards") else {
            panic!("cards is a list");
        };
        for card in cards {
            assert_eq!(card.keys(), CARD_KEYS);
        }
        let Ordered::Array(uses) = node.get("uses") else {
            panic!("uses is a list");
        };
        for row in uses {
            assert_eq!(row.keys(), USES_KEYS);
        }
    }
    let top = node_of(&document, WEEKLY);
    assert_eq!(top["title"], "The weekly run");
    assert_eq!(top["path"], "docs/spec/prn-weekly.md");
    assert_eq!(top["rollup"], "no card");
    assert_eq!(
        top["counts"],
        json!({"no_card": 1, "in_work": 1, "suspect": 1, "departed": 0, "confirmed": 1}),
        "over what it holds by `parent:`, the holder with no card counted nowhere"
    );
    assert_eq!(
        node_of(&document, FRONT)["counts"],
        json!({"no_card": 0, "in_work": 0, "suspect": 0, "departed": 0, "confirmed": 1})
    );
    let card = card_of(node_of(&document, INNER), &long);
    assert_eq!(card["flow"], "copy");
    assert_eq!(card["column"], "draft");
    assert_eq!(card["title"], "Long read");
}

/// "Scope": no REF: every live document, a decision too, and every section a
/// card targets, no other section; REF's holders and what they hold.
#[test]
fn the_scope_of_a_read() {
    let pair = Pair::new("ct-scope", PRESS);
    let shown = trace(&pair, None);
    assert_eq!(shown["ref"], Value::Null);
    assert_eq!(
        node_ids(&shown),
        [RULING, PLATES, WEEKLY, BACK, FRONT, INNER],
        "by (path, position)"
    );
    assert!(
        run(&pair, &["trace"]).stdout.starts_with(
            "trace all | nodes 6, cards 0 | no card 5, in work 0, suspect 0, \
                         departed 0, confirmed 0\n"
        ),
        "the header"
    );

    make(&pair, "Fold", &[FOLD]);
    assert_eq!(
        node_ids(&trace(&pair, None)),
        [RULING, PLATES, WEEKLY, BACK, FRONT, INNER, FOLD]
    );

    assert_eq!(node_ids(&trace(&pair, Some(FOLD))), [FOLD]);
    let path = trace(&pair, Some(INNER_MD));
    assert!(node_ids(&path).contains(&INNER.to_owned()), "{path}");
    let path = trace(&pair, Some(&format!("{INNER}#{FOLD}")));
    assert_eq!(node_ids(&path), [FOLD]);
}

/// "Exit 0 whatever is listed, 1 only a REF naming nothing, 2 as `spec
/// flow`": the exit-1 document holds its reason; a look-alike ID and an
/// unknown flag exit 2, stdout empty.
#[test]
fn the_exits_of_trace() {
    let pair = Pair::new("ct-exits", PRESS);
    let done = run(&pair, &["trace", "SHT-NOPE"]);
    done.code(1);
    assert!(done.stdout.is_empty(), "{}", done.show());
    assert!(done.stderr.contains("SHT-NOPE"), "{}", done.show());

    let done = json_run(&pair, &["trace", "SHT-NOPE"]);
    done.code(1);
    let ordered = Ordered::of(&done.stdout);
    assert_eq!(ordered.keys(), TRACE_KEYS);
    let document = done.json();
    assert_eq!(document["ref"], "SHT-NOPE");
    assert!(document["reason"].is_string(), "{document}");
    assert_eq!(document["nodes"], json!([]));

    for args in [
        vec!["trace", "S\u{041d}T-FRONT"],
        vec!["trace", "--bogus"],
        vec!["trace", FRONT, BACK],
    ] {
        let done = run(&pair, &args);
        done.code(2);
        assert_eq!(done.stdout, "", "{}", done.show());
    }
    let done = pair.spec_piped(&pair.scratch.dir("nowhere"), &["trace"], b"");
    done.code(2);
}

/// "`spec trace`, no queue: `no card`": a fresh data directory answers, and
/// the read runs no schema step: the slug's database holds no queue table.
#[test]
fn no_queue_reads_no_card_and_makes_none() {
    let pair = Pair::new("ct-no-queue", PRESS);
    let shown = trace(&pair, None);
    assert_eq!(
        shown["counts"],
        json!({"no_card": 5, "in_work": 0, "suspect": 0, "departed": 0, "confirmed": 0})
    );
    assert_eq!(
        pair.sql("SELECT name FROM sqlite_master WHERE name IN ('cards', 'proposals', 'tasks');"),
        "",
        "no queue table after a read"
    );
    assert_eq!(pair.sql("PRAGMA user_version;").trim(), "0");
}

/// "Rules and edge cases": a corrupt card row is skipped with a note naming
/// it; a card with no snapshot, or an entry with a null hash: not suspect, a
/// note; a step-4 row without `verdict` reads `briefed`.
#[test]
fn rows_written_before_this_slice_and_corrupt_ones() {
    let pair = Pair::new("ct-old", PRESS);
    let id = accepted(&pair, "Lead story", &[FRONT]);
    let bad = make(&pair, "Bad", &[BACK]);
    let bare = make(&pair, "No snapshot", &[INNER]);
    pair.sql(&format!(
        "UPDATE cards SET acceptance = '{{\"by\":\"ben\",\"at\":\"2026-10-30T09:00:00Z\",\
         \"note\":null}}', snapshot = NULL WHERE id = '{id}'; UPDATE cards SET evidence = 'not \
         json' WHERE id = '{bad}'; UPDATE cards SET snapshot = NULL WHERE id = '{bare}';"
    ));
    approved(&pair, FRONT, MEANING);
    approved(&pair, INNER, INNER_MEANING);

    let shown = trace(&pair, None);
    assert_eq!(
        node_of(&shown, FRONT)["rollup"],
        "confirmed",
        "no snapshot, not suspect: {shown}"
    );
    assert_eq!(node_of(&shown, INNER)["rollup"], "in work", "{shown}");
    assert_eq!(
        node_of(&shown, BACK)["rollup"],
        "no card",
        "a corrupt row skipped: {shown}"
    );
    let notes = shown["notes"].to_string();
    for name in [id.as_str(), &bad, &bare] {
        assert!(notes.contains(name), "{name}: {notes}");
    }
    let card = show(&pair, &id);
    assert_eq!(card["acceptance"]["verdict"], "briefed", "{card}");
    let done = run(&pair, &["trace"]);
    done.code(0);
    assert!(done.stderr.contains(&bad), "{}", done.show());

    // An entry with a null hash.
    let pair = Pair::new("ct-null-hash", PRESS);
    let id = make(&pair, "Lead story", &[FRONT]);
    pair.sql(&format!(
        "UPDATE cards SET snapshot = '[{{\"id\":\"SHT-FRONT\",\"rev\":1,\"norm_hash\":null,\
         \"also\":[]}}]' WHERE id = '{id}';"
    ));
    let shown = trace(&pair, Some(FRONT));
    assert_eq!(node_of(&shown, FRONT)["rollup"], "in work", "{shown}");
    assert!(shown["notes"].to_string().contains(&id), "{shown}");
}

/// "Snapshot and suspect": a target that no longer resolves is suspect, though
/// no node can list it: every read notes it, the mark stands for a person's
/// clear, which leaves a null hash and the note. An accept keeps that entry.
#[test]
fn a_target_that_no_longer_resolves_is_suspect() {
    let pair = Pair::new("ct-gone", PRESS);
    let id = make(&pair, "The fold", &[FOLD, INNER]);
    let inner_before = pair.within(&[INNER, FOLD]);
    let text = read_text(&pair.main, INNER_MD);
    write(
        &pair.main,
        INNER_MD,
        text.replace(
            "## The fold {#SHT-INNER-FOLD}\n\nNo headline crosses the fold.\n",
            "",
        ),
    );
    pair.commit_all(&pair.main, "the fold goes");
    let shown = trace(&pair, None);
    let note = format!("{id}: `{FOLD}` no longer resolves");
    assert!(shown["notes"].to_string().contains(&note), "{shown}");
    let node = node_of(&shown, INNER);
    let card = card_of(node, &id);
    assert_eq!(
        (&card["mark"], &card["on"]),
        (&json!("suspect"), &json!(INNER)),
        "docs/features/trace-truth.md: the pairs of the node around it changed: {node}"
    );

    let old = show(&pair, &id)["snapshot"][0]["norm_hash"].clone();
    clear_ok(&pair, &id, "The fold is gone for good.");
    let event = events_of(&pair, "card.cleared").pop().expect("the event");
    assert_eq!(
        event["nodes"],
        json!([
            {"id": FOLD, "rev": 1, "from": old, "to": null},
            {"id": INNER, "rev": 1, "from": inner_before, "to": norm(&pair, INNER)},
        ]),
        "{event}"
    );
    let card = show(&pair, &id);
    assert_eq!(card["snapshot"][0]["norm_hash"], Value::Null, "{card}");
    assert!(
        trace(&pair, None)["notes"]
            .to_string()
            .contains(&format!("{id}: `{FOLD}` has a null hash")),
        "noted on every read"
    );

    // An accept freezes every target that reads, and keeps the gone one's
    // entry: the card stays suspect on it, for a person's clear.
    let pair = Pair::new("ct-gone-accept", PRESS);
    let id = make(&pair, "The fold", &[FOLD, INNER]);
    to_proof(&pair, &id);
    let old = show(&pair, &id)["snapshot"][0].clone();
    let text = read_text(&pair.main, INNER_MD);
    write(
        &pair.main,
        INNER_MD,
        text.replace(
            "## The fold {#SHT-INNER-FOLD}\n\nNo headline crosses the fold.\n",
            "",
        ),
    );
    pair.commit_all(&pair.main, "the fold goes");
    accept_ok(&pair, &id, &["--briefed"]);
    let card = show(&pair, &id);
    assert_eq!(card["snapshot"][0], old, "the entry stays: {card}");
    assert!(
        trace(&pair, None)["notes"]
            .to_string()
            .contains(&format!("{id}: `{FOLD}` no longer resolves")),
        "still suspect on it"
    );
    clear_ok(&pair, &id, "The fold is gone for good.");
    assert_eq!(
        show(&pair, &id)["snapshot"][0]["norm_hash"],
        Value::Null,
        "a clear takes a null hash"
    );
}

/// "Snapshot and suspect", the reading `edit --target` takes: a target kept
/// keeps its entry, so an edit takes no mark away without a reason; a new one
/// is frozen now.
#[test]
fn edit_target_keeps_a_kept_targets_mark() {
    let pair = Pair::new("ct-retarget", PRESS);
    let id = make(&pair, "Lead story", &[FRONT]);
    let old = show(&pair, &id)["snapshot"][0].clone();
    approved(&pair, FRONT, MEANING);
    ok(
        &pair,
        &["card", "edit", &id, "--target", FRONT, "--target", BACK],
    );
    let card = show(&pair, &id);
    assert_eq!(card["snapshot"][0], old, "{card}");
    assert_eq!(
        card["snapshot"][1]["norm_hash"],
        norm(&pair, BACK),
        "{card}"
    );
    assert_eq!(rollup(&pair, FRONT), "suspect");
    assert_eq!(rollup(&pair, BACK), "in work");
}

/// "A `null` hash ... not suspect, a note": the node's own hash null now, a
/// front matter that fails a type: the card clean, the read noting it; a
/// sound file again compares as ever.
#[test]
fn a_null_hash_now_is_not_suspect_and_is_noted() {
    let pair = Pair::new("ct-null-now", PRESS);
    let id = make(&pair, "Lead story", &[FRONT]);
    let text = read_text(&pair.main, FRONT_MD);
    write(
        &pair.main,
        FRONT_MD,
        text.replace("reviewed: 2026-10-01\n", "reviewed: 2026-10-01\nlinks: 7\n"),
    );
    assert_eq!(pair.node(&pair.main, FRONT).norm_hash, None, "the premise");
    let shown = trace(&pair, Some(FRONT));
    assert_eq!(
        card_of(node_of(&shown, FRONT), &id)["mark"],
        "open",
        "{shown}"
    );
    assert!(
        shown["notes"]
            .to_string()
            .contains(&format!("{id}: `{FRONT}` has a null hash")),
        "{shown}"
    );
    write(&pair.main, FRONT_MD, text.replace(MEANING.0, MEANING.1));
    assert_eq!(rollup(&pair, FRONT), "suspect");
}

/// "Either re-freezes", on a real broken document: an accept while a target's
/// front matter fails finds a null hash, has nothing to freeze and keeps the
/// entry; the file mended, the card compares as ever: clean on the old text,
/// suspect on a new one.
#[test]
fn an_accept_over_a_broken_document_keeps_the_entry() {
    let pair = Pair::new("ct-broken-accept", PRESS);
    let id = make(&pair, "Lead story", &[FRONT, BACK]);
    to_proof(&pair, &id);
    let frozen = show(&pair, &id)["snapshot"].clone();
    let text = read_text(&pair.main, FRONT_MD);
    write(
        &pair.main,
        FRONT_MD,
        text.replace("reviewed: 2026-10-01\n", "reviewed: 2026-10-01\nlinks: 7\n"),
    );
    assert_eq!(pair.node(&pair.main, FRONT).norm_hash, None, "the premise");
    accept_ok(&pair, &id, &["--briefed"]);
    let card = show(&pair, &id);
    assert_eq!(card["snapshot"], frozen, "no entry nulled: {card}");

    write(&pair.main, FRONT_MD, &text);
    assert_eq!(rollup(&pair, FRONT), "confirmed", "the old text again");
    write(&pair.main, FRONT_MD, text.replace(MEANING.0, MEANING.1));
    assert_eq!(rollup(&pair, FRONT), "suspect", "still comparable");
}

/// Target forms: a feature's scoped ID and an ID-less document's path are
/// frozen, compared and listed under the name `targets` holds, on spec-a and
/// the non-Rust spec-c.
#[test]
fn scoped_and_path_targets_are_compared() {
    let pair = Pair::new("ct-forms-a", FIXTURES[0].0);
    let scoped = "stamina-tuning/AC-07";
    let id = doc(
        &pair,
        &["card", "new", "--title", "Scoped", "--target", scoped],
    )["id"]
        .as_str()
        .expect("an ID")
        .to_owned();
    let card = show(&pair, &id);
    assert_eq!(card["snapshot"][0]["id"], scoped, "{card}");
    assert!(card["snapshot"][0]["norm_hash"].is_string(), "{card}");
    replace(
        &pair.main,
        "docs/features/stamina-tuning.md",
        CHECK_FROM,
        CHECK_TO,
    );
    assert_eq!(
        card_line(&pair, &pair.main, scoped, &id),
        format!("  card {id} | plain/backlog | suspect {scoped} | Scoped")
    );

    let pair = Pair::new("ct-forms-c", NON_RUST.0);
    let path = "docs/features/late-return.md";
    let id = doc(
        &pair,
        &["card", "new", "--title", "By path", "--target", path],
    )["id"]
        .as_str()
        .expect("an ID")
        .to_owned();
    let card = show(&pair, &id);
    assert_eq!(card["targets"], json!([path]), "{card}");
    assert_eq!(card["snapshot"][0]["id"], path, "{card}");
    assert!(card["snapshot"][0]["norm_hash"].is_string(), "{card}");
    assert_eq!(
        card_line(&pair, &pair.main, path, &id),
        format!("  card {id} | plain/backlog | open | By path")
    );
    replace(&pair.main, path, "keep the day cap", "lift the day cap");
    let shown = trace(&pair, Some(path));
    assert_eq!(node_of(&shown, path)["rollup"], "suspect", "{shown}");
    assert_eq!(
        card_line(&pair, &pair.main, path, &id),
        format!("  card {id} | plain/backlog | suspect {path} | By path")
    );
}

/// "Clearing": the default title takes the room left: at `TITLE_MAX` a
/// correction's `Correct <title>` is over its cap, refused with the field
/// named, nothing changed; `--title` answers it.
#[test]
fn a_full_title_needs_its_own_correction_title() {
    let pair = Pair::new("ct-title", PRESS);
    let title = "t".repeat(256);
    let id = accepted(&pair, &title, &[FRONT]);
    approved(&pair, FRONT, MEANING);
    let before = state(&pair);
    let done = clear(
        &pair,
        &id,
        &["--needs-correction", "--reason", "R.", "--estimate", "1"],
    );
    assert_eq!(done.code, 1, "{}", done.output);
    assert!(done.output.contains("title"), "{}", done.output);
    assert_eq!(state(&pair), before);
    let done = clear(
        &pair,
        &id,
        &[
            "--needs-correction",
            "--reason",
            "R.",
            "--estimate",
            "1",
            "--title",
            "Shorter",
        ],
    );
    assert_eq!(done.code, 0, "{}", done.output);
}

/// "REF": `@rev` is ignored with a note, as `spec impact`'s; a card's title
/// holding a control character prints escaped, one line.
#[test]
fn a_ref_with_a_revision_and_a_title_with_a_control_character() {
    let pair = Pair::new("ct-escapes", PRESS);
    let id = make(&pair, "Lead\u{7}story", &[FRONT]);
    let done = run(&pair, &["trace", &format!("{FRONT}@1")]);
    done.code(0);
    assert!(
        done.stderr.contains("`spec trace` reads the current files"),
        "{}",
        done.show()
    );
    assert!(!done.stdout.contains('\u{7}'), "{}", done.show());
    let shown = trace(&pair, Some(FRONT));
    assert_eq!(
        card_of(node_of(&shown, FRONT), &id)["title"],
        "Lead\u{7}story",
        "JSON holds it"
    );
}

/// "Backup": marks and verdicts come through `spec export-state` and
/// `import-state` in a fresh `HOME`: the same trace, byte for byte; no schema
/// step: `snapshot`, `acceptance` hold JSON already.
#[test]
fn the_backup_keeps_what_a_trace_reads() {
    let pair = Pair::new("ct-backup", PRESS);
    accepted(&pair, "Lead story", &[FRONT]);
    let long = make(&pair, "Long read", &[INNER]);
    to_proof(&pair, &long);
    accept_ok(&pair, &long, &departing(&[INNER]));
    approved(&pair, FRONT, MEANING);
    let want = ok(&pair, &["--json", "trace"]);
    assert!(want.contains("\"mark\":\"suspect\""), "{want}");

    let file = pair.scratch.join("queue.jsonl");
    let env_of = |home: &Path| Env {
        cwd: pair.main.clone(),
        home: Some(home.as_os_str().to_owned()),
        xdg_data_home: None,
    };
    specengine_cli::export_state(
        &env_of(&pair.home),
        &Globals::default(),
        &specengine_cli::ExportStateRequest {
            out: Some(file.clone()),
            now: LATER.to_owned(),
            git: pair.git_env(&pair.main),
        },
    )
    .expect("exported");
    let fresh = pair.scratch.home("fresh");
    let mut consent = |_: &str| true;
    specengine_cli::import_state(
        &env_of(&fresh),
        &Globals::default(),
        &specengine_cli::ImportStateRequest { file },
        &mut consent,
    )
    .expect("imported");
    let done = spec_with(
        &pair.main,
        &["--json", "trace"],
        &[("HOME", fresh.as_os_str())],
    );
    done.code(0);
    assert_eq!(done.stdout, want);
}

/// "`also` keeps the last 16": seventeen editorial applies leave the sixteen
/// newest, oldest first.
#[test]
fn also_keeps_the_last_sixteen() {
    let pair = Pair::new("ct-also", PRESS);
    let id = make(&pair, "Notices", &[BACK]);
    let mut hashes = Vec::new();
    let mut last = "the tide table".to_owned();
    for number in 0..17 {
        let next = format!("the tide table {number}");
        editorial(&pair, BACK, (&last, &next));
        hashes.push(norm(&pair, BACK));
        last = next;
    }
    assert_eq!(
        show(&pair, &id)["snapshot"][0]["also"],
        json!(hashes[1..].to_vec())
    );
    assert_eq!(rollup(&pair, BACK), "in work");
}

/// "Determinism": a repeat of every read gives the same bytes, and a read
/// leaves the queue as it was.
#[test]
fn a_repeat_gives_the_same_bytes() {
    let pair = Pair::new("ct-repeat", PRESS);
    make(&pair, "Masthead", &[HEAD]);
    accepted(&pair, "Lead story", &[FRONT]);
    let id = make(&pair, "Long read", &[INNER]);
    to_proof(&pair, &id);
    accept_ok(&pair, &id, &departing(&[INNER]));
    approved(&pair, HEAD, HEAD_MEANING);
    let before = state(&pair);
    for args in [
        vec!["trace"],
        vec!["--json", "trace"],
        vec!["trace", WEEKLY],
        vec!["--json", "trace", WEEKLY],
        vec!["impact", HEAD],
        vec!["--json", "impact", HEAD],
    ] {
        assert_eq!(ok(&pair, &args), ok(&pair, &args), "{args:?}");
    }
    assert_eq!(state(&pair), before);
}

/// `count` cards of 250-byte titles on `target`, frozen as it reads now.
fn many_cards(pair: &Pair, target: &str, count: usize) {
    let hash = norm(pair, target);
    let within = match target {
        FRONT => front_within(pair),
        INNER => pair.within(&[INNER, FOLD]),
        _ => hash.clone(),
    };
    let mut queue = pair.queue();
    for _ in 0..count {
        queue
            .create_card_frozen(
                &NewCard {
                    flow: "copy".to_owned(),
                    column: "draft".to_owned(),
                    title: "x".repeat(250),
                    goal: None,
                    discipline: Some("copy".to_owned()),
                    targets: vec![target.to_owned()],
                    criteria: Vec::new(),
                    assignee: None,
                    reviewer: None,
                    estimate: None,
                    due: None,
                    parent: None,
                    corrects: None,
                    by: "local".to_owned(),
                },
                &[CardFrozen {
                    id: target.to_owned(),
                    rev: Some(1),
                    norm_hash: Some(hash.clone()),
                    within_hash: Some(within.clone()),
                    also: Vec::new(),
                }],
                "2026-10-30T09:00:00Z",
            )
            .expect("a card");
    }
}

/// "cap as `spec flow`'s": past the cap the rows stop within it, a tail
/// names what is cut, JSON holds what was printed; the counts stay whole.
#[test]
fn a_long_trace_is_cut_with_a_tail() {
    let pair = Pair::new("ct-cap", PRESS);
    many_cards(&pair, FRONT, 200);
    let done = run(&pair, &["trace"]);
    done.code(0);
    assert!(
        done.stdout.chars().count() <= specengine_cli::OUTPUT_CAP_CHARS,
        "{} chars",
        done.stdout.chars().count()
    );
    assert_eq!(done.stderr, "", "no note: {}", done.stderr.len());
    let tail = done.stdout.lines().last().expect("a line");
    assert_eq!(tail, "[truncated: 2 of 6 nodes not shown; give a REF]");
    assert!(
        done.stdout
            .starts_with("trace all | nodes 6, cards 200 | no card 4, in work 1, suspect 0,"),
        "the header counts whole: {}",
        &done.stdout[..80]
    );

    let document = doc(&pair, &["trace"]);
    assert_eq!(document["truncated"], true);
    assert_eq!(document["omitted"], 2);
    assert_eq!(
        node_ids(&document),
        [RULING, PLATES, WEEKLY, BACK],
        "what was printed"
    );
    assert_eq!(
        document["counts"],
        json!({"no_card": 4, "in_work": 1, "suspect": 0, "departed": 0, "confirmed": 0})
    );
}

/// The lines of `spec trace` args that are card rows, and the tail.
fn cut(done: &Run) -> (usize, String) {
    let rows = done
        .stdout
        .lines()
        .filter(|line| line.starts_with("  card "))
        .count();
    let tail = done.stdout.lines().last().expect("a tail").to_owned();
    (rows, tail)
}

/// "cap as `spec flow`'s", a first node over the cap alone: never an empty
/// answer: its row and the cards that fit, the text within the cap, the tail
/// naming what is cut, `omitted` counting rows, JSON holding the printed ones,
/// the counts whole.
#[test]
fn a_node_over_the_cap_shows_its_row_and_the_cards_that_fit() {
    let pair = Pair::new("ct-cap-node", PRESS);
    many_cards(&pair, INNER, 200);
    let done = run(&pair, &["trace", INNER]);
    done.code(0);
    assert!(
        done.stdout.chars().count() <= specengine_cli::OUTPUT_CAP_CHARS,
        "{} chars",
        done.stdout.chars().count()
    );
    let lines: Vec<&str> = done.stdout.lines().collect();
    assert_eq!(
        lines[0],
        format!(
            "trace {INNER} | nodes 1, cards 200 | no card 0, in work 1, suspect 0, departed 0, \
             confirmed 0"
        ),
        "the header counts whole"
    );
    assert_eq!(
        lines[1],
        format!("in work | {INNER} | The inner sheet | {INNER_MD} | cards 200"),
        "the row counts whole"
    );
    let (rows, tail) = cut(&done);
    assert!(rows > 0, "the cards that fit: {tail}");
    let left = 200 - rows;
    assert!(left > 0, "{tail}");
    assert_eq!(
        tail,
        format!("[truncated: {left} of 200 cards of {INNER} not shown; give a narrower REF]")
    );
    let document = doc(&pair, &["trace", INNER]);
    assert_eq!(document["truncated"], true);
    assert_eq!(document["omitted"], left, "rows, not nodes");
    let node = node_of(&document, INNER);
    assert_eq!(node["cards"].as_array().expect("cards").len(), rows);
    assert_eq!(
        node["counts"],
        json!({"no_card": 0, "in_work": 1, "suspect": 0, "departed": 0, "confirmed": 0})
    );
}

/// The same cut with `uses` lines behind the cards, and a whole read whose
/// first node is over the cap: the tail names each part; a read with no REF
/// asks for one, and `omitted` adds the nodes.
#[test]
fn the_tail_of_a_cut_names_each_part() {
    let pair = Pair::new("ct-cap-parts", PRESS);
    many_cards(&pair, HEAD, 200);
    let done = run(&pair, &["trace", HEAD]);
    done.code(0);
    let (rows, tail) = cut(&done);
    let left = 200 - rows;
    assert_eq!(
        tail,
        format!(
            "[truncated: {left} of 200 cards of {HEAD}, 1 of 1 uses of {HEAD} not shown; give a \
             narrower REF]"
        )
    );
    let document = doc(&pair, &["trace", HEAD]);
    assert_eq!(document["omitted"], left + 1, "cards and `uses`");

    let pair = Pair::new("ct-cap-first", PRESS);
    many_cards(&pair, RULING, 200);
    let done = run(&pair, &["trace"]);
    done.code(0);
    assert!(
        done.stdout.chars().count() <= specengine_cli::OUTPUT_CAP_CHARS,
        "{} chars",
        done.stdout.chars().count()
    );
    let (rows, tail) = cut(&done);
    let left = 200 - rows;
    assert_eq!(
        tail,
        format!("[truncated: {left} of 200 cards of {RULING}, 5 of 6 nodes not shown; give a REF]")
    );
    let document = doc(&pair, &["trace"]);
    assert_eq!(document["omitted"], left + 5, "cards and nodes");
    assert_eq!(node_ids(&document), [RULING], "the node shown in part");
}

// ----------------------------------------------------------------- genre

/// A node of the non-Rust spec-c and a change of its meaning.
const DAY_CAP: (&str, (&str, &str)) = ("FEE-DAY-CAP", ("never exceed 12.00", "never exceed 15.00"));

/// One default flow, no `[card]`: the card to `review`, accepted by its
/// reviewer: `confirmed`; a meaning change `suspect`; a clear `confirmed`.
fn walk(pair: &Pair, node: &str, change: (&str, &str)) {
    let id = doc(
        pair,
        &["card", "new", "--title", "A task", "--target", node],
    )["id"]
        .as_str()
        .expect("an ID")
        .to_owned();
    ok(pair, &["card", "assign", &id, "--reviewer", "ben"]);
    ok(pair, &["card", "move", &id, "--to", "review"]);
    accept_ok(pair, &id, &["--briefed"]);
    assert_eq!(rollup(pair, node), "confirmed", "{node}");
    approved(pair, node, change);
    assert_eq!(rollup(pair, node), "suspect", "{node}");
    clear_ok(pair, &id, "No effect.");
    assert_eq!(rollup(pair, node), "confirmed", "{node}");
}

/// ADR-0008: one code on a game's spec, a command-line tool's in another
/// language and a web service's, no `[card]`, no column named anywhere.
#[test]
fn the_same_walk_on_other_projects() {
    for case in &CASES {
        let pair = Pair::of(&format!("ct-genre-{}", case.fixture), case);
        walk(&pair, case.target, case.first);
    }
    let pair = Pair::new("ct-genre-spec-c", NON_RUST.0);
    walk(&pair, DAY_CAP.0, DAY_CAP.1);
}

/// The brief every test here reads, stated once: `uses`, the accepted decision
/// and its link, in the fixture itself.
#[test]
fn the_fixture_holds_the_brief() {
    let scratch = Scratch::new("ct-fixture");
    let root = scratch.copy(PRESS, "copy");
    let back = fs::read_to_string(root.join(BACK_MD)).expect("the back sheet");
    assert!(
        back.contains("uses: [SHT-FRONT-HEAD]"),
        "the fixture's `uses`: {back}"
    );
    let ruling = fs::read_to_string(root.join(RULING_MD)).expect("the ruling");
    assert!(ruling.contains("status: accepted"), "{ruling}");
    assert!(ruling.contains("constrains: [SHT-FRONT]"), "{ruling}");
    let done = common::spec(&scratch.home("h"), &root, &["check"]);
    done.code(0);
}
