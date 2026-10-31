//! docs/features/trace-truth.md (RM-0066), the CLI half: `within_hash`, the
//! counts that stop at containers and `spec card show --marks`, through the
//! binary on scratch git repositories of `fixtures/trace-almanac` (a village
//! almanac: `STEP-SET` holds the section `STEP-SET-PROOF`, so a card stands on
//! a document and its rule in a section of it), with `fixtures/card-press` for
//! the genre. A test adds what the fixture lacks to its own copy: the
//! fixture's `nodes 8` are pinned. Cards are cut through the library. The tool
//! is MCP `mcp_trace.rs`, the daemon http `cards.rs`, the store's half
//! `queue_trace.rs`, core's `trace_within.rs`.
//!
//! - AC-01: text under a section marks the document's card in `spec trace`,
//!   `spec flow --trace` and `spec card list --marks`; `spec check` and `spec
//!   gate` never read it. M: `within_hash` over the target alone.
//! - AC-02: the mark stops at the file: `parent:`, `uses` and siblings stay
//!   clean. M: walking `held_documents` into the hash.
//! - AC-03: whitespace and a moved section mark nothing, a new `{#ID}` section
//!   marks, the recipe's bytes. M: pairs in file order; pairs without names.
//! - AC-04: an entry with no key: no false mark, one note, its own text still
//!   marks, a clear or an accept writes the key. M: an absent key read as a
//!   change.
//! - AC-05: `--editorial` of the section vouches for the document here and in
//!   a worktree without the commit, one `card.refreshed`, one more `revision`.
//!   M: `card_refresh` without holders; `nodes` not deduplicated. A completion
//!   by a hand commit vouches a `within_hash` only when every member its
//!   trailers do not name reads the same in both blobs. M: `within_hash`
//!   vouched without the compare. An entry's `also` holds the kind it compares,
//!   and a change of a node's members vouches nothing. M: `vouched` ignoring
//!   the kind; `within_pair` without the member-name guard.
//! - AC-06: a clear, a group, a correction's accept freeze `within_hash`. M:
//!   `cleared` freezing `norm_hash` alone.
//! - AC-07: a holder with no card of its own is counted nowhere; its row word
//!   stays. M: dropping every `no card`; keeping holders.
//! - AC-08: `spec card show --marks` equals its list row; plain `show` holds
//!   `null`; a broken index exits 2. M: keys not joined in `run_show`;
//!   `--marks` swallowing the index error.
//! - "Rules and edge cases", the decisions, the determinism, two projects.
//!
//! Every expected value below is worked out from the spec and the fixture,
//! never read back from a run. Non-Latin characters in Rust sources are
//! Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod task_common;

use std::collections::BTreeSet;
use std::path::Path;

use common::decision::PtyRun;
use common::proposal::{LATER, Pair, refused};
use common::{Run, read_text, replace, write};
use serde_json::{Map, Value, json};
use specengine_cli::{
    ApproveFlags, ApproveRequest, CardAcceptRequest, CardAssignRequest, CardClearOutcome,
    CardClearRequest, CardMoveRequest, CardNewRequest, CreateRequest, Env, Exit, Globals,
    ProposedText, StageBody, StageRequest, approve_editorial, card_accept, card_assign, card_clear,
    card_move, card_new,
};
use specengine_store::{ProposalStatus, QUEUE_SCHEMA_VERSION, b3_hash};
use task_common::Ordered;

const ALMANAC: &str = "trace-almanac";
const PRESS: &str = "card-press";

const BOOK: &str = "BOOK-ALMANAC";
const GATHER: &str = "PART-GATHER";
const PRINT: &str = "PART-PRINT";
const CALL: &str = "STEP-CALL";
const SORT: &str = "STEP-SORT";
const READ: &str = "STEP-READ";
const SET: &str = "STEP-SET";
const BIND: &str = "STEP-BIND";
const PROOF: &str = "STEP-SET-PROOF";

/// Sections a test adds to its own copy.
const MARGIN: &str = "STEP-SET-MARGIN";
const INK: &str = "STEP-SET-PROOF-INK";

const CALL_MD: &str = "docs/spec/step-call.md";
const SET_MD: &str = "docs/spec/step-set.md";

/// `STEP-SET-PROOF`'s one line, the last of the file.
const PROOF_LINE: &str = "One proof goes to the reading room before any copy is pulled.\n";

/// The whole section.
const PROOF_SECTION: &str = "## The proof sheet {#STEP-SET-PROOF}\n\nOne proof goes to the \
                             reading room before any copy is pulled.\n";

/// The sibling [`add_margin`] writes.
const MARGIN_SECTION: &str =
    "## The margin {#STEP-SET-MARGIN}\n\nTwo fingers of margin run round each page.\n";

/// The ID-less document [`add_loose`] writes, and its section.
const LOOSE_MD: &str = "docs/spec/loose-notes.md";
const RULE: &str = "STEP-NOTES-RULE";

/// A change of meaning, of each node's own text.
const CALL_MEANING: (&str, &str) = ("on the first of the month", "on the last of the month");
const SET_MEANING: (&str, &str) = ("opens on a right-hand page", "opens on a left-hand page");
const PROOF_MEANING: (&str, &str) = ("One proof goes", "Two proofs go");
const PROOF_AGAIN: (&str, &str) = ("Two proofs go", "Three proofs go");

/// `STEP-CALL`'s text after [`CALL_MEANING`], changed again.
const TENTH: (&str, &str) = ("on the last of the month", "on the tenth of the month");

/// A row's keys a read of marks fills, `spec trace`'s keys of a card.
const MARK_KEYS: [&str; 4] = ["mark", "on", "discrepancy", "correction"];

/// `{no_card, in_work, suspect, departed, confirmed}`.
const COUNT_KEYS: [&str; 5] = ["no_card", "in_work", "suspect", "departed", "confirmed"];

/// An item's summary, observed and documented text.
const SUMMARY: &str = "The result left the brief.";
const OBSERVED: &str = "The tides are set before the recipes.";
const DOCUMENTED: &str = "The tides stand first in each month.";

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

/// `spec --json args` in `cwd`, exit 0: the document.
fn doc_in(pair: &Pair, cwd: &Path, args: &[&str]) -> Value {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    let done = pair.spec_piped(cwd, &all, b"");
    done.code(0);
    done.json()
}

/// `spec --json args`, exit 0: the document.
fn doc(pair: &Pair, args: &[&str]) -> Value {
    doc_in(pair, &pair.main, args)
}

/// The library's `Env` for the main worktree.
fn env(pair: &Pair) -> Env {
    pair.env(&pair.main)
}

/// A card of `discipline` on `targets`: its ID.
fn cut_in(pair: &Pair, discipline: &str, title: &str, targets: &[&str]) -> String {
    let outcome = card_new(
        &env(pair),
        &Globals::default(),
        &CardNewRequest {
            title: title.to_owned(),
            discipline: Some(discipline.to_owned()),
            targets: targets.iter().map(|target| (*target).to_owned()).collect(),
            now: LATER.to_owned(),
            ..CardNewRequest::default()
        },
    )
    .unwrap_or_else(|error| panic!("card new {title}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    outcome.id.expect("a card's ID")
}

/// A card of the almanac's flow on `targets`: its ID.
fn cut(pair: &Pair, title: &str, targets: &[&str]) -> String {
    cut_in(pair, "editing", title, targets)
}

/// The card at `review`: `ana` its assignee, `ben` its reviewer.
fn to_review(pair: &Pair, id: &str) {
    let globals = Globals::default();
    let assigned = card_assign(
        &env(pair),
        &globals,
        &CardAssignRequest {
            id: id.to_owned(),
            assignee: Some("ana".to_owned()),
            reviewer: Some("ben".to_owned()),
            now: LATER.to_owned(),
            ..CardAssignRequest::default()
        },
    )
    .unwrap_or_else(|error| panic!("assign {id}: {error}"));
    assert_eq!(assigned.exit(), Exit::Answered, "{assigned:?}");
    let moved = card_move(
        &env(pair),
        &globals,
        &CardMoveRequest {
            id: id.to_owned(),
            to: Some("review".to_owned()),
            now: LATER.to_owned(),
            ..CardMoveRequest::default()
        },
    )
    .unwrap_or_else(|error| panic!("move {id}: {error}"));
    assert_eq!(moved.exit(), Exit::Answered, "{moved:?}");
}

/// `spec card accept id --as ben`, a person's consent given.
fn accept(pair: &Pair, id: &str, request: CardAcceptRequest) {
    let mut consent = |_: &str| true;
    let outcome = card_accept(
        &env(pair),
        &Globals::default(),
        &CardAcceptRequest {
            id: id.to_owned(),
            actor: "ben".to_owned(),
            now: LATER.to_owned(),
            ..request
        },
        &mut consent,
    )
    .unwrap_or_else(|error| panic!("accept {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
}

/// [`accept`] `--briefed`.
fn accept_briefed(pair: &Pair, id: &str) {
    accept(
        pair,
        id,
        CardAcceptRequest {
            briefed: true,
            ..CardAcceptRequest::default()
        },
    );
}

/// A card accepted `--briefed` on `targets`: its ID.
fn accepted(pair: &Pair, title: &str, targets: &[&str]) -> String {
    let id = cut(pair, title, targets);
    to_review(pair, &id);
    accept_briefed(pair, &id);
    id
}

/// A card accepted `--departs` from `target`: its ID.
fn departed(pair: &Pair, title: &str, target: &str) -> String {
    let id = cut(pair, title, &[target]);
    to_review(pair, &id);
    accept(
        pair,
        &id,
        CardAcceptRequest {
            departs: vec![target.to_owned()],
            summary: Some(SUMMARY.to_owned()),
            observed: Some(OBSERVED.to_owned()),
            documented: Some(DOCUMENTED.to_owned()),
            ..CardAcceptRequest::default()
        },
    );
    id
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

/// `spec card clear id --no-effect --as ben` on a terminal answering `y`.
fn clear(pair: &Pair, id: &str, reason: &str) -> PtyRun {
    pair.spec_pty(
        &pair.main,
        &[
            "card",
            "clear",
            id,
            "--no-effect",
            "--reason",
            reason,
            "--as",
            "ben",
        ],
        Some("y"),
    )
}

/// `spec card clear --group reference --no-effect --as ben`, answering `y`.
fn group(pair: &Pair, reference: &str) -> PtyRun {
    pair.spec_pty(
        &pair.main,
        &[
            "card",
            "clear",
            "--group",
            reference,
            "--no-effect",
            "--reason",
            "A reprint.",
            "--as",
            "ben",
        ],
        Some("y"),
    )
}

/// `spec card clear id --needs-correction`: the correction's ID.
fn needs_correction(pair: &Pair, id: &str) -> String {
    let mut consent = |_: &str| true;
    let outcome = card_clear(
        &env(pair),
        &Globals::default(),
        &CardClearRequest {
            id: id.to_owned(),
            outcome: CardClearOutcome::NeedsCorrection,
            reason: "It changed the result.".to_owned(),
            estimate: Some("1".to_owned()),
            title: None,
            revision: None,
            actor: Some("ben".to_owned()),
            now: LATER.to_owned(),
        },
        &mut consent,
    )
    .unwrap_or_else(|error| panic!("clear {id}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    doc(pair, &["card", "list"])["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .find(|card| card["corrects"] == id)
        .and_then(|card| card["id"].as_str())
        .unwrap_or_else(|| panic!("no correction of {id}"))
        .to_owned()
}

/// `spec show`'s `norm_hash` of `id` in the main worktree.
fn norm(pair: &Pair, id: &str) -> String {
    pair.node(&pair.main, id).norm_hash.expect("a norm hash")
}

/// `spec card show id --json`.
fn show(pair: &Pair, id: &str) -> Value {
    doc(pair, &["card", "show", id])
}

/// The card's first snapshot entry.
fn entry(pair: &Pair, id: &str) -> Value {
    show(pair, id)["snapshot"][0].clone()
}

/// The card's `revision`.
fn revision(pair: &Pair, id: &str) -> u64 {
    show(pair, id)["revision"]
        .as_str()
        .expect("a revision")
        .parse()
        .expect("a number")
}

/// `id`'s row of `spec card list --marks --json` in `cwd`.
fn row_in(pair: &Pair, cwd: &Path, id: &str) -> Value {
    let list = doc_in(pair, cwd, &["card", "list", "--marks"]);
    list["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .find(|card| card["id"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("no row {id}: {list}"))
}

/// `id` reads `word` on `on` in the list, in `cwd`.
fn reads_in(pair: &Pair, cwd: &Path, id: &str, word: &str, on: Option<&str>) {
    let row = row_in(pair, cwd, id);
    assert_eq!(row["mark"], word, "{id} in {cwd:?}: {row}");
    assert_eq!(
        row["on"],
        on.map_or(Value::Null, |on| json!(on)),
        "{id} in {cwd:?}: {row}"
    );
}

/// [`reads_in`] the main worktree.
fn reads(pair: &Pair, id: &str, word: &str, on: Option<&str>) {
    reads_in(pair, &pair.main, id, word, on);
}

/// The node `id` of a `spec trace` or `spec flow` document.
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

/// `{no_card, in_work, suspect, departed, confirmed}`.
fn counts(no_card: u64, in_work: u64, suspect: u64, departed: u64, confirmed: u64) -> Value {
    json!({"no_card": no_card, "in_work": in_work, "suspect": suspect,
        "departed": departed, "confirmed": confirmed})
}

/// `counts`es added key by key.
fn sum<'v>(counted: impl IntoIterator<Item = &'v Value>) -> Value {
    let mut total = Map::new();
    let counted: Vec<&Value> = counted.into_iter().collect();
    for key in COUNT_KEYS {
        let added: u64 = counted
            .iter()
            .map(|counts| counts[key].as_u64().expect("a count"))
            .sum();
        total.insert(key.to_owned(), json!(added));
    }
    Value::Object(total)
}

/// The payloads of `kind`, in `seq` order.
fn events_of(pair: &Pair, kind: &str) -> Vec<Value> {
    pair.events()
        .into_iter()
        .filter(|event| event.event_type == kind)
        .map(|event| event.payload)
        .collect()
}

/// What a refusal must leave: the cards' rows raw, every row of the queue.
fn state(pair: &Pair) -> (String, String) {
    (
        pair.sql("SELECT * FROM cards ORDER BY id;"),
        pair.queue().dump().expect("dump"),
    )
}

/// Text appended under `STEP-SET-PROOF` by hand, committed: nobody vouches.
fn append_under_proof(pair: &Pair) {
    replace(
        &pair.main,
        SET_MD,
        PROOF_LINE,
        &format!("{PROOF_LINE}A second proof goes to the printer.\n"),
    );
    pair.commit_all(&pair.main, "more under the section");
}

/// The sibling section `STEP-SET-MARGIN`, after `STEP-SET-PROOF`, committed.
fn add_margin(pair: &Pair) {
    let text = read_text(&pair.main, SET_MD);
    write(&pair.main, SET_MD, format!("{text}\n{MARGIN_SECTION}"));
    pair.commit_all(&pair.main, "a sibling section");
}

/// An ID-less document with one ID section, committed.
fn add_loose(pair: &Pair) {
    write(
        &pair.main,
        LOOSE_MD,
        format!(
            "---\nclass: canon\ntitle: Loose notes\nowner: owner\nreviewed: 2026-10-01\n---\n\n\
             # Loose notes\n\nNothing here has a name of its own.\n\n## The rule {{#{RULE}}}\n\n\
             A note stays up for a week.\n"
        ),
    );
    pair.commit_all(&pair.main, "an ID-less document");
}

/// A document of the almanac in `parent`, its own hash a plain one.
fn plain_document(id: &str, parent: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\ntitle: {id}\nparent: {parent}\nowner: owner\n\
         reviewed: 2026-10-01\n---\n\n# {id}\n\nA body.\n"
    )
}

/// The stored entry of `id` as a build before this slice wrote it: no key.
fn strip_key(pair: &Pair, id: &str) {
    pair.sql(&format!(
        "UPDATE cards SET snapshot = json_remove(snapshot, '$[0].within_hash') WHERE id = '{id}';"
    ));
    let raw = pair.sql(&format!("SELECT snapshot FROM cards WHERE id = '{id}';"));
    assert!(raw.contains("norm_hash"), "the premise: {raw}");
    assert!(!raw.contains("within_hash"), "the premise: {raw}");
}

/// "Rules and edge cases": the note of an entry with no key.
fn froze_alone(id: &str, target: &str) -> String {
    format!(
        "{id}: {target} froze its own text alone: a change in its sections marks nothing until \
         an accept or a clear"
    )
}

/// The config lines that give `spec gate` a zone, and its scratch file,
/// committed.
fn gate_ready(pair: &Pair) {
    let mut text = read_text(&pair.main, "specengine.toml");
    text.push_str(
        "\n[zones]\ncode = [\"src/**\"]\n\n[gate]\nmode = \"selective\"\nzones = [\"code\"]\n",
    );
    write(&pair.main, "specengine.toml", text);
    write(&pair.main, "src/a.rs", "fn a() {}\n");
    pair.commit_all(&pair.main, "the gate's scratch files");
}

/// A gate's answer on the worktree: exit, stdout, stderr. `spec gate` answers
/// within 3 s of wall clock: a run a loaded machine starves past that says
/// `unavailable`, so it is asked again (five tries; the last answer stands,
/// and a compare shows it).
fn gate_answer(pair: &Pair) -> (i32, String, String) {
    let main = pair.main.to_str().expect("a UTF-8 path");
    let mut done = None;
    for _ in 0..5 {
        let answer = run(
            pair,
            &["gate", "--worktree", main, "--path", "src/a.rs", "--json"],
        );
        let starved = answer.stdout.contains("\"reason\":\"deadline\"")
            || answer.stderr.contains("no answer within");
        done = Some(answer);
        if !starved {
            break;
        }
    }
    let done = done.expect("a run");
    (done.code, done.stdout, done.stderr)
}

/// `(from, to)` each applied to `target`'s span, proposed in the main worktree
/// and never approved: the proposal's ID.
fn proposed(pair: &Pair, target: &str, edits: &[(&str, &str)]) -> String {
    let (span, text) = pair.span(&pair.main, target);
    let new = edits
        .iter()
        .fold(text, |text, (from, to)| text.replace(from, to));
    pair.propose(&pair.main, target, &span, &new)
        .unwrap_or_else(|error| panic!("propose {target}: {error}"))
        .document
        .id
        .clone()
        .expect("an ID")
}

/// The worktree committed by hand with `proposal`'s `Proposal:` trailer and
/// `revisions` as `Revision:` trailers, then `spec approve`: a completion by
/// that commit, which it finds and reads, no second commit made.
fn completed_by_hand(pair: &Pair, proposal: &str, revisions: &[&str]) {
    let mut message = format!("By hand\n\nProposal: {proposal}\n");
    for revision in revisions {
        message.push_str(&format!("Revision: {revision}\n"));
    }
    pair.commit_all(&pair.main, &message);
    let commit = pair.rev(&pair.main, "HEAD");
    pair.approve_ok(&pair.main, proposal);
    let stored = pair.proposal(proposal);
    assert_eq!(stored.status, ProposalStatus::Applied, "{stored:?}");
    assert_eq!(stored.applied_commit.as_deref(), Some(commit.as_str()));
    assert_eq!(pair.rev(&pair.main, "HEAD"), commit, "no new commit");
}

/// The work of a whole cycle on the almanac, trace-views' own: `CALL`
/// confirmed, then its brief changed: suspect; `SORT` confirmed; `READ`
/// departed; `SET` open; `BIND` no card.
fn a_cycle(pair: &Pair) {
    accepted(pair, "Notice posted", &[CALL]);
    accepted(pair, "Entries sorted", &[SORT]);
    departed(pair, "Entries read", READ);
    cut(pair, "Pages set", &[SET]);
    approved(pair, CALL, CALL_MEANING);
}

// ------------------------------------------------------------------- AC-01

/// AC-01: a card on `STEP-SET` accepted, text appended under `STEP-SET-PROOF`,
/// the document's own hash held: `spec trace STEP-SET` reads `suspect` and the
/// card `suspect STEP-SET`; `spec flow --trace` and the list agree. The part
/// above it is left. M: `within_hash` over the target alone.
#[test]
fn ac01_text_under_a_section_marks_the_documents_card() {
    let pair = Pair::new("tt-ac01", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    reads(&pair, &id, "accepted", None);
    assert_eq!(
        node_of(&doc(&pair, &["trace", SET]), SET)["rollup"],
        "confirmed"
    );
    let own = norm(&pair, SET);

    append_under_proof(&pair);
    assert_eq!(
        norm(&pair, SET),
        own,
        "the premise: `norm_hash` is own text"
    );

    let text = ok(&pair, &["trace", SET]);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines[0],
        "trace STEP-SET | nodes 1, cards 1 | no card 0, in work 0, suspect 1, departed 0, \
         confirmed 0",
        "{text}"
    );
    assert!(
        lines.contains(&"suspect | STEP-SET | Set the pages | docs/spec/step-set.md | cards 1"),
        "{text}"
    );
    assert!(
        lines.contains(&format!("  card {id} | almanac/done | suspect {SET} | Pages set").as_str()),
        "{text}"
    );
    let shown = doc(&pair, &["trace", SET]);
    let node = node_of(&shown, SET);
    assert_eq!(node["rollup"], "suspect", "{shown}");
    let card = card_of(node, &id);
    assert_eq!(card["mark"], "suspect", "{card}");
    assert_eq!(card["on"], SET, "{card}");

    let flow = doc(&pair, &["flow", "--trace"]);
    assert_eq!(node_of(&flow, SET)["rollup"], "suspect", "{flow}");
    assert_eq!(node_of(&flow, GATHER)["rollup"], "no card", "{flow}");
    let printed = ok(&pair, &["flow", "--trace"]);
    assert!(
        printed
            .lines()
            .any(|line| line.contains(&format!(" {SET} | "))
                && line.ends_with(" | rollup suspect")),
        "{printed}"
    );

    let row = row_in(&pair, &pair.main, &id);
    assert_eq!(row["mark"], "suspect", "{row}");
    assert_eq!(row["on"], SET, "{row}");
    assert_eq!(row["discrepancy"], Value::Null, "{row}");
    assert_eq!(row["correction"], Value::Null, "{row}");
}

/// AC-01 by the door a person uses: a meaning approve of the section marks the
/// document's card and vouches for nothing: no `card.refreshed`, the document's
/// own hash and revision as they were.
#[test]
fn ac01_a_meaning_approve_of_the_section_marks_and_refreshes_nothing() {
    let pair = Pair::new("tt-ac01-approve", ALMANAC);
    let id = cut(&pair, "Pages set", &[SET]);
    let (own, rev) = (norm(&pair, SET), entry(&pair, &id)["rev"].clone());
    approved(&pair, PROOF, PROOF_MEANING);
    reads(&pair, &id, "suspect", Some(SET));
    assert_eq!(events_of(&pair, "card.refreshed"), Vec::<Value>::new());
    assert_eq!(norm(&pair, SET), own);
    let again = doc(&pair, &["trace", SET]);
    assert_eq!(node_of(&again, SET)["rollup"], "suspect", "{again}");
    assert_eq!(entry(&pair, &id)["rev"], rev, "the entry holds");
}

/// AC-01, ADR-0012: the file bytes the same, the mark there or cleared, `spec
/// check` and `spec gate` answer the same bytes: neither reads a mark.
#[test]
fn ac01_check_and_gate_never_read_the_mark() {
    let pair = Pair::new("tt-ac01-gate", ALMANAC);
    gate_ready(&pair);
    let id = accepted(&pair, "Pages set", &[SET]);
    append_under_proof(&pair);
    reads(&pair, &id, "suspect", Some(SET));
    let check = run(&pair, &["--json", "check"]);
    let gate = gate_answer(&pair);

    let done = clear(&pair, &id, "Only the wording.");
    assert_eq!(done.code, 0, "{}", done.output);
    reads(&pair, &id, "accepted", None);
    let after = run(&pair, &["--json", "check"]);
    assert_eq!(
        (after.code, &after.stdout, &after.stderr),
        (check.code, &check.stdout, &check.stderr),
        "`spec check` unchanged"
    );
    assert_eq!(gate_answer(&pair), gate, "`spec gate` unchanged");
}

// ------------------------------------------------------------------- AC-02

/// AC-02: the same edit leaves clean a card on `PART-GATHER`, on a sibling
/// section and on a node that `uses` `STEP-SET`, the document's card marked
/// beside them, so the edit is a real one. A change in the held document's own
/// text leaves the part clean too. M: walking `held_documents` into the hash.
#[test]
fn ac02_a_mark_stops_at_the_file() {
    let pair = Pair::new("tt-ac02", ALMANAC);
    replace(
        &pair.main,
        CALL_MD,
        "stage: gather\n",
        "stage: gather\nlinks:\n  uses: [STEP-SET]\n",
    );
    add_margin(&pair);
    let on_set = accepted(&pair, "Pages set", &[SET]);
    let on_part = accepted(&pair, "The committee met", &[GATHER]);
    let on_sibling = accepted(&pair, "Margins ruled", &[MARGIN]);
    let on_user = accepted(&pair, "Notice posted", &[CALL]);

    append_under_proof(&pair);
    reads(&pair, &on_set, "suspect", Some(SET));
    for id in [&on_part, &on_sibling, &on_user] {
        reads(&pair, id, "accepted", None);
    }
    let shown = doc(&pair, &["trace"]);
    for (id, word) in [
        (GATHER, "confirmed"),
        (MARGIN, "confirmed"),
        (CALL, "confirmed"),
        (SET, "suspect"),
    ] {
        assert_eq!(node_of(&shown, id)["rollup"], word, "{id}: {shown}");
    }
    assert_eq!(
        node_of(&shown, SET)["uses"],
        json!([{"id": CALL, "rollup": "confirmed", "cards": 1}]),
        "a consequence, never a mark"
    );

    replace(&pair.main, SET_MD, SET_MEANING.0, SET_MEANING.1);
    pair.commit_all(&pair.main, "the document's own text");
    for id in [&on_part, &on_sibling, &on_user] {
        reads(&pair, id, "accepted", None);
    }
}

/// AC-02, "the ID sections nested in it": sections nest by span. A change in
/// `STEP-SET-PROOF`'s own text marks it and its document, never the section
/// nested in it or the sibling; a change in the nested one marks all that hold
/// it.
#[test]
fn ac02_a_nested_section_marks_what_holds_it_and_stops_there() {
    let pair = Pair::new("tt-ac02-nested", ALMANAC);
    replace(
        &pair.main,
        SET_MD,
        PROOF_LINE,
        &format!("{PROOF_LINE}\n### The ink {{#{INK}}}\n\nBlack only.\n"),
    );
    add_margin(&pair);
    let on_set = accepted(&pair, "Pages set", &[SET]);
    let on_proof = accepted(&pair, "Proof pulled", &[PROOF]);
    let on_ink = accepted(&pair, "Ink mixed", &[INK]);
    let on_margin = accepted(&pair, "Margins ruled", &[MARGIN]);
    for id in [&on_set, &on_proof, &on_ink, &on_margin] {
        reads(&pair, id, "accepted", None);
    }

    replace(&pair.main, SET_MD, PROOF_MEANING.0, PROOF_MEANING.1);
    reads(&pair, &on_set, "suspect", Some(SET));
    reads(&pair, &on_proof, "suspect", Some(PROOF));
    reads(&pair, &on_ink, "accepted", None);
    reads(&pair, &on_margin, "accepted", None);

    replace(&pair.main, SET_MD, "Black only.\n", "Blue only.\n");
    reads(&pair, &on_ink, "suspect", Some(INK));
    reads(&pair, &on_margin, "accepted", None);
}

// ------------------------------------------------------------------- AC-03

/// AC-03: whitespace under the section moves its `span_hash` and no
/// `norm_hash`: no mark, on the document's card or the section's.
#[test]
fn ac03_whitespace_under_the_section_marks_nothing() {
    let pair = Pair::new("tt-ac03-space", ALMANAC);
    let on_set = accepted(&pair, "Pages set", &[SET]);
    let on_proof = accepted(&pair, "Proof pulled", &[PROOF]);
    let (hash, span) = (norm(&pair, PROOF), pair.span(&pair.main, PROOF).0);
    // Inside the span: a trailing blank at the file's end is out of it, and
    // the premise below would not hold.
    replace(
        &pair.main,
        SET_MD,
        "{#STEP-SET-PROOF}\n\n",
        "{#STEP-SET-PROOF}\n\n\n",
    );
    assert_eq!(norm(&pair, PROOF), hash, "the premise");
    assert_ne!(pair.span(&pair.main, PROOF).0, span, "the premise");
    reads(&pair, &on_set, "accepted", None);
    reads(&pair, &on_proof, "accepted", None);
}

/// AC-03, D2: a section moved within its file changes no pair: the document's
/// card and each section's stay clean. M: pairs in file order.
#[test]
fn ac03_a_section_moved_in_its_file_marks_nothing() {
    let pair = Pair::new("tt-ac03-moved", ALMANAC);
    add_margin(&pair);
    let on_set = accepted(&pair, "Pages set", &[SET]);
    let on_proof = accepted(&pair, "Proof pulled", &[PROOF]);
    let on_margin = accepted(&pair, "Margins ruled", &[MARGIN]);
    let hashes = (norm(&pair, SET), norm(&pair, PROOF), norm(&pair, MARGIN));
    replace(
        &pair.main,
        SET_MD,
        &format!("{PROOF_SECTION}\n{MARGIN_SECTION}"),
        &format!("{MARGIN_SECTION}\n{PROOF_SECTION}"),
    );
    pair.commit_all(&pair.main, "the margin first");
    assert_eq!(
        (norm(&pair, SET), norm(&pair, PROOF), norm(&pair, MARGIN)),
        hashes,
        "the premise: a move is no change of any node's text"
    );
    for id in [&on_set, &on_proof, &on_margin] {
        reads(&pair, id, "accepted", None);
    }
}

/// AC-03: a new `{#ID}` section marks the document's card, its own hash held:
/// the pairs changed. The section beside it stays clean.
#[test]
fn ac03_a_new_section_marks() {
    let pair = Pair::new("tt-ac03-new", ALMANAC);
    let on_set = accepted(&pair, "Pages set", &[SET]);
    let on_proof = accepted(&pair, "Proof pulled", &[PROOF]);
    let own = norm(&pair, SET);
    add_margin(&pair);
    assert_eq!(norm(&pair, SET), own, "the premise");
    reads(&pair, &on_set, "suspect", Some(SET));
    reads(&pair, &on_proof, "accepted", None);
}

/// "Recipe", the reading a section's going takes: the pairs changed, so a
/// section taken out marks the document's card; a renamed one too, its text
/// the same. M: pairs without names.
#[test]
fn ac03_a_section_gone_or_renamed_marks() {
    let pair = Pair::new("tt-ac03-gone", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    let text = read_text(&pair.main, SET_MD);
    write(
        &pair.main,
        SET_MD,
        text.replace(&format!("\n{PROOF_SECTION}"), ""),
    );
    reads(&pair, &id, "suspect", Some(SET));
    write(&pair.main, SET_MD, &text);
    reads(&pair, &id, "accepted", None);
    replace(
        &pair.main,
        SET_MD,
        "{#STEP-SET-PROOF}",
        "{#STEP-SET-PROOFS}",
    );
    reads(&pair, &id, "suspect", Some(SET));
}

/// "Data", "Recipe": an entry's keys in order; a sectioned document's
/// `within_hash` is `b3_hash` of `<name>\0<norm_hash>\n` per pair by name, not
/// by place in the file (`STEP-SET-MARGIN` stands after `STEP-SET-PROOF`); a
/// section or a plain document, one member, holds its `norm_hash`, the same
/// string. M: pairs in file order; pairs without names.
#[test]
fn ac03_the_recipe_is_the_specs() {
    let pair = Pair::new("tt-recipe", ALMANAC);
    add_margin(&pair);
    let on_set = cut(&pair, "Pages set", &[SET]);
    let on_proof = cut(&pair, "Proof pulled", &[PROOF]);
    let on_call = cut(&pair, "Notice posted", &[CALL]);

    let (own, proof, margin) = (norm(&pair, SET), norm(&pair, PROOF), norm(&pair, MARGIN));
    let input = format!("{SET}\0{own}\n{MARGIN}\0{margin}\n{PROOF}\0{proof}\n");
    let frozen = entry(&pair, &on_set);
    assert_eq!(frozen["within_hash"], b3_hash(input.as_bytes()), "{frozen}");
    assert_eq!(frozen["norm_hash"], own, "as today: {frozen}");
    assert_eq!(frozen["also"], json!([]), "{frozen}");
    assert_eq!(entry(&pair, &on_proof)["within_hash"], proof);
    assert_eq!(entry(&pair, &on_call)["within_hash"], norm(&pair, CALL));

    let done = run(&pair, &["--json", "card", "show", &on_set]);
    done.code(0);
    let ordered = Ordered::of(&done.stdout);
    let Ordered::Array(entries) = ordered.get("snapshot") else {
        panic!("a snapshot: {}", done.show());
    };
    assert_eq!(
        entries[0].keys(),
        ["id", "rev", "norm_hash", "within_hash", "also"]
    );
}

/// "Recipe": the name is `SpecGraph::name`'s: an ID-less document's pair
/// carries its path, which sorts after the section's ID in bytes.
#[test]
fn ac03_an_id_less_document_names_its_path() {
    let pair = Pair::new("tt-recipe-path", ALMANAC);
    add_loose(&pair);
    let id = cut(&pair, "Notes kept", &[LOOSE_MD]);
    let frozen = entry(&pair, &id);
    assert_eq!(frozen["id"], LOOSE_MD, "{frozen}");
    let (own, rule) = (norm(&pair, LOOSE_MD), norm(&pair, RULE));
    let input = format!("{RULE}\0{rule}\n{LOOSE_MD}\0{own}\n");
    assert_eq!(frozen["within_hash"], b3_hash(input.as_bytes()), "{frozen}");
    reads(&pair, &id, "open", None);
    replace(
        &pair.main,
        LOOSE_MD,
        "A note stays up for a week.",
        "A note stays up for a month.",
    );
    reads(&pair, &id, "suspect", Some(LOOSE_MD));
}

/// "Recipe", "Rules and edge cases": a member's null hash gives a null
/// `within_hash`: a broken document's own, its section keeping its hash. A
/// null now reads not suspect, with the note `mark_notes` gives; a card cut
/// meanwhile freezes `null` on both keys.
#[test]
fn ac03_a_null_member_is_null_and_marks_nothing() {
    let pair = Pair::new("tt-null", ALMANAC);
    let id = cut(&pair, "Pages set", &[SET]);
    let text = read_text(&pair.main, SET_MD);
    write(
        &pair.main,
        SET_MD,
        text.replace("stage: print\n", "stage: [print]\n"),
    );
    assert_eq!(pair.node(&pair.main, SET).norm_hash, None, "the premise");
    assert!(
        pair.node(&pair.main, PROOF).norm_hash.is_some(),
        "the premise: a section keeps its hash"
    );
    reads(&pair, &id, "open", None);
    let notes = doc(&pair, &["trace"])["notes"].to_string();
    assert!(
        notes.contains(&format!(
            "{id}: `{SET}` has a null hash: it cannot be compared, so it reads not suspect"
        )),
        "{notes}"
    );

    let broken = cut(&pair, "Cut meanwhile", &[SET]);
    let frozen = entry(&pair, &broken);
    assert_eq!(frozen["norm_hash"], Value::Null, "{frozen}");
    assert_eq!(frozen["within_hash"], Value::Null, "{frozen}");
    write(&pair.main, SET_MD, &text);
    reads(&pair, &id, "open", None);
}

// ------------------------------------------------------------------- AC-04

/// AC-04: a stored entry with no `within_hash` key on a sectioned document
/// reads `accepted` with exactly one note, in every read of marks; a change in
/// the section marks nothing: no false mark; a meaning change of its own text
/// marks it, as today. M: an absent key read as a change.
#[test]
fn ac04_an_entry_with_no_key_compares_its_own_hash() {
    let pair = Pair::new("tt-ac04", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    strip_key(&pair, &id);
    assert_eq!(
        entry(&pair, &id)["within_hash"],
        Value::Null,
        "reads `None`"
    );
    let note = froze_alone(&id, SET);

    let shown = doc(&pair, &["trace"]);
    assert_eq!(card_of(node_of(&shown, SET), &id)["mark"], "accepted");
    assert_eq!(node_of(&shown, SET)["rollup"], "confirmed", "{shown}");
    assert_eq!(shown["notes"], json!([note]), "exactly one");
    let list = doc(&pair, &["card", "list", "--marks"]);
    assert_eq!(list["notes"], json!([note]), "exactly one");

    append_under_proof(&pair);
    reads(&pair, &id, "accepted", None);

    approved(&pair, SET, SET_MEANING);
    reads(&pair, &id, "suspect", Some(SET));
}

/// AC-04, "a note only where the target's `within_hash` now differs from its
/// `norm_hash`": an entry with no key on a plain document or on a section
/// reads as it did, no note.
#[test]
fn ac04_no_note_where_the_target_holds_no_section() {
    let pair = Pair::new("tt-ac04-plain", ALMANAC);
    let on_call = accepted(&pair, "Notice posted", &[CALL]);
    let on_proof = accepted(&pair, "Proof pulled", &[PROOF]);
    strip_key(&pair, &on_call);
    strip_key(&pair, &on_proof);
    reads(&pair, &on_call, "accepted", None);
    reads(&pair, &on_proof, "accepted", None);
    let shown = doc(&pair, &["trace"]);
    assert_eq!(shown["notes"], json!([]), "{shown}");
    approved(&pair, PROOF, PROOF_MEANING);
    reads(&pair, &on_proof, "suspect", Some(PROOF));
}

/// AC-04: the clear of that mark writes the key: `from` the entry's `norm_hash`
/// ("else `norm_hash` on an entry with none"), `to` the `within_hash` frozen;
/// the note is gone and a change in the section now marks.
#[test]
fn ac04_the_clear_of_that_mark_writes_the_key() {
    let pair = Pair::new("tt-ac04-clear", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    let old = norm(&pair, SET);
    strip_key(&pair, &id);
    approved(&pair, SET, SET_MEANING);
    reads(&pair, &id, "suspect", Some(SET));

    let done = clear(&pair, &id, "Only the wording.");
    assert_eq!(done.code, 0, "{}", done.output);
    let to = pair.within(&[SET, PROOF]);
    let frozen = entry(&pair, &id);
    assert_eq!(frozen["within_hash"], to, "{frozen}");
    assert_eq!(frozen["norm_hash"], norm(&pair, SET), "{frozen}");
    assert_eq!(
        events_of(&pair, "card.cleared")[0]["nodes"],
        json!([{"id": SET, "rev": 2, "from": old, "to": to}])
    );
    reads(&pair, &id, "accepted", None);
    assert_eq!(doc(&pair, &["trace"])["notes"], json!([]), "the note goes");

    append_under_proof(&pair);
    reads(&pair, &id, "suspect", Some(SET));
}

/// AC-04: an accept of an open card in the same state writes the key: it
/// compares from then on.
#[test]
fn ac04_an_accept_of_an_open_card_writes_the_key() {
    let pair = Pair::new("tt-ac04-accept", ALMANAC);
    let id = cut(&pair, "Pages set", &[SET]);
    to_review(&pair, &id);
    strip_key(&pair, &id);
    reads(&pair, &id, "open", None);
    accept_briefed(&pair, &id);
    assert_eq!(entry(&pair, &id)["within_hash"], pair.within(&[SET, PROOF]));
    assert_eq!(doc(&pair, &["trace"])["notes"], json!([]), "the note goes");
    append_under_proof(&pair);
    reads(&pair, &id, "suspect", Some(SET));
}

// ------------------------------------------------------------------- AC-05

/// AC-05: `spec approve --editorial` of the section: the document's card stays
/// clean here and in a worktree without the commit (the entry's hash kept, the
/// new one in `also`), one `card.refreshed` names the document, `revision`
/// rises by one, no revision of the document moves. The next meaning change
/// marks. M: `card_refresh` without holders.
#[test]
fn ac05_an_editorial_pass_of_the_section_vouches_for_the_document() {
    let pair = Pair::new("tt-ac05", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    let frozen = entry(&pair, &id);
    let before = revision(&pair, &id);

    let proposal = editorial(&pair, PROOF, PROOF_MEANING);
    let now = pair.within(&[SET, PROOF]);
    assert_ne!(json!(now), frozen["within_hash"], "the premise");
    assert!(
        read_text(&pair.linked, SET_MD).contains(PROOF_MEANING.0),
        "the premise: the worktree lacks the commit"
    );
    assert_eq!(
        events_of(&pair, "card.refreshed"),
        [json!({"id": id, "node": SET, "proposal": proposal})]
    );
    assert_eq!(revision(&pair, &id), before + 1);
    let held = entry(&pair, &id);
    assert_eq!(held["norm_hash"], frozen["norm_hash"], "{held}");
    assert_eq!(held["within_hash"], frozen["within_hash"], "{held}");
    assert_eq!(held["also"], json!([now]), "{held}");
    for cwd in [&pair.main, &pair.linked] {
        reads_in(&pair, cwd, &id, "accepted", None);
        let shown = doc_in(&pair, cwd, &["trace", SET]);
        assert_eq!(node_of(&shown, SET)["rollup"], "confirmed", "{cwd:?}");
    }

    approved(&pair, PROOF, PROOF_AGAIN);
    reads(&pair, &id, "suspect", Some(SET));
}

/// AC-05: an editorial edit of the document's own text moves two hashes, the
/// own and `within_hash`: one `card.refreshed`, `revision` one more, the card
/// clean here and in the worktree. An entry's `also` holds only the kind it
/// compares, so it holds the new `within_hash` alone, never the own hash. M:
/// `nodes` not deduplicated.
#[test]
fn ac05_an_editorial_pass_of_the_documents_text_gives_one_event() {
    let pair = Pair::new("tt-ac05-own", ALMANAC);
    let id = cut(&pair, "Pages set", &[SET]);
    let before = revision(&pair, &id);
    let proposal = editorial(&pair, SET, SET_MEANING);
    assert_eq!(
        events_of(&pair, "card.refreshed"),
        [json!({"id": id, "node": SET, "proposal": proposal})]
    );
    assert_eq!(revision(&pair, &id), before + 1);
    let held = entry(&pair, &id);
    assert_eq!(held["also"], json!([pair.within(&[SET, PROOF])]), "{held}");
    assert_ne!(held["also"][0], norm(&pair, SET), "the kind it compares");
    for cwd in [&pair.main, &pair.linked] {
        reads_in(&pair, cwd, &id, "open", None);
    }
}

/// A legacy entry compares `norm_hash`, so its `also` receives own hashes: an
/// editorial pass of the document's own text keeps it clean, the new own hash
/// its one entry, one `card.refreshed`.
#[test]
fn ac05_a_legacy_entrys_also_receives_own_hashes() {
    let pair = Pair::new("tt-ac05-legacy", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    strip_key(&pair, &id);
    let proposal = editorial(&pair, SET, SET_MEANING);
    assert_eq!(
        events_of(&pair, "card.refreshed"),
        [json!({"id": id, "node": SET, "proposal": proposal})]
    );
    let held = entry(&pair, &id);
    assert_eq!(held["also"], json!([norm(&pair, SET)]), "{held}");
    assert_eq!(held["within_hash"], Value::Null, "no key written: {held}");
    reads(&pair, &id, "accepted", None);
}

/// The deletion sequence: one member's `within_hash` is its `norm_hash`, the
/// same string, so a card accepted on a sectioned document, an editorial pass
/// of its own text, then every section deleted by hand would meet the pass's
/// own hash in `also` and read clean if `also` took both kinds: a false clean.
/// It reads suspect.
#[test]
fn ac05_every_section_deleted_after_a_pass_of_the_documents_text_marks() {
    let pair = Pair::new("tt-ac05-deleted", ALMANAC);
    add_margin(&pair);
    let id = accepted(&pair, "Pages set", &[SET]);
    editorial(&pair, SET, SET_MEANING);
    reads(&pair, &id, "accepted", None);
    let own = norm(&pair, SET);

    let text = read_text(&pair.main, SET_MD);
    write(
        &pair.main,
        SET_MD,
        text.replace(&format!("\n{PROOF_SECTION}"), "")
            .replace(&format!("\n{MARGIN_SECTION}"), ""),
    );
    pair.commit_all(&pair.main, "every section gone, by hand");
    assert_eq!(norm(&pair, SET), own, "the premise: the own text held");
    assert_eq!(
        pair.within(&[SET]),
        own,
        "the premise: one member, the same string"
    );
    reads(&pair, &id, "suspect", Some(SET));
    assert_eq!(
        node_of(&doc(&pair, &["trace", SET]), SET)["rollup"],
        "suspect"
    );
}

/// "Editorial refresh": one `card.refreshed` per (card, node): a card on the
/// document and on its section takes two, its `revision` rising once; a card
/// on the section alone one; a card on a sibling none.
#[test]
fn ac05_a_pass_logs_one_event_per_card_and_node() {
    let pair = Pair::new("tt-ac05-pairs", ALMANAC);
    add_margin(&pair);
    let both = cut(&pair, "Both", &[SET, PROOF]);
    let section = cut(&pair, "Section", &[PROOF]);
    let sibling = cut(&pair, "Sibling", &[MARGIN]);
    let (rev_both, rev_section, rev_sibling) = (
        revision(&pair, &both),
        revision(&pair, &section),
        revision(&pair, &sibling),
    );
    let proposal = editorial(&pair, PROOF, PROOF_MEANING);

    let logged: BTreeSet<(String, String)> = events_of(&pair, "card.refreshed")
        .iter()
        .map(|event| {
            assert_eq!(event["proposal"], proposal.as_str(), "{event}");
            (
                event["id"].as_str().expect("a card").to_owned(),
                event["node"].as_str().expect("a node").to_owned(),
            )
        })
        .collect();
    assert_eq!(
        logged,
        BTreeSet::from([
            (both.clone(), SET.to_owned()),
            (both.clone(), PROOF.to_owned()),
            (section.clone(), PROOF.to_owned()),
        ])
    );
    assert_eq!(events_of(&pair, "card.refreshed").len(), 3, "each once");
    assert_eq!(revision(&pair, &both), rev_both + 1);
    assert_eq!(revision(&pair, &section), rev_section + 1);
    assert_eq!(revision(&pair, &sibling), rev_sibling);
    for id in [&both, &section, &sibling] {
        reads(&pair, id, "open", None);
    }
}

/// "An editorial pass of a section vouches for the document and sections
/// around it": a nested section's pass keeps the section around it and the
/// document clean, one event each.
#[test]
fn ac05_a_pass_of_a_nested_section_vouches_for_all_around_it() {
    let pair = Pair::new("tt-ac05-nested", ALMANAC);
    replace(
        &pair.main,
        SET_MD,
        PROOF_LINE,
        &format!("{PROOF_LINE}\n### The ink {{#{INK}}}\n\nBlack only.\n"),
    );
    pair.commit_all(&pair.main, "a nested section");
    let on_set = cut(&pair, "Pages set", &[SET]);
    let on_proof = cut(&pair, "Proof pulled", &[PROOF]);
    editorial(&pair, INK, ("Black only", "Blue only"));
    let nodes: BTreeSet<(String, String)> = events_of(&pair, "card.refreshed")
        .iter()
        .map(|event| {
            (
                event["id"].as_str().expect("a card").to_owned(),
                event["node"].as_str().expect("a node").to_owned(),
            )
        })
        .collect();
    assert_eq!(
        nodes,
        BTreeSet::from([
            (on_set.clone(), SET.to_owned()),
            (on_proof.clone(), PROOF.to_owned()),
        ])
    );
    reads(&pair, &on_set, "open", None);
    reads(&pair, &on_proof, "open", None);
}

/// "`spec approve --editorial`", the owner's editorial word whichever way it
/// came: staged, then a bare `spec approve`, vouches for the document too.
#[test]
fn ac05_a_staged_editorial_of_the_section_vouches_for_the_document() {
    let pair = Pair::new("tt-ac05-staged", ALMANAC);
    let id = cut(&pair, "Pages set", &[SET]);
    let proposal = pair.propose_edit(&pair.main, PROOF, PROOF_MEANING.0, PROOF_MEANING.1);
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
        },
    )
    .expect("the stage");
    pair.approve_ok(&pair.main, &proposal);
    assert_eq!(
        events_of(&pair, "card.refreshed"),
        [json!({"id": id, "node": SET, "proposal": proposal})]
    );
    reads(&pair, &id, "open", None);
}

/// D5: an editorial apply interrupted after its commit, the row, the cards and
/// the event set back: the completion vouches for what its trailers name, the
/// section's card; the document's card stays marked for a person, who clears
/// it. A false mark, never a false clean.
#[test]
fn ac05_a_completion_vouches_only_what_its_trailers_name() {
    let pair = Pair::new("tt-ac05-complete", ALMANAC);
    let on_set = cut(&pair, "Pages set", &[SET]);
    let on_proof = cut(&pair, "Proof pulled", &[PROOF]);
    let raw = |id: &str| {
        pair.sql(&format!("SELECT snapshot FROM cards WHERE id = '{id}';"))
            .trim_end_matches('\n')
            .to_owned()
    };
    let (raw_set, raw_proof) = (raw(&on_set), raw(&on_proof));
    let proposal = editorial(&pair, PROOF, PROOF_MEANING);
    let commit = pair.rev(&pair.main, "HEAD");
    assert_eq!(events_of(&pair, "card.refreshed").len(), 2, "the apply's");

    pair.sql(&format!(
        "UPDATE proposals SET status = 'approved', applied_commit = NULL WHERE id = \
         '{proposal}'; UPDATE cards SET snapshot = '{raw_set}' WHERE id = '{on_set}'; UPDATE \
         cards SET snapshot = '{raw_proof}' WHERE id = '{on_proof}'; DELETE FROM events WHERE \
         type = 'card.refreshed';"
    ));
    reads(&pair, &on_set, "suspect", Some(SET));
    reads(&pair, &on_proof, "suspect", Some(PROOF));

    pair.approve_ok(&pair.main, &proposal);
    assert_eq!(pair.rev(&pair.main, "HEAD"), commit, "no new commit");
    assert_eq!(
        events_of(&pair, "card.refreshed"),
        [json!({"id": on_proof, "node": PROOF, "proposal": proposal})]
    );
    reads(&pair, &on_proof, "open", None);
    reads(&pair, &on_set, "suspect", Some(SET));
    let done = clear(&pair, &on_set, "Only the wording.");
    assert_eq!(done.code, 0, "{}", done.output);
    reads(&pair, &on_set, "open", None);
}

/// A completion's rule, what a trailer may vouch for: a commit changing the
/// document's own text alone, the document named `(editorial)`: the members
/// left are the same, so `within_hash` goes with it: clean, one
/// `card.refreshed`, the kind the entry compares in `also`.
#[test]
fn ac05_a_completion_of_own_text_alone_stays_clean() {
    let pair = Pair::new("tt-done-own", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    let proposal = proposed(&pair, SET, &[SET_MEANING]);
    replace(&pair.main, SET_MD, SET_MEANING.0, SET_MEANING.1);
    completed_by_hand(&pair, &proposal, &["STEP-SET 1 (editorial)"]);
    assert_eq!(
        events_of(&pair, "card.refreshed"),
        [json!({"id": id, "node": SET, "proposal": proposal})]
    );
    assert_eq!(
        entry(&pair, &id)["also"],
        json!([pair.within(&[SET, PROOF])])
    );
    reads(&pair, &id, "accepted", None);
}

/// A completion's rule: both texts changed, the document `(editorial)`, the
/// section raised: a meaning change sits inside the document's `within_hash`,
/// so no trailer vouches for it: suspect, no event. The own hash goes and an
/// entry with a `within_hash` refuses it. M: `within_hash` vouched without the
/// compare.
#[test]
fn ac05_a_completion_over_a_raised_section_keeps_the_mark() {
    let pair = Pair::new("tt-done-raised", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    let proposal = proposed(&pair, SET, &[SET_MEANING, PROOF_MEANING]);
    replace(&pair.main, SET_MD, SET_MEANING.0, SET_MEANING.1);
    replace(&pair.main, SET_MD, PROOF_MEANING.0, PROOF_MEANING.1);
    completed_by_hand(
        &pair,
        &proposal,
        &["STEP-SET 1 (editorial)", "STEP-SET-PROOF 1 -> 2"],
    );
    assert_eq!(events_of(&pair, "card.refreshed"), Vec::<Value>::new());
    reads(&pair, &id, "suspect", Some(SET));
}

/// A completion's rule: both texts changed, no trailer on the section: nobody
/// vouched for it: suspect, no event. M: `within_hash` vouched without the
/// compare.
#[test]
fn ac05_a_completion_over_an_unnamed_section_keeps_the_mark() {
    let pair = Pair::new("tt-done-unnamed", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    let proposal = proposed(&pair, SET, &[SET_MEANING, PROOF_MEANING]);
    replace(&pair.main, SET_MD, SET_MEANING.0, SET_MEANING.1);
    replace(&pair.main, SET_MD, PROOF_MEANING.0, PROOF_MEANING.1);
    completed_by_hand(&pair, &proposal, &["STEP-SET 1 (editorial)"]);
    assert_eq!(events_of(&pair, "card.refreshed"), Vec::<Value>::new());
    reads(&pair, &id, "suspect", Some(SET));
}

/// A completion's rule: both texts changed, both `(editorial)`: no member is
/// left to differ: clean, one `card.refreshed` for the document's card.
#[test]
fn ac05_a_completion_naming_both_stays_clean() {
    let pair = Pair::new("tt-done-both", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    let proposal = proposed(&pair, SET, &[SET_MEANING, PROOF_MEANING]);
    replace(&pair.main, SET_MD, SET_MEANING.0, SET_MEANING.1);
    replace(&pair.main, SET_MD, PROOF_MEANING.0, PROOF_MEANING.1);
    completed_by_hand(
        &pair,
        &proposal,
        &["STEP-SET 1 (editorial)", "STEP-SET-PROOF 1 (editorial)"],
    );
    assert_eq!(
        events_of(&pair, "card.refreshed"),
        [json!({"id": id, "node": SET, "proposal": proposal})]
    );
    assert_eq!(
        entry(&pair, &id)["also"],
        json!([pair.within(&[SET, PROOF])])
    );
    reads(&pair, &id, "accepted", None);
}

/// A completion's rule: a section coming in the commit is a member left that
/// differs, every other change named: suspect, no event. The proposal is the
/// section's, so the commit may carry more than its span. M: `within_hash`
/// vouched without the compare.
#[test]
fn ac05_a_completion_with_a_section_coming_keeps_the_mark() {
    let pair = Pair::new("tt-done-coming", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    let proposal = proposed(&pair, PROOF, &[PROOF_MEANING]);
    replace(&pair.main, SET_MD, SET_MEANING.0, SET_MEANING.1);
    replace(&pair.main, SET_MD, PROOF_MEANING.0, PROOF_MEANING.1);
    let text = read_text(&pair.main, SET_MD);
    write(&pair.main, SET_MD, format!("{text}\n{MARGIN_SECTION}"));
    completed_by_hand(
        &pair,
        &proposal,
        &["STEP-SET 1 (editorial)", "STEP-SET-PROOF 1 (editorial)"],
    );
    assert_eq!(events_of(&pair, "card.refreshed"), Vec::<Value>::new());
    reads(&pair, &id, "suspect", Some(SET));
}

/// A completion's rule: a section going in the commit: the same, whether a
/// trailer names it or not: a node in one blob alone is no place to take out.
#[test]
fn ac05_a_completion_with_a_section_going_keeps_the_mark() {
    for (label, last) in [
        ("tt-done-going", None),
        ("tt-done-going-named", Some("STEP-SET-MARGIN 1 (editorial)")),
    ] {
        let pair = Pair::new(label, ALMANAC);
        add_margin(&pair);
        let id = accepted(&pair, "Pages set", &[SET]);
        let proposal = proposed(&pair, PROOF, &[PROOF_MEANING]);
        replace(&pair.main, SET_MD, SET_MEANING.0, SET_MEANING.1);
        replace(&pair.main, SET_MD, PROOF_MEANING.0, PROOF_MEANING.1);
        replace(&pair.main, SET_MD, &format!("\n{MARGIN_SECTION}"), "");
        let mut revisions = vec!["STEP-SET 1 (editorial)", "STEP-SET-PROOF 1 (editorial)"];
        revisions.extend(last);
        completed_by_hand(&pair, &proposal, &revisions);
        assert_eq!(
            events_of(&pair, "card.refreshed"),
            Vec::<Value>::new(),
            "{label}"
        );
        reads(&pair, &id, "suspect", Some(SET));
    }
}

/// A completion's rule: the members left compare as sorted pairs, so sections
/// moved in the commit are no difference. An update keeps a file's structure,
/// so the proposal is a third section's and the move is the hand commit's: the
/// document and that section named, clean, one event.
#[test]
fn ac05_a_completion_with_sections_moved_stays_clean() {
    let pair = Pair::new("tt-done-moved", ALMANAC);
    let text = read_text(&pair.main, SET_MD);
    write(
        &pair.main,
        SET_MD,
        format!("{text}\n{MARGIN_SECTION}\n## The seal {{#STEP-SET-SEAL}}\n\nRed wax.\n"),
    );
    pair.commit_all(&pair.main, "two sections more");
    let id = accepted(&pair, "Pages set", &[SET]);
    let proposal = proposed(&pair, "STEP-SET-SEAL", &[("Red wax", "Blue wax")]);
    replace(&pair.main, SET_MD, SET_MEANING.0, SET_MEANING.1);
    replace(&pair.main, SET_MD, "Red wax", "Blue wax");
    replace(
        &pair.main,
        SET_MD,
        &format!("{PROOF_SECTION}\n{MARGIN_SECTION}"),
        &format!("{MARGIN_SECTION}\n{PROOF_SECTION}"),
    );
    completed_by_hand(
        &pair,
        &proposal,
        &["STEP-SET 1 (editorial)", "STEP-SET-SEAL 1 (editorial)"],
    );
    assert_eq!(
        events_of(&pair, "card.refreshed"),
        [json!({"id": id, "node": SET, "proposal": proposal})]
    );
    reads(&pair, &id, "accepted", None);
}

/// `text` as `target`'s new span, new `{#ID}` sections in it, a create proposed
/// in the main worktree and never approved: the proposal's ID.
fn created(pair: &Pair, target: &str, text: &str) -> String {
    let (span, _) = pair.span(&pair.main, target);
    specengine_cli::propose_create(
        &env(pair),
        &Globals::default(),
        &CreateRequest {
            target: target.to_owned(),
            base: Some(span),
            text: ProposedText::Given(text.as_bytes().to_vec()),
            rationale: "A section.".to_owned(),
            author_role: Some("spec-writer".to_owned()),
            author_model: Some("claude-opus-5-5".to_owned()),
            run: None,
            now: common::proposal::NOW.to_owned(),
            git: pair.git_env(&pair.main),
        },
    )
    .expect("a create")
    .document
    .id
    .clone()
    .expect("an ID")
}

/// A card frozen on a plain document, its own and its `within_hash` one string:
/// an editorial pass vouched, then an editorial create of a section: new
/// meaning for a card that has not read it, so it vouches nothing and the card
/// reads `suspect`. The card, and the section as the file holds it.
fn grown_a_section(pair: &Pair) -> (String, String) {
    let id = cut(pair, "Notice posted", &[CALL]);
    editorial(pair, CALL, CALL_MEANING);
    reads(pair, &id, "open", None);
    let line = "A notice goes up at the market cross on the last of the month.\n";
    let section = "## The board {#STEP-CALL-BOARD}\n\nIt stands by the cross.\n";
    let (_, text) = pair.span(&pair.main, CALL);
    let grown = created(
        pair,
        CALL,
        &text.replace(line, &format!("{line}\n{section}")),
    );
    approve_editorially(pair, &grown);
    reads(pair, &id, "suspect", Some(CALL));
    (id, section.to_owned())
}

/// The deletion sequence where the card was frozen on a plain document: the
/// plain pass's hash stands in `also`, one string with the own hash it was, so
/// only the key keeps a later pass's own pair out. The pass changes nothing on
/// the card, and with the section deleted by hand it still reads suspect. M:
/// `vouched` ignoring the kind.
#[test]
fn ac05_a_section_deleted_on_a_card_frozen_plain_marks() {
    let pair = Pair::new("tt-ac05-plain-deleted", ALMANAC);
    let (id, section) = grown_a_section(&pair);
    let also = entry(&pair, &id)["also"].clone();
    editorial(&pair, CALL, TENTH);
    reads(&pair, &id, "suspect", Some(CALL));
    assert_eq!(entry(&pair, &id)["also"], also, "no own hash joins it");
    let text = read_text(&pair.main, CALL_MD);
    write(
        &pair.main,
        CALL_MD,
        text.replace(&format!("\n{section}"), ""),
    );
    pair.commit_all(&pair.main, "the section gone, by hand");
    reads(&pair, &id, "suspect", Some(CALL));
}

/// The moved-text sequence: frozen plain at N0, a section added, `--editorial`
/// moves a sentence out of the document's own text into it, the section deleted
/// by hand. The sentence is gone and the own hash moved from the one the entry
/// holds, so an own pair would be held: `also` stays empty, no event, suspect.
/// M: `vouched` ignoring the kind.
#[test]
fn ac05_a_sentence_moved_into_a_section_then_deleted_marks() {
    const SENTENCE: &str = " It stays up a week.";
    const BOARD: &str = "It stands by the cross.";
    let pair = Pair::new("tt-ac05-moved-text", ALMANAC);
    replace(
        &pair.main,
        CALL_MD,
        "on the first of the month.\n",
        &format!("on the first of the month.{SENTENCE}\n"),
    );
    pair.commit_all(&pair.main, "a second sentence");
    let id = cut(&pair, "Notice posted", &[CALL]);
    let frozen = norm(&pair, CALL);
    assert_eq!(entry(&pair, &id)["within_hash"], frozen, "the premise");

    let section = format!("## The board {{#STEP-CALL-BOARD}}\n\n{BOARD}\n");
    let text = read_text(&pair.main, CALL_MD);
    write(&pair.main, CALL_MD, format!("{text}\n{section}"));
    pair.commit_all(&pair.main, "a section, by hand");
    assert_eq!(norm(&pair, CALL), frozen, "the premise: the own text held");
    reads(&pair, &id, "suspect", Some(CALL));

    let proposal = proposed(
        &pair,
        CALL,
        &[(SENTENCE, ""), (BOARD, &format!("{BOARD}{SENTENCE}"))],
    );
    approve_editorially(&pair, &proposal);
    assert_ne!(norm(&pair, CALL), frozen, "the premise: the sentence moved");

    let text = read_text(&pair.main, CALL_MD);
    let cut_at = text.find("\n## The board").expect("the section");
    write(&pair.main, CALL_MD, &text[..cut_at]);
    pair.commit_all(&pair.main, "the section gone, by hand");
    assert_eq!(pair.within(&[CALL]), norm(&pair, CALL), "the premise");

    assert_eq!(entry(&pair, &id)["also"], json!([]));
    assert_eq!(events_of(&pair, "card.refreshed"), Vec::<Value>::new());
    reads(&pair, &id, "suspect", Some(CALL));
}

/// "`--editorial` on a change that adds an `{#ID}` section vouches nothing": a
/// section coming on a sectioned document moves no node's own text, so the note
/// prints; no event, and the card keeps its mark. M: `within_pair` without the
/// member-name guard.
#[test]
fn ac05_an_editorial_pass_with_a_section_coming_keeps_the_mark() {
    let pair = Pair::new("tt-ac05-coming", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    let (_, text) = pair.span(&pair.main, SET);
    let proposal = created(&pair, SET, &format!("{text}\n{MARGIN_SECTION}"));
    let done = pair.spec_pty(
        &pair.main,
        &["approve", &proposal, "--editorial"],
        Some("y"),
    );
    assert_eq!(done.code, 0, "{}", done.output);
    assert!(
        done.output
            .contains("no node's meaning changes; --editorial changes nothing"),
        "{}",
        done.output
    );
    assert!(read_text(&pair.main, SET_MD).contains(MARGIN), "applied");
    assert_eq!(events_of(&pair, "card.refreshed"), Vec::<Value>::new());
    assert_eq!(entry(&pair, &id)["also"], json!([]));
    reads(&pair, &id, "suspect", Some(SET));
}

/// A section going under `--editorial` cannot be built: no proposal removes a
/// section. An update keeps every `{#ID}` heading, refused at `spec propose`
/// with nothing stored, and a create only adds. Its guard runs through a
/// completion: `ac05_a_completion_with_a_section_going_keeps_the_mark`.
#[test]
fn ac05_no_proposal_takes_a_section_away() {
    let pair = Pair::new("tt-ac05-going", ALMANAC);
    add_margin(&pair);
    let id = accepted(&pair, "Pages set", &[SET]);
    let (span, text) = pair.span(&pair.main, SET);
    let cut_at = text.find("\n## The margin").expect("the section");
    let reason = refused(
        &pair.propose(&pair.main, SET, &span, &text[..cut_at]),
        "a section going",
    );
    assert!(reason.contains("keeps every `{#ID}` heading"), "{reason}");
    assert_eq!(pair.proposals(), Vec::new(), "nothing stored");
    reads(&pair, &id, "accepted", None);
}

/// "A node with no sections sends both pairs, so an ordinary editorial stays
/// clean": a keyed entry takes the within pair, a legacy entry the own one, one
/// event each, here and in the worktree; a meaning change then marks both.
#[test]
fn ac05_an_ordinary_editorial_keeps_a_plain_nodes_cards_clean() {
    let pair = Pair::new("tt-ac05-plain", ALMANAC);
    let keyed = accepted(&pair, "Notice posted", &[CALL]);
    let legacy = accepted(&pair, "Notice read", &[CALL]);
    strip_key(&pair, &legacy);
    let proposal = editorial(&pair, CALL, CALL_MEANING);
    assert_eq!(
        events_of(&pair, "card.refreshed"),
        [
            json!({"id": keyed, "node": CALL, "proposal": proposal}),
            json!({"id": legacy, "node": CALL, "proposal": proposal}),
        ]
    );
    for id in [&keyed, &legacy] {
        assert_eq!(entry(&pair, id)["also"], json!([norm(&pair, CALL)]), "{id}");
        reads_in(&pair, &pair.main, id, "accepted", None);
        reads_in(&pair, &pair.linked, id, "accepted", None);
    }
    approved(&pair, CALL, TENTH);
    reads(&pair, &keyed, "suspect", Some(CALL));
    reads(&pair, &legacy, "suspect", Some(CALL));
}

/// A completion's rule on a legacy entry, which takes own pairs alone: on a
/// plain target and on a sectioned one, the node named `(editorial)`: clean,
/// one event, the own hash in `also`, no `within_hash` written. M: the `Own`
/// push of `named_refresh` deleted.
#[test]
fn ac05_a_completion_keeps_a_legacy_entry_clean() {
    for (label, target, path, meaning) in [
        ("tt-done-legacy-plain", CALL, CALL_MD, CALL_MEANING),
        ("tt-done-legacy-sections", SET, SET_MD, SET_MEANING),
    ] {
        let pair = Pair::new(label, ALMANAC);
        let id = accepted(&pair, "Legacy", &[target]);
        strip_key(&pair, &id);
        let proposal = proposed(&pair, target, &[meaning]);
        replace(&pair.main, path, meaning.0, meaning.1);
        let trailer = format!("{target} 1 (editorial)");
        completed_by_hand(&pair, &proposal, &[trailer.as_str()]);
        assert_eq!(
            events_of(&pair, "card.refreshed"),
            [json!({"id": id, "node": target, "proposal": proposal})],
            "{label}"
        );
        let held = entry(&pair, &id);
        assert_eq!(held["also"], json!([norm(&pair, target)]), "{label}");
        assert_eq!(held["within_hash"], Value::Null, "{label}: {held}");
        reads(&pair, &id, "accepted", None);
    }
}

/// "one `card.refreshed` per (card, node)", reached through a real pass: a card
/// cut on a section, the same `{#ID}` and text added after it (a card cannot be
/// cut on an ID already declared twice). `paired` sends one within pair per
/// holder, both held, both joining `also`: one event, `revision` one more. M:
/// `nodes` not deduplicated.
#[test]
fn ac05_a_duplicated_id_logs_one_event() {
    const SECTION: &str = "## The copy {#STEP-SET-COPY}\n\nTwo hundred.\n";
    let pair = Pair::new("tt-ac05-duplicated", ALMANAC);
    let text = read_text(&pair.main, SET_MD);
    write(&pair.main, SET_MD, format!("{text}\n{SECTION}"));
    pair.commit_all(&pair.main, "a section");
    let id = cut(&pair, "Copies", &["STEP-SET-COPY"]);
    let text = read_text(&pair.main, SET_MD);
    write(&pair.main, SET_MD, format!("{text}\n{SECTION}"));
    pair.commit_all(&pair.main, "its ID twice");
    reads(&pair, &id, "open", None);

    let before = revision(&pair, &id);
    let (span, text) = pair.span(&pair.main, SET);
    let new =
        text.replacen("Two hundred", "Three hundred", 1)
            .replacen("Two hundred", "Four hundred", 1);
    let proposal = pair
        .propose(&pair.main, SET, &span, &new)
        .expect("a proposal")
        .document
        .id
        .clone()
        .expect("an ID");
    approve_editorially(&pair, &proposal);
    assert_eq!(
        events_of(&pair, "card.refreshed"),
        [json!({"id": id, "node": "STEP-SET-COPY", "proposal": proposal})]
    );
    assert_eq!(revision(&pair, &id), before + 1);
    let held = entry(&pair, &id);
    assert_eq!(
        held["also"].as_array().map(Vec::len),
        Some(2),
        "the premise: both pairs joined: {held}"
    );
    reads(&pair, &id, "open", None);
}

/// The guard no other path runs: a named section moving out of a named holder.
/// Every named node taken out, the members left match on both sides, so only
/// the member names stop the holder's within pair: its members changed, and a
/// card on it keeps its mark, no event. A section's move out cannot be an update,
/// so the proposal is a third section's and the move is the hand commit's. M:
/// `within_pair` without the member-name guard.
#[test]
fn ac05_a_completion_with_a_section_moving_out_of_its_holder_keeps_the_mark() {
    const NESTED: &str = "### The ink {#STEP-SET-PROOF-INK}\n\nBlack only.\n";
    const BESIDE: &str = "## The ink {#STEP-SET-PROOF-INK}\n\nBlack only.\n";
    let pair = Pair::new("tt-done-out", ALMANAC);
    replace(
        &pair.main,
        SET_MD,
        PROOF_LINE,
        &format!("{PROOF_LINE}\n{NESTED}"),
    );
    add_margin(&pair);
    let id = accepted(&pair, "Proof pulled", &[PROOF]);
    let own = norm(&pair, PROOF);
    assert_eq!(
        entry(&pair, &id)["within_hash"],
        pair.within(&[PROOF, INK]),
        "the premise: it holds the section"
    );

    let wider = ("Two fingers", "Three fingers");
    let proposal = proposed(&pair, MARGIN, &[wider]);
    replace(&pair.main, SET_MD, &format!("\n{NESTED}"), "");
    replace(&pair.main, SET_MD, wider.0, wider.1);
    let text = read_text(&pair.main, SET_MD);
    write(&pair.main, SET_MD, format!("{text}\n{BESIDE}"));
    completed_by_hand(
        &pair,
        &proposal,
        &[
            "STEP-SET-PROOF 1 (editorial)",
            "STEP-SET-PROOF-INK 1 (editorial)",
            "STEP-SET-MARGIN 1 (editorial)",
        ],
    );
    assert_eq!(norm(&pair, PROOF), own, "the premise: its own text held");
    assert_eq!(events_of(&pair, "card.refreshed"), Vec::<Value>::new());
    assert_eq!(entry(&pair, &id)["also"], json!([]));
    reads(&pair, &id, "suspect", Some(PROOF));
}

/// "Rules and edge cases", `spec-trace.md` "Known limits" holds: `paired` takes
/// nodes with an ID, so an ID-less document's editorial pass still reads
/// suspect: marking falsely, never fresh.
#[test]
fn ac05_an_id_less_documents_pass_still_reads_suspect() {
    let pair = Pair::new("tt-ac05-loose", ALMANAC);
    add_loose(&pair);
    let id = cut(&pair, "Notes kept", &[LOOSE_MD]);
    editorial(&pair, RULE, ("for a week", "for a month"));
    assert_eq!(events_of(&pair, "card.refreshed"), Vec::<Value>::new());
    reads(&pair, &id, "suspect", Some(LOOSE_MD));
}

/// "`also`, the last `ALSO_MAX`": seventeen passes on a sectioned document,
/// one `within_hash` each, the kind the entry compares: the last sixteen kept
/// in order, no own hash among them, the card still clean, here and in the
/// worktree, whose text the frozen entry holds.
#[test]
fn ac05_also_keeps_the_last_sixteen_within_hashes() {
    let pair = Pair::new("tt-ac05-also", ALMANAC);
    let id = cut(&pair, "Pages set", &[SET]);
    let frozen = entry(&pair, &id);
    let mut hashes = Vec::new();
    let mut last = "right-hand page".to_owned();
    for number in 0..17 {
        let next = format!("right-hand page {number}");
        editorial(&pair, SET, (&last, &next));
        hashes.push(pair.within(&[SET, PROOF]));
        last = next;
    }
    let held = entry(&pair, &id);
    assert_eq!(held["also"], json!(hashes[1..].to_vec()), "{held}");
    assert!(
        !held["also"]
            .as_array()
            .expect("also")
            .contains(&json!(norm(&pair, SET))),
        "{held}"
    );
    assert_eq!(held["within_hash"], frozen["within_hash"], "{held}");
    for cwd in [&pair.main, &pair.linked] {
        reads_in(&pair, cwd, &id, "open", None);
    }
}

// ------------------------------------------------------------------- AC-06

/// AC-06: `--no-effect` clears a mark made under a section: `from` != `to`
/// though the own hash held, `rev` the target's own, the entry frozen anew;
/// the next meaning change marks again. M: `cleared` freezing `norm_hash`
/// alone.
#[test]
fn ac06_a_clear_freezes_the_nested_hash_and_the_next_change_marks() {
    let pair = Pair::new("tt-ac06", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    let (own, from) = (norm(&pair, SET), pair.within(&[SET, PROOF]));
    approved(&pair, PROOF, PROOF_MEANING);
    reads(&pair, &id, "suspect", Some(SET));
    let to = pair.within(&[SET, PROOF]);
    assert_ne!(from, to, "the premise");
    assert_eq!(norm(&pair, SET), own, "the premise: the own hash held");

    let done = clear(&pair, &id, "Only the wording.");
    assert_eq!(done.code, 0, "{}", done.output);
    reads(&pair, &id, "accepted", None);
    assert_eq!(
        events_of(&pair, "card.cleared"),
        [json!({
            "id": id,
            "outcome": "no_effect",
            "nodes": [{"id": SET, "rev": 1, "from": from, "to": to}],
            "correction": null,
            "reason": "Only the wording.",
            "as": "ben",
        })]
    );
    let frozen = entry(&pair, &id);
    assert_eq!(frozen["within_hash"], to, "{frozen}");
    assert_eq!(frozen["norm_hash"], own, "{frozen}");
    assert_eq!(frozen["also"], json!([]), "{frozen}");

    approved(&pair, PROOF, PROOF_AGAIN);
    reads(&pair, &id, "suspect", Some(SET));
}

/// AC-06, "WHEN REF is the section, nothing qualifies and it exits 1":
/// re-freezing would swallow other sections' changes. `--group STEP-SET-PROOF`
/// changes nothing; `--group STEP-SET` clears.
#[test]
fn ac06_a_group_on_the_section_refuses_and_on_the_document_clears() {
    let pair = Pair::new("tt-ac06-group", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    approved(&pair, PROOF, PROOF_MEANING);
    reads(&pair, &id, "suspect", Some(SET));

    let before = state(&pair);
    let done = group(&pair, PROOF);
    assert_eq!(done.code, 1, "{}", done.output);
    assert!(
        done.output
            .contains(&format!("no suspect card under `{PROOF}`")),
        "{}",
        done.output
    );
    assert_eq!(state(&pair), before, "nothing changed");
    reads(&pair, &id, "suspect", Some(SET));

    let done = group(&pair, SET);
    assert_eq!(done.code, 0, "{}", done.output);
    assert!(done.asked, "{}", done.output);
    reads(&pair, &id, "accepted", None);
    assert_eq!(
        entry(&pair, &id)["within_hash"],
        pair.within(&[SET, PROOF]),
        "frozen anew"
    );
}

/// AC-06, "names the document or above": a group above the document clears as
/// today.
#[test]
fn ac06_a_group_above_the_document_clears() {
    let pair = Pair::new("tt-ac06-above", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    approved(&pair, PROOF, PROOF_MEANING);
    let done = group(&pair, GATHER);
    assert_eq!(done.code, 0, "{}", done.output);
    reads(&pair, &id, "accepted", None);
    assert_eq!(events_of(&pair, "card.cleared").len(), 1);
}

/// AC-06: a correction's accept re-freezes the original: the mark held
/// `correcting` meanwhile, its entry untouched, `accepted` after it, one
/// `corrected` event with `from` != `to`.
#[test]
fn ac06_a_corrections_accept_refreezes_the_nested_hash() {
    let pair = Pair::new("tt-ac06-fix", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    let (frozen, from) = (entry(&pair, &id), pair.within(&[SET, PROOF]));
    approved(&pair, PROOF, PROOF_MEANING);
    let fix = needs_correction(&pair, &id);
    let row = row_in(&pair, &pair.main, &id);
    assert_eq!(row["mark"], "correcting", "{row}");
    assert_eq!(row["correction"], fix.as_str(), "{row}");
    assert_eq!(entry(&pair, &id), frozen, "the entry stands");

    to_review(&pair, &fix);
    accept_briefed(&pair, &fix);
    reads(&pair, &id, "accepted", None);
    let to = pair.within(&[SET, PROOF]);
    assert_eq!(entry(&pair, &id)["within_hash"], to);
    let cleared = events_of(&pair, "card.cleared");
    assert_eq!(cleared.len(), 2, "{cleared:?}");
    assert_eq!(cleared[1]["outcome"], "corrected", "{cleared:?}");
    assert_eq!(
        cleared[1]["nodes"],
        json!([{"id": SET, "rev": 1, "from": from, "to": to}])
    );

    approved(&pair, PROOF, PROOF_AGAIN);
    reads(&pair, &id, "suspect", Some(SET));
}

/// "An accept and a clear freeze the new hash": a suspect open card accepted
/// reads clean.
#[test]
fn ac06_an_accept_freezes_the_nested_hash() {
    let pair = Pair::new("tt-ac06-accept", ALMANAC);
    let id = cut(&pair, "Pages set", &[SET]);
    to_review(&pair, &id);
    append_under_proof(&pair);
    reads(&pair, &id, "suspect", Some(SET));
    accept_briefed(&pair, &id);
    reads(&pair, &id, "accepted", None);
    assert_eq!(entry(&pair, &id)["within_hash"], pair.within(&[SET, PROOF]));
}

// ------------------------------------------------------------------- AC-07

/// AC-07: the almanac reads `no card 5`: the book and its two parts hold live
/// documents with no card of their own, so they are counted nowhere, their row
/// word still `no card`; the lanes now sum to the header. M: keeping holders.
#[test]
fn ac07_a_holder_with_no_card_is_counted_nowhere() {
    let pair = Pair::new("tt-ac07", ALMANAC);
    let text = ok(&pair, &["trace"]);
    assert_eq!(
        text.lines().next(),
        Some(
            "trace all | nodes 8, cards 0 | no card 5, in work 0, suspect 0, departed 0, \
             confirmed 0"
        ),
        "{text}"
    );
    let shown = doc(&pair, &["trace"]);
    assert_eq!(shown["counts"], counts(5, 0, 0, 0, 0), "{shown}");
    for id in [BOOK, GATHER, PRINT] {
        assert_eq!(node_of(&shown, id)["rollup"], "no card", "{id}: {shown}");
    }
    for (id, held) in [(BOOK, 5), (GATHER, 2), (PRINT, 3), (SET, 1), (BIND, 1)] {
        assert_eq!(
            node_of(&shown, id)["counts"],
            counts(held, 0, 0, 0, 0),
            "{id}: {shown}"
        );
    }
    let lanes: Vec<Value> = shown["stages"]
        .as_array()
        .expect("stages")
        .iter()
        .map(|lane| lane["counts"].clone())
        .collect();
    assert_eq!(sum(&lanes), shown["counts"], "the lanes sum to it");
}

/// AC-07: the trace-views cycle: `PART-GATHER` counts (0,1,1,0,0), its row
/// still `no card`, in `spec trace` and folded in `spec flow --trace`;
/// `BOOK-ALMANAC`'s counts equal the sum of its parts'. M: dropping every
/// `no card`; keeping holders.
#[test]
fn ac07_a_book_counts_the_sum_of_its_parts() {
    let pair = Pair::new("tt-ac07-cycle", ALMANAC);
    a_cycle(&pair);
    let shown = doc(&pair, &["trace"]);
    let gather = node_of(&shown, GATHER);
    assert_eq!(gather["rollup"], "no card", "{gather}");
    assert_eq!(gather["counts"], counts(0, 1, 1, 0, 0), "{gather}");
    let print = node_of(&shown, PRINT);
    assert_eq!(print["rollup"], "no card", "{print}");
    assert_eq!(print["counts"], counts(1, 0, 0, 1, 1), "{print}");
    let book = node_of(&shown, BOOK);
    assert_eq!(book["rollup"], "no card", "{book}");
    assert_eq!(book["counts"], counts(1, 1, 1, 1, 1), "{book}");
    assert_eq!(
        book["counts"],
        sum([&gather["counts"], &print["counts"]]),
        "the sum of its parts"
    );
    assert_eq!(shown["counts"], book["counts"], "the book is the whole");

    let parts = doc(&pair, &["flow", BOOK, "--depth", "1", "--trace"]);
    assert_eq!(node_of(&parts, GATHER)["rollup"], "no card", "{parts}");
    assert_eq!(node_of(&parts, GATHER)["counts"], counts(0, 1, 1, 0, 0));
    let folded = doc(&pair, &["flow", BOOK, "--depth", "0", "--trace"]);
    assert_eq!(node_of(&folded, BOOK)["counts"], counts(1, 1, 1, 1, 1));
}

/// AC-07: a holder with a card counts: over itself and what it holds, in the
/// header, and in the book above it, whose counts still equal its parts'; a
/// card on the book itself counts it too. M: dropping holders with a card.
#[test]
fn ac07_a_holder_with_a_card_counts() {
    let pair = Pair::new("tt-ac07-card", ALMANAC);
    accepted(&pair, "The committee met", &[GATHER]);
    let shown = doc(&pair, &["trace"]);
    let gather = node_of(&shown, GATHER);
    assert_eq!(gather["rollup"], "confirmed", "{gather}");
    assert_eq!(gather["counts"], counts(2, 0, 0, 0, 1), "{gather}");
    let print = node_of(&shown, PRINT);
    assert_eq!(print["counts"], counts(3, 0, 0, 0, 0), "{print}");
    let book = node_of(&shown, BOOK);
    assert_eq!(book["counts"], sum([&gather["counts"], &print["counts"]]));
    assert_eq!(shown["counts"], counts(5, 0, 0, 0, 1), "{shown}");

    cut(&pair, "The book planned", &[BOOK]);
    let shown = doc(&pair, &["trace"]);
    let book = node_of(&shown, BOOK);
    assert_eq!(book["rollup"], "in work", "{book}");
    assert_eq!(book["counts"], counts(5, 1, 0, 0, 1), "itself: {book}");
    assert_eq!(shown["counts"], counts(5, 1, 0, 0, 1), "{shown}");
}

/// AC-07, D3: a document holding nothing counts `no card`: sections alone hold
/// nothing, so `STEP-SET` with none counts; a root document with no children
/// too. M: dropping every `no card`.
#[test]
fn ac07_a_document_holding_nothing_counts_no_card() {
    let pair = Pair::new("tt-ac07-leaf", ALMANAC);
    write(
        &pair.main,
        "docs/spec/step-loose.md",
        "---\nid: STEP-LOOSE\nclass: canon\ntitle: Loose\nowner: owner\nreviewed: 2026-10-01\n\
         ---\n\n# Loose\n\nA body.\n",
    );
    let shown = doc(&pair, &["trace"]);
    assert_eq!(shown["counts"], counts(6, 0, 0, 0, 0), "{shown}");
    for id in [SET, "STEP-LOOSE"] {
        let node = node_of(&shown, id);
        assert_eq!(node["rollup"], "no card", "{node}");
        assert_eq!(node["counts"], counts(1, 0, 0, 0, 0), "{node}");
    }
}

/// D3, "stopping at the first live document": a child hung on a section makes
/// the document a holder, walking through the section (`held_documents`
/// counts through sections or not), so it is counted nowhere.
#[test]
fn ac07_a_child_on_a_section_makes_a_holder() {
    let pair = Pair::new("tt-ac07-section", ALMANAC);
    write(
        &pair.main,
        "docs/spec/step-seal.md",
        plain_document("STEP-SEAL", PROOF),
    );
    let shown = doc(&pair, &["trace"]);
    assert_eq!(shown["counts"], counts(5, 0, 0, 0, 0), "{shown}");
    let set = node_of(&shown, SET);
    assert_eq!(set["rollup"], "no card", "{set}");
    assert_eq!(set["counts"], counts(1, 0, 0, 0, 0), "the seal: {set}");
    assert_eq!(
        node_of(&shown, GATHER)["counts"],
        counts(2, 0, 0, 0, 0),
        "CALL and the seal: {shown}"
    );
}

/// "A node holding live documents": a Tier 3 child is no live document, so a
/// node whose children are all Tier 3 holds nothing and counts `no card`.
#[test]
fn ac07_a_tier_3_child_holds_nothing() {
    let pair = Pair::new("tt-ac07-tier", ALMANAC);
    write(
        &pair.main,
        "docs/spec/step-old.md",
        "---\nid: STEP-OLD\nclass: spec\nstatus: shipped\ntitle: Old\nparent: STEP-BIND\n---\n\n\
         # Old\n\nDone with.\n",
    );
    let shown = doc(&pair, &["trace"]);
    assert_eq!(
        shown["counts"],
        counts(5, 0, 0, 0, 0),
        "BIND still counts: {shown}"
    );
    assert_eq!(node_of(&shown, BIND)["counts"], counts(1, 0, 0, 0, 0));
}

/// "Rules and edge cases": a `parent:` cycle is a finding, never a hang or a
/// panic of the walk that looks for holders.
#[test]
fn ac07_a_parent_cycle_answers() {
    let pair = Pair::new("tt-ac07-cycle-edge", ALMANAC);
    replace(
        &pair.main,
        "docs/spec/book-almanac.md",
        "title: The village almanac\n",
        "title: The village almanac\nparent: PART-GATHER\n",
    );
    let done = run(&pair, &["trace"]);
    done.code(0);
    assert!(
        done.stdout.starts_with("trace all | nodes 8, cards 0 | "),
        "{}",
        done.show()
    );
}

/// "Rules and edge cases": no document at all answers, every count 0.
#[test]
fn ac07_no_document_counts_nothing() {
    let empty = common::Scratch::new("tt-ac07-none");
    let root = empty.dir("root");
    write(
        &root,
        "specengine.toml",
        "[project]\nslug = \"empty-almanac\"\n",
    );
    let home = empty.home("h");
    let cwd = empty.dir("cwd");
    let root = root.to_str().expect("a UTF-8 path");
    let done = common::graph::spec30(&home, &cwd, &["trace", "--root", root]);
    done.code(0);
    assert_eq!(
        done.stdout,
        "trace all | nodes 0, cards 0 | no card 0, in work 0, suspect 0, departed 0, \
         confirmed 0\n"
    );
}

// ------------------------------------------------------------------- AC-08

/// AC-08: `spec card show C --marks` equals its list row on the four keys, on
/// every mark a row reads, the facts of other cards included (a correction, a
/// discrepancy); the rest of the document is plain `show`'s. M: keys not
/// joined in `run_show`.
#[test]
fn ac08_show_marks_equals_its_list_row() {
    let pair = Pair::new("tt-ac08", ALMANAC);
    let open = cut(&pair, "Entries sorted", &[SORT]);
    let clean = accepted(&pair, "Copies bound", &[BIND]);
    let suspect = accepted(&pair, "Pages set", &[SET]);
    let leaving = departed(&pair, "Entries read", READ);
    let corrected = accepted(&pair, "Notice posted", &[CALL]);
    let manual = cut(&pair, "A manual card", &[]);
    approved(&pair, PROOF, PROOF_MEANING);
    approved(&pair, CALL, CALL_MEANING);
    let correction = needs_correction(&pair, &corrected);

    for (id, word) in [
        (&open, "open"),
        (&clean, "accepted"),
        (&suspect, "suspect"),
        (&leaving, "departed"),
        (&corrected, "correcting"),
        (&correction, "open"),
        (&manual, "open"),
    ] {
        let row = row_in(&pair, &pair.main, id);
        assert_eq!(row["mark"], word, "{id}: {row}");
        let shown = doc(&pair, &["card", "show", id, "--marks"]);
        for key in MARK_KEYS {
            assert_eq!(shown[key], row[key], "{id}: {key}: {shown}");
        }
        let mut quiet = shown.clone();
        for key in MARK_KEYS {
            quiet[key] = Value::Null;
        }
        assert_eq!(quiet, show(&pair, id), "{id}: all but its four keys");
    }
    let shown = doc(&pair, &["card", "show", &suspect, "--marks"]);
    assert_eq!(shown["on"], SET, "{shown}");
    let shown = doc(&pair, &["card", "show", &leaving, "--marks"]);
    assert!(shown["discrepancy"].is_string(), "{shown}");
    let shown = doc(&pair, &["card", "show", &corrected, "--marks"]);
    assert_eq!(shown["correction"], correction.as_str(), "{shown}");
}

/// AC-08: plain `show` holds every one of the four keys, `null`, on a suspect
/// card: it reads no mark; `--history` keeps its own key beside `--marks`.
#[test]
fn ac08_plain_show_holds_null() {
    let pair = Pair::new("tt-ac08-plain", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    append_under_proof(&pair);
    reads(&pair, &id, "suspect", Some(SET));
    let shown = show(&pair, &id);
    for key in MARK_KEYS {
        assert!(shown.get(key).is_some(), "{key}: {shown}");
        assert_eq!(shown[key], Value::Null, "{key}: {shown}");
    }
    let both = doc(&pair, &["card", "show", &id, "--marks", "--history"]);
    assert_eq!(both["mark"], "suspect", "{both}");
    assert!(both["history"].is_array(), "{both}");
}

/// AC-08, "Text: a `key: value` line each, `-` for null, as any `show` key".
#[test]
fn ac08_the_text_prints_a_line_a_key() {
    let pair = Pair::new("tt-ac08-text", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    append_under_proof(&pair);
    let text = ok(&pair, &["card", "show", &id, "--marks"]);
    let lines: Vec<&str> = text.lines().collect();
    for line in [
        "mark: suspect",
        "on: STEP-SET",
        "discrepancy: -",
        "correction: -",
    ] {
        assert!(lines.contains(&line), "{line}: {text}");
    }
    let plain = ok(&pair, &["card", "show", &id]);
    let lines: Vec<&str> = plain.lines().collect();
    for line in ["mark: -", "on: -", "discrepancy: -", "correction: -"] {
        assert!(lines.contains(&line), "{line}: {plain}");
    }
}

/// "every key present, `null` without the flag and on a cancelled card": a
/// read of marks ignores a cancelled card, its target's change too.
#[test]
fn ac08_a_cancelled_card_holds_null() {
    let pair = Pair::new("tt-ac08-cancelled", ALMANAC);
    let id = cut(&pair, "Pages set", &[SET]);
    ok(&pair, &["card", "cancel", &id, "--reason", "Pulled."]);
    append_under_proof(&pair);
    let shown = doc(&pair, &["card", "show", &id, "--marks"]);
    for key in MARK_KEYS {
        assert!(shown.get(key).is_some(), "{key}: {shown}");
        assert_eq!(shown[key], Value::Null, "{key}: {shown}");
    }
    let text = ok(&pair, &["card", "show", &id, "--marks"]);
    assert!(text.lines().any(|line| line == "mark: -"), "{text}");
}

/// AC-08, D8: a `--marks` read over a broken index is the read's error, exit 2
/// and no clean card, where plain `show`, which reads no index, answers. M:
/// `--marks` swallowing the index error.
#[test]
fn ac08_a_broken_index_exits_2_and_shows_no_clean_card() {
    let pair = Pair::new("tt-ac08-index", ALMANAC);
    let id = cut(&pair, "Pages set", &[SET]);
    pair.sql("ALTER TABLE files RENAME TO files_gone;");
    for args in [
        vec!["card", "show", id.as_str(), "--marks"],
        vec!["--json", "card", "show", id.as_str(), "--marks"],
    ] {
        let done = run(&pair, &args);
        done.code(2);
        assert_eq!(done.stdout, "", "{}", done.show());
        assert!(done.stderr.contains("index"), "{}", done.show());
    }
    let plain = run(&pair, &["--json", "card", "show", &id]);
    plain.code(0);
    assert_eq!(plain.json()["mark"], Value::Null, "{}", plain.show());
}

/// "`spec card show C --marks`", an answer with no card: exit 1 and the same
/// `{id, reason}` as plain `show`, no key added.
#[test]
fn ac08_no_card_answers_as_plain_show() {
    let pair = Pair::new("tt-ac08-none", ALMANAC);
    let plain = run(&pair, &["--json", "card", "show", "C-0099"]);
    let marks = run(&pair, &["--json", "card", "show", "C-0099", "--marks"]);
    plain.code(1);
    marks.code(1);
    assert_eq!(marks.stdout, plain.stdout, "{}", marks.show());
    assert_eq!(
        marks.json(),
        json!({"id": "C-0099", "reason": "no card C-0099"})
    );
}

// ---------------------------------------------- the rest of the spec's rules

/// "Schema": no queue step: 9 stays, the column holds JSON.
#[test]
fn no_queue_step() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 9);
    let pair = Pair::new("tt-schema", ALMANAC);
    let id = cut(&pair, "Pages set", &[SET]);
    assert_eq!(pair.sql("PRAGMA user_version;").trim(), "9");
    let raw = pair.sql(&format!("SELECT snapshot FROM cards WHERE id = '{id}';"));
    let stored: Value = serde_json::from_str(raw.trim()).expect("the column holds JSON");
    assert!(stored[0]["within_hash"].is_string(), "{stored}");
}

/// "Determinism": a repeat of every read gives the same bytes and leaves the
/// queue as it was; two copies freeze the same `within_hash`: it names no
/// root.
#[test]
fn a_repeat_gives_the_same_bytes() {
    let pair = Pair::new("tt-repeat", ALMANAC);
    let id = accepted(&pair, "Pages set", &[SET]);
    a_cycle(&pair);
    append_under_proof(&pair);
    let before = state(&pair);
    for args in [
        vec!["trace"],
        vec!["--json", "trace"],
        vec!["trace", GATHER],
        vec!["--json", "flow", "--trace"],
        vec!["card", "list", "--marks"],
        vec!["--json", "card", "list", "--marks"],
        vec!["card", "show", id.as_str(), "--marks"],
        vec!["--json", "card", "show", id.as_str(), "--marks"],
    ] {
        assert_eq!(ok(&pair, &args), ok(&pair, &args), "{args:?}");
    }
    assert_eq!(state(&pair), before);

    let other = Pair::new("tt-repeat-other", ALMANAC);
    let again = cut(&other, "Pages set", &[SET]);
    let fresh = Pair::new("tt-repeat-fresh", ALMANAC);
    let third = cut(&fresh, "Pages set", &[SET]);
    assert_eq!(
        entry(&other, &again)["within_hash"],
        entry(&fresh, &third)["within_hash"]
    );
}

/// ADR-0008: the same rules on a print shop, another genre and another flow: a
/// card on a sheet marked from its section, a clean sibling sheet, the weekly
/// run counted nowhere, `show --marks` agreeing, in that project's own words.
#[test]
fn a_print_shop_reads_alike() {
    let pair = Pair::new("tt-press", PRESS);
    let front = cut_in(&pair, "copy", "Lead story", &["SHT-FRONT"]);
    let inner = cut_in(&pair, "copy", "Long read", &["SHT-INNER"]);
    replace(
        &pair.main,
        "docs/spec/sht-front.md",
        "The masthead stays the same from week to week.\n",
        "The masthead stays the same from week to week.\nThe date changes.\n",
    );
    reads(&pair, &front, "suspect", Some("SHT-FRONT"));
    reads(&pair, &inner, "open", None);
    let shown = doc(&pair, &["card", "show", &front, "--marks"]);
    assert_eq!(shown["on"], "SHT-FRONT", "{shown}");

    let tree = doc(&pair, &["trace", "PRN-WEEKLY"]);
    let weekly = node_of(&tree, "PRN-WEEKLY");
    assert_eq!(weekly["rollup"], "no card", "{weekly}");
    assert_eq!(
        weekly["counts"],
        counts(2, 1, 1, 0, 0),
        "plates, back, inner, front: {weekly}"
    );
    assert_eq!(tree["counts"], weekly["counts"], "{tree}");
}
